//! Combo subsystem — button sequences, streaks, and temporary multipliers.

pub mod detector;
pub mod error;
pub mod model;
pub mod multiplier;
pub mod streak;

pub use detector::ComboDetector;
pub use error::ComboError;
pub use model::{ButtonPress, ComboEffect, ComboResult};
pub use multiplier::MultiplierState;
pub use streak::StreakState;
