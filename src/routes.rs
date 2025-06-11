use axum::{Router, routing::get};
use crate::handlers::{hello, sync_trigger};

pub fn create_router() -> Router {
    Router::new()
        .route("/", get(hello))
        .route("/sync", get(sync_trigger))
}