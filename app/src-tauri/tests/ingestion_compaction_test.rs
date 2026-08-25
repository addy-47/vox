use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

use vox_lib::core::constants::SYSTEM_PROMPT_MODULAR;
use vox_lib::services::llm::providers::OpenAiCompatProvider;
use vox_lib::services::llm::ProviderKind;
use vox_lib::services::memory::working_memory::{ConversationManager, Role};

const DEFAULT_CTX_WINDOW_TOKENS: usize = 4096;

fn get_nvidia_api_key() -> Option<String> {
    if let Ok(key) = env::var("NVIDIA_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key.trim().to_string());
        }
    }
    // Try reading temp/.env
    if let Ok(contents) = fs::read_to_string("../../temp/.env") {
        for line in contents.lines() {
            if line.starts_with("NVIDIA_API_KEY=") {
                let val = line.trim_start_matches("NVIDIA_API_KEY=").trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    if let Ok(contents) = fs::read_to_string("temp/.env") {
        for line in contents.lines() {
            if line.starts_with("NVIDIA_API_KEY=") {
                let val = line.trim_start_matches("NVIDIA_API_KEY=").trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

struct DatasetTurn {
    turn: usize,
    user: String,
    assistant: String,
}

fn load_json_dataset(path: &str) -> Vec<DatasetTurn> {
    let resolved_path = if Path::new(path).exists() {
        path.to_string()
    } else {
        format!("../../{}", path)
    };

    println!("[TestLoader] Loading dataset from {}", resolved_path);
    let content = fs::read_to_string(&resolved_path)
        .unwrap_or_else(|e| panic!("Failed to read dataset file at {}: {}", resolved_path, e));

    let json_array: Vec<Value> = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse JSON dataset: {}", e));

    let mut turns = Vec::new();
    for item in json_array {
        let turn_num = item["turn"].as_u64().unwrap_or(0) as usize;
        let user_text = item["user"].as_str().unwrap_or("").to_string();
        let assistant_text = item["assistant"].as_str().unwrap_or("").to_string();

        if !user_text.is_empty() || !assistant_text.is_empty() {
            turns.push(DatasetTurn {
                turn: turn_num,
                user: user_text,
                assistant: assistant_text,
            });
        }
    }

    println!("[TestLoader] Successfully loaded {} turns.", turns.len());
    turns
}

#[test]
fn test_ingestion_and_compaction_session1() {
    let _ = env_logger::builder().is_test(true).try_init();

    let data_path = env::var("TEST_DATA_PATH")
        .unwrap_or_else(|_| "sandbox/datasets/dataset_session1.json".to_string());

    let turns = load_json_dataset(&data_path);
    assert!(!turns.is_empty(), "Dataset must not be empty");

    let api_key = get_nvidia_api_key()
        .expect("CRITICAL: NVIDIA_API_KEY is required for Nvidia API integration test");

    let base_url = env::var("LLM_BASE_URL")
        .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());
    let model = env::var("LLM_MODEL").unwrap_or_else(|_| "meta/llama-3.1-8b-instruct".to_string());
    let provider_name = env::var("LLM_PROVIDER_NAME").unwrap_or_else(|_| "nvidia".to_string());

    println!(
        "[TestSetup] LLM Provider: {} | Model: {} | Base URL: {}",
        provider_name, model, base_url
    );

    let provider =
        OpenAiCompatProvider::new(&base_url, &model, Some(&api_key), Some(&provider_name));

    let ctx_window = env::var("CTX_WINDOW")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CTX_WINDOW_TOKENS);

    println!("[TestSetup] Initializing ConversationManager with CTX_WINDOW = {} tokens (85% trigger = {} tokens)",
        ctx_window, (ctx_window as f32 * 0.85) as usize);

    let mut mgr = ConversationManager::new(ctx_window);
    mgr.new_session(SYSTEM_PROMPT_MODULAR);

    let mut compaction_count = 0;
    let mut total_compaction_time_ms = 0u128;
    let mut total_facts_extracted = 0usize;

    println!("\n=== Starting Session 1 Turn Processing ===");

    for turn in &turns {
        if !turn.user.is_empty() {
            mgr.push_user_turn(turn.user.clone());
        }
        if !turn.assistant.is_empty() {
            mgr.push_assistant_turn(turn.assistant.clone());
        }

        let utilization = mgr.context_utilization();
        if mgr.needs_threshold_maintenance() {
            compaction_count += 1;
            println!(
                "\n[Compaction #{}] Triggered at Turn {} (Utilization: {:.2}%, Tokens: {}/{})",
                compaction_count,
                turn.turn,
                utilization * 100.0,
                mgr.total_token_count(),
                ctx_window
            );

            let start_time = Instant::now();
            let (ctx, speech, personal_memory) =
                mgr.build_context(ProviderKind::OpenAiCompat, false, Some(&provider), None);
            let elapsed_ms = start_time.elapsed().as_millis();
            total_compaction_time_ms += elapsed_ms;

            let turn_facts_count: usize = personal_memory.values().map(|v| v.len()).sum();
            total_facts_extracted += turn_facts_count;

            println!("  -> Duration: {} ms", elapsed_ms);
            println!(
                "  -> Transition Speech: {:?}",
                speech.as_deref().unwrap_or("None")
            );
            println!(
                "  -> Extracted Facts Count: {} across {} collections",
                turn_facts_count,
                personal_memory.len()
            );

            for (col, facts) in &personal_memory {
                println!("     [{}] {} facts: {:?}", col, facts.len(), facts);
            }

            // Verify System Prompt Consolidation
            assert!(
                !ctx.messages.is_empty(),
                "Context messages must not be empty after compaction"
            );
            assert_eq!(
                ctx.messages[0].role,
                Role::System,
                "Message 0 must be System role"
            );

            let sys_content = &ctx.messages[0].content;
            let has_session_history = sys_content.contains("<session_history>");
            let has_narrative_chain = sys_content.contains("<narrative_chain>");
            let has_recent_facts = sys_content.contains("<recent_compaction_facts>");

            println!("  -> System Prompt Consolidation Check:");
            println!("     - <session_history>: {}", has_session_history);
            println!("     - <narrative_chain>: {}", has_narrative_chain);
            println!("     - <recent_compaction_facts>: {}", has_recent_facts);

            assert!(
                has_session_history,
                "System message must contain <session_history>"
            );
            assert!(
                has_narrative_chain,
                "System message must contain <narrative_chain>"
            );
        }
    }

    let avg_latency = if compaction_count > 0 {
        total_compaction_time_ms as f64 / compaction_count as f64
    } else {
        0.0
    };

    println!("\n==================================================");
    println!("             INTEGRATION TEST SUMMARY             ");
    println!("==================================================");
    println!("Total Turns Processed:      {}", turns.len());
    println!("Total Compactions Triggered: {}", compaction_count);
    println!("Total Facts Extracted:       {}", total_facts_extracted);
    println!("Average Compaction Latency:  {:.2} ms", avg_latency);
    println!("Final Context Token Count:   {}", mgr.total_token_count());
    println!(
        "Final Context Utilization:   {:.2}%",
        mgr.context_utilization() * 100.0
    );
    println!("==================================================\n");

    // Save JSON test result report to sandbox/results/ingestion_test_results_session1.json
    let report_json = serde_json::json!({
        "data_path": data_path,
        "provider": provider_name,
        "model": model,
        "total_turns_processed": turns.len(),
        "total_compactions_triggered": compaction_count,
        "total_facts_extracted": total_facts_extracted,
        "avg_compaction_latency_ms": avg_latency,
        "final_token_count": mgr.total_token_count(),
        "final_utilization_pct": mgr.context_utilization() * 100.0,
    });

    let results_dir = Path::new("../../sandbox/results");
    if !results_dir.exists() {
        let _ = fs::create_dir_all(results_dir);
    }
    let output_json_path = results_dir.join("ingestion_test_results_session1.json");
    if let Ok(json_str) = serde_json::to_string_pretty(&report_json) {
        let _ = fs::write(&output_json_path, json_str);
        println!(
            "[TestReport] Saved test report JSON to {:?}",
            output_json_path
        );
    }

    assert!(
        compaction_count > 0,
        "At least 1 compaction should have been triggered for Session 1"
    );
}
