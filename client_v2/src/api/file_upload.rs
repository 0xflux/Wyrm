use axum::{
    Form,
    extract::Multipart,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use shared::pretty_print::print_failed;

#[derive(Default)]
pub struct FileUploadData {
    download_name: String,
    download_api: String,
    file_data: Vec<u8>,
}

pub async fn upload_file_api(mut multipart: Multipart) -> Response {
    let mut form_data = FileUploadData::default();

    while let Some(field) = multipart.next_field().await.ok().flatten() {
        let field_name = field.name().unwrap_or_default();
        match field_name {
            "download_name" => form_data.download_name = field.text().await.unwrap_or_default(),
            "staging_uri" => form_data.download_api = field.text().await.unwrap_or_default(),
            "file_input" => form_data.file_data = field.bytes().await.unwrap_or_default().to_vec(),
            _ => (),
        }
    }

    println!(
        "Received: {}, at: /{}, len: {}",
        form_data.download_name,
        form_data.download_api,
        form_data.file_data.len()
    );
    StatusCode::OK.into_response()
}
