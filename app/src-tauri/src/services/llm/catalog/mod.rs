pub mod discovery;
pub mod gguf;
pub mod presets;
pub mod probe;
pub mod sync;
pub mod types;

pub use discovery::{discover_server_dialect, ServerDialect};
pub use gguf::{read_gguf_facts, GgufFacts};
pub use presets::{list_presets, lookup_preset, provider_catalog};
pub use probe::{list_models, probe_capabilities, read_capabilities_cache, CapabilityProbeEngine};
pub use sync::{get_baseline_spec, load_local_baseline_cache, spawn_catalog_sync};
pub use types::{
    CapabilityProvenance, CatalogAuthScheme, CapabilityCacheRead, LlmModelInfo, ModelCapabilities,
    ModelProbeResult, ModelSpec, ProbeCheck, ProbeOutcome, ProviderPresetMeta, ReasoningOff,
    ReasoningOn, ResponseEnvelope, ToolStreamShape, WireValue, CAP_KIND_CLOUD, CAP_KIND_EMBEDDED,
    CAP_KIND_SERVER,
};
