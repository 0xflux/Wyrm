use chrono::{DateTime, Utc};
use shared::tasks::Command;
use shared_c2_client::{NotificationForAgent, command_to_string};
use tokio::sync::RwLock;

use crate::{api::dashboard::ConnectedAgentData, console_output::FormatOutput, net::Credentials};

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

impl TabConsoleMessages {
    /// Creates a new `TabConsoleMessages` event where the result isn't something that has come about from interacting
    /// with an agent.
    ///
    /// This could be used for commands which just require some form of response back to the user, from the C2 or locally
    /// within the client itself.
    pub fn non_agent_message(event: String, message: String) -> Self {
        Self {
            event,
            time: "-".into(),
            messages: vec![message],
        }
    }
}

/// Tuple which, in order of params, tracks the index of the open tab
/// and a vector of agent ID's.
pub type ActiveTabData = (usize, Vec<String>);

pub struct AppState {
    pub creds: RwLock<Option<Credentials>>,
    pub connected_agents: RwLock<ConnectedAgentData>,
    pub active_tabs: RwLock<ActiveTabData>,
    pub last_tabs_hash: RwLock<Option<u64>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            creds: RwLock::new(None),
            connected_agents: RwLock::new(ConnectedAgentData::default()),
            active_tabs: RwLock::new((0, vec!["Server".into()])),
            last_tabs_hash: RwLock::new(None),
        }
    }

    pub async fn push_console_msg(&self, msg: TabConsoleMessages, agent_id: &str) {
        let mut agents_lock = self.connected_agents.write().await;
        if let Some(agent) = agents_lock.iter_mut().find(|a| a.agent_id == agent_id) {
            agent.output_messages.push(msg);
        }
    }
}
