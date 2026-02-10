//! Platform-specific desktop notifications with click-to-navigate support
//!
//! This module provides a unified interface for showing desktop notifications
//! that can navigate to relevant content when clicked.

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

use crate::constants::APP_NAME;

/// Show a desktop notification with optional click-to-navigate URI
///
/// When the user clicks the notification, it will open the provided URI,
/// which triggers the app's deep link handler to navigate to the content.
///
/// # Arguments
/// * `summary` - The notification title
/// * `body` - Optional notification body text
/// * `uri` - Optional nexus:// URI to open when clicked
#[allow(unused_variables)]
pub fn show(summary: &str, body: Option<&str>, uri: Option<String>) {
    #[cfg(all(unix, not(target_os = "macos")))]
    linux::show(summary, body, uri);

    #[cfg(target_os = "windows")]
    windows::show(summary, body, uri);

    // macOS: Fall back to basic notification without click handling
    #[cfg(target_os = "macos")]
    show_basic(summary, body);
}

/// Basic notification without click handling (fallback for macOS)
#[cfg(target_os = "macos")]
fn show_basic(summary: &str, body: Option<&str>) {
    use notify_rust::Notification;

    let _ = Notification::new()
        .appname(APP_NAME)
        .summary(summary)
        .body(body.unwrap_or(""))
        .auto_icon()
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}
