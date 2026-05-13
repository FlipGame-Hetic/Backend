use thiserror::Error;

#[derive(Error, Debug)]
pub enum ComboError {
    #[error("buffer overflow")]
    BufferOverflow,
}
