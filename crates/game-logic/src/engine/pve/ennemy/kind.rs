use crate::engine::config;

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
        let cfg = config::get();
        match self {
            Self::GLaDOS => cfg.boss_0_hp,
            Self::HAL9000 => cfg.boss_1_hp,
            Self::AUTO => cfg.boss_2_hp,
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
