pub const MAX_SSE_BUFFER_BYTES: usize = 65_536;

/// Buffered line and SSE frame decoder.
#[derive(Default)]
pub struct SseDecoder {
    buffer: String,
}

impl SseDecoder {
    /// Creates a new SSE decoder instance.
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(4096),
        }
    }

    /// Decodes an incoming chunk of bytes, extracting all completed SSE payload lines.
    pub fn decode_chunk(&mut self, chunk: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&text);

        if self.buffer.len() > MAX_SSE_BUFFER_BYTES {
            log::warn!(
                "[SseDecoder] Buffer limit exceeded ({} bytes). Truncating to avoid OOM.",
                self.buffer.len()
            );
            self.buffer.clear();
            return Vec::new();
        }

        let mut lines = Vec::new();
        while let Some(idx) = self.buffer.find('\n') {
            let line = self.buffer[..idx].trim().to_string();
            self.buffer.drain(..=idx);

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(payload) = line.strip_prefix("data:") {
                let trimmed = payload.trim().to_string();
                if !trimmed.is_empty() {
                    lines.push(trimmed);
                }
            } else {
                lines.push(line);
            }
        }
        lines
    }

    /// Flushes any remaining bytes in the buffer as a trailing line.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim().to_string();
        self.buffer.clear();
        if trimmed.is_empty() {
            None
        } else if let Some(payload) = trimmed.strip_prefix("data:") {
            let p_trimmed = payload.trim().to_string();
            if p_trimmed.is_empty() {
                None
            } else {
                Some(p_trimmed)
            }
        } else {
            Some(trimmed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_decoder_chunking() {
        let mut decoder = SseDecoder::new();
        let chunk1 = b"data: {\"token\":\"hello\"}\r\n\r\ndata: {\"token\":\" world";
        let lines1 = decoder.decode_chunk(chunk1);
        assert_eq!(lines1, vec!["{\"token\":\"hello\"}"]);

        let chunk2 = b"\"}\n\ndata: [DONE]\n";
        let lines2 = decoder.decode_chunk(chunk2);
        assert_eq!(lines2, vec!["{\"token\":\" world\"}", "[DONE]"]);
    }
}
