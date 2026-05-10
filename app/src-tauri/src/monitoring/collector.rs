use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::core::state::AppState;
use crate::monitoring::snapshot::RuntimeSnapshot;

/// Spawn the Monitoring Collector on a dedicated OS thread.
/// Runs at 10Hz (100ms ticks).
pub fn spawn_monitoring_collector(state: Arc<AppState>) {
    thread::Builder::new()
        .name("vox-monitor".to_string())
        .spawn(move || {
            tracing::info!("[Monitoring] Collector worker started (10Hz).");
            
            loop {
                // 1. Collect data from various atomics and state handles
                let snapshot = collect_snapshot(&state);
                
                // 2. Push to ringbuffer history
                state.monitoring.push(snapshot);
                
                // 3. Sleep until next tick (100ms = 10Hz)
                thread::sleep(Duration::from_millis(100));
            }
        })
        .expect("Failed to spawn monitoring collector thread");
}

fn collect_snapshot(state: &AppState) -> RuntimeSnapshot {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Get pipeline atomics (cheaper than locking the Mutex)
    let pa = &state.pipeline;
    
    // Convert InteractionState enum from atomic u32
    let pipeline_state_u32 = pa.current_state_atomic.load(Ordering::Relaxed);
    let pipeline_state = match pipeline_state_u32 {
        0 => "Idle".to_string(),
        1 => "Listening".to_string(),
        2 => "Thinking".to_string(),
        3 => "AssistantSpeaking".to_string(),
        4 => "Error".to_string(),
        _ => "Unknown".to_string(),
    };

    // Get interaction owner
    let owner = {
        // We use a try_lock here to avoid blocking if the orchestrator is busy.
        // If locked, we'll just use the last known atomic state if we had one,
        // but for now let's just get it from the mutex as it's not the audio hot path.
        // However, Directive says "Monitoring must never stall".
        match state.owner.try_lock() {
            Ok(o) => format!("{:?}", *o),
            Err(_) => "Locked".to_string(),
        }
    };

    RuntimeSnapshot {
        pipeline_state,
        current_turn_id: pa.turn_id.load(Ordering::Relaxed),
        conversation_id: state.conversation_id.load(Ordering::Relaxed),

        playback_active: pa.playback_active.load(Ordering::Relaxed),
        llm_generating: pa.llm_generating.load(Ordering::Relaxed),
        tts_generating: pa.tts_generating.load(Ordering::Relaxed),

        cpu_usage: f32::from_bits(state.latest_cpu.load(Ordering::Relaxed)),
        ram_mb: state.latest_ram.load(Ordering::Relaxed),

        vad_energy: f32::from_bits(state.latest_energy.load(Ordering::Relaxed)),
        vad_probability: f32::from_bits(state.latest_vad_prob.load(Ordering::Relaxed)),

        stt_latency_ms: Some(state.latest_stt_ms.load(Ordering::Relaxed)).filter(|&v| v > 0),
        ttft_ms: Some(state.latest_ttft_ms.load(Ordering::Relaxed)).filter(|&v| v > 0),
        total_voice_latency_ms: None, // Will be calculated in future phases

        persistence_queue_depth: state.persist_tx.as_ref().map(|tx| tx.len()).unwrap_or(0),
        dropped_persistence_events: state.dropped_persistence_events.load(Ordering::Relaxed),

        playback_buffer_samples: 0, // Need to expose this from PlaybackEngine if needed
        playback_underruns: pa.playback_underruns.load(Ordering::Relaxed),

        active_owner: owner,
        timestamp_ms: now,
    }
}
