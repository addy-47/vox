mod consolidate;
mod document;
mod generation;
mod patch;
mod prompts;
mod suggestions;
#[cfg(test)]
mod tests;

pub use consolidate::{
    consolidate_personal_memory, regenerate_personal_memory, ConsolidationConflictPolicy,
};
pub use document::{
    clean_markdown_payload, format_indexed_document, heading_count, parse_content_elements,
    render_content_elements, validate_document_structure, validate_no_heading_loss, ContentElement,
    ElementKind,
};
pub use patch::{apply_patch_operations, MemoryPatchOperation, PersonalConsolidationOutput};
pub use prompts::consolidation_json_schema;
pub use suggestions::{
    batch_resolve_memory_suggestions, resolve_memory_suggestions, MemorySuggestionError,
};

pub use crate::persistence::personal_memory::{
    fetch_pending_suggestions, get_personal_memory, insert_personal_memory_suggestions,
    list_personal_memory_versions, resolve_batch_suggestions_transaction,
    resolve_suggestions_transaction, save_consolidated_memory, save_personal_memory,
    set_active_personal_memory_version, MemorySuggestionRecord, PersonalMemoryRecord,
    PersonalMemorySuggestionRecord, SuggestionDecision,
};
