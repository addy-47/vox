use std::path::Path;

use crate::services::harness::{ChatMessage, Role};

/// Supported LLM model families for architecture-specific prompt formatting and stop token handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    Gemma,
    Qwen,
    Llama3,
    Nemotron,
    Unknown,
}

impl ModelFamily {
    /// Detects model family from the model file path name.
    pub fn detect(path: &Path) -> Self {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if filename.contains("gemma") {
            ModelFamily::Gemma
        } else if filename.contains("qwen") {
            ModelFamily::Qwen
        } else if filename.contains("llama") {
            ModelFamily::Llama3
        } else if filename.contains("nemotron") {
            ModelFamily::Nemotron
        } else {
            ModelFamily::Unknown
        }
    }

    /// Formats a system prompt string according to family-specific special tokens.
    pub fn format_system_prompt(&self, system_prompt: &str) -> String {
        match self {
            ModelFamily::Gemma => format!("<|turn>system {}<turn|>\n", system_prompt),
            ModelFamily::Qwen => format!("<|im_start|>system\n{}<|im_end|>\n", system_prompt),
            ModelFamily::Llama3 => format!(
                "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|>",
                system_prompt
            ),
            ModelFamily::Nemotron => format!("<extra_id_0>System\n{}\n", system_prompt),
            ModelFamily::Unknown => format!("System: {}\n", system_prompt),
        }
    }

    /// Formats a user prompt string according to family-specific special tokens.
    pub fn format_user_prompt(&self, text: &str) -> String {
        match self {
            ModelFamily::Gemma => format!("<|turn>user {}<turn|>\n<|turn>model\n", text),
            ModelFamily::Qwen => format!(
                "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n",
                text
            ),
            ModelFamily::Llama3 => format!(
                "<|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                text
            ),
            ModelFamily::Nemotron => format!("<extra_id_1>User\n{}\n<extra_id_1>Assistant\n", text),
            ModelFamily::Unknown => format!("User: {}\nAssistant: ", text),
        }
    }

    /// Formats an entire conversation history into a structured prompt.
    pub fn format_conversation(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            match msg.role {
                Role::System => prompt.push_str(&self.format_system_prompt(&msg.content)),
                Role::User => prompt.push_str(&self.format_user_prompt(&msg.content)),
                Role::Assistant => match self {
                    ModelFamily::Gemma => prompt.push_str(&format!("{}<turn|>\n", msg.content)),
                    ModelFamily::Qwen => prompt.push_str(&format!("{}<|im_end|>\n", msg.content)),
                    ModelFamily::Llama3 => prompt.push_str(&format!("{}<|eot_id|>", msg.content)),
                    ModelFamily::Nemotron | ModelFamily::Unknown => {
                        prompt.push_str(&format!("{}\n", msg.content))
                    }
                },
            }
        }
        prompt
    }

    /// Returns family-specific generation stop sequence substrings.
    pub fn stop_sequences(&self) -> &'static [&'static str] {
        match self {
            ModelFamily::Gemma => &["<end", "<eos>", "<|turn>", "turn|>"],
            ModelFamily::Qwen => &[
                "<|im_end|>",
                "<|im_start|>",
                "<|turn|>",
                "<|endoftext|>",
                "<|end|>",
            ],
            ModelFamily::Llama3 => &["<|eot_id|>", "<|end_of_text|>"],
            ModelFamily::Nemotron => &["<extra_id_1>", "<extra_id_0>"],
            ModelFamily::Unknown => &["\nUser:", "\nSystem:"],
        }
    }

    /// Returns list of special token tags to strip from streaming assistant text output.
    pub fn tags_to_strip(&self) -> &'static [&'static str] {
        match self {
            ModelFamily::Gemma => &[
                "<|turn>",
                "<turn|>",
                "<channel|>",
                "system\n",
                "user\n",
                "model\n",
                "<end",
                "<eos>",
            ],
            ModelFamily::Qwen => &[
                "<|im_start|>",
                "<|im_end|>",
                "<|turn|>",
                "user\n",
                "assistant\n",
                "system\n",
                "thought\n",
                "<think>",
                "</think>",
            ],
            ModelFamily::Llama3 => &[
                "<|begin_of_text|>",
                "<|start_header_id|>",
                "<|end_header_id|>",
                "<|eot_id|>",
                "user\n",
                "assistant\n",
                "system\n",
            ],
            ModelFamily::Nemotron => &[
                "<extra_id_0>",
                "<extra_id_1>",
                "User\n",
                "Assistant\n",
                "System\n",
            ],
            ModelFamily::Unknown => &[],
        }
    }

    /// Strips model-specific special tokens and thinking blocks from output text.
    pub fn strip_tags_raw(&self, text: &str) -> String {
        let mut cleaned = text.to_string();

        match self {
            ModelFamily::Gemma => {
                static RE_GEMMA_TAGS: std::sync::OnceLock<regex::Regex> =
                    std::sync::OnceLock::new();
                let re_tags = RE_GEMMA_TAGS.get_or_init(|| {
                    regex::Regex::new(
                        r"<\|turn>|<turn\|>|<channel\|>|system\s*\n|user\s*\n|model\s*\n",
                    )
                    .unwrap()
                });
                cleaned = re_tags.replace_all(&cleaned, "").to_string();
                if cleaned.contains("<end") || cleaned.contains("<eos>") {
                    log::warn!("[LLM] Possible leaked eos tag detected: {:?}", cleaned);
                    return "".to_string();
                }
            }
            ModelFamily::Qwen => {
                static RE_QWEN_THINK: std::sync::OnceLock<regex::Regex> =
                    std::sync::OnceLock::new();
                let re_think = RE_QWEN_THINK
                    .get_or_init(|| regex::Regex::new(r"(?s)<think>.*?</think>").unwrap());
                cleaned = re_think.replace_all(&cleaned, "").to_string();

                let tags = [
                    "<|im_start|>",
                    "<|im_end|>",
                    "<|turn|>",
                    "user\n",
                    "assistant\n",
                    "system\n",
                    "thought\n",
                ];
                for tag in tags {
                    cleaned = cleaned.replace(tag, "");
                }
                cleaned = cleaned.replace("<think>", "").replace("</think>", "");
            }
            ModelFamily::Llama3 => {
                let tags = [
                    "<|begin_of_text|>",
                    "<|start_header_id|>",
                    "<|end_header_id|>",
                    "<|eot_id|>",
                    "user\n",
                    "assistant\n",
                    "system\n",
                ];
                for tag in tags {
                    cleaned = cleaned.replace(tag, "");
                }
            }
            ModelFamily::Nemotron => {
                let tags = [
                    "<extra_id_0>",
                    "<extra_id_1>",
                    "User\n",
                    "Assistant\n",
                    "System\n",
                ];
                for tag in tags {
                    cleaned = cleaned.replace(tag, "");
                }
            }
            ModelFamily::Unknown => {}
        }

        cleaned
    }

    /// Strips special tokens and trims whitespace from output text.
    pub fn strip_tags(&self, text: &str) -> String {
        self.strip_tags_raw(text).trim().to_string()
    }
}

/// Detects the length of a partial trailing tag suffix to hold back during streaming.
pub fn partial_tag_len(text: &str, tags: &[&str]) -> usize {
    for (i, _) in text.char_indices() {
        let suffix = &text[i..];
        for tag in tags {
            if tag.starts_with(suffix) && suffix != *tag {
                return suffix.len();
            }
        }
    }
    0
}
