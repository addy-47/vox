//! ============================================================================
//! evals/common/mod.rs — Shared evaluation helpers
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

// Each bench binary consumes a different subset of `common/`, so unused items here
// are expected rather than a defect.
#![allow(dead_code)]

pub mod cli;
pub mod datasets;
pub mod db;
pub mod keys;
pub mod llm_client;
pub mod metrics;
pub mod reporting;
pub mod stage_dump;
