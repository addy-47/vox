use std::sync::Arc;

use async_trait::async_trait;
use tauri::{AppHandle, Manager, State, Wry};

use crate::{
    core::state::AppState,
    ipc,
    services::remote::{
        control::{classify_command, CommandClass},
        protocol::{
            errors::RemoteErrorCode,
            messages::{ControlMessage, ErrResponse, Req, Res},
        },
        security::redact_settings_value,
        session::CommandGateway,
    },
};

pub struct RealCommandGateway {
    pub app_handle: AppHandle<Wry>,
    pub state: Arc<AppState>,
}

#[async_trait]
impl CommandGateway for RealCommandGateway {
    async fn dispatch(&self, req: Req) -> ControlMessage {
        let app = self.app_handle.clone();
        let st: State<'_, Arc<AppState>> = app.state();

        let domain = req.args.get("domain").and_then(|v| v.as_str());
        let key = req.args.get("key").and_then(|v| v.as_str());

        if req.command == "apply_settings_patch" {
            if let Some(patch) = req.args.get("patch").and_then(|v| v.as_object()) {
                let any_denied = patch.iter().any(|(d, v)| {
                    v.as_object().is_some_and(|keys| {
                        keys.keys().any(|k| {
                            classify_command("update_setting", Some(d), Some(k))
                                == CommandClass::Denied
                        })
                    })
                });
                if any_denied {
                    return ControlMessage::Err(ErrResponse {
                        request_id: req.request_id,
                        code: RemoteErrorCode::Denied,
                        message: "One or more settings in patch are not permitted remotely".into(),
                    });
                }
            }
        } else if matches!(
            classify_command(&req.command, domain, key),
            CommandClass::Local | CommandClass::Denied
        ) {
            return ControlMessage::Err(ErrResponse {
                request_id: req.request_id,
                code: RemoteErrorCode::Denied,
                message: format!("Command '{}' is not permitted remotely", req.command),
            });
        }

        match ipc::dispatcher::dispatch_command(&app, st.inner(), &req.command, &req.args).await {
            Ok(mut val) => {
                if req.command == "get_settings" {
                    if let Some(settings) = val.get_mut("settings") {
                        redact_settings_value(settings);
                    }
                }
                ControlMessage::Res(Res {
                    request_id: req.request_id,
                    ok: true,
                    payload: val,
                })
            }
            Err(e) => ControlMessage::Err(ErrResponse {
                request_id: req.request_id,
                code: RemoteErrorCode::Denied,
                message: e.to_string(),
            }),
        }
    }
}
