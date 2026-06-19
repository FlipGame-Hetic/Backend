use std::time::Instant;

use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

use crate::engine::config::{BOSS_0_HP, BOSS_COOLDOWN_MS, BOSS_DEATH_ANIM_MS};
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
        let mut envelopes = Vec::new();
        let mut extra_events = Vec::new();

        match event {
            GameEvent::StartGame => {
                self.reset_to_boss(0, &mut envelopes);
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

    /// Apply a positive score delta as boss damage.
    ///
    /// Only active during `PvePhase::Fighting`. During cooldown or other phases
    /// no damage is applied (score-only phase between bosses).
    pub fn on_score_delta(&mut self, delta: u64) -> (Vec<ScreenEnvelope>, Vec<GameEvent>) {
        let mut envelopes = Vec::new();
        let mut extra_events = Vec::new();

        if self.state.phase != PvePhase::Fighting || delta == 0 {
            return (envelopes, extra_events);
        }

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

        (envelopes, extra_events)
    }

    /// Advance the cooldown state machine.
    ///
    /// Call this periodically from the service layer. Returns envelopes to dispatch
    /// and extra game events to process.
    pub fn tick(&mut self, now: Instant) -> (Vec<ScreenEnvelope>, Vec<GameEvent>) {
        let mut envelopes = Vec::new();
        let extra_events = Vec::new();

        if self.state.phase != PvePhase::Cooldown {
            return (envelopes, extra_events);
        }

        let cooldown = match self.state.cooldown.as_mut() {
            Some(c) => c,
            None => return (envelopes, extra_events),
        };

        if let Some(cleared_at) = cooldown.cleared_at {
            // Phase 2: cooldown without a boss → spawn next boss
            let elapsed_ms = now.duration_since(cleared_at).as_millis() as u64;
            if elapsed_ms >= BOSS_COOLDOWN_MS {
                let next_index = cooldown.next_boss_index;
                self.state.cooldown = None;
                self.spawn_next(next_index, &mut envelopes);
            }
        } else {
            // Phase 1: waiting for death animation to finish → emit BossCleared
            let elapsed_ms = now.duration_since(cooldown.defeated_at).as_millis() as u64;
            if elapsed_ms >= BOSS_DEATH_ANIM_MS {
                let boss_id = self.boss.kind.id();
                envelopes.push(make_event_envelope(
                    ScreenEventType::BossCleared,
                    serde_json::json!({ "boss_id": boss_id }),
                ));
                cooldown.cleared_at = Some(now);
            }
        }

        (envelopes, extra_events)
    }

    fn enter_cooldown(&mut self) {
        let next_index = self.state.current_boss_index.saturating_add(1);
        self.state.phase = PvePhase::Cooldown;
        self.state.cooldown = Some(CooldownState {
            next_boss_index: next_index,
            defeated_at: Instant::now(),
            cleared_at: None,
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
    use crate::engine::config::DEFAULT_LIVES;
    use crate::engine::states::GameState;
    use std::time::Duration;

    fn make_state() -> GameState {
        GameState::new(DEFAULT_LIVES)
    }

    #[test]
    fn start_game_emits_boss_update() {
        let mut pve = PveEngine::new();
        let mut state = make_state();
        let (envelopes, _) = pve.on_event(&GameEvent::StartGame, &mut state);
        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate)
        );
    }

    #[test]
    fn score_delta_damages_boss() {
        let mut pve = PveEngine::new();
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
        let mut pve = PveEngine::new();
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
        let mut pve = PveEngine::new();
        let hp = pve.boss_max_hp();
        pve.on_score_delta(hp as u64);
        assert_eq!(*pve.phase(), PvePhase::Cooldown);
    }

    #[test]
    fn tick_emits_boss_cleared_after_death_anim() {
        let mut pve = PveEngine::new();
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
    }

    #[test]
    fn tick_emits_next_boss_update_after_full_cooldown() {
        let mut pve = PveEngine::new();
        let hp = pve.boss_max_hp();
        pve.on_score_delta(hp as u64);

        // Advance past death animation
        let t1 = Instant::now() + Duration::from_millis(BOSS_DEATH_ANIM_MS + 100);
        pve.tick(t1);

        // Advance past cooldown
        let t2 = t1 + Duration::from_millis(BOSS_COOLDOWN_MS + 100);
        let (envelopes, _) = pve.tick(t2);

        assert!(
            envelopes
                .iter()
                .any(|e| e.event_type == ScreenEventType::BossUpdate),
            "expected BossUpdate for next boss after cooldown"
        );
        assert_eq!(pve.current_boss_index(), 1);
        assert_eq!(*pve.phase(), PvePhase::Fighting);
    }

    #[test]
    fn score_delta_ignored_during_cooldown() {
        let mut pve = PveEngine::new();
        let hp = pve.boss_max_hp();
        // Kill boss → enter cooldown
        pve.on_score_delta(hp as u64);
        assert_eq!(*pve.phase(), PvePhase::Cooldown);

        // Score during cooldown should not affect boss
        let hp_before = pve.boss_hp();
        pve.on_score_delta(500);
        assert_eq!(
            pve.boss_hp(),
            hp_before,
            "score during cooldown must not damage boss"
        );
    }

    #[test]
    fn all_bosses_defeated_enters_cooldown_then_victory() {
        let mut pve = PveEngine::new();
        let mut state = make_state();

        // Defeat all 3 story bosses
        for _ in 0..3 {
            let hp = pve.boss_max_hp();
            pve.on_score_delta(hp as u64);
            // Advance through death anim + cooldown
            let t1 = Instant::now() + Duration::from_millis(BOSS_DEATH_ANIM_MS + 100);
            pve.tick(t1);
            let t2 = t1 + Duration::from_millis(BOSS_COOLDOWN_MS + 100);
            let (envelopes, _) = pve.tick(t2);
            // After 3rd defeat, VictoireFinale should be emitted
            if pve.current_boss_index() == 2 || *pve.phase() == PvePhase::Victory {
                let victory = envelopes
                    .iter()
                    .any(|e| e.event_type == ScreenEventType::VictoireFinale);
                if victory {
                    break;
                }
            }
        }

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
        let mut pve = PveEngine::new();
        let (envelopes, _) = pve.on_score_delta(100);
        for env in &envelopes {
            assert_eq!(env.from, ScreenId::GameEngine);
        }
    }
}
