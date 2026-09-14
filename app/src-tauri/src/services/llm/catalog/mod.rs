pub mod presets;
pub mod probe;
pub mod sync;
pub mod types;

pub use presets::{list_presets, lookup_preset, provider_catalog};
pub use probe::{list_models, probe_capabilities, CapabilityProbeEngine};
pub use sync::{get_baseline_spec, load_local_baseline_cache, spawn_catalog_sync};
pub use types::{
    CapabilityProvenance, CatalogAuthScheme, ModelProbeResult, ModelSpec, ProviderPresetMeta,
};
