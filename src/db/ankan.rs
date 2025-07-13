// src/db/league_db.rs

use sqlx::{PgPool, Error, postgres::PgQueryResult};
use crate::models::league::{LeaguePlayer, LeagueGame, LeagueResult};

#[derive(Clone)]
pub struct LeagueRepository {
    pool: PgPool,
}

impl LeagueRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // LeaguePlayer CRUD 操作
    pub async fn create_player(&self, player: &LeaguePlayer) -> Result<i32, Error> {
        sqlx::query_scalar!(
        "INSERT INTO meetup_league_player (name) VALUES ($1) RETURNING id",
        player.name
    )
            .fetch_one(&self.pool)
            .await
    }

    // 添加指定ID创建玩家的方法
    pub async fn create_player_with_id(&self, player: &LeaguePlayer) -> Result<i32, Error> {
        // 使用OVERRIDING SYSTEM VALUE插入指定ID
        sqlx::query_scalar!(
            r#"
            INSERT INTO meetup_league_player (id, name)
            OVERRIDING SYSTEM VALUE
            VALUES ($1, $2)
            RETURNING id
            "#,
            player.id,
            player.name
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_player(&self, id: i32) -> Result<LeaguePlayer, Error> {
        sqlx::query_as!(
            LeaguePlayer,
            "SELECT id, name FROM meetup_league_player WHERE id = $1",
            id
        )
            .fetch_one(&self.pool)
            .await
    }

    // 添加通过名称查询玩家的功能
    pub async fn get_player_by_name(&self, name: &str) -> Result<LeaguePlayer, Error> {
        sqlx::query_as!(
            LeaguePlayer,
            "SELECT id, name FROM meetup_league_player WHERE name = $1",
            name
        )
            .fetch_one(&self.pool)
            .await
    }

    pub async fn update_player(&self, player: &LeaguePlayer) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "UPDATE meetup_league_player SET name = $1 WHERE id = $2",
            player.name,
            player.id
        )
            .execute(&self.pool)
            .await
    }

    pub async fn delete_player(&self, id: i32) -> Result<PgQueryResult, Error> {
        sqlx::query!("DELETE FROM meetup_league_player WHERE id = $1", id)
            .execute(&self.pool)
            .await
    }

    pub async fn list_players(&self) -> Result<Vec<LeaguePlayer>, Error> {
        sqlx::query_as!(LeaguePlayer, "SELECT id, name FROM meetup_league_player")
            .fetch_all(&self.pool)
            .await
    }

    // 获取所有玩家
    pub async fn get_all_players(&self) -> Vec<LeaguePlayer> {
        match self.list_players().await {
            Ok(players) => players,
            Err(_) => vec![],
        }
    }

    // 获取指定玩家的所有对战数据（GameInfo）
    pub async fn get_player_matches(&self, name: &str) -> Vec<crate::models::league::GameInfo> {
        // 1. 查询玩家ID
        let player = match self.get_player_by_name(name).await {
            Ok(p) => p,
            Err(_) => return vec![],
        };
        // 2. 查询该玩家参与的所有table_id
        let results = match sqlx::query!(
            "SELECT table_id FROM meetup_league_result WHERE player_id = $1",
            player.id
        ).fetch_all(&self.pool).await {
            Ok(r) => r,
            Err(_) => return vec![],
        };
        let mut games = Vec::new();
        for row in results {
            if let Ok(game) = self.get_game(row.table_id).await {
                // 查询该table所有玩家结果
                let db_results = match sqlx::query!(
                    "SELECT player_id, result, position, uma, penalty, total FROM meetup_league_result WHERE table_id = $1",
                    row.table_id
                ).fetch_all(&self.pool).await {
                    Ok(pr) => pr,
                    Err(_) => vec![],
                };
                // 动态组装 PlayerResult，补 seat 字段
                let mut player_results = Vec::new();
                for r in db_results {
                    let seat = if r.player_id == game.e {
                        "E"
                    } else if r.player_id == game.s {
                        "S"
                    } else if r.player_id == game.w {
                        "W"
                    } else if r.player_id == game.n {
                        "N"
                    } else {
                        "?"
                    };
                    // 查询玩家名字
                    let player_name = match self.get_player(r.player_id).await {
                        Ok(p) => p.name,
                        Err(_) => "未知玩家".to_string(),
                    };
                    player_results.push(crate::models::league::PlayerResult {
                        seat: seat.to_string(),
                        player_name,
                        score: r.result,
                        position: r.position,
                        uma: r.uma,
                        penalty: r.penalty,
                        total: r.total,
                    });
                }
                games.push(crate::models::league::GameInfo {
                    game_id: game.id,
                    played_date: game.game_time.map(|dt| dt.date()).unwrap_or_default(),
                    registered: game.game_time,
                    description: format!("Season {} Table {}", game.season_num, game.table_num),
                    processed: game.processed,
                    player_results,
                    season_num: game.season_num,
                    table_num: game.table_num,
                });
            }
        }
        games
    }

    // LeagueGame CRUD 操作
pub async fn create_game(&self, game: &LeagueGame) -> Result<i32, Error> {
            sqlx::query_scalar!(
            "INSERT INTO meetup_league_table (game_time, season_num, table_num, processed, e, s, w, n)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
            game.game_time,
            game.season_num,
            game.table_num,
            game.processed,
            game.e,
            game.s,
            game.w,
            game.n
        )
                .fetch_one(&self.pool)
                .await
    }

    pub async fn get_game(&self, id: i32) -> Result<LeagueGame, Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT game_time, season_num, table_num, processed, id, e, s, w, n
             FROM meetup_league_table WHERE id = $1",
            id
        )
            .fetch_one(&self.pool)
            .await
    }

    pub async fn update_game(&self, game: &LeagueGame) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "UPDATE meetup_league_table SET
             game_time = $1, season_num = $2, table_num = $3, processed = $4,
             e = $5, s = $6, w = $7, n = $8
             WHERE id = $9",
            game.game_time,
            game.season_num,
            game.table_num,
            game.processed,
            game.e,
            game.s,
            game.w,
            game.n,
            game.id
        )
            .execute(&self.pool)
            .await
    }

    pub async fn delete_game(&self, id: i32) -> Result<PgQueryResult, Error> {
        sqlx::query!("DELETE FROM meetup_league_table WHERE id = $1", id)
            .execute(&self.pool)
            .await
    }

    pub async fn list_games(&self) -> Result<Vec<LeagueGame>, Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT game_time, season_num, table_num, processed, id, e, s, w, n FROM meetup_league_table"
        )
            .fetch_all(&self.pool)
            .await
    }

    pub async fn get_games_by_season(&self, season_num: i32) -> Result<Vec<LeagueGame>, Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT game_time, season_num, table_num, processed, id, e, s, w, n
             FROM meetup_league_table WHERE season_num = $1",
            season_num
        )
            .fetch_all(&self.pool)
            .await
    }

    // LeagueResult CRUD 操作
    pub async fn create_result(&self, result: &LeagueResult) -> Result<PgQueryResult, Error> {
        sqlx::query!(
        "INSERT INTO meetup_league_result
         (table_id, player_id, result, position, uma, penalty, total)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        result.table_id,
        result.player_id,
        result.result,
        result.position,
        result.uma,
        result.penalty,
        result.total
    )
            .execute(&self.pool)
            .await
    }

    pub async fn get_result(&self, id: i32) -> Result<LeagueResult, Error> {
        sqlx::query_as!(
            LeagueResult,
            "SELECT id, table_id, player_id, result, position, uma, penalty, total
             FROM meetup_league_result WHERE id = $1",
            id
        )
            .fetch_one(&self.pool)
            .await
    }

    pub async fn update_result(&self, result: &LeagueResult) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "UPDATE meetup_league_result
             SET table_id = $1, player_id = $2, result = $3,
             position = $4, uma = $5, penalty = $6, total = $7
             WHERE id = $8",
            result.table_id,
            result.player_id,
            result.result,
            result.position,
            result.uma,
            result.penalty,
            result.total,
            result.id
        )
            .execute(&self.pool)
            .await
    }

    pub async fn delete_result(&self, id: i32) -> Result<PgQueryResult, Error> {
        sqlx::query!("DELETE FROM meetup_league_result WHERE id = $1", id)
            .execute(&self.pool)
            .await
    }

    pub async fn list_results(&self) -> Result<Vec<LeagueResult>, Error> {
        sqlx::query_as!(
            LeagueResult,
            "SELECT id, table_id, player_id, result, position, uma, penalty, total FROM meetup_league_result"
        )
            .fetch_all(&self.pool)
            .await
    }

    // 添加通过桌子ID和玩家ID查询结果的方法
    pub async fn get_result_by_table_and_player(&self, table_id: i32, player_id: i32) -> Result<LeagueResult, Error> {
        sqlx::query_as!(
            LeagueResult,
            "SELECT id, table_id, player_id, result, position, uma, penalty, total
             FROM meetup_league_result 
             WHERE table_id = $1 AND player_id = $2",
            table_id,
            player_id
        )
            .fetch_one(&self.pool)
            .await
    }

    pub async fn get_game_by_season_and_table(&self, season_num: i32, table_num: i32) -> Result<LeagueGame, Error> {
        sqlx::query_as!(
        LeagueGame,
        "SELECT game_time, season_num, table_num, processed, id, e, s, w, n
         FROM meetup_league_table
         WHERE season_num = $1 AND table_num = $2",
        season_num,
        table_num
    )
            .fetch_one(&self.pool)
            .await
    }

    // 获取所有赛季编号
    pub async fn get_all_seasons(&self) -> Vec<i32> {
        match sqlx::query!("SELECT DISTINCT season_num FROM meetup_league_table ORDER BY season_num")
            .fetch_all(&self.pool).await {
            Ok(rows) => rows.into_iter().map(|r| r.season_num).collect(),
            Err(_) => vec![],
        }
    }

    // 获取指定赛季的所有玩家（有比赛记录的）
    pub async fn get_players_by_season(&self, season_num: i32) -> Vec<crate::models::league::LeaguePlayer> {
        match sqlx::query!(
            r#"SELECT DISTINCT p.id, p.name FROM meetup_league_player p
                JOIN meetup_league_result r ON p.id = r.player_id
                JOIN meetup_league_table t ON r.table_id = t.id
                WHERE t.season_num = $1"#,
            season_num
        ).fetch_all(&self.pool).await {
            Ok(rows) => rows.into_iter().map(|r| crate::models::league::LeaguePlayer { id: r.id, name: r.name }).collect(),
            Err(_) => vec![],
        }
    }

    // 获取指定赛季指定玩家的所有对战数据
    pub async fn get_player_matches_by_season(&self, name: &str, season_num: i32) -> Vec<crate::models::league::GameInfo> {
        // 1. 查询玩家ID
        let player = match self.get_player_by_name(name).await {
            Ok(p) => p,
            Err(_) => return vec![],
        };
        // 2. 查询该玩家在该赛季参与的所有table_id
        let results = match sqlx::query!(
            r#"SELECT r.table_id FROM meetup_league_result r
                JOIN meetup_league_table t ON r.table_id = t.id
                WHERE r.player_id = $1 AND t.season_num = $2"#,
            player.id, season_num
        ).fetch_all(&self.pool).await {
            Ok(r) => r,
            Err(_) => return vec![],
        };
        let mut games = Vec::new();
        for row in results {
            if let Ok(game) = self.get_game(row.table_id).await {
                // 查询该table所有玩家结果
                let db_results = match sqlx::query!(
                    "SELECT player_id, result, position, uma, penalty, total FROM meetup_league_result WHERE table_id = $1",
                    row.table_id
                ).fetch_all(&self.pool).await {
                    Ok(pr) => pr,
                    Err(_) => vec![],
                };
                // 动态组装 PlayerResult，补 seat 字段
                let mut player_results = Vec::new();
                for r in db_results {
                    let seat = if r.player_id == game.e {
                        "E"
                    } else if r.player_id == game.s {
                        "S"
                    } else if r.player_id == game.w {
                        "W"
                    } else if r.player_id == game.n {
                        "N"
                    } else {
                        "?"
                    };
                    // 查询玩家名字
                    let player_name = match self.get_player(r.player_id).await {
                        Ok(p) => p.name,
                        Err(_) => "Unknown Player".to_string(),
                    };
                    player_results.push(crate::models::league::PlayerResult {
                        seat: seat.to_string(),
                        player_name,
                        score: r.result,
                        position: r.position,
                        uma: r.uma,
                        penalty: r.penalty,
                        total: r.total,
                    });
                }
                games.push(crate::models::league::GameInfo {
                    game_id: game.id,
                    played_date: game.game_time.map(|dt| dt.date()).unwrap_or_default(),
                    registered: game.game_time,
                    description: format!("Season {} Table {}", game.season_num, game.table_num),
                    processed: game.processed,
                    player_results,
                    season_num: game.season_num,
                    table_num: game.table_num,
                });
            }
        }
        games
    }
}