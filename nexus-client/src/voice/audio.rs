//! Audio device management and streaming
//!
//! Provides audio device enumeration, microphone capture, and speaker playback
//! using the cpal crate for cross-platform audio I/O. Uses f32 samples throughout
//! for compatibility with WebRTC audio processing.

use std::collections::HashMap;
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
            if let Ok(desc) = device.description() {
                let name = desc.name().to_string();
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
            if let Ok(desc) = device.description() {
                let name = desc.name().to_string();
                // Skip adding if it's already in the list
                if !devices.iter().any(|d| d.name == name) {
                    devices.push(AudioDevice::new(name, false));
                }
            }
        }
    }

    devices
}

/// Find an output device by name, or return the default
fn find_output_device(name: &str) -> Option<Device> {
    let host = get_host();

    if name.is_empty() || name == SYSTEM_DEFAULT_DEVICE_NAME {
        return host.default_output_device();
    }

    host.output_devices()
        .ok()?
        .find(|d| d.description().is_ok_and(|desc| desc.name() == name))
        .or_else(|| host.default_output_device())
}

/// Find an input device by name, or return the default
fn find_input_device(name: &str) -> Option<Device> {
    let host = get_host();

    if name.is_empty() || name == SYSTEM_DEFAULT_DEVICE_NAME {
        return host.default_input_device();
    }

    host.input_devices()
        .ok()?
        .find(|d| d.description().is_ok_and(|desc| desc.name() == name))
        .or_else(|| host.default_input_device())
}

// =============================================================================
// Audio Capture
// =============================================================================

/// Audio capture from microphone
///
/// Captures audio at 48kHz mono for voice encoding. Uses f32 samples internally
/// for compatibility with WebRTC audio processing.
pub struct AudioCapture {
    /// The cpal input stream
    _stream: Stream,
    /// Buffer for captured audio samples (f32 normalized to -1.0..1.0)
    buffer: Arc<Mutex<Vec<f32>>>,
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
                    && c.min_sample_rate() <= VOICE_SAMPLE_RATE
                    && c.max_sample_rate() >= VOICE_SAMPLE_RATE
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
                        && c.min_sample_rate() <= VOICE_SAMPLE_RATE
                        && c.max_sample_rate() >= VOICE_SAMPLE_RATE
                        && supported_formats.contains(&c.sample_format())
                });

            if let Some(cfg) = stereo_config {
                (2u16, cfg.sample_format())
            } else {
                // On Windows, WASAPI shared mode handles sample rate conversion internally.
                // Try 48kHz with any supported format, even if 48kHz not explicitly listed.
                // Prefer F32 > I16 > U16 for best quality.
                #[cfg(target_os = "windows")]
                {
                    // For input (mic), prefer mono first (matches normal 48kHz path)
                    // Find best mono config (prefer F32 > I16 > U16)
                    let mono_configs: Vec<_> = device
                        .supported_input_configs()
                        .ok()
                        .map(|configs| {
                            configs
                                .filter(|c| {
                                    c.channels() == 1
                                        && supported_formats.contains(&c.sample_format())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let best_mono = mono_configs
                        .iter()
                        .find(|c| c.sample_format() == SampleFormat::F32)
                        .or_else(|| {
                            mono_configs
                                .iter()
                                .find(|c| c.sample_format() == SampleFormat::I16)
                        })
                        .or_else(|| mono_configs.first());

                    if let Some(cfg) = best_mono {
                        (1u16, cfg.sample_format())
                    } else {
                        // Fall back to stereo (will downmix)
                        let stereo_configs: Vec<_> = device
                            .supported_input_configs()
                            .ok()
                            .map(|configs| {
                                configs
                                    .filter(|c| {
                                        c.channels() == 2
                                            && supported_formats.contains(&c.sample_format())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        let best_stereo = stereo_configs
                            .iter()
                            .find(|c| c.sample_format() == SampleFormat::F32)
                            .or_else(|| {
                                stereo_configs
                                    .iter()
                                    .find(|c| c.sample_format() == SampleFormat::I16)
                            })
                            .or_else(|| stereo_configs.first());

                        if let Some(cfg) = best_stereo {
                            (2u16, cfg.sample_format())
                        } else {
                            return Err("Input device has no supported audio format".to_string());
                        }
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // Collect supported sample rates for error message
                    let supported_rates: Vec<String> = device
                        .supported_input_configs()
                        .map(|configs| {
                            configs
                                .map(|c| {
                                    if c.min_sample_rate() == c.max_sample_rate() {
                                        format!("{}Hz", c.min_sample_rate())
                                    } else {
                                        format!("{}-{}Hz", c.min_sample_rate(), c.max_sample_rate())
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let rates_str = if supported_rates.is_empty() {
                        "unknown".to_string()
                    } else {
                        supported_rates.join(", ")
                    };
                    return Err(format!(
                        "Input device doesn't support 48kHz (required for voice chat). \
                         Device supports: {}",
                        rates_str
                    ));
                }
            }
        };

        let config = StreamConfig {
            channels,
            sample_rate: VOICE_SAMPLE_RATE,
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
    /// Samples are f32 normalized to [-1.0, 1.0].
    pub fn take_frame(&self) -> Option<Vec<f32>> {
        let mut buffer = self.buffer.lock().ok()?;
        let frame_size = VOICE_SAMPLES_PER_FRAME as usize;

        if buffer.len() >= frame_size {
            let frame: Vec<f32> = buffer.drain(..frame_size).collect();
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

        // Calculate RMS of recent samples (f32 samples are already normalized)
        let sample_count = buffer.len().min(VOICE_SAMPLES_PER_FRAME as usize);
        let samples = &buffer[buffer.len() - sample_count..];

        let sum_squares: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();

        let rms = (sum_squares / sample_count as f64).sqrt();

        // Convert to 0-1 range with some headroom
        (rms * RMS_DISPLAY_SCALE).min(1.0) as f32
    }
}

/// Build a mono input stream for the given sample type
fn build_input_stream_mono<T>(
    device: &Device,
    config: &StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if active.load(Ordering::SeqCst)
                    && let Ok(mut buf) = buffer.lock()
                {
                    for sample in data {
                        buf.push(f32::from_sample(*sample));
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
    buffer: Arc<Mutex<Vec<f32>>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample,
    f32: FromSample<T>,
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
                        let left = f32::from_sample(chunk[0]);
                        let right = f32::from_sample(chunk[1]);
                        let mono = (left + right) * 0.5;
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
// Audio Mixer
// =============================================================================

/// Per-user audio buffer for mixing
struct UserAudioBuffer {
    /// Audio samples waiting to be mixed
    samples: Vec<f32>,
}

impl UserAudioBuffer {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(
                VOICE_SAMPLES_PER_FRAME as usize * MAX_PLAYBACK_BUFFER_FRAMES,
            ),
        }
    }
}

/// Shared state for the audio mixer (accessed from audio callback)
struct MixerState {
    /// Per-user audio buffers (keyed by lowercase nickname)
    user_buffers: HashMap<String, UserAudioBuffer>,
    /// Set of muted nicknames (lowercase)
    muted: std::collections::HashSet<String>,
    /// Whether all incoming audio is muted (deafened)
    deafened: bool,
}

impl MixerState {
    fn new() -> Self {
        Self {
            user_buffers: HashMap::new(),
            muted: std::collections::HashSet::new(),
            deafened: false,
        }
    }
}

/// Mixes multiple audio streams into one output
///
/// Properly sums audio from multiple simultaneous speakers instead of
/// concatenating sequentially. Each user has their own buffer, and the
/// audio callback mixes them together at playback time.
pub struct AudioMixer {
    /// The cpal output stream
    _stream: Stream,
    /// Shared mixer state (accessed from audio callback and main thread)
    state: Arc<Mutex<MixerState>>,
    /// Flag indicating if playback is active
    active: Arc<AtomicBool>,
    /// Receiver for audio stream errors
    error_rx: std_mpsc::Receiver<String>,
}

impl AudioMixer {
    /// Create a new audio mixer with the specified output device
    pub fn new(device_name: &str) -> Result<Self, String> {
        let device =
            find_output_device(device_name).ok_or_else(|| "Output device not found".to_string())?;

        let state = Arc::new(Mutex::new(MixerState::new()));
        let state_clone = state.clone();
        let active = Arc::new(AtomicBool::new(false));
        let active_clone = active.clone();

        // Create channel for error reporting from audio callback
        let (error_tx, error_rx) = std_mpsc::channel();

        // Check supported formats
        let supported_formats = [SampleFormat::F32, SampleFormat::I16, SampleFormat::U16];

        // First try mono at 48kHz
        let mono_config = device
            .supported_output_configs()
            .map_err(|e| format!("Failed to get supported configs: {}", e))?
            .find(|c| {
                c.channels() == 1
                    && c.min_sample_rate() <= VOICE_SAMPLE_RATE
                    && c.max_sample_rate() >= VOICE_SAMPLE_RATE
                    && supported_formats.contains(&c.sample_format())
            });

        // If mono not available, try stereo
        let (channels, sample_format) = if let Some(cfg) = mono_config {
            (1u16, cfg.sample_format())
        } else {
            let stereo_config = device
                .supported_output_configs()
                .map_err(|e| format!("Failed to get supported configs: {}", e))?
                .find(|c| {
                    c.channels() == 2
                        && c.min_sample_rate() <= VOICE_SAMPLE_RATE
                        && c.max_sample_rate() >= VOICE_SAMPLE_RATE
                        && supported_formats.contains(&c.sample_format())
                });

            if let Some(cfg) = stereo_config {
                (2u16, cfg.sample_format())
            } else {
                // On Windows, WASAPI shared mode handles sample rate conversion internally.
                // Try 48kHz with any supported format, even if 48kHz not explicitly listed.
                // Prefer F32 > I16 > U16 for best quality.
                #[cfg(target_os = "windows")]
                {
                    // Find best stereo config (prefer F32 > I16 > U16)
                    let stereo_configs: Vec<_> = device
                        .supported_output_configs()
                        .ok()
                        .map(|configs| {
                            configs
                                .filter(|c| {
                                    c.channels() == 2
                                        && supported_formats.contains(&c.sample_format())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let best_stereo = stereo_configs
                        .iter()
                        .find(|c| c.sample_format() == SampleFormat::F32)
                        .or_else(|| {
                            stereo_configs
                                .iter()
                                .find(|c| c.sample_format() == SampleFormat::I16)
                        })
                        .or_else(|| stereo_configs.first());

                    if let Some(cfg) = best_stereo {
                        (2u16, cfg.sample_format())
                    } else {
                        // Try mono (prefer F32 > I16 > U16)
                        let mono_configs: Vec<_> = device
                            .supported_output_configs()
                            .ok()
                            .map(|configs| {
                                configs
                                    .filter(|c| {
                                        c.channels() == 1
                                            && supported_formats.contains(&c.sample_format())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        let best_mono = mono_configs
                            .iter()
                            .find(|c| c.sample_format() == SampleFormat::F32)
                            .or_else(|| {
                                mono_configs
                                    .iter()
                                    .find(|c| c.sample_format() == SampleFormat::I16)
                            })
                            .or_else(|| mono_configs.first());

                        if let Some(cfg) = best_mono {
                            (1u16, cfg.sample_format())
                        } else {
                            return Err("Output device has no supported audio format".to_string());
                        }
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // Collect supported sample rates for error message
                    let supported_rates: Vec<String> = device
                        .supported_output_configs()
                        .map(|configs| {
                            configs
                                .map(|c| {
                                    if c.min_sample_rate() == c.max_sample_rate() {
                                        format!("{}Hz", c.min_sample_rate())
                                    } else {
                                        format!("{}-{}Hz", c.min_sample_rate(), c.max_sample_rate())
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let rates_str = if supported_rates.is_empty() {
                        "unknown".to_string()
                    } else {
                        supported_rates.join(", ")
                    };
                    return Err(format!(
                        "Output device doesn't support 48kHz (required for voice chat). \
                         Device supports: {}",
                        rates_str
                    ));
                }
            }
        };

        let config = StreamConfig {
            channels,
            sample_rate: VOICE_SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Default,
        };

        // Build the appropriate stream based on channel count and sample format
        let stream = match (channels, sample_format) {
            (1, SampleFormat::F32) => build_mixer_stream_mono::<f32>(
                &device,
                &config,
                state_clone,
                active_clone,
                error_tx,
            ),
            (1, SampleFormat::I16) => build_mixer_stream_mono::<i16>(
                &device,
                &config,
                state_clone,
                active_clone,
                error_tx,
            ),
            (1, SampleFormat::U16) => build_mixer_stream_mono::<u16>(
                &device,
                &config,
                state_clone,
                active_clone,
                error_tx,
            ),
            (2, SampleFormat::F32) => build_mixer_stream_stereo::<f32>(
                &device,
                &config,
                state_clone,
                active_clone,
                error_tx,
            ),
            (2, SampleFormat::I16) => build_mixer_stream_stereo::<i16>(
                &device,
                &config,
                state_clone,
                active_clone,
                error_tx,
            ),
            (2, SampleFormat::U16) => build_mixer_stream_stereo::<u16>(
                &device,
                &config,
                state_clone,
                active_clone,
                error_tx,
            ),
            _ => Err("Unsupported audio format".to_string()),
        }?;

        Ok(Self {
            _stream: stream,
            state,
            active,
            error_rx,
        })
    }

    /// Start the mixer
    pub fn start(&self) -> Result<(), String> {
        self.active.store(true, Ordering::SeqCst);
        self._stream
            .play()
            .map_err(|e| format!("Failed to start mixer: {}", e))
    }

    /// Stop the mixer
    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
        let _ = self._stream.pause();
    }

    /// Mute a user by nickname
    pub fn mute_user(&mut self, nickname: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.muted.insert(nickname.to_lowercase());
        }
    }

    /// Unmute a user by nickname
    pub fn unmute_user(&mut self, nickname: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.muted.remove(&nickname.to_lowercase());
        }
    }

    /// Check if a user is muted
    #[allow(dead_code)]
    pub fn is_muted(&self, nickname: &str) -> bool {
        self.state
            .lock()
            .map(|state| state.muted.contains(&nickname.to_lowercase()))
            .unwrap_or(false)
    }

    /// Set deafened state (mute all incoming audio)
    pub fn set_deafened(&mut self, deafened: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.deafened = deafened;
        }
    }

    /// Check for audio stream errors (non-blocking)
    pub fn check_error(&self) -> Option<String> {
        self.error_rx.try_recv().ok()
    }

    /// Queue audio from a user for playback
    ///
    /// Audio is buffered per-user and mixed together at playback time.
    /// Samples should be f32 normalized to [-1.0, 1.0].
    pub fn queue_audio(&self, nickname: &str, samples: &[f32]) {
        if let Ok(mut state) = self.state.lock() {
            // Skip if deafened or user is muted
            if state.deafened || state.muted.contains(&nickname.to_lowercase()) {
                return;
            }

            // Get or create buffer for this user
            let key = nickname.to_lowercase();
            let buffer = state
                .user_buffers
                .entry(key)
                .or_insert_with(UserAudioBuffer::new);

            buffer.samples.extend_from_slice(samples);

            // Limit buffer size to prevent latency buildup
            let max_size = VOICE_SAMPLES_PER_FRAME as usize * MAX_PLAYBACK_BUFFER_FRAMES;
            if buffer.samples.len() > max_size {
                let drain_count = buffer.samples.len() - max_size;
                buffer.samples.drain(..drain_count);
            }
        }
    }
}

/// Build a mono mixer output stream
fn build_mixer_stream_mono<T>(
    device: &Device,
    config: &StreamConfig,
    state: Arc<Mutex<MixerState>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample + FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if active.load(Ordering::SeqCst) {
                    if let Ok(mut state) = state.lock() {
                        // Mix all user buffers together
                        for (i, dst) in data.iter_mut().enumerate() {
                            let mut mixed: f32 = 0.0;
                            let mut has_audio = false;

                            for buffer in state.user_buffers.values_mut() {
                                if i < buffer.samples.len() {
                                    mixed += buffer.samples[i];
                                    has_audio = true;
                                }
                            }

                            // Soft clamp to prevent harsh clipping
                            if has_audio {
                                mixed = soft_clip(mixed);
                            }

                            *dst = T::from_sample(mixed);
                        }

                        // Drain consumed samples from all buffers
                        let consumed = data.len();
                        for buffer in state.user_buffers.values_mut() {
                            let drain_count = consumed.min(buffer.samples.len());
                            buffer.samples.drain(..drain_count);
                        }

                        // Remove empty buffers to free memory for users who stopped talking
                        state.user_buffers.retain(|_, b| !b.samples.is_empty());
                    } else {
                        // Couldn't lock state - output silence
                        for sample in data.iter_mut() {
                            *sample = T::from_sample(0.0f32);
                        }
                    }
                } else {
                    // Not active - output silence
                    for sample in data.iter_mut() {
                        *sample = T::from_sample(0.0f32);
                    }
                }
            },
            {
                let error_tx = error_tx.clone();
                move |err| {
                    let _ = error_tx.send(format!("Mixer error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build mixer stream: {}", e))
}

/// Build a stereo mixer output stream (upmixes from mono)
fn build_mixer_stream_stereo<T>(
    device: &Device,
    config: &StreamConfig,
    state: Arc<Mutex<MixerState>>,
    active: Arc<AtomicBool>,
    error_tx: std_mpsc::Sender<String>,
) -> Result<Stream, String>
where
    T: Sample + cpal::SizedSample + FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if active.load(Ordering::SeqCst) {
                    if let Ok(mut state) = state.lock() {
                        let samples_needed = data.len() / 2;

                        // Mix all user buffers together and upmix to stereo
                        for (i, chunk) in data.chunks_exact_mut(2).enumerate() {
                            let mut mixed: f32 = 0.0;
                            let mut has_audio = false;

                            for buffer in state.user_buffers.values_mut() {
                                if i < buffer.samples.len() {
                                    mixed += buffer.samples[i];
                                    has_audio = true;
                                }
                            }

                            // Soft clamp to prevent harsh clipping
                            if has_audio {
                                mixed = soft_clip(mixed);
                            }

                            let sample = T::from_sample(mixed);
                            chunk[0] = sample;
                            chunk[1] = sample;
                        }

                        // Drain consumed samples from all buffers
                        for buffer in state.user_buffers.values_mut() {
                            let drain_count = samples_needed.min(buffer.samples.len());
                            buffer.samples.drain(..drain_count);
                        }

                        // Remove empty buffers
                        state.user_buffers.retain(|_, b| !b.samples.is_empty());
                    } else {
                        // Couldn't lock state - output silence
                        for sample in data.iter_mut() {
                            *sample = T::from_sample(0.0f32);
                        }
                    }
                } else {
                    // Not active - output silence
                    for sample in data.iter_mut() {
                        *sample = T::from_sample(0.0f32);
                    }
                }
            },
            {
                let error_tx = error_tx.clone();
                move |err| {
                    let _ = error_tx.send(format!("Mixer error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to build stereo mixer stream: {}", e))
}

/// Soft clip function to prevent harsh digital clipping
///
/// Uses tanh-based soft clipping which smoothly limits the signal
/// as it approaches the maximum, resulting in less harsh distortion
/// when multiple loud sources are summed together.
fn soft_clip(sample: f32) -> f32 {
    // tanh gives smooth saturation, but we scale input to make it more gradual
    // This provides ~3dB of headroom before noticeable saturation
    (sample * 0.7).tanh() / 0.7_f32.tanh()
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
