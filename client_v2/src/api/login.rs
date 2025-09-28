use std::env;

use axum::{
    Form,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use shared::tasks::AdminCommand;

use crate::net::{Credentials, IsTaskingAgent, api_request};

#[derive(Debug, Deserialize)]
pub struct LoginFormData {
    pub c2: String,
    pub username: String,
    pub password: String,
}

pub async fn try_login(Form(login): Form<LoginFormData>) -> impl IntoResponse {
    let creds = Credentials {
        username: login.username.clone(),
        password: login.password.clone(),
        admin_env_token: env::var("ADMIN_TOKEN")
            .expect("could not find environment variable ADMIN_TOKEN"),
        c2_url: login.c2.clone(),
    };

    let result = api_request(AdminCommand::Login, &IsTaskingAgent::No, &creds).await;

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
    }

    let unknown_error = format!(
        r#"<div class="mt-3 alert alert-danger" role="alert">There was an unknown error whilst processing your request.
        please report this as a bug.</div>"#
    );
    Html(unknown_error).into_response()
}
