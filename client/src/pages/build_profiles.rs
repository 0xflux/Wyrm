use leptos::{component, prelude::*};
use shared::tasks::AdminCommand;

use crate::{
    controller::build_profiles::trigger_download,
    net::{C2Url, IsTaskingAgent, api_request},
    pages::logged_in_headers::LoggedInHeaders,
};

#[component]
pub fn BuildProfilesPage() -> impl IntoView {
    let form_data = RwSignal::new(String::new());
    let submitting = RwSignal::new(false);

    let submit_page = Action::new_local(|input: &String| {
        let input = input.clone();

        async move {
            // Cleanse the input
            let profile_name = input.replace(".toml", "");

            let result = api_request(
                AdminCommand::BuildAllBins((profile_name.clone(), ".".to_string(), None, None)),
                &IsTaskingAgent::No,
                None,
                C2Url::Standard,
                Some("admin_bab"),
            )
            .await;

            result.map(|bytes| (profile_name, bytes))
        }
    });
    let page_response = submit_page.value();

    Effect::new(move |_| {
        page_response.with(|inner| {
            if let Some(res) = inner {
                submitting.set(false);

                match res {
                    Ok((profile_name, bytes)) => {
                        if !bytes.is_empty() {
                            let filename = format!("{profile_name}.7z");
                            trigger_download(&filename, bytes);
                        } else {
                            leptos::logging::log!("Response was empty.");
                        }
                    }
                    Err(e) => {
                        leptos::logging::error!("Error parsing result: {e}");
                    }
                }
            }
        })
    });

    view! {

        <LoggedInHeaders />

        <div id="file-upload-container" class="container-fluid py-4 app-page">
            <div class="row mb-4">
                <div class="col-12 text-center">
                    <h2 class="mb-2 fw-bold">Build all agents</h2>
                    <p>
                        "Type the name of the profile you wish to build from (do not include the "<code>".toml"</code>")."
                        "For example, to build from the default profile, type "<code>"default"</code>"."
                    </p>
                    <p>This builder will serve you the generated payloads as a 7zip archive for which you can do with as you please.
                        It is recommended after using this, you use the upload function to stage a payload on the C2.
                    </p>
                </div>
            </div>
            <div class="row justify-content-center">
                <div class="col-md-7 col-lg-6">

                    <form
                        on:submit=move |ev| {
                            ev.prevent_default(); // dont reload

                            submitting.set(true);
                            submit_page.dispatch(form_data.get());
                        }
                        id="stage-all-form"
                        autocomplete="off"
                        class="border rounded-3 p-4 shadow-sm">

                        <div class="mb-3">
                            <label for="profile_name" class="form-label fw-semibold">Profile name</label>
                            <input
                                type="text"
                                class="form-control"
                                name="profile_name"
                                id="profile_name"
                                placeholder="Profile name"
                                bind:value=form_data
                                required
                                />
                            <div class="form-text">The profile name <strong>should not include the toml extension</strong>, and it should be present under <code>c2/profiles/</code>.</div>
                        </div>

                        <button type="submit" class="btn btn-primary w-100 py-2 fw-bold" disabled=move || submitting.get()>
                            {move || if submitting.get() { "Building..." } else { "Build" }}
                        </button>
                        <div class="form-text">
                            Please do not refresh or navigate away from the page. The builder will return you a 7zip archive
                            containing the agent binaries. Note: This may take some time, and unless you get an error message - <strong>please
                            wait and allow it to serve you the files</strong>.
                        </div>
                    </form>

                    <div id="response-box" class="mt-3"></div>
                </div>
            </div>
        </div>
    }
}
