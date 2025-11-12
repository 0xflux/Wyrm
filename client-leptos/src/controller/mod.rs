use leptos::prelude::{document, window};
use web_sys::HtmlElement;

use crate::{models::C2_STORAGE_KEY, net::admin_health_check};

pub mod dashboard;

pub enum BodyClass {
    Login,
    App,
}

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
pub fn get_c2_url_from_browser() -> Option<String> {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(C2_STORAGE_KEY).ok())
        .unwrap_or_default()
}
