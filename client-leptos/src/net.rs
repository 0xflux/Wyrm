use gloo_net::http::{Headers, Request};
use leptos::logging::log;
use shared::{
    net::{ADMIN_AUTH_SEPARATOR, ADMIN_ENDPOINT},
    tasks::AdminCommand,
};
use thiserror::Error;

use crate::pages::login::LoginData;

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

pub async fn api_request(
    command: AdminCommand,
    is_tasking_agent: &IsTaskingAgent<'_>,
    creds: &LoginData,
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

    let c2_url: String = match is_tasking_agent {
        IsTaskingAgent::Yes(uid) => format!(
            "{}/{}/{}",
            creds.c2_addr,
            custom_uri.unwrap_or(ADMIN_ENDPOINT),
            uid
        ),
        IsTaskingAgent::No => format!("{}/{}", creds.c2_addr, custom_uri.unwrap_or(ADMIN_ENDPOINT)),
    };

    let headers = Headers::new();
    headers.append("authorization", auth_header(&creds).as_str());

    let resp = Request::post(&c2_url)
        .headers(headers)
        .json(&command)?
        .send()
        .await?;

    // Note, all admin commands return ACCEPTED (status 202) on successful authentication / completion
    // not the anticipated 200 OK. Dont recall why I went that route, but here we are :)
    if resp.status() != 202 {
        log!("Bad request: {:?}", resp);
        return Err(ApiError::BadStatus(
            resp.status(),
            resp.text().await.unwrap(),
        ));
    }

    log!("Logged in ok: {:?}", resp);
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

fn auth_header(creds: &LoginData) -> String {
    format!(
        "{}{}{}{}{}",
        creds.username,
        ADMIN_AUTH_SEPARATOR,
        creds.password,
        ADMIN_AUTH_SEPARATOR,
        creds.admin_env_token,
    )
}
