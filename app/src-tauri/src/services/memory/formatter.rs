//! Pure XML Prompt Context Tree Formatter & Relative Timestamp Helper.
//! Formats retrieved personal context into standard <user_profile> structure with zero fact_id leaks.

/// Formats a millisecond epoch timestamp as a human-readable relative time label.
pub fn format_relative_timestamp(created_at_ms: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let diff_ms = now_ms - created_at_ms;

    if diff_ms < 0 {
        return "Just now".to_string();
    }

    let minutes = diff_ms / 60_000;
    let hours = diff_ms / 3_600_000;
    let days = diff_ms / 86_400_000;
    let weeks = days / 7;

    if minutes < 1 {
        "Just now".to_string()
    } else if minutes < 60 {
        format!(
            "{} minute{} ago",
            minutes,
            if minutes == 1 { "" } else { "s" }
        )
    } else if hours < 24 {
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if days == 1 {
        "Yesterday".to_string()
    } else if days < 7 {
        format!("{} days ago", days)
    } else if weeks < 4 {
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        format!("{} days ago", days)
    }
}

/// Assembles memory section blocks into clean <user_profile> XML string.
#[allow(clippy::too_many_arguments)]
pub fn format_user_profile_context(
    manifest_header: &str,
    conflict_block: &str,
    identity_block: &str,
    constraints_block: &str,
    tasks_block: &str,
    goals_block: &str,
    context_block: &str,
    semantic_block: &str,
) -> String {
    let mut out = String::new();
    out.push_str("<user_profile>\n");
    out.push_str(manifest_header);

    if !conflict_block.is_empty() {
        out.push_str("[Unresolved Contradictions]\n");
        out.push_str(conflict_block);
    }
    if !identity_block.is_empty() {
        out.push_str("[Identity]\n");
        out.push_str(identity_block);
    }
    if !constraints_block.is_empty() {
        out.push_str("[Constraints]\n");
        out.push_str(constraints_block);
    }
    if !tasks_block.is_empty() {
        out.push_str("[Active Tasks]\n");
        out.push_str(tasks_block);
    }
    if !goals_block.is_empty() {
        out.push_str("[Active Goals]\n");
        out.push_str(goals_block);
    }
    if !context_block.is_empty() {
        out.push_str(context_block);
    }
    if !semantic_block.is_empty() {
        out.push_str("[Semantic Knowledge Context]\n");
        out.push_str(semantic_block);
    }

    out.push_str("</user_profile>");
    out
}
