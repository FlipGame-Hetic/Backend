use std::time::{Duration, Instant};

use shared::events::InboundMessage;
use shared::model::ButtonId;
use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

use crate::combo::{ComboDetector, ComboResult, MultiplierState, StreakState};
use crate::engine::config::{
    DEFAULT_LIVES, ORACLE_SLOW_FACTOR, ORACLE_ULTI_DURATION_MS, PVE_TICK_INTERVAL_MS,
    ULTIME_CHARGE_RATIO, VIPER_RAMPAGE_MULTIPLIER, VIPER_ULTI_DURATION_MS,
};
use crate::engine::events::{ButtonSide, GameEvent, GameOverReason};
use crate::engine::pve::PveEngine;
use crate::engine::scoring::{
    apply_tilt_penalty, rail_tick_score, score_bumper, score_bumper_triangle, timer_bonus,
};
use crate::engine::states::{GamePhase, GameState, TiltEffect};
use crate::player::personnages::character::{Character, UltiShape, select_character};

/// Ghost cycles through these ulti IDs in order (index mod 3).
const GHOST_CYCLE: [&str; 3] = ["multiball_split", "rampage", "time_slow"];

enum ChargeSource {
    Bumper,
    Rail,
    Combo,
    Other,
}

pub struct GameEngine {
    pub state: GameState,
    combo_detector: ComboDetector,
    multiplier: MultiplierState,
    streak: StreakState,
    pve_engine: PveEngine,
    character: Box<dyn Character>,
    timer_bonus_given: bool,
}

impl GameEngine {
    pub fn new(character_slug: &str) -> Self {
        Self {
            state: GameState::new(DEFAULT_LIVES),
            combo_detector: ComboDetector::new(),
            multiplier: MultiplierState::new(),
            streak: StreakState::new(),
            pve_engine: PveEngine::new(),
            character: select_character(character_slug),
            timer_bonus_given: false,
        }
    }

    fn effective_multiplier(&self, now: Instant) -> f32 {
        // During Viper rampage (or Ghost-copied rampage), override multiplier to exactly N.
        // The override ignores streak so rampage × streak stacking doesn't happen.
        if self.state.is_ulti_active(now)
            && let Some(override_mult) = self.state.ulti_multiplier_override
        {
            return override_mult;
        }
        self.multiplier.current(now) * self.streak.current()
    }

    fn emit_multiplier_update(&self, now: Instant) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::MultiplierUpdate,
            serde_json::json!({
                "multiplier": self.effective_multiplier(now),
                "duration_ms": crate::engine::config::STREAK_WINDOW_MS,
            }),
        )
    }

    /// Tick the PVE engine for cooldown/transition progression.
    /// Also advances time-based character charge (Oracle).
    pub fn pve_tick(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        self.tick_time_charge(now);

        let (envelopes, extra) = self.pve_engine.tick(now);
        let mut all = envelopes;
        for e in extra {
            all.extend(self.process(e));
        }
        all
    }

    fn tick_time_charge(&mut self, now: Instant) {
        let time_rate = self.character.stats().charge_profile.time_rate;
        if time_rate <= 0.0
            || self.state.phase != GamePhase::InGame
            || self.state.is_ulti_active(now)
        {
            return;
        }
        let delta_s = PVE_TICK_INTERVAL_MS as f32 / 1000.0;
        self.state.time_charge_buffer += time_rate * delta_s;
        let to_add = self.state.time_charge_buffer.floor() as u32;
        if to_add > 0 {
            self.state.time_charge_buffer -= to_add as f32;
            let charge_max = self.character.stats().charge_profile.charge_max;
            self.state.ultimate_charge = self
                .state
                .ultimate_charge
                .saturating_add(to_add)
                .min(charge_max);
        }
    }

    pub fn take_snapshot(&self) -> crate::GameSnapshot {
        let now = Instant::now();
        let max_hp = self.pve_engine.boss_max_hp();
        let boss_hp_percent = if max_hp > 0 {
            Some(self.pve_engine.boss_hp() as f32 / max_hp as f32)
        } else {
            None
        };
        crate::GameSnapshot {
            state: self.state.clone(),
            current_multiplier: self.effective_multiplier(now),
            boss_hp_percent,
        }
    }

    pub fn handle_inbound(&mut self, msg: &InboundMessage) -> Vec<ScreenEnvelope> {
        match msg {
            InboundMessage::Button(btn) => {
                if let Some(side) = ButtonSide::from_button_id(&btn.id) {
                    let event_type = match &side {
                        ButtonSide::Left => ScreenEventType::FlipperLeft,
                        ButtonSide::Right => ScreenEventType::FlipperRight,
                    };
                    let mut envelopes = vec![make_event_envelope(
                        event_type,
                        serde_json::json!({ "state": btn.state }),
                    )];
                    if btn.state != 0 {
                        envelopes.extend(self.process(GameEvent::ButtonPressed { side }));
                    }
                    return envelopes;
                }

                if self.state.phase == GamePhase::InGame {
                    match btn.id {
                        ButtonId::L2 | ButtonId::R2 if btn.state > 0 => {
                            let now = Instant::now();
                            return self.process_ulti_press(now);
                        }
                        ButtonId::UnderPlunger => {
                            let mut envelopes = vec![make_event_envelope(
                                ScreenEventType::PlungerCharge,
                                serde_json::json!({ "state": btn.state }),
                            )];
                            if btn.state == 0 {
                                envelopes.extend(self.process(GameEvent::BallLaunched));
                            }
                            return envelopes;
                        }
                        _ => {}
                    }
                }

                vec![]
            }
            InboundMessage::Gyro(gyro) if gyro.tilt => self.process(GameEvent::TiltDetected),
            InboundMessage::Plunger(plunger) if plunger.state == 0 => {
                self.process(GameEvent::BallLaunched)
            }
            _ => vec![],
        }
    }

    pub fn handle_screen_event(&mut self, envelope: &ScreenEnvelope) -> Vec<ScreenEnvelope> {
        let event = match &envelope.event_type {
            ScreenEventType::StartGame => GameEvent::StartGame,
            ScreenEventType::EndGame => GameEvent::EndGame,
            ScreenEventType::BallLost => GameEvent::BallLost,
            ScreenEventType::BallSaved => GameEvent::BallSaved,
            ScreenEventType::LifeUp => GameEvent::LifeUp,
            // UltimateActivated is no longer the activation path.
            // L2/R2 is the authoritative trigger. Ignore this event to avoid the old ping-pong.
            ScreenEventType::UltimateActivated => return vec![],
            ScreenEventType::CapacityL2 | ScreenEventType::CapacityR2 => {
                return self.process_ulti_press(Instant::now());
            }
            ScreenEventType::Bumper => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::BumperHit {
                    pts: crate::engine::config::BUMPER_SCORE,
                    ball_id,
                }
            }
            ScreenEventType::BumperTriangle => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::BumperTriangleHit {
                    pts: crate::engine::config::BUMPER_TRIANGLE_SCORE,
                    ball_id,
                }
            }
            ScreenEventType::PortalUsed => {
                let ball_id = envelope
                    .payload
                    .get("ball_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                GameEvent::PortalUsed { ball_id }
            }
            ScreenEventType::FlipperLeft => GameEvent::ButtonPressed {
                side: ButtonSide::Left,
            },
            ScreenEventType::FlipperRight => GameEvent::ButtonPressed {
                side: ButtonSide::Right,
            },
            ScreenEventType::BallSaverReady => GameEvent::BallSaverReady,
            ScreenEventType::MultiballTriggered => GameEvent::MultiballTriggered,
            other => {
                tracing::debug!(event_type = %other, "unhandled screen event type");
                return vec![];
            }
        };
        self.process(event)
    }

    pub fn process(&mut self, event: GameEvent) -> Vec<ScreenEnvelope> {
        let now = Instant::now();
        let mut envelopes = Vec::new();

        // Lazily expire any sustained ulti that has run its full duration.
        self.try_expire_ulti(now);

        match event {
            GameEvent::StartGame => {
                self.state = GameState::new(DEFAULT_LIVES);
                self.state.phase = GamePhase::InGame;
                self.state.session_start = Some(now);
                self.timer_bonus_given = false;
                self.combo_detector = ComboDetector::new();
                self.multiplier = MultiplierState::new();
                self.streak.reset();
                self.pve_engine = PveEngine::new();

                let (pve_env, extra) = self.pve_engine.on_event(&event, &mut self.state);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                envelopes.push(self.emit_score_update(None));
                envelopes.push(self.emit_life_update());
            }

            GameEvent::EndGame => {
                self.state.phase = GamePhase::GameOver;
                envelopes.push(self.emit_game_over());
            }

            GameEvent::BallLaunched => {
                tracing::debug!("ball launched");
            }

            GameEvent::BallLost => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                self.state.balls_lost_since_start += 1;
                self.state.lives = self.state.lives.saturating_sub(1);
                self.streak.reset();

                let (pve_env, extra) = self.pve_engine.on_event(&event, &mut self.state);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }

                if self.state.lives == 0 {
                    envelopes.extend(self.process(GameEvent::GameOverTriggered {
                        reason: GameOverReason::NoLivesLeft,
                    }));
                } else {
                    envelopes.push(self.emit_life_update());
                }
            }

            GameEvent::BallSaved => {
                tracing::debug!("ball saved");
            }

            GameEvent::ButtonPressed { side } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let press = side.into();
                let result = self.combo_detector.push(press, now);

                match result {
                    ComboResult::Activated(effect) => {
                        envelopes.extend(self.process(GameEvent::ComboActivated(effect)));
                    }
                    ComboResult::Penalty { pts } => {
                        if pts < 0 {
                            self.state.score = apply_tilt_penalty(self.state.score, pts);
                        } else {
                            self.state.score = self.state.score.saturating_add(pts as u64);
                        }
                        envelopes.push(self.emit_score_update(None));
                    }
                    ComboResult::BadgeUnlocked { badge_id } => {
                        envelopes.push(make_event_envelope(
                            ScreenEventType::BadgeUnlocked,
                            serde_json::json!({ "badge_id": badge_id }),
                        ));
                    }
                    ComboResult::None => {}
                }
            }

            GameEvent::BumperHit { pts, ref ball_id }
            | GameEvent::BumperTriangleHit { pts, ref ball_id } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let bid = ball_id.clone();
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let current_multiplier = self.effective_multiplier(now);
                let scored = match &event {
                    GameEvent::BumperHit { .. } => score_bumper(current_multiplier),
                    _ => score_bumper_triangle(current_multiplier),
                };
                self.state.add_score(scored);
                // Charge uses base_pts BEFORE multiplier, with per-character bumper weight.
                self.add_charge(pts as u64, ChargeSource::Bumper, now);

                let (pve_env, extra) = self.pve_engine.on_score_delta(scored);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }

                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.extend(self.check_timer_bonus(now));
                envelopes.push(self.emit_scored_delta(scored, "bumper", bid.clone()));
                envelopes.push(self.emit_score_update(bid));
            }

            GameEvent::MultiballTriggered => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                envelopes.extend(self.process(GameEvent::MultiballWin));
            }

            GameEvent::PortalUsed { ref ball_id } => {
                let bid = ball_id.clone();
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let pts = crate::engine::scoring::score_portal_bonus();
                self.state.add_score(pts);
                self.add_charge(
                    crate::engine::config::PORTAL_SCORE as u64,
                    ChargeSource::Other,
                    now,
                );
                let (pve_env, extra) = self.pve_engine.on_score_delta(pts);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_scored_delta(pts, "portal", bid.clone()));
                envelopes.push(self.emit_score_update(bid));
            }

            GameEvent::BallSaverReady => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let pts = crate::engine::config::BALL_SAVER_SCORE as u64;
                self.state.add_score(pts);
                self.add_charge(pts, ChargeSource::Other, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(pts);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_score_delta(pts, "ball_saver"));
                envelopes.push(make_event_envelope(
                    ScreenEventType::BallSaverReady,
                    serde_json::Value::Null,
                ));
                envelopes.push(self.emit_score_update(None));
            }

            GameEvent::TiltDetected => {
                let effect = self.state.tilt_state.on_tilt();
                match effect {
                    TiltEffect::Penalty(pts) => {
                        self.state.score = apply_tilt_penalty(self.state.score, pts);
                        envelopes.push(self.emit_score_update(None));
                        envelopes.push(make_event_envelope(
                            ScreenEventType::TiltPenalty,
                            serde_json::json!({ "penalty": pts }),
                        ));
                    }
                    TiltEffect::CheatingDetected => {
                        self.state.cheating_detected = true;
                        tracing::warn!("cheating detected — score locked");
                        envelopes.push(make_event_envelope(
                            ScreenEventType::CheatingDetected,
                            serde_json::Value::Null,
                        ));
                    }
                }
            }

            GameEvent::LifeUp => {
                self.state.lives += 1;
                envelopes.push(self.emit_life_update());
            }

            GameEvent::MultiballWin => {
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let pts = crate::engine::config::MULTIBALL_SCORE as u64;
                self.state.add_score(pts);
                self.add_charge(pts, ChargeSource::Other, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(pts);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_score_delta(pts, "multiball"));
                envelopes.push(make_event_envelope(
                    ScreenEventType::MultiballWin,
                    serde_json::Value::Null,
                ));
                envelopes.push(self.emit_score_update(None));
            }

            GameEvent::ScoreMultiplierActivated => {
                envelopes.push(self.emit_multiplier_update(now));
            }

            // Kept in GameEvent for backward compatibility but no longer the activation path.
            GameEvent::UltimateActivated { .. } => {}

            GameEvent::ComboActivated(effect) => {
                let (_streak_changed, streak_armed) = self.streak.record(now);
                let current_multiplier = self.effective_multiplier(now);
                let scaled_bonus = (effect.bonus_pts as f32 * current_multiplier) as u64;
                self.state.add_score(scaled_bonus);
                // Charge on combo uses base bonus_pts (before multiplier).
                if effect.bonus_pts > 0 {
                    self.add_charge(effect.bonus_pts as u64, ChargeSource::Combo, now);
                    let (pve_env, extra) = self.pve_engine.on_score_delta(scaled_bonus);
                    envelopes.extend(pve_env);
                    for e in extra {
                        envelopes.extend(self.process(e));
                    }
                }
                envelopes.push(self.emit_combo_activated(&effect));
                if streak_armed {
                    envelopes.push(self.emit_multiplier_update(now));
                }
                envelopes.push(self.emit_score_delta(scaled_bonus, "combo"));
                envelopes.push(self.emit_score_update(None));
            }

            GameEvent::BossDefeated { boss_id } => {
                tracing::info!(boss_id, "boss defeated event processed");
                envelopes.push(make_event_envelope(
                    ScreenEventType::BossDefeated,
                    serde_json::json!({ "boss_id": boss_id }),
                ));
            }

            GameEvent::GameOverTriggered { reason } => {
                self.state.phase = GamePhase::GameOver;
                tracing::info!(?reason, "game over triggered");
                envelopes.push(self.emit_game_over());
            }

            GameEvent::TimerBonusCheck => {
                envelopes.extend(self.check_timer_bonus(now));
            }

            GameEvent::RailTick { ball_id, fib_step } => {
                if self.state.phase != GamePhase::InGame {
                    return envelopes;
                }
                let bid = ball_id.clone();
                let current_multiplier = self.multiplier.current(now);
                let scored = rail_tick_score(fib_step, current_multiplier);
                self.state.add_score(scored);
                // Charge uses base rail pts (multiplier = 1.0).
                let base_pts = rail_tick_score(fib_step, 1.0);
                self.add_charge(base_pts, ChargeSource::Rail, now);
                let (pve_env, extra) = self.pve_engine.on_score_delta(scored);
                envelopes.extend(pve_env);
                for e in extra {
                    envelopes.extend(self.process(e));
                }
                envelopes.push(self.emit_scored_delta(scored, "rail", ball_id));
                envelopes.push(self.emit_score_update(bid));
            }
        }

        envelopes
    }

    // ── Ulti state machine ────────────────────────────────────────────────────

    /// Lazily expire a sustained ulti that has reached its natural end.
    fn try_expire_ulti(&mut self, now: Instant) {
        if let Some(ends_at) = self.state.ulti_ends_at
            && now >= ends_at
            && self.state.ulti_active_id.is_some()
        {
            self.state.ulti_ends_at = None;
            self.state.ultimate_charge = 0;
            self.state.ulti_active_id = None;
            self.state.ulti_multiplier_override = None;
        }
    }

    fn process_ulti_press(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        let mut envelopes = Vec::new();

        if self.state.is_ulti_active(now) {
            if self.state.ulti_cancellable {
                let charge_max = self.character.stats().charge_profile.charge_max;
                let residual = self.state.residual_charge_with_max(now, charge_max);
                let ulti_id = self.state.ulti_active_id.clone().unwrap_or_default();

                // Expire immediately
                self.state.ulti_ends_at = Some(now);
                self.state.ultimate_charge = residual;
                self.state.ulti_active_id = None;
                self.state.ulti_multiplier_override = None;

                envelopes.push(make_event_envelope(
                    ScreenEventType::UltimateStopped,
                    serde_json::json!({
                        "ulti_id": ulti_id,
                        "ultimate_charge": residual,
                    }),
                ));
                envelopes.push(self.emit_score_update(None));
            }
            // Non-cancellable: silently ignore the press.
        } else {
            let charge_max = self.character.stats().charge_profile.charge_max;
            if self.state.ultimate_charge >= charge_max {
                envelopes.extend(self.activate_ulti(now));
                envelopes.push(self.emit_score_update(None));
            }
            // Not ready: no event (optional UltimateNotReady could go here).
        }

        envelopes
    }

    fn activate_ulti(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        let mut envelopes = Vec::new();
        let character_slug = self.character.slug();

        // Resolve ulti identity and shape.
        let (ulti_id, shape): (&'static str, UltiShape) = if character_slug == "ghost" {
            let idx = (self.state.ghost_cycle_index as usize) % GHOST_CYCLE.len();
            self.state.ghost_cycle_index = self.state.ghost_cycle_index.wrapping_add(1);
            match idx {
                0 => ("multiball_split", UltiShape::Instant),
                1 => (
                    "rampage",
                    UltiShape::Sustained {
                        duration_ms: VIPER_ULTI_DURATION_MS,
                        cancellable: false,
                    },
                ),
                _ => (
                    "time_slow",
                    UltiShape::Sustained {
                        duration_ms: ORACLE_ULTI_DURATION_MS,
                        cancellable: true,
                    },
                ),
            }
        } else {
            (self.character.ulti_id(), self.character.ulti_shape())
        };

        // Apply mechanics.
        match &shape {
            UltiShape::Instant => {
                self.state.ultimate_charge = 0;
                if ulti_id == "multiball_split" {
                    self.state.multiball_active = true;
                }
            }
            UltiShape::Sustained {
                duration_ms,
                cancellable,
            } => {
                self.state.ulti_ends_at = Some(now + Duration::from_millis(*duration_ms));
                self.state.ulti_duration_ms = *duration_ms;
                self.state.ulti_cancellable = *cancellable;
                self.state.ulti_active_id = Some(ulti_id.to_string());
                // Charge stays at charge_max and drains lazily.

                if ulti_id == "rampage" {
                    self.state.ulti_multiplier_override = Some(VIPER_RAMPAGE_MULTIPLIER);
                    envelopes.push(make_event_envelope(
                        ScreenEventType::MultiplierUpdate,
                        serde_json::json!({
                            "multiplier": VIPER_RAMPAGE_MULTIPLIER,
                            "duration_ms": duration_ms,
                        }),
                    ));
                }
            }
            UltiShape::Inherited => {
                unreachable!("ghost always resolves to a concrete shape before this point");
            }
        }

        // Build UltimateTriggered event.
        let mut triggered = serde_json::json!({
            "character": character_slug,
            "ulti_id": ulti_id,
        });
        match &shape {
            UltiShape::Instant => {
                triggered["shape"] = serde_json::json!("instant");
                triggered["cancellable"] = serde_json::json!(false);
            }
            UltiShape::Sustained {
                duration_ms,
                cancellable,
            } => {
                triggered["shape"] = serde_json::json!("sustained");
                triggered["cancellable"] = serde_json::json!(cancellable);
                triggered["duration_ms"] = serde_json::json!(duration_ms);
                let payload = match ulti_id {
                    "rampage" => serde_json::json!({ "multiplier": VIPER_RAMPAGE_MULTIPLIER }),
                    "time_slow" => serde_json::json!({ "slow_factor": ORACLE_SLOW_FACTOR }),
                    _ => serde_json::json!({}),
                };
                triggered["payload"] = payload;
            }
            UltiShape::Inherited => unreachable!(),
        }
        envelopes.push(make_event_envelope(
            ScreenEventType::UltimateTriggered,
            triggered,
        ));

        envelopes
    }

    // ── Charge accumulation ───────────────────────────────────────────────────

    /// Accumulate ultimate charge from a scoring event.
    /// `base_pts` is the score value BEFORE any multiplier is applied.
    /// Charge gain is suspended while a sustained ulti is active.
    fn add_charge(&mut self, base_pts: u64, source: ChargeSource, now: Instant) {
        if self.state.is_ulti_active(now) {
            return;
        }
        let profile = &self.character.stats().charge_profile;
        let weight = match source {
            ChargeSource::Bumper => profile.weight_bumper,
            ChargeSource::Rail => profile.weight_rail,
            ChargeSource::Combo => profile.weight_combo,
            ChargeSource::Other => profile.weight_other,
        };
        let weighted = (base_pts as f32 * weight).round() as u32;
        self.state.point_buffer = self.state.point_buffer.saturating_add(weighted);
        let gain = self.state.point_buffer / ULTIME_CHARGE_RATIO;
        self.state.point_buffer %= ULTIME_CHARGE_RATIO;
        let charge_max = profile.charge_max;
        self.state.ultimate_charge = self
            .state
            .ultimate_charge
            .saturating_add(gain)
            .min(charge_max);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn check_timer_bonus(&mut self, now: Instant) -> Vec<ScreenEnvelope> {
        if self.timer_bonus_given {
            return vec![];
        }
        let Some(start) = self.state.session_start else {
            return vec![];
        };
        let elapsed = now.duration_since(start).as_secs();
        if elapsed >= crate::engine::config::TIMER_BONUS_SECONDS
            && self.state.balls_lost_since_start == 0
        {
            self.timer_bonus_given = true;
            let old_score = self.state.score;
            self.state.score = timer_bonus(self.state.score, 0);
            let delta = self.state.score.saturating_sub(old_score);
            return vec![
                make_event_envelope(
                    ScreenEventType::TimerBonus,
                    serde_json::json!({ "new_score": self.state.score }),
                ),
                self.emit_score_delta(delta, "timer_bonus"),
                self.emit_score_update(None),
            ];
        }
        vec![]
    }

    fn emit_score_update(&self, ball_id: Option<String>) -> ScreenEnvelope {
        let now = Instant::now();
        let current_multiplier = self.effective_multiplier(now);
        let ball = self.state.balls_lost_since_start + 1;
        let charge_max = self.character.stats().charge_profile.charge_max;

        // During a sustained ulti the charge bar shows the live drain residual.
        let displayed_charge = if self.state.is_ulti_active(now) {
            self.state.residual_charge_with_max(now, charge_max)
        } else {
            self.state.ultimate_charge
        };
        let ulti_ready =
            !self.state.is_ulti_active(now) && self.state.ultimate_charge >= charge_max;

        let mut payload = serde_json::json!({
            "score": self.state.score,
            "multiplier": current_multiplier,
            "ball": ball,
            "ultimate_charge": displayed_charge,
            "ultimate_max": charge_max,
            "ulti_ready": ulti_ready,
        });

        if self.character.slug() == "ghost" {
            let next_idx = (self.state.ghost_cycle_index as usize) % GHOST_CYCLE.len();
            payload["next_ulti_id"] = serde_json::json!(GHOST_CYCLE[next_idx]);
        }

        if let Some(bid) = ball_id {
            payload["ball_id"] = serde_json::json!(bid);
        }
        make_event_envelope(ScreenEventType::ScoreUpdate, payload)
    }

    fn emit_score_delta(&self, delta: u64, reason: &str) -> ScreenEnvelope {
        self.emit_scored_delta(delta, reason, None)
    }

    fn emit_scored_delta(
        &self,
        delta: u64,
        reason: &str,
        ball_id: Option<String>,
    ) -> ScreenEnvelope {
        let mut payload = serde_json::json!({
            "delta": delta,
            "reason": reason,
            "total": self.state.score,
        });
        if let Some(bid) = ball_id {
            payload["ball_id"] = serde_json::json!(bid);
        }
        make_event_envelope(ScreenEventType::ScoreDelta, payload)
    }

    fn emit_life_update(&self) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::LifeUpdate,
            serde_json::json!({ "lives_remaining": self.state.lives }),
        )
    }

    fn emit_combo_activated(&self, effect: &crate::combo::ComboEffect) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::ComboActivated,
            serde_json::json!({
                "bonus_pts": effect.bonus_pts,
                "sequence": effect.sequence,
            }),
        )
    }

    fn emit_game_over(&self) -> ScreenEnvelope {
        make_event_envelope(
            ScreenEventType::GameOver,
            serde_json::json!({ "final_score": self.state.score }),
        )
    }
}

fn make_event_envelope(event_type: ScreenEventType, payload: serde_json::Value) -> ScreenEnvelope {
    ScreenEnvelope {
        from: ScreenId::GameEngine,
        to: ScreenTarget::Broadcast,
        event_type,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::events::{GameEvent, GameOverReason};

    fn started_engine() -> GameEngine {
        let mut engine = GameEngine::new("enforcer");
        engine.process(GameEvent::StartGame);
        engine
    }

    #[test]
    fn rail_tick_increases_score() {
        let mut engine = started_engine();
        let before = engine.state.score;
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert!(
            engine.state.score > before,
            "score should increase after RailTick"
        );
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::ScoreDelta),
            "should emit ScoreDelta"
        );
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::ScoreUpdate),
            "should emit ScoreUpdate"
        );
    }

    #[test]
    fn rail_tick_fibonacci_progression() {
        let delta_at = |fib_step: u32| {
            let mut engine = started_engine();
            let before = engine.state.score;
            engine.process(GameEvent::RailTick {
                ball_id: None,
                fib_step,
            });
            engine.state.score - before
        };

        let d0 = delta_at(0);
        let d1 = delta_at(1);
        let d2 = delta_at(2);

        assert_eq!(
            d0, d1,
            "fib(0)==fib(1) so step-0 and step-1 deltas should be equal"
        );
        assert!(
            d2 > d1,
            "step-2 delta should be larger than step-1 (fib grows)"
        );
    }

    #[test]
    fn rail_tick_includes_ball_id_in_delta() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: Some("ball-uuid-2".to_string()),
            fib_step: 0,
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert_eq!(
            delta_env.payload["ball_id"],
            serde_json::json!("ball-uuid-2")
        );
    }

    #[test]
    fn rail_tick_no_ball_id_omits_field() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert!(
            delta_env.payload.get("ball_id").is_none(),
            "ball_id should be absent when None"
        );
    }

    #[test]
    fn rail_tick_ignored_when_not_in_game() {
        let mut engine = GameEngine::new("enforcer");
        let before = engine.state.score;
        engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert_eq!(
            engine.state.score, before,
            "tick outside InGame must not change score"
        );
    }

    #[test]
    fn rail_tick_ignored_when_cheating_detected() {
        let mut engine = started_engine();
        engine.state.cheating_detected = true;
        let before = engine.state.score;
        engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert_eq!(
            engine.state.score, before,
            "score must be locked when cheating detected"
        );
    }

    #[test]
    fn multiball_two_balls_score_independently() {
        let delta_for = |ball_id: &str| {
            let mut engine = started_engine();
            let before = engine.state.score;
            engine.process(GameEvent::RailTick {
                ball_id: Some(ball_id.to_string()),
                fib_step: 3,
            });
            engine.state.score - before
        };

        let d1 = delta_for("ball-uuid-1");
        let d2 = delta_for("ball-uuid-2");
        assert!(d1 > 0);
        assert_eq!(d1, d2, "same fib_step → same delta regardless of ball_id");
    }

    #[test]
    fn game_over_ignored_rail_tick() {
        let mut engine = started_engine();
        engine.process(GameEvent::GameOverTriggered {
            reason: GameOverReason::NoLivesLeft,
        });
        let before = engine.state.score;
        engine.process(GameEvent::RailTick {
            ball_id: None,
            fib_step: 0,
        });
        assert_eq!(
            engine.state.score, before,
            "tick after GameOver must not change score"
        );
    }

    #[test]
    fn rail_tick_includes_ball_id_in_score_update() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::RailTick {
            ball_id: Some("ball-uuid-2".to_string()),
            fib_step: 0,
        });
        let update_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert_eq!(
            update_env.payload["ball_id"],
            serde_json::json!("ball-uuid-2")
        );
    }

    #[test]
    fn bumper_hit_includes_ball_id_in_score_events() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::BumperHit {
            pts: 100,
            ball_id: Some("ball-abc".to_string()),
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert_eq!(delta_env.payload["ball_id"], serde_json::json!("ball-abc"));

        let update_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert_eq!(update_env.payload["ball_id"], serde_json::json!("ball-abc"));
    }

    #[test]
    fn bumper_hit_no_ball_id_omits_field() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::BumperHit {
            pts: 100,
            ball_id: None,
        });
        let delta_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreDelta)
            .expect("ScoreDelta should be emitted");
        assert!(delta_env.payload.get("ball_id").is_none());

        let update_env = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert!(update_env.payload.get("ball_id").is_none());
    }

    #[test]
    fn score_update_includes_charge_fields() {
        let mut engine = started_engine();
        let envelopes = engine.process(GameEvent::BumperHit {
            pts: 100,
            ball_id: None,
        });
        let update = envelopes
            .iter()
            .find(|e| e.event_type == ScreenEventType::ScoreUpdate)
            .expect("ScoreUpdate should be emitted");
        assert!(update.payload.get("ultimate_charge").is_some());
        assert!(update.payload.get("ultimate_max").is_some());
        assert!(update.payload.get("ulti_ready").is_some());
    }

    #[test]
    fn bumper_charge_accumulates_with_buffer() {
        let mut engine = started_engine();
        // Enforcer bumper weight = 1.0; ULTIME_CHARGE_RATIO = 100.
        // 1 bumper hit = 100 pts → 100 / 100 = 1 charge unit.
        for _ in 0..5 {
            engine.process(GameEvent::BumperHit {
                pts: 100,
                ball_id: None,
            });
        }
        assert!(
            engine.state.ultimate_charge >= 5,
            "charge should accumulate"
        );
    }

    #[test]
    fn viper_ulti_triggers_when_full() {
        let mut engine = GameEngine::new("viper");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;

        // Simulate L2/R2 press by calling process_ulti_press directly.
        let now = Instant::now();
        let envelopes = engine.process_ulti_press(now);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateTriggered),
            "should emit UltimateTriggered"
        );
        assert!(
            engine.state.is_ulti_active(Instant::now()),
            "ulti should be active"
        );
        assert_eq!(engine.state.ulti_multiplier_override, Some(5.0));
    }

    #[test]
    fn oracle_ulti_is_cancellable() {
        let mut engine = GameEngine::new("oracle");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;
        engine.state.ultimate_charge = charge_max;

        let now = Instant::now();
        engine.process_ulti_press(now);
        assert!(engine.state.is_ulti_active(Instant::now()));

        let now2 = Instant::now();
        let envelopes = engine.process_ulti_press(now2);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::UltimateStopped),
            "should emit UltimateStopped on cancel"
        );
        assert!(
            !engine.state.is_ulti_active(Instant::now()),
            "ulti should be cancelled"
        );
    }

    #[test]
    fn ghost_cycle_advances_on_each_activation() {
        let mut engine = GameEngine::new("ghost");
        engine.process(GameEvent::StartGame);
        let charge_max = engine.character.stats().charge_profile.charge_max;

        for expected_ulti in &["multiball_split", "rampage", "time_slow"] {
            engine.state.ultimate_charge = charge_max;
            let now = Instant::now();
            let envelopes = engine.process_ulti_press(now);
            let triggered = envelopes
                .iter()
                .find(|e| e.event_type == ScreenEventType::UltimateTriggered)
                .expect("should emit UltimateTriggered");
            assert_eq!(
                triggered.payload["ulti_id"],
                serde_json::json!(expected_ulti)
            );

            // End any active sustained ulti before next loop.
            engine.state.ulti_ends_at = Some(Instant::now());
            engine.state.ulti_active_id = None;
            engine.state.ulti_multiplier_override = None;
        }
    }
}
