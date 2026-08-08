use serde::de::Visitor;
use serde::Deserializer;
use std::fmt;

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
    // Extract JSON block between first '{' and last '}'
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

    let chars = input.chars().collect::<Vec<char>>();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if escaped {
            output.push(c);
            escaped = false;
            i += 1;
        } else if c == '\\' {
            escaped = true;
            output.push(c);
            i += 1;
        } else if c == '"' {
            if !in_string {
                let keys = &[
                    "\"summary\"",
                    "\"profile_updates\"",
                    "\"memory_updates\"",
                    "\"category\"",
                    "\"key\"",
                    "\"value\"",
                    "\"confidence\"",
                ];
                let mut matched_key = None;
                for &k in keys {
                    let k_chars = k.chars().collect::<Vec<char>>();
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

                if let Some(ref k_chars) = matched_key {
                    let mut last_non_ws = None;
                    for prev_char in output.chars().rev() {
                        if !prev_char.is_whitespace() {
                            last_non_ws = Some(prev_char);
                            break;
                        }
                    }

                    if let Some(p) = last_non_ws {
                        if p != '{' && p != ',' && p != '[' {
                            output.push(',');
                        }
                    }

                    for kc in k_chars {
                        output.push(*kc);
                    }
                    i += k_chars.len();
                    continue;
                }
            }

            in_string = !in_string;
            output.push(c);
            i += 1;
        } else {
            output.push(c);
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
                _ if c.is_control() => {
                    // ignore control characters
                }
                _ => output.push(c),
            }
        } else {
            output.push(c);
        }
    }
    output
}

pub fn deserialize_value_resilient<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct ValueVisitor;

    impl<'de> Visitor<'de> for ValueVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string, boolean, or number")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v)
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok("".to_string())
        }
    }

    deserializer.deserialize_any(ValueVisitor)
}

pub fn parse_compaction_json(
    content: &str,
) -> Option<std::collections::HashMap<String, Vec<String>>> {
    let cleaned = clean_json_content(content);
    let parsed_val = serde_json::from_str::<serde_json::Value>(&cleaned).ok()?;
    let obj = parsed_val.as_object()?;

    let mut results = std::collections::HashMap::new();
    let primary_keys = [
        "Identity",
        "Directives",
        "Narrative",
        "Profile",
        "Entities",
        "Constraints",
    ];

    for &k in &primary_keys {
        results.insert(k.to_string(), Vec::new());
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

    for (k, v) in obj {
        let target_key = match k.to_ascii_lowercase().as_str() {
            "identity" => Some("Identity"),
            "directives" | "directive" | "tasks" | "task" | "goals" | "goal" => Some("Directives"),
            "narrative" | "context" => Some("Narrative"),
            "profile" | "preferences" | "preference" | "skills" | "skill" | "experiences"
            | "experience" => Some("Profile"),
            "entities" | "entity" | "projects" | "project" | "relationships" | "relationship" => {
                Some("Entities")
            }
            "constraints" | "constraint" => Some("Constraints"),
            _ => None,
        };

        if let Some(tk) = target_key {
            if let Some(list) = results.get_mut(tk) {
                extract_strings(v, list);
            }
        }
    }

    Some(results)
}
