use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChordError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Playback error: {0}")]
    Playback(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type ChordResult<T> = Result<T, ChordError>;
