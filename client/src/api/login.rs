use std::{env, sync::Arc};

use axum::{
    Form,
    extract::State,
    http::HeaderMap,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use shared::tasks::AdminCommand;

use crate::{
    models::AppState,
    net::{Credentials, IsTaskingAgent, api_request},
};

#[derive(Debug, Deserialize)]
pub struct LoginFormData {
    pub c2: String,
    pub username: String,
    pub password: String,
}

pub async fn try_login(
    state: State<Arc<AppState>>,
    Form(login): Form<LoginFormData>,
) -> impl IntoResponse {
    let creds = Credentials {
        username: login.username.clone(),
        password: login.password.clone(),
        c2_url: login.c2.clone(),
    };

    let result = api_request(AdminCommand::Login, &IsTaskingAgent::No, &creds, None).await;

    let result_deser = match result {
        Ok(b) => match serde_json::from_slice::<String>(&b) {
            Ok(s) => s,
            Err(e) => {
                return Html(format!(
                    r#"<div class="mt-3 alert alert-danger" role="alert">Error reading response {}</div>"#,
                    e
                ))
                .into_response();
            }
        },
        Err(e) => {
            return Html(format!(
                r#"<div class="mt-3 alert alert-danger" role="alert">Error making request: {}</div>"#, 
                e
            ))
            .into_response();
        }
    };

    if result_deser == "success" {
        // Login successful
        let mut lock = state.creds.write().await;
        *lock = Some(creds);

        let mut headers = HeaderMap::new();
        headers.insert("HX-Redirect", "/dashboard".parse().unwrap());

        return (StatusCode::OK, headers).into_response();
    }

    let unknown_error = format!(
        r#"<div class="mt-3 alert alert-danger" role="alert">There was an unknown error whilst processing your request.
        please report this as a bug.</div>"#
    );
    Html(unknown_error).into_response()
}
