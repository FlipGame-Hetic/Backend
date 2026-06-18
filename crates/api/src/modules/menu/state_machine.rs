use shared::model::ButtonId;
use shared::screen::{ScreenEnvelope, ScreenEventType, ScreenId, ScreenTarget};

fn make_menu_envelope(event_type: ScreenEventType, payload: serde_json::Value) -> ScreenEnvelope {
    ScreenEnvelope {
        from: ScreenId::BackScreen,
        to: ScreenTarget::Broadcast,
        event_type,
        payload,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPhase {
    Idle,
    Menu1,
    Menu2,
}

pub enum MenuResult {
    Envelopes(Vec<ScreenEnvelope>),
    StartGame {
        character_id: u8,
        envelopes: Vec<ScreenEnvelope>,
    },
    Ignored,
}

pub struct MenuStateMachine {
    pub phase: MenuPhase,
    cursor: u8,
}

impl Default for MenuStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuStateMachine {
    pub fn new() -> Self {
        Self {
            phase: MenuPhase::Idle,
            cursor: 0,
        }
    }

    pub fn handle_button(&mut self, id: &ButtonId) -> MenuResult {
        match self.phase {
            MenuPhase::Idle => self.handle_idle(id),
            MenuPhase::Menu1 => self.handle_menu1(id),
            MenuPhase::Menu2 => self.handle_menu2(id),
        }
    }

    fn handle_idle(&mut self, id: &ButtonId) -> MenuResult {
        match id {
            ButtonId::R2 => {
                self.phase = MenuPhase::Menu1;
                self.cursor = 0;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuNext,
                    serde_json::json!({ "step": 1, "cursor": 0 }),
                )])
            }
            _ => MenuResult::Ignored,
        }
    }

    fn handle_menu1(&mut self, id: &ButtonId) -> MenuResult {
        match id {
            ButtonId::L1 => {
                self.cursor = self.cursor.saturating_sub(1);
                let cursor = self.cursor;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuNavigateLeft,
                    serde_json::json!({ "step": 1, "cursor": cursor }),
                )])
            }
            ButtonId::R1 => {
                self.cursor = self.cursor.saturating_add(1);
                let cursor = self.cursor;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuNavigateRight,
                    serde_json::json!({ "step": 1, "cursor": cursor }),
                )])
            }
            ButtonId::L2 => {
                self.phase = MenuPhase::Idle;
                self.cursor = 0;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuCancel,
                    serde_json::json!({ "step": 0 }),
                )])
            }
            ButtonId::R2 => {
                let selected_mode = self.cursor;
                self.phase = MenuPhase::Menu2;
                self.cursor = 0;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuNext,
                    serde_json::json!({ "step": 2, "selected_mode": selected_mode, "cursor": 0 }),
                )])
            }
            _ => MenuResult::Ignored,
        }
    }

    fn handle_menu2(&mut self, id: &ButtonId) -> MenuResult {
        match id {
            ButtonId::L1 => {
                self.cursor = self.cursor.saturating_sub(1);
                let cursor = self.cursor;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuNavigateLeft,
                    serde_json::json!({ "step": 2, "cursor": cursor }),
                )])
            }
            ButtonId::R1 => {
                self.cursor = self.cursor.saturating_add(1);
                let cursor = self.cursor;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuNavigateRight,
                    serde_json::json!({ "step": 2, "cursor": cursor }),
                )])
            }
            ButtonId::L2 => {
                self.phase = MenuPhase::Menu1;
                self.cursor = 0;
                MenuResult::Envelopes(vec![make_menu_envelope(
                    ScreenEventType::MenuPrev,
                    serde_json::json!({ "step": 1, "cursor": 0 }),
                )])
            }
            ButtonId::R2 => {
                let character_id = self.cursor;
                self.phase = MenuPhase::Idle;
                self.cursor = 0;
                MenuResult::StartGame {
                    character_id,
                    envelopes: vec![make_menu_envelope(
                        ScreenEventType::GameBegin,
                        serde_json::json!({ "character_id": character_id }),
                    )],
                }
            }
            _ => MenuResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_envelope(result: MenuResult) -> ScreenEnvelope {
        match result {
            MenuResult::Envelopes(mut v) => v.remove(0),
            MenuResult::StartGame { mut envelopes, .. } => envelopes.remove(0),
            MenuResult::Ignored => panic!("expected envelope, got Ignored"),
        }
    }

    fn assert_ignored(result: MenuResult) {
        assert!(matches!(result, MenuResult::Ignored));
    }

    // --- Idle phase ---

    #[test]
    fn idle_r2_transitions_to_menu1() {
        let mut m = MenuStateMachine::new();
        let env = first_envelope(m.handle_button(&ButtonId::R2));
        assert_eq!(m.phase, MenuPhase::Menu1);
        assert_eq!(env.event_type, ScreenEventType::MenuNext);
        assert_eq!(env.payload["step"], 1);
        assert_eq!(env.payload["cursor"], 0);
    }

    #[test]
    fn idle_ignores_l1_r1_l2() {
        let mut m = MenuStateMachine::new();
        assert_ignored(m.handle_button(&ButtonId::L1));
        assert_ignored(m.handle_button(&ButtonId::R1));
        assert_ignored(m.handle_button(&ButtonId::L2));
        assert_eq!(m.phase, MenuPhase::Idle);
    }

    // --- Menu1 phase ---

    #[test]
    fn menu1_r2_transitions_to_menu2_and_resets_cursor() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        m.handle_button(&ButtonId::R1); // cursor = 1
        let env = first_envelope(m.handle_button(&ButtonId::R2)); // → Menu2
        assert_eq!(m.phase, MenuPhase::Menu2);
        assert_eq!(m.cursor, 0);
        assert_eq!(env.event_type, ScreenEventType::MenuNext);
        assert_eq!(env.payload["step"], 2);
        assert_eq!(env.payload["selected_mode"], 1);
    }

    #[test]
    fn menu1_l2_cancels_to_idle() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        let env = first_envelope(m.handle_button(&ButtonId::L2));
        assert_eq!(m.phase, MenuPhase::Idle);
        assert_eq!(env.event_type, ScreenEventType::MenuCancel);
        assert_eq!(env.payload["step"], 0);
    }

    #[test]
    fn menu1_r1_increments_cursor() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        let env = first_envelope(m.handle_button(&ButtonId::R1));
        assert_eq!(m.cursor, 1);
        assert_eq!(env.event_type, ScreenEventType::MenuNavigateRight);
        assert_eq!(env.payload["step"], 1);
        assert_eq!(env.payload["cursor"], 1);
    }

    #[test]
    fn menu1_l1_decrements_cursor_saturating() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        let env = first_envelope(m.handle_button(&ButtonId::L1)); // cursor stays 0
        assert_eq!(m.cursor, 0);
        assert_eq!(env.event_type, ScreenEventType::MenuNavigateLeft);
        assert_eq!(env.payload["cursor"], 0);
    }

    // --- Menu2 phase ---

    #[test]
    fn menu2_r2_starts_game_with_cursor_as_character_id() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        m.handle_button(&ButtonId::R2); // → Menu2
        m.handle_button(&ButtonId::R1); // cursor = 1
        m.handle_button(&ButtonId::R1); // cursor = 2
        let result = m.handle_button(&ButtonId::R2);
        match result {
            MenuResult::StartGame {
                character_id,
                envelopes,
                ..
            } => {
                assert_eq!(character_id, 2);
                assert_eq!(envelopes[0].event_type, ScreenEventType::GameBegin);
                assert_eq!(envelopes[0].payload["character_id"], 2);
            }
            _ => panic!("expected StartGame"),
        }
        assert_eq!(m.phase, MenuPhase::Idle);
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn menu2_l2_goes_back_to_menu1() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        m.handle_button(&ButtonId::R2); // → Menu2
        let env = first_envelope(m.handle_button(&ButtonId::L2));
        assert_eq!(m.phase, MenuPhase::Menu1);
        assert_eq!(m.cursor, 0);
        assert_eq!(env.event_type, ScreenEventType::MenuPrev);
        assert_eq!(env.payload["step"], 1);
    }

    #[test]
    fn menu2_r1_increments_cursor() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        m.handle_button(&ButtonId::R2); // → Menu2
        let env = first_envelope(m.handle_button(&ButtonId::R1));
        assert_eq!(m.cursor, 1);
        assert_eq!(env.event_type, ScreenEventType::MenuNavigateRight);
        assert_eq!(env.payload["step"], 2);
    }

    #[test]
    fn menu2_l1_decrements_cursor_saturating() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        m.handle_button(&ButtonId::R2); // → Menu2
        let env = first_envelope(m.handle_button(&ButtonId::L1));
        assert_eq!(m.cursor, 0);
        assert_eq!(env.event_type, ScreenEventType::MenuNavigateLeft);
    }

    // --- Full flow ---

    #[test]
    fn full_flow_idle_to_game() {
        let mut m = MenuStateMachine::new();
        assert_eq!(m.phase, MenuPhase::Idle);
        m.handle_button(&ButtonId::R2); // → Menu1
        assert_eq!(m.phase, MenuPhase::Menu1);
        m.handle_button(&ButtonId::R2); // → Menu2
        assert_eq!(m.phase, MenuPhase::Menu2);
        let result = m.handle_button(&ButtonId::R2); // → StartGame
        assert!(matches!(result, MenuResult::StartGame { .. }));
        assert_eq!(m.phase, MenuPhase::Idle);
    }

    #[test]
    fn back_navigation_forgets_menu2_progress() {
        let mut m = MenuStateMachine::new();
        m.handle_button(&ButtonId::R2); // → Menu1
        m.handle_button(&ButtonId::R2); // → Menu2
        m.handle_button(&ButtonId::R1); // cursor = 1 in Menu2
        m.handle_button(&ButtonId::L2); // → back to Menu1
        assert_eq!(m.phase, MenuPhase::Menu1);
        assert_eq!(m.cursor, 0, "cursor must reset when going back");
    }

    #[test]
    fn envelopes_are_broadcast() {
        let mut m = MenuStateMachine::new();
        let env = first_envelope(m.handle_button(&ButtonId::R2));
        assert_eq!(env.to, ScreenTarget::Broadcast);
        assert_eq!(env.from, ScreenId::BackScreen);
    }
}
