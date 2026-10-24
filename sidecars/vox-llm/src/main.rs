use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};

use anyhow::Result;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use clap::Parser;
use futures_util::stream::Stream;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

mod compat;
pub use compat::*;

pub mod embedded;
use embedded::EmbeddedProvider;
use services::harness::{ChatMessage, Role};
use services::llm::{
    GenerationInput, GenerationOptions, GenerationPurpose, GenerationRequest, LlmError,
    LlmProvider, LlmStreamEvent, OutputConstraint, ReasoningMode,
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Vox Local LLM Sidecar")]
struct Args {
    #[arg(long)]
    socket: Option<PathBuf>,

    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long)]
    model: Option<PathBuf>,

    #[arg(long, default_value_t = 4096)]
    ctx_size: u32,

    #[arg(long, default_value_t = 4)]
    threads: u32,
}

#[derive(Clone)]
struct AppState {
    model_path: Option<PathBuf>,
    ctx_size: u32,
    threads: u32,
    provider: Arc<Mutex<Option<Arc<EmbeddedProvider>>>>,
}

#[derive(Deserialize, Debug)]
struct ChatCompletionMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatCompletionRequest {
    #[serde(default)]
    model: String,
    messages: Vec<ChatCompletionMessage>,
    #[serde(default)]
    stream: bool,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    max_tokens: Option<u32>,
    #[serde(default)]
    stop: Vec<String>,
    seed: Option<u64>,
}

#[derive(Serialize)]
struct ChatCompletionDelta {
    content: String,
}

#[derive(Serialize)]
struct ChatCompletionChoice {
    delta: ChatCompletionDelta,
    index: usize,
    finish_reason: Option<String>,
}

#[derive(Serialize)]
struct ChatCompletionChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatCompletionChoice>,
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn chat_completions_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Response, (StatusCode, String)> {
    // 1. Ensure provider is loaded
    let provider = {
        let mut guard = state.provider.lock();
        if guard.is_none() {
            let model_path = state.model_path.as_ref().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "No model specified on sidecar startup".to_string(),
                )
            })?;
            let prov = EmbeddedProvider::new(model_path, state.ctx_size, state.threads)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            *guard = Some(Arc::new(prov));
        }
        guard.as_ref().unwrap().clone()
    };

    // 2. Build GenerationRequest
    let messages = payload
        .messages
        .into_iter()
        .map(|m| {
            let role = match m.role.to_lowercase().as_str() {
                "system" => Role::System,
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                _ => Role::User,
            };
            ChatMessage {
                role,
                content: m.content,
            }
        })
        .collect();

    let options = GenerationOptions {
        temperature: payload.temperature,
        top_p: payload.top_p,
        top_k: payload.top_k,
        max_output_tokens: payload.max_tokens,
        stop: payload.stop,
        seed: payload.seed,
        reasoning: ReasoningMode::Disabled,
    };

    let req = GenerationRequest {
        input: GenerationInput { messages },
        options,
        purpose: GenerationPurpose::Conversation,
        output: OutputConstraint::Text,
    };

    let cancel = CancellationToken::new();
    let (tx, rx) = mpsc::channel();

    // 3. Spawn generation
    tokio::task::spawn_blocking(move || {
        let cancel_clone = cancel.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let _ = provider.generate(req, 1, &cancel_clone, &tx).await;
        });
    });

    // 4. Stream response
    let (sse_tx, sse_rx) = tokio::sync::mpsc::channel::<Result<Event, std::convert::Infallible>>(64);
    tokio::task::spawn_blocking(move || {
        while let Ok(event) = rx.recv() {
            match event {
                LlmStreamEvent::Token(token) => {
                    let chunk = ChatCompletionChunk {
                        id: "chatcmpl-sidecar".to_string(),
                        object: "chat.completion.chunk".to_string(),
                        created: 1,
                        model: "local".to_string(),
                        choices: vec![ChatCompletionChoice {
                            delta: ChatCompletionDelta { content: token },
                            index: 0,
                            finish_reason: None,
                        }],
                    };
                    if let Ok(json_str) = serde_json::to_string(&chunk) {
                        let _ = sse_tx.blocking_send(Ok(Event::default().data(json_str)));
                    }
                }
                LlmStreamEvent::Finished { .. } => {
                    let _ = sse_tx.blocking_send(Ok(Event::default().data("[DONE]")));
                    break;
                }
                LlmStreamEvent::Error(err) => {
                    log::error!("Sidecar LLM stream error: {}", err);
                    break;
                }
                _ => {}
            }
        }
    });

    let sse_stream = tokio_stream::wrappers::ReceiverStream::new(sse_rx);
    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()).into_response())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let state = AppState {
        model_path: args.model.clone(),
        ctx_size: args.ctx_size,
        threads: args.threads,
        provider: Arc::new(Mutex::new(None)),
    };

    // Pre-initialize provider if model path provided
    if let Some(ref model_path) = args.model {
        if model_path.exists() {
            if let Ok(prov) = EmbeddedProvider::new(model_path, args.ctx_size, args.threads) {
                *state.provider.lock() = Some(Arc::new(prov));
            }
        }
    }

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .with_state(state);

    if let Some(sock_path) = args.socket {
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }
        if let Some(parent) = sock_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let listener = tokio::net::UnixListener::bind(&sock_path)?;
        println!("VOX_SIDECAR_READY socket={}", sock_path.display());
        axum::serve(listener, app).await?;
    } else {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local_port = listener.local_addr()?.port();
        println!("VOX_SIDECAR_READY port={}", local_port);
        axum::serve(listener, app).await?;
    }

    Ok(())
}
