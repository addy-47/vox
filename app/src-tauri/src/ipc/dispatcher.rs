use std::sync::Arc;

use tauri::{AppHandle, Manager, State, Wry};

use crate::{
    core::{error::VoxIpcError, state::AppState},
    ipc::{
        catalog,
        memory::{self, ResolveRevisionsRequest},
        monitoring, notifications, persistence, pipeline, projects, settings,
        setup::{self, ManageModelsPayload},
        voices,
    },
    persistence::notifications::NotificationFilter,
    services::{health::ProviderConfigPayload, llm::config::LlmRuntimeTarget},
};

/// Unified command executor servicing both local webview calls and remote network requests.
pub async fn dispatch_command(
    app: &AppHandle<Wry>,
    _state: &AppState,
    command: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, VoxIpcError> {
    let st: State<'_, Arc<AppState>> = app.state();

    match command {
        // ─── Pipeline Core ───────────────────────────────────────────────────
        "launch_engine" => {
            pipeline::launch_engine(app.clone()).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "stop_engine" => {
            pipeline::stop_engine(app.clone()).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "start_session" => {
            let session_id = args.get("session_id").and_then(|v| v.as_i64());
            pipeline::start_session(session_id, app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "end_session" => {
            pipeline::end_session(app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "pause_session" => {
            pipeline::pause_session(app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "resume_session" => {
            pipeline::resume_session(app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "submit_text_input" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            pipeline::submit_text_input(query, app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }

        // ─── Pipeline Controls ───────────────────────────────────────────────
        "set_playback_muted" => {
            let muted = args.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
            pipeline::set_playback_muted(muted, app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "set_mic_muted" => {
            let muted = args.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
            pipeline::set_mic_muted(muted, app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "set_session_private_mode" => {
            let enabled = args
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            pipeline::set_session_private_mode(enabled, app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "ptt_start" => {
            pipeline::ptt_start(app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "ptt_stop" => {
            pipeline::ptt_stop(app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "ptt_cancel" => {
            pipeline::ptt_cancel(app.clone(), st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }

        // ─── Settings ────────────────────────────────────────────────────────
        "get_settings" => {
            let boot = settings::get_settings(app.clone()).await?;
            serde_json::to_value(&boot).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_settings_contract" => {
            let contract = settings::get_settings_contract().await?;
            serde_json::to_value(&contract).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_restart_status" => {
            let status = settings::get_restart_status(st).await?;
            serde_json::to_value(&status).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "update_setting" => {
            let domain = args
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let key = args
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let value = args
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let result = settings::update_setting(domain, key, value, app.clone()).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "set_appearance" => {
            let theme = args.get("theme").and_then(|v| v.as_str()).map(String::from);
            let accent_seed = args
                .get("accent_seed")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = settings::set_appearance(theme, accent_seed, app.clone()).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "reset_settings" => {
            let result = settings::reset_settings(app.clone()).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "apply_settings_patch" => {
            let patch = args.get("patch").cloned().unwrap_or(serde_json::json!({}));
            let result = settings::apply_settings_patch(patch, app.clone()).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }

        // ─── Session Management ──────────────────────────────────────────────
        "create_session" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = pipeline::create_session(project_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "continue_session" => {
            let session_id = args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let result = pipeline::continue_session(session_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "update_session" => {
            let session_id = args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let title = args.get("title").and_then(|v| v.as_str()).map(String::from);
            let is_pinned = args.get("is_pinned").and_then(|v| v.as_bool());
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            persistence::update_session(app.clone(), session_id, title, is_pinned, project_id, st)
                .await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "delete_session" => {
            let session_id = args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let hard = args.get("hard").and_then(|v| v.as_bool());
            persistence::delete_session(app.clone(), session_id, hard, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "get_sessions" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = persistence::get_sessions(project_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_turns" => {
            let session_id = args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let result = persistence::get_turns(session_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "search_history" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let limit = args.get("limit").and_then(|v| v.as_u64()).map(|u| u as u32);
            let result = persistence::search_history(query, project_id, limit, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "compact_session" => {
            let session_id = args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let result = persistence::compact_session(app.clone(), session_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_transcript_history" => {
            let result = persistence::get_transcript_history(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }

        // ─── Projects ────────────────────────────────────────────────────────
        "get_projects" => {
            let result = projects::get_projects(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "create_project" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let result = projects::create_project(st, name).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "rename_project" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            projects::rename_project(st, id, name).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "delete_project" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            projects::delete_project(st, id).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }

        // ─── Memory ──────────────────────────────────────────────────────────
        "get_personal_memory" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = memory::get_personal_memory(project_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "save_personal_memory" => {
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let expected_version = args
                .get("expected_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = memory::save_personal_memory(
                app.clone(),
                content,
                expected_version,
                project_id,
                st,
            )
            .await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "consolidate_personal_memory" => {
            let comments = args
                .get("comments")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let forced = args.get("forced").and_then(|v| v.as_bool());
            let result =
                memory::consolidate_personal_memory(app.clone(), comments, project_id, forced, st)
                    .await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "regenerate_personal_memory" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = memory::regenerate_personal_memory(app.clone(), project_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_personal_memory_versions" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = memory::get_personal_memory_versions(project_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "set_active_personal_memory_version" => {
            let version = args.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result =
                memory::set_active_personal_memory_version(app.clone(), version, project_id, st)
                    .await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_observations" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let status = args
                .get("status")
                .and_then(|v| v.as_str())
                .map(String::from);
            let limit = args.get("limit").and_then(|v| v.as_u64()).map(|u| u as u32);
            let offset = args
                .get("offset")
                .and_then(|v| v.as_u64())
                .map(|u| u as u32);
            let obs_type = args
                .get("observation_type")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result =
                memory::get_observations(project_id, status, limit, offset, obs_type, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_ingestion_stats" => {
            let result = memory::get_ingestion_stats(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_memory_revisions" => {
            let project_id = args
                .get("project_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = memory::get_memory_revisions(project_id, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "resolve_memory_revisions" => {
            let request: ResolveRevisionsRequest = args
                .get("request")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(ResolveRevisionsRequest {
                    project_id: None,
                    decisions: Vec::new(),
                });
            let result = memory::resolve_memory_revisions(app.clone(), request, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }

        // ─── Models & Catalog ────────────────────────────────────────────────
        "get_model_catalog" => {
            let result = catalog::get_model_catalog(app.clone()).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_provider_caps" => {
            let provider_id = args
                .get("provider_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let result = catalog::get_provider_caps(provider_id)?;
            serde_json::to_value(result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "check_provider_health" => {
            let kind = args
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("llm")
                .to_string();
            let provider: Option<ProviderConfigPayload> = args
                .get("provider")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let result = catalog::check_provider_health(st, kind, provider).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "list_favorite_models" => {
            let result = catalog::list_favorite_models(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "toggle_model_favorite" => {
            let model_id = args
                .get("model_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let result = catalog::toggle_model_favorite(model_id, st).await?;
            serde_json::to_value(result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "probe_provider_target" => {
            let kind = args
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("llm")
                .to_string();
            let provider_id = args
                .get("provider_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let base_url = args
                .get("base_url")
                .and_then(|v| v.as_str())
                .map(String::from);
            let api_key = args
                .get("api_key")
                .and_then(|v| v.as_str())
                .map(String::from);
            let model = args.get("model").and_then(|v| v.as_str()).map(String::from);
            let result =
                catalog::probe_provider_target(st, kind, provider_id, base_url, api_key, model)
                    .await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "list_llm_models" => {
            let provider: Option<LlmRuntimeTarget> = args
                .get("provider")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let result = catalog::list_llm_models(st, provider).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "probe_model_capabilities" => {
            let provider: Option<LlmRuntimeTarget> = args
                .get("provider")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let model_id = args
                .get("model_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let target_cap = args
                .get("target_cap")
                .and_then(|v| v.as_u64())
                .map(|u| u as u32);
            let result =
                catalog::probe_model_capabilities(st, provider, model_id, target_cap).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_model_capabilities_cache" => {
            let result = catalog::get_model_capabilities_cache().await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "manage_models" => {
            let payload: ManageModelsPayload = serde_json::from_value(
                args.get("payload").cloned().unwrap_or_else(|| args.clone()),
            )
            .map_err(|e| {
                VoxIpcError::InvalidArgument(format!("Invalid manage_models payload: {e}"))
            })?;
            let result = setup::manage_models(app.clone(), st, payload).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }

        // ─── Voices ──────────────────────────────────────────────────────────
        "list_voices" => {
            let scope = args.get("scope").and_then(|v| v.as_str()).map(String::from);
            let result = voices::list_voices(scope, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_voice_scope" => {
            let engine = args
                .get("engine")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = voices::get_voice_scope(engine).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "sync_remote_voices" => {
            let n = voices::sync_remote_voices(st).await?;
            Ok(serde_json::json!({ "synced": n }))
        }
        "synthesize_voice_preview" => {
            let voice_id = args
                .get("voice_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let prompt = args
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(String::from);
            let engine = args
                .get("engine")
                .and_then(|v| v.as_str())
                .map(String::from);
            let result = voices::synthesize_voice_preview(voice_id, prompt, engine, st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "toggle_voice_favorite" => {
            let voice_id = args
                .get("voice_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let result = voices::toggle_voice_favorite(voice_id, st).await?;
            serde_json::to_value(result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "record_voice_used" => {
            let voice_id = args
                .get("voice_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            voices::record_voice_used(voice_id, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "set_active_voice" => {
            let voice_id = args
                .get("voice_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            voices::set_active_voice(voice_id, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "delete_voice" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            voices::delete_voice(id, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "rename_voice" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            voices::rename_voice(id, name, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }

        // ─── Notifications ───────────────────────────────────────────────────
        "notification_permission_state" => {
            let result = notifications::notification_permission_state(app.clone())?;
            serde_json::to_value(result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "request_notification_permission" => {
            let result = notifications::request_notification_permission(app.clone())?;
            serde_json::to_value(result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "debug_fire_transient" => {
            let severity = args
                .get("severity")
                .and_then(|v| v.as_str())
                .map(String::from);
            let outcome = notifications::debug_fire_transient(app.clone(), severity)?;
            serde_json::to_value(outcome).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_notifications" => {
            let result = notifications::get_notifications(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "mark_notifications_read" => {
            let filter: Option<NotificationFilter> = args
                .get("filter")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            notifications::mark_notifications_read(filter, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "dismiss_notifications" => {
            let filter: Option<NotificationFilter> = args
                .get("filter")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            notifications::dismiss_notifications(filter, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }
        "execute_notification_action" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .map(String::from);
            notifications::execute_notification_action(app.clone(), id, action, st).await?;
            Ok(serde_json::json!({ "status": "ok" }))
        }

        // ─── Monitoring & Setup ──────────────────────────────────────────────
        "get_runtime_snapshot" => {
            let snap = monitoring::get_runtime_snapshot(st);
            serde_json::to_value(&snap).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_profiler_snapshot" => {
            let result = monitoring::get_profiler_snapshot(app.clone()).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "check_updates" => {
            let scope = args.get("scope").and_then(|v| v.as_str()).map(String::from);
            let result = setup::check_updates(scope).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "fetch_manifest" => {
            let result = setup::fetch_manifest(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_onboarding_status" => {
            let result = setup::get_onboarding_status(st).await?;
            serde_json::to_value(result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }
        "get_runtime_report" => {
            let result = setup::get_runtime_report(st).await?;
            serde_json::to_value(&result).map_err(|e| VoxIpcError::Internal(e.to_string()))
        }

        _ => Err(VoxIpcError::InvalidArgument(format!(
            "Unknown command: {command}"
        ))),
    }
}
