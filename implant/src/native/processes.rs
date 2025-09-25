//! Native interactions with Windows Processes

use serde::Serialize;
use shared::{
    process::Process,
    tasks::{Task, WyrmResult},
};
use std::{mem::MaybeUninit, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FALSE, GetLastError},
    System::{
        ProcessStatus::{EnumProcesses, GetModuleBaseNameW},
        Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE, PROCESS_VM_READ,
            TerminateProcess,
        },
    },
};

pub fn running_process_details() -> Option<impl Serialize> {
    // Get the pids; if we fail to do so, quit
    let pids = get_pids().ok()?;

    // Convert the pids to Process types, and return the Option containing the Vec<Process>
    pids_to_processes(pids)
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

/// Converts a Vector of pids to pid:name type [`Process`]
fn pids_to_processes(pids: Vec<u32>) -> Option<Vec<Process>> {
    let mut processes = Vec::new();

    for pid in pids {
        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid) };

        if handle.is_null() {
            continue;
        }

        // Zero memset initialise a stack buffer to write the process name to
        let buf: MaybeUninit<[u16; 260]> = MaybeUninit::uninit();
        let mut buf = unsafe { buf.assume_init() };
        // Calculate the size, which is in terms of chars. As we are wide; we need to divide the mem by 2
        let sz: u32 = buf.len() as u32 / 2;

        // SAFETY: Handle is valid at this point, we check the null status above
        let name_len = unsafe { GetModuleBaseNameW(handle, null_mut(), buf.as_mut_ptr(), sz) };

        let _ = unsafe { CloseHandle(handle) };

        if name_len == 0 {
            continue;
        }

        let name = String::from_utf16_lossy(&buf[..name_len as usize]);

        processes.push(Process { pid, name });
    }

    if processes.is_empty() {
        return None;
    }

    Some(processes)
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
        use shared::pretty_print::print_success;
        print_success(format!("Successfully terminated process {pid}"));
    }

    Some(WyrmResult::Ok(pid.to_string()))
}
