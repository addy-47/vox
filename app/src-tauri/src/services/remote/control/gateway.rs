use std::sync::Arc;

use async_trait::async_trait;
use tauri::{AppHandle, Manager, State, Wry};

use crate::core::{error::VoxIpcError, state::AppState};
use crate::ipc;
use crate::services::remote::{
    protocol::{
        errors::RemoteErrorCode,
        messages::{ControlMessage, ErrResponse, Req, Res},
    },
    session::CommandGateway,
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

#[async_trait]
impl CommandGateway for RealCommandGateway {
    async fn dispatch(&self, req: Req) -> ControlMessage {
        let app = self.app_handle.clone();
        let st: State<'_, Arc<AppState>> = app.state();
        let rid = req.request_id;

        match req.command.as_str() {
            "launch_engine" => match ipc::pipeline::launch_engine(app.clone()).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "stop_engine" => match ipc::pipeline::stop_engine(app.clone()).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "start_session" => {
                let session_id = req.args.get("session_id").and_then(|v| v.as_i64());
                match ipc::pipeline::start_session(session_id, app.clone(), st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }
            "end_session" => match ipc::pipeline::end_session(app.clone(), st).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "pause_session" => match ipc::pipeline::pause_session(app.clone(), st).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "resume_session" => match ipc::pipeline::resume_session(app.clone(), st).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "submit_text_input" => {
                let query = req
                    .args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                match ipc::pipeline::submit_text_input(query, app.clone(), st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }
            "set_playback_muted" => {
                let muted = req.args.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
                match ipc::pipeline::set_playback_muted(muted, app.clone(), st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }
            "set_mic_muted" => {
                let muted = req.args.get("muted").and_then(|v| v.as_bool()).unwrap_or(false);
                match ipc::pipeline::set_mic_muted(muted, app.clone(), st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }
            "set_session_private_mode" => {
                let enabled = req.args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                match ipc::pipeline::set_session_private_mode(enabled, app.clone(), st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }
            "ptt_start" => match ipc::pipeline::ptt_start(app.clone(), st).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "ptt_stop" => match ipc::pipeline::ptt_stop(app.clone(), st).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "ptt_cancel" => match ipc::pipeline::ptt_cancel(app.clone(), st).await {
                Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                Err(e) => err(rid, e),
            },
            "get_settings" => match ipc::settings::get_settings(app.clone()).await {
                Ok(boot) => {
                    let mut val = serde_json::to_value(&boot).unwrap_or_default();
                    if let Some(settings) = val.get_mut("settings") {
                        crate::services::remote::security::redact_settings_value(settings);
                    }
                    ok(rid, val)
                }
                Err(e) => err(rid, e),
            },
            "get_settings_contract" => match ipc::settings::get_settings_contract().await {
                Ok(c) => ok(rid, serde_json::to_value(c).unwrap_or_default()),
                Err(e) => err(rid, e),
            },
            "update_setting" => {
                let domain = req
                    .args
                    .get("domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let key = req.args.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let value = req.args.get("value").cloned().unwrap_or(serde_json::Value::Null);
                match ipc::settings::update_setting(domain, key, value, app.clone()).await {
                    Ok(r) => ok(rid, serde_json::to_value(r).unwrap_or_default()),
                    Err(e) => err(rid, e),
                }
            }
            "reset_settings" => match ipc::settings::reset_settings(app.clone()).await {
                Ok(r) => ok(rid, serde_json::to_value(r).unwrap_or_default()),
                Err(e) => err(rid, e),
            },
            "apply_settings_patch" => {
                let patch = req.args.get("patch").cloned().unwrap_or(serde_json::json!({}));
                match ipc::settings::apply_settings_patch(patch, app.clone()).await {
                    Ok(r) => ok(rid, serde_json::to_value(r).unwrap_or_default()),
                    Err(e) => err(rid, e),
                }
            }
            "list_paired_devices" => match ipc::remote::list_paired_devices(st).await {
                Ok(v) => ok(rid, serde_json::to_value(v).unwrap_or_default()),
                Err(e) => err(rid, e),
            },
            "get_link_state" => ok(rid, serde_json::to_value(st.remote_manager.state_dto()).unwrap_or_default()),
            "get_command_routing" => ok(rid, serde_json::to_value(crate::services::remote::control::get_routing_table().clone()).unwrap_or_default()),
            "revoke_device" => {
                let device_id = req.args.get("device_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::remote::revoke_device(device_id, st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }
            "disconnect_device" => {
                let device_id = req.args.get("device_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match ipc::remote::disconnect_device(device_id, st).await {
                    Ok(()) => ok(rid, serde_json::json!({"status":"ok"})),
                    Err(e) => err(rid, e),
                }
            }

            _ => ControlMessage::Err(ErrResponse {
                request_id: rid,
                code: RemoteErrorCode::Denied,
                message: format!("Command '{}' is not implemented on the remote gateway", req.command),
            }),
        }
    }
}