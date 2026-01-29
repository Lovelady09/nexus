//! Voice event subscription
//!
//! Provides a subscription for receiving voice events from an active
//! voice session (connected, speaking started/stopped, errors, etc.).
//!
//! Uses a global registry pattern similar to network streams to handle
//! the non-Clone nature of mpsc receivers.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};

use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use iced::stream;
use once_cell::sync::Lazy;
use tokio::sync::mpsc;

use super::manager::VoiceEvent;
use crate::types::Message;

// =============================================================================
// Constants
// =============================================================================

/// Channel size for the stream
const STREAM_CHANNEL_SIZE: usize = 20;

// =============================================================================
// Global Registry
// =============================================================================

/// Type alias for the voice event registry
/// Maps connection_id to the voice event receiver
/// Uses std::sync::Mutex for synchronous access from non-async context
type VoiceEventRegistry = Arc<StdMutex<HashMap<usize, mpsc::UnboundedReceiver<VoiceEvent>>>>;

/// Global registry for voice event receivers
static VOICE_EVENT_RECEIVERS: Lazy<VoiceEventRegistry> =
    Lazy::new(|| Arc::new(StdMutex::new(HashMap::new())));

/// Register a voice event receiver in the global registry (synchronous)
///
/// This should be called when starting a voice session.
/// Uses synchronous mutex to avoid race conditions with the subscription.
pub fn register_voice_receiver_sync(
    connection_id: usize,
    receiver: mpsc::UnboundedReceiver<VoiceEvent>,
) {
    if let Ok(mut registry) = VOICE_EVENT_RECEIVERS.lock() {
        registry.insert(connection_id, receiver);
    }
}

/// Remove a voice event receiver from the global registry (synchronous)
///
/// This should be called when a voice session ends.
pub fn unregister_voice_receiver_sync(connection_id: usize) {
    if let Ok(mut registry) = VOICE_EVENT_RECEIVERS.lock() {
        registry.remove(&connection_id);
    }
}

// =============================================================================
// Subscription
// =============================================================================

/// Create a subscription for voice session events
///
/// This subscription receives events from an active voice session and emits
/// `Message::VoiceSessionEvent` messages to update the UI.
///
/// # Arguments
/// * `connection_id` - The connection ID this voice session belongs to
pub fn voice_event_subscription(connection_id: usize) -> Subscription<Message> {
    Subscription::run_with(connection_id, voice_event_stream)
}

/// Stream that receives voice events and converts them to messages
///
/// Takes a reference to the connection_id for compatibility with
/// Subscription::run_with. Returns a boxed stream.
pub fn voice_event_stream(connection_id: &usize) -> Pin<Box<dyn Stream<Item = Message> + Send>> {
    let connection_id = *connection_id;

    Box::pin(stream::channel(
        STREAM_CHANNEL_SIZE,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            // Get the receiver from the registry (synchronous access)
            let event_rx = VOICE_EVENT_RECEIVERS
                .lock()
                .ok()
                .and_then(|mut registry| registry.remove(&connection_id));

            if let Some(mut rx) = event_rx {
                // Receive events and forward them to Iced
                while let Some(event) = rx.recv().await {
                    if output
                        .send(Message::VoiceSessionEvent(connection_id, event))
                        .await
                        .is_err()
                    {
                        // Channel closed, stop
                        break;
                    }
                }
            }

            // Stream ends when event_rx is exhausted (session ended) or not found
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
    fn test_stream_channel_size() {
        // Should be large enough to buffer bursts of events
        let size = STREAM_CHANNEL_SIZE;
        assert!(
            size >= 10,
            "Channel size should be at least 10 to buffer event bursts"
        );
    }
}
