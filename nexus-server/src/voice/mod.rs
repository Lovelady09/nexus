//! Voice chat module for real-time audio communication
//!
//! This module manages voice sessions for channels and user messages.
//! Voice state is entirely in-memory (ephemeral) - no database persistence.
//!
//! ## Architecture
//!
//! - **VoiceSession**: Represents a single user's voice session
//! - **VoiceRegistry**: Manages all active voice sessions on the server
//! - **VoiceUdpServer**: Handles UDP/DTLS voice packet relay
//!
//! ## Rules
//!
//! - One voice session per user on this server
//! - Channel voice: user must be a member of the channel
//! - User message voice: target user must be online

mod registry;
mod session;
mod udp;

use nexus_common::framing::MessageId;
use nexus_common::protocol::ServerMessage;

use crate::channels::ChannelManager;
use crate::db::Permission;
use crate::users::UserManager;

pub use registry::{VoiceLeaveInfo, VoiceRegistry};
pub use session::VoiceSession;
pub use udp::{VoiceUdpServer, create_voice_listener};

/// Send VoiceUserLeft notifications for a voice leave event.
///
/// This is the single point of truth for voice leave notifications,
/// consolidating logic that was previously duplicated across:
/// - connection.rs (normal disconnect)
/// - handlers/mod.rs (kick, delete, disable, ban)
/// - voice/udp.rs (DTLS timeout)
///
/// # Arguments
/// * `info` - The computed leave info from `VoiceRegistry::remove_by_*`
/// * `leaving_user_tx` - Channel to send notification to the leaving user (if available)
/// * `user_manager` - For looking up remaining participants to notify
/// * `channel_manager` - For broadcasting to channel members (channels only)
pub async fn send_voice_leave_notifications(
    info: &VoiceLeaveInfo,
    leaving_user_tx: Option<
        &tokio::sync::mpsc::UnboundedSender<(ServerMessage, Option<MessageId>)>,
    >,
    user_manager: &UserManager,
    channel_manager: &ChannelManager,
) {
    // Notify the leaving user
    if let Some(tx) = leaving_user_tx {
        let self_notification = ServerMessage::VoiceUserLeft {
            nickname: info.session.nickname.clone(),
            target: info.self_target.clone(),
        };
        let _ = tx.send((self_notification, None));
    }

    // Broadcast if this was the last session for this nickname
    if info.should_broadcast {
        if info.session.is_channel() {
            // For channels: broadcast to ALL channel members with voice_listen permission
            // (not just voice participants) so everyone can see who's in voice
            let channel_name = info.session.target.first().cloned().unwrap_or_default();
            let members = channel_manager.get_members(&channel_name).await.unwrap_or_default();

            for member_session_id in members {
                // Skip the leaving user
                if member_session_id == info.session.session_id {
                    continue;
                }

                // Check if member has voice_listen permission
                if let Some(member) = user_manager.get_user_by_session_id(member_session_id).await
                    && member.has_permission(Permission::VoiceListen)
                {
                    let leave_notification = ServerMessage::VoiceUserLeft {
                        nickname: info.session.nickname.clone(),
                        target: channel_name.clone(),
                    };
                    let _ = member.tx.send((leave_notification, None));
                }
            }
        } else {
            // For user messages: only notify the other participant
            for participant_nickname in &info.remaining_participants {
                let leave_notification = ServerMessage::VoiceUserLeft {
                    nickname: info.session.nickname.clone(),
                    target: info.broadcast_target.clone(),
                };

                if let Some(participant) = user_manager
                    .get_session_by_nickname(participant_nickname)
                    .await
                {
                    let _ = participant.tx.send((leave_notification, None));
                }
            }
        }
    }
}
