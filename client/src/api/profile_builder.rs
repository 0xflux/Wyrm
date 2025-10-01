use std::sync::Arc;

use crate::{
    models::AppState,
    net::{IsTaskingAgent, api_request},
};
use axum::{
    Form,
    body::Bytes,
    debug_handler,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
};
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use serde::Deserialize;
use shared::tasks::AdminCommand;

#[derive(Deserialize)]
pub struct BuildAllProfilesPageData {
    profile_name: String,
}

#[debug_handler]
pub async fn build_all_profiles(
    state: State<Arc<AppState>>,
    Form(page_data): Form<BuildAllProfilesPageData>,
) -> Response {
    let profile_name = page_data.profile_name.trim();

    if profile_name.is_empty() || profile_name.contains(" ") {
        return (
            StatusCode::BAD_REQUEST,
            Html(
                r#"<div class="alert alert-danger">Please supply the profile name in your request and ensure it does not contain a space.</div>"#,
            ),
        ).into_response();
    }

    // Cleanse the input
    let profile_name = profile_name.replace(".toml", "");

    let creds = {
        let c = state.creds.read().await;
        c.clone().unwrap()
    };

    //
    // We want to send the request up to the C2, have it construct our 7z containing the binaries,
    // and serve the download back to the operator.
    //

    let result = api_request(
        AdminCommand::BuildAllBins((profile_name.clone(), ".".to_string(), None, None)),
        &IsTaskingAgent::No,
        &creds,
        Some(&format!("admin_bab?profile_name={}", profile_name)),
    )
    .await;

    match result {
        Ok(zip_bytes) => {
            let filename = format!("{profile_name}.7z");
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, "application/x-7z-compressed".parse().unwrap());
            headers.insert(
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename)
                    .parse()
                    .unwrap(),
            );
            headers.insert(CONTENT_LENGTH, zip_bytes.len().try_into().unwrap());

            (headers, Bytes::from(zip_bytes)).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Html(format!("Error: {e}"))).into_response(),
    }
}
