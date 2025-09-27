use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use crate::{api::dashboard::ConnectedAgentData, net::Credentials};

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
}

#[derive(Debug, Clone, Default)]
pub struct TabConsoleMessages {
    pub event: String,
    pub time: String,
    pub messages: Option<Vec<String>>,
}

/// Tuple which, in order of params, tracks the index of the open tab
/// and a vector of agent ID's.
pub type ActiveTabData = (usize, Vec<String>);

pub struct AppState {
    pub creds: RwLock<Option<Credentials>>,
    pub connected_agents: RwLock<ConnectedAgentData>,
    pub active_tabs: RwLock<ActiveTabData>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            creds: RwLock::new(Some(Credentials {
                username: "flux".into(),
                password: "password".into(),
                admin_env_token: "fdgiyh%^l!udjfh78364LU7&%df!!".into(),
                c2_url: "http://127.0.0.1:8080".into(),
            })),
            connected_agents: RwLock::new(ConnectedAgentData::default()),
            active_tabs: RwLock::new((0, vec!["Server".into()])),
        }
    }
}
