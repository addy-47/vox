use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::monitoring::aggregator::TelemetryEvent;
use crate::utils::audio_filters::FilterBank;
use crossbeam_channel::Sender;

use super::utils::calculate_rms;

/// Calculates 3-band audio energy telemetry and dispatches to the monitoring channel.
/// Returns the raw RMS energy value for downstream threshold gating.
pub fn process_and_emit_telemetry(
    chunk: &[f32],
    filter_bank: &mut FilterBank,
    noise_gate: f32,
    telemetry_tx: &Sender<TelemetryEvent>,
    dropped_counter: &Arc<AtomicU64>,
) -> f32 {
    let (raw_low, raw_mid, raw_high) = filter_bank.process_chunk(chunk);
    let raw_energy = calculate_rms(chunk);

    let gated_raw = if raw_energy > noise_gate {
        raw_energy
    } else {
        0.0
    };
    let energy = (gated_raw * 12.0).clamp(0.0, 1.0).powf(0.5);

    let gated_low = if raw_low > noise_gate { raw_low } else { 0.0 };
    let gated_mid = if raw_mid > noise_gate { raw_mid } else { 0.0 };
    let gated_high = if raw_high > noise_gate {
        raw_high
    } else {
        0.0
    };

    let low = (gated_low * 12.0).clamp(0.0, 1.0).powf(0.5);
    let mid = (gated_mid * 12.0).clamp(0.0, 1.0).powf(0.5);
    let high = (gated_high * 12.0).clamp(0.0, 1.0).powf(0.5);

    if telemetry_tx
        .try_send(TelemetryEvent::AudioEnergy {
            energy,
            vad_prob: 0.0,
            low,
            mid,
            high,
        })
        .is_err()
    {
        dropped_counter.fetch_add(1, Ordering::Relaxed);
    }

    raw_energy
}
