use gloo_net::http::Request;
use leptos::prelude::window;
use shared::{
    net::{ADMIN_ENDPOINT, ADMIN_HEALTH_CHECK_ENDPOINT, ADMIN_LOGIN_ENDPOINT, AdminLoginPacket},
    tasks::AdminCommand,
};
use thiserror::Error;
use web_sys::RequestCredentials;

use crate::models::C2_STORAGE_KEY;

#[derive(Debug, PartialEq, Eq)]
pub enum IsTaskingAgent<'a> {
    Yes(&'a String),
    No,
}

#[derive(Debug, Error)]
pub enum IsTaskingAgentErr {
    #[error("No ID found on IsTaskingAgent")]
    NoId,
}

impl IsTaskingAgent<'_> {
    pub fn has_agent_id(&self) -> Result<(), IsTaskingAgentErr> {
        if let IsTaskingAgent::Yes(_) = self {
            return Ok(());
        }

        Err(IsTaskingAgentErr::NoId)
    }
}

/// Makes an API request to the C2 via REST & CORS.
///
/// # Args
/// - `command`: The [`AdminCommand`] to dispatch on the C2.
/// - `is_tasking_agent`: Whether an exact agent is being tasked, or the command is generic.
/// - `creds`: A tuple [`Option`] containing (`username`, `password`) if logging in.
/// - `c2_url`: The URL of the C2 to connect to
/// - `custom_uri`: Whether a custom URI is supplied, as an [`Option`]
///
/// # Returns
/// - `Ok`: A Vec of bytes from the C2
/// - `Err` an [`ApiError`] containing the error kind and information.
pub async fn api_request(
    command: AdminCommand,
    is_tasking_agent: &IsTaskingAgent<'_>,
    creds: Option<(String, String)>,
    c2_url: &str,
    custom_uri: Option<&str>,
) -> Result<Vec<u8>, ApiError> {
    // Remove any leading '/' as we want to format correctly in the below builder
    let custom_uri = if let Some(u) = custom_uri {
        let u = match u.strip_prefix("/") {
            Some(s) => s,
            None => u,
        };
        Some(u)
    } else {
        None
    };

    let c2_url: String = {
        let s = match command {
            AdminCommand::Login => {
                format!("{}/{}", c2_url, custom_uri.unwrap_or(ADMIN_LOGIN_ENDPOINT))
            }
            _ => "".into(),
        };

        if !s.is_empty() {
            s
        } else {
            match is_tasking_agent {
                IsTaskingAgent::Yes(uid) => format!(
                    "{}/{}/{}",
                    c2_url,
                    custom_uri.unwrap_or(ADMIN_ENDPOINT),
                    uid
                ),
                IsTaskingAgent::No => {
                    format!("{}/{}", c2_url, custom_uri.unwrap_or(ADMIN_ENDPOINT))
                }
            }
        }
    };

    let resp = match command {
        AdminCommand::Login => {
            let admin_creds = AdminLoginPacket {
                username: creds.clone().unwrap().0,
                password: creds.unwrap().1.clone(),
            };

            Request::post(&c2_url)
                .credentials(RequestCredentials::Include)
                .json(&admin_creds)?
                .send()
                .await?
        }
        _ => {
            Request::post(&c2_url)
                .credentials(RequestCredentials::Include)
                .json(&command)?
                .send()
                .await?
        }
    };

    // Note, all admin commands return ACCEPTED (status 202) on successful authentication / completion
    // not the anticipated 200 OK. Dont recall why I went that route, but here we are :)
    if resp.status() != 202 {
        return Err(ApiError::BadStatus(
            resp.status(),
            resp.text().await.unwrap(),
        ));
    }

    let bytes = resp.binary().await?;
    Ok(bytes.to_vec())
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP error {0}.")]
    Reqwest(#[from] gloo_net::Error),
    #[error("Server returned status {0}. {1}")]
    BadStatus(u16, String),
}

/// Checks whether the user is logged in with a valid session, returning true if they are.
pub async fn admin_health_check() -> bool {
    let mut c2_url = match window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(C2_STORAGE_KEY).ok())
        .unwrap_or_default()
    {
        Some(url) => url,
        None => return false,
    };

    c2_url.push_str(ADMIN_HEALTH_CHECK_ENDPOINT);

    match Request::get(&c2_url)
        .credentials(RequestCredentials::Include)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status() == 200 {
                true
            } else {
                false
            }
        }
        Err(e) => panic!("Could not make request when making logged in check. {e}"),
    }
}
