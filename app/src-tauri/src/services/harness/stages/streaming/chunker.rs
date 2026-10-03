/// Accumulates streaming token fragments and splits them into speakable clause/sentence chunks.
#[derive(Debug, Default, Clone)]
pub struct ClauseChunker {
    buffer: String,
    chunk_index: usize,
}

impl ClauseChunker {
    /// Creates an empty ClauseChunker instance.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            chunk_index: 0,
        }
    }

    /// Appends incoming text slice into the accumulator and returns any completed clauses.
    pub fn push_str(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        self.extract_chunks()
    }

    /// Flushes any remaining unpunctuated text in the buffer as a final speakable chunk.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim().to_string();
        self.buffer.clear();
        if trimmed.is_empty() {
            None
        } else {
            self.chunk_index += 1;
            Some(trimmed)
        }
    }

    /// Clears the internal chunker buffer unconditionally on cancellation or interruption.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.chunk_index = 0;
    }

    /// Returns a slice view of the current unconsumed buffer text.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Returns true if the chunker accumulator contains no text.
    pub fn is_empty(&self) -> bool {
        self.buffer.trim().is_empty()
    }

    /// Scans buffer text and locates valid sentence or dynamic runaway clause split positions.
    fn find_split_point(&mut self) -> Option<(usize, usize)> {
        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();
        if chars.is_empty() {
            return None;
        }

        // 1. Scan for primary sentence boundaries and linebreaks
        for i in 0..chars.len() {
            let (pos, c) = chars[i];

            // Explicit double newline (paragraph break) or single newline (list item)
            if c == '\n' {
                let is_double_newline = i + 1 < chars.len() && chars[i + 1].1 == '\n';
                let text_before = &self.buffer[..pos];
                let word_count = text_before.split_whitespace().count();
                if is_double_newline && word_count >= 2 {
                    let len = chars[i + 1].0 + chars[i + 1].1.len_utf8() - pos;
                    return Some((pos, len));
                } else if word_count >= 3 {
                    return Some((pos, c.len_utf8()));
                }
                continue;
            }

            // Question mark: terminal sentence boundary
            if c == '?' {
                let text_before = &self.buffer[..pos];
                let word_count = text_before.split_whitespace().count();
                if word_count >= 1 {
                    let len = extend_boundary(&self.buffer, pos, c.len_utf8(), '?');
                    return Some((pos, len));
                }
                continue;
            }

            // Gated Exclamation mark: split only if clause has substantive context (>= 4 words).
            // Short conversational openers ("Hey!", "Sure thing!") stay glued to next sentence.
            if c == '!' {
                let text_before = &self.buffer[..pos];
                let word_count = text_before.split_whitespace().count();
                if word_count >= 4 {
                    let len = extend_boundary(&self.buffer, pos, c.len_utf8(), '!');
                    return Some((pos, len));
                }
                continue;
            }

            // Period sentence boundary
            if c == '.' {
                let prev_is_dot = i > 0 && chars[i - 1].1 == '.';
                let next_is_dot = i + 1 < chars.len() && chars[i + 1].1 == '.';
                if prev_is_dot || next_is_dot {
                    continue; // Skip ellipsis (...)
                }

                let prev_is_digit = if i > 0 {
                    chars[i - 1].1.is_ascii_digit()
                } else {
                    false
                };
                let next_is_digit = if i + 1 < chars.len() {
                    chars[i + 1].1.is_ascii_digit()
                } else {
                    false
                };

                if prev_is_digit && next_is_digit {
                    continue; // Decimal guard
                }

                let text_before = &self.buffer[..pos];
                let last_word = text_before
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_matches(|p: char| !p.is_alphanumeric());

                if is_abbreviation(last_word) {
                    continue; // Abbreviation guard
                }

                let word_count = text_before.split_whitespace().count();
                if word_count >= 2 {
                    // Check if this sentence is a long compound sentence (>= 22 words)
                    // that contains an earlier natural conjunction seam.
                    if word_count >= 22 {
                        if let Some(seam) = self.find_compound_seam(&chars[..i], word_count) {
                            return Some(seam);
                        }
                    }

                    let len = extend_boundary(&self.buffer, pos, c.len_utf8(), '.');
                    return Some((pos, len));
                }
            }
        }

        // 2. Dynamic compound runaway seam search (buffer >= 22 words without terminal marks)
        let total_words = self.buffer.split_whitespace().count();
        if total_words >= 22 {
            if let Some(seam) = self.find_compound_seam(&chars, total_words) {
                return Some(seam);
            }
        }

        // 3. Hard runaway safety ceiling fallback (buffer >= 38 words)
        if total_words >= 38 {
            let mut count = 0;
            for (pos, c) in &chars {
                if c.is_whitespace() {
                    count += 1;
                    if count >= 30 {
                        return Some((*pos, c.len_utf8()));
                    }
                }
            }
        }

        None
    }

    /// Finds a natural syntactic seam (semicolon, colon, em-dash, or conjunction comma) in long clauses.
    fn find_compound_seam(
        &self,
        chars: &[(usize, char)],
        total_words: usize,
    ) -> Option<(usize, usize)> {
        for i in 0..chars.len() {
            let (pos, c) = chars[i];
            let text_before = &self.buffer[..pos];
            let words_before = text_before.split_whitespace().count();
            if words_before < 10 {
                continue;
            }
            if total_words - words_before < 4 {
                break;
            }

            // Semicolon or colon followed by space
            if (c == ';' || c == ':') && i + 1 < chars.len() && chars[i + 1].1.is_whitespace() {
                return Some((pos, c.len_utf8()));
            }

            // Em-dash or en-dash
            if c == '—' || c == '–' {
                return Some((pos, c.len_utf8()));
            }

            // Comma followed by a coordinating conjunction
            if c == ',' && i + 1 < chars.len() {
                let text_after = &self.buffer[pos + c.len_utf8()..];
                if is_conjunction_seam(text_after) {
                    return Some((pos, c.len_utf8()));
                }
                // If text is very long (>= 28 words), any comma after 16 words is acceptable
                if total_words >= 28 && words_before >= 16 {
                    return Some((pos, c.len_utf8()));
                }
            }
        }
        None
    }

    /// Extracts all completed speakable clause strings from the buffer.
    fn extract_chunks(&mut self) -> Vec<String> {
        let mut chunks = Vec::new();

        while !self.buffer.is_empty() {
            if let Some((pos, len)) = self.find_split_point() {
                let end = pos + len;
                let chunk = self.buffer[..end].trim().to_string();
                self.buffer = self.buffer[end..].trim_start().to_string();

                if !chunk.is_empty() {
                    chunks.push(chunk);
                    self.chunk_index += 1;
                }
            } else {
                break;
            }
        }

        chunks
    }
}

/// Helper extending boundary offset to consume any immediate trailing closing delimiters
/// (e.g. `."`, `?'`, `!)`, markdown symbols like `*` or `_`).
fn extend_boundary(buffer: &str, pos: usize, initial_len: usize, mark: char) -> usize {
    let mut end = pos + initial_len;
    for c in buffer[end..].chars() {
        if c == mark
            || c == '"'
            || c == '\''
            || c == '”'
            || c == '’'
            || c == ')'
            || c == ']'
            || c == '*'
            || c == '_'
        {
            end += c.len_utf8();
        } else {
            break;
        }
    }
    end - pos
}

/// Identifies coordinating conjunctions following a comma that make for natural speech seams.
fn is_conjunction_seam(text_after: &str) -> bool {
    let lower = text_after.trim_start().to_lowercase();
    const CONJUNCTIONS: &[&str] = &[
        "and ", "but ", "so ", "because ", "which ", "however ", "although ", "yet ", "or ", "then ",
    ];
    CONJUNCTIONS.iter().any(|&conj| lower.starts_with(conj))
}

/// Identifies standard honorifics, abbreviations, and version prefixes that suppress period splits.
fn is_abbreviation(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }

    let lower = word.to_lowercase();

    const ABBREVS: &[&str] = &[
        "dr", "mr", "mrs", "ms", "prof", "sr", "jr", "st", "vs", "e.g", "i.e", "etc", "approx",
        "dept", "fig", "ver", "vol", "inc", "ltd", "co", "no", "p", "pg", "pp",
    ];

    if ABBREVS.contains(&lower.as_str()) {
        return true;
    }

    if lower.starts_with('v') && lower.len() > 1 && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    if word.len() == 1 && word.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests strong terminators (? and .) split when clause meets boundary criteria.
    #[test]
    fn test_chunker_strong_terminators_split() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str(
            "Hello world and welcome here? This is a complete follow up sentence with more than ten words here.",
        );
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "Hello world and welcome here?");
        assert_eq!(
            chunks[1],
            "This is a complete follow up sentence with more than ten words here."
        );
        assert!(c.is_empty());
    }

    /// Tests newline with substantive context is treated as boundary.
    #[test]
    fn test_chunker_newline_splits() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Line one is ready\nLine two is ready");
        assert_eq!(chunks, vec!["Line one is ready"]);
        assert_eq!(c.buffer(), "Line two is ready");
    }

    /// Tests that commas in ordinary sentences do NOT split, preserving prosody.
    #[test]
    fn test_chunker_commas_preserved_in_normal_sentences() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hello, world here. Next sentence begins now.");
        assert_eq!(chunks, vec!["Hello, world here.", "Next sentence begins now."]);

        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str("This is a longer sentence, with multiple commas, and it continues to the end.");
        assert_eq!(chunks2, vec!["This is a longer sentence, with multiple commas, and it continues to the end."]);
    }

    /// Tests dynamic conjunction seam split on long runaway sentences (>= 22 words).
    #[test]
    fn test_chunker_dynamic_conjunction_seam_split() {
        let mut c = ClauseChunker::new();
        // 14 words before comma + "and" conjunction + 10 words after = 25 words total
        let chunks = c.push_str(
            "We have verified that the database cluster is completely healthy and operating within nominal parameters, and we will proceed with the deployment immediately.",
        );
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks[0],
            "We have verified that the database cluster is completely healthy and operating within nominal parameters,"
        );
        assert_eq!(
            chunks[1],
            "and we will proceed with the deployment immediately."
        );
    }

    /// Tests that natural periods are preserved without unnatural period morphing.
    #[test]
    fn test_chunker_prosody_preserves_periods() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hai Addy. I'm Vox. I help you do things today.");
        assert_eq!(chunks, vec!["Hai Addy.", "I'm Vox.", "I help you do things today."]);
        assert_eq!(c.flush(), None);
    }

    /// Tests period does not split on decimal like 3.14
    #[test]
    fn test_chunker_period_decimal_guard() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Value is 3.14 and continues");
        assert!(chunks.is_empty(), "decimal period must not split");
        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str("Value is 3.14 with enough words. Next sentence");
        assert_eq!(chunks2, vec!["Value is 3.14 with enough words."]);
    }

    /// Tests period does not split after known abbreviation.
    #[test]
    fn test_chunker_period_abbreviation_guard() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hello Dr. Smith is here");
        assert!(chunks.is_empty(), "abbreviation period must not split");
        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str("Hello Dr. Smith is here with us today. Next one");
        assert_eq!(chunks2, vec!["Hello Dr. Smith is here with us today."]);
    }

    /// Tests emergency word cap forces split at word 30 when no punctuation exists.
    #[test]
    fn test_chunker_emergency_word_cap() {
        let long = (0..40)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = ClauseChunker::new();
        let chunks = c.push_str(&long);
        assert!(!chunks.is_empty(), "bloat guard must emit chunk on >= 38 words");
        assert_eq!(chunks[0].split_whitespace().count(), 30);
    }

    /// Tests flush returns trimmed remainder and clears buffer.
    #[test]
    fn test_chunker_flush_and_clear() {
        let mut c = ClauseChunker::new();
        c.push_str("Hello world");
        assert_eq!(c.flush(), Some("Hello world".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.flush(), None);
        c.push_str("  trailing  ");
        assert_eq!(c.flush(), Some("trailing".to_string()));
        c.push_str("keep");
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.buffer(), "");
    }

    /// Tests is_abbreviation covers honorifics, version and single-letter cases.
    #[test]
    fn test_is_abbreviation_variants() {
        assert!(is_abbreviation("Dr"));
        assert!(is_abbreviation("dr"));
        assert!(is_abbreviation("e.g"));
        assert!(is_abbreviation("Mrs"));
        assert!(is_abbreviation("v2"));
        assert!(is_abbreviation("v10"));
        assert!(is_abbreviation("J"));
        assert!(!is_abbreviation("Hello"));
        assert!(!is_abbreviation(""));
        assert!(!is_abbreviation("world"));
    }

    /// Tests extract_chunks returns multiple clauses when multiple split points present.
    #[test]
    fn test_chunker_multiple_clauses() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str(
            "This is the first sentence that is long enough! And here is the second sentence that contains more than ten words easily? Third sentence is now significantly longer so that it easily satisfies the steady state minimum threshold.",
        );
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "This is the first sentence that is long enough!");
        assert_eq!(
            chunks[1],
            "And here is the second sentence that contains more than ten words easily?"
        );
        assert_eq!(
            chunks[2],
            "Third sentence is now significantly longer so that it easily satisfies the steady state minimum threshold."
        );
    }

    /// Tests short greetings or exclamations like 'Hey!' stay glued to following sentence.
    #[test]
    fn test_chunker_short_exclamation_stays_glued() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hey! What is up with you today?");
        // "Hey!" has only 1 word, so it stays attached to the full question
        assert_eq!(chunks, vec!["Hey! What is up with you today?"]);
    }

    /// Tests substantive exclamations (>= 4 words) split as full sentences.
    #[test]
    fn test_chunker_substantive_exclamation_splits() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("That is absolutely fantastic news! Let's get started now");
        assert_eq!(chunks, vec!["That is absolutely fantastic news!"]);
        assert_eq!(c.buffer(), "Let's get started now");
    }

    /// Tests trailing quotation marks and parentheses are consumed with the terminal mark.
    #[test]
    fn test_chunker_trailing_delimiters_consumed() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("He said, \"Hello world.\" Then he left");
        assert_eq!(chunks, vec!["He said, \"Hello world.\""]);
        assert_eq!(c.buffer(), "Then he left");
    }
}
