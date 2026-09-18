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

    /// Returns the current adaptive (w_min, w_target, w_max) word count thresholds based on chunk index.
    fn current_word_thresholds(&self) -> (usize, usize, usize) {
        match self.chunk_index {
            0 => (5, 8, 12),
            1 => (10, 15, 20),
            _ => (16, 24, 32),
        }
    }

    /// Scans buffer text and locates valid clause or sentence split byte positions.
    fn find_split_point(&mut self) -> Option<(usize, usize)> {
        let (w_min, w_target, w_max) = self.current_word_thresholds();
        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();

        for i in 0..chars.len() {
            let (pos, c) = chars[i];

            // Primary sentence boundaries: newline, question mark, exclamation mark
            if c == '\n' || c == '?' || c == '!' {
                return Some((pos, c.len_utf8()));
            }

            // Sub-clause boundaries: comma, semicolon, colon, em-dash
            if c == ',' || c == ';' || c == ':' || c == '—' || c == '–' {
                let text_before = &self.buffer[..pos];
                let word_count = text_before.split_whitespace().count();
                if word_count >= w_min {
                    return Some((pos, c.len_utf8()));
                }
                continue;
            }

            // Period sentence boundary
            if c == '.' {
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
                    continue;
                }

                let text_before = &self.buffer[..pos];
                let last_word = text_before
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_matches(|p: char| !p.is_alphanumeric());

                if is_abbreviation(last_word) {
                    continue;
                }

                let word_count = text_before.split_whitespace().count();
                if word_count >= w_min {
                    return Some((pos, c.len_utf8()));
                } else if i + 1 < chars.len() && chars[i + 1].1.is_whitespace() {
                    // Prosody morphing: premature period with < w_min words (e.g. "Hai Addy.")
                    // Rewrite '.' to ',' so StyleTTS2 maintains rising pitch contour and bundles forward.
                    self.buffer.replace_range(pos..pos + 1, ",");
                }
            }
        }

        // Emergency boundary fallback: only when no punctuation split point exists and buffer exceeds w_max words
        let words: Vec<&str> = self.buffer.split_whitespace().collect();
        if words.len() >= w_max {
            let mut count = 0;
            for (pos, c) in &chars {
                if c.is_whitespace() {
                    count += 1;
                    if count >= w_target {
                        return Some((*pos, c.len_utf8()));
                    }
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
                self.buffer = self.buffer[end..].to_string();

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

    /// Tests strong terminators (? ! newline) split when clause meets boundary criteria.
    #[test]
    fn test_chunker_strong_terminators_split() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str(
            "Hello world? This is a complete follow up sentence with more than ten words here.",
        );
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "Hello world?");
        assert_eq!(
            chunks[1],
            "This is a complete follow up sentence with more than ten words here."
        );
        assert!(c.is_empty());
    }

    /// Tests newline is treated as strong terminator.
    #[test]
    fn test_chunker_newline_splits() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Line one\nLine two");
        assert_eq!(chunks, vec!["Line one"]);
        assert_eq!(c.buffer(), "Line two");
    }

    /// Tests comma requires >=5 words before split for prosody pacing.
    #[test]
    fn test_chunker_comma_gated_by_word_count() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hello, world here");
        assert!(chunks.is_empty(), "short comma must not split");

        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str("This is a longer sentence, and it continues");
        assert_eq!(chunks2, vec!["This is a longer sentence,"]);
    }

    /// Tests prosody morphing converts premature periods (< 5 words) into commas to preserve rising intonation.
    #[test]
    fn test_chunker_prosody_morphing() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hai Addy. I'm Vox. I help you do things today.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hai Addy, I'm Vox, I help you do things today.");
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

    /// Tests emergency word cap forces split at target words.
    #[test]
    fn test_chunker_emergency_word_cap() {
        let long = (0..30)
            .map(|i| format!("w{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = ClauseChunker::new();
        let chunks = c.push_str(&long);
        assert!(!chunks.is_empty(), "bloat guard must emit chunk");
        assert_eq!(chunks[0].split_whitespace().count(), 8);
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
        let chunks =
            c.push_str("First sentence! Second? Third sentence is now significantly longer so that it easily satisfies the steady state minimum threshold of sixteen words.");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "First sentence!");
        assert_eq!(chunks[1], "Second?");
        assert_eq!(
            chunks[2],
            "Third sentence is now significantly longer so that it easily satisfies the steady state minimum threshold of sixteen words."
        );
    }
}
