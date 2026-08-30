use super::buffer::{ChatMessage, Role};
use crate::services::memory::ml::estimate_tokens;
use std::collections::HashMap;

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

use crate::services::memory::retrieval::RetrievedProfile;

/// Formats a RetrievedProfile into formatted user profile XML sub-blocks.
pub fn format_retrieved_profile(profile: &RetrievedProfile) -> String {
    if profile.is_empty() {
        return String::new();
    }

    let mut sections = Vec::new();

    let mut sql_lines = Vec::new();
    for fact in &profile.sql_sections {
        let time_str = format_relative_timestamp(fact.created_at);
        sql_lines.push(format!("- ({}) [{}] {}", time_str, fact.collection, fact.fact));
    }
    if !sql_lines.is_empty() {
        sections.push(format!("[Directives & Narrative]\n{}", sql_lines.join("\n")));
    }

    let mut vector_lines = Vec::new();
    for seed in &profile.vector_seeds {
        let time_str = format_relative_timestamp(seed.created_at);
        vector_lines.push(format!("- ({}) [{}] {}", time_str, seed.collection, seed.fact));
    }
    for edge in &profile.graph_children {
        vector_lines.push(format!(
            "  ↳ --[{}]--> [{}] {}",
            edge.relation, edge.target_collection, edge.target_fact
        ));
    }
    if !vector_lines.is_empty() {
        sections.push(format!("[User Context & Knowledge]\n{}", vector_lines.join("\n")));
    }

    sections.join("\n\n")
}

/// Assembles the complete system prompt from base prompt, identity facts, and dynamic profile.
pub fn assemble_system_prompt(
    base_system_prompt: &str,
    identity_facts: &[String],
    dynamic_user_profile: Option<&str>,
) -> String {
    let mut sections = Vec::new();
    if !identity_facts.is_empty() {
        let identity_lines: Vec<String> = identity_facts
            .iter()
            .map(|f| format!("- {}", f))
            .collect();
        sections.push(format!("[Identity]\n{}", identity_lines.join("\n")));
    }

    if let Some(dyn_profile) = dynamic_user_profile {
        let trimmed = dyn_profile.trim();
        if !trimmed.is_empty() {
            let inner = if trimmed.starts_with("<user_profile>")
                && trimmed.ends_with("</user_profile>")
            {
                trimmed[14..trimmed.len() - 15].trim()
            } else {
                trimmed
            };
            if !inner.is_empty() {
                sections.push(inner.to_string());
            }
        }
    }

    if sections.is_empty() {
        base_system_prompt.to_string()
    } else {
        format!(
            "{}\n\n<user_profile>\n{}\n</user_profile>",
            base_system_prompt.trim_end(),
            sections.join("\n\n")
        )
    }
}

/// Formats recent compaction narrative chain and facts into XML session history.
pub fn build_session_history_xml(
    narrative_chain: &str,
    latest_compaction_facts: &HashMap<String, Vec<String>>,
) -> String {
    let mut session_history = String::new();

    if !narrative_chain.is_empty() || !latest_compaction_facts.is_empty() {
        session_history.push_str("<session_history>\n");
        if !narrative_chain.is_empty() {
            session_history.push_str("  <narrative_chain>\n  ");
            session_history.push_str(narrative_chain);
            session_history.push_str("\n  </narrative_chain>\n");
        }
        if !latest_compaction_facts.is_empty() {
            session_history.push_str("  <recent_compaction_facts>\n");
            for (col, facts) in latest_compaction_facts {
                if !facts.is_empty() {
                    session_history.push_str(&format!("    [{}]\n", col));
                    for f in facts {
                        session_history.push_str(&format!("    - {}\n", f));
                    }
                }
            }
            session_history.push_str("  </recent_compaction_facts>\n");
        }
        session_history.push_str("</session_history>");
    }

    session_history
}

/// Consolidates session history XML into the root System Message.
pub fn consolidate_system_message(
    messages: &mut [ChatMessage],
    system_prompt: &ChatMessage,
    session_history: &str,
    total_token_count: &mut usize,
) {
    if session_history.is_empty() || messages.is_empty() || messages[0].role != Role::System {
        return;
    }

    let base_content = &system_prompt.content;
    let cleaned_base = if let (Some(start), Some(end)) = (
        base_content.find("<session_history>"),
        base_content.find("</session_history>"),
    ) {
        let before = &base_content[..start];
        let after = &base_content[end + "</session_history>".len()..];
        format!("{}{}", before.trim_end(), after)
    } else {
        base_content.clone()
    };

    let consolidated_prompt = if let Some(idx) = cleaned_base.find("<user_profile>") {
        let (prefix, suffix) = cleaned_base.split_at(idx);
        format!("{}\n{}\n\n{}", prefix.trim_end(), session_history, suffix)
    } else {
        format!("{}\n\n{}", cleaned_base.trim_end(), session_history)
    };

    let old_sys_tokens = estimate_tokens(&messages[0].content);
    let new_sys_tokens = estimate_tokens(&consolidated_prompt);
    messages[0].content = consolidated_prompt;
    *total_token_count = total_token_count.saturating_sub(old_sys_tokens) + new_sys_tokens;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests relative timestamp humanization across minute, hour, day, and week intervals.
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
}
