use std::sync::Arc;

use async_trait::async_trait;
use tauri::{AppHandle, Manager, State, Wry};

use crate::{
    core::{error::VoxIpcError, state::AppState},
    ipc::{self, memory::ResolveRevisionsRequest},
    persistence::notifications::NotificationFilter,
    services::{
        health::ProviderConfigPayload,
        llm::config::LlmRuntimeTarget,
        remote::{
            control::{classify_command, CommandClass},
            protocol::{
                errors::RemoteErrorCode,
                messages::{ControlMessage, ErrResponse, Req, Res},
            },
            security::redact_settings_value,
            session::CommandGateway,
        },
    },
};

pub struct RealCommandGateway {
    pub app_handle: AppHandle<Wry>,
    pub state: Arc<AppState>,
}

fn ok(request_id: u64, payload: serde_json::Value) -> ControlMessage {
    ControlMessage::Res(Res {
        request_id,
        ok: true,
        payload,
    })
}

fn err(request_id: u64, e: VoxIpcError) -> ControlMessage {
    ControlMessage::Err(ErrResponse {
        request_id,
        code: RemoteErrorCode::Denied,
        message: e.to_string(),
    })
}

impl RealCommandGateway {
    async fn dispatch_pipeline_core(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "launch_engine" => match ipc::pipeline::launch_engine(app.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "stop_engine" => match ipc::pipeline::stop_engine(app.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "start_session" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64());
                match ipc::pipeline::start_session(session_id, app.clone(), st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "end_session" => match ipc::pipeline::end_session(app.clone(), st.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "pause_session" => match ipc::pipeline::pause_session(app.clone(), st.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "resume_session" => match ipc::pipeline::resume_session(app.clone(), st.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "submit_text_input" => {
                let query = req.args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::pipeline::submit_text_input(query, app.clone(), st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_pipeline_controls(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "set_playback_muted" => {
                let muted = req.args.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
                match ipc::pipeline::set_playback_muted(muted, app.clone(), st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "set_mic_muted" => {
                let muted = req.args.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
                match ipc::pipeline::set_mic_muted(muted, app.clone(), st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "set_session_private_mode" => {
                let enabled = req.args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                match ipc::pipeline::set_session_private_mode(enabled, app.clone(), st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "ptt_start" => match ipc::pipeline::ptt_start(app.clone(), st.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "ptt_stop" => match ipc::pipeline::ptt_stop(app.clone(), st.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            "ptt_cancel" => match ipc::pipeline::ptt_cancel(app.clone(), st.clone()).await {
                Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                Err(e) => Some(err(rid, e)),
            },
            _ => None,
        }
    }

    async fn dispatch_settings_command(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_settings" => match ipc::settings::get_settings(app.clone()).await {
                Ok(boot) => {
                    let mut val = serde_json::to_value(&boot).unwrap_or_default();
                    if let Some(settings) = val.get_mut("settings") {
                        redact_settings_value(settings);
                    }
                    Some(ok(rid, val))
                }
                Err(e) => Some(err(rid, e)),
            },
            "get_settings_contract" => match ipc::settings::get_settings_contract().await {
                Ok(c) => Some(ok(rid, serde_json::to_value(c).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "get_restart_status" => match ipc::settings::get_restart_status(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "update_setting" => {
                let domain = req.args.get("domain").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let key = req.args.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let value = req.args.get("value").cloned().unwrap_or(serde_json::Value::Null);
                match ipc::settings::update_setting(domain, key, value, app.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "reset_settings" => match ipc::settings::reset_settings(app.clone()).await {
                Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "apply_settings_patch" => {
                let patch = req.args.get("patch").cloned().unwrap_or(serde_json::json!({}));
                match ipc::settings::apply_settings_patch(patch, app.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_session_crud(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "create_session" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::pipeline::create_session(project_id, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "continue_session" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
                match ipc::pipeline::continue_session(session_id, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "update_session" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = req.args.get("title").and_then(|v| v.as_str()).map(String::from);
                let is_pinned = req.args.get("is_pinned").and_then(|v| v.as_bool());
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::persistence::update_session(app.clone(), session_id, title, is_pinned, project_id, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "delete_session" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let hard = req.args.get("hard").and_then(|v| v.as_bool());
                match ipc::persistence::delete_session(app.clone(), session_id, hard, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_history_query(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_sessions" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::persistence::get_sessions(project_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "get_turns" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
                match ipc::persistence::get_turns(session_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "search_history" => {
                let query = req.args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                let limit = req.args.get("limit").and_then(|v| v.as_u64()).map(|u| u as u32);
                match ipc::persistence::search_history(query, project_id, limit, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "compact_session" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64()).unwrap_or(0);
                match ipc::persistence::compact_session(app.clone(), session_id, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "get_transcript_history" => match ipc::persistence::get_transcript_history(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            _ => None,
        }
    }

    async fn dispatch_project_command(
        &self,
        req: &Req,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_projects" => match ipc::projects::get_projects(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "create_project" => {
                let name = req.args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::projects::create_project(st.clone(), name).await {
                    Ok(p) => Some(ok(rid, serde_json::to_value(p).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "rename_project" => {
                let id = req.args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = req.args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::projects::rename_project(st.clone(), id, name).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "delete_project" => {
                let id = req.args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::projects::delete_project(st.clone(), id).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_memory_crud(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_personal_memory" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::get_personal_memory(project_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "save_personal_memory" => {
                let content = req.args.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let expected_version = req.args.get("expected_version").and_then(|v| v.as_u64()).unwrap_or(0);
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::save_personal_memory(app.clone(), content, expected_version, project_id, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "consolidate_personal_memory" => {
                let comments = req.args.get("comments").and_then(|v| serde_json::from_value(v.clone()).ok());
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                let forced = req.args.get("forced").and_then(|v| v.as_bool());
                match ipc::memory::consolidate_personal_memory(app.clone(), comments, project_id, forced, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "regenerate_personal_memory" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::regenerate_personal_memory(app.clone(), project_id, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_memory_versions(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_personal_memory_versions" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::get_personal_memory_versions(project_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "set_active_personal_memory_version" => {
                let version = req.args.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::set_active_personal_memory_version(app.clone(), version, project_id, st.clone()).await {
                    Ok(r) => Some(ok(rid, serde_json::to_value(r).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "get_observations" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                let status = req.args.get("status").and_then(|v| v.as_str()).map(String::from);
                let limit = req.args.get("limit").and_then(|v| v.as_u64()).map(|u| u as u32);
                let offset = req.args.get("offset").and_then(|v| v.as_u64()).map(|u| u as u32);
                let obs_type = req.args.get("observation_type").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::get_observations(project_id, status, limit, offset, obs_type, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_memory_revisions(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_ingestion_stats" => match ipc::memory::get_ingestion_stats(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "get_memory_revisions" => {
                let project_id = req.args.get("project_id").and_then(|v| v.as_str()).map(String::from);
                match ipc::memory::get_memory_revisions(project_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "resolve_memory_revisions" => {
                let request: ResolveRevisionsRequest = req
                    .args
                    .get("request")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or(ResolveRevisionsRequest {
                        project_id: None,
                        decisions: Vec::new(),
                    });
                match ipc::memory::resolve_memory_revisions(app.clone(), request, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_model_catalog(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_model_catalog" => match ipc::catalog::get_model_catalog(app.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "get_provider_caps" => {
                let provider_id = req.args.get("provider_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::catalog::get_provider_caps(provider_id) {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "check_provider_health" => {
                let kind = req.args.get("kind").and_then(|v| v.as_str()).unwrap_or("llm").to_string();
                let provider: Option<ProviderConfigPayload> = req.args.get("provider").and_then(|v| serde_json::from_value(v.clone()).ok());
                match ipc::catalog::check_provider_health(st.clone(), kind, provider).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "list_favorite_models" => match ipc::catalog::list_favorite_models(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "toggle_model_favorite" => {
                let model_id = req.args.get("model_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::catalog::toggle_model_favorite(model_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_model_probes(
        &self,
        req: &Req,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "probe_provider_target" => {
                let kind = req.args.get("kind").and_then(|v| v.as_str()).unwrap_or("llm").to_string();
                let provider_id = req.args.get("provider_id").and_then(|v| v.as_str()).map(String::from);
                let base_url = req.args.get("base_url").and_then(|v| v.as_str()).map(String::from);
                let api_key = req.args.get("api_key").and_then(|v| v.as_str()).map(String::from);
                let model = req.args.get("model").and_then(|v| v.as_str()).map(String::from);
                match ipc::catalog::probe_provider_target(st.clone(), kind, provider_id, base_url, api_key, model).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "list_llm_models" => {
                let provider: Option<LlmRuntimeTarget> = req.args.get("provider").and_then(|v| serde_json::from_value(v.clone()).ok());
                match ipc::catalog::list_llm_models(st.clone(), provider).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "probe_model_capabilities" => {
                let provider: Option<LlmRuntimeTarget> = req.args.get("provider").and_then(|v| serde_json::from_value(v.clone()).ok());
                let model_id = req.args.get("model_id").and_then(|v| v.as_str()).map(String::from);
                let target_cap = req.args.get("target_cap").and_then(|v| v.as_u64()).map(|u| u as u32);
                match ipc::catalog::probe_model_capabilities(st.clone(), provider, model_id, target_cap).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "get_model_capabilities_cache" => match ipc::catalog::get_model_capabilities_cache().await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            _ => None,
        }
    }

    async fn dispatch_voice_crud(
        &self,
        req: &Req,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "list_voices" => {
                let scope = req.args.get("scope").and_then(|v| v.as_str()).map(String::from);
                match ipc::voices::list_voices(scope, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "get_voice_scope" => {
                let engine = req.args.get("engine").and_then(|v| v.as_str()).map(String::from);
                match ipc::voices::get_voice_scope(engine).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "sync_remote_voices" => match ipc::voices::sync_remote_voices(st.clone()).await {
                Ok(n) => Some(ok(rid, serde_json::json!({ "synced": n }))),
                Err(e) => Some(err(rid, e)),
            },
            "synthesize_voice_preview" => {
                let voice_id = req.args.get("voice_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let prompt = req.args.get("prompt").and_then(|v| v.as_str()).map(String::from);
                let engine = req.args.get("engine").and_then(|v| v.as_str()).map(String::from);
                match ipc::voices::synthesize_voice_preview(voice_id, prompt, engine, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_voice_ops(
        &self,
        req: &Req,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "toggle_voice_favorite" => {
                let voice_id = req.args.get("voice_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::voices::toggle_voice_favorite(voice_id, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "record_voice_used" => {
                let voice_id = req.args.get("voice_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::voices::record_voice_used(voice_id, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "set_active_voice" => {
                let voice_id = req.args.get("voice_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::voices::set_active_voice(voice_id, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "delete_voice" => {
                let id = req.args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::voices::delete_voice(id, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "rename_voice" => {
                let id = req.args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = req.args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::voices::rename_voice(id, name, st.clone()).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    fn dispatch_notification_state(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "notification_permission_state" => match ipc::notifications::notification_permission_state(app.clone()) {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "request_notification_permission" => match ipc::notifications::request_notification_permission(app.clone()) {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "debug_fire_transient" => {
                let severity = req.args.get("severity").and_then(|v| v.as_str()).map(String::from);
                match ipc::notifications::debug_fire_transient(app.clone(), severity) {
                    Ok(outcome) => Some(ok(rid, serde_json::to_value(outcome).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_notification_items(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_notifications" => match ipc::notifications::get_notifications(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "mark_notifications_read" => {
                let filter: Option<NotificationFilter> = req.args.get("filter").and_then(|v| serde_json::from_value(v.clone()).ok());
                match ipc::notifications::mark_notifications_read(filter, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "dismiss_notifications" => {
                let filter: Option<NotificationFilter> = req.args.get("filter").and_then(|v| serde_json::from_value(v.clone()).ok());
                match ipc::notifications::dismiss_notifications(filter, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "execute_notification_action" => {
                let id = req.args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let action = req.args.get("action").and_then(|v| v.as_str()).map(String::from);
                match ipc::notifications::execute_notification_action(app.clone(), id, action, st.clone()).await {
                    Ok(()) => Some(ok(rid, serde_json::json!({"status":"ok"}))),
                    Err(e) => Some(err(rid, e)),
                }
            }
            _ => None,
        }
    }

    async fn dispatch_system_info(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        let rid = req.request_id;
        match req.command.as_str() {
            "get_profiler_snapshot" => match ipc::monitoring::get_profiler_snapshot(app.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "check_updates" => {
                let scope = req.args.get("scope").and_then(|v| v.as_str()).map(String::from);
                match ipc::setup::check_updates(scope).await {
                    Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                    Err(e) => Some(err(rid, e)),
                }
            }
            "fetch_manifest" => match ipc::setup::fetch_manifest(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "get_onboarding_status" => match ipc::setup::get_onboarding_status(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            "get_runtime_report" => match ipc::setup::get_runtime_report(st.clone()).await {
                Ok(v) => Some(ok(rid, serde_json::to_value(v).unwrap_or_default())),
                Err(e) => Some(err(rid, e)),
            },
            _ => None,
        }
    }

    async fn try_dispatch_subsystem(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        if let Some(res) = self.dispatch_pipeline_core(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_pipeline_controls(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_settings_command(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_session_crud(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_history_query(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_project_command(req, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_memory_crud(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_memory_versions(req, app, st).await {
            return Some(res);
        }
        self.dispatch_memory_revisions(req, app, st).await
    }

    async fn try_dispatch_management(
        &self,
        req: &Req,
        app: &AppHandle<Wry>,
        st: &State<'_, Arc<AppState>>,
    ) -> Option<ControlMessage> {
        if let Some(res) = self.dispatch_model_catalog(req, app, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_model_probes(req, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_voice_crud(req, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_voice_ops(req, st).await {
            return Some(res);
        }
        if let Some(res) = self.dispatch_notification_state(req, app) {
            return Some(res);
        }
        if let Some(res) = self.dispatch_notification_items(req, app, st).await {
            return Some(res);
        }
        self.dispatch_system_info(req, app, st).await
    }
}

#[async_trait]
impl CommandGateway for RealCommandGateway {
    async fn dispatch(&self, req: Req) -> ControlMessage {
        let app = self.app_handle.clone();
        let st: State<'_, Arc<AppState>> = app.state();

        let domain = req.args.get("domain").and_then(|v| v.as_str());
        let key = req.args.get("key").and_then(|v| v.as_str());
        if matches!(
            classify_command(&req.command, domain, key),
            CommandClass::Local | CommandClass::Denied
        ) {
            return ControlMessage::Err(ErrResponse {
                request_id: req.request_id,
                code: RemoteErrorCode::Denied,
                message: format!("Command '{}' is not permitted remotely", req.command),
            });
        }

        if let Some(res) = self.try_dispatch_subsystem(&req, &app, &st).await {
            return res;
        }
        if let Some(res) = self.try_dispatch_management(&req, &app, &st).await {
            return res;
        }

        ControlMessage::Err(ErrResponse {
            request_id: req.request_id,
            code: RemoteErrorCode::Denied,
            message: format!(
                "Command '{}' is not implemented on the remote gateway",
                req.command
            ),
        })
    }
}
