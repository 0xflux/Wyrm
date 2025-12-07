use std::{
    ffi::c_void,
    fs::File,
    io::Write,
    ptr::null_mut,
    sync::{
        Once,
        atomic::{AtomicPtr, Ordering},
    },
    thread::spawn,
};

use windows_sys::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::ReadFile,
    System::{
        Console::{AllocConsole, GetConsoleWindow, STD_OUTPUT_HANDLE, SetStdHandle},
        Pipes::CreatePipe,
    },
    UI::WindowsAndMessaging::{SW_HIDE, ShowWindow},
};

static INIT_PIPE: Once = Once::new();
pub static CONSOLE_PIPE_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

pub fn init_agent_console() {
    INIT_PIPE.call_once(|| {
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
                use shared::pretty_print::print_failed;
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
    spawn(|| unsafe {
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
                let mut f = File::options()
                    .write(true)
                    .append(true)
                    .open(r"C:\Users\ian\Documents\write_test.txt")
                    .unwrap();

                let _ = f.write(&buf[..bytes_read as usize]);
            }
            // log.extend_from_slice(&buf[..bytes_read as usize]);
        }
    });
}
