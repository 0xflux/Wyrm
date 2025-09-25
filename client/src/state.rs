use std::{
    collections::HashMap,
    fmt::Display,
    sync::{Arc, RwLock},
};

use chrono::{DateTime, Utc};
use shared::{pretty_print::print_failed, tasks::AdminCommand};

use crate::{
    // flush_and_readline,
    net::{IsTaskingAgent, api_request},
    // notifications::NotificationStatus,
};

#[derive(Debug, Clone, Default)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub admin_env_token: String,
    pub c2_url: String,
}

#[derive(Debug)]
pub struct Cli {
    pub uid: String,
    pub connected_agents: Arc<RwLock<Option<HashMap<String, Agent>>>>,
    pub credentials: Arc<Credentials>,
}

/// A local client representation of an agent with a definition not shared across the
/// `Wyrm` ecosystem.
#[derive(Debug, Clone, Default)]
pub struct Agent {
    pub agent_id: String,
    pub last_check_in: DateTime<Utc>,
    pub pid: u32,
    pub process_name: String,
    // pub notification_status: NotificationStatus,
    pub is_stale: bool,
    /// Messages to be shown in the message box in the GUI
    pub output_messages: Vec<TabConsoleMessages>,
}

#[derive(Debug, Clone, Default)]
pub struct TabConsoleMessages {
    pub event: String,
    pub time: String,
    pub messages: Option<Vec<String>>,
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
            messages: Some(vec![message]),
        }
    }
}

impl Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.agent_id)
    }
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

impl Cli {
    pub fn from_page(
        uid: String,
        connected_agents: Arc<RwLock<Option<HashMap<String, Agent>>>>,
        credentials: Arc<Credentials>,
    ) -> Self {
        Self {
            uid,
            connected_agents,
            credentials,
        }
    }

    pub fn write_console_error<S: Into<String>>(&self, uid: &str, msg: S) {
        let mut lock = self.connected_agents.write().unwrap();
        let agent = lock.as_mut().unwrap().get_mut(uid);

        if let Some(agent) = agent {
            agent.output_messages.push(TabConsoleMessages {
                event: "ConsoleError".into(),
                time: "()".into(),
                messages: Some(vec![msg.into()]),
            });
        }
    }
}

/// Sends a login task, which on the server will do nothing - but we will get a response back
/// relating to the authentication middleware which runs. If we get an error back at any stage of that
/// process it means the login was unsuccessful.
pub fn do_login(creds: Credentials) -> Option<Credentials> {
    let response: String = match api_request(AdminCommand::Login, IsTaskingAgent::No, &creds) {
        Ok(r) => match serde_json::from_slice(&r) {
            Ok(s) => s,
            Err(_) => {
                print_failed("Login failed.");
                return None;
            }
        },
        Err(e) => {
            print_failed(format!("Login failed. {e}"));
            return None;
        }
    };

    if response.eq("success") {
        return Some(creds);
    }

    print_failed("Login failed on return msg?");
    None
}
