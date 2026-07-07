use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    handlers::file_handler::{download_handler, upload_handler},
    types::app_state::AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let file_routes = Router::new()
        .route("/upload", post(upload_handler))
        .route("/download", get(download_handler));

    Router::new()
        .route("/", get(|| async { "Auth Service Running" }))
        .nest("/file", file_routes)
        .with_state(app_state)
}
