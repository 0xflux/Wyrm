use std::{
    collections::{HashMap, HashSet},
    ffi::CStr,
    str::FromStr,
};

use chrono::{DateTime, Utc};
use leptos::prelude::*;

use crate::controller::{
    get_item_from_browser_store, store_item_in_browser_store, wyrm_chat_history_browser_key,
};
use crate::models::dashboard::{
    Agent, AgentC2MemoryNotifications, NotificationForAgent, TabConsoleMessages,
};

/// Updates the local representation of agents that are connected to the C2. As this is a client only app
/// and not a SSR app, it is slightly more messy - we poll the update from the server; store temporarily in the
/// browser store (to persist between refreshes and navigation), and display to the user.
pub fn update_connected_agents(
    set_connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>>,
    polled_agents: Vec<AgentC2MemoryNotifications>,
) {
    // Split out the incoming agents in the poll and do a manual deserialisation based on the presence of
    // \t chars which was sent as the connection string.
    let parsed: Vec<_> = polled_agents
        .into_iter()
        .map(|(agent, is_stale, new_messages)| {
            let split: Vec<&str> = agent.split('\t').collect();
            let uid = split[1].to_string();
            let last_seen = DateTime::from_str(split[3]).unwrap();
            let pid = split[4].parse().unwrap();
            let process_image = split[5].to_string();
            (uid, last_seen, pid, process_image, is_stale, new_messages)
        })
        .collect();

    let new_uids: HashSet<_> = parsed.iter().map(|(uid, ..)| uid.clone()).collect();

    //
    // Retain only agents still present
    //
    set_connected_agents.try_update(|map| {
        map.retain(|uid, _| new_uids.contains(uid));
    });

    //
    // Update or insert agents
    //
    set_connected_agents.try_update(|agents| {
        for (uid, last_seen, pid, process_image, is_stale, new_messages) in parsed {
            let entry = agents.entry(uid.clone()).or_insert_with(|| {
                RwSignal::new(Agent::from(
                    uid.clone(),
                    last_seen,
                    pid,
                    process_image.clone(),
                    is_stale,
                ))
            });

            let mut agent = entry.write();
            agent.last_check_in = last_seen;
            agent.pid = pid;
            agent.is_stale = is_stale;
            agent.process_name = process_image.clone();

            //
            // Always hydrate stored messages once
            //
            if agent.output_messages.is_empty() {
                if let Ok(stored) = get_item_from_browser_store::<Vec<TabConsoleMessages>>(
                    &wyrm_chat_history_browser_key(&uid),
                ) {
                    agent.output_messages = stored;
                }
            }

            //
            // Merge new messages
            //
            if let Some(msgs) = new_messages {
                if let Ok(Some(msgs)) =
                    serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                {
                    let new_msgs: Vec<_> = msgs.into_iter().map(TabConsoleMessages::from).collect();
                    agent.output_messages.extend(new_msgs);
                    let _ = store_item_in_browser_store(
                        &wyrm_chat_history_browser_key(&uid),
                        &agent.output_messages,
                    );
                }
            }
        }
    });
}
