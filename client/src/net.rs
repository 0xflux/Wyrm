use reqwest::header::CONTENT_TYPE;
use shared::{net::ADMIN_ENDPOINT, tasks::AdminCommand};
use shared_c2_client::ADMIN_AUTH_SEPARATOR;

use crate::state::Credentials;

pub enum IsTaskingAgent<'a> {
    Yes(&'a String),
    No,
}

pub fn api_request(
    command: AdminCommand,
    is_tasking_agent: IsTaskingAgent,
    creds: &Credentials,
) -> Result<Vec<u8>, reqwest::Error> {
    let c2_url: String = match is_tasking_agent {
        IsTaskingAgent::Yes(uid) => format!("{}/{}/{}", creds.c2_url, ADMIN_ENDPOINT, uid),
        IsTaskingAgent::No => format!("{}/{}", creds.c2_url, ADMIN_ENDPOINT),
    };

    let body_bytes = serde_json::to_vec(&command).expect("Could not convert command to bytes");

    let request = reqwest::blocking::Client::new()
        .post(c2_url)
        .body(body_bytes)
        .header("Authorization", auth_header(creds))
        .header(CONTENT_TYPE, "application/json")
        .send()?
        .bytes()?;

    Ok(request.to_vec())
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
