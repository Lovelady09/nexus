//! Voice chat UI handlers
//!
//! Handles user interactions with voice chat controls:
//! - VoiceJoinPressed - User clicks to join voice for a channel or user message
//! - VoiceLeavePressed - User clicks to leave current voice session

use iced::Task;
use nexus_common::protocol::ClientMessage;

use crate::NexusApp;
use crate::i18n::t;
use crate::types::{ChatMessage, ChatTab, Message, VoiceSession};
use crate::views::constants::{PERMISSION_VOICE_LISTEN, PERMISSION_VOICE_TALK};

impl NexusApp {
    /// Handle voice join button pressed
    ///
    /// Sends a VoiceJoin request to the server for the specified target.
    /// The target is a channel name (e.g., "#general") or nickname for user message voice.
    pub fn handle_voice_join_pressed(&mut self, target: String) -> Task<Message> {
        let Some(connection_id) = self.active_connection else {
            return Task::none();
        };

        // Check if we already have a voice session on another connection
        if let Some(active_voice_conn) = self.active_voice_connection
            && active_voice_conn != connection_id
        {
            // Show error - can only have one voice session at a time
            return self.add_active_tab_message(
                connection_id,
                ChatMessage::error(t("err-voice-already-active")),
            );
        }

        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        // Check permissions - need at least voice_listen to join
        if !conn.has_permission(PERMISSION_VOICE_LISTEN)
            && !conn.has_permission(PERMISSION_VOICE_TALK)
        {
            return self.add_active_tab_message(
                connection_id,
                ChatMessage::error(t("err-voice-no-permission")),
            );
        }

        // For channels, check if we're a member
        // Note: conn.channels is keyed by lowercase channel name WITH the # prefix
        if target.starts_with('#') {
            let channel_lower = target.to_lowercase();
            if !conn.channels.contains_key(&channel_lower) {
                return self.add_active_tab_message(
                    connection_id,
                    ChatMessage::error(t("err-voice-not-in-channel")),
                );
            }
        }

        // Store the target in a pending voice session so we can use it in the response
        // We create a placeholder session that will be updated with the real token on success
        conn.voice_session = Some(VoiceSession::new(
            target.clone(),
            uuid::Uuid::nil(), // Placeholder, will be replaced on success
            Vec::new(),
        ));

        // Send the VoiceJoin request
        if let Err(e) = conn.send(ClientMessage::VoiceJoin { target }) {
            // Clear the pending session on send failure
            conn.voice_session = None;
            return self.add_active_tab_message(connection_id, ChatMessage::error(e));
        }

        Task::none()
    }

    /// Handle voice leave button pressed
    ///
    /// Sends a VoiceLeave request to the server to leave the current voice session.
    pub fn handle_voice_leave_pressed(&mut self) -> Task<Message> {
        let Some(connection_id) = self.active_connection else {
            return Task::none();
        };

        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        // Check if we're actually in a voice session
        if conn.voice_session.is_none() {
            return self.add_active_tab_message(
                connection_id,
                ChatMessage::error(t("err-voice-not-in-session")),
            );
        }

        // Send the VoiceLeave request
        if let Err(e) = conn.send(ClientMessage::VoiceLeave) {
            return self.add_active_tab_message(connection_id, ChatMessage::error(e));
        }

        Task::none()
    }

    /// Get the voice target for the current chat tab
    ///
    /// Returns the appropriate voice target based on the active chat tab:
    /// - Channel tab: Returns the channel name (e.g., "#general")
    /// - UserMessage tab: Returns the other user's nickname
    /// - Console tab: Returns None (can't join voice from console)
    pub fn get_voice_target_for_current_tab(&self) -> Option<String> {
        let connection_id = self.active_connection?;
        let conn = self.connections.get(&connection_id)?;

        match &conn.active_chat_tab {
            // Channel name already includes the # prefix
            ChatTab::Channel(channel) => Some(channel.clone()),
            ChatTab::UserMessage(nickname) => Some(nickname.clone()),
            ChatTab::Console => None,
        }
    }

    /// Check if the current connection has an active voice session
    #[allow(dead_code)] // Used in Phase 2 for UI state
    pub fn is_in_voice(&self) -> bool {
        let Some(connection_id) = self.active_connection else {
            return false;
        };

        self.connections
            .get(&connection_id)
            .map(|conn| conn.voice_session.is_some())
            .unwrap_or(false)
    }

    /// Check if the current tab matches the active voice session target
    #[allow(dead_code)] // Used in Phase 2 for UI state
    pub fn is_voice_target_current_tab(&self) -> bool {
        let Some(connection_id) = self.active_connection else {
            return false;
        };

        let Some(conn) = self.connections.get(&connection_id) else {
            return false;
        };

        let Some(ref session) = conn.voice_session else {
            return false;
        };

        match &conn.active_chat_tab {
            // Channel name already includes the # prefix
            ChatTab::Channel(channel) => session.target.to_lowercase() == channel.to_lowercase(),
            ChatTab::UserMessage(nickname) => {
                session.target.to_lowercase() == nickname.to_lowercase()
            }
            ChatTab::Console => false,
        }
    }
}
