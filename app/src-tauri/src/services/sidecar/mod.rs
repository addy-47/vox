pub mod download;
pub mod supervisor;
pub mod types;

pub use download::ensure_sidecar_binary;
pub use supervisor::{global_sidecar_supervisor, SidecarSupervisor};
pub use types::SidecarKind;
