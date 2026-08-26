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

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests relative timestamp humanization across minute, hour, day, and week intervals with clock-skew protection.
    #[test]
    fn test_format_relative_timestamp_buckets() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        assert_eq!(format_relative_timestamp(now_ms), "Just now");
        assert_eq!(format_relative_timestamp(now_ms + 50_000), "Just now");
        assert_eq!(
            format_relative_timestamp(now_ms - 60_000),
            "1 minute ago"
        );
        assert_eq!(
            format_relative_timestamp(now_ms - 5 * 60_000),
            "5 minutes ago"
        );
        assert_eq!(
            format_relative_timestamp(now_ms - 3_600_000),
            "1 hour ago"
        );
        assert_eq!(
            format_relative_timestamp(now_ms - 4 * 3_600_000),
            "4 hours ago"
        );
        assert_eq!(format_relative_timestamp(now_ms - 86_400_000), "Yesterday");
        assert_eq!(
            format_relative_timestamp(now_ms - 3 * 86_400_000),
            "3 days ago"
        );
        assert_eq!(
            format_relative_timestamp(now_ms - 7 * 86_400_000),
            "1 week ago"
        );
        assert_eq!(
            format_relative_timestamp(now_ms - 21 * 86_400_000),
            "3 weeks ago"
        );
    }

    /// Tests XML user profile assembly with all populated sections.
    #[test]
    fn test_format_user_profile_context_full() {
        let sections = UserProfileSections {
            manifest_header: "Header Info\n",
            conflict_block: "Conflict detail\n",
            identity_block: "User is a developer\n",
            constraints_block: "Never output markdown tables\n",
            tasks_block: "Refactor voice flow\n",
            goals_block: "Ship sub-200ms latency\n",
            context_block: "Working on Linux\n",
            semantic_block: "Notes on Rust audio\n",
        };

        let formatted = format_user_profile_context(&sections);
        assert!(formatted.starts_with("<user_profile>\n"));
        assert!(formatted.contains("[Unresolved Contradictions]\nConflict detail\n"));
        assert!(formatted.contains("[Identity]\nUser is a developer\n"));
        assert!(formatted.contains("[Constraints]\nNever output markdown tables\n"));
        assert!(formatted.contains("[Active Tasks]\nRefactor voice flow\n"));
        assert!(formatted.contains("[Active Goals]\nShip sub-200ms latency\n"));
        assert!(formatted.contains("[User Context]\nWorking on Linux\n"));
        assert!(formatted.contains("[Knowledge & Notes]\nNotes on Rust audio\n"));
        assert!(formatted.ends_with("</user_profile>"));
    }

    /// Tests XML user profile assembly omitting empty section blocks cleanly.
    #[test]
    fn test_format_user_profile_context_sparse() {
        let sections = UserProfileSections {
            manifest_header: "Header Info\n",
            identity_block: "User is a developer\n",
            ..Default::default()
        };

        let formatted = format_user_profile_context(&sections);
        assert!(formatted.contains("<user_profile>\nHeader Info\n"));
        assert!(formatted.contains("[Identity]\nUser is a developer\n"));
        assert!(!formatted.contains("[Unresolved Contradictions]"));
        assert!(!formatted.contains("[Constraints]"));
        assert!(!formatted.contains("[Active Tasks]"));
        assert!(!formatted.contains("[Active Goals]"));
        assert!(!formatted.contains("[User Context]"));
        assert!(!formatted.contains("[Knowledge & Notes]"));
        assert!(formatted.ends_with("</user_profile>"));
    }
}
