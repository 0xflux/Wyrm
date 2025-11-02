use leptos::{IntoView, component, logging::log, prelude::*, view};
use reactive_stores::Store;

use crate::{GlobalState, GlobalStateStoreFields};

#[component]
pub fn Dashboard() -> impl IntoView {
    let state = expect_context::<Store<GlobalState>>();
    let creds = state.credentials();

    log!("Creds from global: {:?}", creds.get());

    view! {
        <h1>"Hello world!"</h1>
    }
}
