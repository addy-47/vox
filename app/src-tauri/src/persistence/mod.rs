pub mod db;
pub mod events;
pub mod memory_worker;
pub mod mutations;
pub mod queries;
pub mod schema;
pub mod voices;
pub mod worker;

/// Floating-point vector byte-blob encoding and decoding helpers for Turso F32_BLOB columns.
pub fn encode_f32_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn decode_f32_blob(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or_default()))
        .collect()
}
