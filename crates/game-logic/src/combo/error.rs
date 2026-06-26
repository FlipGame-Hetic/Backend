//! Error types for the combo subsystem.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ComboError {
    /// Should never fire in practice `push` evicts before inserting.
    #[error("buffer overflow")]
    BufferOverflow,
}
