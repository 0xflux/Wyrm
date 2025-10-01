use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use shared::tasks::{AdminCommand, FileUploadStagingFromClient, WyrmResult};

use crate::{
    models::AppState,
    net::{IsTaskingAgent, api_request},
};

#[derive(Default)]
pub struct FileUploadData {
    download_name: String,
    download_api: String,
    file_data: Vec<u8>,
}

pub async fn upload_file_api(state: State<Arc<AppState>>, mut multipart: Multipart) -> Response {
    let mut form_data = FileUploadData::default();

    while let Some(field) = multipart.next_field().await.ok().flatten() {
        let field_name = field.name().unwrap_or_default();
        match field_name {
            "download_name" => form_data.download_name = field.text().await.unwrap_or_default(),
            "staging_uri" => form_data.download_api = field.text().await.unwrap_or_default(),
            "file_input" => form_data.file_data = field.bytes().await.unwrap_or_default().to_vec(),
            _ => (),
        }
    }

    let mut download_api = form_data.download_api.trim();

    if form_data.download_name.is_empty() || download_api.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Html(r#"<div class="alert alert-danger">Fields cannot be empty.</div>"#),
        )
            .into_response();
    }

    if form_data.file_data.len() == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Html(r#"<div class="alert alert-danger">File upload must not be empty.</div>"#),
        )
            .into_response();
    }

    if download_api.contains(" ") {
        return (
            StatusCode::BAD_REQUEST,
            Html(r#"<div class="alert alert-danger">Download API cannot contain a space.</div>"#),
        )
            .into_response();
    }

    if download_api.starts_with("/") {
        download_api = download_api.strip_prefix("/").unwrap().into();
    }

    let staging_info = FileUploadStagingFromClient {
        download_name: form_data.download_name,
        api_endpoint: download_api.into(),
        file_data: form_data.file_data,
    };

    let creds_lock = {
        state
            .creds
            .read()
            .await
            .clone()
            .expect("credentials not found")
    };

    let response: WyrmResult<String> = match api_request(
        AdminCommand::StageFileOnC2(staging_info),
        &IsTaskingAgent::No,
        &creds_lock,
        None,
    )
    .await
    {
        Ok(r) => match serde_json::from_slice(&r) {
            Ok(r) => r,
            Err(e) => {
                // TODO return bad
                println!("An error was encountered deserialising the response, {e}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        },
        Err(e) => {
            println!("An error was encountered uploading your file, {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let WyrmResult::Err(e) = response {
        // TODO
        println!("RESPONSE ERR: {:?}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    } else {
        return Redirect::to("/dashboard").into_response();
    }
}
