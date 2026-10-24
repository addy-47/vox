use std::{collections::HashMap, sync::LazyLock};

use serde::{Deserialize, Serialize};

use crate::config::mutation::supported_keys_for_domain;

/// Classification of an IPC command or settings mutation for remote dispatching.
///
/// Remote-control spec §8.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandClass {
    /// Executes on the host; response forwarded back to the controller.
    Remote,
    /// Only meaningful on host hardware or host-local state (rejected if invoked remotely).
    Local,
    /// Forwarded to the host, but sensitive fields redacted in response.
    Split,
    /// Forbidden from being invoked remotely.
    Denied,
}

/// Static map of all registered IPC commands and settings domain.keys to their routing class.
static ROUTING_TABLE: LazyLock<HashMap<String, CommandClass>> = LazyLock::new(|| {
    let mut table = HashMap::new();

    // ─── 1. Core interaction and pipeline commands ───────────────────────
    table.insert("launch_engine".into(), CommandClass::Remote);
    table.insert("stop_engine".into(), CommandClass::Remote);
    table.insert("start_session".into(), CommandClass::Remote);
    table.insert("end_session".into(), CommandClass::Remote);
    table.insert("pause_session".into(), CommandClass::Remote);
    table.insert("resume_session".into(), CommandClass::Remote);
    table.insert("submit_text_input".into(), CommandClass::Remote);
    table.insert("set_playback_muted".into(), CommandClass::Remote);
    table.insert("set_mic_muted".into(), CommandClass::Remote);
    table.insert("set_session_private_mode".into(), CommandClass::Remote);
    table.insert("ptt_start".into(), CommandClass::Remote);
    table.insert("ptt_stop".into(), CommandClass::Remote);
    table.insert("ptt_cancel".into(), CommandClass::Remote);

    // ─── 2. Window Chrome & Input Simulation (Host Local) ────────────────
    table.insert("show_main_window".into(), CommandClass::Local);
    table.insert("hide_tray_window".into(), CommandClass::Local);
    table.insert("set_window_click_through".into(), CommandClass::Local);
    table.insert("list_audio_devices".into(), CommandClass::Local);

    // ─── 3. Settings commands ────────────────────────────────────────────
    table.insert("get_settings".into(), CommandClass::Split);
    table.insert("get_restart_status".into(), CommandClass::Remote);
    table.insert("get_settings_contract".into(), CommandClass::Remote);
    table.insert("update_setting".into(), CommandClass::Remote);
    table.insert("apply_settings_patch".into(), CommandClass::Remote);
    table.insert("reset_settings".into(), CommandClass::Remote);

    // ─── 4. Model Catalog & Management ───────────────────────────────────
    table.insert("get_model_catalog".into(), CommandClass::Remote);
    table.insert("get_provider_caps".into(), CommandClass::Remote);
    table.insert("check_provider_health".into(), CommandClass::Remote);
    table.insert("probe_provider_target".into(), CommandClass::Remote);
    table.insert("list_llm_models".into(), CommandClass::Remote);
    table.insert("probe_model_capabilities".into(), CommandClass::Remote);
    table.insert("get_model_capabilities_cache".into(), CommandClass::Remote);
    table.insert("list_favorite_models".into(), CommandClass::Remote);
    table.insert("toggle_model_favorite".into(), CommandClass::Remote);
    table.insert("manage_models".into(), CommandClass::Local);
    table.insert("setup_remote_server".into(), CommandClass::Local);

    // ─── 5. Projects ─────────────────────────────────────────────────────
    table.insert("get_projects".into(), CommandClass::Remote);
    table.insert("create_project".into(), CommandClass::Remote);
    table.insert("rename_project".into(), CommandClass::Remote);
    table.insert("delete_project".into(), CommandClass::Remote);

    // ─── 6. Sessions & History ───────────────────────────────────────────
    table.insert("create_session".into(), CommandClass::Remote);
    table.insert("continue_session".into(), CommandClass::Remote);
    table.insert("get_sessions".into(), CommandClass::Remote);
    table.insert("get_turns".into(), CommandClass::Remote);
    table.insert("search_history".into(), CommandClass::Remote);
    table.insert("update_session".into(), CommandClass::Remote);
    table.insert("delete_session".into(), CommandClass::Remote);
    table.insert("compact_session".into(), CommandClass::Remote);
    table.insert("get_transcript_history".into(), CommandClass::Remote);

    // ─── 7. Personal Memory ──────────────────────────────────────────────
    table.insert("get_personal_memory".into(), CommandClass::Remote);
    table.insert("save_personal_memory".into(), CommandClass::Remote);
    table.insert("consolidate_personal_memory".into(), CommandClass::Remote);
    table.insert("regenerate_personal_memory".into(), CommandClass::Remote);
    table.insert("get_personal_memory_versions".into(), CommandClass::Remote);
    table.insert(
        "set_active_personal_memory_version".into(),
        CommandClass::Remote,
    );
    table.insert("get_observations".into(), CommandClass::Remote);
    table.insert("get_ingestion_stats".into(), CommandClass::Remote);
    table.insert("get_memory_revisions".into(), CommandClass::Remote);
    table.insert("resolve_memory_revisions".into(), CommandClass::Remote);

    // ─── 8. Voices & Speech ──────────────────────────────────────────────
    table.insert("list_voices".into(), CommandClass::Remote);
    table.insert("get_voice_scope".into(), CommandClass::Remote);
    table.insert("sync_remote_voices".into(), CommandClass::Remote);
    table.insert("synthesize_voice_preview".into(), CommandClass::Remote);
    table.insert("toggle_voice_favorite".into(), CommandClass::Remote);
    table.insert("record_voice_used".into(), CommandClass::Remote);
    table.insert("set_active_voice".into(), CommandClass::Remote);
    table.insert("add_voice_from_file".into(), CommandClass::Remote);
    table.insert("add_voice_from_recording".into(), CommandClass::Remote);
    table.insert("start_backend_recording".into(), CommandClass::Remote);
    table.insert("stop_backend_recording".into(), CommandClass::Remote);
    table.insert("delete_voice".into(), CommandClass::Remote);
    table.insert("rename_voice".into(), CommandClass::Remote);

    // ─── 9. Monitoring & Profiler ────────────────────────────────────────
    table.insert("get_runtime_snapshot".into(), CommandClass::Remote);
    table.insert("get_profiler_snapshot".into(), CommandClass::Remote);
    table.insert("record_memory_profile_event".into(), CommandClass::Remote);

    // ─── 10. Setup & Updates ─────────────────────────────────────────────
    table.insert("fetch_manifest".into(), CommandClass::Remote);
    table.insert("check_updates".into(), CommandClass::Remote);
    table.insert("get_onboarding_status".into(), CommandClass::Remote);
    table.insert("get_runtime_report".into(), CommandClass::Remote);
    table.insert("complete_setup_wizard".into(), CommandClass::Local);
    table.insert("reveal_wizard".into(), CommandClass::Local);

    // ─── 11. Notifications ───────────────────────────────────────────────
    table.insert("notification_permission_state".into(), CommandClass::Remote);
    table.insert(
        "request_notification_permission".into(),
        CommandClass::Remote,
    );
    table.insert("debug_fire_transient".into(), CommandClass::Remote);
    table.insert("get_notifications".into(), CommandClass::Remote);
    table.insert("mark_notifications_read".into(), CommandClass::Remote);
    table.insert("dismiss_notifications".into(), CommandClass::Remote);
    table.insert("execute_notification_action".into(), CommandClass::Remote);

    // ─── 12. Remote Control Domain Commands ──────────────────────────────
    table.insert("pair_new_device".into(), CommandClass::Local);
    table.insert("approve_pairing".into(), CommandClass::Local);
    table.insert("list_paired_devices".into(), CommandClass::Local);
    table.insert("revoke_device".into(), CommandClass::Local);
    table.insert("disconnect_device".into(), CommandClass::Local);
    table.insert("connect_host".into(), CommandClass::Local);
    table.insert("disconnect_host".into(), CommandClass::Local);
    table.insert("get_link_state".into(), CommandClass::Local);
    table.insert("get_command_routing".into(), CommandClass::Local);

    // ─── 13. Settings domain.key entries ─────────────────────────────────
    let domains = [
        "appearance",
        "vad",
        "stt",
        "llm",
        "tts",
        "interaction",
        "dictation",
        "memory",
        "persona",
        "realtime",
        "system",
        "remote",
    ];

    for domain in domains {
        for key in supported_keys_for_domain(domain) {
            let key_path = format!("{domain}.{key}");
            let classification = match (domain, *key) {
                ("appearance", _) => CommandClass::Local,
                ("remote", "accepts_controllers") => CommandClass::Local,
                ("remote", "execution_mode") => CommandClass::Local,
                _ => CommandClass::Remote,
            };
            table.insert(key_path, classification);
        }
    }

    // Pseudo-domain for per-model overrides
    table.insert("model_parameters".into(), CommandClass::Remote);

    table
});

/// Returns a reference to the global static command routing table.
pub fn get_routing_table() -> &'static HashMap<String, CommandClass> {
    &ROUTING_TABLE
}

/// Looks up the routing classification for a specific command or domain.key setting.
pub fn classify_command(command: &str, domain: Option<&str>, key: Option<&str>) -> CommandClass {
    if let (Some(d), Some(k)) = (domain, key) {
        let path = format!("{d}.{k}");
        if let Some(class) = ROUTING_TABLE.get(&path) {
            return *class;
        }
    }

    if let Some(d) = domain {
        if d == "model_parameters" {
            return CommandClass::Remote;
        }
    }

    ROUTING_TABLE
        .get(command)
        .copied()
        .unwrap_or(CommandClass::Denied)
}
