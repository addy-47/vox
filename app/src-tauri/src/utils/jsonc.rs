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

    #[test]
    fn test_strip_comments_basic() {
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

    #[test]
    fn test_escaped_quotes_in_string() {
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

    #[test]
    fn test_trailing_commas_and_bom() {
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
