//! Game engine central event loop, state machine, scoring, and PVE.

pub mod components;
pub mod config;
pub mod core;
pub mod events;
pub mod pve;
pub mod scoring;
pub mod services;
pub mod states;

pub use core::GameEngine;
