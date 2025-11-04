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
            }
        });
    });

    Effect::new(move || {
        if !logged_in.get() {
            let navigate = use_navigate();
            navigate("/", Default::default());
        }
    });

    let url_path = match extract_path() {
        Some(p) => RwSignal::new(p),
        None => {
            leptos::logging::log!("Could not get path for current URL.");
            let navigate = use_navigate();
            navigate("/", Default::default());
            RwSignal::new("".to_string())
        }
    };

    //
    // Build the header section of the page
    //
    view! {
    <nav class="navbar navbar-expand-lg">
    <div class="container-fluid">
        <a class="navbar-brand plain" href="#">Wyrm C2</a>
        <button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarSupportedContent" aria-controls="navbarSupportedContent" aria-expanded="false" aria-label="Toggle navigation">
        <span class="navbar-toggler-icon"></span>
        </button>

        <div class="collapse navbar-collapse" id="navbarSupportedContent">
        <ul class="navbar-nav me-auto mb-2 mb-lg-0">
            <li class="nav-item">
                <a  class="nav-link"
                    class=("active", move || url_path.get().eq("dashboard"))
                    aria-current="page"
                    href="/dashboard">
                    Dashboard
                </a>
            </li>
            <li class="nav-item">
                <a  class="nav-link"
                    class=("active", move || url_path.get().eq("file_upload"))
                    aria-current="page"
                    href="/file_upload">
                    Upload
                </a>
            </li>
            <li class="nav-item dropdown">
            <a class="nav-link dropdown-toggle"
                    href="#"
                    role="button"
                    data-bs-toggle="dropdown"
                    aria-expanded="false">
                Preparation
            </a>
            <ul class="dropdown-menu">
                <li>
                    <a  class="dropdown-item"
                        class=("active", move || url_path.get().eq("build_profiles"))
                        href="/build_profiles">
                    Build all agents
                    </a>
                </li>
                <li><a class="dropdown-item disabled" href="#">Website clone</a></li>
                <li><hr class="dropdown-divider" /></li>
                <li>
                    <a  class="dropdown-item"
                        class=("active", move || url_path.get().eq("staged_resources"))
                        href="/staged_resources">
                        View staged resources
                    </a>
                </li>
            </ul>
            </li>
            <li class="nav-item">
                <a class="nav-link" href="/logout">Logout</a>
            </li>
        </ul>
        </div>
    </div>
    </nav>

    }
}

fn extract_path() -> Option<String> {
    let uri = document().document_uri().expect("could not get uri");
    let split = uri.split(':').collect::<Vec<&str>>();
    let remaining = split.get(2)?;
    let path = remaining.split_once('/')?.1.to_string();

    Some(path)
}
