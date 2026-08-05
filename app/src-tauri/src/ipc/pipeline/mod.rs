//! ============================================================================
//! src/ipc/pipeline/mod.rs — Pipeline IPC module declarations and re-exports
//! ============================================================================

pub mod engine_launch;
pub mod lifecycle;
pub mod realtime;
pub mod test_clip;

pub use engine_launch::*;
pub use lifecycle::*;
pub use realtime::*;
pub use test_clip::*;
