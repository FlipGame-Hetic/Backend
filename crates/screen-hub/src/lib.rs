//! `screen-hub` — thread-safe connection registry and message routing for
//! pinball screen clients.
//!
//! Screens (front, back, DMD) connect over WebSocket.  On connect they call
//! [`registry::ScreenRegistry::register`], which returns a [`registry::ScreenHandle`]
//! containing a message receiver and a RAII guard.  When the connection closes
//! the guard is dropped and the screen is automatically removed from the
//! registry.
//!
//! [`router::ScreenRouter`] wraps the registry and adds an interceptor
//! pipeline so messages can be inspected, mutated, or dropped before delivery.
pub mod error;
pub mod registry;
pub mod router;
