//! Audio device management and streaming
//!
//! Provides audio device enumeration, microphone capture, and speaker playback
//! using the cpal crate for cross-platform audio I/O.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Host, Sample, SampleFormat, Stream, StreamConfig};

use nexus_common::voice::{VOICE_SAMPLE_RATE, VOICE_SAMPLES_PER_FRAME};

// =============================================================================
// Constants
// =============================================================================

/// System default device display name
pub const SYSTEM_DEFAULT_DEVICE_NAME: &str = "System Default";

/// Scaling factor for RMS to UI level conversion (provides headroom for typical speech)
const RMS_DISPLAY_SCALE: f64 = 2.0;

/// Maximum capture buffer size in frames (prevents unbounded growth if processing stalls)
const MAX_CAPTURE_BUFFER_FRAMES: usize = 10;

/// Maximum playback buffer size in frames (prevents latency buildup)
const MAX_PLAYBACK_BUFFER_FRAMES: usize = 20;

// =============================================================================
// Audio Device
// =============================================================================

/// Represents an audio device (input or output)
#[derive(Debug, Clone)]
pub struct AudioDevice {
    /// Device name for display
    pub name: String,
    /// Whether this represents the system default device
    pub is_default: bool,
}

impl AudioDevice {
    /// Create a new audio device entry
    pub fn new(name: String, is_default: bool) -> Self {
        Self { name, is_default }
    }

    /// Create the system default device entry
    pub fn system_default() -> Self {
        Self {
            name: SYSTEM_DEFAULT_DEVICE_NAME.to_string(),
            is_default: true,
        }
    }
}

impl std::fmt::Display for AudioDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl PartialEq for AudioDevice {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for AudioDevice {}

// =============================================================================
// Device Enumeration
// =============================================================================

/// Get the default audio host for the platform
fn get_host() -> Host {
    cpal::default_host()
}

/// List available audio output devices
///
/// Returns a list of output devices with "System Default" as the first entry.
pub fn list_output_devices() -> Vec<AudioDevice> {
    let mut devices = vec![AudioDevice::system_default()];
    let host = get_host();

    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            if let Ok(name) = device.name() {
                // Skip adding if it's already in the list
                if !devices.iter().any(|d| d.name == name) {
                    devices.push(AudioDevice::new(name, false));
                }
            }
        }
    }

    devices
}

/// List available audio input devices
///
/// Returns a list of input devices with "System Default" as the first entry.
pub fn list_input_devices() -> Vec<AudioDevice> {
    let mut devices = vec![AudioDevice::system_default()];
    let host = get_host();

    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            if let Ok(name) = device.name() {
                // Skip adding if it's already in the list
                if !devices.iter().any(|d| d.name == name) {
                    devices.push(AudioDevice::new(name, false));
                }
            }
        }
    }

    devices
}

/// Find an output device by name
///
/// If name is empty or "System Default", returns the default output device.
fn find_output_device(name: &str) -> Option<Device> {
    let host = get_host();

    if name.is_empty() || name == SYSTEM_DEFAULT_DEVICE_NAME {
        return host.default_output_device();
    }

    host.output_devices()
        .ok()?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
}

/// Find an input device by name
///
/// If name is empty or "System Default", returns the default input device.
fn find_input_device(name: &str) -> Option<Device> {
    let host = get_host();

    if name.is_empty() || name == SYSTEM_DEFAULT_DEVICE_NAME {
        return host.default_input_device();
    }

    host.input_devices()
        .ok()?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
}

// =============================================================================
// Audio Capture
// =============================================================================

/// Audio capture from microphone
///
/// Captures audio samples at 48kHz mono and provides them in frames
/// suitable for Opus encoding.
pub struct AudioCapture {
    /// The cpal input stream
    _stream: Stream,
    /// Buffer for captured audio samples
    buffer: Arc<Mutex<Vec<i16>>>,
    /// Flag indicating if capture is active
    active: Arc<AtomicBool>,
    /// Receiver for audio stream errors
    error_rx: std_mpsc::Receiver<String>,
}

impl AudioCapture {
    /// Create a new audio capture from the specified device
    ///
    /// # Arguments
    /// * `device_name` - Device name, or empty string for system default
    ///
    /// # Returns
    /// * `Ok(AudioCapture)` - Capture ready to start
    /// * `Err(String)` - Error message if device not found or couldn't be opened
    pub fn new(device_name: &str) -> Result<Self, String> {
        let device =
            find_input_device(device_name).ok_or_else(|| "Input device not found".to_string())?;

        let buffer = Arc::new(Mutex::new(Vec::with_capacity(
            VOICE_SAMPLES_PER_FRAME as usize * 4,
        )));
        let buffer_clone = buffer.clone();
        let active = Arc::new(AtomicBool::new(false));
        let active_clone = active.clone();

        // Create channel for error reporting from audio callback
        let (error_tx, error_rx) = std_mpsc::channel();

        // Check supported formats - must support 48kHz and a format we can handle
        let supported_formats = [SampleFormat::F32, SampleFormat::I16, SampleFormat::U16];

        // First try mono at 48kHz
        let mono_config = device
            .supported_input_configs()
            .map_err(|e| format!("Failed to get supported configs: {}", e))?
            .find(|c| {
                c.channels() == 1
                    && c.min_sample_rate().0 <= VOICE_SAMPLE_RATE
                    && c.max_sample_rate().0 >= VOICE_SAMPLE_RATE
                    && supported_formats.contains(&c.sample_format())
            });

        // If mono not available, try stereo (we'll downmix)
        let (channels, sample_format) = if let Some(cfg) = mono_config {
            (1u16, cfg.sample_format())
        } else {
            let stereo_config = device
                .supported_input_configs()
                .map_err(|e| format!("Failed to get supported configs: {}", e))?
                .find(|c| {
                    c.channels() == 2
                        && c.min_sample_rate().0 <= VOICE_SAMPLE_RATE
                        && c.max_sample_rate().0 >= VOICE_SAMPLE_RATE
                        && supported_formats.contains(&c.sample_format())
                });

            if let Some(cfg) = stereo_config {
                (2u16, cfg.sample_format())
            } else {
                return Err(
                    "No compatible audio format found (need 48kHz mono or stereo)".to_string(),
                );
            }
        };

        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(VOICE_SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        // Build stream based on sample format and channel count
        let stream = match (sample_format, channels) {
            (SampleFormat::I16, 1) => build_input_stream_mono::<i16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::F32, 1) => build_input_stream_mono::<f32>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::U16, 1) => build_input_stream_mono::<u16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::I16, 2) => build_input_stream_stereo::<i16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::F32, 2) => build_input_stream_stereo::<f32>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::U16, 2) => build_input_stream_stereo::<u16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            _ => return Err(format!("Unsupported sample format: {:?}", sample_format)),
        }?;

        Ok(Self {
            _stream: stream,
            buffer,
            active,
            error_rx,
        })
    }

    /// Start capturing audio
    pub fn start(&self) -> Result<(), String> {
        self.active.store(true, Ordering::SeqCst);
        self._stream
            .play()
            .map_err(|e| format!("Failed to start capture: {}", e))
    }

    /// Stop capturing audio
    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
        // Clear the buffer when stopping
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
    }

    /// Check if capture is currently active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Take a frame of audio samples for encoding
    ///
    /// Returns a frame of VOICE_SAMPLES_PER_FRAME samples if available,
    /// or None if not enough samples have been captured yet.
    pub fn take_frame(&self) -> Option<Vec<i16>> {
        let mut buffer = self.buffer.lock().ok()?;
        let frame_size = VOICE_SAMPLES_PER_FRAME as usize;

        if buffer.len() >= frame_size {
            let frame: Vec<i16> = buffer.drain(..frame_size).collect();
            Some(frame)
        } else {
            None
        }
    }

    /// Check for audio stream errors (non-blocking)
    ///
    /// Returns the first error if one has occurred, or None if no errors.
    /// Only returns the first error since the session will be torn down anyway.
    pub fn check_error(&self) -> Option<String> {
        self.error_rx.try_recv().ok()
    }

    /// Get the current input level (0.0 - 1.0) for UI display
    pub fn get_input_level(&self) -> f32 {
        let buffer = match self.buffer.lock() {
            Ok(b) => b,
            Err(_) => return 0.0,
        };

        if buffer.is_empty() {
            return 0.0;
        }

        // Calculate RMS of recent samples
        let sample_count = buffer.len().min(VOICE_SAMPLES_PER_FRAME as usize);
        let samples = &buffer[buffer.len() - sample_count..];

        let sum_squares: f64 = samples
            .iter()
            .map(|&s| {
                let normalized = s as f64 / i16::MAX as f64;
                normalized * normalized
            })
            .sum();

        let rms = (sum_squares / sample_count as f64).sqrt();

        // Convert to 0-1 range with some headroom
        (rms * RMS_DISPLAY_SCALE).min(1.0) as f32
    }
}

/// Build a mono input stream for the given sample type
fn build_input_stream_mono<T>(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<i16>>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample,
    i16: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if active.load(Ordering::SeqCst)
                    && let Ok(mut buf) = buffer.lock()
                {
                    for sample in data {
                        buf.push(i16::from_sample(*sample));
                    }
                    // Limit buffer size to prevent unbounded growth
                    let max_size = VOICE_SAMPLES_PER_FRAME as usize * MAX_CAPTURE_BUFFER_FRAMES;
                    if buf.len() > max_size {
                        let drain_count = buf.len() - max_size;
                        buf.drain(..drain_count);
                    }
                }
            },
            {
                let error_tx = error_tx.clone();
                move |err| {
                    // Send error to manager (ignore if receiver dropped)
                    let _ = error_tx.send(format!("Audio capture error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))
}

/// Build a stereo input stream that downmixes to mono
fn build_input_stream_stereo<T>(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<i16>>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample,
    i16: FromSample<T>,
    i32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if active.load(Ordering::SeqCst)
                    && let Ok(mut buf) = buffer.lock()
                {
                    // Downmix stereo to mono by averaging L+R channels
                    for chunk in data.chunks_exact(2) {
                        let left = i32::from_sample(chunk[0]);
                        let right = i32::from_sample(chunk[1]);
                        let mono = ((left + right) / 2) as i16;
                        buf.push(mono);
                    }
                    // Limit buffer size to prevent unbounded growth
                    let max_size = VOICE_SAMPLES_PER_FRAME as usize * MAX_CAPTURE_BUFFER_FRAMES;
                    if buf.len() > max_size {
                        let drain_count = buf.len() - max_size;
                        buf.drain(..drain_count);
                    }
                }
            },
            {
                let error_tx = error_tx.clone();
                move |err| {
                    // Send error to manager (ignore if receiver dropped)
                    let _ = error_tx.send(format!("Audio capture error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build stereo input stream: {}", e))
}

// =============================================================================
// Audio Playback
// =============================================================================

/// Audio playback to speakers
///
/// Plays back audio samples at 48kHz mono, mixing multiple incoming streams.
pub struct AudioPlayback {
    /// The cpal output stream
    _stream: Stream,
    /// Buffer for audio samples to play
    buffer: Arc<Mutex<Vec<i16>>>,
    /// Flag indicating if playback is active
    active: Arc<AtomicBool>,
    /// Receiver for audio stream errors
    error_rx: std_mpsc::Receiver<String>,
}

impl AudioPlayback {
    /// Create a new audio playback to the specified device
    ///
    /// # Arguments
    /// * `device_name` - Device name, or empty string for system default
    ///
    /// # Returns
    /// * `Ok(AudioPlayback)` - Playback ready to start
    /// * `Err(String)` - Error message if device not found or couldn't be opened
    pub fn new(device_name: &str) -> Result<Self, String> {
        let device =
            find_output_device(device_name).ok_or_else(|| "Output device not found".to_string())?;

        let buffer = Arc::new(Mutex::new(Vec::with_capacity(
            VOICE_SAMPLES_PER_FRAME as usize * MAX_CAPTURE_BUFFER_FRAMES,
        )));
        let buffer_clone = buffer.clone();
        let active = Arc::new(AtomicBool::new(false));
        let active_clone = active.clone();

        // Create channel for error reporting from audio callback
        let (error_tx, error_rx) = std_mpsc::channel();

        // Check supported formats - must support 48kHz and a format we can handle
        let supported_formats = [SampleFormat::F32, SampleFormat::I16, SampleFormat::U16];

        // First try mono at 48kHz
        let mono_config = device
            .supported_output_configs()
            .map_err(|e| format!("Failed to get supported configs: {}", e))?
            .find(|c| {
                c.channels() == 1
                    && c.min_sample_rate().0 <= VOICE_SAMPLE_RATE
                    && c.max_sample_rate().0 >= VOICE_SAMPLE_RATE
                    && supported_formats.contains(&c.sample_format())
            });

        // If mono not available, try stereo (we'll upmix)
        let (channels, sample_format) = if let Some(cfg) = mono_config {
            (1u16, cfg.sample_format())
        } else {
            let stereo_config = device
                .supported_output_configs()
                .map_err(|e| format!("Failed to get supported configs: {}", e))?
                .find(|c| {
                    c.channels() == 2
                        && c.min_sample_rate().0 <= VOICE_SAMPLE_RATE
                        && c.max_sample_rate().0 >= VOICE_SAMPLE_RATE
                        && supported_formats.contains(&c.sample_format())
                });

            if let Some(cfg) = stereo_config {
                (2u16, cfg.sample_format())
            } else {
                return Err(
                    "No compatible audio format found (need 48kHz mono or stereo)".to_string(),
                );
            }
        };

        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(VOICE_SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        // Build stream based on sample format and channel count
        let stream = match (sample_format, channels) {
            (SampleFormat::I16, 1) => build_output_stream_mono::<i16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::F32, 1) => build_output_stream_mono::<f32>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::U16, 1) => build_output_stream_mono::<u16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::I16, 2) => build_output_stream_stereo::<i16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::F32, 2) => build_output_stream_stereo::<f32>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            (SampleFormat::U16, 2) => build_output_stream_stereo::<u16>(
                &device,
                &config,
                buffer_clone,
                active_clone,
                error_tx,
            ),
            _ => return Err(format!("Unsupported sample format: {:?}", sample_format)),
        }?;

        Ok(Self {
            _stream: stream,
            buffer,
            active,
            error_rx,
        })
    }

    /// Start audio playback
    pub fn start(&self) -> Result<(), String> {
        self.active.store(true, Ordering::SeqCst);
        self._stream
            .play()
            .map_err(|e| format!("Failed to start playback: {}", e))
    }

    /// Stop audio playback
    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
        // Clear the buffer when stopping
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
    }

    /// Queue audio samples for playback
    ///
    /// Samples are added to the playback buffer and will be played
    /// in order as the audio device consumes them.
    pub fn queue_samples(&self, samples: &[i16]) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.extend_from_slice(samples);

            // Limit buffer size to prevent latency buildup
            let max_size = VOICE_SAMPLES_PER_FRAME as usize * MAX_PLAYBACK_BUFFER_FRAMES;
            if buffer.len() > max_size {
                let drain_count = buffer.len() - max_size;
                buffer.drain(..drain_count);
            }
        }
    }

    /// Check for audio stream errors (non-blocking)
    ///
    /// Returns the first error if one has occurred, or None if no errors.
    /// Only returns the first error since the session will be torn down anyway.
    pub fn check_error(&self) -> Option<String> {
        self.error_rx.try_recv().ok()
    }
}

/// Build a mono output stream for the given sample type
fn build_output_stream_mono<T>(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<i16>>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample + FromSample<i16>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if active.load(Ordering::SeqCst) {
                    if let Ok(mut buf) = buffer.lock() {
                        // Use drain for O(n) instead of remove(0) which is O(n²)
                        let available = buf.len().min(data.len());
                        for (dst, src) in data.iter_mut().zip(buf.drain(..available)) {
                            *dst = T::from_sample(src);
                        }
                        // Fill remainder with silence if underrun
                        for sample in &mut data[available..] {
                            *sample = T::from_sample(0i16);
                        }
                    } else {
                        // Couldn't lock buffer - output silence
                        for sample in data.iter_mut() {
                            *sample = T::from_sample(0i16);
                        }
                    }
                } else {
                    // Not active - output silence
                    for sample in data.iter_mut() {
                        *sample = T::from_sample(0i16);
                    }
                }
            },
            {
                let error_tx = error_tx.clone();
                move |err| {
                    // Send error to manager (ignore if receiver dropped)
                    let _ = error_tx.send(format!("Audio playback error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))
}

/// Build a stereo output stream that upmixes from mono
fn build_output_stream_stereo<T>(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<i16>>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample + FromSample<i16>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if active.load(Ordering::SeqCst) {
                    if let Ok(mut buf) = buffer.lock() {
                        // Upmix mono to stereo by duplicating samples to L+R
                        let samples_needed = data.len() / 2;
                        let available = buf.len().min(samples_needed);
                        let mut drain_iter = buf.drain(..available);

                        for chunk in data.chunks_exact_mut(2) {
                            if let Some(mono) = drain_iter.next() {
                                let sample = T::from_sample(mono);
                                chunk[0] = sample;
                                chunk[1] = sample;
                            } else {
                                // Underrun - output silence
                                chunk[0] = T::from_sample(0i16);
                                chunk[1] = T::from_sample(0i16);
                            }
                        }
                    } else {
                        // Couldn't lock buffer - output silence
                        for sample in data.iter_mut() {
                            *sample = T::from_sample(0i16);
                        }
                    }
                } else {
                    // Not active - output silence
                    for sample in data.iter_mut() {
                        *sample = T::from_sample(0i16);
                    }
                }
            },
            {
                let error_tx = error_tx.clone();
                move |err| {
                    // Send error to manager (ignore if receiver dropped)
                    let _ = error_tx.send(format!("Audio playback error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build stereo output stream: {}", e))
}

// =============================================================================
// Audio Mixer
// =============================================================================

/// Mixes multiple audio streams into one output
///
/// Used to combine audio from multiple voice chat participants.
pub struct AudioMixer {
    /// Playback device
    playback: AudioPlayback,
    /// Set of muted nicknames
    muted: std::collections::HashSet<String>,
    /// Whether all incoming audio is muted (deafened)
    deafened: bool,
}

impl AudioMixer {
    /// Create a new audio mixer with the specified output device
    pub fn new(device_name: &str) -> Result<Self, String> {
        let playback = AudioPlayback::new(device_name)?;
        Ok(Self {
            playback,
            muted: std::collections::HashSet::new(),
            deafened: false,
        })
    }

    /// Start the mixer
    pub fn start(&self) -> Result<(), String> {
        self.playback.start()
    }

    /// Stop the mixer
    pub fn stop(&self) {
        self.playback.stop();
    }

    /// Mute a user by nickname
    pub fn mute_user(&mut self, nickname: &str) {
        self.muted.insert(nickname.to_lowercase());
    }

    /// Unmute a user by nickname
    pub fn unmute_user(&mut self, nickname: &str) {
        self.muted.remove(&nickname.to_lowercase());
    }

    /// Check if a user is muted
    pub fn is_muted(&self, nickname: &str) -> bool {
        self.muted.contains(&nickname.to_lowercase())
    }

    /// Set deafened state (mute all incoming audio)
    pub fn set_deafened(&mut self, deafened: bool) {
        self.deafened = deafened;
    }

    /// Check for audio stream errors (non-blocking)
    ///
    /// Returns the first error if one has occurred, or None if no errors.
    /// Only returns the first error since the session will be torn down anyway.
    pub fn check_error(&self) -> Option<String> {
        self.playback.check_error()
    }

    /// Queue audio from a user for playback
    ///
    /// Audio from muted or deafened users is silently discarded.
    pub fn queue_audio(&self, nickname: &str, samples: &[i16]) {
        if !self.deafened && !self.is_muted(nickname) {
            self.playback.queue_samples(samples);
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_device_system_default() {
        let device = AudioDevice::system_default();
        assert_eq!(device.name, SYSTEM_DEFAULT_DEVICE_NAME);
        assert!(device.is_default);
    }

    #[test]
    fn test_audio_device_new() {
        let device = AudioDevice::new("Test Device".to_string(), false);
        assert_eq!(device.name, "Test Device");
        assert!(!device.is_default);
    }

    #[test]
    fn test_audio_device_equality() {
        let device1 = AudioDevice::new("Test".to_string(), false);
        let device2 = AudioDevice::new("Test".to_string(), true);
        let device3 = AudioDevice::new("Other".to_string(), false);

        assert_eq!(device1, device2); // Same name, different is_default
        assert_ne!(device1, device3); // Different name
    }

    #[test]
    fn test_list_output_devices_includes_default() {
        let devices = list_output_devices();
        assert!(!devices.is_empty());
        assert!(devices[0].is_default);
        assert_eq!(devices[0].name, SYSTEM_DEFAULT_DEVICE_NAME);
    }

    #[test]
    fn test_list_input_devices_includes_default() {
        let devices = list_input_devices();
        assert!(!devices.is_empty());
        assert!(devices[0].is_default);
        assert_eq!(devices[0].name, SYSTEM_DEFAULT_DEVICE_NAME);
    }
}
