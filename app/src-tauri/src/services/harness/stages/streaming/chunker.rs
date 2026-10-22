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

                // Decimal guard: if preceded by a digit and either followed by a digit
                // OR at the very end of the streaming buffer (lookahead needed to confirm decimal vs sentence end).
                if prev_is_digit && (i + 1 == chars.len() || chars[i + 1].1.is_ascii_digit()) {
                    continue; // Decimal guard
                }

                // Sentence ending period requires either end-of-sentence closing delimiter or whitespace after period
                // when not at end of buffer, to avoid splitting mid-token abbreviations or filenames.
                if i + 1 < chars.len() {
                    let next_char = chars[i + 1].1;
                    if !next_char.is_whitespace()
                        && next_char != '"'
                        && next_char != '\''
                        && next_char != '”'
                        && next_char != '’'
                        && next_char != ')'
                        && next_char != ']'
                        && next_char != '*'
                    {
                        continue;
                    }
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
        "and ",
        "but ",
        "so ",
        "because ",
        "which ",
        "however ",
        "although ",
        "yet ",
        "or ",
        "then ",
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

    /// PURPOSE:  A clause must be emitted at a strong terminator, or the speech runs on without a pause.
    /// SCENARIO: A question and a long follow-up sentence are streamed into the chunker.
    /// ASSERTS:  Two clauses are emitted, the first ends with the question, and the buffer is empty.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Make find_split_point ignore the '?' and '!' terminators.
    #[test]
    fn chunker_strong_terminator_splits_the_clause() {
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

    /// PURPOSE:  A newline must end the current clause, or list items are spoken as one run.
    /// SCENARIO: Two lines are streamed in one push.
    /// ASSERTS:  The first line is emitted as a clause and the second line stays buffered.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Remove the newline rule from the split search.
    #[test]
    fn chunker_newline_ends_the_previous_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Line one is ready\nLine two is ready");
        assert_eq!(chunks, vec!["Line one is ready"]);
        assert_eq!(c.buffer(), "Line two is ready");
    }

    /// PURPOSE:  A comma must not break the prosody of an ordinary sentence.
    /// SCENARIO: A short sentence with one comma and a long sentence with three commas are streamed.
    /// ASSERTS:  Each input stays a single clause with its commas intact.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Split at every comma.
    #[test]
    fn chunker_comma_in_an_ordinary_sentence_does_not_split() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hello, world here. Next sentence begins now.");
        assert_eq!(
            chunks,
            vec!["Hello, world here.", "Next sentence begins now."]
        );

        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str(
            "This is a longer sentence, with multiple commas, and it continues to the end.",
        );
        assert_eq!(
            chunks2,
            vec!["This is a longer sentence, with multiple commas, and it continues to the end."]
        );
    }

    /// PURPOSE:  A long compound sentence must split at a natural conjunction seam, or it buffers forever.
    /// SCENARIO: A 25-word sentence with a comma and the conjunction "and" is streamed.
    /// ASSERTS:  Two clauses are emitted, the first ending at the comma before the conjunction.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Lower the compound-sentence word threshold below 22.
    #[test]
    fn chunker_long_runaway_splits_at_the_conjunction_seam() {
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

    /// PURPOSE:  Sentence periods must survive chunking, or the spoken prosody changes.
    /// SCENARIO: Three short sentences are streamed.
    /// ASSERTS:  Three clauses are emitted, each keeping its trailing period, and the flush returns nothing.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Drop the trailing period when a clause is emitted.
    #[test]
    fn chunker_natural_periods_are_preserved_in_the_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hai Addy. I'm Vox. I help you do things today.");
        assert_eq!(
            chunks,
            vec!["Hai Addy.", "I'm Vox.", "I help you do things today."]
        );
        assert_eq!(c.flush(), None);
    }

    /// PURPOSE:  A decimal point or a version number must not end the sentence.
    /// SCENARIO: "3.14" and a split "Gemini 1." / "5 Pro" are streamed.
    /// ASSERTS:  No clause is emitted at the decimal or the version period.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Remove the digit-after-period guard from find_split_point.
    #[test]
    fn chunker_decimal_period_does_not_end_the_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Value is 3.14 and continues");
        assert!(chunks.is_empty(), "decimal period must not split");
        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str("Value is 3.14 with enough words. Next sentence");
        assert_eq!(chunks2, vec!["Value is 3.14 with enough words."]);

        // Streaming scenario: "Gemini 1." arrives first, followed by "5 Pro"
        let mut c3 = ClauseChunker::new();
        let chunks3_a = c3.push_str("Google rolled out Gemini 1.");
        assert!(
            chunks3_a.is_empty(),
            "Trailing period after digit must not split early during streaming"
        );
        let chunks3_b = c3.push_str("5 Pro, which features a massive window.");
        assert_eq!(
            chunks3_b,
            vec!["Google rolled out Gemini 1.5 Pro, which features a massive window."]
        );
    }

    /// PURPOSE:  An abbreviation must not end the sentence.
    /// SCENARIO: "Hello Dr. Smith is here" is streamed, then a longer variant.
    /// ASSERTS:  No clause is emitted at the abbreviation period.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Remove the is_abbreviation guard from the split search.
    #[test]
    fn chunker_abbreviation_period_does_not_end_the_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hello Dr. Smith is here");
        assert!(chunks.is_empty(), "abbreviation period must not split");
        let mut c2 = ClauseChunker::new();
        let chunks2 = c2.push_str("Hello Dr. Smith is here with us today. Next one");
        assert_eq!(chunks2, vec!["Hello Dr. Smith is here with us today."]);
    }

    /// PURPOSE:  Unpunctuated speech must not grow the buffer without bound.
    /// SCENARIO: Forty words without punctuation are streamed.
    /// ASSERTS:  A chunk is emitted and the first chunk holds exactly 30 words.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Raise the emergency word ceiling from 30 to 38.
    #[test]
    fn chunker_unpunctuated_text_is_capped_at_thirty_words() {
        let long = (0..40)
            .map(|i| format!("word{}", i))
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = ClauseChunker::new();
        let chunks = c.push_str(&long);
        assert!(
            !chunks.is_empty(),
            "bloat guard must emit chunk on >= 38 words"
        );
        assert_eq!(chunks[0].split_whitespace().count(), 30);
    }

    /// PURPOSE:  The flush must hand over the buffered tail, and clear must drop it.
    /// SCENARIO: Text is pushed, flushed, pushed again, and cleared.
    /// ASSERTS:  The flush returns the trimmed tail once, a second flush returns None, and clear empties the buffer.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Make flush keep the buffer content after returning it.
    #[test]
    fn chunker_flush_returns_the_tail_and_clear_empties_the_buffer() {
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

    /// PURPOSE:  The abbreviation table must cover the tokens that would otherwise end a sentence early.
    /// SCENARIO: The abbreviation predicate is called with honorifics, a dotted prefix, a version and a single letter.
    /// ASSERTS:  Dr, dr, e.g, Mrs, v2, v10 and J are abbreviations; Hello, the empty string and world are not.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Remove the version-number branch from is_abbreviation.
    #[test]
    fn abbreviation_lookup_accepts_honorifics_versions_and_initials() {
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

    /// PURPOSE:  Several terminators in one push must each produce their own clause.
    /// SCENARIO: A sentence with an exclamation, a question and a long statement is streamed.
    /// ASSERTS:  Exactly three clauses are emitted, in the original order.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Emit only the first clause found in a push.
    #[test]
    fn chunker_three_terminators_emit_three_clauses() {
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

    /// PURPOSE:  A one-word exclamation is a conversational opener and must not split the sentence.
    /// SCENARIO: "Hey! What is up with you today?" is streamed.
    /// ASSERTS:  One clause is emitted containing both the exclamation and the question.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Remove the exclamation word-count gate.
    #[test]
    fn chunker_short_exclamation_stays_glued_to_the_next_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("Hey! What is up with you today?");
        // "Hey!" has only 1 word, so it stays attached to the full question
        assert_eq!(chunks, vec!["Hey! What is up with you today?"]);
    }

    /// PURPOSE:  A full-clause exclamation must end the clause.
    /// SCENARIO: "That is absolutely fantastic news! Let's get started now" is streamed.
    /// ASSERTS:  The exclamation sentence is emitted and the remainder stays buffered.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Raise the exclamation word gate above 4.
    #[test]
    fn chunker_substantive_exclamation_ends_the_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("That is absolutely fantastic news! Let's get started now");
        assert_eq!(chunks, vec!["That is absolutely fantastic news!"]);
        assert_eq!(c.buffer(), "Let's get started now");
    }

    /// PURPOSE:  Quotation marks and parentheses must not leak into the next clause.
    /// SCENARIO: A quoted sentence inside a larger utterance is streamed.
    /// ASSERTS:  The emitted clause ends with the closing quote and the rest stays buffered.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Stop consuming trailing delimiters after the terminal mark.
    #[test]
    fn chunker_trailing_delimiters_are_consumed_with_the_clause() {
        let mut c = ClauseChunker::new();
        let chunks = c.push_str("He said, \"Hello world.\" Then he left");
        assert_eq!(chunks, vec!["He said, \"Hello world.\""]);
        assert_eq!(c.buffer(), "Then he left");
    }

    /// Feeds the text through the chunker and returns the clauses plus the flushed tail.
    fn chunk(text: &str) -> Vec<String> {
        let mut chunker = ClauseChunker::new();
        let mut clauses = chunker.push_str(text);
        if let Some(rest) = chunker.flush() {
            if !rest.trim().is_empty() {
                clauses.push(rest);
            }
        }
        clauses
    }
/// PURPOSE:  The same words must become the same clauses no matter how the model
///           fragments its tokens.
/// SCENARIO: The same sentence is fed as word tokens and as one chunk.
/// ASSERTS:  Both fragmentations yield identical clauses.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make the chunker depend on the incoming token boundaries.
#[test]
fn clause_boundaries_do_not_depend_on_token_fragmentation() {
    let text = "Good morning everyone, today we are testing. Dr. Smith is here. How are you doing today? Let's verify this now.";

    let mut chunker = ClauseChunker::new();
    let mut from_words: Vec<String> = Vec::new();
    for word in text.split_inclusive(' ') {
        from_words.extend(chunker.push_str(word));
    }
    if let Some(rest) = chunker.flush() {
        if !rest.trim().is_empty() {
            from_words.push(rest);
        }
    }

    let from_chunks = chunk(text);

    assert!(!from_words.is_empty(), "the chunker must produce clauses");
    assert_eq!(
        from_words, from_chunks,
        "clauses must be identical regardless of token fragmentation"
    );
    assert!(chunker.is_empty(), "the buffer must be empty after a flush");
}

/// PURPOSE:  A runaway sentence without punctuation must not grow the buffer forever.
/// SCENARIO: 42 unpunctuated words are streamed.
/// ASSERTS:  The text is split into a 30-word emergency chunk and the remainder.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Raise the emergency ceiling from 30 words.
#[test]
fn unpunctuated_runaway_is_force_chunked_at_thirty_words() {
    let words: Vec<String> = (0..42)
        .map(|i| format!("word{} ", i))
        .collect();
    let text = words.concat();

    let clauses = chunk(&text);

    assert_eq!(clauses.len(), 2, "42 words must become two clauses");
    assert_eq!(clauses[0].split_whitespace().count(), 30);
    assert_eq!(clauses[1].split_whitespace().count(), 12);
}

/// PURPOSE:  A comma inside a short sentence must not break the prosody.
/// SCENARIO: A short sentence with a comma is streamed.
/// ASSERTS:  It stays one clause.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Split at every comma.
#[test]
fn a_short_sentence_with_a_comma_stays_one_clause() {
    let clauses = chunk("Hello my friend, how are you today?");
    assert_eq!(clauses.len(), 1);
    assert_eq!(clauses[0], "Hello my friend, how are you today?");
}

/// PURPOSE:  The compound seam must not fire below the 22 word threshold.
/// SCENARIO: An 18 word sentence with a comma after the tenth word is streamed.
/// ASSERTS:  It stays one clause: the seam search needs a long sentence.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Lower the compound-sentence threshold from 22 words to 2.
#[test]
fn a_mid_length_sentence_with_a_comma_stays_one_clause() {
    let text =
        "The deployment pipeline has finished the whole database migration tonight, and the team celebrated the successful release.";
    let clauses = chunk(text);
    assert_eq!(
        clauses.len(),
        1,
        "a sentence below the compound threshold must not split, got {:?}",
        clauses
    );
}

/// PURPOSE:  A long compound sentence must be split at a natural seam.
/// SCENARIO: A 24-word sentence with a comma and a coordinating conjunction is streamed.
/// ASSERTS:  It becomes two clauses split at the conjunction seam.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Lower the compound-sentence threshold below 22 words.
#[test]
fn a_long_compound_sentence_splits_at_the_conjunction_seam() {
    let text = "We have carefully verified that all system components and background services are running smoothly, and we can proceed with the live rollout now.";
    let clauses = chunk(text);

    assert_eq!(clauses.len(), 2, "a 24-word compound must split once");
    assert!(clauses[0].ends_with("running smoothly,"));
    assert_eq!(clauses[1], "and we can proceed with the live rollout now.");
}

/// PURPOSE:  An abbreviation must not end the sentence.
/// SCENARIO: "Dr. Smith is here." is streamed.
/// ASSERTS:  It stays one clause.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the abbreviation guard.
#[test]
fn an_abbreviation_does_not_end_the_sentence() {
    let clauses = chunk("Dr. Smith is here.");
    assert_eq!(clauses.len(), 1, "the period in Dr. must not split");
    assert_eq!(clauses[0], "Dr. Smith is here.");
}

/// PURPOSE:  A decimal number must not end the sentence.
/// SCENARIO: "The value is 3.14 exactly." is streamed.
/// ASSERTS:  It stays one clause.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the decimal guard.
#[test]
fn a_decimal_number_does_not_end_the_sentence() {
    let clauses = chunk("The value is 3.14 exactly.");
    assert_eq!(clauses.len(), 1, "the decimal point must not split");
    assert_eq!(clauses[0], "The value is 3.14 exactly.");
}

/// PURPOSE:  A filename must not be split at its dot.
/// SCENARIO: "Open config.toml now." is streamed.
/// ASSERTS:  It stays one clause.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Split at a period that is not followed by whitespace.
#[test]
fn a_period_inside_a_word_does_not_split() {
    let clauses = chunk("Open config.toml now.");
    assert_eq!(clauses.len(), 1, "a mid-token period must not split");
}

/// PURPOSE:  A question ends a sentence even after a single word.
/// SCENARIO: "Why? Because." is streamed.
/// ASSERTS:  It becomes two clauses.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Require two words before a question mark splits.
#[test]
fn a_question_mark_ends_the_clause() {
    let clauses = chunk("Why? Because.");
    assert_eq!(clauses.len(), 2);
    assert_eq!(clauses[0], "Why?");
}

/// PURPOSE:  A short exclamation is a conversational opener and must not split.
/// SCENARIO: "Hey! Sure thing." is streamed.
/// ASSERTS:  It stays one clause.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Split at every exclamation mark.
#[test]
fn a_short_exclamation_does_not_split() {
    let clauses = chunk("Hey! Sure thing.");
    assert_eq!(clauses.len(), 1, "a three-word opener must stay glued");
}

/// PURPOSE:  An exclamation after a full clause ends it.
/// SCENARIO: "That was a long answer indeed! Next." is streamed.
/// ASSERTS:  It becomes two clauses.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Raise the exclamation word gate.
#[test]
fn a_long_exclamation_ends_the_clause() {
    let clauses = chunk("That was a long answer indeed! Next.");
    assert_eq!(clauses.len(), 2);
    assert_eq!(clauses[0], "That was a long answer indeed!");
}

/// PURPOSE:  A newline ends a list item.
/// SCENARIO: A three-word line followed by a newline is streamed.
/// ASSERTS:  It becomes two clauses.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the newline split rule.
#[test]
fn a_newline_ends_a_list_item() {
    let clauses = chunk("first item line\nsecond item line");
    assert_eq!(clauses.len(), 2, "a newline must end the list item");
}

/// PURPOSE:  A semicolon inside a long clause is a natural seam.
/// SCENARIO: A 25-word sentence with a semicolon is streamed.
/// ASSERTS:  It splits at the semicolon.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Drop the semicolon from the compound seam search.
#[test]
fn a_semicolon_in_a_long_clause_is_a_split_point() {
    let text = "The rollout is scheduled for tomorrow morning after the review board has approved the final checklist ; and signed the release document for production use today.";
    let clauses = chunk(text);

    assert!(clauses.len() >= 2, "a long clause with a semicolon must split");
    assert!(
        clauses[0].ends_with(';'),
        "the split must land on the semicolon, got {:?}",
        clauses[0]
    );
}

/// PURPOSE:  The flush must hand over the unpunctuated tail exactly once.
/// SCENARIO: An unpunctuated tail is buffered and then flushed.
/// ASSERTS:  The tail is returned and the buffer is empty afterwards.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Keep the buffer after a flush.
#[test]
fn flush_returns_the_tail_and_empties_the_buffer() {
    let mut chunker = ClauseChunker::new();
    assert!(chunker.push_str("a tail without punctuation").is_empty());

    let tail = chunker.flush().expect("the tail must be returned");
    assert_eq!(tail, "a tail without punctuation");
    assert!(chunker.is_empty(), "the buffer must be empty after a flush");
    assert!(chunker.flush().is_none(), "a second flush must return nothing");
}

/// PURPOSE:  A cancelled turn must not leave buffered speech behind.
/// SCENARIO: A cancelled turn clears the chunker buffer.
/// ASSERTS:  Clear empties the buffer.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Make clear() a no-op.
#[test]
fn clear_drops_the_buffered_text() {
    let mut chunker = ClauseChunker::new();
    chunker.push_str("half a sentence");

    chunker.clear();

    assert!(chunker.is_empty(), "clear must drop the buffered text");
}
}
