# ZipVoice TTS Param Tweaking — Full Task Learnings (2026-09-29)

Sprint-driven investigation into ZipVoice output clarity: 8 voice sprints (orig vs 3s
prompts × int8 vs fp32 weights), param sweeps, weight verification, Chatterbox
prompt extension. All audio verdicts are the user's; metrics only here.

## 1. Root cause: the cloner reproduces prompt timbre

- Prompt-swap proof: bright Kokoro wav as atlas prompt → output HF share 0.138 vs
  0.043–0.095 on VCTK-derived packs, same model/weights/run. The engine faithfully
  reproduces the reference's spectral profile — dull prompt in, dull output.
- Chain drag measured: every prompt exits duller than it enters (~25–30% HF loss:
  0.197→0.138, 0.105→0.077). Prompt sets the ceiling; the chain lowers it.
- Pipeline exonerated: 24kHz→playback 2× upsample→ring→bench decimation is
  rate-correct and shared with clean Kokoro output.

## 2. Weights: fully verified, not the issue

- `vocos_24khz.onnx`, `encoder.int8.onnx`, `decoder.int8.onnx`, `tokens.txt`,
  `lexicon.txt`, `espeak-ng-data/` all byte-match fresh official release downloads
  (sha256). Our call also matches upstream `zipvoice_tts.rs` verbatim
  (feat_scale 0.1, t_shift 0.5, target_rms 0.1, guidance 1.0, steps 4, min_char 10).

## 3. Param sweeps: second-order vs prompt choice

- Steps 1/2/3/4/6/8 on atlas: HF flat 0.046–0.067. Steps buy speed, not clarity.
  12-run bench matrix (atlas + bright audition voice × steps/guidance/feat/threads):
  prompt band dominates (aud 0.13–0.23 vs atlas 0.04–0.10 across ALL settings).
- Threads: 2 baseline; 8 slower than 2 (oversubscription); 4 marginal. Keep 2–4.
- Outputs are stochastic run-to-run (no fixed seed) — single-sample deltas are noise.
- Bench flags added (bench-only surface): `--zv-steps/--zv-guidance/--zv-feat-scale/`
  `--zv-t-shift/--zv-target-rms/--zv-min-char/--zv-threads/--slug`, backed by
  `ZipvoiceTuning` + `set_tuning` in `zipvoice.rs` (defaults = old behavior; also
  fixed dead `set_guidance_scale`, which stored an atomic nobody read).
- EQ experiment: +6 dB high-shelf @4kHz lifted prompt HF 0.060→0.105, output flat
  (0.077 vs 0.089 control). EQ can't invent missing harmonics — needs real HF content.

## 4. Prompt rules (docs + measurement)

- Official demo prompts are 6–9s @24kHz; README prescribes <3s single-speaker.
  Measured: 10s/6s tiers render slower; atlas-3s anomaly rendered 26.6s for text
  that takes 16–17s on longer prompts. Final: ~3s prompts (3.0–4.8s accepted,
  completeness wins over exact duration).
- Docs warning (confirmed): reference-text must EXACTLY match reference-audio or
  quality degrades (vera pack violated this: clip speaks leading "transcript").
- Cut ends must sit in ≥250ms silence runs — 50ms minima land inside stop
  consonants and clip words. Starts snap backward only. Every `reference.txt`
  transcribed from its own segment with Qwen (STT-verified, user ear-verified).

## 5. fp32 vs int8 (455MB vs 119MB decoder)

- Durations identical across weights in all 8 sprints (quantization doesn't move
  prosody timing). RTF ~60–80% higher on fp32. Brightness edge inconsistent:
  +0.01 nova, none vera/alfred/maya/claire/iris/sage (within noise).
- Both weight dirs live side by side: `~/.vox/models/tts/zipvoice/` (int8, live)
  and `~/.vox/models/tts/zipvoice-fp32/` (fp32, verified runnable via swap test).
  NOTE: fp32 files keep loader-compatible names (`decoder.int8.onnx` contains
  fp32 data). Manual switch = rename dirs. int8 md5: `ea26cf77…`; fp32 decoder
  md5: `079dccc2…`.

## 6. Chatterbox extension (for short originals)

- Upstream operates on ~10s refs (`your_10s_ref_clip.wav`); our 30s auto-stitch
  worked. Chatterbox needs NO transcript (speaker-encoder design) vs ZipVoice's
  mandatory exact transcript — architecturally different prompting.
- Pacing: cfg 0.5→0.3 slows output (16.96s→18.68s same text); 0.2 non-monotonic;
  exagg 0.7 unstable (40.16s anomaly, noises). Prod default set to
  `DEFAULT_CHATTERBOX_CFG_WEIGHT = 0.3`. `--cfg-weight/--exaggeration/--seed`
  flags added to `voice_clone` example.
- Hard stall: iris 50-word text stalls deterministically at exactly 40.16s
  (cfg/seed-independent — generation token cap). Shorter text completed (12.3s).
- Repeat-to-length does NOT transfer to ZipVoice (exact-match rule), unless both
  audio and transcript repeat identically — untested, risky.

## 7. Sprint scoreboard (numbers sentence, threads 4; user verdicts)

| voice | verdict | notes |
|---|---|---|
| atlas | PRUNED | 3s anomaly (26.6s render); decommissioned |
| nova | fp32 + orig | fp32 +0.01 HF over int8 |
| vera | fp32 + 3s | orig renders 25.4s (slow speaker); int8 also good |
| alfred | fp32 + 3s | darkest sprint (HF 0.044–0.067); int8 also good |
| maya | fp32 + 3s | orig≈3s durations; int8 also good |
| claire | fp32 + 3s | no separation on any axis (14.4s all); int8 also good |
| iris | fp32 + orig | orig clearly brighter (0.10 vs 0.06); int8 also good |
| sage | fp32 + 3s | orig brighter band but 3s wins on listen; int8 runs kept |

## 8. Final pack state

- `~/.vox/models/tts/zipvoice/voices/`: 7 voices (atlas pruned). vera/alfred/
  maya/claire/sage carry new ~3–5s prompts with Qwen-verified transcripts;
  nova/iris keep original clips. Manifest rebuilt (7 entries).
- Sources: `sandbox/zipvoice_prompts/` (24 prompts + `manifest.json`),
  `sandbox/cb_extended/` (extension clones), full 6s/10s tiers retained in
  sandbox only (dropped from pack per README <3s guidance).
- Standing rule: never judge audio quality in reports — metrics + paths only.
