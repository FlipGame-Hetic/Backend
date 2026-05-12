use shared::screen::{ScreenEnvelope, ScreenId, ScreenTarget};

use crate::engine::config::BOSS_0_HP;
use crate::engine::events::{GameEvent, GameOverReason};
use crate::engine::pve::difficulty::{boss_damage_to_health, scale_hp};
use crate::engine::pve::ennemy::boss::Boss;
use crate::engine::pve::ennemy::kind::BossKind;
use crate::engine::pve::states::{PvePhase, PveState};
use crate::engine::states::GameState;

pub struct PveEngine {
    state: PveState,
    boss: Boss,
}

impl PveEngine {
    pub fn new() -> Self {
        let kind = BossKind::GLaDOS;
        let boss = Boss::new(kind, 0);
        let initial_hp = boss.health.max;
        Self {
            state: PveState::new(initial_hp),
            boss,
        }
    }

    pub fn on_event(
        &mut self,
        event: &GameEvent,
        game_state: &mut GameState,
    ) -> (Vec<ScreenEnvelope>, Vec<GameEvent>) {
        let mut envelopes = Vec::new();
        let mut extra_events = Vec::new();

        match event {
            GameEvent::StartGame { .. } => {
                self.reset_to_boss(0, &mut envelopes);
            }

            GameEvent::BumperHit { pts } | GameEvent::BumperTriangleHit { pts } => {
                let damage = boss_damage_to_health(*pts, self.state.current_boss_index);
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
                    self.transition_after_defeat(&mut envelopes, &mut extra_events);
                }
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

    fn transition_after_defeat(
        &mut self,
        envelopes: &mut Vec<ScreenEnvelope>,
        _extra_events: &mut Vec<GameEvent>,
    ) {
        let next_index = self.state.current_boss_index + 1;

        if BossKind::from_index(next_index).is_some() {
            self.reset_to_boss(next_index, envelopes);
        } else {
            // All 3 story bosses defeated
            self.state.endless_level += 1;
            if self.state.endless_level == 1 {
                self.state.phase = PvePhase::Victory;
                envelopes.push(make_event_envelope(
                    "VictoireFinale",
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
                    "EndlessScaling",
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
        "BossUpdate",
        serde_json::json!({ "boss_id": boss_id, "boss_hp": hp, "boss_max_hp": max_hp }),
    )
}

fn make_event_envelope(event_type: &str, payload: serde_json::Value) -> ScreenEnvelope {
    ScreenEnvelope {
        from: ScreenId::BackScreen,
        to: ScreenTarget::Broadcast,
        event_type: event_type.to_owned(),
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::DEFAULT_LIVES;
    use crate::engine::states::GameState;

    fn make_state() -> GameState {
        GameState::new(DEFAULT_LIVES)
    }

    #[test]
    fn test_boss_defeated_transitions_to_next() {
        let mut pve = PveEngine::new();
        let mut state = make_state();

        let hp = pve.boss.health.max;
        let (_, events) = pve.on_event(&GameEvent::BumperHit { pts: hp }, &mut state);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::BossDefeated { boss_id: 0 }))
        );
        assert_eq!(pve.current_boss_index(), 1);
    }

    #[test]
    fn test_all_bosses_defeated_triggers_victory() {
        let mut pve = PveEngine::new();
        let mut state = make_state();

        for _ in 0..3 {
            let hp = pve.boss.health.max;
            pve.on_event(&GameEvent::BumperHit { pts: hp }, &mut state);
        }

        assert_eq!(*pve.phase(), PvePhase::Victory);
    }

    #[test]
    fn test_no_lives_triggers_game_over() {
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
}
