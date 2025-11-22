use leptos::wasm_bindgen::JsCast;
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

    // Create a reactive signal for the current URL first segment and
    // initialize history hooks via helper to keep component body clean.
    let url_path = create_url_path_signal();

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
    let window = web_sys::window()?;
    let pathname = window.location().pathname().ok()?;

    let first_segment = pathname
        .split('/')
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();

    Some(first_segment)
}

fn create_url_path_signal() -> RwSignal<String> {
    let initial = extract_path().unwrap_or_else(|| "".to_string());
    let url_path = RwSignal::new(initial);

    if let Some(win) = web_sys::window() {
        if let Some(doc) = win.document() {
            if let Ok(script) = doc.create_element("script") {
                script.set_inner_html(r#"
                    (function(){
                        if (window.__wyrm_history_hook_installed) return;
                        const _push = history.pushState;
                        history.pushState = function(){ _push.apply(this, arguments); window.dispatchEvent(new Event('locationchange')); };
                        const _replace = history.replaceState;
                        history.replaceState = function(){ _replace.apply(this, arguments); window.dispatchEvent(new Event('locationchange')); };
                        window.addEventListener('popstate', function(){ window.dispatchEvent(new Event('locationchange')); });
                        window.__wyrm_history_hook_installed = true;
                    })();
                "#);

                if let Some(head) = doc.head() {
                    let _ = head.append_child(&script);
                }
            }
        }

        let url_path_clone = url_path.clone();
        let closure =
            leptos::wasm_bindgen::closure::Closure::wrap(Box::new(move |_ev: web_sys::Event| {
                let new = extract_path().unwrap_or_else(|| "".to_string());
                url_path_clone.set(new);
            }) as Box<dyn FnMut(_)>);

        let _ = win
            .add_event_listener_with_callback("locationchange", closure.as_ref().unchecked_ref());

        closure.forget();
    }

    url_path
}
