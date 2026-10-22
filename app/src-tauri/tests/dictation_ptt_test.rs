//! ============================================================================
//! dictation_ptt_test.rs — PTT lifecycle and dispatch routing seam
//! ============================================================================
//! Category  : Integration (Seam 4 — PTT event → VAD window → dispatch arm)
//! Component : pipeline/assistant/ptt.rs + pipeline/dictation/{ptt,mod}.rs
//!             + services/vad/actor.rs + pipeline/router.rs
//! Binary    : dictation_ptt_test
//! Prereqs   : none. Earshot VAD needs no weights. No STT, LLM or TTS model.
//! Run       : cargo nextest run --test dictation_ptt_test --release --test-threads=1
//! ============================================================================
//!
//! The STT sink is a raw channel. This seam owns the decision "was the hold
//! validated, and which arm receives it", not the transcript. Transcription is
//! covered by `vad_segmentation_test`, turn identity by `stt_turn_identity_test`.
//!
//! WHY THE SCENARIOS SHARE ONE FUNCTION
//! ------------------------------------
//! `TempPathsGuard` and `paths::init_with_root` are process-global. Two test
//! functions in one binary would fight over the same VOX_HOME. The scenarios are
//! therefore sequential inside one function, each with its own state reset and
//! its own quiescence proof.

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use tauri::AppHandle;
use vox_lib::{
    config::AudioOutputMode,
    core::{
        events::{InteractionMode, PipelineMode, VoxEvent},
        state::{AppState, InteractionOwner, InteractionState},
    },
    pipeline::{
        assistant::ptt::{ptt_cancel, ptt_start, ptt_stop},
        dictation::{
            ptt as dictation_ptt, transition_dictation, DictationInteractionMode,
            DictationOutputMode,
        },
    },
    services::{
        llm::LlmCommand,
        stt::actor::SttCommand,
        vad::{actor::VadActorConfig, VadCommand},
    },
};

type MockApp = AppHandle<tauri::test::MockRuntime>;

const DEADLINE: Duration = Duration::from_secs(10);

struct Rig {
    app: MockApp,
    state: Arc<AppState>,
    stt_rx: mpsc::Receiver<SttCommand>,
    event_tx: mpsc::Sender<VoxEvent>,
    llm_rx: mpsc::Receiver<LlmCommand>,
    producer: common::harness::RbProducer,
    vad_cmd_tx: mpsc::Sender<VadCommand>,
    vad_shutdown: Arc<AtomicBool>,
    forward_shutdown: Arc<AtomicBool>,
    _paths: common::paths::TempPathsGuard,
    vad_join: Option<JoinHandle<()>>,
    forward_join: Option<JoinHandle<()>>,
    router_join: Option<JoinHandle<()>>,
}

impl Rig {
    async fn new() -> Self {
        let paths = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let vad_config = VadActorConfig {
            initial_threshold: vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD,
            initial_noise_gate: vox_lib::core::defaults::DEFAULT_VAD_PTT_NOISE_GATE,
            initial_silence_duration_ms: vox_lib::core::defaults::DEFAULT_VAD_SILENCE_DURATION_MS,
            initial_speech_onset_ms: vox_lib::core::defaults::DEFAULT_VAD_SPEECH_ONSET_MS,
            initial_mode: InteractionMode::PTT,
            initial_audio_mode: AudioOutputMode::Headset,
        };

        let (stt_tx, stt_rx) = mpsc::channel();
        let (llm_tx, llm_rx) = mpsc::channel();
        let audio_suppressed = Arc::new(AtomicBool::new(false));
        let vad_shutdown = Arc::new(AtomicBool::new(false));

        let (vad_cmd_tx, vad_event_rx, producer, vad_join) = common::harness::setup_vad_actor(
            stt_tx.clone(),
            vad_config,
            state.pipeline.current_state_atomic.clone(),
            state.pipeline.turn_id.clone(),
            audio_suppressed,
            state.pipeline.ingestion_gate.clone(),
            vad_shutdown.clone(),
        );
        common::harness::attach_mock_engine_with_llm_vad_to_state(
            &app,
            &state,
            stt_tx,
            vad_cmd_tx.clone(),
            Some(llm_tx),
        );

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        *state.event_tx.lock() = Some(event_tx.clone());
        let router_join =
            vox_lib::pipeline::router::spawn_router(app.clone(), event_rx).expect("router");

        // Forward VAD-originated events (window auto-stop) into the router.
        let forward_shutdown = Arc::new(AtomicBool::new(false));
        let forward_flag = forward_shutdown.clone();
        let forward_tx = event_tx.clone();
        let forward_join = std::thread::spawn(move || {
            while !forward_flag.load(Ordering::Relaxed) {
                match vad_event_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(event) => {
                        if forward_tx.send(event).is_err() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Self {
            app,
            state,
            stt_rx,
            event_tx,
            llm_rx,
            producer,
            vad_cmd_tx,
            vad_shutdown,
            forward_shutdown,
            _paths: paths,
            vad_join: Some(vad_join),
            forward_join: Some(forward_join),
            router_join: Some(router_join),
        }
    }

    /// Assistant PTT mode. Clears both tracks so the next scenario starts clean.
    fn assistant_mode(&self, pipeline_mode: PipelineMode) {
        {
            let mut settings = self.state.settings.write().unwrap();
            settings.interaction.mode = InteractionMode::PTT;
            settings.interaction.pipeline_mode = pipeline_mode;
        }
        self.state
            .owner
            .store(InteractionOwner::Assistant as u32, Ordering::Relaxed);
        self.state.pipeline.set_state(InteractionState::Ready);
        transition_dictation(InteractionState::Ready, &self.app, &self.state);
        self.state.pipeline.update_ingestion_gate();
    }

    /// Dictation PTT mode with tray output.
    fn dictation_mode(&self) {
        {
            let mut settings = self.state.settings.write().unwrap();
            settings.dictation.enabled = true;
            settings.dictation.interaction_mode = DictationInteractionMode::Ptt;
            settings.dictation.output_mode = DictationOutputMode::Tray;
            settings.interaction.mode = InteractionMode::PTT;
        }
        self.state
            .owner
            .store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
        self.state.pipeline.set_state(InteractionState::Ready);
        transition_dictation(InteractionState::Ready, &self.app, &self.state);
        self.state.pipeline.update_ingestion_gate();
    }

    fn send(&self, event: VoxEvent) {
        self.event_tx
            .send(event)
            .expect("router must accept events");
    }

    async fn wait_dictation(&self, expected: InteractionState, label: &str) {
        assert!(
            common::harness::wait_for_dictation_state(&self.state, expected, DEADLINE).await,
            "{label}: dictation must reach {:?}, saw {:?}",
            expected,
            self.state.pipeline.dictation_state()
        );
    }

    /// Returns the first Final, skipping any StreamChunks produced before it.
    fn expect_final(&self, label: &str) -> (u32, Vec<f32>, InteractionOwner) {
        let deadline = Instant::now() + DEADLINE;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.stt_rx.recv_timeout(remaining) {
                Ok(SttCommand::Final {
                    turn_id,
                    audio,
                    owner,
                }) => return (turn_id, audio, owner),
                Ok(_) => continue,
                Err(_) => panic!("{label}: no SttCommand::Final before the deadline"),
            }
        }
    }

    /// Drains the sink after a proven terminal state and panics on any Final.
    fn assert_no_final(&self, label: &str) {
        let finals: Vec<u32> = self
            .stt_rx
            .try_iter()
            .filter_map(|cmd| match cmd {
                SttCommand::Final { turn_id, .. } => Some(turn_id),
                _ => None,
            })
            .collect();
        assert!(
            finals.is_empty(),
            "{label}: no Final may be dispatched, saw turns {:?}",
            finals
        );
    }

    fn assert_llm_silent(&self, label: &str) {
        common::harness::assert_channel_empty(&self.llm_rx, label);
    }

    fn close(mut self) {
        self.forward_shutdown.store(true, Ordering::Relaxed);
        if let Some(join) = self.forward_join.take() {
            join.join().expect("VAD event forwarder panicked");
        }
        let _ = self.event_tx.send(VoxEvent::Shutdown);
        if let Some(join) = self.router_join.take() {
            join.join().expect("router thread panicked");
        }
        let _ = self.vad_cmd_tx.send(VadCommand::Shutdown);
        self.vad_shutdown.store(true, Ordering::Relaxed);
        if let Some(join) = self.vad_join.take() {
            join.join().expect("VAD actor thread panicked");
        }
    }
}

/// PURPOSE:  Validated speech must reach exactly one sink, chosen by pipeline mode;
///           silence and cancellation must reach none.
/// SCENARIO: Assistant Modular, Assistant Realtime, and Dictation PTT tracks, each driven
///           through start → speech/silence/cancel → stop.
/// ASSERTS:  Modular sends one `SttCommand::Final` with the Assistant owner and advances to
///           Thinking; Realtime advances to Thinking and sends nothing; dictation sends one
///           Final with the Dictation owner. Silence and cancel always return to Ready and
///           send no Final. Rejected starts do not consume a turn. The LLM channel stays
///           empty throughout.
/// CLASS:    happy-path (dispatch) + suppression (silence, cancel, rejected start)
/// REQUIRES: none (Earshot VAD, raw STT sink, mock engine)
/// MUTANT:   Invert the `pipeline_mode == Modular` branch in `dispatch_ptt_speech_audio`.
///
/// ABSENCE PROOF: assistant stop is synchronous; dictation is asserted after the state
/// terminal. Both are the quiescence boundary named in the intent block (F-12), and the
/// STT sink is read only after it.
#[tokio::test]
async fn validated_speech_reaches_only_the_selected_dispatch_arm() {
    let mut rig = Rig::new().await;

    // ---------------------------------------------------------------- S1
    // Assistant, Modular: speech → one Final(Assistant), Thinking.
    {
        rig.assistant_mode(PipelineMode::Modular);
        ptt_start(&rig.app, &rig.state).expect("ptt_start");
        assert_eq!(rig.state.pipeline.state(), InteractionState::Listening);

        common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut rig.producer);
        ptt_stop(&rig.app, &rig.state).await.expect("ptt_stop");

        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Thinking,
            "validated assistant speech must advance to Thinking"
        );
        let (turn_id, audio, owner) = rig.expect_final("assistant modular");
        assert_eq!(owner, InteractionOwner::Assistant);
        assert_eq!(turn_id, rig.state.pipeline.peek_turn_id());
        assert!(!audio.is_empty(), "the validated window must carry audio");
        rig.assert_no_final("assistant modular: exactly one Final");
        rig.assert_llm_silent("assistant modular: no LLM dispatch");
    }

    // ---------------------------------------------------------------- S2
    // Assistant, Modular: silence → Ready, no Final.
    {
        rig.assistant_mode(PipelineMode::Modular);
        ptt_start(&rig.app, &rig.state).expect("ptt_start");
        common::audio::stream_silence_frames(&mut rig.producer, 30);
        common::audio::wait_for_buffer_drain(&rig.producer, 5);
        ptt_stop(&rig.app, &rig.state).await.expect("ptt_stop");

        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Ready,
            "ghost gate: silence must revert to Ready"
        );
        rig.assert_no_final("assistant ghost gate");
    }

    // ---------------------------------------------------------------- S3
    // Assistant, Modular: cancel → Ready, token cancelled, no Final.
    {
        rig.assistant_mode(PipelineMode::Modular);
        ptt_start(&rig.app, &rig.state).expect("ptt_start");
        let token = rig.state.pipeline.turn_token();
        common::audio::stream_test_clip_prefix(
            common::ASSET_SUPERTONIC_01_EN_FILENAME,
            4000,
            &mut rig.producer,
        );
        ptt_cancel(&rig.app, &rig.state).expect("ptt_cancel");

        assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
        assert!(token.is_cancelled(), "cancel must cancel the turn token");
        rig.assert_no_final("assistant cancel");
    }

    // ---------------------------------------------------------------- S4
    // Assistant, Realtime: speech → Thinking, zero local STT.
    {
        rig.assistant_mode(PipelineMode::Realtime);
        ptt_start(&rig.app, &rig.state).expect("ptt_start");
        common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut rig.producer);
        ptt_stop(&rig.app, &rig.state).await.expect("ptt_stop");

        assert_eq!(
            rig.state.pipeline.state(),
            InteractionState::Thinking,
            "validated realtime speech must advance to Thinking"
        );
        rig.assert_no_final("realtime: local STT must never receive the turn");
        rig.assert_llm_silent("realtime: no LLM dispatch");
    }

    // ---------------------------------------------------------------- S5
    // Assistant, Realtime: silence → Ready, zero local STT.
    {
        rig.assistant_mode(PipelineMode::Realtime);
        ptt_start(&rig.app, &rig.state).expect("ptt_start");
        common::audio::stream_silence_frames(&mut rig.producer, 30);
        common::audio::wait_for_buffer_drain(&rig.producer, 5);
        ptt_stop(&rig.app, &rig.state).await.expect("ptt_stop");

        assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
        rig.assert_no_final("realtime ghost gate");
    }

    // ---------------------------------------------------------------- S6
    // Assistant, Realtime: cancel → Ready, token cancelled, zero local STT.
    {
        rig.assistant_mode(PipelineMode::Realtime);
        ptt_start(&rig.app, &rig.state).expect("ptt_start");
        let token = rig.state.pipeline.turn_token();
        common::audio::stream_test_clip_prefix(
            common::ASSET_SUPERTONIC_01_EN_FILENAME,
            4000,
            &mut rig.producer,
        );
        ptt_cancel(&rig.app, &rig.state).expect("ptt_cancel");

        assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
        assert!(token.is_cancelled());
        rig.assert_no_final("realtime cancel");
    }

    // ---------------------------------------------------------------- S7
    // Dictation: PttStart/PttStop through the router → Final(Dictation), Thinking.
    {
        rig.dictation_mode();
        rig.send(VoxEvent::PttStart {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Listening, "dictation start")
            .await;

        common::audio::stream_test_clip(common::ASSET_SUPERTONIC_01_EN_FILENAME, &mut rig.producer);
        rig.send(VoxEvent::PttStop {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Thinking, "dictation stop")
            .await;

        let (turn_id, audio, owner) = rig.expect_final("dictation speech");
        assert_eq!(owner, InteractionOwner::Dictation);
        assert_eq!(turn_id, rig.state.pipeline.peek_turn_id());
        assert!(!audio.is_empty());
        rig.assert_no_final("dictation speech: exactly one Final");
        rig.assert_llm_silent("dictation speech: no LLM dispatch");

        transition_dictation(InteractionState::Ready, &rig.app, &rig.state);
    }

    // ---------------------------------------------------------------- S8
    // Dictation: silence → Ready, no Final (streamed partials are allowed).
    {
        rig.dictation_mode();
        rig.send(VoxEvent::PttStart {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Listening, "dictation ghost start")
            .await;

        common::audio::stream_silence_frames(&mut rig.producer, 30);
        common::audio::wait_for_buffer_drain(&rig.producer, 5);
        rig.send(VoxEvent::PttStop {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Ready, "dictation ghost gate")
            .await;
        rig.assert_no_final("dictation ghost gate");

        transition_dictation(InteractionState::Ready, &rig.app, &rig.state);
    }

    // ---------------------------------------------------------------- S9
    // Dictation: cancel → Ready, no Final.
    {
        rig.dictation_mode();
        rig.send(VoxEvent::PttStart {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Listening, "dictation cancel start")
            .await;
        common::audio::stream_test_clip_prefix(
            common::ASSET_SUPERTONIC_01_EN_FILENAME,
            4000,
            &mut rig.producer,
        );
        rig.send(VoxEvent::PttCancel {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Ready, "dictation cancel")
            .await;
        rig.assert_no_final("dictation cancel");

        transition_dictation(InteractionState::Ready, &rig.app, &rig.state);
    }

    // ---------------------------------------------------------------- S10
    // Dictation, passive speech: SpeechStart/SpeechEnd drive the same state machine.
    {
        rig.dictation_mode();
        rig.send(VoxEvent::SpeechStart {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Listening, "passive start")
            .await;

        rig.send(VoxEvent::SpeechEnd {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Thinking, "passive end")
            .await;

        let text = "passive dictation routing check".to_string();
        rig.send(VoxEvent::TranscriptFinal {
            turn_id: rig.state.pipeline.peek_turn_id(),
            text: text.clone(),
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Ready, "passive transcript")
            .await;
        assert_eq!(
            rig.state.dictation_last_transcript.lock().as_deref(),
            Some(text.as_str()),
            "passive dictation must store the transcript"
        );
        rig.assert_no_final("passive dictation: routing must not inject into STT");
        rig.assert_llm_silent("passive dictation: no LLM dispatch");
    }

    // ---------------------------------------------------------------- S11
    // Dictation disabled: start is rejected without consuming a turn.
    {
        let before = rig.state.pipeline.peek_turn_id();
        transition_dictation(InteractionState::Idle, &rig.app, &rig.state);
        dictation_ptt::on_ptt_start(&rig.app, &rig.state);

        assert_eq!(
            rig.state.pipeline.dictation_state(),
            InteractionState::Idle,
            "a disabled dictation start must not enter Listening"
        );
        assert_eq!(
            rig.state.pipeline.peek_turn_id(),
            before,
            "a rejected start must not consume a turn"
        );
        rig.assert_no_final("dictation disabled");
    }

    // ---------------------------------------------------------------- S12
    // Dictation: duplicate start while Listening is ignored.
    {
        transition_dictation(InteractionState::Ready, &rig.app, &rig.state);
        rig.state.pipeline.update_ingestion_gate();
        dictation_ptt::on_ptt_start(&rig.app, &rig.state);
        assert_eq!(
            rig.state.pipeline.dictation_state(),
            InteractionState::Listening
        );
        let turn = rig.state.pipeline.peek_turn_id();

        dictation_ptt::on_ptt_start(&rig.app, &rig.state);
        assert_eq!(
            rig.state.pipeline.dictation_state(),
            InteractionState::Listening,
            "a duplicate start must be ignored"
        );
        assert_eq!(
            rig.state.pipeline.peek_turn_id(),
            turn,
            "a duplicate start must not consume a second turn"
        );
        dictation_ptt::on_ptt_cancel(&rig.app, &rig.state);
        assert_eq!(
            rig.state.pipeline.dictation_state(),
            InteractionState::Ready
        );
    }

    // ---------------------------------------------------------------- S13
    // Dictation: a hotkey press while the Assistant is engaged is dropped.
    {
        rig.dictation_mode();
        rig.state.pipeline.set_state(InteractionState::Listening);
        let before = rig.state.pipeline.peek_turn_id();

        dictation_ptt::on_ptt_start(&rig.app, &rig.state);

        assert_eq!(
            rig.state.pipeline.dictation_state(),
            InteractionState::Ready,
            "dictation must not start while the Assistant owns the mic"
        );
        assert_eq!(
            rig.state.pipeline.peek_turn_id(),
            before,
            "a dropped dictation start must not consume a turn"
        );
        rig.assert_no_final("dictation while assistant engaged");
        rig.state.pipeline.set_state(InteractionState::Ready);
    }

    // ---------------------------------------------------------------- S14
    // Assistant: stop without a start is ignored.
    {
        rig.assistant_mode(PipelineMode::Modular);
        ptt_stop(&rig.app, &rig.state).await.expect("ptt_stop");
        assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
        rig.assert_no_final("assistant stop without start");
    }

    // ---------------------------------------------------------------- S15
    // Assistant: cancel without a start is ignored and leaves the cancel flag alone.
    {
        rig.assistant_mode(PipelineMode::Modular);
        rig.state
            .pipeline
            .cancel_flag
            .store(false, Ordering::Relaxed);
        ptt_cancel(&rig.app, &rig.state).expect("ptt_cancel");
        assert_eq!(rig.state.pipeline.state(), InteractionState::Ready);
        assert!(
            !rig.state.pipeline.cancel_flag.load(Ordering::Relaxed),
            "a no-op cancel must not poison the next turn"
        );
        rig.assert_no_final("assistant cancel without start");
    }

    // ---------------------------------------------------------------- S16
    // Dictation: window auto-stop finalizes the turn without a PttStop event.
    {
        rig.dictation_mode();
        {
            let mut settings = rig.state.settings.write().unwrap();
            settings.dictation.silence_auto_stop_ms = 150;
        }
        rig.send(VoxEvent::PttStart {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Listening, "autostop start")
            .await;

        common::audio::stream_test_clip_prefix(
            common::ASSET_SUPERTONIC_01_EN_FILENAME,
            8000,
            &mut rig.producer,
        );
        common::audio::stream_silence_frames(&mut rig.producer, 40);
        common::audio::wait_for_buffer_drain(&rig.producer, 5);

        rig.wait_dictation(InteractionState::Thinking, "autostop stop")
            .await;
        let (_, audio, owner) = rig.expect_final("autostop");
        assert_eq!(owner, InteractionOwner::Dictation);
        assert!(!audio.is_empty());

        let mut settings = rig.state.settings.write().unwrap();
        settings.dictation.silence_auto_stop_ms =
            vox_lib::core::defaults::DEFAULT_DICTATION_SILENCE_AUTO_STOP_MS;
        transition_dictation(InteractionState::Ready, &rig.app, &rig.state);
    }

    // ---------------------------------------------------------------- S17
    // Dictation: a speech onset must clear a stale cancel flag.
    {
        rig.dictation_mode();
        rig.state
            .pipeline
            .cancel_flag
            .store(true, Ordering::Relaxed);
        assert!(
            rig.state.pipeline.cancel_flag.load(Ordering::Relaxed),
            "precondition: the cancel flag is set by the previous session teardown"
        );

        rig.send(VoxEvent::SpeechStart {
            owner: InteractionOwner::Dictation,
        });
        rig.wait_dictation(InteractionState::Listening, "cancel flag reset")
            .await;

        assert!(
            !rig.state.pipeline.cancel_flag.load(Ordering::Relaxed),
            "a dictation speech onset must clear the cancel flag, or STT drops the turn"
        );
        transition_dictation(InteractionState::Ready, &rig.app, &rig.state);
    }

    rig.close();
}
