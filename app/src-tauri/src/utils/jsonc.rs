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

/// Deserializes an instance of type `T` from a JSON with Comments (JSONC) string.
pub fn from_jsonc_str<T: DeserializeOwned>(input: &str) -> Result<T, serde_json::Error> {
    let stripped = strip_comments(input);
    serde_json::from_str(&stripped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

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

        let sample: Sample = from_jsonc_str(jsonc).expect("Failed to parse jsonc with escaped quotes");
        assert_eq!(sample.name, "He said \"hello // world\" /* not a comment */");
    }
}
