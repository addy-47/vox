mod consolidate;
mod generation;
mod manual;
mod model;
mod operations;
mod prompts;
mod revisions;

pub use consolidate::{
    consolidate_personal_memory, consolidate_personal_memory_with_telemetry,
    regenerate_personal_memory, ConfirmationReason, ConsolidateOutcome, ConsolidationRequest,
    ConsolidationTelemetry,
};
pub use manual::save_personal_memory_from_markdown;
pub use model::{
    generate_block_id, generate_section_id, HandleMap, MemoryBlock, MemorySection, NewSectionDraft,
    PersonalMemory,
};
pub use operations::{
    apply_operations, resolve_operations, ApplyReport, ConsolidationOutput, CreateBlockOutput,
    DeleteBlockOutput, NewSectionOutput, RejectedOperation, ResolvedOp, UpdateBlockOutput,
};
pub use prompts::consolidation_json_schema;
pub use revisions::{
    batch_resolve_memory_revisions, list_memory_revision_views, stage_revisions,
    MemoryRevisionError, MemoryRevisionView,
};

pub use crate::persistence::personal_memory::{
    fetch_pending_revisions, get_personal_memory, insert_personal_memory_revisions,
    list_personal_memory_versions, reject_all_pending_revisions,
    resolve_batch_revisions_transaction, save_consolidated_memory, save_personal_memory,
    set_active_personal_memory_version, PersonalMemoryRecord, PersonalMemoryRevisionRecord,
    RevisionDecision,
};
