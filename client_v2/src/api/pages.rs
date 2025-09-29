use crate::models::{ActiveTabData, AppState};
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use std::sync::Arc;

struct PageAttributes {
    active_page: &'static str,
    title: &'static str,
}

trait Page {
    fn page_attributes() -> PageAttributes;
}

#[derive(Template)]
#[template(path = "login.html")]
struct Login;

#[axum::debug_handler]
pub async fn serve_login() -> impl IntoResponse {
    Html(Login.render().unwrap())
}

#[derive(Template)]
#[template(path = "dash.html")]
struct Dash {
    tab_data: ActiveTabData,
    active_page: &'static str,
    title: &'static str,
}

impl Page for Dash {
    fn page_attributes() -> PageAttributes {
        PageAttributes {
            active_page: "dashboard",
            title: "Dashboard",
        }
    }
}

pub async fn serve_dash(state: State<Arc<AppState>>) -> impl IntoResponse {
    let lock = state.active_tabs.read().await;
    let tab_data = (lock.0, lock.1.clone());
    Html(
        Dash {
            tab_data,
            active_page: Dash::page_attributes().active_page,
            title: Dash::page_attributes().title,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "file_upload.html")]
struct FileUpload {
    active_page: &'static str,
    title: &'static str,
}

impl Page for FileUpload {
    fn page_attributes() -> PageAttributes {
        PageAttributes {
            active_page: "upload",
            title: "Upload file",
        }
    }
}

pub async fn upload_file_page() -> impl IntoResponse {
    Html(
        FileUpload {
            active_page: FileUpload::page_attributes().active_page,
            title: FileUpload::page_attributes().title,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "profile_builder.html")]
struct BuildAllProfilesPage {
    active_page: &'static str,
    title: &'static str,
}

impl Page for BuildAllProfilesPage {
    fn page_attributes() -> PageAttributes {
        PageAttributes {
            active_page: "build_profiles",
            title: "Build profiles",
        }
    }
}

pub async fn build_all_profiles_page() -> impl IntoResponse {
    Html(
        BuildAllProfilesPage {
            active_page: BuildAllProfilesPage::page_attributes().active_page,
            title: BuildAllProfilesPage::page_attributes().title,
        }
        .render()
        .unwrap(),
    )
}
