pub struct CharacterChargeProfile {
    pub charge_max: u32,
    pub weight_bumper: f32,
    pub weight_rail: f32,
    pub weight_combo: f32,
    pub weight_other: f32,
    /// Charge units per second added by the time component (0 = none).
    pub time_rate: f32,
}

pub struct CharacterStats {
    pub charge_profile: CharacterChargeProfile,
    pub bonus_cooldown_ms: u64,
    pub malus_cooldown_ms: u64,
}
