//! Voice chat manager
//!
//! Orchestrates all voice chat components: DTLS connection, audio capture/playback,
//! Opus codec, jitter buffer, and push-to-talk.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::mpsc;
use uuid::Uuid;

use nexus_common::voice::VoiceQuality;

use super::audio::{AudioCapture, AudioMixer};
use super::codec::{DecoderPool, VoiceEncoder};
use super::dtls::{VoiceDtlsCommand, VoiceDtlsEvent, run_voice_client};
use super::jitter::JitterBufferPool;

// =============================================================================
// Constants
// =============================================================================

/// Interval for processing audio frames (20ms = 50 frames/second)
const AUDIO_PROCESS_INTERVAL_MS: u64 = 20;

// =============================================================================
// Voice Events
// =============================================================================

/// Events emitted by the voice manager
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// DTLS connection established
    Connected,
    /// DTLS connection failed
    ConnectionFailed(String),
    /// DTLS connection lost
    Disconnected(Option<String>),
    /// A user started speaking
    SpeakingStarted(String),
    /// A user stopped speaking
    SpeakingStopped(String),
    /// Audio device error (should leave voice)
    AudioError(String),
    /// Local speaking state changed
    LocalSpeakingChanged(bool),
}

/// Commands to control the voice manager
#[derive(Debug)]
pub enum VoiceCommand {
    /// Start PTT (begin transmitting)
    StartTransmitting,
    /// Stop PTT (stop transmitting)
    StopTransmitting,
    /// Mute a user
    MuteUser(String),
    /// Unmute a user
    UnmuteUser(String),
    /// Set deafened state (mute all incoming audio)
    SetDeafened(bool),
    /// Update voice quality (bitrate) dynamically
    SetQuality(VoiceQuality),
    /// Stop voice session
    Stop,
}

// =============================================================================
// Voice Session Runner
// =============================================================================

/// Run a voice session
///
/// This function runs the voice session in the background, handling:
/// - DTLS connection to server
/// - Audio capture from microphone
/// - Audio playback to speakers
/// - Opus encoding/decoding
/// - Jitter buffering
///
/// # Arguments
/// * `server_addr` - Server address for DTLS connection
/// * `token` - Voice session token
/// * `input_device` - Input device name (empty for default)
/// * `output_device` - Output device name (empty for default)
/// * `quality` - Voice quality preset
/// * `event_tx` - Channel to send voice events
/// * `command_rx` - Channel to receive voice commands
pub async fn run_voice_session(
    server_addr: SocketAddr,
    token: Uuid,
    input_device: String,
    output_device: String,
    quality: VoiceQuality,
    event_tx: mpsc::UnboundedSender<VoiceEvent>,
    mut command_rx: mpsc::UnboundedReceiver<VoiceCommand>,
) {
    // Create channels for DTLS client
    let (dtls_event_tx, mut dtls_event_rx) = mpsc::unbounded_channel();
    let (dtls_command_tx, dtls_command_rx) = mpsc::unbounded_channel();

    // Spawn DTLS client task
    let dtls_handle = tokio::spawn(run_voice_client(
        server_addr,
        token,
        dtls_event_tx,
        dtls_command_rx,
    ));

    // Wait for DTLS connection
    let connected = loop {
        tokio::select! {
            event = dtls_event_rx.recv() => {
                match event {
                    Some(VoiceDtlsEvent::Connected) => {
                        let _ = event_tx.send(VoiceEvent::Connected);
                        break true;
                    }
                    Some(VoiceDtlsEvent::Error(e)) => {
                        let _ = event_tx.send(VoiceEvent::ConnectionFailed(e));
                        break false;
                    }
                    Some(VoiceDtlsEvent::Disconnected) => {
                        let _ = event_tx.send(VoiceEvent::ConnectionFailed("Connection closed".to_string()));
                        break false;
                    }
                    _ => continue,
                }
            }
            cmd = command_rx.recv() => {
                if matches!(cmd, Some(VoiceCommand::Stop) | None) {
                    let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
                    break false;
                }
            }
        }
    };

    if !connected {
        dtls_handle.abort();
        return;
    }

    // Initialize audio components
    let capture = match AudioCapture::new(&input_device) {
        Ok(c) => c,
        Err(e) => {
            let _ = event_tx.send(VoiceEvent::AudioError(format!("Input device error: {}", e)));
            let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
            dtls_handle.abort();
            return;
        }
    };

    let mut mixer = match AudioMixer::new(&output_device) {
        Ok(m) => m,
        Err(e) => {
            let _ = event_tx.send(VoiceEvent::AudioError(format!(
                "Output device error: {}",
                e
            )));
            let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
            dtls_handle.abort();
            return;
        }
    };

    // Start audio playback
    if let Err(e) = mixer.start() {
        let _ = event_tx.send(VoiceEvent::AudioError(format!(
            "Failed to start playback: {}",
            e
        )));
        let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
        dtls_handle.abort();
        return;
    }

    // Initialize codec
    let mut encoder = match VoiceEncoder::new(quality) {
        Ok(e) => e,
        Err(e) => {
            let _ = event_tx.send(VoiceEvent::AudioError(format!("Encoder error: {}", e)));
            let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
            dtls_handle.abort();
            return;
        }
    };

    let mut decoder_pool = DecoderPool::new();
    let mut jitter_pool = JitterBufferPool::new();

    // State tracking
    let mut transmitting = false;
    let mut muted_users: HashSet<String> = HashSet::new();

    // Audio processing interval
    let mut audio_interval =
        tokio::time::interval(Duration::from_millis(AUDIO_PROCESS_INTERVAL_MS));

    loop {
        tokio::select! {
            // Process audio at regular intervals
            _ = audio_interval.tick() => {
                // Check for audio device errors
                if let Some(err) = capture.check_error() {
                    let _ = event_tx.send(VoiceEvent::AudioError(err));
                    let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
                    break;
                }
                if let Some(err) = mixer.check_error() {
                    let _ = event_tx.send(VoiceEvent::AudioError(err));
                    let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
                    break;
                }

                // If transmitting, capture and send audio
                if transmitting && capture.is_active()
                    && let Some(samples) = capture.take_frame()
                    && let Ok(encoded) = encoder.encode(&samples)
                {
                    let _ = dtls_command_tx.send(VoiceDtlsCommand::SendVoice(encoded));
                }

                // Process jitter buffers and play audio
                // Iterate directly over buffers to avoid Vec allocation
                for (sender, buffer) in jitter_pool.iter_mut() {
                    // Skip muted users
                    if muted_users.contains(sender) {
                        continue;
                    }

                    // Check for packet loss and use PLC
                    if buffer.has_loss() {
                        if let Ok(samples) = decoder_pool.decode_lost(sender) {
                            mixer.queue_audio(sender, &samples);
                        }
                        // Pop to advance the jitter buffer
                        let _ = buffer.pop();
                    } else if let Some(samples) = buffer.pop() {
                        mixer.queue_audio(sender, &samples);
                    }
                }
            }

            // Handle DTLS events
            event = dtls_event_rx.recv() => {
                match event {
                    Some(VoiceDtlsEvent::VoiceReceived { sender, sequence, timestamp, payload }) => {
                        // Decode and buffer the audio
                        if let Ok(samples) = decoder_pool.decode(&sender, &payload) {
                            jitter_pool.push(&sender, sequence, timestamp, samples);
                        }
                    }
                    Some(VoiceDtlsEvent::SpeakingStarted { sender }) => {
                        let _ = event_tx.send(VoiceEvent::SpeakingStarted(sender));
                    }
                    Some(VoiceDtlsEvent::SpeakingStopped { sender }) => {
                        let _ = event_tx.send(VoiceEvent::SpeakingStopped(sender));
                    }
                    Some(VoiceDtlsEvent::Error(e)) => {
                        let _ = event_tx.send(VoiceEvent::Disconnected(Some(e)));
                        break;
                    }
                    Some(VoiceDtlsEvent::Disconnected) => {
                        let _ = event_tx.send(VoiceEvent::Disconnected(None));
                        break;
                    }
                    Some(VoiceDtlsEvent::Connected) => {
                        // Already handled above
                    }
                    None => {
                        let _ = event_tx.send(VoiceEvent::Disconnected(None));
                        break;
                    }
                }
            }

            // Handle commands
            cmd = command_rx.recv() => {
                match cmd {
                    Some(VoiceCommand::StartTransmitting) => {
                        if !transmitting {
                            transmitting = true;
                            if let Err(e) = capture.start() {
                                let _ = event_tx.send(VoiceEvent::AudioError(format!("Capture error: {}", e)));
                            } else {
                                let _ = dtls_command_tx.send(VoiceDtlsCommand::SendSpeakingStarted);
                                let _ = event_tx.send(VoiceEvent::LocalSpeakingChanged(true));
                            }
                        }
                    }
                    Some(VoiceCommand::StopTransmitting) => {
                        if transmitting {
                            transmitting = false;
                            capture.stop();
                            let _ = dtls_command_tx.send(VoiceDtlsCommand::SendSpeakingStopped);
                            let _ = event_tx.send(VoiceEvent::LocalSpeakingChanged(false));
                        }
                    }
                    Some(VoiceCommand::MuteUser(nickname)) => {
                        let key = nickname.to_lowercase();
                        muted_users.insert(key.clone());
                        mixer.mute_user(&nickname);
                        // Clear their jitter buffer
                        jitter_pool.remove(&nickname);
                        decoder_pool.remove(&nickname);
                    }
                    Some(VoiceCommand::UnmuteUser(nickname)) => {
                        let key = nickname.to_lowercase();
                        muted_users.remove(&key);
                        mixer.unmute_user(&nickname);
                    }
                    Some(VoiceCommand::SetDeafened(deafened)) => {
                        mixer.set_deafened(deafened);
                    }
                    Some(VoiceCommand::SetQuality(quality)) => {
                        if let Err(e) = encoder.set_quality(quality) {
                            eprintln!("Failed to update voice quality: {}", e);
                        }
                    }
                    Some(VoiceCommand::Stop) | None => {
                        // Clean shutdown
                        if transmitting {
                            capture.stop();
                            let _ = dtls_command_tx.send(VoiceDtlsCommand::SendSpeakingStopped);
                        }
                        let _ = dtls_command_tx.send(VoiceDtlsCommand::Disconnect);
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    mixer.stop();
    dtls_handle.abort();
    let _ = event_tx.send(VoiceEvent::Disconnected(None));
}

// =============================================================================
// Voice Session Handle
// =============================================================================

/// Handle for controlling an active voice session
pub struct VoiceSessionHandle {
    /// Command sender
    command_tx: mpsc::UnboundedSender<VoiceCommand>,
    /// Join handle for the session thread
    /// Using std::thread instead of tokio::spawn because cpal's Stream is not Send
    handle: Option<JoinHandle<()>>,
}

impl VoiceSessionHandle {
    /// Start a new voice session
    ///
    /// Returns a handle for controlling the session and a receiver for events.
    ///
    /// Note: This spawns a dedicated OS thread because cpal's audio streams
    /// are not Send-safe and cannot be used across async task boundaries.
    pub fn start(
        server_addr: SocketAddr,
        token: Uuid,
        input_device: String,
        output_device: String,
        quality: VoiceQuality,
    ) -> (Self, mpsc::UnboundedReceiver<VoiceEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // Spawn on a dedicated thread because cpal's Stream is not Send
        // The thread runs its own tokio runtime for async operations
        let handle = std::thread::spawn(move || {
            // Create a new tokio runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime for voice thread");

            rt.block_on(run_voice_session(
                server_addr,
                token,
                input_device,
                output_device,
                quality,
                event_tx,
                command_rx,
            ));
        });

        (
            Self {
                command_tx,
                handle: Some(handle),
            },
            event_rx,
        )
    }

    /// Start transmitting (PTT pressed)
    pub fn start_transmitting(&self) {
        let _ = self.command_tx.send(VoiceCommand::StartTransmitting);
    }

    /// Stop transmitting (PTT released)
    pub fn stop_transmitting(&self) {
        let _ = self.command_tx.send(VoiceCommand::StopTransmitting);
    }

    /// Mute a user
    pub fn mute_user(&self, nickname: &str) {
        let _ = self
            .command_tx
            .send(VoiceCommand::MuteUser(nickname.to_string()));
    }

    /// Unmute a user
    pub fn unmute_user(&self, nickname: &str) {
        let _ = self
            .command_tx
            .send(VoiceCommand::UnmuteUser(nickname.to_string()));
    }

    /// Set deafened state (mute all incoming audio)
    pub fn set_deafened(&self, deafened: bool) {
        let _ = self.command_tx.send(VoiceCommand::SetDeafened(deafened));
    }

    /// Update voice quality (bitrate) dynamically
    ///
    /// Can be called while in a voice session to change quality without
    /// needing to leave and rejoin.
    pub fn set_quality(&self, quality: VoiceQuality) {
        let _ = self.command_tx.send(VoiceCommand::SetQuality(quality));
    }

    /// Stop the voice session and wait for cleanup
    ///
    /// Sends the stop command and waits for the voice thread to finish.
    /// This ensures clean shutdown of audio devices and DTLS connection.
    pub fn stop(&mut self) {
        let _ = self.command_tx.send(VoiceCommand::Stop);

        // Wait for the thread to finish (with a timeout to avoid blocking forever)
        if let Some(handle) = self.handle.take() {
            // Give the thread a reasonable time to clean up
            let _ = handle.join();
        }
    }
}

impl Drop for VoiceSessionHandle {
    fn drop(&mut self) {
        // Ensure the voice thread is stopped when the handle is dropped
        // This prevents orphaned threads if stop() wasn't called explicitly
        if self.handle.is_some() {
            self.stop();
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
    fn test_voice_event_variants() {
        // Verify enum variants compile
        let _ = VoiceEvent::Connected;
        let _ = VoiceEvent::ConnectionFailed("test".to_string());
        let _ = VoiceEvent::Disconnected(Some("test".to_string()));
        let _ = VoiceEvent::SpeakingStarted("Alice".to_string());
        let _ = VoiceEvent::SpeakingStopped("Alice".to_string());
        let _ = VoiceEvent::AudioError("test".to_string());
        let _ = VoiceEvent::LocalSpeakingChanged(true);
    }

    #[test]
    fn test_voice_command_variants() {
        // Verify enum variants compile
        let _ = VoiceCommand::StartTransmitting;
        let _ = VoiceCommand::StopTransmitting;
        let _ = VoiceCommand::MuteUser("Alice".to_string());
        let _ = VoiceCommand::UnmuteUser("Alice".to_string());
        let _ = VoiceCommand::Stop;
    }
}
