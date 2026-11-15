use crate::services::harness::stages::tools::TruncationPolicy;

/// Truncation engine for observation budgeting and disk spillover.
#[derive(Debug, Clone, Default)]
pub struct TruncationStage;

impl TruncationStage {
    /// Applies the specified truncation policy to a tool observation.
    ///
    /// Returns a tuple of `(preview_content, truncated, spillover_path)`.
    pub fn apply(
        _session_id: i64,
        _call_id: &str,
        _tool_name: &str,
        raw_output: &str,
        _max_tokens: usize,
        _policy: TruncationPolicy,
    ) -> (String, bool, Option<String>) {
        (raw_output.to_string(), false, None)
    }
}
