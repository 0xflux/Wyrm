use windows_sys::Win32::Foundation::MAX_PATH;

/// Generates a safe system `Global` mutex name given an input string.
///
/// **IMPORTANT NOTE**: This function is copied (for convenience) between loader and implant for generating a matching
/// mutex name (because of nostd and shared library limits [im being lazy]). **THEREFORE** if there is a change to the
/// logic in this function it **MUST** !!!!!!!!!!!! be reflected in both crates.
#[allow(unused)]
pub fn generate_mutex_name(mutex: &str) -> [u8; MAX_PATH as usize] {
    let mut mtx_name = [0u8; MAX_PATH as usize];
    let mut cursor: usize = 0;
    const GLOBAL_PREFIX_STR: &[u8] = br"Global\";

    for b in GLOBAL_PREFIX_STR {
        mtx_name[cursor] = *b;
        cursor += 1;
    }

    // Need to be very careful to check we aren't going to overflow the buffer in a way which wont panic
    // as a panic will lead to an infinite loop happening in the panic handler.
    let max_mutex_len = (MAX_PATH as usize)
        .saturating_sub(GLOBAL_PREFIX_STR.len())
        .saturating_sub(1);
    let mutex_bytes = mutex.as_bytes();
    let copy_len = mutex_bytes.len().min(max_mutex_len);

    // Now safely copy into the buffer
    mtx_name[cursor..cursor + copy_len].copy_from_slice(&mutex_bytes[..copy_len]);
    cursor += copy_len;

    // Add a null termiantor
    if cursor < MAX_PATH as usize {
        mtx_name[cursor] = 0;
    };

    mtx_name
}
