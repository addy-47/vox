//! ============================================================================
//! audio_capture_aec_test.rs — Capture and AEC seam
//! ============================================================================
//! Category  : Integration (Seam 11 — microphone capture → AEC → clean stream)
//! Component : services/audio/aec.rs + services/audio/device.rs + services/audio/sink.rs
//! Binary    : audio_capture_aec_test
//! Prereqs   : the test audio assets
//! Run       : cargo nextest run --test audio_capture_aec_test --release --test-threads=1
//! ============================================================================

mod common;

#[path = "audio_capture/aec_pipeline.rs"]
mod aec_pipeline;

#[path = "audio_capture/barge_in.rs"]
mod barge_in;
