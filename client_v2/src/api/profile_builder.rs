use std::sync::Arc;

use crate::{
    models::AppState,
    net::{IsTaskingAgent, api_request},
};
use axum::{
    Form, debug_handler,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
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
    if page_data.profile_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Html(
                r#"<div class="alert alert-danger">Please supply the profile name in your request.</div>"#,
            ),
        ).into_response();
    }

    // Cleanse the input
    let profile_name = page_data.profile_name.replace(".toml", "");

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

    StatusCode::OK.into_response()
}
