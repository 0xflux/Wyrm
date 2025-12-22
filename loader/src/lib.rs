#![no_std]
#![no_main]

use windows_sys::Win32::{Foundation::HINSTANCE, System::SystemServices::DLL_PROCESS_ATTACH};

use crate::export_comptime::{StartType, internal_dll_start};

mod export_comptime;
mod injector;
mod utils;

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_hmod_instance: HINSTANCE, dw_reason: u32, _: usize) -> i32 {
    match dw_reason {
        DLL_PROCESS_ATTACH => internal_dll_start(StartType::DllMain),
        _ => (),
    }

    1
}
