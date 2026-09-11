pub fn clean_json_content(content: &str) -> String {
    let mut cleaned = content.trim().to_string();
    if cleaned.starts_with("```") {
        if let Some(first_newline) = cleaned.find('\n') {
            cleaned = cleaned[first_newline..].trim().to_string();
        }
    }
    if cleaned.ends_with("```") {
        cleaned.truncate(cleaned.len() - 3);
        cleaned = cleaned.trim().to_string();
    }
    if let (Some(start_idx), Some(end_idx)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start_idx < end_idx {
            cleaned = cleaned[start_idx..=end_idx].to_string();
        }
    }
    cleaned = fix_missing_commas_in_json(&cleaned);
    escape_control_chars_in_json(&cleaned)
}

pub fn fix_missing_commas_in_json(input: &str) -> String {
    let mut output = String::with_capacity(input.len() + 16);
    let mut in_string = false;
    let mut escaped = false;
    let mut last_non_ws: Option<char> = None;

    let chars = input.chars().collect::<Vec<char>>();
    let mut i = 0;

    let keys = [
        "\"summary\"",
        "\"profile_updates\"",
        "\"memory_updates\"",
        "\"category\"",
        "\"key\"",
        "\"value\"",
        "\"confidence\"",
    ];
    let parsed_keys: Vec<Vec<char>> = keys.iter().map(|k| k.chars().collect()).collect();

    while i < chars.len() {
        let c = chars[i];
        if escaped {
            output.push(c);
            last_non_ws = Some(c);
            escaped = false;
            i += 1;
        } else if c == '\\' {
            escaped = true;
            output.push(c);
            last_non_ws = Some(c);
            i += 1;
        } else if c == '"' {
            if !in_string {
                let mut matched_key = None;
                for k_chars in &parsed_keys {
                    if i + k_chars.len() <= chars.len() {
                        let sub = &chars[i..i + k_chars.len()];
                        if sub == k_chars.as_slice() {
                            let mut next_idx = i + k_chars.len();
                            while next_idx < chars.len() && chars[next_idx].is_whitespace() {
                                next_idx += 1;
                            }
                            if next_idx < chars.len() && chars[next_idx] == ':' {
                                matched_key = Some(k_chars);
                                break;
                            }
                        }
                    }
                }

                if let Some(k_chars) = matched_key {
                    if let Some(p) = last_non_ws {
                        if p != '{' && p != ',' && p != '[' && p != ':' {
                            output.push(',');
                        }
                    }

                    for kc in k_chars {
                        output.push(*kc);
                    }
                    last_non_ws = Some('"');
                    i += k_chars.len();
                    continue;
                }
            }

            in_string = !in_string;
            output.push(c);
            last_non_ws = Some(c);
            i += 1;
        } else {
            output.push(c);
            if !c.is_whitespace() {
                last_non_ws = Some(c);
            }
            i += 1;
        }
    }
    output
}

pub fn escape_control_chars_in_json(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;

    for c in input.chars() {
        if escaped {
            match c {
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                _ => output.push(c),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
            output.push(c);
        } else if c == '"' {
            in_string = !in_string;
            output.push(c);
        } else if in_string {
            match c {
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                _ if c.is_control() => {}
                _ => output.push(c),
            }
        } else {
            output.push(c);
        }
    }
    output
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct UnifiedCompactionPayload {
    #[serde(default)]
    pub context_summary: String,
    #[serde(default)]
    pub personal: Vec<String>,
    #[serde(default)]
    pub objective: Vec<String>,
    #[serde(default)]
    pub workdone: Vec<String>,
    #[serde(default)]
    pub blocker: Vec<String>,
    #[serde(default)]
    pub next_step: Vec<String>,
    #[serde(default)]
    pub pitfall: Vec<String>,
}

impl UnifiedCompactionPayload {
    /// Formats the entire structured compaction output into readable session context.
    pub fn format_session_context(&self) -> String {
        let mut out = String::new();
        if !self.personal.is_empty() {
            out.push_str("Personal:\n");
            for item in &self.personal {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.objective.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("Objectives:\n");
            for item in &self.objective {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.workdone.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("Completed Work:\n");
            for item in &self.workdone {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.blocker.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("Blockers:\n");
            for item in &self.blocker {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.next_step.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("Next Steps:\n");
            for item in &self.next_step {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.pitfall.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("Pitfalls & Constraints:\n");
            for item in &self.pitfall {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if out.is_empty() && !self.context_summary.is_empty() {
            out.push_str(&self.context_summary);
        }
        out
    }
}

pub fn parse_unified_compaction_json(content: &str) -> Option<UnifiedCompactionPayload> {
    let cleaned = clean_json_content(content);
    let parsed_val = serde_json::from_str::<serde_json::Value>(&cleaned).ok()?;
    let obj = parsed_val.as_object()?;

    let mut payload = UnifiedCompactionPayload::default();

    for (k, v) in obj {
        let key_lower = k.to_ascii_lowercase();
        match key_lower.as_str() {
            "context_summary" | "narrative" | "summary" | "context" => {
                if let Some(s) = v.as_str() {
                    payload.context_summary = s.trim().to_string();
                } else if let Some(arr) = v.as_array() {
                    let parts: Vec<String> = arr
                        .iter()
                        .filter_map(|x| x.as_str())
                        .map(|s| s.trim().to_string())
                        .collect();
                    payload.context_summary = parts.join("\n");
                }
            }
            "personal" | "identity" | "profile" => {
                extract_strings(v, &mut payload.personal);
            }
            "objective" | "directives" | "goals" | "tasks" => {
                extract_strings(v, &mut payload.objective);
            }
            "workdone" | "work_done" | "completed" | "progress" => {
                extract_strings(v, &mut payload.workdone);
            }
            "blocker" | "blockers" | "constraints" | "issues" => {
                extract_strings(v, &mut payload.blocker);
            }
            "next_step" | "next_steps" | "nextstep" => {
                extract_strings(v, &mut payload.next_step);
            }
            "pitfall" | "pitfalls" | "lessons" => {
                extract_strings(v, &mut payload.pitfall);
            }
            _ => {}
        }
    }

    Some(payload)
}

fn extract_strings(val: &serde_json::Value, list: &mut Vec<String>) {
    match val {
        serde_json::Value::Null => {}
        serde_json::Value::Bool(b) => list.push(b.to_string()),
        serde_json::Value::Number(n) => list.push(n.to_string()),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                list.push(trimmed.to_string());
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_strings(item, list);
            }
        }
        serde_json::Value::Object(map) => {
            let mut found = false;
            for possible_key in &[
                "text",
                "fact",
                "description",
                "desc",
                "content",
                "value",
                "name",
            ] {
                if let Some(sub_val) = map.get(*possible_key) {
                    extract_strings(sub_val, list);
                    found = true;
                    break;
                }
            }
            if !found {
                let mut parts = Vec::new();
                for (k, v) in map {
                    let mut sub_strs = Vec::new();
                    extract_strings(v, &mut sub_strs);
                    if !sub_strs.is_empty() {
                        parts.push(format!("{}: {}", k, sub_strs.join(", ")));
                    }
                }
                if !parts.is_empty() {
                    list.push(parts.join("; "));
                }
            }
        }
    }
}
