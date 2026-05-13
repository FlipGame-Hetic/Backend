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
            ButtonId::FlipperLeft => Some(Self::Left),
            ButtonId::FlipperRight => Some(Self::Right),
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
    StartGame { player_id: String },
    EndGame,
    BallLaunched,
    BallLost,
    BallSaved,
    ButtonPressed { side: ButtonSide },
    BumperHit { pts: u32 },
    BumperTriangleHit { pts: u32 },
    BumperCombo { count: u32 },
    PortalUsed,
    TiltDetected,
    LifeUp,
    MultiballWin,
    ScoreMultiplierActivated,
    UltimateActivated { player_id: String },
    ComboActivated(crate::combo::ComboEffect),
    BossDefeated { boss_id: u8 },
    GameOverTriggered { reason: GameOverReason },
    TimerBonusCheck,
}
