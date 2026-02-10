//! Disconnect dialog state (kick/ban)

// =============================================================================
// Disconnect Dialog State
// =============================================================================

/// Action to take in the disconnect dialog
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DisconnectAction {
    /// Kick the user (can reconnect immediately)
    #[default]
    Kick,
    /// Ban the user's IP (cannot reconnect until ban expires)
    Ban,
}

/// Pre-defined ban duration options
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BanDuration {
    /// 10 minutes
    TenMinutes,
    /// 1 hour
    #[default]
    OneHour,
    /// 1 day
    OneDay,
    /// 7 days
    SevenDays,
    /// 30 days
    ThirtyDays,
    /// Permanent (no expiry)
    Permanent,
}

impl BanDuration {
    /// Get the duration string to send to the server
    pub fn as_duration_string(self) -> Option<String> {
        match self {
            BanDuration::TenMinutes => Some("10m".to_string()),
            BanDuration::OneHour => Some("1h".to_string()),
            BanDuration::OneDay => Some("1d".to_string()),
            BanDuration::SevenDays => Some("7d".to_string()),
            BanDuration::ThirtyDays => Some("30d".to_string()),
            BanDuration::Permanent => None,
        }
    }

    /// Get all duration options for the dropdown
    pub fn all() -> &'static [BanDuration] {
        &[
            BanDuration::TenMinutes,
            BanDuration::OneHour,
            BanDuration::OneDay,
            BanDuration::SevenDays,
            BanDuration::ThirtyDays,
            BanDuration::Permanent,
        ]
    }

    /// Get the translation key for this duration
    pub fn translation_key(&self) -> &'static str {
        match self {
            BanDuration::TenMinutes => "ban-duration-10m",
            BanDuration::OneHour => "ban-duration-1h",
            BanDuration::OneDay => "ban-duration-1d",
            BanDuration::SevenDays => "ban-duration-7d",
            BanDuration::ThirtyDays => "ban-duration-30d",
            BanDuration::Permanent => "ban-duration-permanent",
        }
    }
}

/// State for the disconnect user dialog
#[derive(Debug, Clone, Default)]
pub struct DisconnectDialogState {
    /// Nickname of the user to disconnect
    pub nickname: String,
    /// Selected action (kick or ban)
    pub action: DisconnectAction,
    /// Ban duration (only used when action is Ban)
    pub duration: BanDuration,
    /// Ban reason (optional, only used when action is Ban)
    pub reason: String,
    /// Error message from server (if any)
    pub error: Option<String>,
}

impl DisconnectDialogState {
    /// Create a new disconnect dialog for a user
    pub fn new(nickname: String) -> Self {
        Self {
            nickname,
            action: DisconnectAction::Kick,
            duration: BanDuration::OneHour,
            reason: String::new(),
            error: None,
        }
    }

    /// Create a new disconnect dialog with a specific action pre-selected
    pub fn with_action(nickname: String, action: DisconnectAction) -> Self {
        Self {
            nickname,
            action,
            duration: BanDuration::OneHour,
            reason: String::new(),
            error: None,
        }
    }
}
