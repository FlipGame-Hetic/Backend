use game_logic::engine::config::GameConfig;
use sqlx::SqlitePool;

/// Persistence layer for the live game configuration stored in `game_config` (single row, id=1)
pub struct AdminService;

impl AdminService {
    /// Load the persisted `GameConfig` from the DB
    ///
    /// Returns `None` on first boot (no row yet) or if the stored JSON fails to deserialize
    /// (e.g. after a schema-breaking config change) callers fall back to compiled defaults
    pub async fn load_config(pool: &SqlitePool) -> Option<GameConfig> {
        let row =
            sqlx::query_as::<_, (String,)>("SELECT config_json FROM game_config WHERE id = 1")
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();

        match row {
            Some((json,)) => serde_json::from_str(&json).ok(),
            None => None,
        }
    }

    /// Persist `cfg` using an upsert on the fixed row `id=1`
    ///
    /// The table holds exactly one row; `ON CONFLICT` ensures idempotency
    /// so callers don't need to distinguish first-write from update
    pub async fn save_config(pool: &SqlitePool, cfg: &GameConfig) -> Result<(), sqlx::Error> {
        let json = serde_json::to_string(cfg).expect("GameConfig serialization is infallible");
        sqlx::query(
            "INSERT INTO game_config (id, config_json)
             VALUES (1, ?)
             ON CONFLICT(id) DO UPDATE SET config_json = excluded.config_json, updated_at = datetime('now')",
        )
        .bind(json)
        .execute(pool)
        .await?;
        Ok(())
    }
}
