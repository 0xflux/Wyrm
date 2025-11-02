use leptos::prelude::*;
use leptos_meta::{Meta, Title, provide_meta_context};
use leptos_router::{components::*, path};
use reactive_stores::Store;

use crate::pages::{
    dashboard::Dashboard,
    login::{Login, LoginData},
};

mod net;
mod pages;

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    leptos::mount::mount_to_body(App)
}

#[component]
fn App() -> impl IntoView {
    provide_meta_context();
    provide_context(Store::new(GlobalState::default()));

    view! {
        <Title text="Login | Wyrm C2 Panel" />
        <Meta charset="UTF-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1.0" />

        <Router>
            <Routes fallback=|| view! { NotFound }>
                <Route path=path!("/") view=Login />
                <Route path=path!("/dashboard") view=Dashboard />
            </Routes>
        </Router>
    }
}

#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    credentials: LoginData,
}
