//! ============================================================================
//! stt_turn_identity_test.rs — STT turn identity and stream reset seam
//! ============================================================================
//! Category  : Component (Seam 3 — SttCommand stream → STT worker → VoxEvent)
//! Component : services/stt/actor.rs
//! Binary    : stt_turn_identity_test
//! Prereqs   : none (recording provider double; no model weights)
//! Run       : cargo nextest run --test stt_turn_identity_test --release --test-threads=1
//! ============================================================================
//!
//! The worker loop, the channel protocol, the cancellation flag and the turn
//! state machine are real. Only the speech model is replaced by a recording
//! double, because turn identity must not depend on decoder output.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use vox_lib::{
    core::{events::VoxEvent, state::InteractionOwner},
    services::stt::{
        spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand, SttProvider,
        SttProviderKind,
    },
};

/// End of a wait bound, not a latency assertion. The work itself is synchronous.
const DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Recording {
    chunks: Mutex<Vec<(usize, bool)>>,
    resets: AtomicUsize,
    warmups: AtomicUsize,
}

impl Recording {
    fn chunk_calls(&self) -> Vec<(usize, bool)> {
        self.chunks.lock().expect("recording lock").clone()
    }

    fn final_calls(&self) -> usize {
        self.chunk_calls()
            .iter()
            .filter(|(_, is_final)| *is_final)
            .count()
    }

    fn reset_count(&self) -> usize {
        self.resets.load(Ordering::SeqCst)
    }

    fn warmup_count(&self) -> usize {
        self.warmups.load(Ordering::SeqCst)
    }
}

/// Delegating newtype so the worker can own the provider while the test keeps
/// read access to the recording.
struct Provider(Arc<Recording>);

impl SttProvider for Provider {
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String> {
        self.0
            .chunks
            .lock()
            .expect("recording lock")
            .push((chunk.len(), is_final));
        Ok(format!("len{}", chunk.len()))
    }

    fn reset_state(&self) -> anyhow::Result<()> {
        self.0.resets.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn health_check(&self) -> bool {
        true
    }

    fn kind(&self) -> SttProviderKind {
        SttProviderKind::Embedded
    }

    fn warmup(&self) -> anyhow::Result<()> {
        self.0.warmups.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct Harness {
    tx: mpsc::Sender<SttCommand>,
    events: mpsc::Receiver<VoxEvent>,
    cancel: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    recording: Arc<Recording>,
    join: Option<JoinHandle<()>>,
}

impl Harness {
    fn new() -> Self {
        let recording = Arc::new(Recording::default());
        let (tx, rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));

        let channels = SttActorChannels {
            rx,
            pipeline_event_tx: Some(event_tx),
            partial_emitter: None,
        };
        let handles = SttActorHandles {
            cancel_flag: cancel.clone(),
            engine_shutdown: shutdown.clone(),
        };
        let join = spawn_stt_worker(channels, Box::new(Provider(recording.clone())), handles)
            .expect("STT worker must spawn");

        Self {
            tx,
            events,
            cancel,
            shutdown,
            recording,
            join: Some(join),
        }
    }

    fn send(&self, cmd: SttCommand) {
        self.tx.send(cmd).expect("worker must accept commands");
    }

    fn next_event(&self) -> VoxEvent {
        self.events
            .recv_timeout(DEADLINE)
            .expect("worker must emit one event before the deadline")
    }

    /// Sends Shutdown and joins. Used as the barrier of last resort.
    fn close(mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.tx.send(SttCommand::Shutdown);
        if let Some(join) = self.join.take() {
            join.join().expect("STT worker must not panic");
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.tx.send(SttCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn chunk(turn_id: u32, samples: usize) -> SttCommand {
    SttCommand::StreamChunk {
        turn_id,
        audio: vec![0.05; samples],
    }
}

fn final_cmd(turn_id: u32, samples: usize, owner: InteractionOwner) -> SttCommand {
    SttCommand::Final {
        turn_id,
        audio: vec![0.05; samples],
        owner,
    }
}

/// PURPOSE:  A final transcript must carry the turn id it was produced for.
/// SCENARIO: One Final for turn 4 with the Assistant owner.
/// ASSERTS:  TranscriptFinal carries turn 4, the provider text, and the Assistant owner;
///           the provider was finalised once and reset once.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Emit `state.current_active_turn` instead of the command's `tid`.
#[test]
fn final_transcript_carries_its_turn_and_owner() {
    let h = Harness::new();
    h.send(final_cmd(4, 100, InteractionOwner::Assistant));

    match h.next_event() {
        VoxEvent::TranscriptFinal {
            turn_id,
            text,
            owner,
        } => {
            assert_eq!(turn_id, 4);
            assert_eq!(text, "len100");
            assert_eq!(owner, InteractionOwner::Assistant);
        }
        other => panic!("expected TranscriptFinal, got {:?}", other),
    }
    assert_eq!(h.recording.final_calls(), 1);
    assert_eq!(h.recording.reset_count(), 1);
    h.close();
}

/// PURPOSE:  Dictation results must not be delivered as Assistant results.
/// ASSERTS:  The event preserves InteractionOwner::Dictation.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Hard-code InteractionOwner::Assistant in `emit_final_events`.
#[test]
fn final_transcript_keeps_the_dictation_owner() {
    let h = Harness::new();
    h.send(final_cmd(2, 64, InteractionOwner::Dictation));

    match h.next_event() {
        VoxEvent::TranscriptFinal { owner, .. } => {
            assert_eq!(owner, InteractionOwner::Dictation)
        }
        other => panic!("expected TranscriptFinal, got {:?}", other),
    }
    h.close();
}

/// PURPOSE:  The router increments the turn id between pre-roll and body. The STT
///           worker must adopt it without wiping the open stream.
/// SCENARIO: Chunk turn 2, then chunk turn 3, then Final turn 3.
/// ASSERTS:  The provider is reset for turn 2 and for the final, never for the +1 step.
/// CLASS:    regression (RCA: pre-roll tagged N-1, body tagged N)
/// REQUIRES: none
/// MUTANT:   Remove the `wrapping_add(1)` adoption branch.
#[test]
fn open_stream_adopts_incremented_turn_without_reset() {
    let h = Harness::new();
    h.send(chunk(2, 16));
    h.send(chunk(3, 16));
    h.send(final_cmd(3, 16, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 3, .. }
    ));
    assert_eq!(
        h.recording.reset_count(),
        2,
        "reset once for turn 2 and once after the final; the +1 adoption must not reset"
    );
    h.close();
}

/// PURPOSE:  A real turn jump must clear the previous turn's decoder state.
/// SCENARIO: Chunk turn 2, then chunk turn 9.
/// ASSERTS:  The provider is reset for the jump, not only for the first turn.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Treat every turn change as an adoption.
#[test]
fn turn_jump_resets_provider_state() {
    let h = Harness::new();
    h.send(chunk(2, 16));
    h.send(chunk(9, 16));
    h.send(final_cmd(9, 16, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 9, .. }
    ));
    assert_eq!(
        h.recording.reset_count(),
        3,
        "turn 2, turn 9, and post-final resets"
    );
    h.close();
}

/// PURPOSE:  Repeating a chunk of the same turn must not restart the decoder.
/// ASSERTS:  Two chunks of one turn reset once, not twice.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Reset on every StreamChunk.
#[test]
fn repeated_chunk_for_same_turn_keeps_stream_state() {
    let h = Harness::new();
    h.send(chunk(5, 16));
    h.send(chunk(5, 16));
    h.send(final_cmd(5, 16, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 5, .. }
    ));
    assert_eq!(h.recording.reset_count(), 2, "open and post-final only");
    h.close();
}

/// PURPOSE:  The turn counter wraps. A wrap must look like +1, not like a jump.
/// ASSERTS:  MAX → 0 is adopted without a reset.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Compare with `current + 1` instead of `wrapping_add(1)`.
#[test]
fn wrapped_turn_id_is_adopted_without_reset() {
    let h = Harness::new();
    h.send(chunk(u32::MAX, 16));
    h.send(chunk(0, 16));
    h.send(final_cmd(0, 16, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 0, .. }
    ));
    assert_eq!(
        h.recording.reset_count(),
        2,
        "one reset for MAX and one post-final; the wrap must not add one"
    );
    h.close();
}

/// PURPOSE:  A cancelled turn must be reported as cancelled, not transcribed.
/// SCENARIO: Transcribe turn 6, raise the cancel flag, then send Final turn 6 again.
/// ASSERTS:  Cancelled{turn 6} is emitted, no second final transcription runs, and
///           the provider state is reset.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `cancelled && tid == current_active_turn` branch.
#[test]
fn cancelled_final_for_current_turn_emits_cancelled() {
    let h = Harness::new();
    h.send(final_cmd(6, 16, InteractionOwner::Assistant));
    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 6, .. }
    ));

    h.cancel.store(true, Ordering::SeqCst);
    h.send(final_cmd(6, 16, InteractionOwner::Assistant));

    match h.next_event() {
        VoxEvent::Cancelled { turn_id, owner } => {
            assert_eq!(turn_id, 6);
            assert_eq!(owner, InteractionOwner::Assistant);
        }
        other => panic!("expected Cancelled, got {:?}", other),
    }
    assert_eq!(
        h.recording.final_calls(),
        1,
        "only the open turn may be transcribed; the cancelled repeat must not be"
    );
    assert_eq!(
        h.recording.reset_count(),
        2,
        "open and cancelled-final resets"
    );
    h.close();
}

/// PURPOSE:  A stale cancel must not drop the next turn's final.
/// SCENARIO: Cancel is raised, then Final turn 3 arrives while the current turn is 0.
/// ASSERTS:  TranscriptFinal turn 3 is emitted.
/// CLASS:    regression (F-5 stale-cancel ordering)
/// REQUIRES: none
/// MUTANT:   Reject every final while the cancel flag is set.
#[test]
fn cancelled_final_for_a_later_turn_is_not_rejected() {
    let h = Harness::new();
    h.cancel.store(true, Ordering::SeqCst);
    h.send(final_cmd(3, 32, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 3, .. }
    ));
    assert_eq!(h.recording.final_calls(), 1);
    h.close();
}

/// PURPOSE:  ResetStream must clear decoder state before the next turn.
/// SCENARIO: Chunk turn 1, ResetStream, Final turn 1.
/// ASSERTS:  The provider is reset on the explicit reset and the final still runs.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make ResetStream a no-op.
#[test]
fn reset_stream_clears_provider_before_the_next_final() {
    let h = Harness::new();
    h.send(chunk(1, 16));
    h.send(SttCommand::ResetStream);
    h.send(final_cmd(1, 16, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 1, .. }
    ));
    assert_eq!(
        h.recording.reset_count(),
        3,
        "open, explicit reset, post-final"
    );
    h.close();
}

/// PURPOSE:  Warmup must reach the provider before the first real turn.
/// ASSERTS:  The provider records exactly one warmup call.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the Warmup arm from the dispatch.
#[test]
fn warmup_reaches_the_provider() {
    let h = Harness::new();
    h.send(SttCommand::Warmup);
    h.send(final_cmd(1, 8, InteractionOwner::Assistant));

    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 1, .. }
    ));
    assert_eq!(h.recording.warmup_count(), 1);
    h.close();
}

/// PURPOSE:  Every partial audio buffer is borrowed from a pool. A leak starves
///           the capture path.
/// SCENARIO: One Partial command, then a Final that acts as the barrier.
/// ASSERTS:  The recycle channel receives the buffer back exactly once.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Skip the recycle send after `handle_partial_command`.
///
/// NOTE: multi-partial coalescing is covered deterministically by the unit test
/// `coalesce_partials_keeps_latest_audio_and_recycles_earlier_buffers`, which is
/// ignored pending F-15. Driving coalescing from here would be timing-dependent.
#[test]
fn partial_command_recycles_its_buffer() {
    let h = Harness::new();
    let (recycle_tx, recycle_rx) = mpsc::sync_channel(1);
    h.send(SttCommand::Partial {
        turn_id: 1,
        audio: vec![0.1; 4],
        recycle_tx,
    });
    h.send(final_cmd(1, 16, InteractionOwner::Assistant));
    assert!(matches!(
        h.next_event(),
        VoxEvent::TranscriptFinal { turn_id: 1, .. }
    ));

    let returned = recycle_rx
        .recv_timeout(DEADLINE)
        .expect("partial buffer must return to the pool");
    assert_eq!(returned.len(), 4);
    h.close();
}

/// PURPOSE:  Shutdown must end the worker thread, not leave it parked.
/// ASSERTS:  The join completes before the deadline.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove the `Shutdown` arm from the dispatch.
#[test]
fn shutdown_terminates_the_worker_within_deadline() {
    let mut h = Harness::new();
    h.shutdown.store(true, Ordering::SeqCst);
    h.send(SttCommand::Shutdown);
    let join = h.join.take().expect("worker handle");

    let (done_tx, done_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("stt-shutdown-joiner".into())
        .spawn(move || {
            join.join().expect("worker must not panic");
            let _ = done_tx.send(());
        })
        .expect("spawn stt-shutdown-joiner");
    done_rx
        .recv_timeout(DEADLINE)
        .expect("worker must exit after Shutdown");
}
