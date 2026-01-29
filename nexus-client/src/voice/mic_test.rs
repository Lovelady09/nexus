//! Microphone test functionality
//!
//! Provides a subscription for monitoring microphone input levels
//! in the Settings > Audio tab.
//!
//! Note: cpal's audio streams are not Send-safe, so mic testing runs
//! on a dedicated OS thread that sends level updates through a channel.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use iced::stream;
use tokio::sync::mpsc;

use super::audio::AudioCapture;
use crate::types::Message;

// =============================================================================
// Constants
// =============================================================================

/// How often to update the mic level display (60 fps for smooth meter)
const MIC_LEVEL_UPDATE_INTERVAL_MS: u64 = 16;

/// Channel size for the stream
const STREAM_CHANNEL_SIZE: usize = 10;

// =============================================================================
// Mic Test Thread
// =============================================================================

/// Run mic test on a dedicated thread
///
/// This function runs the audio capture on a separate OS thread because
/// cpal's Stream type is not Send-safe.
fn run_mic_test_thread(
    input_device: String,
    level_tx: mpsc::UnboundedSender<f32>,
    running: Arc<AtomicBool>,
) {
    // Create audio capture
    let capture = match AudioCapture::new(&input_device) {
        Ok(c) => c,
        Err(_) => {
            // Send 0 level to indicate error
            let _ = level_tx.send(0.0);
            return;
        }
    };

    // Start capturing
    if capture.start().is_err() {
        let _ = level_tx.send(0.0);
        return;
    }

    // Poll at regular intervals
    let interval = Duration::from_millis(MIC_LEVEL_UPDATE_INTERVAL_MS);

    while running.load(Ordering::SeqCst) {
        // Check if capture is still active
        if !capture.is_active() {
            break;
        }

        // Get current input level
        let level = capture.get_input_level();

        // Send level update
        if level_tx.send(level).is_err() {
            // Receiver dropped, stop the test
            break;
        }

        // Sleep until next update
        thread::sleep(interval);
    }

    // Capture stops automatically when dropped
}

// =============================================================================
// Subscription
// =============================================================================

/// Create a subscription for mic test level updates
///
/// This subscription captures audio from the microphone and emits
/// `Message::AudioMicLevel` messages with the current input level.
pub fn mic_test_subscription(input_device: String) -> Subscription<Message> {
    Subscription::run_with(input_device, mic_test_stream)
}

/// Stream that monitors microphone input level
///
/// Takes a reference to input_device for compatibility with Subscription::run_with.
/// Returns a boxed stream to allow use as a function pointer.
#[allow(clippy::ptr_arg)] // Required for Subscription::run_with function pointer signature
pub fn mic_test_stream(input_device: &String) -> Pin<Box<dyn Stream<Item = Message> + Send>> {
    let input_device = input_device.clone();
    Box::pin(stream::channel(
        STREAM_CHANNEL_SIZE,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            // Create channel for level updates from the audio thread
            let (level_tx, mut level_rx) = mpsc::unbounded_channel::<f32>();

            // Flag to signal the thread to stop
            let running = Arc::new(AtomicBool::new(true));
            let running_clone = running.clone();

            // Spawn dedicated thread for audio capture
            let _handle: JoinHandle<()> = thread::spawn(move || {
                run_mic_test_thread(input_device, level_tx, running_clone);
            });

            // Receive level updates and forward to Iced
            while let Some(level) = level_rx.recv().await {
                if output.send(Message::AudioMicLevel(level)).await.is_err() {
                    // Channel closed, signal thread to stop
                    running.store(false, Ordering::SeqCst);
                    break;
                }
            }

            // Signal thread to stop (in case loop ended due to level_rx closing)
            running.store(false, Ordering::SeqCst);

            // Thread will stop on its own when running flag is false
            // or when level_tx is dropped (which happens when thread exits)
        },
    ))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mic_level_update_interval() {
        // 60 fps = ~16ms per frame
        // Verify the constant is in a reasonable range
        let interval = MIC_LEVEL_UPDATE_INTERVAL_MS;
        assert!(
            interval <= 20,
            "Update interval should be <= 20ms for smooth animation"
        );
        assert!(
            interval >= 10,
            "Update interval should be >= 10ms to avoid CPU overhead"
        );
    }
}
