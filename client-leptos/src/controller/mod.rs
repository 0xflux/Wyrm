use leptos::{
    logging::log,
    prelude::{Get, expect_context},
};
use reactive_stores::Store;

use crate::models::{GlobalState, GlobalStateStoreFields};

pub fn is_logged_in() -> bool {
    let state = expect_context::<Store<GlobalState>>();
    let creds = state.credentials().get();

    log!("Creds is: {:?}", creds);

    if creds.is_none() {
        return false;
    };

    true
}
