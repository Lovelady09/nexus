//! Voice chat message handlers
//!
//! Handles server messages for voice chat:
//! - VoiceJoinResponse - Response to VoiceJoin request
//! - VoiceLeaveResponse - Response to VoiceLeave request
//! - VoiceUserJoined - Notification when another user joins voice
//! - VoiceUserLeft - Notification when another user leaves voice

use iced::Task;
use uuid::Uuid;

use crate::NexusApp;
use crate::i18n::{t, t_args};
use crate::types::{ChatMessage, Message, VoiceSession};

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
        conn.voice_session = Some(VoiceSession::new(target, token, participants));

        // Track that this connection has the active voice session
        self.active_voice_connection = Some(connection_id);

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
        if let Some(conn) = self.connections.get_mut(&connection_id) {
            conn.voice_session = None;
        }

        // Clear active voice connection if it was this one
        if self.active_voice_connection == Some(connection_id) {
            self.active_voice_connection = None;
        }

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

        // Only update if we're in the same voice session
        if let Some(ref mut session) = conn.voice_session
            && session.target.to_lowercase() == target.to_lowercase()
        {
            session.add_participant(nickname.clone());

            // Show notification in target tab if events are enabled
            if self.config.settings.show_join_leave_events {
                let message = ChatMessage::system(t_args(
                    "msg-voice-user-joined",
                    &[("nickname", &nickname)],
                ));

                // Route to channel or user message tab based on target
                if target.starts_with('#') {
                    return self.add_channel_message(connection_id, &target, message);
                } else {
                    return self.add_user_message(connection_id, &target, message);
                }
            }
        }

        Task::none()
    }

    /// Handle VoiceUserLeft - notification when another user leaves voice
    ///
    /// Removes the user from our local participants list if we're in the same voice session.
    /// Shows notification in the target tab (channel or user message) if join/leave events are enabled.
    pub fn handle_voice_user_left(
        &mut self,
        connection_id: usize,
        nickname: String,
        target: String,
    ) -> Task<Message> {
        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        // Only update if we're in the same voice session
        if let Some(ref mut session) = conn.voice_session
            && session.target.to_lowercase() == target.to_lowercase()
        {
            session.remove_participant(&nickname);

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
        }

        Task::none()
    }
}
