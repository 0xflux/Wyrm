use leptos::{IntoView, component, logging::log, prelude::*, view};
use reactive_stores::Store;

use crate::{
    GlobalState, models::GlobalStateStoreFields, pages::logged_in_headers::LoggedInHeaders,
};

#[component]
pub fn Dashboard() -> impl IntoView {
    let state = expect_context::<Store<GlobalState>>();
    let creds = state.credentials();

    view! {
        <LoggedInHeaders />
        <h1>"Hello world!"</h1>
    }
}
