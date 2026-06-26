//! Types shared across all crates in the pinball backend.
//!
//! - [`dto`] MQTT topic parsing and structure
//! - [`events`] Message payloads exchanged between the ESP32, the API, and the bridge
//! - [`model`] Enums for button IDs, game phases, hit types, etc.
//! - [`screen`] Types for the screen WebSocket protocol
pub mod dto;
pub mod events;
pub mod model;
pub mod screen;
