use std::collections::VecDeque;
use std::time::Instant;

use crate::combo::model::{ButtonPress, ComboDefinition, ComboEffect, ComboResult};
use crate::engine::config::{
    COMBO_2_BONUS, COMBO_3_BONUS, COMBO_4_BONUS, COMBO_5_BONUS, COMBO_6_BONUS, COMBO_7_BONUS,
    COMBO_8_BONUS, COMBO_9_BONUS, COMBO_10_BONUS, COMBO_11_BONUS, COMBO_14_BONUS, COMBO_15_BONUS,
    COMBO_16_BONUS, COMBO_BUFFER_MAX, COMBO_DETECTION_WINDOW_MS, COMBO_PENALTY_PTS,
    COMBO_PENALTY_REPEAT,
};

fn combo_table() -> Vec<ComboDefinition> {
    use ButtonPress::{Left as G, Right as D};
    vec![
        // 7-button combos first (longest → highest priority)
        // Very hard: pure alternating
        ComboDefinition {
            id: 14,
            sequence: vec![D, G, D, G, D, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_14_BONUS,
        },
        ComboDefinition {
            id: 15,
            sequence: vec![G, G, D, G, D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_15_BONUS,
        },
        ComboDefinition {
            id: 16,
            sequence: vec![D, D, G, D, G, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_16_BONUS,
        },
        // Standard 7-button combos
        ComboDefinition {
            id: 6,
            sequence: vec![G, G, D, D, G, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_6_BONUS,
        },
        ComboDefinition {
            id: 7,
            sequence: vec![G, G, D, G, D, G, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_7_BONUS,
        },
        ComboDefinition {
            id: 8,
            sequence: vec![D, D, D, G, G, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_8_BONUS,
        },
        // 6-button combos
        // Hard: alternating patterns
        ComboDefinition {
            id: 9,
            sequence: vec![D, G, D, G, D, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_9_BONUS,
        },
        ComboDefinition {
            id: 10,
            sequence: vec![G, G, D, G, D, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_10_BONUS,
        },
        ComboDefinition {
            id: 11,
            sequence: vec![D, G, G, D, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_11_BONUS,
        },
        // Standard 6-button combos
        ComboDefinition {
            id: 3,
            sequence: vec![G, G, D, D, D, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_3_BONUS,
        },
        ComboDefinition {
            id: 4,
            sequence: vec![G, G, D, D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_4_BONUS,
        },
        ComboDefinition {
            id: 5,
            sequence: vec![G, G, D, G, G, D],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_5_BONUS,
        },
        // 5-button combos
        ComboDefinition {
            id: 2,
            sequence: vec![G, G, D, D, G],
            max_duration_ms: COMBO_DETECTION_WINDOW_MS,
            bonus_pts: COMBO_2_BONUS,
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

        let result = self.match_combos(now);
        if matches!(result, ComboResult::Activated(_)) {
            self.buffer.clear();
        }
        result
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
                let sequence = combo
                    .sequence
                    .iter()
                    .map(|p| match p {
                        ButtonPress::Left => "L",
                        ButtonPress::Right => "R",
                    })
                    .map(String::from)
                    .collect();
                return ComboResult::Activated(ComboEffect {
                    combo_id: combo.id,
                    bonus_pts: combo.bonus_pts,
                    sequence,
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
    fn test_no_duplicate_sequences() {
        let table = combo_table();
        for i in 0..table.len() {
            for j in (i + 1)..table.len() {
                assert_ne!(
                    table[i].sequence, table[j].sequence,
                    "combo {} et {} ont la même séquence",
                    table[i].id, table[j].id
                );
            }
        }
    }

    #[test]
    fn test_no_suffix_conflicts() {
        let table = combo_table();
        for longer in &table {
            for shorter in &table {
                if shorter.sequence.len() >= longer.sequence.len() {
                    continue;
                }
                let n = shorter.sequence.len();
                let suffix = &longer.sequence[longer.sequence.len() - n..];
                assert_ne!(
                    suffix,
                    shorter.sequence.as_slice(),
                    "la séquence du combo {} est un suffixe du combo {} → conflit de détection",
                    shorter.id,
                    longer.id
                );
            }
        }
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
