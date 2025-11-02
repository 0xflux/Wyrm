use leptos::{IntoView, component, prelude::*, view};

use crate::pages::logged_in_headers::LoggedInHeaders;

#[component]
pub fn Dashboard() -> impl IntoView {
    view! {
        <LoggedInHeaders />
        <h1>"Hello world!"</h1>
    }
}
