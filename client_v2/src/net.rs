use reqwest::{StatusCode, header::CONTENT_TYPE};
use shared::{net::ADMIN_ENDPOINT, tasks::AdminCommand};
use shared_c2_client::ADMIN_AUTH_SEPARATOR;
use thiserror::Error;

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

#[derive(Debug, Clone, Default)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub admin_env_token: String,
    pub c2_url: String,
}

fn auth_header(creds: &Credentials) -> String {
    format!(
        "{}{}{}{}{}",
        creds.username,
        ADMIN_AUTH_SEPARATOR,
        creds.password,
        ADMIN_AUTH_SEPARATOR,
        creds.admin_env_token,
    )
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP error {0}.")]
    Reqwest(#[from] reqwest::Error),
    #[error("Server returned status {0}.")]
    BadStatus(reqwest::StatusCode),
}

/// Make an API request to the C2 from the GUI
pub async fn api_request(
    command: AdminCommand,
    is_tasking_agent: &IsTaskingAgent<'_>,
    creds: &Credentials,
) -> Result<Vec<u8>, ApiError> {
    let c2_url: String = match is_tasking_agent {
        IsTaskingAgent::Yes(uid) => format!("{}/{}/{}", creds.c2_url, ADMIN_ENDPOINT, uid),
        IsTaskingAgent::No => format!("{}/{}", creds.c2_url, ADMIN_ENDPOINT),
    };

    let body_bytes = serde_json::to_vec(&command).expect("Could not convert command to bytes");

    let request = reqwest::Client::new()
        .post(c2_url)
        .body(body_bytes)
        .header("Authorization", auth_header(creds))
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await?;

    // Note, all admin commands return ACCEPTED (status 202) on successful authentication / completion
    // not the anticipated 200 OK. Dont recall why I went that route, but here we are :)
    if request.status() != StatusCode::ACCEPTED {
        return Err(ApiError::BadStatus(request.status()));
    }

    let bytes = request.bytes().await?;
    Ok(bytes.to_vec())
}
