CREATE TABLE IF NOT EXISTS scores (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id    TEXT    NOT NULL,
    character_id INTEGER NOT NULL,
    score        INTEGER NOT NULL,
    boss_reached INTEGER NOT NULL DEFAULT 0,
    created_at   DATETIME DEFAULT CURRENT_TIMESTAMP
);
