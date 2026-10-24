use std::{
    path::PathBuf,
    sync::{
        atomic::AtomicBool,
        mpsc, Arc,
    },
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

pub mod azure_tts;
pub mod google_tts;

use azure_tts::AzureTtsProvider;
use google_tts::GoogleTtsProvider;

#[derive(Parser, Debug)]
#[command(author, version, about = "Vox Cloud TTS Sidecar")]
struct Args {
    #[arg(long)]
    socket: Option<PathBuf>,

    #[arg(long, default_value_t = 0)]
    port: u16,
}

#[derive(Clone)]
struct AppState {}

#[derive(Deserialize)]
struct GoogleSpeechReq {
    voice: String,
    text: String,
    speed: Option<f32>,
    project_override: Option<String>,
}

#[derive(Deserialize)]
struct GoogleVoicesReq {
    #[serde(default)]
    regions: Vec<String>,
}

#[derive(Deserialize)]
struct AzureSpeechReq {
    region_or_endpoint: String,
    voice: String,
    #[serde(default)]
    key: String,
    resource_id: Option<String>,
    text: String,
    speed: Option<f32>,
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn google_speech_handler(
    State(_): State<AppState>,
    Json(payload): Json<GoogleSpeechReq>,
) -> Result<Response, (StatusCode, String)> {
    let provider = GoogleTtsProvider::new(&payload.voice, payload.project_override.as_deref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if let Some(spd) = payload.speed {
        provider.set_speed(spd);
    }

    let (pcm_tx, mut pcm_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(32);
    let playback = services::audio::PlaybackEngine::new(pcm_tx);
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

    let text = payload.text;
    tokio::task::spawn_blocking(move || {
        let _ = provider.synthesize_chunk(&text, &ctx);
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

#[derive(serde::Serialize)]
struct RemoteVoiceEntry {
    name: String,
    short_name: String,
    gender: String,
    locale: String,
    friendly_name: String,
}

async fn google_voices_handler(
    State(_): State<AppState>,
    Json(payload): Json<GoogleVoicesReq>,
) -> Result<Json<Vec<RemoteVoiceEntry>>, (StatusCode, String)> {
    let client = google_cloud_texttospeech_v1::client::TextToSpeech::builder()
        .build()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let regions = if payload.regions.is_empty() {
        vec!["en-US".to_string(), "en-GB".to_string(), "en-AU".to_string(), "en-IN".to_string()]
    } else {
        payload.regions
    };

    let mut voices = Vec::new();
    for region in regions {
        if let Ok(resp) = client.list_voices().set_language_code(&region).send().await {
            for v in resp.voices {
                if !v.name.contains("Chirp3-HD") {
                    continue;
                }
                let locale = v.language_codes.first().cloned().unwrap_or_default();
                let style = v.name.rsplit('-').next().unwrap_or(&v.name).to_string();
                let gender = match v.ssml_gender {
                    google_cloud_texttospeech_v1::model::SsmlVoiceGender::Male => "male".to_string(),
                    google_cloud_texttospeech_v1::model::SsmlVoiceGender::Female => "female".to_string(),
                    _ => "neutral".to_string(),
                };
                voices.push(RemoteVoiceEntry {
                    name: v.name.clone(),
                    short_name: v.name,
                    gender,
                    locale: locale.clone(),
                    friendly_name: format!("{style} ({locale})"),
                });
            }
        }
    }

    Ok(Json(voices))
}

async fn azure_speech_handler(
    State(_): State<AppState>,
    Json(payload): Json<AzureSpeechReq>,
) -> Result<Response, (StatusCode, String)> {
    let provider = AzureTtsProvider::new(
        &payload.region_or_endpoint,
        &payload.voice,
        &payload.key,
        payload.resource_id.as_deref(),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if let Some(spd) = payload.speed {
        provider.set_speed(spd);
    }

    let (pcm_tx, mut pcm_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(32);
    let playback = services::audio::PlaybackEngine::new(pcm_tx);
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

    let text = payload.text;
    tokio::task::spawn_blocking(move || {
        let _ = provider.synthesize_chunk(&text, &ctx);
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let state = AppState {};
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/google/speech", post(google_speech_handler))
        .route("/v1/google/voices", post(google_voices_handler))
        .route("/v1/azure/speech", post(azure_speech_handler))
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
            tracing::info!("Listening on Unix domain socket: {:?}", socket_path);

            loop {
                let (stream, _) = listener.accept().await?;
                let app = app.clone();
                tokio::spawn(async move {
                    let io = hyper_util::rt::TokioIo::new(stream);
                    let service = tower::service_fn(move |req| {
                        let mut app = app.clone();
                        use tower::Service;
                        async move { app.call(req).await }
                    });
                    let _ = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                        .serve_connection(io, service)
                        .await;
                });
            }
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("Unix domain sockets not supported on this platform");
        }
    } else {
        let addr = format!("127.0.0.1:{}", args.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        tracing::info!("Listening on TCP: {}", listener.local_addr()?);
        axum::serve(listener, app).await?;
    }

    Ok(())
}
