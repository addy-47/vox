//! The transport-agnostic remote link.
//!
//! **Owns:** the `RemoteLink` trait, the in-process `LoopbackLink`, and the
//! link lifecycle (connect, close, link-loss reporting).
//!
//! **Must never contain:** protocol semantics and application state. A link
//! moves opaque bytes; it does not know what a message means, and it does not
//! know whether the peer is a host or a controller.
//!
//! Workspace rule: `AGENTS.md` §7.1, `link/`.
//!
//! | File | Owns |
//! | :--- | :--- |
//! | `remote_link.rs` | the `RemoteLink` trait, `LinkError`, `LinkCloseReason` |
//! | `loopback.rs` | the in-process `LoopbackLink` |
//!
//! The WebSocket transport that will join this module is wave 2. It is not
//! listed here, because this block states what the module owns today.
//!
//! The trait lives in `remote_link.rs` rather than `link.rs` because a
//! module may not share the name of its parent: clippy's `module_inception`
//! lint rejects `link/mod.rs` declaring `pub mod link;`, and this repository
//! allows no `#[allow(...)]`.

pub mod loopback;
pub mod remote_link;

pub use self::{
    loopback::LoopbackLink,
    remote_link::{LinkCloseReason, LinkError, RawLink, RemoteLink},
};
pub use super::{HANDSHAKE_DEADLINE, LOOPBACK_CHANNEL_CAPACITY};
