use crate::services::remote::session::RemoteLinkState;

/// The origin of an incoming control command.
///
/// Remote-control spec §8.2: The desktop-vs-remote distinction is an
/// explicit parameter on affected service functions, never a thread-local or
/// ambient global flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOrigin {
    Desktop,
    Controller,
}

/// Permitted read-only or chrome command names while Desktop Host is in an active remote session.
const PERMITTED_DESKTOP_READS: &[&str] = &[
    // Audio
    "list_audio_devices",
    // Catalog & Models
    "get_model_catalog",
    "get_provider_caps",
    "check_provider_health",
    "probe_provider_target",
    "list_llm_models",
    "probe_model_capabilities",
    "get_model_capabilities_cache",
    "list_favorite_models",
    // Memory
    "get_personal_memory",
    "get_personal_memory_versions",
    "get_observations",
    "get_ingestion_stats",
    "get_memory_revisions",
    // Monitoring & Profiler
    "get_runtime_snapshot",
    "get_profiler_snapshot",
    // Notifications
    "notification_permission_state",
    "get_notifications",
    // Persistence & History
    "get_transcript_history",
    "get_sessions",
    "get_turns",
    "search_history",
    // Projects
    "get_projects",
    // Settings reads
    "get_restart_status",
    "get_settings",
    "get_settings_contract",
    // Setup & Diagnostics
    "check_updates",
    "fetch_manifest",
    "get_runtime_report",
    "get_onboarding_status",
    // Voices
    "list_voices",
    "get_voice_scope",
    // Window chrome
    "show_main_window",
    "hide_tray_window",
    "set_window_click_through",
    // Remote Admin
    "disconnect_device",
    "revoke_device",
    "pair_new_device",
    "approve_pairing",
    "list_paired_devices",
    "get_link_state",
    "get_command_routing",
];

/// Evaluates whether an IPC command is permitted from Desktop under `RULE-RC-GUARD`.
///
/// When `state == RemoteLinkState::Connected`, all desktop mutating commands
/// are refused. Only reads, window chrome, appearance settings, and remote
/// administration commands remain callable.
pub fn is_desktop_command_permitted(
    command: &str,
    domain: Option<&str>,
    state: &RemoteLinkState,
) -> bool {
    if *state != RemoteLinkState::Connected {
        return true;
    }

    if let Some(d) = domain {
        if d == "appearance" {
            return true;
        }
        if d == "remote" && command == "update_setting" {
            return true;
        }
        return false;
    }

    PERMITTED_DESKTOP_READS.contains(&command)
}

/// Evaluates command permission considering explicit `ControlOrigin`.
pub fn is_command_permitted(
    origin: ControlOrigin,
    command: &str,
    domain: Option<&str>,
    state: &RemoteLinkState,
) -> bool {
    match origin {
        ControlOrigin::Controller => true,
        ControlOrigin::Desktop => is_desktop_command_permitted(command, domain, state),
    }
}
