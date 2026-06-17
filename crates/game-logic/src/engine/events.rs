use shared::model::ButtonId;

use crate::combo::ButtonPress;

#[derive(Debug, Clone)]
pub enum ButtonSide {
    Left,
    Right,
}

impl ButtonSide {
    pub fn from_button_id(id: &ButtonId) -> Option<Self> {
        match id {
            ButtonId::L1 => Some(Self::Left),
            ButtonId::R1 => Some(Self::Right),
            _ => None,
        }
    }
}

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

#[derive(Debug, Clone)]
pub enum GameEvent {
    StartGame {
        player_id: String,
    },
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
