//! WebRTC audio processor for voice enhancement
//!
//! Wraps the webrtc-audio-processing crate to provide:
//! - Automatic Gain Control (AGC)
//! - Noise Suppression (NS)
//! - Echo Cancellation (AEC)

use nexus_common::voice::{VOICE_CHANNELS, VOICE_SAMPLES_PER_FRAME};
use webrtc_audio_processing::{
    Config, EchoCancellation, EchoCancellationSuppressionLevel, GainControl, GainControlMode,
    InitializationConfig, NoiseSuppression, NoiseSuppressionLevel, Processor, VoiceDetection,
    VoiceDetectionLikelihood,
};

// =============================================================================
// Audio Processor Settings
// =============================================================================

/// Settings for audio processing features
///
/// Default values are tuned for the common case of headphone users:
/// - Noise suppression ON: Removes background noise with minimal latency cost
/// - Echo cancellation OFF: Most users wear headphones; AEC adds latency and CPU overhead
/// - AGC ON: Normalizes volume levels across different microphones
/// - Transient suppression OFF: Can occasionally clip word beginnings; enable if typing while talking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioProcessorSettings {
    /// Enable noise suppression (default: true)
    /// Removes steady-state background noise (fans, AC, etc.)
    pub noise_suppression: bool,
    /// Enable echo cancellation (default: false)
    /// Only needed when using speakers instead of headphones.
    /// Adds latency and CPU overhead, so disabled by default.
    pub echo_cancellation: bool,
    /// Enable automatic gain control (default: true)
    /// Normalizes microphone volume to consistent levels.
    pub agc: bool,
    /// Enable transient suppression (default: false)
    /// Reduces keyboard clicks, mouse clicks, and other sudden noises.
    /// Can occasionally clip the start of words, so disabled by default.
    pub transient_suppression: bool,
}

impl Default for AudioProcessorSettings {
    fn default() -> Self {
        Self {
            noise_suppression: true,
            echo_cancellation: false,
            agc: true,
            transient_suppression: false,
        }
    }
}

// =============================================================================
// Audio Processor
// =============================================================================

/// WebRTC audio processor for voice enhancement
///
/// Processes audio frames to improve voice quality through AGC,
/// noise suppression, and echo cancellation.
pub struct AudioProcessor {
    /// The WebRTC audio processor instance
    processor: Processor,
    /// Current settings
    settings: AudioProcessorSettings,
}

impl AudioProcessor {
    /// Create a new audio processor with the given settings
    ///
    /// # Arguments
    /// * `settings` - Initial processor settings
    ///
    /// # Returns
    /// * `Ok(AudioProcessor)` - Processor ready for use
    /// * `Err(String)` - Error message if initialization failed
    pub fn new(settings: AudioProcessorSettings) -> Result<Self, String> {
        // WebRTC processor is hardcoded to 48kHz, 10ms frames (480 samples)
        // which matches our VOICE_* constants exactly
        let init_config = InitializationConfig {
            num_capture_channels: VOICE_CHANNELS as i32,
            num_render_channels: VOICE_CHANNELS as i32,
            ..InitializationConfig::default()
        };

        let mut processor =
            Processor::new(&init_config).map_err(|e| format!("Failed to create processor: {e}"))?;

        // Apply initial settings
        let config = Self::build_config(&settings);
        processor.set_config(config);

        Ok(Self {
            processor,
            settings,
        })
    }

    /// Build a Config from our settings
    fn build_config(settings: &AudioProcessorSettings) -> Config {
        Config {
            echo_cancellation: if settings.echo_cancellation {
                Some(EchoCancellation {
                    suppression_level: EchoCancellationSuppressionLevel::Moderate,
                    enable_extended_filter: true,
                    enable_delay_agnostic: true,
                    stream_delay_ms: None,
                })
            } else {
                None
            },
            gain_control: if settings.agc {
                Some(GainControl {
                    mode: GainControlMode::AdaptiveDigital,
                    target_level_dbfs: 3,
                    compression_gain_db: 9,
                    enable_limiter: true,
                })
            } else {
                None
            },
            noise_suppression: if settings.noise_suppression {
                Some(NoiseSuppression {
                    suppression_level: NoiseSuppressionLevel::Moderate,
                })
            } else {
                None
            },
            voice_detection: Some(VoiceDetection {
                detection_likelihood: VoiceDetectionLikelihood::High,
            }),
            enable_transient_suppressor: settings.transient_suppression,
            enable_high_pass_filter: true,
        }
    }

    /// Update processor settings dynamically
    ///
    /// # Arguments
    /// * `settings` - New processor settings
    pub fn update_settings(&mut self, settings: AudioProcessorSettings) {
        if settings != self.settings {
            let config = Self::build_config(&settings);
            self.processor.set_config(config);
            self.settings = settings;
        }
    }

    /// Get current settings
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn settings(&self) -> AudioProcessorSettings {
        self.settings
    }

    /// Check if voice was detected in the last processed capture frame
    ///
    /// Used for VAD-gated transmission in toggle PTT mode.
    pub fn has_voice(&self) -> bool {
        self.processor.get_stats().has_voice.unwrap_or(false)
    }

    /// Process a capture (microphone) frame
    ///
    /// This should be called on microphone input before encoding.
    /// The frame is modified in place.
    ///
    /// # Arguments
    /// * `frame` - Audio frame (must be VOICE_SAMPLES_PER_FRAME samples)
    pub fn process_capture_frame(&mut self, frame: &mut [f32]) -> Result<(), String> {
        if frame.len() != VOICE_SAMPLES_PER_FRAME as usize {
            return Err(format!(
                "Expected {} samples, got {}",
                VOICE_SAMPLES_PER_FRAME,
                frame.len()
            ));
        }

        self.processor
            .process_capture_frame(frame)
            .map_err(|e| format!("Capture processing error: {e}"))
    }

    /// Process a render (speaker) frame
    ///
    /// This should be called on audio before playback. Required for
    /// echo cancellation to work - the processor needs to know what
    /// audio is being played to remove it from the microphone signal.
    ///
    /// # Arguments
    /// * `frame` - Audio frame (must be VOICE_SAMPLES_PER_FRAME samples)
    pub fn process_render_frame(&mut self, frame: &mut [f32]) -> Result<(), String> {
        if frame.len() != VOICE_SAMPLES_PER_FRAME as usize {
            return Err(format!(
                "Expected {} samples, got {}",
                VOICE_SAMPLES_PER_FRAME,
                frame.len()
            ));
        }

        self.processor
            .process_render_frame(frame)
            .map_err(|e| format!("Render processing error: {e}"))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_default_settings() {
        let settings = AudioProcessorSettings::default();
        assert!(settings.noise_suppression);
        assert!(!settings.echo_cancellation);
        assert!(settings.agc);
    }

    // Serialize WebRTC processor tests - the library has global state that isn't
    // thread-safe when creating multiple Processor instances concurrently.
    #[test]
    #[serial]
    fn test_processor_creation() {
        let processor = AudioProcessor::new(AudioProcessorSettings::default());
        assert!(processor.is_ok());
    }

    #[test]
    #[serial]
    fn test_process_capture_frame() {
        let mut processor = AudioProcessor::new(AudioProcessorSettings::default()).unwrap();
        let mut frame = vec![0.0f32; VOICE_SAMPLES_PER_FRAME as usize];

        let result = processor.process_capture_frame(&mut frame);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_process_render_frame() {
        let mut processor = AudioProcessor::new(AudioProcessorSettings::default()).unwrap();
        let mut frame = vec![0.0f32; VOICE_SAMPLES_PER_FRAME as usize];

        let result = processor.process_render_frame(&mut frame);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_wrong_frame_size() {
        let mut processor = AudioProcessor::new(AudioProcessorSettings::default()).unwrap();
        let mut frame = vec![0.0f32; 100]; // Wrong size

        assert!(processor.process_capture_frame(&mut frame).is_err());
        assert!(processor.process_render_frame(&mut frame).is_err());
    }

    #[test]
    #[serial]
    fn test_update_settings() {
        let mut processor = AudioProcessor::new(AudioProcessorSettings::default()).unwrap();

        let new_settings = AudioProcessorSettings {
            noise_suppression: false,
            echo_cancellation: true,
            agc: false,
            transient_suppression: true,
        };

        processor.update_settings(new_settings);
        assert_eq!(processor.settings(), new_settings);
    }

    #[test]
    #[serial]
    fn test_signal_processing() {
        use nexus_common::voice::VOICE_SAMPLE_RATE;

        let mut processor = AudioProcessor::new(AudioProcessorSettings {
            noise_suppression: true,
            echo_cancellation: false,
            agc: true,
            transient_suppression: false,
        })
        .unwrap();

        // Create a test signal (sine wave with some noise)
        let mut frame: Vec<f32> = (0..VOICE_SAMPLES_PER_FRAME)
            .map(|i| {
                let t = i as f32 / VOICE_SAMPLE_RATE as f32;
                let signal = f32::sin(2.0 * std::f32::consts::PI * 440.0 * t) * 0.3;
                let noise = ((i * 12345) % 100) as f32 / 1000.0 - 0.05;
                signal + noise
            })
            .collect();

        // Process should succeed
        let result = processor.process_capture_frame(&mut frame);
        assert!(result.is_ok());

        // Frame should still have reasonable values
        let max_val = frame.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_val <= 1.5, "Output should be reasonably bounded");
    }
}
