use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose};
use crypto::bcrypt::bcrypt;
use rand::{RngCore, rng};
use shared_c2_client::ADMIN_AUTH_SEPARATOR;

use crate::{
    app_state::AppState,
    logging::{
        log_admin_login_attempt, log_download_accessed, log_error_async, log_page_accessed_auth,
        log_page_accessed_no_auth,
    },
};

const BCRYPT_HASH_BYTES: usize = 24;
const BCRYPT_COST: u32 = 12;
const SALT_BYTES: usize = 16;

/// Authenticates access to an admin route via the `Authorization` header present with the request. This includes
/// encoded username/password which will be validated.
///
/// In the event there is no user in the db, a new one will be created. We make this secure by requiring a third
/// parameter sent in the headers which is a unique token set in the `.env` of the server to ensure we cannot be
/// vulnerable to remote takeover.
pub async fn authenticate_admin(
    State(state): State<Arc<AppState>>,
    addr: ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    // Extrapolate information from the request first of all and validate we got what we wanted.
    // We can do some unwraps here, as if the data is missing then the request is not likely to be
    // from an operator.
    // We have three parts to the request;
    // 1 -  username
    // 2 -  password
    // 3 -  a unique token the operator SHOULD have set up that prevents somebody taking over the server
    //      before the operator has had chance to create a user.
    //

    let admin_auth_header = request
        .headers()
        .get("Authorization")
        .unwrap()
        .to_str()
        .unwrap();
    let parts: Vec<&str> = admin_auth_header.split(ADMIN_AUTH_SEPARATOR).collect();
    if parts.len() != 3 {
        return StatusCode::NOT_FOUND.into_response();
    }

    let username = parts[0];
    let password = parts[1];
    let token = parts[2];

    let ip = &addr.to_string();

    //
    // Look up the user in the database; if they exist we can grab the salt and the hash - then hash
    // the password to validate.
    // If it is a brand new user (i.e. no rows returned) then we can insert a new user after hashing
    // the password.
    //

    // Use the token that should be set manually by the operator in installation to prevent a takeover attack
    // where an adversary could register an account on the C2 before the operator has had chance to. This is why
    // it is super important to set the admin token manually.
    if token.ne(&state.admin_token) {
        log_admin_login_attempt(username, password, token, ip, false).await;
        return StatusCode::NOT_FOUND.into_response();
    }

    // Lookup the operator from the db, if its empty we will create the user in the inner match here.
    let operator = match state.db_pool.lookup_operator(username).await {
        Ok(o) => o,
        Err(e) => {
            match e {
                sqlx::Error::RowNotFound => {
                    // The db is empty so create the user. The db insert function checks
                    // for us if a user already exists, if so, it will panic as we don't want anybody
                    // and everybody creating accounts! And we aren't yet multiplayer
                    create_new_operator(username, password, state.clone()).await;
                    log_admin_login_attempt(username, password, token, ip, true).await;
                    return next.run(request).await.into_response();
                }
                _ => {
                    log_error_async(&format!(
                        "There was an error with the db whilst trying to log in with creds: \
                        {username} {password}. {e}",
                    ))
                    .await;
                    log_admin_login_attempt(username, password, token, ip, false).await;
                    return StatusCode::NOT_FOUND.into_response();
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
            log_admin_login_attempt(username, password, token, ip, false).await;
            return StatusCode::NOT_FOUND.into_response();
        }

        if verify_password(password, &db_hash, &db_salt).await {
            log_admin_login_attempt(username, password, token, ip, true).await;
            return next.run(request).await.into_response();
        } else {
            log_admin_login_attempt(username, password, token, ip, false).await;
            return StatusCode::NOT_FOUND.into_response();
        }
    }

    log_admin_login_attempt(username, password, token, ip, false).await;
    // Anything that falls through to this point is invalid
    StatusCode::NOT_FOUND.into_response()
}

/// Verify the password passed into the admin route by comparing its calculated hash with the
/// expected hash from the db.
async fn verify_password(password: &str, password_hash: &str, salt: &str) -> bool {
    let salt = general_purpose::STANDARD
        .decode(salt)
        .expect("invalid base64");

    let expected_hash = general_purpose::STANDARD
        .decode(password_hash)
        .expect("invalid b64 on password");

    let password = password.to_string();

    // Validate with bcrypt on same salt
    let computed_hash: Vec<u8> = tokio::task::spawn_blocking(move || {
        let mut h = [0u8; BCRYPT_HASH_BYTES];
        bcrypt(BCRYPT_COST, &salt, password.as_bytes(), &mut h);
        h.to_vec()
    })
    .await
    .expect("bcrypt task panicked");

    computed_hash == expected_hash
}

/// Create a new operator in the database, taking in a plaintext password and hashing it with BCrypt
/// and a random salt.
///
/// The hashed password will be stored in the database, **not** the plaintext version.
async fn create_new_operator(username: &str, password: &str, state: Arc<AppState>) {
    let mut salt = [0u8; SALT_BYTES];
    rng().fill_bytes(&mut salt);

    let salt_clone = salt.to_vec();
    let password = password.to_string();

    let computed_hash = tokio::task::spawn_blocking(move || {
        let mut hash_output = [0u8; BCRYPT_HASH_BYTES];
        bcrypt(
            BCRYPT_COST,
            &salt_clone,
            password.as_bytes(),
            &mut hash_output,
        );

        hash_output.to_vec()
    })
    .await
    .expect("Could not compute hash in create_new_operator");

    let salt_b64 = general_purpose::STANDARD.encode(salt);
    let hash_b64 = general_purpose::STANDARD.encode(&computed_hash);

    state
        .db_pool
        .add_operator(username, &hash_b64, &salt_b64)
        .await
        .unwrap();
}

/// Authenticates an agent based on a header: `Authorization`. The agent will carry a security token which
/// was set by the operator so that we can verify the inbound connection DOES in fact relate to an agent under
/// our control.
///
/// This will reduce the attack surface of API's close to the database, and reduce the likelihood of a DDOS due to
/// batting the request off before we actually deal with it past middleware.
///
/// If the checks fail, a BAD_GATEWAY status will be returned, which may be a little more OPSEC savvy in that it may
/// throw off analysis thinking the server is down, whereas a 404 may indicate the server is active.
pub async fn authenticate_agent_by_header_token(
    State(state): State<Arc<AppState>>,
    addr: ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    let ip = &addr.to_string();

    //
    // First, we need to check whether the request is going to a URI in which a download is staged
    // as we do not want to gate keep that as requiring the Auth header.
    //

    let uri = request.uri().to_string();
    let uri = &uri[1..];
    {
        let lock = state.endpoints.read().await;

        if lock.download_endpoints.contains_key(uri) {
            log_download_accessed(uri, ip).await;
            return next.run(request).await.into_response();
        }
    }

    //
    // That URI wasn't requested, therefore we want to apply our auth check.
    //

    let h = match request.headers().get("Authorization") {
        Some(h) => h,
        None => {
            log_page_accessed_no_auth(uri, ip).await;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let auth_header = match h.to_str() {
        Ok(head) => head,
        Err(_) => {
            log_page_accessed_no_auth(uri, ip).await;
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    {
        let lock = state.agent_tokens.read().await;

        if lock.contains(auth_header) {
            // The happy path, token present
            log_page_accessed_auth(uri, ip).await;
            return next.run(request).await.into_response();
        }
    }

    // The unhappy path
    log_page_accessed_auth(uri, ip).await;
    StatusCode::BAD_GATEWAY.into_response()
}
