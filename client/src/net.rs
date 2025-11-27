use gloo_net::http::{Request, Response};
use leptos::prelude::window;
use serde_json::Value;
use shared::{
    net::{ADMIN_ENDPOINT, ADMIN_HEALTH_CHECK_ENDPOINT, ADMIN_LOGIN_ENDPOINT, AdminLoginPacket},
    tasks::{AdminCommand, BaBData},
};
use thiserror::Error;
use web_sys::RequestCredentials;

use crate::{controller::get_item_from_browser_store, models::C2_STORAGE_KEY};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IsTaskingAgent {
    Yes(String),
    No,
}

#[derive(Debug, Error)]
pub enum IsTaskingAgentErr {
    #[error("No ID found on IsTaskingAgent")]
    NoId,
}

impl IsTaskingAgent {
    pub fn has_agent_id(&self) -> Result<(), IsTaskingAgentErr> {
        if let IsTaskingAgent::Yes(_) = self {
            return Ok(());
        }

        Err(IsTaskingAgentErr::NoId)
    }
}

pub enum C2Url {
    /// Will be obtained from the key `C2_STORAGE_KEY`
    Standard,
    /// Whatever is in the inner will be used as the C2 URL
    Custom(String),
}

impl C2Url {
    /// Retrieve the C2 url depending upon the type. The [`C2Url::Standard`] will be pulled from the browser
    /// store at the key `C2_STORAGE_KEY`.
    ///
    /// In the case of [`C2Url::Standard`], the inner `String` will be retrieved.
    fn get(&self) -> anyhow::Result<String> {
        match self {
            C2Url::Standard => {
                // Get from browser store
                get_item_from_browser_store::<String>(C2_STORAGE_KEY)
            }
            C2Url::Custom(url) => Ok(url.clone()),
        }
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
    is_tasking_agent: &IsTaskingAgent,
    creds: Option<(String, String)>,
    c2_url: C2Url,
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

    let c2_url: String = construct_c2_url(c2_url, &command, custom_uri, is_tasking_agent);

    //
    // Send the HTTP request to the C2
    //

    let post_body_data = prepare_body_data(command, creds);
    let resp = make_post(&c2_url, post_body_data).await?;

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

/// Prepare the POST request body data by serialising the input to an expected JSON value which
/// the C2 will expect.
///
/// For some C2  API's, the JSON body is expected to be of a certain type, so this ensures we sent the correct
/// type to the C2. If no such exact type is required (e.g. the data is included in the [`AdminCommand`]) then it will
/// just prepare that as-is without converting to another expected type.
///
/// # Returns
/// The function returns a [`serde_json::Value`] of the body data.
fn prepare_body_data(input: AdminCommand, creds: Option<(String, String)>) -> Value {
    match input {
        AdminCommand::Login => serde_json::to_value(AdminLoginPacket {
            username: creds.clone().unwrap().0,
            password: creds.unwrap().1.clone(),
        })
        .unwrap(),
        AdminCommand::BuildAllBins(data) => {
            serde_json::to_value(BaBData::from(data.clone())).unwrap()
        }
        _ => serde_json::to_value(input).unwrap(),
    }
}

async fn make_post(c2_url: &str, body: Value) -> Result<Response, ApiError> {
    let r = Request::post(c2_url)
        .credentials(RequestCredentials::Include)
        .json(&body)?
        .send()
        .await?;

    Ok(r)
}

fn construct_c2_url(
    c2_url: C2Url,
    command: &AdminCommand,
    custom_uri: Option<&str>,
    is_tasking_agent: &IsTaskingAgent,
) -> String {
    // Extrapolate the C2 url from the input enum
    let c2_url = c2_url.get().expect("could not get C2 url");

    //
    // If its a login command, we need to explicitly handle building that URI. If the command
    // was not login, then deal with inputting the UID of the implant being tasked, otherwise, it
    // can be constructed without.
    //
    // This allows for the format url.com/api_endpoint/agent_uid on the C2 to handle those paths.
    //
    let s = match command {
        AdminCommand::Login => {
            format!("{}/{}", c2_url, custom_uri.unwrap_or(ADMIN_LOGIN_ENDPOINT))
        }
        _ => "".into(),
    };

    if !s.is_empty() {
        // For the login URL, return this out as the C2 url
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
        Some(url) => {
            // Because of serde_json we need to remove " from the stored value
            url.replace("\"", "")
        }
        None => return false,
    };

    c2_url.push_str(ADMIN_HEALTH_CHECK_ENDPOINT);

    match Request::get(&c2_url)
        .credentials(RequestCredentials::Include)
        .send()
        .await
    {
        Ok(resp) => resp.status() == 200,
        Err(e) => panic!("Could not make request when making logged in check. {e}"),
    }
}
