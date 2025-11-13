use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::{
    process::Process,
    tasks::{Command, PowershellOutput, WyrmResult},
};

use crate::{
    controller::{get_item_from_browser_store, store_item_in_browser_store},
    models::TAB_STORAGE_KEY,
};

/// A representation of in memory agents on the C2, being a tuple of:
/// - `String`: Agent display representation
/// - `bool`: Is stale
/// - `Option<Value>`: Any new notifications
pub type AgentC2MemoryNotifications = (String, bool, Option<Value>);

/// A local client representation of an agent with a definition not shared across the
/// `Wyrm` ecosystem.
#[derive(Debug, Clone, Default)]
pub struct Agent {
    pub agent_id: String,
    pub last_check_in: DateTime<Utc>,
    pub pid: u32,
    pub process_name: String,
    // TODO
    // pub notification_status: NotificationStatus,
    pub is_stale: bool,
    /// Messages to be shown in the message box in the GUI
    pub output_messages: Vec<TabConsoleMessages>,
}

impl Agent {
    pub fn from(
        agent_id: String,
        last_check_in: DateTime<Utc>,
        pid: u32,
        process_name: String,
        is_stale: bool,
    ) -> Self {
        Self {
            agent_id,
            // notification_status: NotificationStatus::None,
            last_check_in,
            pid,
            process_name,
            is_stale,
            ..Default::default()
        }
    }

    pub fn from_messages(
        messages: Vec<NotificationForAgent>,
        agent_id: String,
        last_check_in: DateTime<Utc>,
        pid: u32,
        process_name: String,
        is_stale: bool,
    ) -> Self {
        let mut agent = Self::from(agent_id, last_check_in, pid, process_name, is_stale);

        let mut new_messages = vec![];

        for msg in messages {
            new_messages.push(TabConsoleMessages::from(msg));
        }

        agent.output_messages.append(&mut new_messages);

        agent
    }
}

#[derive(Debug, Clone, Default)]
pub struct TabConsoleMessages {
    pub event: String,
    pub time: String,
    pub messages: Vec<String>,
}

/// A representation of the database information pertaining to agent notifications which have not
/// yet been pulled by the operator.
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationForAgent {
    pub completed_id: i32,
    pub task_id: i32,
    pub command_id: i32,
    pub agent_id: String,
    pub result: Option<String>,
    pub time_completed_ms: i64,
}

impl From<NotificationForAgent> for TabConsoleMessages {
    fn from(notification_data: NotificationForAgent) -> Self {
        let cmd = Command::from_u32(notification_data.command_id as _);
        let cmd_string = command_to_string(&cmd);
        let result = notification_data.format_console_output();

        let time_seconds = if notification_data.time_completed_ms == 0 {
            let now = Utc::now();
            now.timestamp()
        } else {
            notification_data.time_completed_ms
        };

        // I am happy with the unwrap here, and I would prefer it over a default or half working product; if we make a change
        // to how time is represented then this will crash the client - forcing a bug fix. In any case, this should not be a real problem
        let time_utc_str = DateTime::from_timestamp(time_seconds, 0)
            .unwrap()
            .format("%d/%m/%Y %H:%M:%S")
            .to_string();

        Self {
            event: cmd_string,
            time: time_utc_str,
            messages: result,
        }
    }
}

/// Converts a [`Command`] to a `String`
fn command_to_string(cmd: &Command) -> String {
    let c = match cmd {
        Command::Sleep => "Sleep",
        Command::Ps => "ListProcesses",
        Command::GetUsername => "GetUsername",
        Command::Pillage => "Pillage",
        Command::UpdateSleepTime => "UpdateSleepTime",
        Command::Pwd => "Pwd",
        Command::AgentsFirstSessionBeacon => "AgentsFirstSessionBeacon",
        Command::Cd => "Cd",
        Command::KillAgent => "KillAgent",
        Command::Ls => "Ls",
        Command::Run => "Run",
        Command::KillProcess => "KillProcess",
        Command::Drop => "Drop",
        Command::Undefined => "Undefined",
        Command::Copy => "Copy",
        Command::Move => "Move",
        Command::Pull => "Pull",
        Command::RegQuery => "reg query",
        Command::RegAdd => "reg add",
        Command::RegDelete => "reg del",
        Command::RmFile => "RmFile",
        Command::RmDir => "RmDir",
    };

    c.into()
}

pub trait FormatOutput {
    fn format_console_output(&self) -> Vec<String>;
}

impl FormatOutput for NotificationForAgent {
    fn format_console_output(&self) -> Vec<String> {
        match Command::from_u32(self.command_id as _) {
            Command::Sleep => {
                return vec!["Agent received task to adjust sleep time.".into()];
            }
            Command::Ps => {
                let listings_serialised = match self.result.as_ref() {
                    Some(inner) => inner,
                    None => {
                        return vec![format!("No data returned from ls command.")];
                    }
                };

                let deser: Option<Vec<Process>> =
                    serde_json::from_str(listings_serialised).unwrap();
                if deser.is_none() {
                    return vec![format!("Directory listings empty.")];
                }

                let mut builder = vec![];

                for row in deser.unwrap() {
                    builder.push(format!("{}: {} ({})", row.pid, row.name, row.user));
                }

                return builder;
            }
            Command::GetUsername => (),
            Command::Pillage => {
                let result = match self.result.as_ref() {
                    Some(r) => r,
                    None => {
                        return vec!["No data.".into()];
                    }
                };

                let deser: Vec<String> = match serde_json::from_str(result) {
                    Ok(d) => d,
                    Err(e) => {
                        return vec![format!("Failed to deserialise results {e}.")];
                    }
                };

                return deser;
            }
            Command::UpdateSleepTime => (),
            Command::Undefined => {
                return vec!["Congrats, you found a bug. This should never print.".into()];
            }
            Command::Pwd => {
                let result = match self.result.as_ref() {
                    Some(r) => r,
                    None => {
                        return vec!["An error occurred with the data from pwd.".into()];
                    }
                };
                let s: String = match serde_json::from_str(result) {
                    Ok(s) => s,
                    Err(e) => format!(
                        "An error occurred whilst trying to unwrap. {e}. Data: {}",
                        result
                    ),
                };
                return vec![format!("{s}")];
            }
            Command::AgentsFirstSessionBeacon => (),
            Command::Cd => {
                let result = match self.result.as_ref() {
                    Some(r) => r,
                    None => {
                        return vec![format!("No data.")];
                    }
                };

                let deser: WyrmResult<PathBuf> = match serde_json::from_str(result) {
                    Ok(d) => d,
                    Err(e) => {
                        return vec![print_client_error(&format!(
                            "Ensure your request was properly formatted: {e}"
                        ))];
                    }
                };
                match deser {
                    WyrmResult::Ok(result) => return vec![result.as_path().try_strip_prefix()],
                    WyrmResult::Err(e) => return vec![print_client_error(&e)],
                }
            }
            Command::KillAgent => (),
            Command::Ls => {
                let listings_serialised = match self.result.as_ref() {
                    Some(inner) => inner,
                    None => {
                        return vec![format!("No data returned from ls command.")];
                    }
                };

                let deser: Option<Vec<PathBuf>> =
                    serde_json::from_str(listings_serialised).unwrap();
                if deser.is_none() {
                    return vec![format!("Directory listings empty.")];
                }

                let mut builder = vec![];

                for row in deser.unwrap() {
                    builder.push(row.as_path().try_strip_prefix());
                }

                return builder;
            }
            Command::Run => {
                let powershell_output: PowershellOutput = match &self.result {
                    Some(result) => match serde_json::from_str(result) {
                        Ok(result) => result,
                        Err(e) => {
                            return vec![format!("Could not deser PowershellOutput result. {e}")];
                        }
                    },
                    None => {
                        return vec!["No output returned from PowerShell command.".into()];
                    }
                };

                if let Some(out) = powershell_output.stderr
                    && !out.is_empty()
                {
                    return vec![format!("stderr: {out}")];
                }

                if let Some(out) = powershell_output.stdout
                    && !out.is_empty()
                {
                    return vec![format!("stdout: {out}")];
                }
            }
            Command::KillProcess => match &self.result {
                Some(s) => {
                    let result: WyrmResult<String> = match serde_json::from_str(s) {
                        Ok(r) => r,
                        Err(e) => {
                            return vec![format!(
                                "Could not serialise result for KillProcess. {e}."
                            )];
                        }
                    };

                    match result {
                        WyrmResult::Ok(s) => {
                            return vec![format!("Successfully killed process ID {s}.")];
                        }
                        WyrmResult::Err(e) => {
                            return vec![format!(
                                "An error occurred whilst trying to kill a process. {e}"
                            )];
                        }
                    }
                }
                None => {
                    return vec![
                        "An unknown error occurred whilst trying to kill a process.".into(),
                    ];
                }
            },
            Command::Drop => match &self.result {
                Some(s) => {
                    let result: WyrmResult<String> = match serde_json::from_str(s) {
                        Ok(r) => r,
                        Err(e) => {
                            return vec![format!("Could not serialise result. {e}.")];
                        }
                    };

                    if let WyrmResult::Err(e) = result {
                        return vec![format!(
                            "An error occurred whilst trying to drop a file. {e}"
                        )];
                    }

                    return vec![format!("File dropped successfully.")];
                }
                None => {
                    return vec!["An unknown error occurred whilst trying to drop a file.".into()];
                }
            },
            Command::Copy => {
                //
                // In the result we get back from the agent, Some("null") is representative of the success.
                // If `Some` != "null", contains a `WyrmError` that we can print.
                //
                if let Some(inner) = &self.result {
                    if inner == "null" {
                        return vec!["File copied.".into()];
                    }

                    if let Ok(e) = serde_json::from_str::<WyrmResult<String>>(inner) {
                        return vec![format!("An error occurred copying the file: {:?}", e)];
                    }
                }

                return vec!["File copied".into()];
            }
            Command::Move => {
                //
                // In the result we get back from the agent, Some("null") is representative of the success.
                // If `Some` != "null", contains a `WyrmError` that we can print.
                //
                if let Some(inner) = &self.result {
                    if inner == "null" {
                        return vec!["File moved.".into()];
                    }

                    if let Ok(e) = serde_json::from_str::<WyrmResult<String>>(inner) {
                        return vec![format!("An error occurred moving the file: {:?}", e)];
                    }
                }

                return vec!["File moved".into()];
            }
            Command::Pull => {
                if let Some(response) = &self.result {
                    if let Ok(msg) = serde_json::from_str::<String>(response) {
                        // If we had an error message from the implant
                        return vec![format!("Implant suffered error executing Pull. {msg}")];
                    } else {
                        return vec!["Unknown error.".into()];
                    }
                }

                return vec!["File exfiltrated successfully and can be found on the C2.".into()];
            }
            Command::RegQuery => {
                //
                // alright this deser is gross ...
                //
                if let Some(response) = &self.result {
                    match serde_json::from_str::<WyrmResult<String>>(&response) {
                        Ok(data) => match data {
                            WyrmResult::Ok(inner_string_from_result) => {
                                match serde_json::from_str::<Vec<String>>(&inner_string_from_result)
                                {
                                    Ok(results_as_vec) => return results_as_vec,
                                    Err(_) => {
                                        // Try as a single string (in the event it was querying an exact value)
                                        return vec![inner_string_from_result];
                                    }
                                }
                            }
                            WyrmResult::Err(e) => {
                                return vec![format!("Error with operation. {e}")];
                            }
                        },
                        Err(e) => {
                            return vec![format!("Could not deserialise response data. {e}.")];
                        }
                    }
                } else {
                    return vec!["No data returned, something may have gone wrong.".into()];
                }
            }
            Command::RegAdd => {
                if let Some(response) = &self.result {
                    return print_wyrm_result_string(response);
                } else {
                    return vec![format!("Unknown error. Got: {:#?}", self.result)];
                }
            }
            Command::RegDelete => {
                if let Some(response) = &self.result {
                    return print_wyrm_result_string(response);
                } else {
                    return vec![format!("Unknown error. Got: {:#?}", self.result)];
                }
            }
            Command::RmFile => {
                if let Some(response) = &self.result {
                    return print_wyrm_result_string(response);
                } else {
                    return vec![format!("Unknown error. Got: {:#?}", self.result)];
                }
            }
            Command::RmDir => {
                if let Some(response) = &self.result {
                    return print_wyrm_result_string(response);
                } else {
                    return vec![format!("Unknown error. Got: {:#?}", self.result)];
                }
            }
        }

        //
        // The fallthrough
        //
        match self.result.as_ref() {
            Some(result) => {
                vec![format!(
                    "[DISPLAY ERROR] Did not match / parse correctly. {result:?}"
                )]
            }
            None => {
                vec![format!("Action completed with no data to present.")]
            }
        }
    }
}

fn print_client_error(msg: &str) -> String {
    format!("Error: {msg}")
}

trait StripCannon {
    fn try_strip_prefix(&self) -> String;
}

impl StripCannon for Path {
    /// Where a path has been canonicalised, try strip the Windows \\?\ prefix for pretty
    /// printing.
    //
    // If this function fails, it will return the original path as a `String`
    fn try_strip_prefix(&self) -> String {
        let s = self.to_string_lossy().into_owned();
        if s.starts_with(r"\\?\") {
            let stripped = s.strip_prefix(r"\\?\").unwrap_or(&s);
            stripped.into()
        } else {
            s.into()
        }
    }
}

/// A helper function to print values when it is just a WyrmResult<String>
fn print_wyrm_result_string(encoded_data: &String) -> Vec<String> {
    match serde_json::from_str::<WyrmResult<String>>(&encoded_data) {
        Ok(wyrm_result) => match wyrm_result {
            WyrmResult::Ok(d) => return vec![d],
            WyrmResult::Err(e) => return vec![format!("An error occurred: {e}")],
        },
        Err(e) => {
            return vec![format!(
                "Could not deserialise response: {e}. Got: {encoded_data:#?}"
            )];
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ActiveTabs {
    pub tabs: HashSet<String>,
    pub active_id: Option<String>,
}

impl ActiveTabs {
    /// Instantiates a new [`ActiveTabs`] from the store. If it did not exist, a new [`ActiveTabs`] will be
    /// created.
    pub fn from_store() -> Self {
        match get_item_from_browser_store(TAB_STORAGE_KEY) {
            Ok(s) => s,
            Err(_) => Self::default(),
        }
    }

    /// Writes the current tab layout to the browser store
    pub fn save_to_store(&self) -> anyhow::Result<()> {
        store_item_in_browser_store(TAB_STORAGE_KEY, self)?;

        Ok(())
    }

    /// Adds a tab to the tracked tabs, doing nothing if the value already exists
    pub fn add_tab(&mut self, name: &str) {
        let name = name.to_string();
        let _ = self.tabs.insert(name.clone());
        let _ = self.save_to_store();
        self.active_id = Some(name);
    }

    /// Removes a tab to the tracked tabs, doing nothing if the value did not exists
    pub fn remove_tab(&mut self, name: &str) {
        self.active_id = None;
        let _ = self.tabs.remove(name);
        let _ = self.save_to_store();
    }
}
