use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

type UnifiedCompactionPayload = std::collections::HashMap<String, Vec<String>>;

#[derive(Debug, Deserialize)]
struct JudgeReportDetail {
    pass: bool,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct JudgeReport {
    identity_check: JudgeReportDetail,
    constraints_check: JudgeReportDetail,
    preferences_check: JudgeReportDetail,
    tasks_check: JudgeReportDetail,
    goals_check: JudgeReportDetail,
}

fn strip_trailing_commas(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
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
            continue;
        }
        if c == '\\' {
            output.push(c);
            escaped = true;
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            output.push(c);
            i += 1;
            continue;
        }
        if in_string {
            output.push(c);
            i += 1;
            continue;
        }
        
        // Outside string literal:
        if c == ',' {
            // Check if this comma is followed only by whitespace and then a closing delimiter '}' or ']'
            let mut j = i + 1;
            let mut is_trailing = false;
            while j < chars.len() {
                let next_c = chars[j];
                if next_c.is_whitespace() {
                    j += 1;
                } else if next_c == '}' || next_c == ']' {
                    is_trailing = true;
                    break;
                } else {
                    break;
                }
            }
            if is_trailing {
                // Skip the comma!
                i += 1;
                continue;
            }
        }
        output.push(c);
        i += 1;
    }
    output
}

fn load_env_var(key: &str) -> String {
    let candidates = vec![
        PathBuf::from("temp/.env"),
        PathBuf::from("../temp/.env"),
        PathBuf::from("../../temp/.env"),
    ];
    for env_path in candidates {
        if env_path.exists() {
            if let Ok(content) = fs::read_to_string(&env_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with('#') {
                        continue;
                    }
                    if let Some(val) = trimmed.strip_prefix(&format!("{}=", key)) {
                        return val.trim().trim_matches('"').trim_matches('\'').to_string();
                    }
                }
            }
        }
    }
    std::env::var(key).unwrap_or_default()
}

async fn query_llm(
    prompt: &str,
    system_prompt: &str,
    use_large: bool,
) -> Result<String> {
    let nvidia_key = load_env_var("NVIDIA_API_KEY");
    let gemini_key = load_env_var("GEMINI_API_KEY");

    // Model selection based on user directive:
    // Large model for dataset gen / judging: meta/llama-3.3-70b-instruct or gemini-2.5-pro
    // Small model for compaction execution: meta/llama-3.1-8b-instruct or gemini-2.5-flash
    let (nvidia_model, gemini_model) = if use_large {
        ("meta/llama-3.3-70b-instruct", "gemini-2.5-pro")
    } else {
        ("meta/llama-3.1-8b-instruct", "gemini-2.5-flash")
    };

    // Attempt 1: NVIDIA
    if !nvidia_key.is_empty() {
        println!("[LLM] Attempting NVIDIA API using model {}", nvidia_model);
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "model": nvidia_model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.1,
            "max_tokens": 1024
        });

        match client.post("https://integrate.api.nvidia.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", nvidia_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(text_val) = json["choices"][0]["message"]["content"].as_str() {
                            return Ok(text_val.to_string());
                        }
                    }
                    println!("[LLM] NVIDIA API parsing failed. Raw response: {}", text);
                } else {
                    println!("[LLM] NVIDIA API returned status: {} with body: {}", status, text);
                }
            }
            Err(e) => {
                println!("[LLM] NVIDIA API request failed: {}", e);
            }
        }
    }

    // Attempt 2: Gemini Fallback
    if !gemini_key.is_empty() {
        println!("[LLM] Attempting Gemini API fallback using model {}", gemini_model);
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        {"text": format!("System Prompt: {}\n\nUser Input: {}", system_prompt, prompt)}
                    ]
                }
            ],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 1024
            }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            gemini_model, gemini_key
        );

        match client.post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(candidate) = json["candidates"].as_array().and_then(|c| c.first()) {
                            if let Some(text_val) = candidate["content"]["parts"][0]["text"].as_str() {
                                return Ok(text_val.to_string());
                            }
                        }
                    }
                    println!("[LLM] Gemini API parsing failed. Raw response: {}", text);
                } else {
                    println!("[LLM] Gemini API returned status: {} with body: {}", status, text);
                }
            }
            Err(e) => {
                println!("[LLM] Gemini API request failed: {}", e);
            }
        }
    }

    Err(anyhow!("All LLM providers failed or API keys are missing."))
}

#[tokio::test]
#[ignore] // External API test, runs only with `cargo test --test memory_v3_compaction_eval -- --ignored`
async fn test_v3_compaction_and_judging() -> Result<()> {
    println!("\n=== Starting V3 Compaction Quality Evaluation ===");

    // 1. Generate a diverse multi-turn humanistic conversation dataset (using Large Model)
    let dataset_prompt = "Generate a realistic multi-turn conversation between a user and an assistant. \
        The user should express explicit human details: their name (Alex), their system constraints (strictly gluten-free), \
        two tasks they want to do later (call grandmother Evelyn at 4 PM, clean the oven), a long-term goal (wants to read 50 books this year), \
        and a preference (prefers dark roast coffee over light roast). \
        Keep the conversation natural, containing small talk and at least 6 turns total.";

    let conversation_history = query_llm(
        dataset_prompt,
        "You are a test dataset generator. Return only the raw multi-turn conversation dialogue.",
        true, // use large model
    ).await?;

    println!("\nGenerated Test Conversation History:\n{}", conversation_history);

    // 2. Perform compaction using the V3 system prompt (using Small Model)
    let compaction_prompt = vox_lib::core::constants::COMPACTION_SYSTEM_PROMPT.to_string();
    let user_msg = format!("Here is the conversation history to compress:\n\n{}", conversation_history);

    let compaction_result_raw = query_llm(
        &user_msg,
        &compaction_prompt,
        false, // use small model (pipeline test validation)
    ).await?;

    println!("\nCompaction Raw Result:\n{}", compaction_result_raw);

    // Clean JSON content using utility and remove trailing commas
    let cleaned_base = vox_lib::utils::json::clean_json_content(&compaction_result_raw);
    let cleaned = strip_trailing_commas(&cleaned_base);
    println!("\nCompaction Cleaned Result:\n{}", cleaned);
    
    // 3. Deserialize and validate JSON format correctness
    let payload: UnifiedCompactionPayload = serde_json::from_str(&cleaned)
        .map_err(|e| anyhow!("Compaction output failed JSON parsing: {}\nRaw output: {}", e, cleaned))?;

    let summary = payload.get("Context").and_then(|v| v.first()).cloned().unwrap_or_default();
    println!("\nParsed Compaction Summary:\n{}", summary);
    println!("Extracted Facts:");
    for (collection, facts) in &payload {
        println!("  {}: {:?}", collection, facts);
    }

    // Direct Rust-level assertions to verify mapping of human-centric facts
    let identity_facts = payload.get("Identity")
        .ok_or_else(|| anyhow!("Missing 'Identity' collection"))?;
    let has_alex = identity_facts.iter().any(|f| f.to_lowercase().contains("alex"));
    assert!(has_alex, "Rust-level validation failed: 'Identity' collection does not contain 'Alex'. Got: {:?}", identity_facts);

    let constraints_facts = payload.get("Constraints")
        .ok_or_else(|| anyhow!("Missing 'Constraints' collection"))?;
    let has_gluten = constraints_facts.iter().any(|f| f.to_lowercase().contains("gluten"));
    assert!(has_gluten, "Rust-level validation failed: 'Constraints' collection does not contain 'gluten'. Got: {:?}", constraints_facts);

    let preferences_facts = payload.get("Preferences")
        .ok_or_else(|| anyhow!("Missing 'Preferences' collection"))?;
    let has_coffee = preferences_facts.iter().any(|f| f.to_lowercase().contains("coffee") || f.to_lowercase().contains("roast"));
    assert!(has_coffee, "Rust-level validation failed: 'Preferences' collection does not contain 'coffee' or 'roast'. Got: {:?}", preferences_facts);

    let tasks_facts = payload.get("Tasks")
        .ok_or_else(|| anyhow!("Missing 'Tasks' collection"))?;
    let has_grandmother = tasks_facts.iter().any(|f| f.to_lowercase().contains("grandm") || f.to_lowercase().contains("evelyn"));
    let has_oven = tasks_facts.iter().any(|f| f.to_lowercase().contains("oven") || f.to_lowercase().contains("clean"));
    assert!(has_grandmother, "Rust-level validation failed: 'Tasks' collection does not contain 'grandmother' / 'Evelyn'. Got: {:?}", tasks_facts);
    assert!(has_oven, "Rust-level validation failed: 'Tasks' collection does not contain 'oven' / 'clean'. Got: {:?}", tasks_facts);

    let goals_facts = payload.get("Goals")
        .ok_or_else(|| anyhow!("Missing 'Goals' collection"))?;
    let has_books = goals_facts.iter().any(|f| f.to_lowercase().contains("book") || f.to_lowercase().contains("50") || f.to_lowercase().contains("read"));
    assert!(has_books, "Rust-level validation failed: 'Goals' collection does not contain 'books' / 'read' / '50'. Got: {:?}", goals_facts);

    println!("\nDirect Rust-level assertions passed successfully.");

    // 4. LLM-as-Judge Evaluation (using Large Model)
    let judge_system_prompt = "You are an AI Evaluator checking the accuracy of a memory compaction engine.\n\n\
        You will be given:\n\
        1. The source conversation history.\n\
        2. The output JSON payload of the compaction engine.\n\n\
        Evaluate whether:\n\
        - The Name 'Alex' was correctly assigned to the 'Identity' collection.\n\
        - The Gluten-free constraint was correctly assigned to the 'Constraints' collection.\n\
        - The Coffee preference was correctly assigned to the 'Preferences' collection.\n\
        - The tasks (call grandmother, clean oven) were correctly assigned to the 'Tasks' collection.\n\
        - The book goal (read 50 books) was correctly assigned to the 'Goals' collection.\n\n\
        You MUST return ONLY a JSON object with the following structure, with no extra conversational text or markdown blocks:\n\
        {\n\
          \"identity_check\": { \"pass\": true, \"reason\": \"string\" },\n\
          \"constraints_check\": { \"pass\": true, \"reason\": \"string\" },\n\
          \"preferences_check\": { \"pass\": true, \"reason\": \"string\" },\n\
          \"tasks_check\": { \"pass\": true, \"reason\": \"string\" },\n\
          \"goals_check\": { \"pass\": true, \"reason\": \"string\" }\n\
        }";

    let judge_prompt = format!(
        "Source Conversation:\n{}\n\nCompaction Output:\n{}",
        conversation_history, cleaned
    );

    let evaluation_raw = query_llm(
        &judge_prompt,
        judge_system_prompt,
        true, // use large model
    ).await?;

    println!("\nRaw Judge Output:\n{}", evaluation_raw);

    // Clean judge JSON and strip trailing commas
    let cleaned_judge_base = vox_lib::utils::json::clean_json_content(&evaluation_raw);
    let cleaned_judge = strip_trailing_commas(&cleaned_judge_base);
    println!("\nCleaned Judge Output:\n{}", cleaned_judge);

    let judge_payload: JudgeReport = serde_json::from_str(&cleaned_judge)
        .map_err(|e| anyhow!("Judge output failed JSON parsing: {}\nRaw output: {}", e, cleaned_judge))?;

    println!("\n=== Parseable Judge Evaluation Report ===");
    println!("Identity Check:    {:?}", judge_payload.identity_check);
    println!("Constraints Check: {:?}", judge_payload.constraints_check);
    println!("Preferences Check: {:?}", judge_payload.preferences_check);
    println!("Tasks Check:       {:?}", judge_payload.tasks_check);
    println!("Goals Check:       {:?}", judge_payload.goals_check);

    // Assert each check explicitly to prevent bypassing core validation
    assert!(judge_payload.identity_check.pass, "Identity check failed: {}", judge_payload.identity_check.reason);
    assert!(judge_payload.constraints_check.pass, "Constraints check failed: {}", judge_payload.constraints_check.reason);
    assert!(judge_payload.preferences_check.pass, "Preferences check failed: {}", judge_payload.preferences_check.reason);
    assert!(judge_payload.tasks_check.pass, "Tasks check failed: {}", judge_payload.tasks_check.reason);
    assert!(judge_payload.goals_check.pass, "Goals check failed: {}", judge_payload.goals_check.reason);

    println!("\nAll LLM-as-Judge evaluation checks passed successfully.");

    Ok(())
}
