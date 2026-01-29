//! Voice chat UI handlers
//!
//! Handles user interactions with voice chat controls:
//! - VoiceJoinPressed - User clicks to join voice for a channel or user message
//! - VoiceLeavePressed - User clicks to leave current voice session
//! - VoiceSessionEvent - Events from the voice session (connected, speaking, etc.)
//! - VoicePttStateChanged - PTT hotkey pressed/released
//! - VoiceUserMute/VoiceUserUnmute - Mute/unmute a user (client-side)

use global_hotkey::GlobalHotKeyEvent;
use iced::Task;
use nexus_common::protocol::ClientMessage;

use crate::NexusApp;
use crate::i18n::{t, t_args};
use crate::types::{ChatMessage, ChatTab, Message, VoiceState};
use crate::views::constants::{PERMISSION_VOICE_LISTEN, PERMISSION_VOICE_TALK};
use crate::voice::manager::VoiceEvent;
use crate::voice::ptt::PttState;

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
        // We create a placeholder session that will be replaced on success
        conn.voice_session = Some(VoiceState::new(target.clone(), Vec::new()));

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

    /// Check if the current tab matches the active voice session target
    #[allow(dead_code)] // Available for UI state checks
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

    /// Handle voice session events from the DTLS client
    ///
    /// These events come from the voice session thread and update the UI
    /// with connection status, speaking indicators, and errors.
    pub fn handle_voice_session_event(
        &mut self,
        connection_id: usize,
        event: VoiceEvent,
    ) -> Task<Message> {
        match event {
            VoiceEvent::Connected => {
                // DTLS connection established - voice is now active
                // The voice bar already shows, so no additional feedback needed
                Task::none()
            }

            VoiceEvent::ConnectionFailed(error) => {
                // DTLS connection failed - clean up and show error
                self.cleanup_voice_session(connection_id);
                self.add_active_tab_message(
                    connection_id,
                    ChatMessage::error(t_args("err-voice-connection-failed", &[("error", &error)])),
                )
            }

            VoiceEvent::Disconnected(reason) => {
                // DTLS connection lost - clean up and optionally show reason
                self.cleanup_voice_session(connection_id);
                if let Some(reason) = reason {
                    self.add_active_tab_message(
                        connection_id,
                        ChatMessage::error(t_args(
                            "err-voice-disconnected",
                            &[("reason", &reason)],
                        )),
                    )
                } else {
                    Task::none()
                }
            }

            VoiceEvent::SpeakingStarted(nickname) => {
                // A user started speaking - update speaking set
                if let Some(conn) = self.connections.get_mut(&connection_id)
                    && let Some(ref mut session) = conn.voice_session
                {
                    session.set_speaking(&nickname);
                }
                Task::none()
            }

            VoiceEvent::SpeakingStopped(nickname) => {
                // A user stopped speaking - update speaking set
                if let Some(conn) = self.connections.get_mut(&connection_id)
                    && let Some(ref mut session) = conn.voice_session
                {
                    session.set_not_speaking(&nickname);
                }
                Task::none()
            }

            VoiceEvent::AudioError(error) => {
                // Audio device error - clean up and show error
                self.cleanup_voice_session(connection_id);
                self.add_active_tab_message(
                    connection_id,
                    ChatMessage::error(t_args("err-voice-audio", &[("error", &error)])),
                )
            }

            VoiceEvent::LocalSpeakingChanged(speaking) => {
                // Local user started/stopped speaking - update PTT indicator
                self.is_local_speaking = speaking;
                Task::none()
            }
        }
    }

    /// Clean up voice session state when disconnecting or on error
    pub fn cleanup_voice_session(&mut self, connection_id: usize) {
        // Clear voice session from connection
        if let Some(conn) = self.connections.get_mut(&connection_id) {
            conn.voice_session = None;
        }

        // Clear active voice connection if it was this one
        if self.active_voice_connection == Some(connection_id) {
            self.active_voice_connection = None;

            // Stop the voice session handle
            if let Some(mut handle) = self.voice_session_handle.take() {
                handle.stop();
            }

            // Clean up the voice event receiver from the registry
            crate::voice::subscription::unregister_voice_receiver_sync(connection_id);

            // Stop PTT and unregister hotkey
            if let Some(ref mut ptt) = self.ptt_manager {
                ptt.set_in_voice(false);
                ptt.unregister_hotkey();
            }

            // Clear local speaking and deafened state
            self.is_local_speaking = false;
            self.is_deafened = false;
        }
    }

    /// Handle PTT state changed (from global hotkey event)
    ///
    /// Called when the PTT hotkey is pressed or released. Starts or stops
    /// audio transmission based on the new state.
    pub fn handle_voice_ptt_state_changed(&mut self, state: PttState) -> Task<Message> {
        // Only act if we have an active voice session
        let Some(ref handle) = self.voice_session_handle else {
            return Task::none();
        };

        match state {
            PttState::Transmitting => {
                handle.start_transmitting();
            }
            PttState::Idle => {
                handle.stop_transmitting();
            }
        }

        Task::none()
    }

    /// Handle voice user mute (client-side)
    ///
    /// Mutes a user so the local client no longer hears their audio.
    /// This is purely client-side and doesn't affect other users.
    pub fn handle_voice_user_mute(&mut self, nickname: String) -> Task<Message> {
        let Some(connection_id) = self.active_voice_connection else {
            return Task::none();
        };

        // Update local session state
        if let Some(conn) = self.connections.get_mut(&connection_id)
            && let Some(ref mut session) = conn.voice_session
        {
            session.mute_user(&nickname);
        }

        // Tell the voice manager to stop playing this user's audio
        if let Some(ref handle) = self.voice_session_handle {
            handle.mute_user(&nickname);
        }

        Task::none()
    }

    /// Handle voice user unmute (client-side)
    ///
    /// Unmutes a previously muted user so the local client can hear them again.
    pub fn handle_voice_user_unmute(&mut self, nickname: String) -> Task<Message> {
        let Some(connection_id) = self.active_voice_connection else {
            return Task::none();
        };

        // Update local session state
        if let Some(conn) = self.connections.get_mut(&connection_id)
            && let Some(ref mut session) = conn.voice_session
        {
            session.unmute_user(&nickname);
        }

        // Tell the voice manager to resume playing this user's audio
        if let Some(ref handle) = self.voice_session_handle {
            handle.unmute_user(&nickname);
        }

        Task::none()
    }

    /// Handle voice deafen toggle
    ///
    /// Toggles the deafened state - when deafened, all incoming voice audio is muted.
    /// The user remains in voice and can still transmit if they use PTT.
    pub fn handle_voice_deafen_toggle(&mut self) -> Task<Message> {
        // Toggle deafened state
        self.is_deafened = !self.is_deafened;

        // Tell the voice manager to update audio output
        if let Some(ref handle) = self.voice_session_handle {
            handle.set_deafened(self.is_deafened);
        }

        Task::none()
    }

    /// Handle raw PTT hotkey event from global hotkey subscription
    ///
    /// Forwards the event to the PttManager to determine if it's our hotkey
    /// and what state change (if any) occurred.
    pub fn handle_voice_ptt_event(&mut self, event: GlobalHotKeyEvent) -> Task<Message> {
        // Only process if we have a PTT manager
        let Some(ref mut ptt) = self.ptt_manager else {
            return Task::none();
        };

        // Let the PTT manager handle the event
        if let Some(state) = ptt.handle_event(event) {
            // State changed, forward to the state change handler
            return self.handle_voice_ptt_state_changed(state);
        }

        Task::none()
    }
}
