use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shared::tasks::{AdminCommand, WyrmResult};
use shared_c2_client::StagedResourceData;

use crate::{
    models::AppState,
    net::{IsTaskingAgent, api_request},
};

pub async fn fetch_staged_resources(state: State<Arc<AppState>>) -> Response {
    let creds = {
        let c_lock = state.creds.read().await;
        c_lock.clone().unwrap()
    };

    let res = api_request(
        AdminCommand::ListStagedResources,
        &IsTaskingAgent::No,
        &creds,
        None,
    )
    .await;

    if let Ok(res) = res {
        let inner = match serde_json::from_slice::<WyrmResult<Vec<StagedResourceData>>>(&res) {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to deserialise data. {e}"),
                )
                    .into_response();
            }
        };

        println!("Inner: {:?}", inner.unwrap());
    }

    StatusCode::OK.into_response()
}
