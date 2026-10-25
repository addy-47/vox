//! The canonical remote session state.
//!
//! **Owns:** `RemoteLinkState` (the single source of truth for remote state),
//! `SessionRole`, the session manager, the heartbeat, and the attach
//! snapshot.
//!
//! **Must never contain:** protocol encoding and Tauri commands. The host and
//! the controller are two roles of one state machine, never two branches of
//! two state machines.
//!
//! Workspace rule: `AGENTS.md` §7.1, `session/`.

pub mod listener;
pub mod manager;
pub mod state;

pub use self::{
    listener::spawn_host_listener,
    manager::{RemoteSessionManager, SessionRole, StateChangeCallback},
    state::{RemoteFailureKind, RemoteLinkState, RemoteStateDto},
};
pub use super::{
    HEARTBEAT_INTERVAL, HEARTBEAT_MAX_MISSED_PONGS, HEARTBEAT_PING_TIMEOUT, HEARTBEAT_PONG_TIMEOUT,
};
