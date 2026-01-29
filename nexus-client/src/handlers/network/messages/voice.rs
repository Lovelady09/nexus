//! Voice chat message handlers
//!
//! Handles server messages for voice chat:
//! - VoiceJoinResponse - Response to VoiceJoin request
//! - VoiceLeaveResponse - Response to VoiceLeave request
//! - VoiceUserJoined - Notification when another user joins voice
//! - VoiceUserLeft - Notification when another user leaves voice

use std::net::ToSocketAddrs;

use iced::Task;
use uuid::Uuid;

use crate::NexusApp;
use crate::i18n::{t, t_args};
use crate::types::{ChatMessage, Message, VoiceState};
use crate::voice::manager::{VoiceSessionConfig, VoiceSessionHandle};
use crate::voice::ptt::PttManager;
use crate::voice::subscription::register_voice_receiver_sync;

impl NexusApp {
    /// Handle response to VoiceJoin request
    ///
    /// On success: Create voice session with token and participants
    /// On error: Show error in active tab
    pub fn handle_voice_join_response(
        &mut self,
        connection_id: usize,
        success: bool,
        token: Option<Uuid>,
        target: Option<String>,
        participants: Option<Vec<String>>,
        error: Option<String>,
    ) -> Task<Message> {
        if !success {
            // Clear the placeholder voice session on failure
            if let Some(conn) = self.connections.get_mut(&connection_id) {
                conn.voice_session = None;
            }

            let error_msg = error.unwrap_or_else(|| t("err-unknown"));
            return self.add_active_tab_message(
                connection_id,
                ChatMessage::error(t_args("err-voice-join", &[("error", &error_msg)])),
            );
        }

        let Some(token) = token else {
            // Clear the placeholder voice session - no token means failure
            if let Some(conn) = self.connections.get_mut(&connection_id) {
                conn.voice_session = None;
            }
            return self.add_active_tab_message(
                connection_id,
                ChatMessage::error(t("err-voice-no-token")),
            );
        };

        // Use target from server, fall back to placeholder target if not provided
        let target = match target {
            Some(t) => t,
            None => {
                let Some(conn) = self.connections.get(&connection_id) else {
                    return Task::none();
                };
                conn.voice_session
                    .as_ref()
                    .map(|s| s.target.clone())
                    .unwrap_or_default()
            }
        };

        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        // Create the voice session
        let participants = participants.unwrap_or_default();
        conn.voice_session = Some(VoiceState::new(target.clone(), participants));

        // Track that this connection has the active voice session
        self.active_voice_connection = Some(connection_id);

        // Start the voice DTLS client
        // Resolve server address to SocketAddr
        let server_addr = format!(
            "{}:{}",
            conn.connection_info.address, conn.connection_info.port
        );
        let socket_addr = match server_addr.to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(addr) => addr,
                None => {
                    return self.add_active_tab_message(
                        connection_id,
                        ChatMessage::error(t("err-voice-resolve-address")),
                    );
                }
            },
            Err(e) => {
                return self.add_active_tab_message(
                    connection_id,
                    ChatMessage::error(t_args("err-voice-resolve", &[("error", &e.to_string())])),
                );
            }
        };

        // Start voice session with audio settings
        let (handle, event_rx) = VoiceSessionHandle::start(VoiceSessionConfig {
            server_addr: socket_addr,
            token,
            input_device: self.config.settings.audio.input_device.clone(),
            output_device: self.config.settings.audio.output_device.clone(),
            quality: self.config.settings.audio.voice_quality,
            processor_settings: crate::voice::processor::AudioProcessorSettings {
                noise_suppression: self.config.settings.audio.noise_suppression,
                echo_cancellation: self.config.settings.audio.echo_cancellation,
                agc: self.config.settings.audio.agc,
            },
            ptt_mode: self.config.settings.audio.ptt_mode,
            mic_level: self.mic_level.clone(),
        });

        // Store the handle
        self.voice_session_handle = Some(handle);

        // Register the event receiver in the global registry for the subscription
        // Must be synchronous to avoid race with subscription starting
        register_voice_receiver_sync(connection_id, event_rx);

        // Initialize PTT manager if not already created
        // If PTT manager fails to initialize, voice still works but PTT won't function
        if self.ptt_manager.is_none()
            && let Ok(ptt) = PttManager::new()
        {
            self.ptt_manager = Some(ptt);
        }

        // Register PTT hotkey and enable it for voice
        if let Some(ref mut ptt) = self.ptt_manager {
            // Set mode from settings
            ptt.set_mode(self.config.settings.audio.ptt_mode);

            // Register the hotkey (silently ignore failures - PTT just won't work)
            let _ = ptt.register_hotkey(&self.config.settings.audio.ptt_key);

            // Enable PTT for voice
            ptt.set_in_voice(true);
        }

        // Voice bar appearing provides visual feedback - no console message needed
        Task::none()
    }

    /// Handle response to VoiceLeave request
    ///
    /// On success: Clear voice session
    /// On error: Show error in console (but still clear local state)
    pub fn handle_voice_leave_response(
        &mut self,
        connection_id: usize,
        success: bool,
        error: Option<String>,
    ) -> Task<Message> {
        // Clear local voice state regardless of success
        // (if server says we're not in voice, we should clear our state too)
        self.cleanup_voice_session(connection_id);

        if !success {
            let error_msg = error.unwrap_or_else(|| t("err-unknown"));
            return self.add_active_tab_message(
                connection_id,
                ChatMessage::error(t_args("err-voice-leave", &[("error", &error_msg)])),
            );
        }

        // Voice bar disappearing provides visual feedback - no console message needed
        Task::none()
    }

    /// Handle VoiceUserJoined - notification when another user joins voice
    ///
    /// Adds the user to our local participants list if we're in the same voice session.
    /// Also tracks voice users per channel for UI indicators (even when not in voice).
    /// Shows notification in the target tab (channel or user message) if join/leave events are enabled.
    pub fn handle_voice_user_joined(
        &mut self,
        connection_id: usize,
        nickname: String,
        target: String,
    ) -> Task<Message> {
        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        // Track voiced nicknames per channel (even when we're not in voice)
        // Use lowercase for consistency with ChatJoinResponse population
        if target.starts_with('#') {
            conn.channel_voiced
                .entry(target.to_lowercase())
                .or_default()
                .insert(nickname.to_lowercase());
        }

        // Update voice session participants if we're in the same session
        if let Some(ref mut session) = conn.voice_session
            && session.target.to_lowercase() == target.to_lowercase()
        {
            session.add_participant(nickname.clone());
        }

        // Show notification in target tab if events are enabled
        if self.config.settings.show_join_leave_events {
            let message =
                ChatMessage::system(t_args("msg-voice-user-joined", &[("nickname", &nickname)]));

            // Route to channel or user message tab based on target
            if target.starts_with('#') {
                return self.add_channel_message(connection_id, &target, message);
            } else {
                return self.add_user_message(connection_id, &target, message);
            }
        }

        Task::none()
    }

    /// Handle VoiceUserLeft - notification when a user leaves voice
    ///
    /// If the leaving user is us (kicked due to permission revocation), clears our voice session.
    /// Otherwise, removes the user from our local participants list.
    /// Also updates per-channel voice tracking for UI indicators.
    /// Shows notification in the target tab (channel or user message) if join/leave events are enabled.
    pub fn handle_voice_user_left(
        &mut self,
        connection_id: usize,
        nickname: String,
        target: String,
    ) -> Task<Message> {
        // Check if we're the one who left (kicked due to permission revocation)
        let is_self = self
            .connections
            .get(&connection_id)
            .map(|conn| conn.nickname.to_lowercase() == nickname.to_lowercase())
            .unwrap_or(false);

        if is_self {
            // We left voice - clean up fully
            self.cleanup_voice_session(connection_id);

            // Also remove ourselves from channel voice tracking
            if let Some(conn) = self.connections.get_mut(&connection_id)
                && target.starts_with('#')
                && let Some(voiced) = conn.channel_voiced.get_mut(&target.to_lowercase())
            {
                voiced.remove(&nickname.to_lowercase());
            }

            // Show notification in target tab
            let message = ChatMessage::system(t("msg-voice-you-left"));

            if target.starts_with('#') {
                return self.add_channel_message(connection_id, &target, message);
            } else {
                return self.add_user_message(connection_id, &target, message);
            }
        }

        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        // Remove from per-channel voiced tracking (use lowercase for consistency)
        if target.starts_with('#')
            && let Some(voiced) = conn.channel_voiced.get_mut(&target.to_lowercase())
        {
            voiced.remove(&nickname.to_lowercase());
        }

        // Update voice session participants if we're in the same session
        if let Some(ref mut session) = conn.voice_session
            && session.target.to_lowercase() == target.to_lowercase()
        {
            session.remove_participant(&nickname);
        }

        // Show notification in target tab if events are enabled
        if self.config.settings.show_join_leave_events {
            let message =
                ChatMessage::system(t_args("msg-voice-user-left", &[("nickname", &nickname)]));

            // Route to channel or user message tab based on target
            if target.starts_with('#') {
                return self.add_channel_message(connection_id, &target, message);
            } else {
                return self.add_user_message(connection_id, &target, message);
            }
        }

        Task::none()
    }
}
