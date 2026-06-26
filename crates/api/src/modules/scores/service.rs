use sqlx::{Row, SqlitePool};

use super::dto::{SaveScoreRequest, ScoreEntry};

// Hard cap on leaderboard size enforced at write time, never exceeded in the DB
const LEADERBOARD_LIMIT: i64 = 10;

/// Attempts to insert `req` into the top-10 leaderboard.
///
/// - If the board has fewer than 10 entries, always inserts.
/// - If the board is full, only inserts when `req.score` is strictly greater than
///   the current minimum; the minimum entry is then deleted atomically.
///
/// Returns `true` when the score was persisted, `false` when it did not qualify.
pub async fn save_score(pool: &SqlitePool, req: SaveScoreRequest) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scores")
        .fetch_one(&mut *tx)
        .await?;

    if count >= LEADERBOARD_LIMIT {
        let min_row = sqlx::query("SELECT id, score FROM scores ORDER BY score ASC LIMIT 1")
            .fetch_one(&mut *tx)
            .await?;

        let min_id: i64 = min_row.get("id");
        let min_score: i64 = min_row.get("score");

        if req.score as i64 <= min_score {
            tx.rollback().await?;
            return Ok(false);
        }

        sqlx::query("DELETE FROM scores WHERE id = ?")
            .bind(min_id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("INSERT INTO scores (character_id, score, boss_reached) VALUES (?, ?, ?)")
        .bind(req.character_id as i64)
        .bind(req.score as i64)
        .bind(req.boss_reached as i64)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(true)
}

/// Fetch the top-`limit` scores sorted by score descending
pub async fn get_leaderboard(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<ScoreEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, character_id, score, boss_reached, created_at \
         FROM scores ORDER BY score DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}

// Explicit field mapping avoids `FromRow` macro magic that breaks silently on column renames
fn row_to_entry(row: sqlx::sqlite::SqliteRow) -> ScoreEntry {
    ScoreEntry {
        id: row.get("id"),
        character_id: row.get("character_id"),
        score: row.get("score"),
        boss_reached: row.get("boss_reached"),
        created_at: row.get::<Option<String>, _>("created_at"),
    }
}
