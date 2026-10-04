//! ============================================================================
//! evals/agentic-tool/tools/mod.rs — Per-Tool Script Registry
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals agentic-tool harness
//! Prerequisites: none
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : none; registry only
//!
//! One module per tool under evaluation. `main.rs` stays tool-agnostic; adding a
//! tool means adding a file here.

pub mod web_search;
