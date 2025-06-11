// src/db/league_db.rs

use sqlx::{PgPool, Error, postgres::PgQueryResult};
use crate::models::league::{LeaguePlayer, LeagueGame, LeagueResult};

pub struct LeagueRepository {
    pool: PgPool,
}

impl LeagueRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // LeaguePlayer CRUD 操作
    pub async fn create_player(&self, player: &LeaguePlayer) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "INSERT INTO league_player (id, name) VALUES ($1, $2)",
            player.id,
            player.name
        )
            .execute(&self.pool)
            .await
    }

    pub async fn get_player(&self, id: i32) -> Result<LeaguePlayer, Error> {
        sqlx::query_as!(
            LeaguePlayer,
            "SELECT id, name FROM league_player WHERE id = $1",
            id
        )
            .fetch_one(&self.pool)
            .await
    }

    pub async fn update_player(&self, player: &LeaguePlayer) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "UPDATE league_player SET name = $1 WHERE id = $2",
            player.name,
            player.id
        )
            .execute(&self.pool)
            .await
    }

    pub async fn delete_player(&self, id: i32) -> Result<PgQueryResult, Error> {
        sqlx::query!("DELETE FROM league_player WHERE id = $1", id)
            .execute(&self.pool)
            .await
    }

    pub async fn list_players(&self) -> Result<Vec<LeaguePlayer>, Error> {
        sqlx::query_as!(LeaguePlayer, "SELECT id, name FROM league_player")
            .fetch_all(&self.pool)
            .await
    }

    // LeagueGame CRUD 操作
    pub async fn create_game(&self, game: &LeagueGame) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "INSERT INTO league_game (game_time, season_num, table_num, processed, id, e, s, w, n)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            game.game_time,
            game.season_num,
            game.table_num,
            game.processed,
            game.id,
            game.e,
            game.s,
            game.w,
            game.n
        )
            .execute(&self.pool)
            .await
    }

    pub async fn get_game(&self, id: i32) -> Result<LeagueGame, Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT game_time, season_num, table_num, processed, id, e, s, w, n
             FROM league_game WHERE id = $1",
            id
        )
            .fetch_one(&self.pool)
            .await
    }

    pub async fn update_game(&self, game: &LeagueGame) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "UPDATE league_game SET
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
        sqlx::query!("DELETE FROM league_game WHERE id = $1", id)
            .execute(&self.pool)
            .await
    }

    pub async fn list_games(&self) -> Result<Vec<LeagueGame>, Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT game_time, season_num, table_num, processed, id, e, s, w, n FROM league_game"
        )
            .fetch_all(&self.pool)
            .await
    }

    pub async fn get_games_by_season(&self, season_num: i32) -> Result<Vec<LeagueGame>, Error> {
        sqlx::query_as!(
            LeagueGame,
            "SELECT game_time, season_num, table_num, processed, id, e, s, w, n
             FROM league_game WHERE season_num = $1",
            season_num
        )
            .fetch_all(&self.pool)
            .await
    }

    // LeagueResult CRUD 操作
    pub async fn create_result(&self, result: &LeagueResult) -> Result<PgQueryResult, Error> {
        sqlx::query!(
            "INSERT INTO meetup_league_result
             (id, table_id, player_id, result, position, uma, penalty, total)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            result.id,
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

    pub async fn get_results_by_table(&self, table_id: i32) -> Result<Vec<LeagueResult>, Error> {
        sqlx::query_as!(
            LeagueResult,
            "SELECT id, table_id, player_id, result, position, uma, penalty, total
             FROM meetup_league_result WHERE table_id = $1",
            table_id
        )
            .fetch_all(&self.pool)
            .await
    }

    pub async fn get_results_by_player(&self, player_id: i32) -> Result<Vec<LeagueResult>, Error> {
        sqlx::query_as!(
            LeagueResult,
            "SELECT id, table_id, player_id, result, position, uma, penalty, total
             FROM meetup_league_result WHERE player_id = $1",
            player_id
        )
            .fetch_all(&self.pool)
            .await
    }
}