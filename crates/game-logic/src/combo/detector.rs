//! Detects button-press combos from a rolling buffer of recent inputs.

use std::collections::VecDeque;
use std::time::Instant;

use crate::combo::model::{ButtonPress, ComboDefinition, ComboEffect, ComboResult};
use crate::engine::config;

/// Build the static combo table ordered longest → shortest so the first match
/// is always the most specific one (no shorter suffix can fire instead).
fn combo_table() -> Vec<ComboDefinition> {
    use ButtonPress::{Left as G, Right as D};
    let cfg = config::get();
    let window = cfg.combo_detection_window_ms;
    vec![
        // Very hard: pure alternating rhythm
        ComboDefinition {
            id: 14,
            sequence: vec![D, G, D, G, D, G, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_14_bonus,
        },
        ComboDefinition {
            id: 15,
            sequence: vec![G, G, D, G, D, D, G],
            max_duration_ms: window,
            bonus_pts: cfg.combo_15_bonus,
        },
        ComboDefinition {
            id: 16,
            sequence: vec![D, D, G, D, G, G, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_16_bonus,
        },
        // Standard 7-button combos
        ComboDefinition {
            id: 6,
            sequence: vec![G, G, D, D, G, G, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_6_bonus,
        },
        ComboDefinition {
            id: 7,
            sequence: vec![G, G, D, G, D, G, G],
            max_duration_ms: window,
            bonus_pts: cfg.combo_7_bonus,
        },
        ComboDefinition {
            id: 8,
            sequence: vec![D, D, D, G, G, D, G],
            max_duration_ms: window,
            bonus_pts: cfg.combo_8_bonus,
        },
        // Hard: alternating patterns
        ComboDefinition {
            id: 9,
            sequence: vec![D, G, D, G, D, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_9_bonus,
        },
        ComboDefinition {
            id: 10,
            sequence: vec![G, G, D, G, D, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_10_bonus,
        },
        ComboDefinition {
            id: 11,
            sequence: vec![D, G, G, D, G, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_11_bonus,
        },
        // Standard 6-button combos
        ComboDefinition {
            id: 3,
            sequence: vec![G, G, D, D, D, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_3_bonus,
        },
        ComboDefinition {
            id: 4,
            sequence: vec![G, G, D, D, D, G],
            max_duration_ms: window,
            bonus_pts: cfg.combo_4_bonus,
        },
        ComboDefinition {
            id: 5,
            sequence: vec![G, G, D, G, G, D],
            max_duration_ms: window,
            bonus_pts: cfg.combo_5_bonus,
        },
        ComboDefinition {
            id: 2,
            sequence: vec![G, G, D, D, G],
            max_duration_ms: window,
            bonus_pts: cfg.combo_2_bonus,
        },
    ]
}

/// Stateful combo detector: keeps a rolling buffer of recent button presses
/// and checks for matching sequences after each new press.
pub struct ComboDetector {
    /// Sliding window of (button, timestamp) pairs — oldest at the front.
    buffer: VecDeque<(ButtonPress, Instant)>,
    table: Vec<ComboDefinition>,
}

impl ComboDetector {
    pub fn new() -> Self {
        let buffer_max = config::get().combo_buffer_max;
        Self {
            buffer: VecDeque::with_capacity(buffer_max),
            table: combo_table(),
        }
    }

    /// Record a new button press. Checks for spam penalty first, then combo
    /// match. Clears the buffer after a penalty or successful combo.
    pub fn push(&mut self, button: ButtonPress, now: Instant) -> ComboResult {
        let buffer_max = config::get().combo_buffer_max;
        if self.buffer.len() >= buffer_max {
            self.buffer.pop_front();
        }
        self.buffer.push_back((button, now));

        // Penalty check runs before matching so spam can never complete a combo.
        if let Some(penalty) = self.check_penalty() {
            self.buffer.clear();
            return penalty;
        }

        let result = self.match_combos(now);
        // Clear after a hit so the same presses can't feed into the next combo.
        if matches!(result, ComboResult::Activated(_)) {
            self.buffer.clear();
        }
        result
    }

    /// Scan the combo table against the tail of the buffer (longest match first).
    /// Both the buffer tail and the sequence definition are compared in reverse
    /// so index 0 always means "most recent press".
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

            // `tail[n-1]` is the oldest press that belongs to this match.
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

    /// Return a penalty if the last `combo_penalty_repeat` presses are all
    /// the same button (pure left-spam or right-spam).
    fn check_penalty(&self) -> Option<ComboResult> {
        let cfg = config::get();
        if self.buffer.len() < cfg.combo_penalty_repeat {
            return None;
        }
        let last: Vec<&ButtonPress> = self
            .buffer
            .iter()
            .rev()
            .take(cfg.combo_penalty_repeat)
            .map(|(p, _)| p)
            .collect();

        let all_left = last.iter().all(|p| **p == ButtonPress::Left);
        let all_right = last.iter().all(|p| **p == ButtonPress::Right);

        if all_left || all_right {
            Some(ComboResult::Penalty {
                pts: cfg.combo_penalty_pts,
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

    /// Push a sequence of buttons spaced 100 ms apart, return the last result.
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
        assert!(matches!(result, ComboResult::Penalty { .. }));
    }

    #[test]
    fn test_combo_10_penalty_left() {
        let mut d = ComboDetector::new();
        let now = Instant::now();
        let result = press_seq(&mut d, &[ButtonPress::Left; 7], now);
        assert!(matches!(result, ComboResult::Penalty { .. }));
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
