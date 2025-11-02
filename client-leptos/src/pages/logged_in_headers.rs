use leptos::{IntoView, component, view};
use leptos_router::hooks::use_navigate;

#[component]
pub fn LoggedInHeaders() -> impl IntoView {
    let navigate = use_navigate();

    // Are we logged in..
    // if !is_logged_in() {
    //     navigate("/", Default::default());
    // }

    view!()
}
