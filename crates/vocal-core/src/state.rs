//! State machine for voice engine lifecycle
//!
//! Tracks the state of synthesis operations: UNINITIALIZED -> READY -> SYNTHESIZING -> PLAYING -> READY

use serde::{Deserialize, Serialize};

/// State of a voice engine or synthesis operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Engine not yet initialized
    Uninitialized,
    /// Engine ready for synthesis
    Ready,
    /// Synthesis in progress
    Synthesizing,
    /// Audio being played
    Playing,
    /// Synthesis was cancelled
    Cancelled,
    /// Engine encountered an error
    Error,
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::Ready => "ready",
            Self::Synthesizing => "synthesizing",
            Self::Playing => "playing",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_strings() {
        assert_eq!(State::Ready.as_str(), "ready");
        assert_eq!(State::Synthesizing.as_str(), "synthesizing");
    }
}
