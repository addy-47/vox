# Audio Input Reliability: AEC + Speaker Lock

**Status:** Research and planning context\
**Scope:** Candidate options and implementation constraints for the Vox
audio/VAD service. This is not an implementation plan or a final
architecture decision.

## Goal

Improve two related but distinct behaviors in the audio/VAD layer:

-   **Acoustic Echo Cancellation (AEC):** reduce Vox's own TTS playback
    leaking into microphone input, so it does not trigger speech
    detection or prevent reliable barge-in.
-   **Speaker Lock / personalized VAD (pVAD):** determine whether
    detected speech matches the enrolled user, reducing triggers from
    other people or background speech.

They belong in the same service layer because both consume audio and
contribute evidence to speech/interruption decisions. They are separate
branches of the problem and must be evaluated independently.

## Responsibility boundary

The audio/VAD service owns audio processing and evidence: microphone
audio, playback-reference audio, VAD candidates, optional
speaker-verification results, and timing/quality metadata.

The existing pipeline/router and conversation state machine remain
responsible for deciding what an event means: whether to accept speech,
start STT, interrupt playback, cancel a turn, or transition state. The
audio service should not directly mutate conversational state or
independently cancel turns.

Conceptual boundary:
`playback reference + microphone audio -> optional AEC -> existing VAD -> optional speaker verification -> speech/interruption evidence -> existing router/state machine`

This is a boundary sketch, not a required sequence for every mode.
Speaker verification may run asynchronously and return a later decision;
barge-in must not wait for full-utterance verification if that would
make interruption unacceptably slow.

## AEC: candidates to investigate

AEC needs both the microphone signal and a reference representing the
audio sent to playback. A microphone-only denoiser is not AEC.

  ----------------------------------------------------------------------------------------------------------------------------------
  Candidate                                                                          Why investigate it      Main risks / checks
  ---------------------------------------------------------------------------------- ----------------------- -----------------------
  [`sonora`](https://docs.rs/sonora/latest/sonora/)                                  Rust-facing             Verify supported
                                                                                     audio-processing        platforms, API
                                                                                     library with            maturity,
                                                                                     WebRTC-style AEC3       frame/configuration
                                                                                     functionality;          requirements, delay
                                                                                     potentially simpler     handling, audio
                                                                                     integration into a Rust quality, and CPU cost
                                                                                     application             on Vox's target
                                                                                                             hardware

  [`webrtc-audio-processing`](https://github.com/tonarino/webrtc-audio-processing)   Rust bindings around    Native C++
                                                                                     WebRTC audio            build/linking,
                                                                                     processing, including   packaging, platform
                                                                                     AEC3; useful fallback   support,
                                                                                     if a more established   dependency/version
                                                                                     implementation is       management
                                                                                     needed                  

  [`aec3`](https://docs.rs/aec3/latest/aec3/)                                        Rust implementation of  Confirm maturity, API
                                                                                     WebRTC AEC3 worth       stability, performance,
                                                                                     comparing if its        and real-device
                                                                                     current API and         behavior before
                                                                                     maintenance status fit  selecting it
  ----------------------------------------------------------------------------------------------------------------------------------

**Initial research order:** check `sonora` first for integration fit;
compare it with `webrtc-audio-processing` if the first candidate fails
required platform, quality, stability, or performance checks. Do not
decide on language or crate ergonomics alone. The quality of the
playback reference, capture/render timing, and delay alignment are
fundamental.

No neural model is required for the initial AEC candidate set. Start by
evaluating adaptive AEC3-style implementations rather than introducing a
separate neural echo-removal model.

## Speaker Lock / pVAD: candidates to investigate

Speaker Lock consumes a speech candidate and compares its speaker
representation with an enrolled profile. It is not a replacement for VAD
or AEC.

  ----------------------------------------------------------------------------------------------------------
  Candidate                                                  Why investigate it      Main risks / checks
  ---------------------------------------------------------- ----------------------- -----------------------
  **CAM++ through                                            Existing                Confirm that the exact
  [`sherpa-onnx`](https://github.com/k2-fsa/sherpa-onnx)**   speaker-embedding and   model, model format,
                                                             speaker-recognition     preprocessing,
                                                             interfaces may reduce   sample-rate
                                                             custom inference        expectations, and Rust
                                                             plumbing; aligns with   API are supported
                                                             the earlier Vox         together; measure
                                                             proposal to evaluate    quality and latency on
                                                             CAM++                   Vox hardware

  **ERes2Net through a compatible runtime**                  Alternative             Model variant, model
                                                             speaker-embedding       packaging, runtime
                                                             family for comparison   support,
                                                             if CAM++ does not meet  short-utterance
                                                             quality targets         behavior, memory use,
                                                                                     and latency must be
                                                                                     verified

  **Direct ONNX Runtime integration**                        May fit Vox's existing  Vox would own more
                                                             inference stack and     preprocessing,
                                                             avoid adding another    tensor/input handling,
                                                             native runtime if the   model compatibility,
                                                             chosen model is         and output
                                                             compatible              interpretation; inspect
                                                                                     the current code before
                                                                                     choosing this route
  ----------------------------------------------------------------------------------------------------------

The older Speaker Lock note proposes CAM++, `simsimd` for cosine
similarity, and a dedicated verification worker. Treat these as
candidates, not settled decisions. In particular, do not carry forward a
proposed similarity threshold or assumed inference latency without
measuring them.

Enrollment and verification must use compatible preprocessing and the
same model/version. Test whether a profile made from multiple enrollment
samples is more robust than a single sample. A cosine score is not
itself a calibrated probability; acceptance policy must be chosen from
measured false-accept and false-reject behavior.

## Keep work out of the actual hot path

The capture callback and high-priority VAD actor must remain responsive.
Do not perform model inference, disk I/O, enrollment persistence,
blocking waits, or potentially unbounded work inside the capture
callback or the VAD frame-processing loop.

-   Keep capture callbacks limited to the existing real-time-safe work
    and bounded handoff.
-   Keep AEC processing in a dedicated, bounded audio-processing path
    with explicit capture/render inputs and correct ordering/timing.
    Confirm whether the selected implementation can safely run on the
    intended audio thread; do not assume that because an API is
    synchronous it is callback-safe.
-   Run speaker-embedding inference on a dedicated worker or an existing
    suitable inference executor. Pass owned/bounded audio segments or
    references with a clear lifetime contract; do not block the VAD
    actor waiting for the result.
-   Use bounded channels/queues and define what happens under overload:
    defer, reject, fall back, or drop stale work deliberately. Avoid
    silent unbounded buffering.
-   Make worker shutdown, cancellation, stale-result rejection, and
    model-load failure explicit.
-   Keep profile/model loading and enrollment outside the streaming hot
    path.
-   Preserve the existing router/state machine as the authority for
    playback cancellation, turn cancellation, and state transitions.

For speaker verification, do not run an embedding model on every tiny
VAD frame. Use candidate windows with enough informative speech, while
measuring the latency this adds. For barge-in, explore a rolling/early
verification strategy only if AEC plus VAD is insufficient; do not let a
full-utterance gate silently make interruptions sluggish.

## Gates and fallback behavior

Introduce separate, independently controllable feature gates for AEC and
Speaker Lock. The exact settings/configuration location should be
decided after inspecting the current settings and service lifecycle.

-   **AEC off:** preserve current audio behavior.
-   **AEC on:** use AEC only when the selected implementation is
    initialized and receiving valid playback-reference and microphone
    streams; define a safe fallback for failure or unsupported devices.
-   **Speaker Lock off:** preserve current speech-routing behavior.
-   **Speaker Lock on:** define accept, reject, and uncertain/error
    behavior. Avoid treating inference failure as a confident speaker
    rejection without an explicit product decision.
-   **Barge-in:** retain the existing state-machine cancellation path.
    The audio layer supplies evidence; it does not bypass the router.
-   **Rollout:** benchmark each feature independently first, then test
    their combined behavior. Do not require both features to be enabled
    together.

Defaults, thresholds, queue capacities, frame sizes, and timeouts should
not be selected by intuition. Use constraints imposed by the chosen
library where required, then tune remaining values from measurements and
document why each value exists.

## Benchmark plan: no magic numbers

Before selecting a candidate, define test sets and acceptance criteria
based on Vox's actual workload. Record the device, OS, model/version,
audio configuration, and test conditions so results are reproducible.

### AEC measurements

Test at least: TTS-only playback with no user speech; user speech
overlapping TTS; speech beginning before, during, and after playback
transitions; different playback levels and microphone distances;
background speech/noise; and supported speaker/headset configurations.

Measure: - Residual echo and how often TTS alone triggers VAD. - Missed
or delayed detection of genuine user speech during TTS. - Time from user
speech onset to accepted barge-in and playback cancellation. - Audio
artifacts or speech damage introduced by AEC. - CPU cost, processing
time, queue depth, dropped/late frames, and stability during sustained
playback. - Behavior when playback reference is missing, delayed,
discontinuous, or changes format.

Compare against the current behavior using the same recordings and
conditions. Do not infer quality from a single demonstration.

### Speaker-verification measurements

Use separate enrolled-user and non-enrolled-speaker samples, including
background conversation/TV speech, different distances, loudness,
microphones, rooms, and realistic short utterances.

Measure: - False Accept Rate (FAR) and False Reject Rate (FRR),
including the trade-off across thresholds. - Detection/decision latency
for full-utterance and any proposed early/rolling strategy. -
Performance by usable speech duration and acoustic condition. - CPU,
memory, worker queue delay, overload behavior, and failure handling. -
Enrollment repeatability and profile behavior after re-enrollment or
model changes.

Choose the operating point from Vox's product trade-off: missing the
actual user and allowing background speech have different costs. Do not
copy a generic threshold from a model card or the old design note.

### Combined-system measurements

Test AEC and Speaker Lock separately, then together. Track false
interruptions, missed interruptions, speech acceptance/rejection,
end-to-end decision latency, and CPU/memory impact. Verify that enabling
either feature does not regress headset mode, push-to-talk,
tray/always-on mode, or existing interruption lifecycle behavior.

## Implementation-planning guardrails

Ask the agent to inspect the current audio capture/playback paths, VAD
actor, settings/gates, model runtime, worker/channel patterns, and
interruption routing before recommending concrete file changes. Confirm
where the actual playback PCM is available and whether it can be
provided to AEC with reliable timing.

The planning result should compare candidates against Vox's actual
platforms and dependencies, identify unknowns, propose a reproducible
benchmark, and recommend an option with evidence. It should not assume
the old Speaker Lock proposal is authoritative or begin by implementing
a model export, a threshold, or a new worker before confirming the
existing code boundaries.

## Research links

-   Sonora: https://docs.rs/sonora/latest/sonora/
-   WebRTC audio-processing Rust bindings:
    https://github.com/tonarino/webrtc-audio-processing
-   Rust AEC3 project: https://docs.rs/aec3/latest/aec3/
-   sherpa-onnx: https://github.com/k2-fsa/sherpa-onnx
-   Alibaba 3D-Speaker (speaker embedding models):
    https://github.com/modelscope/3D-Speaker
