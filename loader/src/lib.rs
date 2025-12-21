#![no_std]
#![no_main]

use crate::injector::inject_current_process;

mod injector;

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn Run() -> i32 {
    inject_current_process();
    0
}
