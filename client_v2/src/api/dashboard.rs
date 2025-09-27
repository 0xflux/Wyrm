use std::{collections::HashMap, ffi::CStr, str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use shared::tasks::AdminCommand;

use crate::{
    AppState,
    models::Agent,
    net::{IsTaskingAgent, api_request},
};

type ConnectedAgentData = HashMap<String, Agent>;

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
            // TODO differentiate between unauthorised vs a bad request / other error
            //  probably one to do in `api_request` and return the error out properly

            // match e {
            //     crate::net::ApiError::Reqwest(error) => todo!(),
            //     crate::net::ApiError::BadStatus(status_code) => todo!(),
            // }
            println!("Failed to make API request/ {e}");
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

    let mut buf: HashMap<String, Agent> = HashMap::new();

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

        buf.insert(
            uid.clone(),
            Agent::from(uid, last_seen, pid, process_image, is_stale),
        );
    }

    let page = ConnectedAgentPage { data: buf };

    return Html(page.render().unwrap()).into_response();
}
