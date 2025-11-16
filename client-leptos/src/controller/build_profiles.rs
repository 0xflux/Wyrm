use leptos::wasm_bindgen::JsCast;
use web_sys::{
    Blob, BlobPropertyBag, Url,
    js_sys::{self, Uint8Array},
    window,
};

/// Trigger a local download given an input file name and byte body.
pub fn trigger_download(filename: &str, bytes: &[u8]) {
    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document");
    let body = document.body().expect("document should have a body");

    // Vec<u8> -> Uint8Array
    let uint8_array = Uint8Array::from(bytes);

    let parts = js_sys::Array::new();
    parts.push(&uint8_array.buffer());

    let props = BlobPropertyBag::new();
    props.set_type("application/x-7z-compressed");

    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &props)
        .expect("failed to create Blob");

    let url = Url::create_object_url_with_blob(&blob).expect("failed to create Object URL");

    let a = document
        .create_element("a")
        .expect("create_element failed")
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .expect("element should be an HtmlAnchorElement");

    a.set_href(&url);
    a.set_download(filename);

    body.append_child(&a).expect("append_child failed");
    a.click();
    body.remove_child(&a).expect("remove_child failed");

    Url::revoke_object_url(&url).expect("revoke_object_url failed");
}
