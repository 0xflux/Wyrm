use leptos::prelude::document;
use web_sys::HtmlElement;

use crate::net::admin_health_check;

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
