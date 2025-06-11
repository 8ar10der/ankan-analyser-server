use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct LeaguePlayer {
    pub id: i32,
    pub name: String,
}

impl LeaguePlayer {
    pub fn new(id: i32, name: String) -> Self {
        Self {
            id,
            name,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeagueGame {
    pub game_time: Option<NaiveDateTime>,
    pub season_num: i32,
    pub table_num: i32,
    pub processed: bool,
    pub id: i32,
    pub e: i32,
    pub s: i32,
    pub w: i32,
    pub n: Option<i32>,
}

impl LeagueGame {
    pub fn new(
        game_time: Option<NaiveDateTime>,
        season_num: i32,
        table_num: i32,
        processed: bool,
        id: i32,
        e: i32,
        s: i32,
        w: i32,
        n: Option<i32>,
    ) -> Self {
        Self {
            game_time,
            season_num,
            table_num,
            processed,
            id,
            e,
            s,
            w,
            n,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeagueResult {
    pub id: i32,
    pub table_id: i32,
    pub player_id: i32,
    pub result: f64,
    pub position: i32,
    pub uma: f64,
    pub penalty: f64,
    pub total: f64,
}

impl LeagueResult {
    pub fn new(
        id: i32,
        table_id: i32,
        player_id: i32,
        result: f64,
        position: i32,
        uma: f64,
        penalty: f64,
        total: f64,
    ) -> Self {
        Self {
            id,
            table_id,
            player_id,
            result,
            position,
            uma,
            penalty,
            total,
        }
    }
}