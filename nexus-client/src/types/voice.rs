//! Voice session types for tracking active voice state

use uuid::Uuid;

/// Active voice session state
///
/// Tracks the current voice session for a connection, including the target
/// (channel or user message), authentication token, and list of participants.
#[derive(Debug, Clone)]
pub struct VoiceSession {
    /// Target channel (e.g., "#general") or other user's nickname for user message voice
    pub target: String,
    /// Voice token for UDP authentication
    #[allow(dead_code)] // Used in Phase 2 for UDP authentication
    pub token: Uuid,
    /// Nicknames of users currently in this voice session
    pub participants: Vec<String>,
}

impl VoiceSession {
    /// Create a new voice session
    pub fn new(target: String, token: Uuid, participants: Vec<String>) -> Self {
        Self {
            target,
            token,
            participants,
        }
    }

    /// Check if the target is a channel (starts with #)
    #[allow(dead_code)] // Used in Phase 2 for UI logic
    pub fn is_channel(&self) -> bool {
        self.target.starts_with('#')
    }

    /// Add a participant to the session
    pub fn add_participant(&mut self, nickname: String) {
        if !self.participants.iter().any(|n| n == &nickname) {
            self.participants.push(nickname);
            self.participants.sort_by_key(|a| a.to_lowercase());
        }
    }

    /// Remove a participant from the session
    pub fn remove_participant(&mut self, nickname: &str) {
        self.participants.retain(|n| n != nickname);
    }

    /// Get the number of participants
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}
