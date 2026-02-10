//! Linux desktop notifications with click-to-navigate support
//!
//! Uses notify-rust with D-Bus actions to handle notification clicks.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use notify_rust::{Notification, NotificationHandle};

use crate::constants::APP_NAME;

/// How long to keep notification handles alive (slightly longer than the notification timeout)
const HANDLE_LIFETIME: Duration = Duration::from_secs(6);

/// Keep notification handles alive to prevent GNOME/Cinnamon from dismissing them.
/// These desktop environments close notifications when the D-Bus connection drops,
/// so we hold onto handles until they expire naturally.
/// See: https://gitlab.gnome.org/GNOME/gnome-shell/-/issues/8797
static NOTIFICATION_HANDLES: Mutex<Vec<(Instant, NotificationHandle)>> = Mutex::new(Vec::new());

/// Show a notification with optional click-to-navigate URI
pub fn show(summary: &str, body: Option<&str>, uri: Option<String>) {
    let mut notification = Notification::new();
    notification
        .appname(APP_NAME)
        .summary(summary)
        .body(body.unwrap_or(""))
        .auto_icon()
        .timeout(notify_rust::Timeout::Milliseconds(5000));

    // Add a default action for clicking the notification body
    if uri.is_some() {
        notification.action("default", "Open");
    }

    match notification.show() {
        Ok(handle) => {
            // Always clean up expired handles to prevent unbounded growth
            if let Ok(mut handles) = NOTIFICATION_HANDLES.lock() {
                let now = Instant::now();
                handles.retain(|(created, _)| now.duration_since(*created) < HANDLE_LIFETIME);
            }

            if let Some(uri) = uri {
                // Spawn a short-lived thread to wait for the action callback.
                // This blocks until the notification is dismissed or clicked (bounded by
                // the 5-second notification timeout). The handle is moved into the thread
                // which keeps the D-Bus connection alive for the notification's lifetime.
                std::thread::spawn(move || {
                    handle.wait_for_action(|action| {
                        if action == "default" {
                            let _ = open::that(&uri);
                        }
                    });
                });
            } else {
                // No URI - just keep handle alive to prevent early dismissal on GNOME/Cinnamon
                if let Ok(mut handles) = NOTIFICATION_HANDLES.lock() {
                    handles.push((Instant::now(), handle));
                }
            }
        }
        Err(_) => {
            // Notification failed to show - nothing we can do
        }
    }
}
