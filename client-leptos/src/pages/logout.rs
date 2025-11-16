use leptos::{component, prelude::*};
use leptos_router::hooks::use_navigate;
use shared::tasks::AdminCommand;
use web_sys::{js_sys::Reflect, window};

use crate::net::{C2Url, IsTaskingAgent, api_request};

#[component]
pub fn Logout() -> impl IntoView {
    let send_request = Action::new_local(|_: &()| async move {
        api_request(
            AdminCommand::None,
            &IsTaskingAgent::No,
            None,
            C2Url::Standard,
            Some("logout_admin"),
        )
        .await
    });
    let logout_response = send_request.value();

    Effect::new(move |_| {
        logout_response.with(|inner| {
            if let Some(res) = inner {
                match res {
                    Ok(i) => leptos::logging::log!("Happy response: {}", i.len()),
                    Err(e) => {
                        leptos::logging::error!("Error in response: {e}");
                    }
                }

                // Clear the session cookie by setting it to expire in the past
                if let Some(window) = window() {
                    if let Some(doc) = window.document() {
                        let cookie_str = "session=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/;";
                        if let Ok(val) = Reflect::set(&doc, &"cookie".into(), &cookie_str.into()) {
                            if !val {
                                leptos::logging::error!("Failed to set cookie to clear 'session'");
                            }
                        }
                    }
                }

                let navigate = use_navigate();
                navigate("/", Default::default());
            }
        })
    });

    Effect::new(move |_| {
        send_request.dispatch(());
    });

    view! {}
}
