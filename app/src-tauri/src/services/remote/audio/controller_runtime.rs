//! Controller audio runtime managing speaker playout and microphone VAD streaming.
//!
//! Spec §7.6:
//! - Downlink: Decodes 24kHz Mono PCM16 frames from host TTS, upsamples to 48kHz,
//!   and plays through the controller speaker via CPAL/Oboe.
//! - Uplink: Captures 16kHz Mono audio from the microphone, processes through VAD,
//!   and transmits SpeechStart, PCM16 audio frames, and SpeechEnd over the audio WebSocket.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
    thread::Builder,
    time::Duration,
};

use anyhow::Result;
use cpal::traits::{DeviceTrait, StreamTrait};
use parking_lot::Mutex;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use tokio::sync::RwLock as AsyncRwLock;
use tokio_util::sync::CancellationToken;

use crate::{
    paths,
    services::{
        audio::{
            device::{resolve_audio_host, resolve_output_device_and_config, AudioStream},
            resampler::upsample_2x_into,
            SharedRingProducer, PLAYBACK_BUFFER_SAMPLES,
        },
        remote::{
            audio::sender::NetworkAudioSender,
            link::RemoteLink,
            protocol::{framing::AudioPayload, messages::AudioControlMessage},
        },
        vad::{
            create_vad_instance_from_settings,
            factory::VadInitOutput,
            providers::{EarshotVadEngine, VadBackend, VadEngine},
            PreRollBuffer, VadBackendOption, VadSettings, VAD_CHUNK_SIZE, VAD_PRE_ROLL_CAPACITY,
        },
    },
};

/// Minimum consecutive speech frames (3 frames = ~48ms @ 256 samples/16kHz) to confirm speech onset.
const CONTROLLER_ONSET_CONFIRM_FRAMES: usize = 3;

/// Silence hangover frames (25 frames = ~400ms @ 256 samples/16kHz) to detect speech end.
const CONTROLLER_HANGOVER_FRAMES: usize = 25;

#[allow(dead_code)]
struct SafeStream(cpal::Stream);
unsafe impl Send for SafeStream {}
unsafe impl Sync for SafeStream {}

/// Starts the controller audio playout and microphone capture pipeline for a connected remote session.
pub fn spawn_controller_audio_runtime(
    audio_link: Arc<dyn RemoteLink>,
    cancel: CancellationToken,
    vad_settings: VadSettings,
) -> Result<()> {
    log::info!("[Remote::ControllerAudio] Spawning controller audio runtime");

    // ─── 1. Downlink Playout Task ─────────────────────────────────────────────
    let host = resolve_audio_host();
    let (output_device, output_config) = resolve_output_device_and_config(&host)?;

    let rb = ringbuf::HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
    let (play_prod, mut play_cons) = rb.split();
    let play_prod = Arc::new(Mutex::new(play_prod));

    let channels = output_config.channels as usize;
    let stream = output_device.build_output_stream(
        &output_config,
        move |output: &mut [f32], _| {
            for frame in output.chunks_mut(channels) {
                let sample = play_cons.try_pop().unwrap_or(0.0);
                for out in frame.iter_mut() {
                    *out = sample;
                }
            }
        },
        move |err| {
            log::error!("[Remote::ControllerAudio] Output stream error: {err}");
        },
        None,
    )?;

    stream.play()?;

    // Downlink reader task receiving frames over audio WebSocket
    let play_link = Arc::clone(&audio_link);
    let play_cancel = cancel.clone();
    let play_prod_clone = Arc::clone(&play_prod);
    let stream_guard = SafeStream(stream);

    tokio::spawn(async move {
        let _stream = stream_guard; // Keep stream alive while task runs
        let mut scratch_48k = Vec::with_capacity(2048);

        while !play_cancel.is_cancelled() {
            tokio::select! {
                _ = play_cancel.cancelled() => break,
                res = play_link.recv_audio() => {
                    match res {
                        Ok(AudioPayload::Frame(frame)) => {
                            let f32_chunk: Vec<f32> = frame
                                .samples
                                .iter()
                                .map(|&s| s as f32 / 32768.0)
                                .collect();

                            scratch_48k.clear();
                            upsample_2x_into(&f32_chunk, &mut scratch_48k);

                            let mut prod = play_prod_clone.lock();
                            for &s in &scratch_48k {
                                let _ = prod.try_push(s);
                            }
                        }
                        Ok(AudioPayload::Control(AudioControlMessage::Flush(_))) => {
                            // Flush active audio
                            let mut prod = play_prod_clone.lock();
                            while prod.try_push(0.0).is_ok() {}
                        }
                        Ok(_) => {}
                        Err(e) => {
                            log::debug!("[Remote::ControllerAudio] Downlink recv ended: {e}");
                            break;
                        }
                    }
                }
            }
        }
    });

    // ─── 2. Uplink Capture & VAD Worker ───────────────────────────────────────
    let cap_rb = ringbuf::HeapRb::<f32>::new(16384);
    let (cap_prod, mut cap_cons) = cap_rb.split();
    let shared_prod = SharedRingProducer::new(cap_prod);
    let ingestion_gate = Arc::new(AtomicBool::new(true));

    let audio_stream = AudioStream::new(shared_prod, Arc::clone(&ingestion_gate))?;
    audio_stream.start()?;

    let models_dir = paths::get().models.clone();
    let vad_init =
        create_vad_instance_from_settings(&vad_settings, &models_dir).unwrap_or_else(|e| {
            log::warn!("[Remote::ControllerAudio] VAD init error: {e}; falling back to Earshot");
            let engine = EarshotVadEngine::new(0.5).expect("Earshot fallback must succeed");
            VadInitOutput {
                backend: VadBackend::Earshot(engine),
                fallback_from: Some(VadBackendOption::Earshot),
                fallback_reason: Some(e),
            }
        });

    let mut vad_backend = vad_init.backend;
    let sender_slot = Arc::new(AsyncRwLock::new(Some(Arc::clone(&audio_link))));
    let epoch = Arc::new(AtomicU64::new(1));
    let sender = Arc::new(NetworkAudioSender::new(sender_slot, epoch));

    let uplink_cancel = cancel.clone();
    Builder::new()
        .name("vox-ctrl-audio".to_string())
        .spawn(move || {
            let _stream_guard = audio_stream;
            let mut pre_roll = PreRollBuffer::new(VAD_PRE_ROLL_CAPACITY);
            let mut in_speech = false;
            let mut onset_frames = 0;
            let mut silence_frames = 0;
            let mut sample_count = 0u64;
            let mut chunk = vec![0.0f32; VAD_CHUNK_SIZE];

            while !uplink_cancel.is_cancelled() {
                if cap_cons.occupied_len() < VAD_CHUNK_SIZE {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }

                for s in chunk.iter_mut() {
                    *s = cap_cons.try_pop().unwrap_or(0.0);
                }

                let is_speech = vad_backend.predict(&chunk);

                if !in_speech {
                    pre_roll.push(&chunk);
                    if is_speech {
                        onset_frames += 1;
                        if onset_frames >= CONTROLLER_ONSET_CONFIRM_FRAMES {
                            in_speech = true;
                            onset_frames = 0;
                            silence_frames = 0;
                            sample_count = 0;

                            let sender_clone = Arc::clone(&sender);
                            let mut pre_roll_samples = Vec::new();
                            pre_roll.copy_into(&mut pre_roll_samples);
                            pre_roll.clear();
                            let current_chunk = chunk.clone();

                            tauri::async_runtime::spawn(async move {
                                sender_clone.send_speech_start().await;
                                if !pre_roll_samples.is_empty() {
                                    sender_clone.send_samples(&pre_roll_samples).await;
                                }
                                sender_clone.send_samples(&current_chunk).await;
                            });
                        }
                    } else {
                        onset_frames = 0;
                    }
                } else {
                    sample_count += chunk.len() as u64;
                    let sender_clone = Arc::clone(&sender);
                    let current_chunk = chunk.clone();
                    tauri::async_runtime::spawn(async move {
                        sender_clone.send_samples(&current_chunk).await;
                    });

                    if is_speech {
                        silence_frames = 0;
                    } else {
                        silence_frames += 1;
                        if silence_frames >= CONTROLLER_HANGOVER_FRAMES {
                            in_speech = false;
                            silence_frames = 0;
                            let sender_clone = Arc::clone(&sender);
                            let count = sample_count;
                            tauri::async_runtime::spawn(async move {
                                sender_clone.send_speech_end(count).await;
                            });
                        }
                    }
                }
            }

            log::info!("[Remote::ControllerAudio] Controller audio thread exited cleanly");
        })?;

    Ok(())
}
