use std::slice::from_raw_parts;

/// Converts a WSTR to a String by the **number of chars** NOT the length in bytes.
///
/// # Safety
/// Pointers should be validated before passing into the function
pub unsafe fn utf_16_to_string_lossy(p_w_str: *const u16, num_chars: usize) -> String {
    let parts = unsafe { from_raw_parts(p_w_str, num_chars) };

    String::from_utf16_lossy(&parts)
}
