use askama::Template;
use axum::extract::Form;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::{pretty_print::print_failed, tasks::AdminCommand};
use shared_c2_client::{AgentC2MemoryNotifications, NotificationForAgent};
use std::{collections::HashMap, ffi::CStr, str::FromStr, sync::Arc};

use crate::{
    models::{ActiveTabData, Agent, AppState, TabConsoleMessages},
    net::{IsTaskingAgent, api_request},
    tasks::task_dispatch::dispatch_task,
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
        // TODO logging
        println!("UNAUTH");
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let api_result = match api_request(
        AdminCommand::ListAgents,
        &IsTaskingAgent::No,
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

    let deser_agents_on_c2: Vec<AgentC2MemoryNotifications> =
        match serde_json::from_slice(&api_result) {
            Ok(agents) => agents,
            Err(e) => {
                // TODO client logging
                println!("Failed to deser agents: {e:?}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    // Reconstruct the in memory list
    let connected_agents = {
        let connected_agents = state.connected_agents.read().await;
        connected_agents.clone()
    };
    let mut buf = connected_agents;

    let mut index: HashMap<String, usize> = buf
        .iter()
        .enumerate()
        .map(|(i, a)| (a.agent_id.clone(), i))
        .collect();

    for (agent, is_stale, new_messages) in deser_agents_on_c2 {
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

        if let Some(i) = index.get(&uid).copied() {
            let agent = &mut buf[i];
            agent.last_check_in = last_seen;
            agent.pid = pid;
            agent.process_name = process_image;
            agent.is_stale = is_stale;

            if let Some(msgs) = new_messages {
                if let Ok(Some(msgs)) =
                    serde_json::from_value::<Option<Vec<NotificationForAgent>>>(msgs)
                {
                    agent
                        .output_messages
                        .extend(msgs.into_iter().map(TabConsoleMessages::from));
                }
            }
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
    {
        let mut connected_agents_lock = state.connected_agents.write().await;
        *connected_agents_lock = buf.clone();
    }

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

pub async fn draw_tabs(state: State<Arc<AppState>>) -> Response {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let lock = state.active_tabs.read().await;
    let tab_data = (lock.0, lock.1.clone());

    // Hash the tab_data to detect changes
    let mut hasher = DefaultHasher::new();
    tab_data.hash(&mut hasher);
    let current_hash = hasher.finish();

    // Store last sent hash in AppState
    let mut last_hash_lock = state.last_tabs_hash.write().await;
    if let Some(last_hash) = *last_hash_lock {
        if last_hash == current_hash {
            // No change, return 204
            return StatusCode::NO_CONTENT.into_response();
        }
    }
    // Update last hash
    *last_hash_lock = Some(current_hash);

    // Render tabs as before
    let tab_page = TabsPage { tab_data };
    return Html(tab_page.render().unwrap()).into_response();
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

    render_tabs(selected_agent.agent.clone(), state.clone()).await
}

#[derive(Deserialize)]
pub struct SelectedAgentIdx {
    index: usize,
}

pub async fn select_agent_tab_idx(
    state: State<Arc<AppState>>,
    index: Query<SelectedAgentIdx>,
) -> Response {
    let tab_name = {
        let tab_name_lock = state.active_tabs.read().await;

        match tab_name_lock.1.get(index.index) {
            Some(t) => t.clone(),
            None => {
                println!("Could not find selected tab by ID: {}", index.index);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    render_tabs(tab_name, state.clone()).await
}

async fn render_tabs(needle: String, state: State<Arc<AppState>>) -> Response {
    let mut lock = state.active_tabs.write().await;

    // if we already have the tab on the bar, just set the index to the active tab
    if let Some(idx) = lock.1.iter().position(|a| *a == needle) {
        lock.0 = idx;

        let tab_page = TabsPage {
            tab_data: (idx, lock.1.clone()),
        };
        return Html(tab_page.render().unwrap()).into_response();
    }

    lock.1.push(needle.clone());
    // As the len is starting at an idx of 1, we need to pull it back by
    // 1 to keep counting with the same starting reference.
    lock.0 = lock.1.len() - 1;

    let tab_page = TabsPage {
        tab_data: (lock.0, lock.1.clone()),
    };

    return Html(tab_page.render().unwrap()).into_response();
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

    let agent_name = match lock_tabs.1.get(lock_tabs.0) {
        Some(a) => a,
        None => {
            println!("Tab not found.");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

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
    let mut tasking_agent_id: String = String::new();

    //
    // Write the input to the console
    //
    {
        let tab_lock = state.active_tabs.read().await;
        let tab = tab_lock.1.get(tab_lock.0);

        if let Some(agent_id) = tab {
            tasking_agent_id = agent_id.clone();

            let mut agents_lock = state.connected_agents.write().await;
            if let Some(agent) = agents_lock.iter_mut().find(|a| a.agent_id == *agent_id) {
                agent
                    .output_messages
                    .push(TabConsoleMessages::non_agent_message(
                        "Console input".into(),
                        msg.cmd_input.clone(),
                    ));
            }
        } else {
            // TODO logging
            print_failed("Could not get tab ID for agent.");
            return StatusCode::BAD_REQUEST.into_response();
        }
    }

    //
    // Send the command up to the C2
    //

    let creds = {
        // This feels a little safer than holding the tokio rwlock over several async boundaries?
        let creds = state.creds.read().await;
        creds.clone()
    };

    if tasking_agent_id.is_empty() || creds.is_none() {
        let tab_lock = state.active_tabs.read().await;
        let tab = tab_lock.1.get(tab_lock.0);

        if let Some(agent_id) = tab {
            let msg = TabConsoleMessages::non_agent_message(
                "Error issuing command.".into(),
                "Credentials or agent ID not found.".into(),
            );
            state.push_console_msg(msg, agent_id).await;
        }
    }

    let result = dispatch_task(
        msg.cmd_input,
        &creds.unwrap(),
        IsTaskingAgent::Yes(&tasking_agent_id),
        state.clone(),
    )
    .await;

    //
    // If there was an error, print it to the console
    //
    if let Err(e) = result {
        let tab_lock = state.active_tabs.read().await;
        let tab = tab_lock.1.get(tab_lock.0);

        if let Some(agent_id) = tab {
            let msg = TabConsoleMessages::non_agent_message(
                "Error issuing command.".into(),
                e.to_string(),
            );
            state.push_console_msg(msg, agent_id).await;
        }
    }

    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct CloseTabRequest {
    pub index: usize,
}

pub async fn close_tab(state: State<Arc<AppState>>, Form(req): Form<CloseTabRequest>) -> Response {
    let mut lock = state.active_tabs.write().await;
    if lock.1.len() > 1 && req.index < lock.1.len() {
        lock.1.remove(req.index);
        if lock.0 >= lock.1.len() {
            lock.0 = lock.1.len().saturating_sub(1);
        }
    }
    let mut last_hash_lock = state.last_tabs_hash.write().await;
    *last_hash_lock = None;
    let tab_page = TabsPage {
        tab_data: (lock.0, lock.1.clone()),
    };
    Html(tab_page.render().unwrap()).into_response()
}
