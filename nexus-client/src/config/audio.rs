//! Audio settings for voice chat
//!
//! Configuration for voice chat audio including device selection,
//! voice quality, and push-to-talk settings.

use nexus_common::voice::VoiceQuality;
use serde::{Deserialize, Serialize};

// =============================================================================
// Localized Voice Quality
// =============================================================================

/// Wrapper for VoiceQuality that implements Display with i18n
///
/// This is needed because VoiceQuality is in nexus-common which doesn't
/// have access to the client's i18n system. The pick_list widget uses
/// Display to render options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalizedVoiceQuality(pub VoiceQuality);

impl LocalizedVoiceQuality {
    /// Get all quality options as localized wrappers
    pub fn all() -> Vec<LocalizedVoiceQuality> {
        VoiceQuality::all()
            .iter()
            .map(|&q| LocalizedVoiceQuality(q))
            .collect()
    }
}

impl std::fmt::Display for LocalizedVoiceQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", crate::i18n::t(self.0.translation_key()))
    }
}

impl From<VoiceQuality> for LocalizedVoiceQuality {
    fn from(q: VoiceQuality) -> Self {
        LocalizedVoiceQuality(q)
    }
}

impl From<LocalizedVoiceQuality> for VoiceQuality {
    fn from(lq: LocalizedVoiceQuality) -> Self {
        lq.0
    }
}

// =============================================================================
// Constants
// =============================================================================

/// Default PTT key (backtick)
pub const DEFAULT_PTT_KEY: &str = "`";

/// System default device identifier
pub const SYSTEM_DEFAULT_DEVICE: &str = "";

// =============================================================================
// PTT Mode
// =============================================================================

/// Push-to-talk activation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PttMode {
    /// Hold key to talk, release to stop
    #[default]
    Hold,
    /// Press to start talking, press again to stop
    Toggle,
}

impl PttMode {
    /// All PTT modes for the picker
    #[allow(dead_code)]
    pub const ALL: &'static [PttMode] = &[PttMode::Hold, PttMode::Toggle];

    /// Get the translation key for this mode
    pub fn translation_key(self) -> &'static str {
        match self {
            PttMode::Hold => "ptt-mode-hold",
            PttMode::Toggle => "ptt-mode-toggle",
        }
    }
}

impl std::fmt::Display for PttMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", crate::i18n::t(self.translation_key()))
    }
}

// =============================================================================
// Audio Settings
// =============================================================================

/// Audio settings for voice chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Output device name (empty string = system default)
    #[serde(default)]
    pub output_device: String,

    /// Input device name (empty string = system default)
    #[serde(default)]
    pub input_device: String,

    /// Voice quality preset (affects Opus bitrate)
    #[serde(default)]
    pub voice_quality: VoiceQuality,

    /// Push-to-talk key binding
    #[serde(default = "default_ptt_key")]
    pub ptt_key: String,

    /// Push-to-talk mode (hold or toggle)
    #[serde(default)]
    pub ptt_mode: PttMode,

    /// Enable noise suppression (default: true)
    #[serde(default = "default_true")]
    pub noise_suppression: bool,

    /// Enable echo cancellation (default: false, for headphone users)
    #[serde(default)]
    pub echo_cancellation: bool,

    /// Enable automatic gain control (default: true)
    #[serde(default = "default_true")]
    pub agc: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            output_device: SYSTEM_DEFAULT_DEVICE.to_string(),
            input_device: SYSTEM_DEFAULT_DEVICE.to_string(),
            voice_quality: VoiceQuality::default(),
            ptt_key: default_ptt_key(),
            ptt_mode: PttMode::default(),
            noise_suppression: true,
            echo_cancellation: false,
            agc: true,
        }
    }
}

fn default_ptt_key() -> String {
    DEFAULT_PTT_KEY.to_string()
}

impl AudioSettings {
    /// Check if using system default output device
    #[allow(dead_code)]
    pub fn is_default_output(&self) -> bool {
        self.output_device.is_empty()
    }

    /// Check if using system default input device
    #[allow(dead_code)]
    pub fn is_default_input(&self) -> bool {
        self.input_device.is_empty()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_audio_settings() {
        let settings = AudioSettings::default();
        assert!(settings.is_default_output());
        assert!(settings.is_default_input());
        assert_eq!(settings.voice_quality, VoiceQuality::High);
        assert_eq!(settings.ptt_key, DEFAULT_PTT_KEY);
        assert_eq!(settings.ptt_mode, PttMode::Hold);
        assert!(settings.noise_suppression);
        assert!(!settings.echo_cancellation);
        assert!(settings.agc);
    }

    #[test]
    fn test_audio_settings_serialization_roundtrip() {
        let settings = AudioSettings {
            output_device: "Headphones".to_string(),
            input_device: "USB Microphone".to_string(),
            voice_quality: VoiceQuality::VeryHigh,
            ptt_key: "F1".to_string(),
            ptt_mode: PttMode::Toggle,
            noise_suppression: false,
            echo_cancellation: true,
            agc: false,
        };

        let json = serde_json::to_string(&settings).expect("serialize");
        let deserialized: AudioSettings = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(settings.output_device, deserialized.output_device);
        assert_eq!(settings.input_device, deserialized.input_device);
        assert_eq!(settings.voice_quality, deserialized.voice_quality);
        assert_eq!(settings.ptt_key, deserialized.ptt_key);
        assert_eq!(settings.ptt_mode, deserialized.ptt_mode);
        assert_eq!(settings.noise_suppression, deserialized.noise_suppression);
        assert_eq!(settings.echo_cancellation, deserialized.echo_cancellation);
        assert_eq!(settings.agc, deserialized.agc);
    }

    #[test]
    fn test_ptt_mode_all() {
        assert_eq!(PttMode::ALL.len(), 2);
        assert!(PttMode::ALL.contains(&PttMode::Hold));
        assert!(PttMode::ALL.contains(&PttMode::Toggle));
    }

    #[test]
    fn test_is_default_device() {
        let mut settings = AudioSettings::default();
        assert!(settings.is_default_output());
        assert!(settings.is_default_input());

        settings.output_device = "Some Device".to_string();
        assert!(!settings.is_default_output());
        assert!(settings.is_default_input());

        settings.input_device = "Another Device".to_string();
        assert!(!settings.is_default_output());
        assert!(!settings.is_default_input());
    }
}
