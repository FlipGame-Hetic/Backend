use game_logic::engine::config::GameConfig;
use sqlx::SqlitePool;

pub struct AdminService;

impl AdminService {
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
