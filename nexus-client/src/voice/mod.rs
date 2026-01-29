//! Voice chat audio system
//!
//! This module provides client-side audio for voice chat:
//! - Audio device enumeration and selection
//! - Microphone capture and speaker playback
//! - Opus encoding/decoding
//! - DTLS connection to server
//! - Jitter buffering for smooth playback
//! - Push-to-talk handling
//! - Microphone testing for settings

pub mod audio;
pub mod codec;
pub mod dtls;
pub mod jitter;
pub mod manager;
pub mod mic_test;
pub mod ptt;
pub mod subscription;
