use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::tasks::Command;
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
        Command::RegQuery => "reg query",
        Command::RegAdd => "reg add",
    };

    c.into()
}
