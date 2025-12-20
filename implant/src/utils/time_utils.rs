use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;

pub fn epoch_now() -> i64 {
    unsafe {
        let mut ft: u64 = 0;
        GetSystemTimeAsFileTime(&mut ft as *mut u64 as *mut _);
        ((ft - 116444736000000000) / 10000000) as i64
    }
}
