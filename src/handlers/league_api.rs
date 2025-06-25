use axum::{extract::{State, Path, Query}, Json};
use crate::db::LeagueRepository;
use crate::models::league::{GameInfo};
use std::collections::HashMap;

// 获取所有玩家名字列表
pub async fn get_players(State(repo): State<LeagueRepository>) -> Json<Vec<String>> {
    let players = repo.get_all_players().await;
    let names = players.into_iter().map(|p| p.name).collect();
    Json(names)
}

// 获取指定玩家的所有对战数据
pub async fn get_player_matches(
    State(repo): State<LeagueRepository>,
    Path(name): Path<String>,
) -> Json<Vec<GameInfo>> {
    let matches = repo.get_player_matches(&name).await;
    Json(matches)
}

// 获取所有赛季编号
pub async fn get_seasons(State(repo): State<LeagueRepository>) -> Json<Vec<i32>> {
    let seasons = repo.get_all_seasons().await;
    Json(seasons)
}

// 支持赛季参数的玩家列表
pub async fn get_players_by_season(
    State(repo): State<LeagueRepository>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<String>> {
    if let Some(season) = params.get("season") {
        if let Ok(season_num) = season.parse::<i32>() {
            let players = repo.get_players_by_season(season_num).await;
            let names = players.into_iter().map(|p| p.name).collect();
            return Json(names);
        }
    }
    // 无season参数时返回全部
    let players = repo.get_all_players().await;
    let names = players.into_iter().map(|p| p.name).collect();
    Json(names)
}

// 支持赛季参数的对战数据
pub async fn get_player_matches_by_season(
    State(repo): State<LeagueRepository>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<GameInfo>> {
    if let Some(season) = params.get("season") {
        if let Ok(season_num) = season.parse::<i32>() {
            let matches = repo.get_player_matches_by_season(&name, season_num).await;
            return Json(matches);
        }
    }
    let matches = repo.get_player_matches(&name).await;
    Json(matches)
}
