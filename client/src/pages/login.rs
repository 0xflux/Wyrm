use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use shared::tasks::AdminCommand;

use crate::{
    controller::{BodyClass, apply_body_class, store_item_in_browser_store},
    models::{C2_STORAGE_KEY, LoginData},
    net::{ApiError, C2Url, IsTaskingAgent, api_request},
};

#[component]
pub fn Login() -> impl IntoView {
    let navigate = use_navigate();

    let c2_addr = RwSignal::new("".to_string());
    let username = RwSignal::new("".to_string());
    let password = RwSignal::new("".to_string());
    let login_data = RwSignal::new(LoginData::default());

    // Inner HTML container for the error box
    let login_box_html = RwSignal::new("".to_string());

    let submit_page = Action::new_local(|input: &LoginData| {
        let input = input.clone();
        async move {
            api_request(
                AdminCommand::Login,
                &IsTaskingAgent::No,
                Some((input.username, input.password)),
                C2Url::Custom(input.c2_addr),
                None,
            )
            .await
        }
    });
    let submit_value = submit_page.value();

    Effect::new(move |_| {
        submit_value.with(|inner| {
            if let Some(response) = inner {
                match response {
                    Ok(_) => {
                        store_item_in_browser_store(
                            C2_STORAGE_KEY, 
                            &c2_addr.get()
                        ).expect("could not store c2 url");

                        navigate("/dashboard", Default::default());
                    }
                    Err(e) => match e {
                        ApiError::Reqwest(e) => {
                            login_box_html.set(
                                format!(r#"<div class="mt-3 alert alert-danger" role="alert">Error making request: {}</div>"#, e)
                            );
                        },
                        ApiError::BadStatus(code, _) => {
                            if *code == 404 {
                                login_box_html.set(format!(r#"<div class="mt-3 alert alert-danger" role="alert">Invalid credentials</div>"#, ));
                            } else {
                                login_box_html.set(format!(r#"<div class="mt-3 alert alert-danger" role="alert">Error making request: {}</div>"#, e));
                            }
                        },
                    },
                }
            }
        })
    });

    apply_body_class(BodyClass::Login);

    view! {
        <div class="login-container">
        <div class="grid text-center">

            <form
                on:submit=move |ev| {
                    ev.prevent_default(); // dont reload

                    login_data.set(LoginData {
                        c2_addr: c2_addr.get(),
                        username: username.get(),
                        password: password.get(),
                    });

                    submit_page.dispatch(login_data.get());
                }
                autocomplete="off"
                class="form-signin">

                <img class="mb-4 logo" src="/static/wyrm_portrait.png" alt="" />
                <h1 class="h3 mb-3 font-weight-normal">
                    "Please sign in"
                </h1>

                <label for="c2" class="sr-only">C2 address (and port if non-standard)</label>
                <input
                    bind:value=c2_addr
                    type="url"
                    id="c2"
                    name="c2"
                    class="form-control"
                    placeholder="https://myc2.com" required autofocus />

                <label for="username" class="sr-only">Username</label>
                <input
                    bind:value=username
                    type="username"
                    id="username"
                    name="username"
                    class="form-control" placeholder="Username" required />

                <label for="password" class="sr-only">Password</label>
                <input
                    bind:value=password
                    type="password"
                    id="password"
                    name="password"
                    class="form-control" placeholder="Password" required />

                <button
                    type="submit"
                    class="btn btn-lg btn-primary btn-block">
                    "Sign in"
                </button>

                <div id="login-box" inner_html=login_box_html></div>

            </form>

            <footer>
                <p class="mt-5 mb-3">
                    "© Wyrm C2 "
                    <a href="https://github.com/0xflux/" target="_blank">
                        0xflux
                    </a>
                </p>
            </footer>
        </div>
        </div>
    }
}
