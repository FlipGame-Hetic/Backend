use game_logic::GameEngine;
use shared::events::InboundMessage;
use shared::screen::{ScreenEnvelope, ScreenEventType};
use thiserror::Error;
use tracing::warn;

use crate::errors::ApiError;
use crate::modules::realtime::bridge_sync::sync_game_state_to_bridge;
use crate::modules::scores::dto::SaveScoreRequest;
use crate::modules::scores::service as score_service;
use crate::state::{AppState, GameSession};

#[derive(Debug, Error)]
pub enum GameServiceError {
    #[error("a game is already in progress")]
    AlreadyInProgress,
    #[error("no game is in progress")]
    NotInProgress,
    #[error("score persistence failed: {0}")]
    Score(ApiError),
}

impl From<GameServiceError> for ApiError {
    fn from(e: GameServiceError) -> Self {
        match e {
            GameServiceError::AlreadyInProgress => {
                ApiError::Conflict("game_already_in_progress".to_owned())
            }
            GameServiceError::NotInProgress => {
                ApiError::NotFound("no_game_in_progress".to_owned())
            }
            GameServiceError::Score(inner) => inner,
        }
    }
}

pub struct GameService<'a> {
    state: &'a AppState,
}

/// Results extracted from the engine while both mutex guards are held.
///
/// `session_snapshot` is `Some` exclusively when `game_over` is true — it carries
/// the session data needed to persist the final score. If it is `None` on a game-over
/// path, the engine existed without a corresponding session (corrupt state).
struct EngineResult {
    envelopes: Vec<ScreenEnvelope>,
    state_snapshot: game_logic::GameSnapshot,
    session_snapshot: Option<GameSession>,
    game_over: bool,
}

impl<'a> GameService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// Applies `event_fn` to the engine while both locks are held.
    /// Tracks boss defeats, detects game over, clears the engine on game over.
    /// Returns `None` if no game is currently in progress.
    fn tick_engine<F>(
        engine_guard: &mut Option<GameEngine>,
        session_guard: &mut Option<GameSession>,
        event_fn: F,
    ) -> Option<EngineResult>
    where
        F: FnOnce(&mut GameEngine) -> Vec<ScreenEnvelope>,
    {
        let engine = engine_guard.as_mut()?;
        let envelopes = event_fn(engine);

        for env in &envelopes {
            if env.event_type == ScreenEventType::BossDefeated {
                if let Some(session) = session_guard.as_mut() {
                    session.boss_reached = session.boss_reached.saturating_add(1);
                }
            }
        }

        let game_over = envelopes.iter().any(|e| e.event_type == ScreenEventType::GameOver);
        let state_snapshot = engine.take_snapshot();

        let session_snapshot = if game_over {
            *engine_guard = None;
            session_guard.take()
        } else {
            None
        };

        Some(EngineResult {
            envelopes,
            state_snapshot,
            session_snapshot,
            game_over,
        })
    }

    /// Dispatches engine-produced envelopes to screens, syncs to bridge, and persists
    /// the score on game over.
    async fn dispatch_sync_and_save(&self, result: EngineResult) -> Result<(), GameServiceError> {
        let device_id = self.state.active_device_id.read().await.clone();

        for env in result.envelopes {
            let _ = self.state.screen_router.dispatch(env).await;
        }

        if let Some(id) = device_id {
            sync_game_state_to_bridge(&result.state_snapshot.state, &self.state.hub, &id);
        }

        if result.game_over {
            match result.session_snapshot {
                Some(session) => {
                    let req = SaveScoreRequest {
                        player_id: session.player_id,
                        character_id: session.character_id,
                        score: result.state_snapshot.state.score,
                        boss_reached: session.boss_reached,
                    };
                    score_service::save_score(&self.state.db_pool, req)
                        .await
                        .map_err(|e| GameServiceError::Score(e.into()))?;
                }
                None => {
                    warn!("game over fired but session was already cleared — score not persisted");
                }
            }
        }

        Ok(())
    }

    /// Start a new game. Returns the initial game state snapshot.
    /// Fails with `AlreadyInProgress` if a session is already active.
    pub async fn start(
        &self,
        player_id: String,
        character_id: u8,
    ) -> Result<game_logic::GameSnapshot, GameServiceError> {
        // Lock order: engine FIRST, session SECOND [§ 4.4]
        let mut engine_guard = self.state.game_engine.lock().await;
        let mut session_guard = self.state.active_session.lock().await;

        if session_guard.is_some() {
            return Err(GameServiceError::AlreadyInProgress);
        }

        let mut engine = GameEngine::new(character_id);
        let envelopes = engine.process(game_logic::GameEvent::StartGame {
            player_id: player_id.clone(),
        });
        let state_snapshot = engine.take_snapshot();

        *engine_guard = Some(engine);
        *session_guard = Some(GameSession {
            player_id,
            character_id,
            boss_reached: 0,
        });

        // Unlock before any await [§ 4.4]
        drop(session_guard);
        drop(engine_guard);

        let device_id = self.state.active_device_id.read().await.clone();
        if device_id.is_none() {
            warn!("no bridge connected — ESP32 sync skipped");
        }

        for env in envelopes {
            let _ = self.state.screen_router.dispatch(env).await;
        }

        if let Some(id) = device_id {
            sync_game_state_to_bridge(&state_snapshot.state, &self.state.hub, &id);
        }

        Ok(state_snapshot)
    }

    /// Force-end the current game and persist the final score.
    /// Fails with `NotInProgress` if no session is active.
    pub async fn end(&self) -> Result<game_logic::GameSnapshot, GameServiceError> {
        // Lock order: engine FIRST, session SECOND [§ 4.4]
        let mut engine_guard = self.state.game_engine.lock().await;
        let mut session_guard = self.state.active_session.lock().await;

        // The engine is the authoritative "in-progress" signal for end(): it is cleared
        // atomically inside this lock scope, so a concurrent second call to end() will
        // correctly get NotInProgress even while the first call is still awaiting save_score.
        let Some(engine) = engine_guard.as_mut() else {
            return Err(GameServiceError::NotInProgress);
        };

        let envelopes = engine.process(game_logic::GameEvent::EndGame);
        let state_snapshot = engine.take_snapshot();
        let session_snapshot = session_guard.take();

        *engine_guard = None;

        // Unlock before any await [§ 4.4]
        drop(session_guard);
        drop(engine_guard);

        for env in envelopes {
            let _ = self.state.screen_router.dispatch(env).await;
        }

        if let Some(session) = session_snapshot {
            let req = SaveScoreRequest {
                player_id: session.player_id,
                character_id: session.character_id,
                score: state_snapshot.state.score,
                boss_reached: session.boss_reached,
            };
            score_service::save_score(&self.state.db_pool, req)
                .await
                .map_err(|e| GameServiceError::Score(e.into()))?;
        }

        Ok(state_snapshot)
    }

    /// Process an inbound message from the ESP32 bridge.
    /// Silently no-ops if no game is in progress.
    pub async fn process_inbound(
        &self,
        payload: &InboundMessage,
    ) -> Result<(), GameServiceError> {
        // Lock order: engine FIRST, session SECOND [§ 4.4]
        let mut engine_guard = self.state.game_engine.lock().await;
        let mut session_guard = self.state.active_session.lock().await;

        let Some(result) = Self::tick_engine(
            &mut engine_guard,
            &mut session_guard,
            |engine| engine.handle_inbound(payload),
        ) else {
            return Ok(());
        };

        // Unlock before any await [§ 4.4]
        drop(session_guard);
        drop(engine_guard);

        self.dispatch_sync_and_save(result).await
    }

    /// Process an event originating from a screen WebSocket.
    /// Silently no-ops if no game is in progress.
    pub async fn process_screen_event(
        &self,
        envelope: &ScreenEnvelope,
    ) -> Result<(), GameServiceError> {
        // Lock order: engine FIRST, session SECOND [§ 4.4]
        let mut engine_guard = self.state.game_engine.lock().await;
        let mut session_guard = self.state.active_session.lock().await;

        let Some(result) = Self::tick_engine(
            &mut engine_guard,
            &mut session_guard,
            |engine| engine.handle_screen_event(envelope),
        ) else {
            return Ok(());
        };

        // Unlock before any await [§ 4.4]
        drop(session_guard);
        drop(engine_guard);

        self.dispatch_sync_and_save(result).await
    }
}
