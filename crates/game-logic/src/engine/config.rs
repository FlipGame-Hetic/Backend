//Core game settings
pub const DEFAULT_LIVES: u8 = 3;
pub const ULTIME_CHARGE_RATIO: u32 = 100;
pub const BALL_SAVER_SCORE: u32 = 300;

// Bumper scoring
pub const BUMPER_SCORE: u32 = 100;
pub const BUMPER_TRIANGLE_SCORE: u32 = 150;
pub const PORTAL_SCORE: u32 = 200;

// Multiball
pub const MULTIBALL_RING_THRESHOLD: u32 = 10;
pub const MULTIBALL_SCORE: u32 = 600;

// Timer bonus (BonusGameTimerMultiplier)
pub const TIMER_BONUS_SECONDS: u64 = 60;
pub const TIMER_BONUS_SCORE: u32 = 500;
pub const TIMER_BONUS_MULTIPLIER: f32 = 1.5;

// Tilt penalties
pub const TILT_PENALTY_1: i64 = -2_000;
pub const TILT_PENALTY_2: i64 = -6_000;

// Boss HP
pub const BOSS_0_HP: u32 = 42_000;
pub const BOSS_1_HP: u32 = 256_000;
pub const BOSS_2_HP: u32 = 1_024_000;

// Boss difficulty scaling
pub const BOSS_0_DIFFICULTY_SCALE: f32 = 1.0;
pub const BOSS_1_DIFFICULTY_SCALE: f32 = 1.6;
pub const BOSS_2_DIFFICULTY_SCALE: f32 = 2.4;
pub const ENDLESS_BASE_DIFFICULTY_SCALE: f32 = 2.4;
pub const ENDLESS_LEVEL_SCALE_EXPONENT: f32 = 1.3;

// Combo system
pub const COMBO_BUFFER_MAX: usize = 10;
pub const COMBO_DETECTION_WINDOW_MS: u64 = 2_000;
pub const COMBO_PENALTY_REPEAT: usize = 7;
pub const COMBO_PENALTY_PTS: i64 = 2_000;

// Combo stats: bonus_pts only (combos grant points, not a score multiplier)
pub const COMBO_2_BONUS: u32 = 0;
pub const COMBO_3_BONUS: u32 = 0;
pub const COMBO_4_BONUS: u32 = 1_000;
pub const COMBO_5_BONUS: u32 = 2_000;
pub const COMBO_6_BONUS: u32 = 2_000;
pub const COMBO_7_BONUS: u32 = 2_000;
pub const COMBO_8_BONUS: u32 = 2_000;

// Hard 6-button combos
pub const COMBO_9_BONUS: u32 = 1_500;
pub const COMBO_10_BONUS: u32 = 1_500;
pub const COMBO_11_BONUS: u32 = 1_550;

// Very hard 7-button combos
pub const COMBO_14_BONUS: u32 = 2_000;
pub const COMBO_15_BONUS: u32 = 2_000;
pub const COMBO_16_BONUS: u32 = 2_000;

// Character stats
pub const ROBOCP_ULTIMATE_MAX: u32 = 500;
pub const ROBOCP_BONUS_COOLDOWN_MS: u64 = 30_000;
pub const ROBOCP_MALUS_COOLDOWN_MS: u64 = 45_000;

pub const DREDD_ULTIMATE_MAX: u32 = 400;
pub const DREDD_BONUS_COOLDOWN_MS: u64 = 25_000;
pub const DREDD_MALUS_COOLDOWN_MS: u64 = 35_000;

pub const HACKER_ULTIMATE_MAX: u32 = 300;
pub const HACKER_BONUS_COOLDOWN_MS: u64 = 20_000;
pub const HACKER_MALUS_COOLDOWN_MS: u64 = 30_000;

pub const CYBORG_ULTIMATE_MAX: u32 = 450;
pub const CYBORG_BONUS_COOLDOWN_MS: u64 = 28_000;
pub const CYBORG_MALUS_COOLDOWN_MS: u64 = 40_000;

// Streak multiplier (triggers on rapid successive scoring events)
pub const STREAK_WINDOW_MS: u64 = 2_000;
pub const STREAK_TIER_1_COUNT: u32 = 2;
pub const STREAK_TIER_2_COUNT: u32 = 5;
pub const STREAK_TIER_3_COUNT: u32 = 10;
pub const STREAK_TIER_1_MULTIPLIER: f32 = 1.5;
pub const STREAK_TIER_2_MULTIPLIER: f32 = 2.0;
pub const STREAK_TIER_3_MULTIPLIER: f32 = 3.0;

// Rail
pub const RAIL_TICK_INTERVAL_MS: u64 = 100;
pub const RAIL_BASE_SCORE: u32 = 4;
/// Fibonacci step is capped so the score per tick doesn't blow up.
/// fib(10) = 89 → 890 pts/tick at ×1 multiplier, which is a sane ceiling.
pub const RAIL_MAX_FIB_STEP: u32 = 7;

// Boss transition timing
/// Delay after BossDefeated before BossCleared is emitted (death animation window).
pub const BOSS_DEATH_ANIM_MS: u64 = 3_000;
/// Cooldown between BossCleared and the next BossUpdate (score-only phase).
pub const BOSS_COOLDOWN_MS: u64 = 10_000;
/// Interval at which the service layer ticks the PVE engine for cooldown transitions.
pub const PVE_TICK_INTERVAL_MS: u64 = 250;

// Skill effects
pub const SKILL_SHIELD_DURATION_MS: u64 = 8_000;

pub const SKILL_DAMAGE_BOOST_MULTIPLIER: f32 = 2.0;
pub const SKILL_DAMAGE_BOOST_DURATION_MS: u64 = 5_000;

pub const SKILL_COMBO_MULTIPLIER_FACTOR: f32 = 3.0;
pub const SKILL_COMBO_MULTIPLIER_DURATION_MS: u64 = 8_000;

pub const SKILL_EXTRA_FLIPPERS_DURATION_MS: u64 = 6_000;
pub const SKILL_TIME_SLOWDOWN_DURATION_MS: u64 = 5_000;
pub const SKILL_FREEZE_DURATION_MS: u64 = 3_000;
pub const SKILL_PORTAL_BONUS_PTS: u32 = 1_000;
