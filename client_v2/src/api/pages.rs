use std::sync::Arc;

use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};

use crate::models::{ActiveTabData, AppState};

#[derive(Template)]
#[template(path = "login.html")]
struct Login;

#[axum::debug_handler]
pub async fn serve_login() -> impl IntoResponse {
    Html(Login.render().unwrap())
}

#[derive(Template)]
#[template(path = "dash.html")]
struct Dash {
    tab_data: ActiveTabData,
}

#[axum::debug_handler]
pub async fn serve_dash(state: State<Arc<AppState>>) -> impl IntoResponse {
    let lock = state.active_tabs.read().await;
    let tab_data = (lock.0, lock.1.clone());
    Html(Dash { tab_data }.render().unwrap())
}
