use std::sync::mpsc;

use crate::services::llm::LlmStreamEvent;

const OPEN_TAG: &str = "<think>";
const CLOSE_TAG: &str = "</think>";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DemuxState {
    #[default]
    Normal,
    Thinking,
}

/// Ingress streaming state machine that strips inline `<think>...</think>` tags
/// and demuxes text into `LlmStreamEvent::Thinking` vs `LlmStreamEvent::Token`.
#[derive(Default)]
pub struct ThinkTagDemuxer {
    state: DemuxState,
    buffer: String,
}

impl ThinkTagDemuxer {
    /// Constructs a fresh think tag demuxer in `Normal` state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds an incoming token chunk into the state machine and dispatches demuxed stream events.
    pub fn feed(&mut self, text: &str, tx: &mpsc::Sender<LlmStreamEvent>) {
        self.buffer.push_str(text);
        self.drain_buffer(tx);
    }

    /// Flushes any pending buffered fragment at stream termination.
    pub fn flush(&mut self, tx: &mpsc::Sender<LlmStreamEvent>) {
        if !self.buffer.is_empty() {
            let remaining = std::mem::take(&mut self.buffer);
            match self.state {
                DemuxState::Normal => {
                    let _ = tx.send(LlmStreamEvent::Token(remaining));
                }
                DemuxState::Thinking => {
                    let _ = tx.send(LlmStreamEvent::Thinking(remaining));
                }
            }
        }
    }

    fn drain_buffer(&mut self, tx: &mpsc::Sender<LlmStreamEvent>) {
        loop {
            if self.buffer.is_empty() {
                break;
            }

            match self.state {
                DemuxState::Normal => {
                    if let Some(pos) = find_case_insensitive(&self.buffer, OPEN_TAG) {
                        let prefix = &self.buffer[..pos];
                        if !prefix.is_empty() {
                            let _ = tx.send(LlmStreamEvent::Token(prefix.to_string()));
                        }
                        self.buffer.drain(..pos + OPEN_TAG.len());
                        self.state = DemuxState::Thinking;
                    } else {
                        let matched_len = match_partial_suffix(&self.buffer, OPEN_TAG);

                        if matched_len > 0 {
                            let safe_len = self.buffer.len() - matched_len;
                            if safe_len > 0 {
                                let token = self.buffer[..safe_len].to_string();
                                self.buffer.drain(..safe_len);
                                let _ = tx.send(LlmStreamEvent::Token(token));
                            }
                            break;
                        } else {
                            let token = std::mem::take(&mut self.buffer);
                            let _ = tx.send(LlmStreamEvent::Token(token));
                            break;
                        }
                    }
                }
                DemuxState::Thinking => {
                    if let Some(pos) = find_case_insensitive(&self.buffer, CLOSE_TAG) {
                        let prefix = &self.buffer[..pos];
                        if !prefix.is_empty() {
                            let _ = tx.send(LlmStreamEvent::Thinking(prefix.to_string()));
                        }
                        self.buffer.drain(..pos + CLOSE_TAG.len());
                        self.state = DemuxState::Normal;
                    } else {
                        let matched_len = match_partial_suffix(&self.buffer, CLOSE_TAG);

                        if matched_len > 0 {
                            let safe_len = self.buffer.len() - matched_len;
                            if safe_len > 0 {
                                let thinking = self.buffer[..safe_len].to_string();
                                self.buffer.drain(..safe_len);
                                let _ = tx.send(LlmStreamEvent::Thinking(thinking));
                            }
                            break;
                        } else {
                            let thinking = std::mem::take(&mut self.buffer);
                            let _ = tx.send(LlmStreamEvent::Thinking(thinking));
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Finds the starting byte offset of an ASCII tag case-insensitively within a UTF-8 string.
/// Guarantees that the returned offset lies on a valid UTF-8 character boundary.
fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    let needle_bytes = needle.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    for i in 0..=(haystack_bytes.len() - needle_bytes.len()) {
        if haystack.is_char_boundary(i)
            && haystack.is_char_boundary(i + needle_bytes.len())
            && haystack_bytes[i..i + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes)
        {
            return Some(i);
        }
    }
    None
}

/// Matches a partial prefix of `needle` at the end of `haystack` case-insensitively.
/// Guarantees that the start of the matched suffix lies on a valid UTF-8 character boundary.
fn match_partial_suffix(haystack: &str, needle: &str) -> usize {
    let needle_bytes = needle.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    for len in (1..needle_bytes.len()).rev() {
        if haystack_bytes.len() >= len {
            let start = haystack_bytes.len() - len;
            if haystack.is_char_boundary(start) {
                let suffix = &haystack_bytes[start..];
                let prefix = &needle_bytes[..len];
                if suffix.eq_ignore_ascii_case(prefix) {
                    return len;
                }
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_tokens_without_thinking() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("Hello ", &tx);
        demuxer.feed("world!", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Token("Hello ".to_string()),
                LlmStreamEvent::Token("world!".to_string()),
            ]
        );
    }

    #[test]
    fn test_inline_thinking_single_chunk() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("<think>Let me ponder</think>Final answer", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Thinking("Let me ponder".to_string()),
                LlmStreamEvent::Token("Final answer".to_string()),
            ]
        );
    }

    #[test]
    fn test_split_tag_across_chunks() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("<th", &tx);
        demuxer.feed("ink>Pondering", &tx);
        demuxer.feed(" deeply</th", &tx);
        demuxer.feed("ink>Done!", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Thinking("Pondering".to_string()),
                LlmStreamEvent::Thinking(" deeply".to_string()),
                LlmStreamEvent::Token("Done!".to_string()),
            ]
        );
    }

    #[test]
    fn test_false_alarm_tag() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("<table class=\"data\">", &tx);
        demuxer.feed("value", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Token("<table class=\"data\">".to_string()),
                LlmStreamEvent::Token("value".to_string()),
            ]
        );
    }

    #[test]
    fn test_case_insensitive_uppercase_think_tags() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("<THINK>Uppercase reasoning</THINK>Answer", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Thinking("Uppercase reasoning".to_string()),
                LlmStreamEvent::Token("Answer".to_string()),
            ]
        );
    }

    #[test]
    fn test_case_insensitive_mixed_case_think_tags() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("<Think>Mixed reasoning</Think>Done", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Thinking("Mixed reasoning".to_string()),
                LlmStreamEvent::Token("Done".to_string()),
            ]
        );
    }

    #[test]
    fn test_multibyte_utf8_cjk_and_emojis() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("こんにちは 🤖 <THINK> 思案中... 🚀 </THINK> さようなら 🎉", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Token("こんにちは 🤖 ".to_string()),
                LlmStreamEvent::Thinking(" 思案中... 🚀 ".to_string()),
                LlmStreamEvent::Token(" さようなら 🎉".to_string()),
            ]
        );
    }

    #[test]
    fn test_multibyte_utf8_split_at_boundary() {
        let (tx, rx) = mpsc::channel();
        let mut demuxer = ThinkTagDemuxer::new();

        demuxer.feed("ユーザー: 🤖 <THI", &tx);
        demuxer.feed("NK>プログラミング</THINK> 完了 🚀", &tx);
        demuxer.flush(&tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            events,
            vec![
                LlmStreamEvent::Token("ユーザー: 🤖 ".to_string()),
                LlmStreamEvent::Thinking("プログラミング".to_string()),
                LlmStreamEvent::Token(" 完了 🚀".to_string()),
            ]
        );
    }
}
