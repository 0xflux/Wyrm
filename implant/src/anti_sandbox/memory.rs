use windows_sys::Win32::{
    Foundation::FALSE, System::SystemInformation::GetPhysicallyInstalledSystemMemory,
};

const MIN_ACCEPTABLE_MEMORY: u64 = 4000000; // ~ 4 GB

/// Checks the installed amount of memory, and panics if it's less than [`MIN_ACCEPTABLE_MEMORY`]
/// or if the WinAPI call failed.
pub fn validate_ram_sz_or_panic() {
    let mut total_memory: u64 = 0;

    if unsafe { GetPhysicallyInstalledSystemMemory(&mut total_memory) } == FALSE {
        #[cfg(debug_assertions)]
        {
            panic!("GetPhysicallyInstalledSystemMemory error")
        }

        panic!()
    }

    if total_memory < MIN_ACCEPTABLE_MEMORY {
        #[cfg(debug_assertions)]
        {
            panic!("Total memory ({total_memory}) was less than {MIN_ACCEPTABLE_MEMORY}")
        }

        panic!()
    }
}
