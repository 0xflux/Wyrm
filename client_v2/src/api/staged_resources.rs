use std::sync::Arc;

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use shared::tasks::{AdminCommand, WyrmResult};
use shared_c2_client::StagedResourceData;

use crate::{
    models::AppState,
    net::{IsTaskingAgent, api_request},
};

#[derive(Template)]
#[template(path = "htmx_applets/staged_resource_rows.html")]
pub struct StagedResourceRowsApplet {
    inner: Vec<StagedResourcesRowInner>,
}

pub struct StagedResourcesRowInner {
    download_name: String,
    uri: String,
    // TODO
    _num_downloads: usize,
}

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

        // SAFETY: Should be guaranteed at this point
        let inner = inner.unwrap();
        let mut staged_rows = Vec::<StagedResourcesRowInner>::with_capacity(inner.len());

        for line in inner {
            staged_rows.push(StagedResourcesRowInner {
                download_name: line.pe_name,
                uri: line.staged_endpoint,
                _num_downloads: 0,
            });
        }

        let page = StagedResourceRowsApplet { inner: staged_rows };

        return Html(page.render().unwrap()).into_response();
    }

    StatusCode::OK.into_response()
}
