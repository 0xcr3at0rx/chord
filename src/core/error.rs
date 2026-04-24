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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(
            ChordError::Config("missing file".into()).to_string(),
            "Configuration error: missing file"
        );
        assert_eq!(
            ChordError::Playback("ALSA underrun".into()).to_string(),
            "Playback error: ALSA underrun"
        );
        assert_eq!(
            ChordError::Internal("null pointer".into()).to_string(),
            "Internal error: null pointer"
        );
    }
}
