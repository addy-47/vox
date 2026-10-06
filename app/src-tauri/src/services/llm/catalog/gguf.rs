use std::{fs::File, io::Read, path::Path};

/// Maximum header bytes read from the file start. Model metadata always lives
/// at the head of a GGUF file; tensor data follows. Two megabytes covers every
/// real-world metadata block with wide margin.
const HEADER_READ_LIMIT: u64 = 2 * 1024 * 1024;

/// Declared model facts read from a GGUF file header without loading weights.
#[derive(Debug, Clone, Default)]
pub struct GgufFacts {
    pub architecture: Option<String>,
    pub context_length: Option<u32>,
}

/// Reads declared facts from a GGUF file header. Returns None when the file is
/// missing, unreadable, or not a supported GGUF version. Partial facts are
/// returned when the header block ends before every key is found.
pub fn read_gguf_facts(path: &Path) -> Option<GgufFacts> {
    let file = File::open(path).ok()?;
    let mut buf = Vec::new();
    file
        .take(HEADER_READ_LIMIT)
        .read_to_end(&mut buf)
        .ok()?;
    parse_gguf_header(&buf)
}

/// Parses GGUF metadata key-values from a header byte buffer.
fn parse_gguf_header(buf: &[u8]) -> Option<GgufFacts> {
    let mut cursor = Cursor::new(buf);
    if cursor.read_bytes(4)? != b"GGUF" {
        return None;
    }
    let version = cursor.read_u32()?;
    if version != 2 && version != 3 {
        return None;
    }
    cursor.skip(8)?; // tensor count (u64)
    let kv_count = cursor.read_u64()?;

    let mut facts = GgufFacts::default();
    for _ in 0..kv_count.min(10_000) {
        let key = cursor.read_string()?;
        let value_type = cursor.read_u32()?;
        if key == "general.architecture" && value_type == 8 {
            facts.architecture = cursor.read_string();
        } else if key.ends_with(".context_length") && value_type == 4 {
            facts.context_length = cursor.read_u32();
        } else {
            cursor.skip_value(value_type)?;
        }
        if facts.architecture.is_some() && facts.context_length.is_some() {
            break;
        }
    }
    if facts.architecture.is_none() && facts.context_length.is_none() {
        return None;
    }
    Some(facts)
}

/// Bounds-checked little-endian cursor over a byte buffer.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        let slice = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn read_u32(&mut self) -> Option<u32> {
        let bytes = self.read_bytes(4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Option<u64> {
        let bytes = self.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Some(u64::from_le_bytes(arr))
    }

    fn read_string(&mut self) -> Option<String> {
        let len = self.read_u64()? as usize;
        if len > 1_000_000 {
            return None;
        }
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    fn skip(&mut self, len: usize) -> Option<()> {
        self.read_bytes(len).map(|_| ())
    }

    fn skip_value(&mut self, value_type: u32) -> Option<()> {
        match value_type {
            0 | 1 | 7 => self.skip(1),
            2 | 3 => self.skip(2),
            4..=6 => self.skip(4),
            10..=12 => self.skip(8),
            8 => self.read_string().map(|_| ()),
            9 => self.skip_array(),
            _ => None,
        }
    }

    fn skip_array(&mut self) -> Option<()> {
        let elem_type = self.read_u32()?;
        let len = self.read_u64()? as usize;
        if len > 10_000_000 {
            return None;
        }
        if elem_type == 8 {
            for _ in 0..len {
                self.read_string()?;
            }
            return Some(());
        }
        let elem_size = match elem_type {
            0 | 1 | 7 => 1,
            2 | 3 => 2,
            4..=6 => 4,
            10..=12 => 8,
            _ => return None,
        };
        self.skip(len.checked_mul(elem_size)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Appends one GGUF metadata key-value to a byte buffer.
    fn push_kv(buf: &mut Vec<u8>, key: &str, value_type: u32, value: &[u8]) {
        buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        buf.extend_from_slice(key.as_bytes());
        buf.extend_from_slice(&value_type.to_le_bytes());
        buf.extend_from_slice(value);
    }

    /// Appends a GGUF string value (length-prefixed bytes).
    fn push_string_value(buf: &mut Vec<u8>, text: &str) {
        buf.extend_from_slice(&(text.len() as u64).to_le_bytes());
        buf.extend_from_slice(text.as_bytes());
    }

    /// Builds a minimal version-3 GGUF header declaring an architecture and context length.
    fn minimal_header() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&2u64.to_le_bytes());
        let mut arch_value = Vec::new();
        push_string_value(&mut arch_value, "qwen2");
        push_kv(&mut buf, "general.architecture", 8, &arch_value);
        push_kv(&mut buf, "qwen2.context_length", 4, &32768u32.to_le_bytes());
        buf
    }

    #[test]
    fn reads_declared_architecture_and_context_length() {
        let facts = parse_gguf_header(&minimal_header()).expect("header must parse");
        assert_eq!(facts.architecture.as_deref(), Some("qwen2"));
        assert_eq!(facts.context_length, Some(32768));
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut header = minimal_header();
        header[0] = 0x00;
        assert!(parse_gguf_header(&header).is_none());
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut header = minimal_header();
        header[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(parse_gguf_header(&header).is_none());
    }

    #[test]
    fn truncated_header_yields_no_facts() {
        assert!(parse_gguf_header(&minimal_header()[..10]).is_none());
    }

    #[test]
    fn missing_file_yields_no_facts() {
        let missing = std::path::Path::new("/nonexistent/model.gguf");
        assert!(read_gguf_facts(missing).is_none());
    }
}
