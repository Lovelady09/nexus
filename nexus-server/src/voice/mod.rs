//! Voice chat module for real-time audio communication
//!
//! This module manages voice sessions for channels and user messages.
//! Voice state is entirely in-memory (ephemeral) - no database persistence.
//!
//! ## Architecture
//!
//! - **VoiceSession**: Represents a single user's voice session
//! - **VoiceRegistry**: Manages all active voice sessions on the server
//!
//! ## Rules
//!
//! - One voice session per user on this server
//! - Channel voice: user must be a member of the channel
//! - User message voice: target user must be online

mod registry;
mod session;

pub use registry::VoiceRegistry;
pub use session::VoiceSession;
