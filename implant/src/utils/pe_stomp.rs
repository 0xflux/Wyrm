/// Given an input mutable buffer, stomps the first 50 bytes at hte `MZ` point, and
/// the "This program cannot be run in DOS mode...".
///
/// The function operates mutably on the input buffer.
pub fn stomp_pe_header_bytes(buf: &mut Vec<u8>) {
    // overwrite the MZ header but keeping the e_lfanew
    const MAX_OVERWRITE_END: usize = 50;
    buf[0..MAX_OVERWRITE_END].fill(0);

    // overwrite the THIS PROGRAM CANNOT BE RUN IN DOS MODE...
    const RANGE_START: usize = 0x4E;
    const RANGE_END: usize = 0x73;
    buf[RANGE_START..RANGE_END].fill(0);
}
