use std::{ffi::CStr, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::tasks::AdminCommand;

use crate::{
    models::{ActiveTabData, Agent, AppState},
    net::{IsTaskingAgent, api_request},
};

pub type ConnectedAgentData = Vec<Agent>;

#[derive(Template)]
#[template(path = "htmx_applets/connected_agent_panel.html")]
struct ConnectedAgentPage {
    data: ConnectedAgentData,
}

#[axum::debug_handler]
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

    let serialised: Vec<(String, bool)> = match serde_json::from_slice(&api_result) {
        Ok(agents) => agents,
        Err(e) => {
            // TODO client logging
            println!("Failed to deser agents: {e:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut buf: Vec<Agent> = Vec::new();

    for (agent, is_stale) in serialised {
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

        buf.push(Agent::from(uid, last_seen, pid, process_image, is_stale));
    }

    {
        let mut lock = state.connected_agents.write().await;
        *lock = buf.clone();
    }

    let page = ConnectedAgentPage { data: buf };

    return Html(page.render().unwrap()).into_response();
}

#[derive(Deserialize)]
pub struct SelectedAgent {
    agent: String,
}

#[derive(Template)]
#[template(path = "htmx_applets/agent_tabs.html")]
struct TabsPage {
    data: ActiveTabData,
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
        // maybe a better error?
        println!("Not discovered. Searching for: {}", selected_agent.agent);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    {
        let mut lock = state.active_tabs.write().await;
        // if we already have the tab on the bar, just set the index to the active tab
        if let Some(idx) = lock.1.iter().position(|a| *a == selected_agent.agent) {
            lock.0 = idx;

            let tab_page = TabsPage {
                data: (idx, lock.1.clone()),
            };
            return Html(tab_page.render().unwrap()).into_response();
        }

        lock.1.push(selected_agent.agent.clone());
        // As the len is starting at an idx of 1, we need to pull it back by
        // 1 to keep counting with the same starting reference.
        lock.0 = lock.1.len() - 1;

        let tab_page = TabsPage {
            data: (lock.0, lock.1.clone()),
        };
        return Html(tab_page.render().unwrap()).into_response();
    }
}
