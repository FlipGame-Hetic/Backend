use axum::body::Body;
use axum::http::{Request, StatusCode};
use reqwest::header::CONTENT_TYPE;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::ServiceExt;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

fn build_app(pool: SqlitePool) -> axum::Router {
    use api::app;
    use api::config::ApiConfig;
    // SAFETY: tests run sequentially via tokio and these vars are only read, not written concurrently.
    unsafe {
        std::env::set_var(
            "SCREEN_JWT_SECRET",
            "flipper-dev-secret-change-in-prod-test",
        );
        std::env::set_var("ALLOWED_ORIGINS", "http://localhost:3000");
        std::env::set_var("API_PORT", "8081");
    }
    let config = ApiConfig::from_env().unwrap();
    app::build(&config, pool)
}

async fn post_json(app: &axum::Router, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get_json(app: &axum::Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

// Test 1: POST /game/start → GET /game/state returns in_game with character slug
#[tokio::test]
async fn start_game_then_state_is_in_game() {
    let app = build_app(test_pool().await);

    let (status, body) = post_json(
        &app,
        "/api/v1/game/start",
        json!({ "character": "enforcer" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["phase"], "in_game");
    assert_eq!(body["character"], "enforcer");

    let (status, body) = get_json(&app, "/api/v1/game/state").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["phase"], "in_game");
    assert_eq!(body["character"], "enforcer");
}

// Test 2: POST /game/start twice → 409 Conflict
#[tokio::test]
async fn double_start_returns_conflict() {
    let app = build_app(test_pool().await);

    let (s1, _) = post_json(&app, "/api/v1/game/start", json!({ "character": "viper" })).await;
    assert_eq!(s1, StatusCode::OK);

    let (s2, body) = post_json(&app, "/api/v1/game/start", json!({ "character": "viper" })).await;
    assert_eq!(s2, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
}

// Test 3: GET /game/state when no game → 404
#[tokio::test]
async fn state_without_game_returns_404() {
    let app = build_app(test_pool().await);
    let (status, _) = get_json(&app, "/api/v1/game/state").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Test 4: POST /game/end saves score and GET /scores returns entry
// ghost → db_id 2, same as the old Hacker=2 slot
#[tokio::test]
async fn end_game_persists_score_to_db() {
    let app = build_app(test_pool().await);

    let (s, _) = post_json(&app, "/api/v1/game/start", json!({ "character": "ghost" })).await;
    assert_eq!(s, StatusCode::OK);

    let (s, body) = post_json(&app, "/api/v1/game/end", json!({})).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["phase"], "game_over");

    let (s, scores) = get_json(&app, "/api/v1/scores").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(scores["scores"].as_array().unwrap().len(), 1);
    assert_eq!(scores["scores"][0]["character_id"], 2); // ghost → db_id 2
}

// Test 5: POST /game/end when no game → 404
#[tokio::test]
async fn end_game_without_active_game_returns_404() {
    let app = build_app(test_pool().await);
    let (status, _) = post_json(&app, "/api/v1/game/end", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Test 6: Manual score save via POST /scores, then GET /scores returns it
#[tokio::test]
async fn manual_save_score_appears_in_leaderboard() {
    let app = build_app(test_pool().await);

    let (s, _) = post_json(
        &app,
        "/api/v1/scores",
        json!({ "character_id": 3, "score": 9999, "boss_reached": 2 }),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    let (s, body) = get_json(&app, "/api/v1/scores").await;
    assert_eq!(s, StatusCode::OK);
    let entries = body["scores"].as_array().unwrap();
    assert_eq!(entries[0]["score"], 9999);
}

// Test 7: Leaderboard is capped at 10 — inserting 15 scores keeps only the top 10
#[tokio::test]
async fn leaderboard_capped_at_10_entries() {
    let app = build_app(test_pool().await);

    for i in 0..15u64 {
        let (s, _) = post_json(
            &app,
            "/api/v1/scores",
            json!({ "character_id": 1, "score": (i + 1) * 1000, "boss_reached": 0 }),
        )
        .await;
        assert!(s == StatusCode::CREATED || s == StatusCode::OK);
    }

    let (s, body) = get_json(&app, "/api/v1/scores").await;
    assert_eq!(s, StatusCode::OK);
    let entries = body["scores"].as_array().unwrap();
    assert_eq!(entries.len(), 10);
    assert_eq!(entries[0]["score"], 15000);
}

// Test 8: Score that does not beat the minimum is rejected (returns 200, not 201)
#[tokio::test]
async fn score_below_top10_minimum_is_rejected() {
    let app = build_app(test_pool().await);

    for i in 1..=10u64 {
        post_json(
            &app,
            "/api/v1/scores",
            json!({ "character_id": 1, "score": i * 1000, "boss_reached": 0 }),
        )
        .await;
    }

    let (s, _) = post_json(
        &app,
        "/api/v1/scores",
        json!({ "character_id": 1, "score": 500, "boss_reached": 0 }),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (_, body) = get_json(&app, "/api/v1/scores").await;
    assert_eq!(body["scores"].as_array().unwrap().len(), 10);
}

// Test 9: GET /characters returns 4 entries with expected slugs
#[tokio::test]
async fn get_characters_returns_roster() {
    let app = build_app(test_pool().await);

    let (status, body) = get_json(&app, "/api/v1/characters").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().expect("should be a JSON array");
    assert_eq!(arr.len(), 4);
    let ids: Vec<&str> = arr.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"enforcer"));
    assert!(ids.contains(&"viper"));
    assert!(ids.contains(&"ghost"));
    assert!(ids.contains(&"oracle"));
}
