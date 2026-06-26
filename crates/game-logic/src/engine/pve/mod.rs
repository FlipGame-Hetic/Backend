//! PVE subsystem — boss spawning, HP management, phase transitions, and death animation.

pub mod difficulty;
pub mod engine;
pub mod ennemy;
pub mod events;
pub mod states;

pub use engine::PveEngine;
