#![no_std]
#![no_main]

mod export_comptime;
mod injector;
mod utils;

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
