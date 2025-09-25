use std::{collections::HashMap, ffi::CStr, str::FromStr, sync::Arc};

use chrono::DateTime;
use shared::{pretty_print::print_failed, tasks::AdminCommand};

use crate::{
    net::{IsTaskingAgent, api_request},
    state::{Agent, Credentials},
};

pub fn get_agents(
    creds: Arc<Credentials>,
) -> Result<Option<HashMap<String, Agent>>, reqwest::Error> {
    let agents = api_request(AdminCommand::ListAgents, IsTaskingAgent::No, &creds)?;

    let agents_serialised: Vec<(String, bool)> = match serde_json::from_slice(&agents) {
        Ok(a) => a,
        Err(e) => {
            print_failed(format!("Failed to deser agents. {e}"));
            return Ok(None);
        }
    };

    let mut buf: HashMap<String, Agent> = HashMap::new();

    for (agent, is_stale) in agents_serialised {
        let split: Vec<&str> = agent.split('\t').collect();

        // Because we sent the string over separated by \t, we can use this to explode the
        // params we sent in order to split it out correctly.
        // todo we should in the future send a struct over, it will be better semantics.. than a string split by tabs
        let uid = split[1].to_string();
        let last_seen = DateTime::from_str(split[3]).unwrap();
        let pid: u32 = split[4].to_string().parse().unwrap();

        let process_image: String = CStr::from_bytes_until_nul(split[5].as_bytes())
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        buf.insert(
            uid.clone(),
            Agent::from(uid, last_seen, pid, process_image, is_stale),
        );
    }

    Ok(Some(buf))
}
