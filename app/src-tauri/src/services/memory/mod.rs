pub mod compaction;
pub mod ingestion;
pub mod ml;
pub mod retrieval;

pub use crate::core::error::MemoryError;

pub use crate::services::harness::prompt_builder::format_relative_timestamp;
pub use crate::services::harness::{
    prepare_turn_context, spawn_state_compaction_observer, trigger_background_compaction,
    ChatMessage, ContextHarness, ConversationContext, ConversationManager, PrepareTurnParams, Role,
};
pub use compaction::{run_compaction, CompactionResult, COMPACTION_SYSTEM_PROMPT};
pub use ml::edge_classifier::{
    classify_edge, ensure_edge_classifier_loaded, init_edge_classifier, is_edge_classifier_loaded,
};
pub use ml::embedder::{
    cosine_similarity, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded,
};
pub use ml::nli::{
    classify_batch, ensure_nli_loaded, init_nli_engine, is_nli_loaded, relation_from_result,
    NliLabel, NliRelation,
};
pub use ml::scope_classifier::{
    classify_scope, ensure_scope_classifier_loaded, init_scope_classifier,
    is_scope_classifier_loaded,
};
pub use ml::tokenizer::estimate_tokens;
pub(crate) use ml::trim_heap;
pub use ml::{unload_all_onnx_models, unload_memory_pipeline_onnx_models};
pub use query_sieve::MemoryScope;
pub use retrieval::{retrieve_turn_profile, MemoryFact, RetrievedProfile};

pub const RESERVED_GENERATION_TOKENS: usize = 512;
pub const CONTEXT_CRITICAL_THRESHOLD: f32 = 0.85;
pub const CONTEXT_SOFT_THRESHOLD: f32 = 0.65;
pub const SOFT_COMPACTION_DEBOUNCE_SECS: u64 = 20;

pub const COSINE_HARD_MATCH_THRESHOLD: f32 = 0.98;
pub const JACCARD_EXACT_MATCH_THRESHOLD: f32 = 1.0;
pub const SOFT_VECTOR_DEDUP_THRESHOLD: f32 = 0.95;
pub const SAME_COLLECTION_CANDIDATE_SEARCH: f32 = 0.60;
pub const INTER_COLLECTION_CANDIDATE_SEARCH: f32 = 0.40;
pub const SUBFLOOR_CANDIDATE_FLOOR: f32 = 0.25;

pub const NLI_CONTRADICTION_THRESHOLD: f32 = 0.85;
pub const NLI_ENTAILMENT_THRESHOLD: f32 = 0.85;
pub const NLI_CONTRADICTION_CONFIDENCE_THRESHOLD: f32 = 0.85;
pub const NLI_CONTRADICTION_MARGIN_THRESHOLD: f32 = 0.20;
pub const NLI_ENTAILMENT_CONFIDENCE_THRESHOLD: f32 = 0.85;

pub const EDGE_CLASSIFIER_THRESHOLD: f32 = 0.80;

pub const STAGE1_BATCH_CEILING: usize = 128;
pub const STAGE2_BATCH_SIZE: usize = 16;
pub const STAGE3_BATCH_SIZE: usize = 16;
pub const STAGE4_BATCH_SIZE: usize = 32;

pub const NARRATIVE_CHAIN_SOFT_CAP_SHARE: f32 = 0.05;
pub const EMBEDDING_DIM: usize = 384;
pub const PRIMARY_EMBEDDING_MODEL_DIR: &str = "minilm-l12-v2";
pub const PRIMARY_EMBEDDING_MODEL_FILENAME: &str = "model_int8.onnx";
pub const FALLBACK_EMBEDDING_MODEL_DIR: &str = "bge-m3";
pub const FALLBACK_EMBEDDING_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const EMBEDDING_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const NLI_MODEL_DIR: &str = "nli-deberta-v3-base";
pub const NLI_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const NLI_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const EDGE_CLASSIFIER_MODEL_DIR: &str = "classifier/modernbert_edge_creation";
pub const EDGE_CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const EDGE_CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const MEMORY_SCOPE_MODEL_DIR: &str = "modernbert_memory_scope";
pub const CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const COMPACTION_SENTINEL_TURN_ID: u32 = 999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MemoryCollection {
    Identity,
    Directives,
    Narrative,
    Profile,
    Entities,
    Constraints,
}

impl MemoryCollection {
    pub const ALL: [MemoryCollection; 6] = [
        MemoryCollection::Identity,
        MemoryCollection::Directives,
        MemoryCollection::Narrative,
        MemoryCollection::Profile,
        MemoryCollection::Entities,
        MemoryCollection::Constraints,
    ];

    pub const SPECIAL_STATE: [MemoryCollection; 3] = [
        MemoryCollection::Identity,
        MemoryCollection::Directives,
        MemoryCollection::Narrative,
    ];

    pub const SEMANTIC_GRAPH: [MemoryCollection; 3] = [
        MemoryCollection::Profile,
        MemoryCollection::Entities,
        MemoryCollection::Constraints,
    ];

    pub const SEMANTIC_GRAPH_NAMES: &'static [&'static str] =
        &["Profile", "Entities", "Constraints"];

    pub fn priority(&self) -> u8 {
        match self {
            Self::Identity => 6,
            Self::Constraints => 5,
            Self::Directives => 4,
            Self::Profile => 3,
            Self::Entities => 2,
            Self::Narrative => 1,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Directives => "Directives",
            Self::Narrative => "Narrative",
            Self::Profile => "Profile",
            Self::Entities => "Entities",
            Self::Constraints => "Constraints",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Identity" => Some(Self::Identity),
            "Directives" => Some(Self::Directives),
            "Narrative" => Some(Self::Narrative),
            "Profile" => Some(Self::Profile),
            "Entities" => Some(Self::Entities),
            "Constraints" => Some(Self::Constraints),
            _ => None,
        }
    }

    pub fn collection_type(&self) -> CollectionType {
        match self {
            Self::Identity | Self::Directives | Self::Narrative => CollectionType::SpecialState,
            _ => CollectionType::SemanticGraph,
        }
    }
}

impl std::fmt::Display for MemoryCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionType {
    SpecialState,
    SemanticGraph,
}

impl CollectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SpecialState => "special_state",
            Self::SemanticGraph => "semantic_graph",
        }
    }
}

impl std::fmt::Display for CollectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub const PM_SEMANTIC_GRAPH_COLLECTIONS: &[&str] = MemoryCollection::SEMANTIC_GRAPH_NAMES;

/// Returns the structural type for a given collection name.
pub fn collection_type(collection: &str) -> CollectionType {
    if let Some(col) = MemoryCollection::parse(collection) {
        col.collection_type()
    } else {
        CollectionType::SemanticGraph
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Relation {
    Supports,
    Conflicts,
    Supersedes,
    Shapes,
    DependsOn,
}

impl Relation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supports => "SUPPORTS",
            Self::Conflicts => "CONFLICTS",
            Self::Supersedes => "SUPERSEDES",
            Self::Shapes => "SHAPES",
            Self::DependsOn => "DEPENDS_ON",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "SUPPORTS" => Some(Self::Supports),
            "CONFLICTS" => Some(Self::Conflicts),
            "SUPERSEDES" => Some(Self::Supersedes),
            "SHAPES" => Some(Self::Shapes),
            "DEPENDS_ON" => Some(Self::DependsOn),
            _ => None,
        }
    }

    pub fn inverse(&self) -> &'static str {
        match self {
            Self::Shapes => "shaped_by",
            Self::DependsOn => "dependency_of",
            Self::Conflicts => "conflicts_with",
            Self::Supports => "supported_by",
            Self::Supersedes => "superseded_by",
        }
    }
}

/// Checks if the collection pair is allowed for inter-collection edge classification (spec §4.2).
pub fn is_valid_inter_collection_pair(src: &str, tgt: &str) -> bool {
    matches!(
        (src, tgt),
        ("Identity", "Profile")
            | ("Directives", "Constraints")
            | ("Directives", "Entities")
            | ("Entities", "Constraints")
            | ("Entities", "Profile")
            | ("Entities", "Entities")
            | ("Profile", "Profile")
    )
}

/// Checks if there is a sanctioned inter-collection relationship between `col1` and `col2` in EITHER direction (spec §4.2).
pub fn has_inter_collection_relationship(col1: &str, col2: &str) -> bool {
    is_valid_inter_collection_pair(col1, col2) || is_valid_inter_collection_pair(col2, col1)
}

/// Returns the deterministic inverse relation string for any edge relation label (spec §4.3).
pub fn inverse_edge_for_relation(relation: &str) -> &'static str {
    match Relation::parse(relation) {
        Some(r) => r.inverse(),
        None => match relation {
            "shaped_by" => Relation::Shapes.as_str(),
            "dependency_of" => Relation::DependsOn.as_str(),
            "conflicts_with" => Relation::Conflicts.as_str(),
            "supported_by" => Relation::Supports.as_str(),
            "superseded_by" => Relation::Supersedes.as_str(),
            _ => "related_to",
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FactSource {
    Llm,
    User,
    Import,
    Nli,
}

impl FactSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Llm => "LLM",
            Self::User => "User",
            Self::Import => "Import",
            Self::Nli => "NLI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    StagedPending,
    ProcessingDedup,
    Deduped,
    ProcessingEmbed,
    Embedded,
    ProcessingEval,
    Evaluated,
    ProcessingCommit,
    Superseded,
    Completed,
    Failed,
    Paused,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StagedPending => "staged_pending",
            Self::ProcessingDedup => "processing_dedup",
            Self::Deduped => "deduped",
            Self::ProcessingEmbed => "processing_embed",
            Self::Embedded => "embedded",
            Self::ProcessingEval => "processing_eval",
            Self::Evaluated => "evaluated",
            Self::ProcessingCommit => "processing_commit",
            Self::Superseded => "superseded",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Paused => "paused",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "staged_pending" => Some(Self::StagedPending),
            "processing_dedup" => Some(Self::ProcessingDedup),
            "deduped" => Some(Self::Deduped),
            "processing_embed" => Some(Self::ProcessingEmbed),
            "embedded" => Some(Self::Embedded),
            "processing_eval" => Some(Self::ProcessingEval),
            "evaluated" => Some(Self::Evaluated),
            "processing_commit" => Some(Self::ProcessingCommit),
            "superseded" => Some(Self::Superseded),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "paused" => Some(Self::Paused),
            _ => None,
        }
    }
}