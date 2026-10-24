//! The remote control plane.
//!
//! **Owns:** the routing table, the argument specs, the command gateway, the
//! secret redaction rules, and the event policy.
//!
//! **Must never contain:** transport and audio. The gateway decides *whether*
//! a command may run and *with which arguments*; it does not move bytes and
//! it does not touch the microphone.
//!
//! Workspace rule: `AGENTS.md` §7.1, `control/`.

pub mod guard;
pub mod routing;

pub use self::{
    guard::{is_command_permitted, is_desktop_command_permitted, ControlOrigin},
    routing::{classify_command, get_routing_table, CommandClass},
};
