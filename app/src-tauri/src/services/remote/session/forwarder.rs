//! FIFO outbound event queue with telemetry throttling and token coalescing.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::mpsc;

use crate::services::remote::{
    link::RemoteLink,
    protocol::messages::{ControlMessage, Event},
};

pub type EventForwarderSender = mpsc::Sender<(String, serde_json::Value)>;

pub async fn run_event_forwarder(
    mut rx: tokio::sync::mpsc::Receiver<(String, serde_json::Value)>,
    link_slot: Arc<tokio::sync::RwLock<Option<Arc<dyn RemoteLink>>>>,
) {
    let mut last_telemetry = Instant::now() - Duration::from_secs(1);
    let mut pending_token: Option<(u32, String)> = None;
    let mut token_deadline: Option<Instant> = None;

    loop {
        // Determine sleep until pending token deadline.
        let sleep_fut = if let Some(d) = token_deadline {
            let now = Instant::now();
            if d <= now {
                None
            } else {
                Some(tokio::time::sleep_until(tokio::time::Instant::from_std(d)))
            }
        } else {
            None
        };

        let msg = if let Some(sleep) = sleep_fut {
            tokio::select! {
                biased;
                _ = sleep => {
                    // flush pending token
                    if let Some((turn, text)) = pending_token.take() {
                        let ev = Event { event_name: "llm_token".to_string(), payload: serde_json::json!({"turn_id": turn, "token": text}) };
                        if let Some(link) = link_slot.read().await.clone() {
                            let _ = link.send_control(ControlMessage::Event(ev)).await;
                        }
                    }
                    token_deadline = None;
                    continue;
                }
                m = rx.recv() => m,
            }
        } else {
            rx.recv().await
        };

        let Some((name, payload)) = msg else {
            // channel closed: flush pending token
            if let Some((turn, text)) = pending_token.take() {
                let ev = Event {
                    event_name: "llm_token".to_string(),
                    payload: serde_json::json!({"turn_id": turn, "token": text}),
                };
                if let Some(link) = link_slot.read().await.clone() {
                    let _ = link.send_control(ControlMessage::Event(ev)).await;
                }
            }
            break;
        };

        // Flush pending token before a non-token event to preserve order.
        if pending_token.is_some() && name != "llm_token" {
            if let Some((turn, text)) = pending_token.take() {
                let ev = Event {
                    event_name: "llm_token".to_string(),
                    payload: serde_json::json!({"turn_id": turn, "token": text}),
                };
                if let Some(link) = link_slot.read().await.clone() {
                    let _ = link.send_control(ControlMessage::Event(ev)).await;
                }
            }
            token_deadline = None;
        }

        if name == "telemetry" {
            if last_telemetry.elapsed() < Duration::from_millis(33) {
                continue;
            }
            last_telemetry = Instant::now();
        }

        if name == "llm_token" {
            let turn = payload.get("turn_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let tok = payload
                .get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some((pt, pt_text)) = &mut pending_token {
                if *pt == turn {
                    pt_text.push_str(&tok);
                } else {
                    // flush previous turn's token
                    let prev_text = std::mem::take(pt_text);
                    let prev_turn = *pt;
                    let ev = Event {
                        event_name: "llm_token".to_string(),
                        payload: serde_json::json!({"turn_id": prev_turn, "token": prev_text}),
                    };
                    if let Some(link) = link_slot.read().await.clone() {
                        let _ = link.send_control(ControlMessage::Event(ev)).await;
                    }
                    *pt = turn;
                    pt_text.push_str(&tok);
                }
            } else {
                pending_token = Some((turn, tok));
            }
            if token_deadline.is_none() {
                token_deadline = Some(Instant::now() + Duration::from_millis(50));
            }
            // If deadline already passed, flush immediately.
            if let Some(d) = token_deadline {
                if d <= Instant::now() {
                    if let Some((t, text)) = pending_token.take() {
                        let ev = Event {
                            event_name: "llm_token".to_string(),
                            payload: serde_json::json!({"turn_id": t, "token": text}),
                        };
                        if let Some(link) = link_slot.read().await.clone() {
                            let _ = link.send_control(ControlMessage::Event(ev)).await;
                        }
                    }
                    token_deadline = None;
                }
            }
            continue;
        }

        let event = Event {
            event_name: name,
            payload,
        };
        if let Some(link) = link_slot.read().await.clone() {
            let _ = link.send_control(ControlMessage::Event(event)).await;
        }
    }
}
