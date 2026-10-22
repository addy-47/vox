use serde::de::DeserializeOwned;

/// Strips single-line (`// ...`) and multi-line (`/* ... */`) comments from a JSONC string,
/// strictly preserving string literals (including escape sequences like `\"` and `\\`) and line breaks.
pub fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' {
            match chars.peek() {
                Some('/') => {
                    // Line comment: skip until newline or EOF
                    chars.next(); // consume second '/'
                    for next_c in chars.by_ref() {
                        if next_c == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    // Block comment: skip until "*/" or EOF
                    chars.next(); // consume '*'
                    while let Some(next_c) = chars.next() {
                        if next_c == '*' && chars.peek() == Some(&'/') {
                            chars.next(); // consume '/'
                            break;
                        } else if next_c == '\n' {
                            out.push('\n');
                        }
                    }
                }
                _ => {
                    out.push(c);
                }
            }
        } else {
            out.push(c);
        }
    }

    out
}

/// Drops `,` that is followed only by whitespace and then `}` or `]`,
/// strictly preserving string literals (including escaped quotes).
pub fn strip_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }

        if c == ',' {
            let mut is_trailing = false;
            let mut clone_iter = chars.clone();
            for next_c in clone_iter.by_ref() {
                if next_c.is_whitespace() {
                    continue;
                }
                if next_c == '}' || next_c == ']' {
                    is_trailing = true;
                }
                break;
            }

            if is_trailing {
                continue;
            }
        }

        out.push(c);
    }

    out
}

/// Deserializes an instance of type `T` from a JSON with Comments (JSONC) string.
/// Tolerates UTF-8 BOM (`\u{feff}`), single/multi-line comments, and trailing commas.
pub fn from_jsonc_str<T: DeserializeOwned>(input: &str) -> Result<T, serde_json::Error> {
    let mut s = strip_comments(input);
    if s.starts_with('\u{feff}') {
        s.remove(0);
    }
    let cleaned = strip_trailing_commas(&s);
    serde_json::from_str(&cleaned)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Sample {
        name: String,
        url: String,
        count: i32,
    }

    /// PURPOSE:  A JSONC file must parse with its comments removed, or the model config cannot be read.
    /// SCENARIO: A document with line, inline and block comments is parsed.
    /// ASSERTS:  The name, url and count fields are read with their original values.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Strip comment markers inside string values in from_jsonc_str.
    #[test]
    fn jsonc_parser_strips_comments_without_touching_string_content() {
        let jsonc = r#"
        // Top-level comment
        {
            "name": "Vox", // inline comment
            /* Block comment
               over multiple lines */
            "url": "http://100.67.98.126:11435", // URL with // must not be stripped
            "count": 42 /* trailing block comment */
        }
        // Bottom-level comment
        "#;

        let sample: Sample = from_jsonc_str(jsonc).expect("Failed to parse sample jsonc");
        assert_eq!(sample.name, "Vox");
        assert_eq!(sample.url, "http://100.67.98.126:11435");
        assert_eq!(sample.count, 42);
    }

    /// PURPOSE:  Escaped quotes and comment markers inside a string value must survive.
    /// SCENARIO: A value containing an escaped quote and both comment markers is parsed.
    /// ASSERTS:  The parsed name is the original string.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Treat an escaped quote as the end of the string in from_jsonc_str.
    #[test]
    fn jsonc_parser_keeps_escaped_quotes_inside_strings() {
        let jsonc = r#"
        {
            "name": "He said \"hello // world\" /* not a comment */",
            "url": "https://example.com",
            "count": 1
        }
        "#;

        let sample: Sample =
            from_jsonc_str(jsonc).expect("Failed to parse jsonc with escaped quotes");
        assert_eq!(
            sample.name,
            "He said \"hello // world\" /* not a comment */"
        );
    }

    /// PURPOSE:  A byte-order mark and trailing commas must not stop the parse.
    /// SCENARIO: A BOM-prefixed object with a trailing comma and an array with a trailing comma are parsed.
    /// ASSERTS:  Both parse, with the expected fields and the three array items.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Reject a document that starts with the BOM byte in from_jsonc_str.
    #[test]
    fn jsonc_parser_accepts_a_bom_and_trailing_commas() {
        let jsonc = "\u{feff}{\n  \"name\": \"Vox\",\n  \"url\": \"https://example.com\",\n  \"count\": 10,\n}";
        let sample: Sample =
            from_jsonc_str(jsonc).expect("Failed to parse jsonc with BOM and trailing comma");
        assert_eq!(sample.name, "Vox");
        assert_eq!(sample.count, 10);

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct ArraySample {
            items: Vec<String>,
        }

        let array_jsonc = "{\n  \"items\": [\"a\", \"b\", \"c\",],\n}";
        let parsed: ArraySample =
            from_jsonc_str(array_jsonc).expect("Failed to parse array with trailing comma");
        assert_eq!(parsed.items, vec!["a", "b", "c"]);
    }
}
