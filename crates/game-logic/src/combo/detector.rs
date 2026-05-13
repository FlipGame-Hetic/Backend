use std::collections::VecDeque;
use std::time::Instant;

use crate::combo::model::{ButtonPress, ComboDefinition, ComboEffect, ComboResult};
use crate::engine::config::{
    COMBO_1_BONUS, COMBO_1_DURATION_MS, COMBO_1_MULTIPLIER, COMBO_2_BONUS, COMBO_2_DURATION_MS,
    COMBO_2_MULTIPLIER, COMBO_3_BONUS, COMBO_3_DURATION_MS, COMBO_3_MULTIPLIER, COMBO_4_BONUS,
    COMBO_4_DURATION_MS, COMBO_4_MULTIPLIER, COMBO_5_BONUS, COMBO_5_DURATION_MS,
    COMBO_5_MULTIPLIER, COMBO_6_BONUS, COMBO_6_DURATION_MS, COMBO_6_MULTIPLIER, COMBO_7_BONUS,
    COMBO_7_DURATION_MS, COMBO_7_MULTIPLIER, COMBO_8_BONUS, COMBO_8_DURATION_MS,
    COMBO_8_MULTIPLIER, COMBO_12_BONUS, COMBO_12_DURATION_MS, COMBO_12_MULTIPLIER, COMBO_13_BONUS,
    COMBO_13_DURATION_MS, COMBO_13_MULTIPLIER, COMBO_BUFFER_MAX, COMBO_DETECTION_WINDOW_MS,
    COMBO_PENALTY_PTS, COMBO_PENALTY_REPEAT,
};

fn combo_table() -> Vec<ComboDefinition> {
    use ButtonPress::{Left as G, Right as D};
    vec![
        // 7-button combos first (longest → highest priority)
        ComboDefinition {
            id: 6,
            sequence: vec![G, G, D, D, G, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_6_BONUS,
            multiplier: COMBO_6_MULTIPLIER,
            duration_ms: COMBO_6_DURATION_MS,
        },
        ComboDefinition {
            id: 7,
            sequence: vec![G, G, D, G, D, G, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_7_BONUS,
            multiplier: COMBO_7_MULTIPLIER,
            duration_ms: COMBO_7_DURATION_MS,
        },
        ComboDefinition {
            id: 8,
            sequence: vec![D, D, D, G, G, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_8_BONUS,
            multiplier: COMBO_8_MULTIPLIER,
            duration_ms: COMBO_8_DURATION_MS,
        },
        // 6-button combos
        ComboDefinition {
            id: 3,
            sequence: vec![G, G, D, D, D, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_3_BONUS,
            multiplier: COMBO_3_MULTIPLIER,
            duration_ms: COMBO_3_DURATION_MS,
        },
        ComboDefinition {
            id: 4,
            sequence: vec![G, G, D, D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_4_BONUS,
            multiplier: COMBO_4_MULTIPLIER,
            duration_ms: COMBO_4_DURATION_MS,
        },
        ComboDefinition {
            id: 5,
            sequence: vec![G, G, D, G, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_5_BONUS,
            multiplier: COMBO_5_MULTIPLIER,
            duration_ms: COMBO_5_DURATION_MS,
        },
        // 5-button combos
        ComboDefinition {
            id: 2,
            sequence: vec![G, G, D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_2_BONUS,
            multiplier: COMBO_2_MULTIPLIER,
            duration_ms: COMBO_2_DURATION_MS,
        },
        // 4-button combos
        ComboDefinition {
            id: 1,
            sequence: vec![G, G, D, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_1_BONUS,
            multiplier: COMBO_1_MULTIPLIER,
            duration_ms: COMBO_1_DURATION_MS,
        },
        ComboDefinition {
            id: 12,
            sequence: vec![D, D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_12_BONUS,
            multiplier: COMBO_12_MULTIPLIER,
            duration_ms: COMBO_12_DURATION_MS,
        },
        // 3-button combos
        ComboDefinition {
            id: 13,
            sequence: vec![D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_13_BONUS,
            multiplier: COMBO_13_MULTIPLIER,
            duration_ms: COMBO_13_DURATION_MS,
        },
    ]
}

pub struct ComboDetector {
    buffer: VecDeque<(ButtonPress, Instant)>,
    table: Vec<ComboDefinition>,
}

impl ComboDetector {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(COMBO_BUFFER_MAX),
            table: combo_table(),
        }
    }

    pub fn push(&mut self, button: ButtonPress, now: Instant) -> ComboResult {
        if self.buffer.len() >= COMBO_BUFFER_MAX {
            self.buffer.pop_front();
        }
        self.buffer.push_back((button, now));

        if let Some(penalty) = self.check_penalty() {
            self.buffer.clear();
            return penalty;
        }

        self.match_combos(now)
    }

    fn match_combos(&self, now: Instant) -> ComboResult {
        for combo in &self.table {
            let n = combo.sequence.len();
            if self.buffer.len() < n {
                continue;
            }

            let tail: Vec<&(ButtonPress, Instant)> = self.buffer.iter().rev().take(n).collect();

            let matches = tail
                .iter()
                .zip(combo.sequence.iter().rev())
                .all(|((press, _), expected)| press == expected);

            if !matches {
                continue;
            }

            let oldest_time = tail[n - 1].1;
            let elapsed = now.duration_since(oldest_time).as_millis() as u64;
            if elapsed <= combo.max_duration_ms {
                return ComboResult::Activated(ComboEffect {
                    combo_id: combo.id,
                    bonus_pts: combo.bonus_pts,
                    multiplier: combo.multiplier,
                    duration_ms: combo.duration_ms,
                });
            }
        }
        ComboResult::None
    }

    fn check_penalty(&self) -> Option<ComboResult> {
        if self.buffer.len() < COMBO_PENALTY_REPEAT {
            return None;
        }
        let last: Vec<&ButtonPress> = self
            .buffer
            .iter()
            .rev()
            .take(COMBO_PENALTY_REPEAT)
            .map(|(p, _)| p)
            .collect();

        let all_left = last.iter().all(|p| **p == ButtonPress::Left);
        let all_right = last.iter().all(|p| **p == ButtonPress::Right);

        if all_left || all_right {
            Some(ComboResult::Penalty {
                pts: COMBO_PENALTY_PTS,
            })
        } else {
            None
        }
    }
}

impl Default for ComboDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn press_seq(detector: &mut ComboDetector, seq: &[ButtonPress], now: Instant) -> ComboResult {
        let mut result = ComboResult::None;
        for (i, btn) in seq.iter().enumerate() {
            let t = now + Duration::from_millis(i as u64 * 100);
            result = detector.push(*btn, t);
        }
        result
    }

    #[test]
    fn test_combo_1_ggdd() {
        let mut d = ComboDetector::new();
        let now = Instant::now();
        let result = press_seq(
            &mut d,
            &[
                ButtonPress::Left,
                ButtonPress::Left,
                ButtonPress::Right,
                ButtonPress::Right,
            ],
            now,
        );
        match result {
            ComboResult::Activated(e) => {
                assert_eq!(e.combo_id, 1);
                assert!((e.multiplier - COMBO_1_MULTIPLIER).abs() < f32::EPSILON);
            }
            other => panic!("expected Activated, got {other:?}"),
        }
    }

    #[test]
    fn test_combo_2_priority_over_combo_1() {
        let mut d = ComboDetector::new();
        let now = Instant::now();
        let result = press_seq(
            &mut d,
            &[
                ButtonPress::Left,
                ButtonPress::Left,
                ButtonPress::Right,
                ButtonPress::Right,
                ButtonPress::Left,
            ],
            now,
        );
        match result {
            ComboResult::Activated(e) => assert_eq!(e.combo_id, 2),
            other => panic!("expected combo 2, got {other:?}"),
        }
    }

    #[test]
    fn test_combo_9_penalty_right() {
        let mut d = ComboDetector::new();
        let now = Instant::now();
        let result = press_seq(&mut d, &[ButtonPress::Right; 7], now);
        assert!(matches!(
            result,
            ComboResult::Penalty {
                pts: COMBO_PENALTY_PTS
            }
        ));
    }

    #[test]
    fn test_combo_10_penalty_left() {
        let mut d = ComboDetector::new();
        let now = Instant::now();
        let result = press_seq(&mut d, &[ButtonPress::Left; 7], now);
        assert!(matches!(
            result,
            ComboResult::Penalty {
                pts: COMBO_PENALTY_PTS
            }
        ));
    }

    #[test]
    fn test_combo_too_slow() {
        let mut d = ComboDetector::new();
        let base = Instant::now();
        d.push(ButtonPress::Left, base);
        d.push(ButtonPress::Left, base + Duration::from_millis(5_000));
        let result = d.push(ButtonPress::Right, base + Duration::from_millis(10_000));
        assert!(matches!(result, ComboResult::None));
    }

    #[test]
    fn test_longest_match_priority() {
        let mut d = ComboDetector::new();
        let now = Instant::now();
        let result = press_seq(
            &mut d,
            &[
                ButtonPress::Left,
                ButtonPress::Left,
                ButtonPress::Right,
                ButtonPress::Right,
                ButtonPress::Left,
            ],
            now,
        );
        match result {
            ComboResult::Activated(e) => assert_eq!(e.combo_id, 2),
            other => panic!("expected combo 2 (longest), got {other:?}"),
        }
    }
}
