use std::net::SocketAddr;

use axum::{
    Router,
    routing::{get, post},
    serve,
};
use tower_http::services::ServeDir;

use crate::api::{login::try_login, pages::serve_login};

mod api;
mod net;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //
    // Environment setup
    //

    dotenvy::dotenv()
        .expect("dotenv file must be present and must contain admin token, refer to docs.");

    let static_files = ServeDir::new("static");

    //
    // Build the routes
    //

    let app = Router::new()
        .route("/", get(serve_login))
        .route("/do_login", post(try_login))
        .nest_service("/static", static_files);

    //
    // Serve the app
    //

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4040").await?;

    serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
