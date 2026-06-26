//! HTTP API binary for the Flipper pinball backend
//!
//! Exposes REST endpoints and two WebSocket upgrade routes via Axum
//! `AppState` is the single shared object injected into every handler;
//! it holds the DB pool, JWT secret, game engine, and all realtime channels

pub mod app;
pub mod config;
pub mod errors;
pub mod modules;
pub mod router;
pub mod state;
