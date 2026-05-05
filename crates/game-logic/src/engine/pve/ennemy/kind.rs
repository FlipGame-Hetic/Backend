use crate::engine::config::{BOSS_0_HP, BOSS_1_HP, BOSS_2_HP};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossKind {
    GLaDOS,
    HAL9000,
    AUTO,
}

impl BossKind {
    pub fn id(&self) -> u8 {
        match self {
            Self::GLaDOS => 0,
            Self::HAL9000 => 1,
            Self::AUTO => 2,
        }
    }

    pub fn base_hp(&self) -> u32 {
        match self {
            Self::GLaDOS => BOSS_0_HP,
            Self::HAL9000 => BOSS_1_HP,
            Self::AUTO => BOSS_2_HP,
        }
    }

    pub fn malus_name(&self) -> &'static str {
        match self {
            Self::GLaDOS => "ModifyBounce",
            Self::HAL9000 => "InkBlot",
            Self::AUTO => "BlackHole",
        }
    }

    pub fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::GLaDOS),
            1 => Some(Self::HAL9000),
            2 => Some(Self::AUTO),
            _ => None,
        }
    }
}
