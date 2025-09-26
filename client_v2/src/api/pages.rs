use askama::Template;
use axum::response::{Html, IntoResponse};

#[derive(Template)]
#[template(path = "login.html")]
struct Login;

#[axum::debug_handler]
pub async fn serve_login() -> impl IntoResponse {
    Html(Login.render().unwrap())
}

#[derive(Template)]
#[template(path = "dash.html")]
struct Dash;

#[axum::debug_handler]
pub async fn serve_dash() -> impl IntoResponse {
    Html(Dash.render().unwrap())
}
