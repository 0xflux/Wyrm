use std::{ffi::CStr, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Form,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::tasks::AdminCommand;
use shared_c2_client::{AgentC2MemoryNotifications, NotificationForAgent};

use crate::{
    models::{ActiveTabData, Agent, AppState, TabConsoleMessages},
    net::{IsTaskingAgent, api_request},
};

pub type ConnectedAgentData = Vec<Agent>;

#[derive(Template)]
#[template(path = "htmx_applets/connected_agent_panel.html")]
struct ConnectedAgentPage {
    data: ConnectedAgentData,
}

pub async fn poll_connected_agents(state: State<Arc<AppState>>) -> Response {
    let creds = &*state.creds.read().await;

    if creds.is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let api_result = match api_request(
        AdminCommand::ListAgents,
        IsTaskingAgent::No,
        creds.as_ref().unwrap(),
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            // TODO GUI logging
            println!("Failed to make API request. {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let deserialised: Vec<AgentC2MemoryNotifications> = match serde_json::from_slice(&api_result) {
        Ok(agents) => agents,
        Err(e) => {
            // TODO client logging
            println!("Failed to deser agents: {e:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // A temp buffer to store all agents, including ones that we currently have in memory.
    // Rather than editing them in place, we will edit and add to this new buffer to save on
    // a little searching later to only update changed entries. I feel this is on par memory
    // and efficiency wise; this could be improved perhaps by noting indexes and doing something
    // there but... I think this is ok for now. Does need profiling though
    let mut buf: Vec<Agent> = Vec::new();

    let mut connected_agents_lock: tokio::sync::RwLockWriteGuard<'_, Vec<Agent>> =
        state.connected_agents.write().await;

    for (agent, is_stale, new_messages) in deserialised {
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

        //
        // Try to find an existing agent - if it exists then we need to update its fields with the new poll data,
        // and determine if there are any new messages from the C2 that were pulled, if there were, add them.
        //
        if let Some(agent) = connected_agents_lock.iter_mut().find(|a| a.agent_id == uid) {
            agent.last_check_in = last_seen;
            agent.pid = pid;
            agent.process_name = process_image;
            agent.is_stale = is_stale;

            if let Some(msgs) = new_messages {
                if let Ok(Some(msgs)) =
                    serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                {
                    for m in msgs {
                        agent.output_messages.push(TabConsoleMessages::from(m));
                    }
                }
            }

            buf.push(agent.clone());
        } else {
            //
            // This branch runs if the agent did NOT exist in memory already within the UI
            //

            if let Some(msgs) = new_messages {
                if let Ok(Some(msgs)) =
                    serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                {
                    buf.push(Agent::from_messages(
                        msgs,
                        uid,
                        last_seen,
                        pid,
                        process_image,
                        is_stale,
                    ));

                    continue;
                }
            } else {
                buf.push(Agent::from(uid, last_seen, pid, process_image, is_stale));
            }
        }
    }

    // Unfortunately we cant do a mem::take here, we do need to clone it as we return it back to the
    // UI...
    *connected_agents_lock = buf.clone();

    return Html(ConnectedAgentPage { data: buf }.render().unwrap()).into_response();
}

#[derive(Deserialize)]
pub struct SelectedAgent {
    agent: String,
}

#[derive(Template)]
#[template(path = "htmx_applets/agent_tabs.html")]
struct TabsPage {
    tab_data: ActiveTabData,
}

pub async fn select_agent_tab(
    state: State<Arc<AppState>>,
    selected_agent: Query<SelectedAgent>,
) -> Response {
    let mut discovered: bool = false;

    // Check if its the page load searching for the "Server" tab
    if selected_agent.agent == "Server" {
        discovered = true;
    }

    // Search for the agent in the live agents we track
    if !discovered {
        let lock = state.connected_agents.read().await;

        for a in &*lock {
            if a.agent_id == selected_agent.agent {
                discovered = true;
                break;
            }
        }
    }

    if !discovered {
        // TODO maybe a better error?
        println!("Not discovered. Searching for: {}", selected_agent.agent);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    {
        let mut lock = state.active_tabs.write().await;
        // if we already have the tab on the bar, just set the index to the active tab
        if let Some(idx) = lock.1.iter().position(|a| *a == selected_agent.agent) {
            lock.0 = idx;

            let tab_page = TabsPage {
                tab_data: (idx, lock.1.clone()),
            };
            return Html(tab_page.render().unwrap()).into_response();
        }

        lock.1.push(selected_agent.agent.clone());
        // As the len is starting at an idx of 1, we need to pull it back by
        // 1 to keep counting with the same starting reference.
        lock.0 = lock.1.len() - 1;

        let tab_page = TabsPage {
            tab_data: (lock.0, lock.1.clone()),
        };

        return Html(tab_page.render().unwrap()).into_response();
    }
}

#[derive(Template)]
#[template(path = "htmx_applets/implant_messages.html")]
struct ImplantMessages {
    page_content: Vec<TabConsoleMessages>,
}

/// Called by the UI on the active tab ever `x` ms (defined in the HTMX).
///
/// This should be relatively ok - we aren't making HTTP requests with this and only pulling strings from internal
/// structs.
pub async fn show_implant_messages(state: State<Arc<AppState>>) -> Response {
    let lock_tabs = state.active_tabs.read().await;
    let lock_agents = state.connected_agents.read().await;

    let agent_name = lock_tabs.1.get(lock_tabs.0).unwrap();

    if let Some(agent) = lock_agents.iter().find(|a| a.agent_id == *agent_name) {
        let page = ImplantMessages {
            page_content: agent.output_messages.clone(),
        };

        return Html(page.render().unwrap()).into_response();
    };

    "No data".into_response()
}

#[derive(Deserialize)]
pub struct SendCommandForm {
    cmd_input: String,
}

pub async fn send_command(
    state: State<Arc<AppState>>,
    Form(msg): Form<SendCommandForm>,
) -> Response {
    {
        let tab_lock = state.active_tabs.read().await;
        let tab = tab_lock.1.get(tab_lock.0);

        if let Some(agent_id) = tab {
            let mut agents_lock = state.connected_agents.write().await;
            if let Some(agent) = agents_lock.iter_mut().find(|a| a.agent_id == *agent_id) {
                agent
                    .output_messages
                    .push(TabConsoleMessages::non_agent_message(
                        "Console input".into(),
                        msg.cmd_input.clone(),
                    ));
            }
        }
    }

    // todo send to c2

    StatusCode::OK.into_response()
}
