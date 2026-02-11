//! Windows desktop notifications with click-to-navigate support
//!
//! Uses tauri-winrt-notification for Windows Toast notifications with activation callbacks.

use tauri_winrt_notification::{Result, Toast};

/// Show a notification with optional click-to-navigate URI
pub fn show(summary: &str, body: Option<&str>, uri: Option<String>) {
    // Use PowerShell's App ID as a fallback since we don't have a registered AUMID.
    // This is the standard approach for apps without Windows Store registration.
    let mut toast = Toast::new(Toast::POWERSHELL_APP_ID);
    toast = toast.title(summary);

    if let Some(body) = body {
        toast = toast.text1(body);
    }

    // Set up click handler if URI is provided
    if let Some(uri) = uri {
        toast = toast.on_activated(move |_| -> Result<()> {
            let _ = open::that(&uri);
            Ok(())
        });
    }

    let _ = toast.show();
}
