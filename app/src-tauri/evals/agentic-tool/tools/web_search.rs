//! ============================================================================
//! evals/agentic-tool/tools/web_search.rs — web_search Tool Script
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals agentic-tool harness (per-tool definition)
//! Prerequisites: none
//! Execution    : cargo bench --bench agentic_tool_eval --release -- tool --help
//! Metrics      : none; this file only maps flags to a tool call and to text
//!
//! One file per tool under evaluation. `main.rs` is deliberately tool-agnostic:
//! it knows about mock LLM, harness, TTS and artifact capture, and nothing about
//! what any individual tool's arguments mean. Adding a tool means adding a file
//! here, not editing the harness.
//!
//! ## Why the script lives here
//!
//! The eval's *input* is the LLM's output to the harness. For `web_search` that is
//! a tool call carrying a query and retrieval knobs. Building it from flags — and
//! from the corpus when batching — keeps the harness free of tool semantics while
//! letting every knob be varied per run.

use anyhow::Result;
use serde_json::{json, Value};

use crate::common::reporting::resolve_output_dir;

/// Flag set for one `web_search` invocation.
///
/// Every field maps to a model-facing schema property, so varying any of them
/// exercises a different production code path.
#[derive(Debug, Clone)]
pub struct WebSearchScript {
    /// Model-facing tool name.
    pub tool: String,
    pub query: String,
    pub time_filter: String,
    pub ranking_mode: String,
    pub max_passages: u64,
    pub spoken_filler: String,

    // ── Depth / focus knobs. Not in the current schema; the harness ignores
    //    unknown properties, so these are inert until the spec is amended and the
    //    tool adopts them. Kept here so the eval can already measure them.
    pub depth: Option<String>,
    pub focus: Option<String>,
    pub sub_queries: Vec<String>,
    pub must_contain: Vec<String>,
    pub source_diversity: bool,
}

impl Default for WebSearchScript {
    fn default() -> Self {
        Self {
            tool: "web_search".to_string(),
            query: String::new(),
            time_filter: "any".to_string(),
            ranking_mode: "hybrid".to_string(),
            max_passages: 5,
            spoken_filler: "Searching the web now...".to_string(),
            depth: None,
            focus: None,
            sub_queries: Vec::new(),
            must_contain: Vec::new(),
            source_diversity: true,
        }
    }
}

impl WebSearchScript {
    /// Builds the `arguments` object for the tool call.
    ///
    /// Only properties the model-facing schema actually declares today are always
    /// emitted. Depth-family properties are included only when set, so a baseline
    /// run sends exactly the arguments the shipped schema would produce.
    pub fn arguments(&self) -> Value {
        let mut args = json!({
            "query": self.query,
            "time_filter": self.time_filter,
            "ranking_mode": self.ranking_mode,
            "max_passages": self.max_passages,
            "spoken_filler": self.spoken_filler,
        });
        let obj = args.as_object_mut().expect("json! literal is an object");

        if let Some(d) = &self.depth {
            obj.insert("depth".into(), json!(d));
        }
        if let Some(f) = &self.focus {
            obj.insert("focus".into(), json!(f));
        }
        if !self.sub_queries.is_empty() {
            obj.insert("sub_queries".into(), json!(self.sub_queries));
        }
        if !self.must_contain.is_empty() {
            obj.insert("must_contain".into(), json!(self.must_contain));
        }
        if !self.source_diversity {
            obj.insert("source_diversity".into(), json!(false));
        }
        args
    }

    /// One-line human summary recorded in the manifest.
    pub fn summary(&self) -> String {
        let extras = [
            self.depth.as_ref().map(|d| format!(" depth={d}")),
            self.focus.as_ref().map(|f| format!(" focus={f:?}")),
            (!self.sub_queries.is_empty())
                .then(|| format!(" sub_queries={}", self.sub_queries.len())),
            (!self.must_contain.is_empty())
                .then(|| format!(" must_contain={}", self.must_contain.len())),
        ]
        .into_iter()
        .flatten()
        .collect::<String>();
        format!(
            "{}(query={:?}, time_filter={}, ranking={}, max_passages={}{})",
            self.tool, self.query, self.time_filter, self.ranking_mode, self.max_passages, extras
        )
    }

    /// Corpus entry form: id plus query only.
    ///
    /// The corpus carries no expectations. Ground truth is established by the QA
    /// subagent doing its own search, so encoding expected facts or latency
    /// budgets in the dataset would prejudge the comparison.
    pub fn from_corpus_entry(query: &str, base: &Self) -> Self {
        Self {
            query: query.to_string(),
            tool: base.tool.clone(),
            time_filter: base.time_filter.clone(),
            ranking_mode: base.ranking_mode.clone(),
            max_passages: base.max_passages,
            spoken_filler: base.spoken_filler.clone(),
            depth: base.depth.clone(),
            focus: base.focus.clone(),
            sub_queries: base.sub_queries.clone(),
            must_contain: base.must_contain.clone(),
            source_diversity: base.source_diversity,
        }
    }
}

/// Corpus entry. Deliberately only an id and a query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorpusEntry {
    pub id: String,
    pub query: String,
}

/// Loads `evals/assets/web_search_corpus.json`.
pub fn load_corpus() -> Result<Vec<CorpusEntry>> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evals/assets/web_search_corpus.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read corpus at {:?}: {}", path, e))?;
    serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("Failed to parse corpus: {}", e))
}

/// Default results root for this tool's runs.
#[allow(dead_code)]
pub fn default_results_root() -> std::path::PathBuf {
    resolve_output_dir(None, "agentic-tool")
}
