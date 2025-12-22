use gloo_net::http::Request;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;
use leptos::{IntoView, component, prelude::*, view};
use leptos_router::hooks::use_navigate;
use web_sys::{
    FileReader, FormData, HtmlFormElement, HtmlInputElement, RequestCredentials, js_sys,
    wasm_bindgen,
};

use crate::controller::get_item_from_browser_store;
use crate::models::C2_STORAGE_KEY;
use crate::pages::logged_in_headers::LoggedInHeaders;

#[component]
pub fn FileUploadPage() -> impl IntoView {
    let submitting = RwSignal::new(false);

    view! {
        <LoggedInHeaders />

        <div id="file-upload-container" class="container-fluid py-4 app-page">
            <div class="row mb-4">
                <div class="col-12 text-center">
                    <h2 class="mb-2 fw-bold">Upload a File</h2>
                    <p>Easily stage files for download and delivery, use the below form to upload a file. Note, the maximum upload size is
                        whatever you set in your environment settings, or defaults to 500 mb.
                    </p>
                </div>
            </div>
            <div class="row justify-content-center">
                <div class="col-md-7 col-lg-6">
                    <form
                        id="file-upload-form"
                        autocomplete="off"
                        enctype="multipart/form-data"
                        on:submit=move |ev| {
                            use wasm_bindgen::closure::Closure;

                            ev.prevent_default();
                            submitting.set(true);

                            let form = ev.target().unwrap().dyn_into::<HtmlFormElement>().unwrap();

                            let download_name = form
                                .elements()
                                .named_item("download_name")
                                .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
                                .map(|input| input.value())
                                .unwrap_or_default();
                            let staging_uri = form
                                .elements()
                                .named_item("staging_uri")
                                .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
                                .map(|input| input.value())
                                .unwrap_or_default();
                            let file_input = form
                                .elements()
                                .named_item("file_input")
                                .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
                                .and_then(|input| input.files())
                                .and_then(|files| files.get(0));

                            let mut download_api = staging_uri.trim().to_string();
                            if download_api.starts_with("/") {
                                download_api = download_api.strip_prefix("/").unwrap().to_string();
                            }

                            if let Some(file) = file_input {
                                let file_reader = FileReader::new().unwrap();
                                let fr_c = file_reader.clone();
                                let download_name = download_name.clone();
                                let download_api = download_api.clone();

                                let navigate = use_navigate();
                                let status_el = web_sys::window()
                                    .and_then(|w| w.document())
                                    .and_then(|d| d.get_element_by_id("upload-status"));

                                let c2_addr = get_item_from_browser_store::<String>(C2_STORAGE_KEY)
                                    .unwrap_or_default()
                                    .replace("\"", "");
                                let c2_addr = c2_addr.trim_end_matches('/').to_string();

                                let value = file.clone();
                                let onload = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                                    let result = fr_c.result().unwrap();
                                    let _array = js_sys::Uint8Array::new(&result);

                                    let form = FormData::new().unwrap();
                                    form.append_with_str("download_name", &download_name).unwrap();
                                    form.append_with_str("api_endpoint", &download_api).unwrap();
                                    form.append_with_blob("file", &value).unwrap();

                                    let url = format!("{}/admin_upload", c2_addr);

                                    let status_el = status_el.clone();
                                    let navigate = navigate.clone();

                                    spawn_local(async move {
                                        let resp = Request::post(&url)
                                            .credentials(RequestCredentials::Include)
                                            .body(form)
                                            .unwrap()
                                            .send()
                                            .await;

                                        match resp {
                                            Ok(r) if r.status() == 202 => {
                                                if let Some(el) = status_el.as_ref() {
                                                    el.set_inner_html("Upload complete.");
                                                }
                                                navigate("/dashboard", Default::default());
                                            }
                                            Ok(r) => {
                                                if let Some(el) = status_el.as_ref() {
                                                    el.set_inner_html(&format!(
                                                        "Upload failed. Status {}",
                                                        r.status()
                                                    ));
                                                }
                                            }
                                            Err(e) => {
                                                if let Some(el) = status_el.as_ref() {
                                                    el.set_inner_html(&format!(
                                                        "Upload failed. {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                        submitting.set(false);
                                    });
                                }) as Box<dyn FnMut(_)>);

                                file_reader.set_onload(onload.as_ref().dyn_ref());
                                file_reader.read_as_array_buffer(&file).unwrap();
                                onload.forget();
                            } else {
                                submitting.set(false);
                            }

                        }
                        class="border rounded-3 p-4 shadow-sm"
                        >
                        <div class="mb-3">
                            <label for="download_name" class="form-label fw-semibold">Download Name (INCLUDING file extension)</label>
                            <input type="text" class="form-control" placeholder="invoice.pdf" name="download_name" id="download_name" required />
                            <div class="form-text">Include the file extension (e.g. <strong>.pdf</strong>, <strong>.exe</strong>). This is the name that will be downloaded onto the machine of the person downloading (e.g. what the browser will save it as), unless you grab it programmatically.</div>
                        </div>
                        <div class="mb-3">
                            <label for="staging_uri" class="form-label fw-semibold">Staging C2 URI Endpoint</label>
                            <input type="text" class="form-control" placeholder="contracts/microsoft/2025/msft_contract_25&auth=..." name="staging_uri" id="staging_uri" required />
                            <div class="form-text">Multi-path and URL params allowed. Example: <code>download</code> or <code>files/secret?auth=token</code>. Note: the server will reject the path if it contains a space, so do not include a space here.</div>
                        </div>
                        <div class="mb-3">
                            <label for="file_input" class="form-label fw-semibold">Choose File</label>
                            <input class="form-control" type="file" id="file_input" name="file_input" required />
                        </div>
                        <button type="submit" class="btn btn-primary w-100 py-2 fw-bold" disabled=move || submitting.get()>
                            {move || if submitting.get() { "Uploading..." } else { "Upload" }}
                        </button>
                    </form>
                    <div id="upload-status" class="mt-3"></div>
                </div>
            </div>
        </div>
    }
}
