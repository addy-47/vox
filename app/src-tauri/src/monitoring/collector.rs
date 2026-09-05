use crate::core::state::AppState;
use crate::core::state::InteractionOwner;
use crate::core::state::InteractionState;
use crate::monitoring::snapshot::RuntimeSnapshot;
use crate::monitoring::COLLECTOR_TICK_INTERVAL;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Spawn the Monitoring Collector on a dedicated OS thread.
pub fn spawn_monitoring_collector(state: Arc<AppState>) {
    thread::Builder::new()
        .name("vox-monitor".to_string())
        .spawn(move || {
            log::info!("[Monitoring::Collector] Collector worker started (10Hz)");

            let mut sys = sysinfo::System::new_with_specifics(
                sysinfo::RefreshKind::new()
                    .with_memory(sysinfo::MemoryRefreshKind::everything())
                    .with_cpu(sysinfo::CpuRefreshKind::everything()),
            );
            sys.refresh_memory();
            let total_ram_mb = (sys.total_memory() / 1024 / 1024) as u32;
            let cpu_cores = sys.cpus().len() as u32;

            let mut tick_count: u64 = 0;
            loop {
                tick_count = tick_count.wrapping_add(1);
                if tick_count.is_multiple_of(50) {
                    if let Some(governor) = crate::utils::check_cpu_governor() {
                        let is_optimal = governor == "performance";
                        *state.cpu_governor.lock() = governor;
                        state
                            .cpu_governor_optimal
                            .store(is_optimal, Ordering::Relaxed);
                    }
                }
                let threads = state.telemetry.latest_threads.load(Ordering::Relaxed);
                let snapshot = collect_snapshot(&state, threads, total_ram_mb, cpu_cores);
                state.monitoring.push(snapshot);
                thread::sleep(COLLECTOR_TICK_INTERVAL);
            }
        })
        .expect("[Monitoring::Collector] Failed to spawn monitoring collector thread");
}

fn map_pipeline_state_string(state_u32: u32) -> String {
    match InteractionState::from(state_u32) {
        InteractionState::Idle => "Idle".into(),
        InteractionState::Ready => "Ready".into(),
        InteractionState::Listening => "Listening".into(),
        InteractionState::Thinking => "Thinking".into(),
        InteractionState::Speaking => "Speaking".into(),
        InteractionState::Paused => "Paused".into(),
        InteractionState::Error => "Error".into(),
        InteractionState::Sleeping => "Sleeping".into(),
    }
}

fn get_playback_buffer_samples(state: &AppState) -> usize {
    if let Ok(lock) = state.engine.try_lock() {
        if let Some(engine) = lock.as_ref() {
            engine.playback_engine.buffer_len()
        } else {
            0
        }
    } else {
        0
    }
}

fn get_llm_provider_kind(state: &AppState) -> String {
    let settings = match state.settings.read() {
        Ok(s) => s,
        Err(_) => return "embedded".to_string(),
    };
    match settings.llm.active {
        crate::core::settings::LlmActiveProvider::Embedded => "embedded".to_string(),
        crate::core::settings::LlmActiveProvider::Server => {
            if let Some(ref name) = settings.llm.server.provider_name {
                format!("server:{}", name.to_lowercase())
            } else {
                "server".to_string()
            }
        }
        crate::core::settings::LlmActiveProvider::Cloud => {
            if let Some(ref name) = settings.llm.cloud.provider_name {
                format!("cloud:{}", name.to_lowercase())
            } else {
                "cloud".to_string()
            }
        }
    }
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

    let pa = &state.pipeline;
    let pipeline_state = map_pipeline_state_string(pa.current_state_atomic.load(Ordering::Relaxed));
    let owner_enum: InteractionOwner = state.owner.load(Ordering::Relaxed).into();
    let owner = format!("{:?}", owner_enum);
    let buffer_samples = get_playback_buffer_samples(state);
    let sys_ram_pct = f32::from_bits(state.telemetry.latest_sys_ram.load(Ordering::Relaxed));
    let llm_provider_kind = get_llm_provider_kind(state);

    RuntimeSnapshot {
        pipeline_state,
        current_turn_id: pa.turn_id.load(Ordering::Relaxed),
        conversation_id: state.conversation_id.load(Ordering::Relaxed),

        playback_active: pa.state() == InteractionState::Speaking,

        system_cpu_usage: f32::from_bits(state.telemetry.latest_sys_cpu.load(Ordering::Relaxed)),
        system_ram_mb: (sys_ram_pct * 0.01 * total_ram_mb as f32) as u32,
        vox_cpu_usage: f32::from_bits(state.telemetry.latest_vox_cpu.load(Ordering::Relaxed)),
        vox_ram_mb: state.telemetry.latest_vox_ram.load(Ordering::Relaxed),
        total_ram_mb,
        cpu_cores,

        vad_energy: f32::from_bits(state.telemetry.latest_energy.load(Ordering::Relaxed)),
        vad_probability: f32::from_bits(state.telemetry.latest_vad_prob.load(Ordering::Relaxed)),

        stt_latency_ms: Some(state.telemetry.latest_stt_ms.load(Ordering::Relaxed))
            .filter(|&v| v > 0),
        ttft_ms: Some(state.telemetry.latest_ttft_ms.load(Ordering::Relaxed)).filter(|&v| v > 0),
        total_voice_latency_ms: Some(
            state
                .telemetry
                .latest_voice_latency_ms
                .load(Ordering::Relaxed),
        )
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
            let bits = state.telemetry.latest_tts_rtf.load(Ordering::Relaxed);
            let val = f32::from_bits(bits);
            if val > 0.0 {
                Some(val)
            } else {
                None
            }
        },
        playback_start_ms: Some(
            state
                .telemetry
                .latest_playback_start_ms
                .load(Ordering::Relaxed),
        )
        .filter(|&v| v > 0),
        persistence_writes_per_sec: f32::from_bits(
            state
                .telemetry
                .latest_persistence_rate
                .load(Ordering::Relaxed),
        ),
        is_db_healthy: state.telemetry.is_db_healthy.load(Ordering::Relaxed),

        is_llm_loaded: state
            .engine
            .try_lock()
            .map(|e| e.as_ref().map(|eng| eng.llm_tx.is_some()).unwrap_or(false))
            .unwrap_or(false),
        llm_provider_kind,
        is_tts_loaded: state
            .engine
            .try_lock()
            .map(|e| e.as_ref().map(|eng| eng.tts_tx.is_some()).unwrap_or(false))
            .unwrap_or(false),
        is_stt_loaded: state
            .engine
            .try_lock()
            .map(|e| {
                e.as_ref()
                    .map(|eng| eng.stt_handle.is_some())
                    .unwrap_or(false)
            })
            .unwrap_or(false),
        is_vad_loaded: state
            .engine
            .try_lock()
            .map(|e| {
                e.as_ref()
                    .map(|eng| eng.vad_handle.is_some())
                    .unwrap_or(false)
            })
            .unwrap_or(false),
        is_embedder_loaded: crate::services::memory::is_embedder_loaded(),
        is_query_classifier_loaded: crate::services::memory::is_scope_classifier_loaded(),
        is_intra_edge_classifier_loaded: crate::services::memory::is_nli_loaded(),
        is_inter_edge_classifier_loaded: crate::services::memory::is_edge_classifier_loaded(),
        is_translit_loaded: crate::services::translit::is_transliteration_engine_loaded(),

        cpu_governor: state.cpu_governor.lock().clone(),
        cpu_governor_optimal: state.cpu_governor_optimal.load(Ordering::Relaxed),

        main_webview_ram_mb: None,
        tray_webview_ram_mb: None,
        wizard_webview_ram_mb: None,

        timestamp_ms: now,
    }
}
