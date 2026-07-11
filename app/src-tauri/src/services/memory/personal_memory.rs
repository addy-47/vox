use anyhow::Result;
use turso::Connection;
use crate::services::memory::estimate_tokens;

/// Loads the resolved key-value personal personal memory profile from the database,
/// and formats it as a structured <user_profile> block for prompt injection.
pub async fn load_user_profile(conn: &Connection) -> Result<String> {
    let mut rows = conn
        .query(
            "SELECT category, key, value FROM personal_memory ORDER BY category, key",
            (),
        )
        .await?;

    let mut current_category = String::new();
    let mut profile_text = String::new();

    while let Some(row) = rows.next().await? {
        let category: String = row.get(0)?;
        let key: String = row.get(1)?;
        let value: String = row.get(2)?;

        if category != current_category {
            if !current_category.is_empty() {
                profile_text.push_str("\n");
            }
            current_category = category.clone();
            profile_text.push_str(&format!("[{}]\n", current_category));
        }

        profile_text.push_str(&format!("{}: {}\n", key, value));
    }

    if profile_text.trim().is_empty() {
        return Ok(String::new());
    }

    // Wrap in <user_profile> tags with explicit instructions
    let formatted = format!(
        "<user_profile>\n{}\nInstructions: Always address the user by name (Alex) when recalling their profile.\n</user_profile>",
        profile_text.trim()
    );

    // Apply token limit budget (approx 120 tokens max)
    let tokens = estimate_tokens(&formatted);
    if tokens <= 120 {
        return Ok(formatted);
    }

    // Budget exceeded, perform line-based truncation
    tracing::warn!(
        "[PersonalMemory] Profile block exceeds 120 tokens ({}), truncating.",
        tokens
    );
    
    let mut truncated = String::new();
    truncated.push_str("<user_profile>\n");
    let mut current_tokens = estimate_tokens("<user_profile>\nInstructions: Always address the user by name (Alex) when recalling their profile.\n</user_profile>");
    
    for line in profile_text.lines() {
        let line_tokens = estimate_tokens(line) + 1; // +1 for newline
        if current_tokens + line_tokens > 115 {
            break;
        }
        truncated.push_str(line);
        truncated.push_str("\n");
        current_tokens += line_tokens;
    }
    truncated.push_str("Instructions: Always address the user by name (Alex) when recalling their profile.\n</user_profile>");
    Ok(truncated)
}
