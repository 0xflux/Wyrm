use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::tasks::{Command, Task};
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
    pub time_completed_ms: i64,
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
        Command::RegQuery => "reg query",
        Command::RegAdd => "reg add",
        Command::RegDelete => "reg del",
        Command::RmFile => "RmFile",
        Command::RmDir => "RmDir",
        Command::DotEx => "DotEx",
        Command::ConsoleMessages => "Agent console messages",
    };

    c.into()
}

#[derive(Serialize)]
pub struct MitreTTP<'a> {
    ttp_major: &'a str,
    ttp_minor: Option<&'a str>,
    name: &'a str,
    link: &'a str,
}

impl<'a> MitreTTP<'a> {
    pub fn from(
        ttp_major: &'a str,
        ttp_minor: Option<&'a str>,
        name: &'a str,
        link: &'a str,
    ) -> Self {
        MitreTTP {
            ttp_major,
            ttp_minor,
            name,
            link,
        }
    }
}

pub trait MapToMitre<'a> {
    fn map_to_mitre(&'a self) -> MitreTTP<'a>;
}

impl<'a> MapToMitre<'a> for Command {
    fn map_to_mitre(&'a self) -> MitreTTP<'a> {
        match self {
            Command::Sleep => MitreTTP::from(
                "TA0011",
                None,
                "Command and Control",
                "https://attack.mitre.org/tactics/TA0011/",
            ),
            Command::Ps => MitreTTP::from(
                "T1057",
                None,
                "Process Discovery",
                "https://attack.mitre.org/techniques/T1057/",
            ),
            Command::GetUsername => MitreTTP::from(
                "T1033",
                None,
                "System Owner/User Discovery",
                "https://attack.mitre.org/techniques/T1033/",
            ),
            Command::Pillage => MitreTTP::from(
                "T1083",
                None,
                "File and Directory Discovery",
                "https://attack.mitre.org/techniques/T1083/",
            ),
            Command::UpdateSleepTime => MitreTTP::from(
                "TA0011",
                None,
                "Command and Control",
                "https://attack.mitre.org/tactics/TA0011/",
            ),
            Command::Pwd => MitreTTP::from(
                "T1083",
                None,
                "File and Directory Discovery",
                "https://attack.mitre.org/techniques/T1083/",
            ),
            Command::AgentsFirstSessionBeacon => MitreTTP::from(
                "TA0011",
                None,
                "Command and Control",
                "https://attack.mitre.org/tactics/TA0011/",
            ),
            Command::Cd => MitreTTP::from(
                "T1083",
                None,
                "File and Directory Discovery",
                "https://attack.mitre.org/techniques/T1083/",
            ),
            Command::KillAgent => MitreTTP::from(
                "T1070",
                None,
                "Indicator Removal",
                "https://attack.mitre.org/techniques/T1070/",
            ),
            Command::KillProcess => MitreTTP::from(
                "T1489",
                None,
                " Service Stop",
                "https://attack.mitre.org/techniques/T1489/",
            ),
            Command::Ls => MitreTTP::from(
                "T1083",
                None,
                "File and Directory Discovery",
                "https://attack.mitre.org/techniques/T1083/",
            ),
            Command::Run => MitreTTP::from(
                "T1059",
                Some("001"),
                "Command and Scripting Interpreter: PowerShell",
                "https://attack.mitre.org/techniques/T1059/001/",
            ),
            Command::Drop => MitreTTP::from(
                "T1105",
                None,
                "Ingress Tool Transfer",
                "https://attack.mitre.org/techniques/T1105/",
            ),
            Command::Copy => MitreTTP::from(
                "T1074",
                Some("001"),
                "Data Staged: Local Data Staging",
                "https://attack.mitre.org/techniques/T1074/001/",
            ),
            Command::Move => MitreTTP::from(
                "T1074",
                Some("001"),
                "Data Staged: Local Data Staging",
                "https://attack.mitre.org/techniques/T1074/001/",
            ),
            Command::RmFile => MitreTTP::from(
                "T1070",
                Some("004"),
                "Indicator Removal: File Deletion",
                "https://attack.mitre.org/techniques/T1070/004/",
            ),
            Command::RmDir => MitreTTP::from(
                "T1070",
                Some("004"),
                "Indicator Removal: File Deletion",
                "https://attack.mitre.org/techniques/T1070/004/",
            ),
            Command::Pull => MitreTTP::from(
                "T1041",
                None,
                "Exfiltration Over C2 Channel",
                "https://attack.mitre.org/techniques/T1041/",
            ),
            Command::RegQuery => MitreTTP::from(
                "T1012",
                None,
                "Query Registry",
                "https://attack.mitre.org/techniques/T1012/",
            ),
            Command::RegAdd => MitreTTP::from(
                "T1112",
                None,
                "Modify Registry",
                "https://attack.mitre.org/techniques/T1112/",
            ),
            Command::RegDelete => MitreTTP::from(
                "T1112",
                None,
                "Modify Registry",
                "https://attack.mitre.org/techniques/T1112/",
            ),
            Command::Undefined => MitreTTP::from("UNDEFINED", None, "UNDEFINED", "UNDEFINED"),
            Command::DotEx => MitreTTP::from(
                "T1620",
                None,
                "Reflective Code Loading",
                "https://attack.mitre.org/techniques/T1620/",
            ),
            Command::ConsoleMessages => MitreTTP::from(
                "TA0011",
                None,
                "Command and Control",
                "https://attack.mitre.org/tactics/TA0011/",
            ),
        }
    }
}

#[derive(Serialize)]
pub struct TaskExport<'a> {
    task: &'a Task,
    mitre: MitreTTP<'a>,
}

impl<'a> TaskExport<'a> {
    pub fn new(task: &'a Task, mitre: MitreTTP<'a>) -> Self {
        Self { task, mitre }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, FromRow)]
pub struct StagedResourceData {
    pub agent_name: String,
    pub c2_endpoint: String,
    pub staged_endpoint: String,
    pub pe_name: String,
    pub sleep_time: i64,
    pub port: i16,
    pub num_downloads: i64,
}
