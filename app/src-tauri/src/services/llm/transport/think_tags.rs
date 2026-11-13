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
                    if let Some(pos) = self.buffer.find(OPEN_TAG) {
                        let prefix = &self.buffer[..pos];
                        if !prefix.is_empty() {
                            let _ = tx.send(LlmStreamEvent::Token(prefix.to_string()));
                        }
                        self.buffer.drain(..pos + OPEN_TAG.len());
                        self.state = DemuxState::Thinking;
                    } else {
                        let mut matched_len = 0;
                        for len in (1..OPEN_TAG.len()).rev() {
                            let prefix = &OPEN_TAG[..len];
                            if self.buffer.ends_with(prefix) {
                                matched_len = len;
                                break;
                            }
                        }

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
                    if let Some(pos) = self.buffer.find(CLOSE_TAG) {
                        let prefix = &self.buffer[..pos];
                        if !prefix.is_empty() {
                            let _ = tx.send(LlmStreamEvent::Thinking(prefix.to_string()));
                        }
                        self.buffer.drain(..pos + CLOSE_TAG.len());
                        self.state = DemuxState::Normal;
                    } else {
                        let mut matched_len = 0;
                        for len in (1..CLOSE_TAG.len()).rev() {
                            let prefix = &CLOSE_TAG[..len];
                            if self.buffer.ends_with(prefix) {
                                matched_len = len;
                                break;
                            }
                        }

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
}
