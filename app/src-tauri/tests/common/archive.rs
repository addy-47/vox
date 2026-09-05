//! ============================================================================
//! tests/common/archive.rs — Archive and Model Synthesis Helpers for Tests
//! ============================================================================

use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use vox_lib::setup::manifest::ModelEntry;

/// Computes lowercase hex-encoded SHA-256 digest of byte slice.
pub fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Constructs a synthetic `ModelEntry` with specified parameters.
pub fn create_synthetic_model_entry(
    id: &str,
    path: &str,
    size_bytes: u64,
    sha256: &str,
    archive_type: Option<&str>,
) -> ModelEntry {
    ModelEntry {
        id: id.to_string(),
        path: path.to_string(),
        size_bytes,
        sha256: sha256.to_string(),
        archive_type: archive_type.map(|s| s.to_string()),
        required: false,
    }
}

/// Builds a zip archive containing entries (filename, content bytes) and writes to dest_path.
pub fn create_test_zip_archive(dest_path: &Path, entries: &[(&str, &[u8])]) -> anyhow::Result<()> {
    let file = std::fs::File::create(dest_path)?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for (name, content) in entries {
        zip_writer.start_file(*name, options)?;
        zip_writer.write_all(content)?;
    }
    zip_writer.finish()?;
    Ok(())
}

/// Builds a tar.gz archive containing entries (filename, content bytes) and writes to dest_path.
/// Supports arbitrary paths (including traversal patterns) by bypassing high-level path sanitization if needed.
pub fn create_test_tar_gz_archive(
    dest_path: &Path,
    entries: &[(&str, &[u8])],
) -> anyhow::Result<()> {
    let file = std::fs::File::create(dest_path)?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar_builder = tar::Builder::new(enc);

    for (name, content) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        // Write raw name into the 100-byte name field of the tar header
        let name_bytes = name.as_bytes();
        let header_bytes = header.as_mut_bytes();
        let copy_len = name_bytes.len().min(100);
        header_bytes[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        header.set_cksum();
        tar_builder.append(&header, *content)?;
    }
    tar_builder.finish()?;
    Ok(())
}
