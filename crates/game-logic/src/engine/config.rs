use std::sync::{LazyLock, RwLock, RwLockReadGuard};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GameConfig {
    // Core game settings
    pub default_lives: u8,
    pub ultime_charge_ratio: u32,
    pub ball_saver_score: u32,

    // Bumper scoring
    pub bumper_score: u32,
    pub bumper_triangle_score: u32,
    pub portal_score: u32,

    // Multiball
    pub multiball_ring_threshold: u32,
    pub multiball_score: u32,

    // Timer bonus
    pub timer_bonus_seconds: u64,
    pub timer_bonus_score: u32,
    pub timer_bonus_multiplier: f32,

    // Tilt penalties
    pub tilt_penalty_1: i64,
    pub tilt_penalty_2: i64,

    // Boss HP
    pub boss_0_hp: u32,
    pub boss_1_hp: u32,
    pub boss_2_hp: u32,

    // Boss difficulty scaling
    pub boss_0_difficulty_scale: f32,
    pub boss_1_difficulty_scale: f32,
    pub boss_2_difficulty_scale: f32,
    pub endless_base_difficulty_scale: f32,
    pub endless_level_scale_exponent: f32,

    // Combo system
    pub combo_buffer_max: usize,
    pub combo_detection_window_ms: u64,
    pub combo_penalty_repeat: usize,
    pub combo_penalty_pts: i64,

    // Combo bonuses
    pub combo_2_bonus: u32,
    pub combo_3_bonus: u32,
    pub combo_4_bonus: u32,
    pub combo_5_bonus: u32,
    pub combo_6_bonus: u32,
    pub combo_7_bonus: u32,
    pub combo_8_bonus: u32,
    pub combo_9_bonus: u32,
    pub combo_10_bonus: u32,
    pub combo_11_bonus: u32,
    pub combo_14_bonus: u32,
    pub combo_15_bonus: u32,
    pub combo_16_bonus: u32,

    // Enforcer (KEENU) — multiball_split
    pub enforcer_charge_max: u32,
    pub enforcer_weight_bumper: f32,
    pub enforcer_weight_rail: f32,
    pub enforcer_weight_combo: f32,
    pub enforcer_weight_other: f32,

    // Viper (VIPER) — rampage
    pub viper_charge_max: u32,
    pub viper_ulti_duration_ms: u64,
    pub viper_rampage_multiplier: f32,

    // Ghost (GHOST) — mimic
    pub ghost_charge_max: u32,

    // Oracle (ORACLE) — time_slow
    pub oracle_charge_max: u32,
    pub oracle_ulti_duration_ms: u64,
    pub oracle_slow_factor: f32,
    pub oracle_time_rate: f32,

    // Streak multiplier
    pub streak_window_ms: u64,
    pub streak_tier_1_count: u32,
    pub streak_tier_2_count: u32,
    pub streak_tier_3_count: u32,
    pub streak_tier_1_multiplier: f32,
    pub streak_tier_2_multiplier: f32,
    pub streak_tier_3_multiplier: f32,

    // Rail
    pub rail_tick_interval_ms: u64,
    pub rail_max_session_ms: u64,
    pub rail_base_score: u32,
    /// Fibonacci step cap so score per tick stays sane. fib(7) = 21.
    pub rail_max_fib_step: u32,

    // Boss transition timing
    /// Delay after BossDefeated before BossCleared is emitted.
    pub boss_death_anim_ms: u64,
    /// Points the player must score before the first boss appears (and between each boss).
    pub boss_score_threshold: u64,
    /// Interval at which the service layer ticks the PVE engine.
    pub pve_tick_interval_ms: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            default_lives: 3,
            ultime_charge_ratio: 100,
            ball_saver_score: 300,
            bumper_score: 100,
            bumper_triangle_score: 150,
            portal_score: 200,
            multiball_ring_threshold: 10,
            multiball_score: 600,
            timer_bonus_seconds: 60,
            timer_bonus_score: 500,
            timer_bonus_multiplier: 1.5,
            tilt_penalty_1: -2_000,
            tilt_penalty_2: -6_000,
            boss_0_hp: 64_000,
            boss_1_hp: 128_000,
            boss_2_hp: 512_000,
            boss_0_difficulty_scale: 1.0,
            boss_1_difficulty_scale: 1.6,
            boss_2_difficulty_scale: 2.4,
            endless_base_difficulty_scale: 2.4,
            endless_level_scale_exponent: 1.3,
            combo_buffer_max: 10,
            combo_detection_window_ms: 2_000,
            combo_penalty_repeat: 7,
            combo_penalty_pts: 2_000,
            combo_2_bonus: 0,
            combo_3_bonus: 0,
            combo_4_bonus: 1_000,
            combo_5_bonus: 2_000,
            combo_6_bonus: 2_000,
            combo_7_bonus: 2_000,
            combo_8_bonus: 2_000,
            combo_9_bonus: 1_500,
            combo_10_bonus: 1_500,
            combo_11_bonus: 1_550,
            combo_14_bonus: 2_000,
            combo_15_bonus: 2_000,
            combo_16_bonus: 2_000,
            enforcer_charge_max: 80,
            enforcer_weight_bumper: 1.0,
            enforcer_weight_rail: 0.3,
            enforcer_weight_combo: 1.0,
            enforcer_weight_other: 1.0,
            viper_charge_max: 80,
            viper_ulti_duration_ms: 8_000,
            viper_rampage_multiplier: 5.0,
            ghost_charge_max: 60,
            oracle_charge_max: 80,
            oracle_ulti_duration_ms: 5_000,
            oracle_slow_factor: 0.25,
            oracle_time_rate: 1.0,
            streak_window_ms: 2_000,
            streak_tier_1_count: 2,
            streak_tier_2_count: 5,
            streak_tier_3_count: 10,
            streak_tier_1_multiplier: 1.5,
            streak_tier_2_multiplier: 2.0,
            streak_tier_3_multiplier: 3.0,
            rail_tick_interval_ms: 100,
            rail_max_session_ms: 10_000,
            rail_base_score: 4,
            rail_max_fib_step: 7,
            boss_death_anim_ms: 3_000,
            boss_score_threshold: 15_000,
            pve_tick_interval_ms: 250,
        }
    }
}

static CONFIG: LazyLock<RwLock<GameConfig>> = LazyLock::new(|| RwLock::new(GameConfig::default()));

pub fn get() -> RwLockReadGuard<'static, GameConfig> {
    CONFIG.read().expect("game config lock poisoned")
}

pub fn set(cfg: GameConfig) {
    *CONFIG.write().expect("game config lock poisoned") = cfg;
}
