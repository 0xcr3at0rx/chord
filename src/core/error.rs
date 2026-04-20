use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum ChordError {
    #[error("Audio error: {0}")]
    Audio(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Playback error: {0}")]
    Playback(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Decoding error: {0}")]
    Decode(String),

    #[error("Metadata error: {0}")]
    Metadata(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type ChordResult<T> = Result<T, ChordError>;
