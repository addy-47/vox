pub mod assistant;
pub mod atomics;
pub mod dictation;
pub mod lifecycle;
pub mod router;
pub mod test;

use std::time::Duration;

pub const ROUTER_THREAD_NAME: &str = "vox-router";
pub const INACTIVITY_READY_TIMEOUT: Duration = Duration::from_secs(420);
pub const INACTIVITY_PAUSED_TIMEOUT: Duration = Duration::from_secs(300);

pub use atomics::PipelineAtomics;
pub use lifecycle::{init_new_session, init_new_session_sync, resume_session, spawn_idle_monitor};
pub use router::{spawn_router, target_window, transition, RoutingContext};
