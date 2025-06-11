use axum::{Router, routing::get};
use crate::handlers::{hello, sync_trigger};
use crate::db::LeagueRepository;

pub fn create_router() -> Router<LeagueRepository> {
    Router::new()
        .route("/", get(hello))
        .route("/sync", get(sync_trigger))
}