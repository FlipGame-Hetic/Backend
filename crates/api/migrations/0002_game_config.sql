CREATE TABLE IF NOT EXISTS game_config (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    config_json TEXT    NOT NULL,
    updated_at  TEXT    DEFAULT (datetime('now'))
);
