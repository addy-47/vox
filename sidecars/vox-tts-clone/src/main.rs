use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc, Arc},
};

use anyhow::Result;
use axum::{
    body::Body,
    extract::{Json, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use clap::Parser;
use serde::Deserialize;
use tokio_util::io::ReaderStream;

mod compat;
pub use compat::*;

pub mod chatterbox;
use chatterbox::ChatterboxEngine;

#[derive(Parser, Debug)]
#[command(author, version, about = "Vox Voice Cloning Sidecar")]
struct Args {
    #[arg(long)]
    socket: Option<PathBuf>,

    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long, default_value = "")]
    model_dir: PathBuf,
}

#[derive(Clone)]
struct AppState {
    model_dir: Arc<PathBuf>,
}

#[derive(Deserialize)]
struct SpeechReq {
    text: String,
    speed: Option<f32>,
    reference_audio: Option<String>,
}

#[derive(Deserialize)]
struct PreBakeReq {
    source_wav: PathBuf,
    baked_dir: PathBuf,
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn speech_handler(
    State(state): State<AppState>,
    Json(payload): Json<SpeechReq>,
) -> Result<Response, (StatusCode, String)> {
    let speed = payload.speed.unwrap_or(1.0);
    let engine = ChatterboxEngine::new(
        &state.model_dir,
        "en",
        speed,
        payload.reference_audio.as_deref(),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (pcm_tx, mut pcm_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(32);
    let playback = services::audio::PlaybackEngine::new(pcm_tx);

    let text = payload.text;
    tokio::task::spawn_blocking(move || {
        let (event_tx, _event_rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let ctx = SynthesisContext {
            turn_id: 1,
            intent: core::events::AudioIntent::Speech,
            cancel,
            playback: &playback,
            event_tx,
            telemetry_rtf: None,
        };
        let _ = engine.synthesize_chunk(&text, &ctx);
    });

    let (byte_tx, byte_rx) = tokio::io::duplex(65536);
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let mut sink = byte_tx;
        while let Some(f32_samples) = pcm_rx.recv().await {
            let mut bytes = Vec::with_capacity(f32_samples.len() * 2);
            for s in f32_samples {
                let clamped = s.clamp(-1.0, 1.0);
                let s16 = (clamped * 32767.0) as i16;
                bytes.extend_from_slice(&s16.to_le_bytes());
            }
            if sink.write_all(&bytes).await.is_err() {
                break;
            }
        }
    });

    let stream = ReaderStream::new(byte_rx);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/pcm;rate=24000;channels=1")
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn pre_bake_handler(
    State(state): State<AppState>,
    Json(payload): Json<PreBakeReq>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    use chatterbox_rs::{Engine, EngineOptions};

    let t3_path = state.model_dir.join("t3.gguf");
    let s3_path = state.model_dir.join("s3gen.gguf");

    if !t3_path.exists() || !s3_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Chatterbox models not found in {:?}", state.model_dir),
        ));
    }

    std::fs::create_dir_all(&payload.baked_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let engine = Engine::new(EngineOptions {
        t3_gguf_path: t3_path.to_string_lossy().into_owned(),
        s3gen_gguf_path: s3_path.to_string_lossy().into_owned(),
        reference_audio: payload.source_wav.to_string_lossy().into_owned(),
        verbose: false,
        ..Default::default()
    })
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    engine
        .save_voice(&payload.baked_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::OK, "Baked successfully"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let state = AppState {
        model_dir: Arc::new(args.model_dir),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/audio/speech", post(speech_handler))
        .route("/v1/voices/pre-bake", post(pre_bake_handler))
        .with_state(state);

    if let Some(socket_path) = args.socket {
        #[cfg(unix)]
        {
            if socket_path.exists() {
                let _ = std::fs::remove_file(&socket_path);
            }
            if let Some(parent) = socket_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let listener = tokio::net::UnixListener::bind(&socket_path)?;
            println!("VOX_SIDECAR_READY socket={}", socket_path.display());
            axum::serve(listener, app).await?;
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("Unix domain sockets not supported on this platform");
        }
    } else {
        let addr = format!("127.0.0.1:{}", args.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        println!("VOX_SIDECAR_READY port={}", listener.local_addr()?.port());
        axum::serve(listener, app).await?;
    }

    Ok(())
}
