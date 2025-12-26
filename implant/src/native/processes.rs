//! Native interactions with Windows Processes

use serde::Serialize;
use shared::{
    stomped_structs::Process,
    tasks::{Task, WyrmResult},
};
use std::{ffi::CStr, mem::MaybeUninit, ptr::null_mut};
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FALSE, GetLastError, HANDLE, TRUE},
    Security::{
        GetTokenInformation, LookupAccountSidW, PSID, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32, Process32First, Process32Next, TH32CS_SNAPALL,
        },
        ProcessStatus::EnumProcesses,
        Threading::{
            OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
            QueryFullProcessImageNameW, TerminateProcess,
        },
    },
};

use crate::utils::console::print_failed;
use crate::utils::strings::utf_16_to_string_lossy;

pub fn running_process_details() -> Option<impl Serialize> {
    // Get the pids; if we fail to do so, quit
    // let pids = get_pids().ok()?;

    // Convert the pids to Process types, and return the Option containing the Vec<Process>
    // pids_to_processes(pids)
    enum_all_processes()
}

/// Retrieves the PIDS of running processes
///
/// # Returns
/// - Ok - A vector of PIDs
/// - Err - The GetLastError code after calling EnumProcesses
fn get_pids() -> Result<Vec<u32>, u32> {
    const STARTING_NUM_ELEMENTS: usize = 1024;
    let mut pids = vec![0u32; STARTING_NUM_ELEMENTS];

    loop {
        let array_len = (pids.len() * size_of::<u32>()) as u32;
        let mut returned_len = 0;

        if unsafe { EnumProcesses(pids.as_mut_ptr(), array_len, &mut returned_len) } == 0 {
            return Err(unsafe { GetLastError() });
        }

        let num_pids = (returned_len as usize) / size_of::<u32>();

        if num_pids < pids.len() {
            pids.truncate(num_pids);
            return Ok(pids);
        }

        pids.resize(pids.len() * 2, 0);
    }
}

// /// Converts a Vector of pids to pid:name type [`Process`]
// fn pids_to_processes(pids: Vec<u32>) -> Option<Vec<Process>> {
//     let mut processes = Vec::new();

//     for pid in pids {
//         let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) };

//         if handle.is_null() {
//             continue;
//         }

//         let name = lookup_process_name(handle, pid);
//         let user = lookup_process_owner_name(handle, pid);

//         processes.push(Process { pid, name, user });
//         let _ = unsafe { CloseHandle(handle) };
//     }

//     if processes.is_empty() {
//         return None;
//     }

//     Some(processes)
// }

fn lookup_process_name(handle: HANDLE, pid: u32) -> String {
    const BUF_LEN: u32 = 512;
    // Zero memset initialise a stack buffer to write the process name to
    let buf: MaybeUninit<[u16; BUF_LEN as _]> = MaybeUninit::uninit();
    let mut buf = unsafe { buf.assume_init() };

    let mut sz: u32 = BUF_LEN;

    let result = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut sz) };

    if result == 0 {
        #[cfg(debug_assertions)]
        {
            print_failed(format!(
                "Failed to look up image name for pid {pid}. Error code: {:#X}",
                unsafe { GetLastError() }
            ));
        }

        return sc!("unknown", 87).unwrap();
    }

    let full_str = unsafe { utf_16_to_string_lossy(buf.as_ptr(), sz as _) };
    let parts: Vec<&str> = full_str.split('\\').collect();

    parts[parts.len() - 1].to_string()
}

fn lookup_process_owner_name(pid: u32) -> String {
    let mut token_handle: HANDLE = HANDLE::default();
    let mut user = String::new();

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) };

    if handle.is_null() {
        return String::new();
    }

    let result = unsafe { OpenProcessToken(handle, TOKEN_QUERY, &mut token_handle) } as u8;

    if result == 0 {
        #[cfg(debug_assertions)]
        {
            let gle = unsafe { GetLastError() };
            print_failed(format!(
                "Failed to initially open token on process {pid}. {gle:#X}"
            ));
        }
        return sc!("unknown", 78).unwrap();
    }

    let mut token_size = 0;
    unsafe { GetTokenInformation(token_handle, TokenUser, null_mut(), 0, &mut token_size) };

    //
    // If we received data, pull out the token info (gives us the users SID which we can convert to a username)
    //
    if token_size > 0 {
        let mut token_info: Vec<u8> = Vec::with_capacity(token_size as _);

        let result = unsafe {
            GetTokenInformation(
                token_handle,
                TokenUser,
                token_info.as_mut_ptr() as *mut _,
                token_size,
                &mut token_size,
            )
        };

        if result == 0 {
            #[cfg(debug_assertions)]
            {
                let gle = unsafe { GetLastError() };
                print_failed(format!(
                    "Failed to read token info on process {pid}. {gle:#X}"
                ));
            }
            unsafe { CloseHandle(token_handle) };
            return sc!("unknown", 78).unwrap();
        }

        //
        // At this point we have properly got the token info, it now needs parsing as a SID
        // and looking up.
        //
        let sid = unsafe { *(token_info.as_ptr() as *const TOKEN_USER) }
            .User
            .Sid as PSID;

        const BUF_LEN: u32 = 256;
        let mut name_tchars = BUF_LEN;
        let mut domain_tchars = BUF_LEN;
        let mut sid_type = SID_NAME_USE::default();

        // Zero memset initialise a stack buffer to write the process name to
        let wide_name: MaybeUninit<[u16; BUF_LEN as _]> = MaybeUninit::uninit();
        let mut wide_name = unsafe { wide_name.assume_init() };
        let wide_domain: MaybeUninit<[u16; BUF_LEN as _]> = MaybeUninit::uninit();
        let mut wide_domain = unsafe { wide_domain.assume_init() };

        let result = unsafe {
            LookupAccountSidW(
                null_mut(),
                sid,
                wide_name.as_mut_ptr(),
                &mut name_tchars,
                wide_domain.as_mut_ptr(),
                &mut domain_tchars,
                &mut sid_type,
            )
        };

        if result == 0 {
            #[cfg(debug_assertions)]
            {
                let gle = unsafe { GetLastError() };
                print_failed(format!("Failed to lookup account SID {pid}. {gle:#X}"));
            }

            return sc!("unknown", 78).unwrap();
        }

        //
        // Convert to a native string
        //
        user = unsafe { utf_16_to_string_lossy(wide_name.as_ptr(), name_tchars as _) };
    } else {
        #[cfg(debug_assertions)]
        {
            let gle = unsafe { GetLastError() };
            print_failed(format!(
                "No data received when trying to open token {pid}. {gle:#X}"
            ));
        }

        user = sc!("unknown", 78).unwrap();
    }

    unsafe { CloseHandle(token_handle) };

    user
}

/// Kills a process by its pid.
///
/// # Returns
///
/// ## On success
/// - `Some(WyrmResult(pid))` where the inner pid is the PID of the killed process.
///
/// ## On Error
/// - `None`: A non-descript silent error (to maintain some pattern OPSEC)#
/// - `Some(WyrmResult(String))`: An error which can be printed to the client
pub fn kill_process(pid: &Task) -> Option<WyrmResult<String>> {
    let pid: u32 = match pid.metadata.as_ref().unwrap().parse() {
        Ok(p) => p,
        Err(_) => return None,
    };

    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, pid as _) };
    if handle.is_null() {
        return Some(WyrmResult::Err(format!("Error code: {}", unsafe {
            GetLastError()
        })));
    }

    if unsafe { TerminateProcess(handle, 0) } == FALSE {
        let _ = unsafe { CloseHandle(handle) };
        return Some(WyrmResult::Err(format!("Error code: {}", unsafe {
            GetLastError()
        })));
    }

    let _ = unsafe { CloseHandle(handle) };

    #[cfg(debug_assertions)]
    {
        use crate::utils::console::print_success;

        print_success(format!("Successfully terminated process {pid}"));
    }

    Some(WyrmResult::Ok(pid.to_string()))
}

// fn sort_processes(processes: Vec<Process>) {
//     let mut sp = SortedProcesses(vec![]);
//     for p in processes {
//         if sp.0.is_empty() {
//             sp.0.insert(p.pid as usize, SortedProcess::from(p.clone()));
//             continue;
//         }
//     }
// }

fn enum_all_processes() -> Option<Vec<Process>> {
    let h_snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPALL, 0) };
    if h_snapshot.is_null() {
        return None;
    }

    let mut processes: Vec<Process> = Vec::new();

    let mut process_entry = PROCESSENTRY32::default();
    process_entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

    if unsafe { Process32First(h_snapshot, &mut process_entry) } == TRUE {
        loop {
            //
            // Get the process name
            //
            let current_process_name_ptr = process_entry.szExeFile.as_ptr() as *const _;
            let current_process_name =
                match unsafe { CStr::from_ptr(current_process_name_ptr) }.to_str() {
                    Ok(process) => process.to_string(),
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        print_failed(format!("Error converting process name. {e}"));

                        continue;
                    }
                };

            let pid = process_entry.th32ProcessID;

            let username = lookup_process_owner_name(pid);
            let ppid = process_entry.th32ParentProcessID;

            processes.push(Process {
                pid,
                name: current_process_name,
                user: username,
                ppid,
            });

            // continue enumerating
            if unsafe { Process32Next(h_snapshot, &mut process_entry) } == FALSE {
                break;
            }
        }
    }

    unsafe {
        let _ = CloseHandle(h_snapshot);
    };

    Some(processes)
}
