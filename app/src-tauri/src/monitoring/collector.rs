use crate::core::state::AppState;
use crate::monitoring::snapshot::RuntimeSnapshot;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Spawn the Monitoring Collector on a dedicated OS thread.
/// Runs at 10Hz (100ms ticks).
pub fn spawn_monitoring_collector(state: Arc<AppState>) {
    thread::Builder::new()
        .name("vox-monitor".to_string())
        .spawn(move || {
            tracing::info!("[Monitoring] Collector worker started (10Hz).");

            // Fetch static hardware context once
            let sys = sysinfo::System::new_with_specifics(
                sysinfo::RefreshKind::new()
                    .with_memory(sysinfo::MemoryRefreshKind::everything())
                    .with_cpu(sysinfo::CpuRefreshKind::everything()),
            );
            let total_ram_mb = (sys.total_memory() / 1024 / 1024) as u32;
            let cpu_cores = sys.cpus().len() as u32;

            loop {
                // Read current thread count from atomic (updated by system_monitor.rs at 1Hz)
                let threads = state.latest_threads.load(Ordering::Relaxed);

                let snapshot = collect_snapshot(&state, threads, total_ram_mb, cpu_cores);

                // 2. Push to ringbuffer history
                state.monitoring.push(snapshot);

                // 3. Sleep until next tick (100ms = 10Hz)
                thread::sleep(Duration::from_millis(100));
            }
        })
        .expect("Failed to spawn monitoring collector thread");
}

fn collect_snapshot(
    state: &AppState,
    threads: u32,
    total_ram_mb: u32,
    cpu_cores: u32,
) -> RuntimeSnapshot {
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
    let owner_enum: crate::core::state::InteractionOwner =
        state.owner.load(Ordering::Relaxed).into();
    let owner = format!("{:?}", owner_enum);

    // Get buffer length from playback engine if it exists
    let buffer_samples = {
        if let Ok(lock) = state.engine.try_lock() {
            if let Some(engine) = lock.as_ref() {
                engine.playback_engine.buffer_len()
            } else {
                0
            }
        } else {
            0
        }
    };

    let sys_ram_pct = f32::from_bits(state.latest_sys_ram.load(Ordering::Relaxed));

    let llm_provider_kind = {
        let settings = state.settings.read().unwrap();
        match &settings.llm.provider {
            crate::core::settings::LlmProviderConfig::Embedded => "embedded".to_string(),
            crate::core::settings::LlmProviderConfig::OpenAiCompat { provider_name, .. } => {
                if let Some(ref name) = provider_name {
                    format!("openai_compat:{}", name.to_lowercase())
                } else {
                    "openai_compat".to_string()
                }
            }
        }
    };

    RuntimeSnapshot {
        pipeline_state,
        current_turn_id: pa.turn_id.load(Ordering::Relaxed),
        conversation_id: state.conversation_id.load(Ordering::Relaxed),

        playback_active: pa.playback_active.load(Ordering::Relaxed),
        tts_generating: pa.tts_generating.load(Ordering::Relaxed),

        system_cpu_usage: f32::from_bits(state.latest_sys_cpu.load(Ordering::Relaxed)),
        system_ram_mb: (sys_ram_pct * 0.01 * total_ram_mb as f32) as u32,
        vox_cpu_usage: f32::from_bits(state.latest_vox_cpu.load(Ordering::Relaxed)),
        vox_ram_mb: state.latest_vox_ram.load(Ordering::Relaxed),
        total_ram_mb,
        cpu_cores,

        vad_energy: f32::from_bits(state.latest_energy.load(Ordering::Relaxed)),
        vad_probability: f32::from_bits(state.latest_vad_prob.load(Ordering::Relaxed)),

        stt_latency_ms: Some(state.latest_stt_ms.load(Ordering::Relaxed)).filter(|&v| v > 0),
        ttft_ms: Some(state.latest_ttft_ms.load(Ordering::Relaxed)).filter(|&v| v > 0),
        total_voice_latency_ms: Some(state.latest_voice_latency_ms.load(Ordering::Relaxed))
            .filter(|&v| v > 0),

        persistence_queue_depth: state
            .persist_tx
            .lock()
            .as_ref()
            .map(|tx| tx.len())
            .unwrap_or(0),
        dropped_persistence_events: state.dropped_persistence_events.load(Ordering::Relaxed),

        playback_buffer_samples: buffer_samples,
        playback_underruns: pa.playback_underruns.load(Ordering::Relaxed),

        active_owner: owner,

        active_threads: threads,
        tts_rtf: {
            let bits = state.latest_tts_rtf.load(Ordering::Relaxed);
            let val = f32::from_bits(bits);
            if val > 0.0 {
                Some(val)
            } else {
                None
            }
        },
        playback_start_ms: Some(state.latest_playback_start_ms.load(Ordering::Relaxed))
            .filter(|&v| v > 0),
        persistence_writes_per_sec: f32::from_bits(
            state.latest_persistence_rate.load(Ordering::Relaxed),
        ),
        is_db_healthy: state.is_db_healthy.load(Ordering::Relaxed),

        is_llm_loaded: state.is_llm_loaded.load(Ordering::Relaxed),
        llm_provider_kind,
        is_tts_loaded: state.is_tts_loaded.load(Ordering::Relaxed),
        is_stt_loaded: state.is_stt_loaded.load(Ordering::Relaxed),
        is_vad_loaded: state.is_vad_loaded.load(Ordering::Relaxed),
        is_embedder_loaded: state.is_embedder_loaded.load(Ordering::Relaxed)
            || crate::services::memory::is_embedder_loaded(),
        is_query_classifier_loaded: state.is_query_classifier_loaded.load(Ordering::Relaxed)
            || crate::services::memory::is_scope_classifier_loaded(),
        is_intra_edge_classifier_loaded: state
            .is_intra_edge_classifier_loaded
            .load(Ordering::Relaxed)
            || crate::services::memory::is_nli_loaded(),
        is_inter_edge_classifier_loaded: state
            .is_inter_edge_classifier_loaded
            .load(Ordering::Relaxed)
            || crate::services::memory::is_edge_classifier_loaded(),
        is_translit_loaded: state.is_translit_loaded.load(Ordering::Relaxed)
            || crate::services::translit::is_transliteration_engine_loaded(),
        is_sleeping: state.is_sleeping.load(Ordering::Relaxed),
        is_engaged: state.pipeline.is_engaged.load(Ordering::Relaxed),

        cpu_governor: state.cpu_governor.lock().clone(),
        cpu_governor_optimal: state.cpu_governor_optimal.load(Ordering::Relaxed),

        timestamp_ms: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::runtime_state::MonitoringState;

    fn mock_snapshot(timestamp_ms: u64) -> RuntimeSnapshot {
        RuntimeSnapshot {
            pipeline_state: "Idle".to_string(),
            current_turn_id: 1,
            conversation_id: 100,
            playback_active: false,
            tts_generating: false,
            system_cpu_usage: 12.5,
            system_ram_mb: 2048,
            vox_cpu_usage: 3.2,
            vox_ram_mb: 180,
            total_ram_mb: 16384,
            cpu_cores: 8,
            vad_energy: 0.05,
            vad_probability: 0.01,
            stt_latency_ms: Some(150),
            ttft_ms: Some(280),
            total_voice_latency_ms: Some(430),
            persistence_queue_depth: 0,
            dropped_persistence_events: 0,
            playback_buffer_samples: 512,
            playback_underruns: 0,
            active_owner: "MainWindow".to_string(),
            active_threads: 10,
            tts_rtf: Some(0.42),
            playback_start_ms: Some(300),
            persistence_writes_per_sec: 2.5,
            is_db_healthy: true,
            is_llm_loaded: true,
            llm_provider_kind: "embedded".to_string(),
            is_tts_loaded: true,
            is_stt_loaded: true,
            is_vad_loaded: true,
            is_embedder_loaded: true,
            is_query_classifier_loaded: true,
            is_intra_edge_classifier_loaded: true,
            is_inter_edge_classifier_loaded: true,
            is_translit_loaded: true,
            is_sleeping: false,
            is_engaged: true,
            cpu_governor: "performance".to_string(),
            cpu_governor_optimal: true,
            timestamp_ms,
        }
    }

    #[test]
    fn test_telemetry_metrics_collector_window() {
        let monitoring = MonitoringState::new();

        // 1. Initial state check
        assert!(monitoring.get_latest().is_none());
        assert!(monitoring.get_history().is_empty());

        // 2. Push 10 snapshots and verify order and latest update
        for i in 0..10 {
            monitoring.push(mock_snapshot(1000 + i));
        }

        let history = monitoring.get_history();
        assert_eq!(history.len(), 10);
        assert_eq!(history[0].timestamp_ms, 1000);
        assert_eq!(history[9].timestamp_ms, 1009);

        let latest = monitoring.get_latest().unwrap();
        assert_eq!(latest.timestamp_ms, 1009);

        // 3. Overflow test: Push 650 total snapshots (capacity is MAX_SNAPSHOT_HISTORY = 600)
        for i in 10..650 {
            monitoring.push(mock_snapshot(1000 + i));
        }

        let bounded_history = monitoring.get_history();
        assert_eq!(
            bounded_history.len(),
            600,
            "History should be bounded by MAX_SNAPSHOT_HISTORY (600)"
        );

        // Oldest 50 snapshots (timestamps 1000..1049) should be evicted
        assert_eq!(
            bounded_history[0].timestamp_ms, 1050,
            "Oldest snapshot in window should have timestamp 1050"
        );
        assert_eq!(
            bounded_history[599].timestamp_ms, 1649,
            "Newest snapshot in window should have timestamp 1649"
        );

        let new_latest = monitoring.get_latest().unwrap();
        assert_eq!(new_latest.timestamp_ms, 1649);

        // 4. Clear history check
        monitoring.clear();
        assert!(monitoring.get_latest().is_none());
        assert!(monitoring.get_history().is_empty());
    }
}
