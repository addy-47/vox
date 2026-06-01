// LLM Family Parsing and Formatting Unit Tests
// Run with: cargo test --test llm_family_test -- --nocapture

use std::path::Path;
use vox_lib::services::llm::llama_cpp::{ModelFamily};

#[test]
fn test_gemma_family_detection() {
    let path = Path::new("Gemma-4-E2B-Uncensored-HauhauCS-Aggressive-Q2_K_P.gguf");
    assert_eq!(ModelFamily::detect(path), ModelFamily::Gemma);

    let path_google = Path::new("google_gemma-4-E2B-it-Q4_K_M.gguf");
    assert_eq!(ModelFamily::detect(path_google), ModelFamily::Gemma);

    let path_e4b = Path::new("google_gemma-4-E4B-it-Q4_K_M.gguf");
    assert_eq!(ModelFamily::detect(path_e4b), ModelFamily::Gemma);
}

#[test]
fn test_qwen_family_detection() {
    let path_15b = Path::new("qwen2.5-1.5b-instruct-q4_k_m.gguf");
    assert_eq!(ModelFamily::detect(path_15b), ModelFamily::Qwen);

    let path_3b = Path::new("qwen2.5-3b-instruct-q4_k_m.gguf");
    assert_eq!(ModelFamily::detect(path_3b), ModelFamily::Qwen);

    let path_obliterated = Path::new("Qwen3-4B-OBLITERATED.Q4_K_M.gguf");
    assert_eq!(ModelFamily::detect(path_obliterated), ModelFamily::Qwen);

    let path_qwen35 = Path::new("Qwen3.5-4B-Q4_K_M.gguf");
    assert_eq!(ModelFamily::detect(path_qwen35), ModelFamily::Qwen);
}

#[test]
fn test_llama_family_detection() {
    let path = Path::new("Llama-3.2-3B-Instruct-Q6_K_L.gguf");
    assert_eq!(ModelFamily::detect(path), ModelFamily::Llama3);
}

#[test]
fn test_nemotron_family_detection() {
    let path = Path::new("NVIDIA-Nemotron3-Nano-4B-Q4_K_M.gguf");
    assert_eq!(ModelFamily::detect(path), ModelFamily::Nemotron);
}

#[test]
fn test_gemma_formatting_and_stripping() {
    let family = ModelFamily::Gemma;
    let system = "You are Vox.";
    let user = "Namaste!";
    
    // Prompt structure verification
    let prompt = family.format_prompt(user, system);
    assert_eq!(prompt, "<|turn>system You are Vox.<turn|>\n<|turn>user Namaste!<turn|>\n<|turn>model\n");

    // Tag stripping verification
    let dirty = "<|turn>system \nHello world!<turn|>\n";
    let clean = family.strip_tags(dirty);
    assert_eq!(clean, "Hello world!");

    // Leak / EOS warning returns empty
    let dirty_eos = "<|turn>system \nHello world!<end_of_turn>\n";
    let clean_eos = family.strip_tags(dirty_eos);
    assert_eq!(clean_eos, "");
}

#[test]
fn test_qwen_formatting_and_stripping() {
    let family = ModelFamily::Qwen;
    let system = "You are Vox.";
    let user = "Namaste!";

    // Prompt structure verification
    let prompt = family.format_prompt(user, system);
    assert_eq!(prompt, "<|im_start|>system\nYou are Vox.<|im_end|>\n<|im_start|>user\nNamaste!<|im_end|>\n<|im_start|>assistant\n");

    // Tag stripping verification
    let dirty = "<|im_start|>assistant\nSwagat hai!<|im_end|>";
    let clean = family.strip_tags(dirty);
    assert_eq!(clean, "Swagat hai!");

    let dirty_think = "<think>Mujhe sochna padega</think> Swagat hai!";
    let clean_think = family.strip_tags(dirty_think);
    assert_eq!(clean_think, "Swagat hai!");
}

#[test]
fn test_llama_formatting_and_stripping() {
    let family = ModelFamily::Llama3;
    let system = "You are Vox.";
    let user = "Namaste!";

    // Prompt structure verification
    let prompt = family.format_prompt(user, system);
    assert_eq!(prompt, "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nYou are Vox.<|eot_id|><|start_header_id|>user<|end_header_id|>\n\nNamaste!<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n");

    // Tag stripping verification
    let dirty = "<|start_header_id|>assistant<|end_header_id|>\n\nNamaste!<|eot_id|>";
    let clean = family.strip_tags(dirty);
    assert_eq!(clean, "Namaste!");
}

#[test]
fn test_nemotron_formatting_and_stripping() {
    let family = ModelFamily::Nemotron;
    let system = "You are Vox.";
    let user = "Namaste!";

    // Prompt structure verification
    let prompt = family.format_prompt(user, system);
    assert_eq!(prompt, "<extra_id_0>System\nYou are Vox.\n<extra_id_1>User\nNamaste!\n<extra_id_1>Assistant\n");

    // Tag stripping verification
    let dirty = "<extra_id_1>Assistant\nNamaste!<extra_id_1>";
    let clean = family.strip_tags(dirty);
    assert_eq!(clean, "Namaste!");
}
