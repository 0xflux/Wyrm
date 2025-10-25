use std::{env, io::Write, path::PathBuf};

use chrono::{SecondsFormat, Utc};
use shared::pretty_print::print_failed;
use tokio::io::AsyncWriteExt;

use crate::{ACCESS_LOG, ERROR_LOG, LOG_PATH, LOGIN_LOG};

pub async fn log_download_accessed(uri: &str, addr: &str) {
    let mut path = PathBuf::from(LOG_PATH);
    path.push(ACCESS_LOG);

    let msg = format!("Download accessed: /{uri}.");

    log(&path, &msg, Some(addr)).await;
}

pub async fn log_page_accessed_no_auth(uri: &str, addr: &str) {
    if let Ok(v) = env::var("DISABLE_ACCESS_LOG") {
        if v == "1" {
            return;
        }
    }

    let mut path = PathBuf::from(LOG_PATH);
    path.push(ACCESS_LOG);

    let msg = format!("Unauthenticated request at: /{uri}.");

    log(&path, &msg, Some(addr)).await;
}

pub async fn log_page_accessed_auth(uri: &str, addr: &str) {
    if let Ok(v) = env::var("DISABLE_ACCESS_LOG")
        && v == "1"
    {
        return;
    }

    let mut path = PathBuf::from(LOG_PATH);
    path.push(ACCESS_LOG);

    let msg = format!("Authenticated request at: /{uri}.");

    log(&path, &msg, Some(addr)).await;
}

pub async fn log_admin_login_attempt(
    username: &str,
    password: &str,
    token: &str,
    addr: &str,
    success: bool,
) {
    if let Ok(v) = env::var("DISABLE_LOGIN_LOG")
        && v == "1"
    {
        return;
    }

    let mut path = PathBuf::from(LOG_PATH);
    path.push(LOGIN_LOG);

    // check if IP is unique, for size concerns only log those
    let r = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let (ip, _) = addr.split_once(":").unwrap();
    let msg = if r.contains(ip) && success {
        // Don't log success attempts after the addr has already logged in successfully
        return;
    } else if r.contains(addr) && !success {
        format!(
            "[REPEAT ATTEMPT] Login {success}. Username: {username}, Password: REDACTED, Token: {token}"
        )
    } else if !success {
        if let Ok(v) = env::var("DISABLE_PLAINTXT_PW_BAD_LOGIN") {
            if v == "1" {
                format!(
                    "Login {success}. Username: {username}, Password: [REDACTED], Token: {token}"
                )
            } else {
                format!(
                    "Login {success}. Username: {username}, Password: {password}, Token: {token}"
                )
            }
        } else {
            format!("Login {success}. Username: {username}, Password: {password}, Token: {token}")
        }
    } else {
        // Dont log plaintext password in the event of a successful login..
        format!("Login {success}. Username: {username}, Password: [REDACTED], Token: {token}")
    };

    log(&path, &msg, Some(addr)).await;
}

pub fn log_error(message: &str) {
    let mut path = PathBuf::from(LOG_PATH);
    path.push(ERROR_LOG);

    log_sync(&path, message, None);
}

pub async fn log_error_async(message: &str) {
    let mut path = PathBuf::from(LOG_PATH);
    path.push(ERROR_LOG);

    print_failed(message);

    log(&path, message, None).await
}

/// An internal function to log an event to a given log file.
///
/// This function takes care of adding the date and IP to the log for consistency, and also appends
/// a newline at the end of the line.
async fn log(path: &PathBuf, message: &str, addr: Option<&str>) {
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .append(true)
        .open(path)
        .await;

    let message = construct_msg(addr, message);

    if let Ok(mut file) = file {
        let _ = file.write(message.as_bytes()).await;
    }
}

fn log_sync(path: &PathBuf, message: &str, addr: Option<&str>) {
    let msg = construct_msg(addr, message);

    let file = std::fs::OpenOptions::new()
        .read(true)
        .append(true)
        .open(path);

    if let Ok(mut file) = file {
        let _ = file.write(msg.as_bytes());
    }
}

fn construct_msg(addr: Option<&str>, message: &str) -> String {
    let time_now = Utc::now();
    let time_now = time_now.to_rfc3339_opts(SecondsFormat::Secs, true);

    if let Some(addr) = addr {
        let (ip, _) = addr.split_once(":").unwrap();
        format!("[{time_now}] [{ip}] {message}\n")
    } else {
        format!("[{time_now}] {message}\n")
    }
}
