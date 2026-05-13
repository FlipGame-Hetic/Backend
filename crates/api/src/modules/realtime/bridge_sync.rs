use shared::events::{GameState, OutboundMessage, WsMessage};

use super::hub::BridgeHub;

/// Maps the game-logic state to a `WsMessage::Outbound` and sends it to all bridges.
pub fn sync_game_state_to_bridge(
    game_state: &game_logic::GameState,
    hub: &BridgeHub,
    device_id: &str,
) {
    // Map game_logic::GamePhase → shared::model::GamePhase
    let phase = match game_state.phase {
        game_logic::GamePhase::Idle => shared::model::GamePhase::Idle,
        game_logic::GamePhase::InGame => shared::model::GamePhase::Playing,
        game_logic::GamePhase::GameOver => shared::model::GamePhase::GameOver,
    };

    let payload = GameState {
        state: phase,
        ball_number: (3u8.saturating_sub(game_state.lives)) as u32 + 1,
        score: game_state.score,
        player: 1,
        total_players: 1,
    };

    hub.send(WsMessage::Outbound {
        device_id: device_id.to_owned(),
        payload: OutboundMessage::GameState(payload),
    });
}
