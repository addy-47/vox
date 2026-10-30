//! Network audio for a remote session.
//!
//! **Owns:** the network audio capture (the phone's microphone), the network
//! audio sink (the phone's speaker), and the frame pacing that keeps a
//! 20 ms frame cadence.
//!
//! **Must never contain:** session state and protocol encoding. The audio
//! path moves samples; it does not know who is speaking or which link it
//! belongs to.
//!
//! Workspace rule: `AGENTS.md` §7.1, `audio/`.

pub mod capture;
pub mod sink;

pub use capture::{handle_inbound_audio_payload, NetworkAudioCapture};
pub use sink::NetworkAudioSink;
