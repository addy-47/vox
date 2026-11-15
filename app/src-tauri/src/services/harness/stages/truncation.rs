use crate::{
    services::harness::stages::tools::TruncationPolicy,
    services::memory::ml::estimate_tokens,
};

const ESTIMATED_CHARS_PER_TOKEN: usize = 4;
const NEWLINE_SEARCH_WINDOW_BYTES: usize = 200;

/// Truncation engine for observation budgeting and Tier 4 disk spillover.
#[derive(Debug, Clone, Default)]
pub struct TruncationStage;

impl TruncationStage {
    /// Applies the specified truncation policy to a tool observation.
    ///
    /// If the policy is `None` or the observation fits within `max_tokens`,
    /// the content is returned unmodified without disk spillover.
    ///
    /// If truncated, full content is persisted to `paths::session_tool_outputs_dir(session_id).join("<sanitized_id>.txt")`,
    /// and a character-boundary-safe head/tail preview is spliced with a truncation marker.
    ///
    /// Returns `(preview_content, truncated, spillover_path)`.
    pub fn apply(
        session_id: i64,
        call_id: &str,
        tool_name: &str,
        raw_output: &str,
        max_tokens: usize,
        policy: TruncationPolicy,
    ) -> (String, bool, Option<String>) {
        if matches!(policy, TruncationPolicy::None) {
            return (raw_output.to_string(), false, None);
        }

        let total_tokens = estimate_tokens(raw_output);
        if total_tokens <= max_tokens {
            return (raw_output.to_string(), false, None);
        }

        let sanitized_id: String = call_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();

        let spillover_dir = crate::utils::paths::session_tool_outputs_dir(session_id);
        if let Err(e) = std::fs::create_dir_all(&spillover_dir) {
            log::warn!(
                "[TruncationStage] Failed to create spillover directory {:?}: {}",
                spillover_dir,
                e
            );
        }

        let spillover_file = spillover_dir.join(format!("{}.txt", sanitized_id));
        let spillover_path_str = spillover_file.to_string_lossy().to_string();

        if let Err(e) = std::fs::write(&spillover_file, raw_output.as_bytes()) {
            log::warn!(
                "[TruncationStage] Failed to write spillover file at {:?}: {}",
                spillover_file,
                e
            );
        }

        let (head_ratio, _tail_ratio) = match policy {
            TruncationPolicy::None => unreachable!(),
            TruncationPolicy::HeadTail { head_ratio } => (head_ratio, (1.0 - head_ratio).max(0.0)),
            TruncationPolicy::TailHeavy { tail_ratio } => ((1.0 - tail_ratio).max(0.0), tail_ratio),
        };

        let char_budget = max_tokens * ESTIMATED_CHARS_PER_TOKEN;
        let marker = format!(
            "\n\n[... Output truncated: showing {} of {} tokens. Full output saved to {}. Use read_file to view specific sections ...]\n\n",
            max_tokens, total_tokens, spillover_file.display()
        );

        let marker_chars = marker.chars().count();
        let content_char_budget = if char_budget > marker_chars {
            char_budget - marker_chars
        } else {
            char_budget / 2
        };

        let head_target_bytes = ((content_char_budget as f32) * head_ratio) as usize;
        let tail_target_bytes = content_char_budget.saturating_sub(head_target_bytes);

        // Head cut point: look for newline within search window of target, else snap to char boundary
        let head_search_end = head_target_bytes.min(raw_output.len());
        let head_search_start = head_search_end.saturating_sub(NEWLINE_SEARCH_WINDOW_BYTES);
        let head_valid_start = raw_output.floor_char_boundary(head_search_start);
        let head_valid_end = raw_output.floor_char_boundary(head_search_end);

        let head_cut = if let Some(pos) = raw_output[head_valid_start..head_valid_end].rfind('\n') {
            head_valid_start + pos + 1
        } else {
            head_valid_end
        };

        // Tail cut point: look for newline within search window of target, else snap to char boundary
        let tail_target_start = raw_output.len().saturating_sub(tail_target_bytes);
        let tail_search_start = tail_target_start.max(head_cut);
        let tail_search_end = tail_search_start.saturating_add(NEWLINE_SEARCH_WINDOW_BYTES).min(raw_output.len());
        let tail_valid_start = raw_output.ceil_char_boundary(tail_search_start);
        let tail_valid_end = raw_output.floor_char_boundary(tail_search_end);

        let tail_cut = if tail_valid_start < tail_valid_end {
            if let Some(pos) = raw_output[tail_valid_start..tail_valid_end].find('\n') {
                tail_valid_start + pos + 1
            } else {
                tail_valid_start
            }
        } else {
            raw_output.len()
        };

        let final_tail_cut = tail_cut.max(head_cut);
        let preview = format!(
            "{}{}{}",
            &raw_output[..head_cut],
            marker,
            &raw_output[final_tail_cut..]
        );

        let preview_tokens = estimate_tokens(&preview);
        log::info!(
            "[TruncationAudit] call_id={} tool={} policy={:?} original_tokens={} preview_tokens={} spillover_path={}",
            call_id,
            tool_name,
            policy,
            total_tokens,
            preview_tokens,
            spillover_path_str
        );

        (preview, true, Some(spillover_path_str))
    }
}
