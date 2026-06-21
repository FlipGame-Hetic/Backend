use std::time::Instant;

use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

use crate::engine::config::{BOSS_0_HP, BOSS_DEATH_ANIM_MS, BOSS_SCORE_THRESHOLD};
use crate::engine::events::{GameEvent, GameOverReason};
use crate::engine::pve::difficulty::{boss_damage_to_health, scale_hp};
use crate::engine::pve::ennemy::boss::Boss;
use crate::engine::pve::ennemy::kind::BossKind;
use crate::engine::pve::states::{CooldownState, PvePhase, PveState};
use crate::engine::states::GameState;

pub struct PveEngine {
    state: PveState,
    boss: Boss,
}

impl PveEngine {
    pub fn new() -> Self {
        let kind = BossKind::from_index(0).unwrap_or(BossKind::GLaDOS);
        let boss = Boss::new(kind, 0);
        let initial_hp = boss.health.max;
        Self {
            state: PveState::new(initial_hp),
            boss,
        }
    }

    /// Handles game-lifecycle events (StartGame, BallLost).
    /// Scoring events are handled via `on_score_delta` instead.
    pub fn on_event(
        &mut self,
        event: &GameEvent,
        game_state: &mut GameState,
    ) -> (Vec<ScreenEnvelope>, Vec<GameEvent>) {
        let envelopes = Vec::new();
        let mut extra_events = Vec::new();

        match event {
            GameEvent::StartGame => {
                // Boss spawns only once BOSS_SCORE_THRESHOLD points are scored.
                self.state.phase = PvePhase::WaitingForScore;
                self.state.next_boss_index = 0;
                self.state.score_accumulated = 0;
                self.state.cooldown = None;
                self.state.current_boss_index = 0;
                self.state.endless_level = 0;
            }

            GameEvent::BallLost if game_state.lives == 0 => {
                self.state.phase = PvePhase::GameOver;
                extra_events.push(GameEvent::GameOverTriggered {
                    reason: GameOverReason::NoLivesLeft,
                });
            }

            _ => {}
        }

        (envelopes, extra_events)
    }

    /// Apply a positive score delta.
    ///
    /// - `WaitingForScore`: accumulates toward the spawn threshold; spawns the next boss
    ///   once BOSS_SCORE_THRESHOLD points have been scored since the last boss event.
    /// - `Fighting`: applies damage to the current boss.
    /// - All other phases: no-op.
    pub fn on_score_delta(&mut self, delta: u64) -> (Vec<ScreenEnvelope>, Vec<GameEvent>) {
        let mut envelopes = Vec::new();
        let mut extra_events = Vec::new();

        if delta == 0 {
            return (envelopes, extra_events);
        }

        match self.state.phase {
            PvePhase::WaitingForScore => {
                self.state.score_accumulated = self.state.score_accumulated.saturating_add(delta);
                if self.state.score_accumulated >= BOSS_SCORE_THRESHOLD {
                    self.state.score_accumulated = 0;
                    let next = self.state.next_boss_index;
                    self.spawn_next(next, &mut envelopes);
                }
            }

            PvePhase::Fighting => {
                let damage = boss_damage_to_health(
                    delta.min(u32::MAX as u64) as u32,
                    self.state.current_boss_index,
                );
                let died = self.boss.take_hit(damage);
                self.state.boss_health.current = self.boss.health.current;

                envelopes.push(make_boss_update(
                    self.boss.kind.id(),
                    self.boss.health.current,
                    self.boss.health.max,
                ));

                if died {
                    let boss_id = self.boss.kind.id();
                    tracing::info!(boss_id, "boss defeated");
                    extra_events.push(GameEvent::BossDefeated { boss_id });
                    self.enter_cooldown();
                }
            }

            _ => {}
        }

        (envelopes, extra_events)
    }

    /// Advance the death-animation cooldown.
    ///
    /// Call this periodically from the service layer. Once BOSS_DEATH_ANIM_MS have
    /// elapsed, emits BossCleared and transitions to WaitingForScore so the next boss
    /// spawns after BOSS_SCORE_THRESHOLD new points are scored.
    pub fn tick(&mut self, now: Instant) -> (Vec<ScreenEnvelope>, Vec<GameEvent>) {
        let mut envelopes = Vec::new();
        let extra_events = Vec::new();

        if self.state.phase != PvePhase::Cooldown {
            return (envelopes, extra_events);
        }

        let cooldown = match self.state.cooldown.as_ref() {
            Some(c) => c,
            None => return (envelopes, extra_events),
        };

        let elapsed_ms = now.duration_since(cooldown.defeated_at).as_millis() as u64;
        if elapsed_ms >= BOSS_DEATH_ANIM_MS {
            let boss_id = self.boss.kind.id();
            let next_index = cooldown.next_boss_index;
            self.state.cooldown = None;
            self.state.next_boss_index = next_index;
            self.state.score_accumulated = 0;
            self.state.phase = PvePhase::WaitingForScore;
            envelopes.push(make_event_envelope(
                ScreenEventType::BossCleared,
                serde_json::json!({ "boss_id": boss_id }),
            ));
        }

        (envelopes, extra_events)
    }

    fn enter_cooldown(&mut self) {
        let next_index = self.state.current_boss_index.saturating_add(1);
        self.state.phase = PvePhase::Cooldown;
        self.state.cooldown = Some(CooldownState {
            next_boss_index: next_index,
            defeated_at: Instant::now(),
        });
    }

    fn spawn_next(&mut self, next_index: u8, envelopes: &mut Vec<ScreenEnvelope>) {
        if BossKind::from_index(next_index).is_some() {
            self.reset_to_boss(next_index, envelopes);
        } else {
            // All story bosses defeated
            self.state.endless_level += 1;
            if self.state.endless_level == 1 {
                self.state.phase = PvePhase::Victory;
                envelopes.push(make_event_envelope(
                    ScreenEventType::VictoireFinale,
                    serde_json::Value::Null,
                ));
            } else {
                // Endless: respawn AUTO with scaled HP
                let level = self.state.endless_level;
                let hp = scale_hp(BOSS_0_HP, 3, level);
                self.boss = Boss::new_endless(BossKind::AUTO, level);
                self.state.boss_health.reset_with_new_max(hp);
                self.state.phase = PvePhase::Fighting;
                envelopes.push(make_boss_update(
                    self.boss.kind.id(),
                    self.boss.health.current,
                    self.boss.health.max,
                ));
                envelopes.push(make_event_envelope(
                    ScreenEventType::EndlessScaling,
                    serde_json::json!({ "level": level }),
                ));
            }
        }
    }

    fn reset_to_boss(&mut self, index: u8, envelopes: &mut Vec<ScreenEnvelope>) {
        let kind = BossKind::from_index(index).unwrap_or(BossKind::GLaDOS);
        self.boss = Boss::new(kind, index);
        self.state.current_boss_index = index;
        self.state
            .boss_health
            .reset_with_new_max(self.boss.health.max);
        self.state.phase = PvePhase::Fighting;
        self.state.cooldown = None;

        envelopes.push(make_boss_update(
            kind.id(),
            self.boss.health.current,
            self.boss.health.max,
        ));
    }

    pub fn current_boss_index(&self) -> u8 {
        self.state.current_boss_index
    }

    pub fn boss_hp(&self) -> u32 {
        self.boss.health.current
    }

    pub fn boss_max_hp(&self) -> u32 {
        self.boss.health.max
    }

    pub fn boss_id(&self) -> u8 {
        self.boss.kind.id()
    }

    pub fn phase(&self) -> &PvePhase {
        &self.state.phase
    }
}

impl Default for PveEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn make_boss_update(boss_id: u8, hp: u32, max_hp: u32) -> ScreenEnvelope {
    make_event_envelope(
        ScreenEventType::BossUpdate,
        serde_json::json!({ "boss_id": boss_id, "boss_hp": hp, "boss_max_hp": max_hp }),
    )
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
    use crate::engine::config::{BOSS_SCORE_THRESHOLD, DEFAULT_LIVES};
    use crate::engine::states::GameState;
    use std::time::Duration;

    fn make_state() -> GameState {
        GameState::new(DEFAULT_LIVES)
    }

    /// Returns a PveEngine with the first boss already active (threshold cleared).
    fn pve_with_boss_active() -> PveEngine {
        let mut pve = PveEngine::new();
        let mut state = make_state();
        pve.on_event(&GameEvent::StartGame, &mut state);
        pve.on_score_delta(BOSS_SCORE_THRESHOLD);
        pve
    }

    #[test]
    fn start_game_does_not_spawn_boss_immediately() {
        let mut pve = PveEngine::new();
        let mut state = make_state();
        let (envelopes, _) = pve.on_event(&GameEvent::StartGame, &mut state);
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate),
            "boss must not appear before score threshold"
        );
        assert_eq!(*pve.phase(), PvePhase::WaitingForScore);
    }

    #[test]
    fn boss_spawns_after_score_threshold() {
        let mut pve = PveEngine::new();
        let mut state = make_state();
        pve.on_event(&GameEvent::StartGame, &mut state);

        // One point short — still waiting
        let (envelopes, _) = pve.on_score_delta(BOSS_SCORE_THRESHOLD - 1);
        assert!(
            !envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate),
            "boss must not appear before threshold"
        );
        assert_eq!(*pve.phase(), PvePhase::WaitingForScore);

        // One more point crosses the threshold
        let (envelopes, _) = pve.on_score_delta(1);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate),
            "BossUpdate expected when threshold is reached"
        );
        assert_eq!(*pve.phase(), PvePhase::Fighting);
    }

    #[test]
    fn score_delta_damages_boss() {
        let mut pve = pve_with_boss_active();
        let max_hp = pve.boss_max_hp();
        let (envelopes, _) = pve.on_score_delta(100);
        assert_eq!(pve.boss_hp(), max_hp - 100);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate)
        );
    }

    #[test]
    fn score_delta_emits_boss_defeated_when_hp_zero() {
        let mut pve = pve_with_boss_active();
        let hp = pve.boss_max_hp();
        let (_, extra) = pve.on_score_delta(hp as u64);
        assert!(
            extra
                .iter()
                .any(|e| matches!(e, GameEvent::BossDefeated { boss_id: 0 }))
        );
    }

    #[test]
    fn boss_defeated_enters_cooldown_phase() {
        let mut pve = pve_with_boss_active();
        let hp = pve.boss_max_hp();
        pve.on_score_delta(hp as u64);
        assert_eq!(*pve.phase(), PvePhase::Cooldown);
    }

    #[test]
    fn tick_emits_boss_cleared_after_death_anim() {
        let mut pve = pve_with_boss_active();
        let hp = pve.boss_max_hp();
        pve.on_score_delta(hp as u64);

        let future = Instant::now() + Duration::from_millis(BOSS_DEATH_ANIM_MS + 100);
        let (envelopes, _) = pve.tick(future);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossCleared),
            "expected BossCleared after death animation"
        );
        // After death anim, engine waits for score threshold (not a timer).
        assert_eq!(*pve.phase(), PvePhase::WaitingForScore);
    }

    #[test]
    fn next_boss_spawns_after_score_threshold_post_kill() {
        let mut pve = pve_with_boss_active();
        let hp = pve.boss_max_hp();
        // Kill boss 0
        pve.on_score_delta(hp as u64);

        // Advance past death animation → WaitingForScore
        let t1 = Instant::now() + Duration::from_millis(BOSS_DEATH_ANIM_MS + 100);
        pve.tick(t1);
        assert_eq!(*pve.phase(), PvePhase::WaitingForScore);

        // Score threshold triggers boss 1
        let (envelopes, _) = pve.on_score_delta(BOSS_SCORE_THRESHOLD);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate),
            "expected BossUpdate for next boss after score threshold"
        );
        assert_eq!(pve.current_boss_index(), 1);
        assert_eq!(*pve.phase(), PvePhase::Fighting);
    }

    #[test]
    fn score_delta_ignored_during_cooldown() {
        let mut pve = pve_with_boss_active();
        let hp = pve.boss_max_hp();
        // Kill boss → enter cooldown
        pve.on_score_delta(hp as u64);
        assert_eq!(*pve.phase(), PvePhase::Cooldown);

        // Score during death-animation cooldown must not damage boss
        let hp_before = pve.boss_hp();
        pve.on_score_delta(500);
        assert_eq!(
            pve.boss_hp(),
            hp_before,
            "score during cooldown must not damage boss"
        );
    }

    #[test]
    fn all_bosses_defeated_then_victory() {
        let mut pve = PveEngine::new();
        let mut state = make_state();
        pve.on_event(&GameEvent::StartGame, &mut state);

        // Defeat all 3 story bosses; each cycle is:
        //   score threshold → boss spawns → kill boss → death anim → WaitingForScore
        for _ in 0..3 {
            // Reach score threshold to spawn the next boss
            pve.on_score_delta(BOSS_SCORE_THRESHOLD);
            assert_eq!(*pve.phase(), PvePhase::Fighting);

            // Kill the boss
            let hp = pve.boss_max_hp();
            pve.on_score_delta(hp as u64);
            assert_eq!(*pve.phase(), PvePhase::Cooldown);

            // Advance through death animation → WaitingForScore
            let t1 = Instant::now() + Duration::from_millis(BOSS_DEATH_ANIM_MS + 100);
            pve.tick(t1);
            assert_eq!(*pve.phase(), PvePhase::WaitingForScore);
        }

        // After 3 bosses, next threshold triggers VictoireFinale
        let (envelopes, _) = pve.on_score_delta(BOSS_SCORE_THRESHOLD);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::VictoireFinale),
            "VictoireFinale expected after all bosses defeated"
        );
        assert_eq!(*pve.phase(), PvePhase::Victory);

        // BallLost with 0 lives still triggers GameOver from on_event
        state.lives = 0;
        let (_, events) = pve.on_event(&GameEvent::BallLost, &mut state);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::GameOverTriggered { .. }))
        );
    }

    #[test]
    fn no_lives_triggers_game_over() {
        let mut pve = PveEngine::new();
        let mut state = make_state();
        state.lives = 0;

        let (_, events) = pve.on_event(&GameEvent::BallLost, &mut state);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::GameOverTriggered { .. }))
        );
    }

    #[test]
    fn envelopes_use_game_engine_sender() {
        // Score accumulated in WaitingForScore — no envelopes until threshold
        let mut pve = PveEngine::new();
        let (envelopes, _) = pve.on_score_delta(BOSS_SCORE_THRESHOLD);
        // Threshold reached → BossUpdate emitted from GameEngine
        for env in &envelopes {
            assert_eq!(env.from, ScreenId::GameEngine);
        }
    }
}
