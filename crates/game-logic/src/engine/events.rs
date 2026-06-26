//! All events that can flow through the game engine's `process` method.

use shared::model::ButtonId;

use crate::combo::ButtonPress;

/// Which flipper button was pressed (L1 = Left, R1 = Right).
#[derive(Debug, Clone)]
pub enum ButtonSide {
    Left,
    Right,
}

impl ButtonSide {
    /// Map a raw hardware `ButtonId` to a side; returns `None` for non-flipper buttons.
    pub fn from_button_id(id: &ButtonId) -> Option<Self> {
        match id {
            ButtonId::L1 => Some(Self::Left),
            ButtonId::R1 => Some(Self::Right),
            _ => None,
        }
    }
}

/// Why the game ended used in the final `GameOver` screen payload.
#[derive(Debug, Clone)]
pub enum GameOverReason {
    NoLivesLeft,
    PlayerQuit,
}

impl From<ButtonSide> for ButtonPress {
    fn from(side: ButtonSide) -> Self {
        match side {
            ButtonSide::Left => ButtonPress::Left,
            ButtonSide::Right => ButtonPress::Right,
        }
    }
}

/// Every distinct game event the engine can handle.
/// The engine's `process` method pattern-matches on this enum and produces
/// a list of `ScreenEnvelope`s to broadcast to the frontend.
#[derive(Debug, Clone)]
pub enum GameEvent {
    StartGame,
    EndGame,
    BallLaunched,
    BallLost,
    BallSaved,
    ButtonPressed {
        side: ButtonSide,
    },
    BumperHit {
        pts: u32,
        ball_id: Option<String>,
    },
    BumperTriangleHit {
        pts: u32,
        ball_id: Option<String>,
    },
    PortalUsed {
        ball_id: Option<String>,
    },
    BallSaverReady,
    TiltDetected,
    LifeUp,
    MultiballTriggered,
    MultiballWin,
    ScoreMultiplierActivated,
    /// Kept for backward compatibility L2/R2 is now the authoritative trigger.
    UltimateActivated {
        player_id: String,
    },
    ComboActivated(crate::combo::ComboEffect),
    BossDefeated {
        boss_id: u8,
    },
    GameOverTriggered {
        reason: GameOverReason,
    },
    TimerBonusCheck,
    /// Internal tick emitted by the API-layer rail ticker task.
    /// `fib_step` drives the Fibonacci score progression.
    RailTick {
        ball_id: Option<String>,
        fib_step: u32,
    },
}
