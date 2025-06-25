use axum::{Router, routing::get};
use crate::handlers::{sync_trigger, get_players_by_season, get_player_matches_by_season, get_seasons};
use crate::db::LeagueRepository;

pub fn create_router() -> Router<LeagueRepository> {
    Router::new()
        .route("/", get(|| async {
            axum::response::Html(std::fs::read_to_string("static/index.html").unwrap_or_else(|_| "<h1>未找到主页</h1>".to_string()))
        }))
        .route("/sync", get(sync_trigger))
        .route("/seasons", get(get_seasons))
        .route("/players", get(get_players_by_season))
        .route("/player/{name}/matches", get(get_player_matches_by_season))
}