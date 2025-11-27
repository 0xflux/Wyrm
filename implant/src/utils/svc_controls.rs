use std::{
    ffi::c_void,
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::Services::{
        SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_HANDLE,
        SERVICE_STOPPED, SERVICE_WIN32_OWN_PROCESS, SetServiceStatus,
    },
};

use crate::entry::IS_IMPLANT_SVC;

pub static SERVICE_STOP_EVENT: AtomicBool = AtomicBool::new(false);
pub static SERVICE_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// Update the service status in the SCM
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

/// In the event the implant is built as a service, attempt to cleanly stop the service and
/// cleanly exit
pub fn stop_svc_and_exit() -> ! {
    let h_svc = SERVICE_HANDLE.load(Ordering::SeqCst);

    unsafe {
        if !IS_IMPLANT_SVC.load(Ordering::SeqCst) || h_svc.is_null() {
            std::process::exit(-2);
        }

        update_service_status(h_svc, SERVICE_STOPPED);
    }

    std::process::exit(0);
}
