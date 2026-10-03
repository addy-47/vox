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

---

# Round 2 — Prompt-Factory Sweep + Production Config (2026-10-03)

Second pass on the same question, prompted by the clip actually sounding worse than
`temp/voices/*`. Supersedes §3 (params are second-order) and §4 (the ≤3s rule).

## 1. What was rebuilt to ask the question

- `tts_bench` gained bench-only flags so no run has to touch the live pack:
  `--zv-model-dir` (int8/fp32 A/B), `--zv-pack-dir` (candidate packs in sandbox),
  `--text-file` (one synthesis text per line), `--label` (wav subdir + name prefix).
- Analysis venv `temp/zv-audio-venv` (uv) + `sandbox/scripts/zv_analyze.py`:
  DNSMOS P.835 (SIG/BAK/OVRL) + P.808, ECAPA speaker cosine, band energies, tilt.
- Prompt factory: 12 distinct sentences rendered in the target voice via
  `voice_clone --en-prompt`, then cut to a 1.0/1.5/2.0/2.5/3.0/3.5/4.5s ladder
  (`zv_cut_ladder.py`). Transcripts from **Nemotron** (`stt_bench --model nemotron`),
  never hand-patched. 101 prompts, 539 syntheses.

## 2. Loudness was never the problem

Prompts spanned 8.3 dBFS; their outputs spanned only 2.9 dBFS. `target_rms` already
normalises level — a prompt 8 dB down produced output ~2 dB down. The dull-sounding
outputs were already at baseline loudness (−19.5 vs −19.1 dBFS). Making prompts louder
is a dead end.

## 3. Nor was it missing bandwidth

Energy above 12 kHz is **0.21%** of the baseline. 44.1 kHz-vs-24 kHz is a red herring.
The gap is spectral *tilt*, recoverable.

## 4. Prompt source dominates everything else

Prompt-side speaker similarity: real recording **0.840** > Chatterbox extension 0.695 >
Chatterbox-generated **0.576**. The first Chatterbox-generated prompt measured
`hf_ratio 0.0037` vs baseline `0.0240` — ~6.5x duller. Chatterbox is a poor prompt
source; it is useful only for making *more* material when the original is too short.

## 5. The ≤3s rule is wrong

Prompt-side speaker similarity by duration is monotone: 1.0s 0.499 → 1.5s 0.591 →
2.0s 0.640 → 2.5s 0.675 → 3.0s 0.703 → **3.5s 0.732** → 4.5s 0.774. Output-side tracks
it (1.0s 0.454 → 3.5s 0.552). The 1s prompts measured worst on every metric. 3.5s is the
sweet spot; completeness still beats hitting an exact duration.

## 6. Params: guidance 3.0 and t_shift 0.7 are the wins

Mean output speaker similarity, marginals over a 14-config grid (438 syntheses,
medians over repeats, threads=4, int8):

- `guidance`: 1.0→0.547, 1.5→0.556, 2.0→0.595, 2.5→0.594, **3.0→0.624**, 4.0→0.609
- `t_shift`: **0.7→0.599** vs 0.5→0.553
- `feat_scale`: **0.1→0.583** vs 0.15→0.496 (0.15 actively harmful; one prompt → 0.266)
- `steps`: 2→0.548, 4→0.576, 6→0.580, 8→0.557 — flat on quality, but RTF
  0.44 / 0.86 / 1.37 / **1.78**. `steps=4` wins on the combined criterion.

Guidance and t_shift sit off the RTF critical path, so both gains are ~free.
**Shipped: `guidance 1.0→3.0`, `t_shift 0.5→0.7`** in `zipvoice.rs` (engine constants,
deliberately *not* user settings).

## 7. fp32 rejected on RTF

`zipvoice-fp32` measured RTF 0.83–**2.72** against int8's 0.44–1.21. Gate is ≤1.5 target
/ ≤2.0 reject. int8 stays. No fp16 attempt — there was no quality case to preserve.

## 8. Output-side DSP (the lever never previously tried)

Every prior attempt EQ'd the *prompt*, which cannot work. Per-clip **output** correction
— a presence bell plus an independent air shelf — was swept in
`zv_dsp_sweep.py` / `zv_dsp_match.py`.

- Best fixed correction took one clip from spectral rmsDev 2.186 → 0.964 with
  **no** DNSMOS or speaker-similarity cost.
- **But gains solved on one clip wreck another** (text01 1.638 → 3.291). So the
  correction must be solved per clip against its own measured deviation; a shipped
  implementation needs a runtime solver or one safe global setting.
- Transient sharpening and a harmonic exciter both **failed** — they add high-frequency
  energy the baseline does not have. Dropped.

## 9. Honest bottom line on the shipped change

Fair A/B, same methodology (3 repeats × 3 disjoint texts, threads=4, int8):

| | OVRL | SIG | speaker | dBFS | RTF |
|---|---|---|---|---|---|
| old prompt + old params | 3.248 | 3.503 | 0.645 | −22.4 | 0.77 |
| **new prompt + new params** | 3.206 | 3.489 | **0.661** | **−19.5** | **0.64** |
| baseline | 3.483 | 3.688 | 1.000 | −19.1 | — |

**This is a wash, not a win.** Speaker similarity +0.016, loudness match +2.9 dB better,
~17% faster — but DNSMOS OVRL −0.042 and speech ~19% slower. The earlier "meets baseline"
figure (OVRL 3.487) came from the *DSP'd* clip on a single favourable sample; without DSP
the production path sits at OVRL 3.206, spk 0.661. Neither is baseline parity.

## 10. Reusable

Bench flags, the analysis venv, the prompt factory, the cut ladder, and the per-clip
spectral matcher all live in `sandbox/scripts/` + `temp/zv-audio-venv` and are voice-
agnostic. The **params** should transfer to other voices; each voice still needs its own
prompt cut. Winner clips: `sandbox/results/zv_winners/sage/FINAL/`.
