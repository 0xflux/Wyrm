use anyhow::bail;
use leptos::prelude::{document, window};
use serde::{Serialize, de::DeserializeOwned};
use web_sys::HtmlElement;

use crate::net::admin_health_check;

pub mod build_profiles;
pub mod dashboard;

pub enum BodyClass {
    Login,
    App,
}

/// Returns the browser storage key for a user's chat history.
pub fn wyrm_chat_history_browser_key(uid: &str) -> String {
    format!("WYRM_C2_HISTORY_{}", uid)
}

/// Switches the document body's CSS class between login and app states.
///
/// Ensures only one of the two exclusive classes (`login` or `app`) is applied to the body
/// element at any time, enabling distinct styling for different application states.
pub fn apply_body_class(target: BodyClass) {
    let body: HtmlElement = document().body().expect("no <body>");

    match target {
        BodyClass::Login => {
            let _ = body.class_list().remove_1("app");
            let _ = body.class_list().add_1("login");
        }
        BodyClass::App => {
            let _ = body.class_list().remove_1("login");
            let _ = body.class_list().add_1("app");
        }
    }
}

pub async fn is_logged_in() -> bool {
    admin_health_check().await
}

/// Retrieves the saved C2 URL entered by the operator as a `String` if located
pub fn get_item_from_browser_store<T>(key: &str) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let x = window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(key).ok())
        .unwrap_or_default();

    if let Some(x_inner) = x {
        // Inner is stored as a JSON serialised String
        return Ok(serde_json::from_str(&x_inner)?);
    }

    bail!("Could not find key: {key}")
}

/// Serialises and stores an item in the browser's local storage.
///
/// # Error
/// Returns an error if JSON serialisation fails.
pub fn store_item_in_browser_store<T: Serialize>(key: &str, item: &T) -> anyhow::Result<()> {
    let ser = serde_json::to_string(item)?;

    let _ = window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.set_item(key, &ser).ok());

    Ok(())
}

/// Removes an item from the browser's local storage.
///
/// Silently handles cases where local storage is unavailable or the deletion fails,
/// logging errors for debugging purposes.
pub fn delete_item_in_browser_store(key: &str) {
    let _: Option<()> = window().local_storage().ok().flatten().and_then(|s| {
        if let Err(e) = s.remove_item(key) {
            leptos::logging::log!("Error deleting chat: {e:?}");
        }

        None
    });
}
