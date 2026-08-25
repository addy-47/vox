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

/// Section blocks for user profile context assembly.
#[derive(Debug, Default)]
pub struct UserProfileSections<'a> {
    pub manifest_header: &'a str,
    pub conflict_block: &'a str,
    pub identity_block: &'a str,
    pub constraints_block: &'a str,
    pub tasks_block: &'a str,
    pub goals_block: &'a str,
    pub context_block: &'a str,
    pub semantic_block: &'a str,
}

/// Assembles memory section blocks into clean <user_profile> XML string.
pub fn format_user_profile_context(sections: &UserProfileSections<'_>) -> String {
    let mut out = String::new();
    out.push_str("<user_profile>\n");
    out.push_str(sections.manifest_header);

    if !sections.conflict_block.is_empty() {
        out.push_str("[Unresolved Contradictions]\n");
        out.push_str(sections.conflict_block);
    }
    if !sections.identity_block.is_empty() {
        out.push_str("[Identity]\n");
        out.push_str(sections.identity_block);
    }
    if !sections.constraints_block.is_empty() {
        out.push_str("[Constraints]\n");
        out.push_str(sections.constraints_block);
    }
    if !sections.tasks_block.is_empty() {
        out.push_str("[Active Tasks]\n");
        out.push_str(sections.tasks_block);
    }
    if !sections.goals_block.is_empty() {
        out.push_str("[Active Goals]\n");
        out.push_str(sections.goals_block);
    }
    if !sections.context_block.is_empty() {
        out.push_str("[User Context]\n");
        out.push_str(sections.context_block);
    }
    if !sections.semantic_block.is_empty() {
        out.push_str("[Knowledge & Notes]\n");
        out.push_str(sections.semantic_block);
    }
    out.push_str("</user_profile>");
    out
}
