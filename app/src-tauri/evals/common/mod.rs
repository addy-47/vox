//! ============================================================================
//! mod.rs — Shared harness for the memory evals
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common
//! Prerequisites: None (pure helpers; runnables declare their own)
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : N/A
//! ============================================================================

//! Shared across all three memory eval bins; each bin uses a subset, so
//! per-bin dead-code lints are disabled for this subtree.
#![allow(dead_code)]

pub mod db;
pub mod judge;
pub mod report;
pub mod settings_cfg;
pub mod turns;
