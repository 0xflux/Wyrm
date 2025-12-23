use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use leptos::prelude::RwSignal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::{
    stomped_structs::{Process, RegQueryResult},
    tasks::{Command, PowershellOutput, WyrmResult},
};

use crate::{
    controller::{
        delete_item_in_browser_store, get_item_from_browser_store, store_item_in_browser_store,
        wyrm_chat_history_browser_key,
    },
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TabConsoleMessages {
    pub completed_id: i32,
    pub event: String,
    pub time: String,
    pub messages: Vec<String>,
}

impl TabConsoleMessages {
    /// Creates a new `TabConsoleMessages` event where the result isn't something that has come about from interacting
    /// with an agent.
    ///
    /// This could be used for commands which just require some form of response back to the user, from the C2 or locally
    /// within the client itself.
    pub fn non_agent_message(event: String, message: String) -> Self {
        Self {
            completed_id: 0,
            event,
            time: "-".into(),
            messages: vec![message],
        }
    }
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
            completed_id: notification_data.completed_id,
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
        Command::DotEx => "DotEx",
        Command::ConsoleMessages => "Agent console messages",
        Command::WhoAmI => "whoami",
        Command::Spawn => "Spawn",
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
                        return vec![format!("No data returned from ps command.")];
                    }
                };

                let deser: Option<Vec<Process>> =
                    serde_json::from_str(listings_serialised).unwrap();
                if deser.is_none() {
                    return vec![format!("Process listings empty.")];
                }

                let mut builder = vec![];

                const PID_W: usize = 10;
                const PPID_W: usize = 10;
                const NAME_W: usize = 40;
                const USER_W: usize = 16;

                let pid = "PID:";
                let ppid = "PPID:";
                let name = "Name:";
                let user = "User:";
                let f = format!(
                    "{:<PID_W$}{:<PPID_W$}{:<NAME_W$}{:<USER_W$}",
                    pid, ppid, name, user
                );
                builder.push(f);

                for row in deser.unwrap() {
                    let f = format!(
                        "{:<PID_W$}{:<PPID_W$}{:<NAME_W$}{:<USER_W$}",
                        row.pid, row.ppid, row.name, row.user
                    );
                    builder.push(f);
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
                if let Some(response) = &self.result {
                    match RegQueryResult::try_from(response.as_str()) {
                        Ok(r) => return r.client_print_formatted(),
                        Err(e) => return e,
                    }
                } else {
                    return vec!["No data.".to_string()];
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
            Command::DotEx => {
                if let Some(response) = &self.result {
                    let deser = match serde_json::from_str::<WyrmResult<String>>(response) {
                        Ok(i) => i,
                        Err(e) => {
                            return vec![format!(
                                "Could not deserialise response, {e}. Got raw: {response:?}"
                            )];
                        }
                    };

                    match deser {
                        WyrmResult::Ok(msg) => {
                            return vec![msg];
                        }
                        WyrmResult::Err(e) => {
                            return vec![format!("Error whilst trying to execute dotex: {e}")];
                        }
                    }
                } else {
                    return vec!["No data.".to_owned()];
                }
            }
            Command::ConsoleMessages => {
                if let Some(ser) = &self.result {
                    let deser = serde_json::from_str::<Vec<u8>>(&ser).unwrap();
                    let s = String::from_utf8_lossy(&deser);
                    return vec![s.to_string()];
                }
            }
            Command::WhoAmI => {
                if let Some(msg) = &self.result {
                    let s = serde_json::from_str::<WyrmResult<String>>(msg).unwrap();
                    match s {
                        WyrmResult::Ok(s) => return vec![s],
                        WyrmResult::Err(e) => return vec![format!("Error: {e}")],
                    }
                } else {
                    return vec!["An error occurred. See console output.".to_string()];
                }
            }
            Command::Spawn => {
                if let Some(msg) = &self.result {
                    let s = serde_json::from_str::<WyrmResult<String>>(msg).unwrap();
                    match s {
                        WyrmResult::Ok(s) => return vec![s],
                        WyrmResult::Err(e) => return vec![format!("Error: {e}")],
                    }
                } else {
                    return vec!["An error occurred. See console output.".to_string()];
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
    match serde_json::from_str::<WyrmResult<String>>(encoded_data) {
        Ok(wyrm_result) => match wyrm_result {
            WyrmResult::Ok(d) => vec![d],
            WyrmResult::Err(e) => vec![format!("An error occurred: {e}")],
        },
        Err(e) => {
            vec![format!(
                "Could not deserialise response: {e}. Got: {encoded_data:#?}"
            )]
        }
    }
}

/// Tracks the set of open tabs and which tab is currently active.
///
/// Used to maintain tab state in the UI, where `tabs` contains all open tab identifiers
/// and `active_id` points to the currently selected tab (if any).
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct ActiveTabs {
    pub tabs: HashSet<String>,
    pub active_id: Option<String>,
}

impl ActiveTabs {
    /// Instantiates a new [`ActiveTabs`] from the store. If it did not exist, a new [`ActiveTabs`] will be
    /// created.
    pub fn from_store() -> Self {
        get_item_from_browser_store(TAB_STORAGE_KEY).unwrap_or_default()
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
        self.active_id = Some(name.clone());
        let _ = self.save_to_store();
    }

    /// Removes a tab to the tracked tabs, doing nothing if the value did not exists
    pub fn remove_tab(&mut self, name: &str) {
        self.active_id = None;
        let _ = self.tabs.remove(name);
        let key = wyrm_chat_history_browser_key(name);
        delete_item_in_browser_store(&key);
        let _ = self.save_to_store();
    }
}

/// Information we wish to pull out of the agent ID, which has the format
/// `hostname|serial|username|integrity|pid|epoch`. This information is used by
/// the DB to uniquely identify each agent.
pub enum AgentIdSplit {
    Hostname,
    Integrity,
    Username,
}

/// Get a `String` of the component from a custom deserialisation of the Agent's ID string.
pub fn get_info_from_agent_id<'a>(haystack: &'a str, needle: AgentIdSplit) -> Option<&'a str> {
    let parts: Vec<&str> = haystack.split('|').collect();
    // How many variants the enum `AgentIdSplit` has, to make sure we are dealing with good data.
    const MAX_VARIANTS: usize = 3;

    if parts.len() < MAX_VARIANTS {
        return None;
    }

    // WARNING: This is highly dependant on the Agent ID not changing positional chars. If bugs appear,
    // its almost certain because the ordering of delimited args are in the str.
    let extracted_slice = match needle {
        AgentIdSplit::Hostname => parts[0],
        AgentIdSplit::Integrity => parts[3],
        AgentIdSplit::Username => parts[2],
    };

    Some(extracted_slice)
}

pub fn get_agent_tab_name(haystack: &str) -> Option<String> {
    let parts: Vec<&str> = haystack.split('|').collect();
    // We want to make sure we have enough parts collected
    const MAX_VARIANTS: usize = 5;

    if parts.len() < MAX_VARIANTS {
        return None;
    }

    Some(format!(
        "{username}@{hostname} [{integrity}] - {pid}",
        integrity = parts[3],
        username = parts[2],
        hostname = parts[0],
        pid = parts[4],
    ))
}

pub fn resolve_tab_to_agent_id(
    tab: &str,
    agent_map: &HashMap<String, RwSignal<Agent>>,
) -> Option<String> {
    if agent_map.contains_key(tab) {
        return Some(tab.to_string());
    }

    agent_map
        .keys()
        .find(|id| get_agent_tab_name(id).as_deref() == Some(tab))
        .cloned()
}
