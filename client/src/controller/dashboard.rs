use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use chrono::DateTime;
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

    set_connected_agents.try_update(|map| {
        map.retain(|uid, _| new_uids.contains(uid));
    });

    //
    // Ensure all UIDs exist in the map (but don't touch messages yet)
    //
    set_connected_agents.try_update(|agents| {
        for (uid, last_seen, pid, process_image, is_stale, _) in &parsed {
            agents.entry(uid.clone()).or_insert_with(|| {
                RwSignal::new(Agent::from(
                    uid.clone(),
                    *last_seen,
                    *pid,
                    process_image.clone(),
                    *is_stale,
                ))
            });
        }
    });

    //
    // Now merge fields & messages using the known goods
    //
    let agent_map_snapshot = set_connected_agents.get();

    for (uid, last_seen, pid, process_image, is_stale, new_messages) in parsed {
        let Some(agent_sig) = agent_map_snapshot.get(&uid).cloned() else {
            continue;
        };

        agent_sig.update(|agent| {
            // Basic fields
            agent.last_check_in = last_seen;
            agent.pid = pid;
            agent.is_stale = is_stale;
            agent.process_name = process_image.clone();

            // Hydrate from store when empty or when the store has more messages
            if let Ok(stored) = get_item_from_browser_store::<Vec<TabConsoleMessages>>(
                &wyrm_chat_history_browser_key(&uid),
            ) {
                if agent.output_messages.is_empty() || stored.len() > agent.output_messages.len() {
                    agent.output_messages = stored;
                }
            }

            // Merge new messages
            if let Some(raw) = new_messages {
                match serde_json::from_value::<Vec<NotificationForAgent>>(raw) {
                    Ok(msgs) if !msgs.is_empty() => {
                        let new_msgs: Vec<_> =
                            msgs.into_iter().map(TabConsoleMessages::from).collect();

                        agent.output_messages.extend(new_msgs);

                        let _ = store_item_in_browser_store(
                            &wyrm_chat_history_browser_key(&uid),
                            &agent.output_messages,
                        );
                    }
                    Ok(_) => {
                        leptos::logging::log!("Parsed empty new_messages vec for {uid}");
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to parse new_messages for {uid}: {e}");
                    }
                }
            }
        });
    }
}
