use std::{collections::HashMap, ffi::CStr, str::FromStr};

use chrono::{DateTime, Utc};
use leptos::prelude::*;

use crate::models::dashboard::{
    Agent, AgentC2MemoryNotifications, NotificationForAgent, TabConsoleMessages,
};

pub fn update_connected_agents(
    set_connected_agents: WriteSignal<HashMap<String, RwSignal<Agent>>>,
    polled_agents: Vec<AgentC2MemoryNotifications>,
) {
    for (agent, is_stale, new_messages) in polled_agents {
        let split: Vec<&str> = agent.split('\t').collect();

        // Because we sent the string over separated by \t, we can use this to explode the
        // params we sent in order to split it out correctly.
        // todo we should in the future send a struct over, it will be better semantics.. than a string split by tabs
        let uid = split[1].to_string();
        let last_seen: DateTime<Utc> = DateTime::from_str(split[3]).unwrap();
        let pid: u32 = split[4].to_string().parse().unwrap();

        let process_image: String = CStr::from_bytes_until_nul(split[5].as_bytes())
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        if let Some(_) = set_connected_agents.try_update(|sig| {
            //
            // If we have the tracked agent already in the HashMap, update its fields
            //
            if let Some(ta) = (*sig).get_mut(&uid) {
                let mut tracked_agent = ta.write();
                tracked_agent.last_check_in = last_seen;
                tracked_agent.pid = pid;
                tracked_agent.is_stale = is_stale;
                tracked_agent.process_name = process_image;

                if let Some(msgs) = new_messages {
                    if let Ok(Some(msgs)) =
                        serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                    {
                        tracked_agent
                            .output_messages
                            .extend(msgs.into_iter().map(TabConsoleMessages::from));
                    }
                }
            } else {
                //
                // Otherwise, if the agent is not in the HashMap, we need to insert it
                //
                if let Some(msgs) = new_messages {
                    if let Ok(Some(msgs)) =
                        serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                    {
                        (*sig).insert(
                            uid.clone(),
                            RwSignal::new(Agent::from_messages(
                                msgs,
                                uid,
                                last_seen,
                                pid,
                                process_image,
                                is_stale,
                            )),
                        );
                    }
                } else {
                    (*sig).insert(
                        uid.clone(),
                        RwSignal::new(Agent::from(uid, last_seen, pid, process_image, is_stale)),
                    );
                }
            }
        }) {
            ()
        };
    }
}
