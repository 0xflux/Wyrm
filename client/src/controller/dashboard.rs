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

pub fn update_connected_agents(
    set_connected_agents: RwSignal<HashMap<String, RwSignal<Agent>>>,
    polled_agents: Vec<AgentC2MemoryNotifications>,
) {
    // Parse the incoming polled agents into a local vec so we can
    // determine which UIDs are present and perform updates/inserts.
    let mut parsed: Vec<(
        String,
        DateTime<Utc>,
        u32,
        String,
        bool,
        Option<serde_json::Value>,
    )> = Vec::new();

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

        parsed.push((uid, last_seen, pid, process_image, is_stale, new_messages));
    }

    // Build a set of UIDs returned by the server.
    let new_uids: HashSet<String> = parsed
        .iter()
        .map(|(uid, _, _, _, _, _)| uid.clone())
        .collect();

    // Remove any agents from the tracked map that are no longer present in the latest poll.
    if let Some(_) = set_connected_agents.try_update(|map| {
        map.retain(|k, _| new_uids.contains(k));
    }) {
        ()
    };

    // Update existing agents or insert new ones from the parsed list.
    if let Some(_) = set_connected_agents.try_update(|sig| {
        for (uid, last_seen, pid, process_image, is_stale, new_messages) in parsed {
            if let Some(ta) = (*sig).get_mut(&uid) {
                let mut tracked_agent = ta.write();
                tracked_agent.last_check_in = last_seen;
                tracked_agent.pid = pid;
                tracked_agent.is_stale = is_stale;
                tracked_agent.process_name = process_image.clone();

                if tracked_agent.output_messages.is_empty() {
                    if let Ok(stored) = get_item_from_browser_store::<Vec<TabConsoleMessages>>(
                        &wyrm_chat_history_browser_key(&uid),
                    ) {
                        tracked_agent.output_messages = stored;
                    }
                }

                if let Some(msgs) = new_messages {
                    if let Ok(Some(msgs)) =
                        serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                    {
                        let new_msgs = msgs
                            .into_iter()
                            .map(TabConsoleMessages::from)
                            .collect::<Vec<_>>();
                        tracked_agent.output_messages.extend(new_msgs);
                        // persist updated history
                        let _ = store_item_in_browser_store(
                            &wyrm_chat_history_browser_key(&uid),
                            &tracked_agent.output_messages,
                        );
                    }
                } else {
                    let _ = store_item_in_browser_store(
                        &wyrm_chat_history_browser_key(&uid),
                        &tracked_agent.output_messages,
                    );
                }
            } else {
                // Insert new tracked agent.
                if let Some(msgs) = new_messages {
                    if let Ok(Some(msgs)) =
                        serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                    {
                        (*sig).insert(
                            uid.clone(),
                            RwSignal::new(Agent::from_messages(
                                msgs,
                                uid.clone(),
                                last_seen,
                                pid,
                                process_image,
                                is_stale,
                            )),
                        );
                    } else {
                        (*sig).insert(
                            uid.clone(),
                            RwSignal::new(Agent::from(
                                uid.clone(),
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
                        RwSignal::new(Agent::from(
                            uid.clone(),
                            last_seen,
                            pid,
                            process_image,
                            is_stale,
                        )),
                    );
                }

                // after insertion, try to rehydrate stored history
                if let Some(ta) = (*sig).get_mut(&uid) {
                    let mut tracked_agent = ta.write();
                    if tracked_agent.output_messages.is_empty() {
                        if let Ok(stored) = get_item_from_browser_store::<Vec<TabConsoleMessages>>(
                            &wyrm_chat_history_browser_key(&uid),
                        ) {
                            tracked_agent.output_messages = stored;
                        }
                    }
                }
            }
        }
    }) {
        ()
    };
}
