use sqlx::{Row, SqlitePool};

use super::dto::{SaveScoreRequest, ScoreEntry};

pub async fn save_score(pool: &SqlitePool, req: SaveScoreRequest) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO scores (player_id, character_id, score, boss_reached) VALUES (?, ?, ?, ?)",
    )
    .bind(&req.player_id)
    .bind(req.character_id as i64)
    .bind(req.score as i64)
    .bind(req.boss_reached as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_leaderboard(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<ScoreEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, player_id, character_id, score, boss_reached, \
         COALESCE(created_at, '') as created_at \
         FROM scores ORDER BY score DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}

pub async fn get_player_scores(
    pool: &SqlitePool,
    player_id: &str,
) -> Result<Vec<ScoreEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, player_id, character_id, score, boss_reached, \
         COALESCE(created_at, '') as created_at \
         FROM scores WHERE player_id = ? ORDER BY score DESC",
    )
    .bind(player_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}

fn row_to_entry(row: sqlx::sqlite::SqliteRow) -> ScoreEntry {
    ScoreEntry {
        id: row.get("id"),
        player_id: row.get("player_id"),
        character_id: row.get("character_id"),
        score: row.get("score"),
        boss_reached: row.get("boss_reached"),
        created_at: row.get::<String, _>("created_at"),
    }
}
