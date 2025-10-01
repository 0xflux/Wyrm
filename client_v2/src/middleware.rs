use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};

use crate::models::AppState;

pub async fn check_logged_in(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    // public allowlist
    let path = req.uri().path();
    if path == "/" || path == "/api/do_login" || path.starts_with("/static/") {
        return next.run(req).await;
    }

    //
    // Protect all other pages and API's, ensuring we have creds in the bank
    //

    let lock = state.creds.read().await;

    if lock.is_none() {
        return Redirect::temporary("/").into_response();
    }

    next.run(req).await
}
