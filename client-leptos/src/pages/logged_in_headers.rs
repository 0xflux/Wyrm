use leptos::{IntoView, component, prelude::*, task::spawn_local, view};
use leptos_router::hooks::use_navigate;

use crate::controller::{BodyClass, apply_body_class, is_logged_in};

/// Creates the header section of a page which is behind token authentication; this will make a request to the
/// C2 to ensure that the user is logged in - whilst also applying the necessary styles for the logged in area.
///
/// This will render the nav bar, and anything you would expect to be in the 'headers' section, (not 'head').
#[component]
pub fn LoggedInHeaders() -> impl IntoView {
    // Apply the `app` class to the body for our CSS stuff
    apply_body_class(BodyClass::App);

    let (checked_login, set_checked_login) = signal(false);
    let (logged_in, set_logged_in) = signal(true);

    Effect::new(move |_| {
        if checked_login.get() {
            return;
        }

        set_checked_login.set(true);

        spawn_local({
            async move {
                let logged_in_result = is_logged_in().await;
                set_logged_in.set(logged_in_result);
                leptos::logging::log!("Is logged in: {logged_in_result}");
            }
        });
    });

    Effect::new(move || {
        if !logged_in.get() {
            let navigate = use_navigate();
            navigate("/", Default::default());
        }
    });

    view! {}
}
