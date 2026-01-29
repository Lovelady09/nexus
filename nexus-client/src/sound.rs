//! Sound playback for event notifications
//!
//! Uses a persistent audio thread with a queue to ensure all sounds play reliably,
//! even when multiple sounds are requested simultaneously. Supports user-selected
//! output device from audio settings.

use std::io::Cursor;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

use once_cell::sync::Lazy;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, Sink};

use crate::config::events::SoundChoice;

// =============================================================================
// Embedded Sounds
// =============================================================================

/// Alert sound - synth notification (CC0: Freesound #651629 "Notify" by Martcraft)
const SOUND_ALERT: &[u8] = include_bytes!("../sounds/alert.ogg");
/// Bell sound - UI approval chime (CC0: Freesound #625174 "UI Sound Approval" by GabFitzgerald)
const SOUND_BELL: &[u8] = include_bytes!("../sounds/bell.ogg");
/// Chime sound - xylophone tone (CC0: Freesound #536748 "Phone" by egomassive)
const SOUND_CHIME: &[u8] = include_bytes!("../sounds/chime.ogg");
/// Ding sound - hand bell (CC0: Freesound #804740 "Bell Hand Ding" by DesignersChoice)
const SOUND_DING: &[u8] = include_bytes!("../sounds/ding.ogg");
/// Pop sound - short blip (CC0: Freesound #757175 "Blip 1" by Henri Kähkönen)
const SOUND_POP: &[u8] = include_bytes!("../sounds/pop.ogg");

// =============================================================================
// Audio Thread
// =============================================================================

/// Request to play a sound
struct SoundRequest {
    /// Sound data to play
    data: &'static [u8],
    /// Volume level (0.0 - 1.0)
    volume: f32,
    /// Output device name (empty string = system default)
    device_name: String,
}

/// State for the sound system
struct SoundState {
    /// Sender to the audio thread
    sender: Sender<SoundRequest>,
}

/// Global sound state
static SOUND_STATE: Lazy<Mutex<Option<SoundState>>> = Lazy::new(|| Mutex::new(None));

/// Initialize the audio thread if not already running
fn ensure_audio_thread() -> bool {
    let mut state = SOUND_STATE.lock().unwrap();

    if state.is_some() {
        return true;
    }

    let (tx, rx) = mpsc::channel::<SoundRequest>();

    // Spawn the persistent audio thread
    std::thread::spawn(move || {
        run_audio_thread(rx);
    });

    *state = Some(SoundState { sender: tx });
    true
}

/// Run the audio thread - handles device switching and sound playback
fn run_audio_thread(rx: mpsc::Receiver<SoundRequest>) {
    // Current output stream and its device name
    let mut current_device_name: String = String::new();
    let mut current_stream: Option<(OutputStream, rodio::OutputStreamHandle)> = None;

    // Keep track of active sinks so they stay alive until finished
    let mut active_sinks: Vec<Sink> = Vec::new();

    // Process sound requests forever
    for request in rx {
        // Clean up finished sinks
        active_sinks.retain(|sink| !sink.empty());

        // Check if we need to switch devices
        if current_stream.is_none() || current_device_name != request.device_name {
            // Drop old stream first
            drop(current_stream.take());
            active_sinks.clear();

            // Get the new stream
            current_stream = get_output_stream(&request.device_name);
            current_device_name = request.device_name.clone();

            if current_stream.is_none() {
                // Can't get output stream, skip this sound
                continue;
            }
        }

        // Play the sound
        let Some((_, ref handle)) = current_stream else {
            continue;
        };

        // Create a new sink for this sound
        let Ok(sink) = Sink::try_new(handle) else {
            continue;
        };

        // Try to decode the audio data
        let Ok(source) = Decoder::new(Cursor::new(request.data)) else {
            continue;
        };

        // Set volume and play (non-blocking)
        sink.set_volume(request.volume);
        sink.append(source);

        // Keep sink alive until it finishes playing
        active_sinks.push(sink);
    }
}

/// Get an output stream for the specified device
///
/// If device_name is empty, uses the system default device.
fn get_output_stream(device_name: &str) -> Option<(OutputStream, rodio::OutputStreamHandle)> {
    if device_name.is_empty() {
        // Use system default
        OutputStream::try_default().ok()
    } else {
        // Find the specific device
        let host = rodio::cpal::default_host();
        let devices = host.output_devices().ok()?;

        for device in devices {
            if let Ok(name) = device.name()
                && name == device_name
            {
                return OutputStream::try_from_device(&device).ok();
            }
        }

        // Device not found, fall back to default
        OutputStream::try_default().ok()
    }
}

/// Get the sender for sound requests
fn get_sound_sender() -> Option<Sender<SoundRequest>> {
    if !ensure_audio_thread() {
        return None;
    }

    let state = SOUND_STATE.lock().unwrap();
    state.as_ref().map(|s| s.sender.clone())
}

// =============================================================================
// Public API
// =============================================================================

/// Play a sound at the given volume (0.0 - 1.0) on the specified output device
///
/// Sounds are queued and played by a persistent audio thread.
/// If the audio system is unavailable, the request is silently ignored.
///
/// # Arguments
/// * `sound` - Which sound to play
/// * `volume` - Volume level (0.0 - 1.0)
/// * `device_name` - Output device name, or empty string for system default
pub fn play_sound_on_device(sound: &SoundChoice, volume: f32, device_name: &str) {
    let data = get_sound_data(sound);

    if let Some(sender) = get_sound_sender() {
        // Send is non-blocking - if channel is full or disconnected, we just ignore
        let _ = sender.send(SoundRequest {
            data,
            volume,
            device_name: device_name.to_string(),
        });
    }
}

/// Play a sound at the given volume (0.0 - 1.0) on the system default device
///
/// This is a convenience wrapper around `play_sound_on_device` for cases
/// where device selection is not needed.
///
/// # Arguments
/// * `sound` - Which sound to play
/// * `volume` - Volume level (0.0 - 1.0)
#[allow(dead_code)]
pub fn play_sound(sound: &SoundChoice, volume: f32) {
    play_sound_on_device(sound, volume, "");
}

// =============================================================================
// Internal Helpers
// =============================================================================

/// Get the raw audio data for a sound choice
fn get_sound_data(sound: &SoundChoice) -> &'static [u8] {
    match sound {
        SoundChoice::Alert => SOUND_ALERT,
        SoundChoice::Bell => SOUND_BELL,
        SoundChoice::Chime => SOUND_CHIME,
        SoundChoice::Ding => SOUND_DING,
        SoundChoice::Pop => SOUND_POP,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_sound_data_alert() {
        let data = get_sound_data(&SoundChoice::Alert);
        // Verify it's a valid OGG file (starts with "OggS")
        assert!(data.len() > 4);
        assert_eq!(&data[0..4], b"OggS");
    }

    #[test]
    fn test_get_sound_data_bell() {
        let data = get_sound_data(&SoundChoice::Bell);
        assert_eq!(&data[0..4], b"OggS");
    }

    #[test]
    fn test_get_sound_data_chime() {
        let data = get_sound_data(&SoundChoice::Chime);
        assert_eq!(&data[0..4], b"OggS");
    }

    #[test]
    fn test_get_sound_data_ding() {
        let data = get_sound_data(&SoundChoice::Ding);
        assert_eq!(&data[0..4], b"OggS");
    }

    #[test]
    fn test_get_sound_data_pop() {
        let data = get_sound_data(&SoundChoice::Pop);
        assert_eq!(&data[0..4], b"OggS");
    }
}
