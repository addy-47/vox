# Kokoro Stage-2 Fine-Tuning Plan — Vox Conversational TTS

> **Status:** **HELD as fallback** · **Date:** 2026-09-27 · **Author:** ML research engineer
> **Supersedes:** `docs/plans/wip/kokoro-finetuning.md` (moved here and refactored; the old
> PRD is superseded on every point where the two disagree — see §1)
> **Primary path:** `zipvoice-integration-plan.md` · **Governed by:** `/opt/vox/agents.md`
> **Experiment record:** `/opt/vox/goal.md`

---

## 0. TL;DR for the implementer

This path exists to answer one question: **how much conversational naturalness can be bought
by training alone, with the model architecture unchanged?**

**Run it only if the ZipVoice path fails the blind gate.** It preserves the exact 3-file
deployment swap and requires zero Rust changes, which is its entire appeal. It is 2–4 weeks
with a real chance of producing something worse than the baseline.

**Do not skip to the 95M/110M experiments.** The original plan treated them as the natural
continuation of the ablation matrix. On the real target hardware they are not affordable —
see §3.

---

## 1. What changed from the original PRD, and why

The superseded `wip/kokoro-finetuning.md` was written before this work and contains several
claims that are now **verified wrong or materially misleading**. Corrections:

| Original PRD claim | Verified position | Evidence |
|---|---|---|
| "Vox currently uses **Kokoro 0.19**… the major v0.19 → v1.0 change was primarily the training/data/voice/language release" | ✅ **Correct.** v0.19 and v1.0 share one architecture; only training data and voice/language coverage differ | ONNX `metadata_props`: `model_url=hexgrad/kLegacy/`, `language=English`, `n_speakers=11`, the exact v0.19 voice roster |
| Target is "**~90–110M params**" | The incumbent is **86,141,119**. Verified by ONNX initializer count, not the "82M" figure in the PRD | ONNX graph inspection |
| "The 16 GB constraint should influence experiment design" | ⚠️ **Inverted.** The 16 GB RTX 5070 Ti is a **training** box. The **target** is a 4-core / 8 GB laptop at **0.65 RTF** (user-measured) | headroom is **~1.5×**, not 4× |
| "Baseline RTF ~0.7, so increasing model size is only justified if…" | True on target hardware, and it **rules out** most of §8's E3/E4 | §3 below |
| "Do not spend time investigating whether moving to later upstream Kokoro releases represents an architectural upgrade" | ✅ **Correct and important** — but note v1.1-zh is *also* a **downgrade for English**: it drops 51 of v1.0's 54 voices and adds only ~3 h of English data; its own card says *"not a strict upgrade"* | Kokoro-82M-v1.1-zh README |
| "`hexgrad/kLegacy` contains the original Kokoro training code" (implied by its citation as the source of the two-stage structure) | 🔴 **FALSE.** `kLegacy` is **inference-only** — 28 files, 3 commits, last touched Dec 2024, no training scripts, no losses, no `Utils/`. Author: *"Kokoro is released as-is, and there is no intention to release a method of fine-tuning."* | `hexgrad/kLegacy` tree; hexgrad/kokoro#207 |
| Implicit: "Kokoro's ceiling is a **data** problem, so more data will fix it" | ⚠️ **Half right, and this is the crux.** It is *primarily* a data-distribution problem (hexgrad: *"mostly long-form reading and narration, not conversation"*), **and** partly architectural: hexgrad shipped *"Decoder only: no diffusion, no encoder release"*, so there is no per-utterance style/emotion input at inference. **Both matter, and neither alone is sufficient.** | model card + ONNX signature |

**A correction to a prior agent's note** (`/tmp/opencode/sec45.md`, not in this repo): it
"corrected" v0.19 → v1.x on the grounds that the model *"synthesizes Chinese"* and the model
directory contains a jieba `dict/`. **That reasoning is invalid** — the Chinese `dict/` and
the full `espeak-ng-data` ship in *every* sherpa-onnx Kokoro tarball regardless of the
weights. The 11-voice English roster in `metadata_props` is decisive. Do not re-open this.

**A correction to a prior *conversation* in this project:** an earlier agent stated that
Kokoro quality **cannot** be improved by fine-tuning. **That is wrong and must not propagate.**
What is true is narrower — Kokoro has no *per-utterance style input at inference*. The
Stage-2 duration/F0/energy predictor and the decoder are **fully trainable**, and the base
English prosody is good, which is precisely why *undertrained* Stage-2 runs report "flat,
slow, evenly paced, robotic" prosody. **Fine-tuning is a legitimate fix. Execute it properly.**

---

## 2. What is actually being trained

Kokoro ships **5 modules** in the `.pth`: `bert`, `bert_encoder`, `predictor`,
`text_encoder`, `decoder`.

**Not shipped:** `style_encoder`, `predictor_encoder`, `diffusion`, `mpd`, `msd`, `wd`.

| Stage | Trains | Requires |
|---|---|---|
| **Stage 1** (`train_first.py`) | `decoder` (iSTFTNet), `style_encoder`, `text_aligner`, `pitch_extractor` | multi-speaker corpus (**≥1000 speakers** to generalise, per yl4579) |
| **Stage 2** (`train_second.py`) | `predictor` (duration + F0 + energy), refines `decoder` | a Stage-1 checkpoint — `train_second.py` raises `ValueError("You need to specify the path to the first stage model.")` |

> **Stage 1 = many voices. Stage 2 = one voice. This is architectural, not a preference.**

**Therefore: for Vox, run Stage 2 only, initialised from stock `kokoro-v1_0.pth`.** This is
the pattern the community converged on, and it is what the shipped decoder-only checkpoint is
actually suited to. Running Stage 1 on a single voice is a documented anti-pattern
(StyleTTS2#177).

### 2.1 Pretrained artifacts you must download

| Artifact | What | Where |
|---|---|---|
| `step_1000000.t7` (~25 MB) | PL-BERT, 12-layer **ALBERT**, vocab 178, hidden 768, 12 heads, FF 2048, max_pos 512 | `yl4579/StyleTTS2/Utils/PLBERT` |
| `epoch_00080.pth` (~91 MB) + `config.yml` | ASR text aligner, n_token 40, trained on LibriTTS + JVS + AISHELL | `yl4579/StyleTTS2/Utils/ASR` |
| `bst.t7` (~21 MB) | JDCNet pitch extractor | `yl4579/StyleTTS2/Utils/JDC` |
| WavLM | SLM adversarial discriminator, loaded from your config via `transformers.AutoModel` (commonly `microsoft/wavlm-base-plus`) — **exact default UNVERIFIED** | your config |
| `kokoro-v1_0.pth` + `config.json` | base weights + the 178-symbol vocab | `hexgrad/Kokoro-82M` |
| `monotonic_align` | must be compiled | `resemble-ai/monotonic_align` |
| `kokoro_base.pth` | ⚠️ **inference-only, has NO `style_encoder`** — see §4.1 | — |

### 2.2 Reference recipes (all community; none is official)

| Repo | Notes |
|---|---|
| **`semidark/kikiri-tts`** (Apache-2.0) | The canonical patched StyleTTS2 fork. `docs/TRAINING_GUIDE.md` + `TROUBLESHOOTING.md`. Reports 10 GB+ VRAM, `bs=4` on 12 GB |
| `tterrasson/kokoro-french` | Best-documented fork. Adds `export_checkpoint.py`, speaker clustering, a `verify` step, and a **`--style-encoder-model`** flag |
| `Jeevav62/tts-finetune-recipes/kokoro-recipe` | **Closest to our case** — self-contained English single-voice, bundles all four pretrained artifacts, 1 058 clips. 12–24 GB VRAM; RTX 4090 ≈ 10 min–1.25 h Stage 1, 1–3 h Stage 2 |
| `shreyaskarnik/bol-tts-marathi` | Documents four transferable tricks (see §4.4) |
| ~~`sammy4321/Kokoro-Indic-Fine-Tuning`~~ | ⚠️ **Could not be verified to exist.** The GitHub account has 10 public repos, none TTS-related. Do not cite it |

---

## 3. Capacity scaling (E3/E4) is NOT the recommended path

The original plan's ablation matrix ran 82M → ~95M → ~110M. **On the real target that
sequence is largely unaffordable**, and §1's correction compounds it: the target is a
**4-core laptop at 0.65 RTF**, giving ~1.5× headroom, not the ~4× a 32-core box suggests.

Widening components also has a specific structural cost here: any change to the text
representation, `dim_in`, `hidden_dim`, or the style path **invalidates every existing
voicepack**, because `ref_s` semantics change. §5 of the original plan flagged this; it is
not a footnote, it is the dominant cost of a scaling experiment.

**Recommendation: hold E3/E4 in reserve.** If E2 succeeds and the blind gate says "better
but not enough", scale **one** component — the **prosody predictor** is the highest-leverage
candidate, because duration/F0/energy prediction is precisely what produces the "no pauses,
no pitch, no cadence" complaint — and re-measure. Do not widen uniformly.

---

## 4. The documented failure modes you will hit

This is the section that determines whether this plan succeeds. **All of these are
reproducible; all have known fixes.**

### 4.1 🔴 Silent random-init of `style_encoder` → Stage 2 outputs static noise

`load_state_dict(..., strict=False)` means loading `kokoro-v0_19.pth` / `kokoro_base.pth`
leaves `style_encoder` **randomly initialised, with no error at all.** (Kokoro never shipped
that module.) Extracted voicepack acoustic norm collapses **2.2 → 0.38** or `NaN`.

**Falsifiable, cheap diagnostic — memorise this:**

| Stage-2 mel loss | Meaning |
|---|---|
| **≈ 8.3** | nothing loaded — broken |
| **≈ 0.43** | pretrained weights loaded correctly |

**Fix:** `second_stage_load_pretrained: false` → load from `first_stage.pth` (all 13 modules).
Never rely on `strict=False` succeeding silently.

### 4.2 🔴 Six independent bugs in the Stage-2 recipe (kikiri-tts#1)

1. `DataParallel` wrapped **before** `load_checkpoint` → `module.` prefix mismatch → silent weight-load failure. **Load first, wrap after.**
2. `ignore_modules` still listed `bert`, `bert_encoder`, `predictor` → pretrained prosody silently discarded. Remove them.
3. `y_rec_gt` / `y_rec_gt_pred` deleted from `train_second.py` (looked like dead code because `joint_epoch=999`) → WavLM discriminator had no ground truth. Restore before the adversarial block.
4. GAN gated on `diff_epoch` instead of `joint_epoch` → discriminator never activated. Gate `start_ds` on `joint_epoch`.
5. Diffusion sampler invoked with diffusion disabled → garbage styles into the discriminator. Bypass with `s_preds = s_trg`.
6. `second_stage_load_pretrained: true` loading the inference-only `kokoro_base.pth` → see §4.1.

### 4.3 🔴 Quality collapses after epoch 1, then rebuilds

*"After the first epoch, the model seems to lose much of the original Kokoro
linguistic/prosodic quality (it actually sounds terrible!) and then gradually rebuild quality
over subsequent epochs with the target speaker ID progressively adding up. This feels more
like a full fine-tune than a speaker adaptation."* (kikiri-tts#37)

**Checkpoint every epoch from 0.** The best checkpoint is routinely **not** the last one;
community rule of thumb is **overfit after epoch 4–5**.

Related and important: *"when `joint_epoch` starts and SLM/adversarial/decoder updates become
active, I hear more metallic / muffled / typical TTS artifacts… Once adversarial kicks in,
those artifacts become much more pronounced. **The fine-tuned checkpoint sounds much cleaner
when I use an OFFICIAL Kokoro voicepack** like `am_michael.pt`."*

→ **The style-encoder/voicepack path is where the metallic artifact lives, and the
adversarial phase amplifies it.** Two responses: (a) delay or disable `joint_epoch`
adversarial training initially; (b) always audition the fine-tuned weights against a
**stock** voicepack as a control — that isolates "bad voicepack" from "bad model".

### 4.4 Prosodic-half norm collapse — a diagnostic *and* an emergency knob

Measured prosodic-half norms: stock packs `1.996 / 1.932 / 1.936 / 1.892 / 2.021`; a Stage-2
pack at **0.397**. The 0.397 one **sounded better**, because *"a near-zero prosodic vector
effectively mutes speaker conditioning of the duration/F0/energy predictors, so the
well-trained English Kokoro prosody comes through instead of my under-trained one."*
Measured effect of the swap alone: duration 6.91 s → 4.47 s; spectral-movement spread
1101 → 1658.

**A workaround, not a solution** — but a usable diagnostic and a usable emergency knob.

### 4.5 Upstream StyleTTS2 traps

| Trap | Detail |
|---|---|
| Unseen-speaker collapse | yl4579: *"If your fine-tuning data has less than 1000 speakers, the performance for unseen speakers will be worse than the base model."* (StyleTTS2#177) |
| `max_len` too small | `max_len=100` (≈1.25 s) causes end-of-audio pops/distortion; `800` (≈10 s) is clean. Add 100 ms trailing silence + a stop token (#81) |
| **DDP is broken** for `train_second.py` | spectral-norm buffer broadcast is an in-place op. `predictor_encoder.train()` "fixes" the error but gives **much higher F0 loss and a worse model**. Use `accelerate` single-process (#7) |
| `torch >= 2.6` | `torch.load` defaults to `weights_only=True` → legacy checkpoints fail. Pass `weights_only=False` |
| `weight_norm` | API migration required in `istftnet.py` |

### 4.6 Inference-mode limits (pre-existing, not caused by training)

- Context length **512**; >400 tokens → *"rushing"*; <10–20 → weak
- Premature stop + trailing noise on long input (token-size limit, #8)
- The `510 × 256` style bank means `voices.bin` must be **re-extracted** for every new
  backbone, and **every one of the 11 production voices re-validated** with an audio
  compatibility test — even when dimensions are unchanged

---

## 5. Data

### 5.1 What we need

**5–20 h of conversational English** for Stage-2 personalisation on a good base. (2–5 h is
the community floor; a working ~5 h fine-tune exists; StyleTTS2's own Colab uses 15 min for
a single speaker and yl4579 says 1 h is good.) Utterances must not exceed `max_len`.

⚠️ **Kokoro's own per-voice quality grading**: `_M_ < 1 min` → grade D−, up to `HH`
(10–100 h) → A−. **Under ~10 min per voice = grade C/C+.** So a 5 h Stage-2 run starts from
a weak voice by Kokoro's own grading, and that is a reason for scepticism about how much a
small dataset can buy.

### 5.2 The conversational-data problem is unsolved, and that is the finding

There is **no** corpus that is simultaneously permissive **and** transcribed **and**
conversational **and** multi-speaker. Verified:

| Corpus | Licence | Conversational? | Speaker IDs | Transcript exactness | Usable? |
|---|---|---|---|---|---|
| **LibriTTS-R** | `CC BY 4.0` | 🔴 read | ✅ 2 456 | ✅ exact | best foundation data available |
| **The People's Speech** | `CC-BY` via `clean`/`dirty` configs (**exclude `*_sa`**) | 🟡 | ❌ **no speaker field** | ❌ known misalignments | usable for a single target voice, not for speaker work |
| **VoxPopuli** (en) | `CC0-1.0` (data) | 🟡 oratory, interruptions | ✅ 4 990 | ✅ EP verbatim | good prosody source |
| **Emilia-YODAS** | `CC-BY-4.0` ✅ | 🟡 in-the-wild | ⚠️ video-scoped | ⚠️ Whisper ASR | needs ASR round-trip gate |
| **AMI** (`ihm`) | `CC BY 4.0` | 🟢 true spontaneous | ✅ persistent | ✅ human | 16 kHz, mostly non-native |
| **CREMA-D** | `ODbL-1.0` + `DbCL-1.0` | 🟡 acted, 6 emotions, 91 spk | ✅ | ✅ fixed sentences | **share-alike** — viral |
| Emilia / Expresso / EmoV-DB / DailyDialog / TED-LIUM | 🔴 NC or NC+ND | — | — | — | **excluded** |
| OpenDialog (6.8k h dialogue) | 🔴 **`CC-BY-NC-4.0`** | 🟢 best | ❌ per-dialogue only | ⚠️ WhisperD | **NC — excluded** |

**Recommended mixture:** LibriTTS-R (foundation, exact transcripts) + VoxPopuli or
The People's Speech CC-BY subset (prosody) + AMI `ihm` (genuine spontaneous cadence).

### 5.3 Synthetic teacher data: augmentation only, never the target

BELLE (arXiv:2510.24372) is the strongest positive result: 706 h real + 4 111 h synthetic
from **six** heterogeneous teachers → ~5 k hours beat leading open models trained on 50 k
hours (25.8 % relative WER reduction). **But its own ablation: GT-only 3.81 % WER vs
GT+2-teachers 7.78 %.** Pure-synthetic is *worse* than human ground truth. Synthetic is
augmentation for variance, not a target.

Failure modes to engineer against:
- **Artifact inheritance compounds** — *"Even if the average human listener cannot perceive
  these artifacts… they may be captured and can potentially accumulate in the second model"*
  (arXiv:2208.13183)
- **Noise sensitivity is brutal** — degradation is only mitigable when noisy speech is
  **≤0.1 %** of the corpus (arXiv:2512.17356). Resampling + Whisper + synthesis is a 3-step
  pipeline; each step can introduce >0.1 % bad audio
- **Distillation collapses output entropy** — Spotify KD measured student top-1 prob >0.8
  (SSW 2025). Direct risk for prosodic range, which is our whole goal
- **⚠️ "Standard speaking styles facilitate more effective model learning"**
  (arXiv:2512.17356) — this cuts *directly against* a conversational objective. The
  literature's own finding is that monotone data trains better
- **Watermark contamination** — Chatterbox embeds a PerTh watermark in every clip
- **ShareAlike viral contamination** — Fish-Speech S1-mini is `cc-by-nc-sa-4.0`; the legal
  status of TTS-generated audio under CC is unsettled
- **hexgrad's own data policy** — Kokoro was trained on synthetic audio *"generated by closed
  TTS models from large providers"* and explicitly **not** on open-TTS output, citing US
  Copyright Office guidance. Treat that as a signal about provenance risk
- **Unaudited:** the paper *"Training Text-to-Speech Model with Purely Synthetic Data"*
  transfers WER/UTMOS. **Nobody has shown prosody transfers** from a large teacher into a
  small non-autoregressive StyleTTS2-family model. Our core hypothesis is untested in the
  literature.

**Teacher candidates** (Apache-2.0, fits 16 GB): `Qwen/Qwen3-TTS-12Hz-1.7B-Base` or
`0.6B-Base`, `FunAudioLLM/Fun-CosyVoice3-0.5B-2512` (+`_RL`). ❌ avoid Fish-Speech
(`CC-BY-NC-SA`), XTTS-v2 (CPML, no commercial licence obtainable since Coqui shut down),
Spark-TTS (`CC-BY-NC-SA`), IndexTTS-2 (bilibili licence, **not** Apache-2.0 despite
third-party sites claiming otherwise).

**If you generate synthetic data, use ≥3 heterogeneous teachers** (BELLE used 6) and
enforce a hard 0.1 % noise floor with DNSMOS + ASR round-trip on 100 % of it.

### 5.4 Data hygiene — non-negotiable

*"The wav files were abruptly cut off, but you couldn't actually hear it in the source data!
The model basically got hiccups. A closer look in Audacity then showed the waveforms went all
the way to the very end of the file."* → **clean the entire dataset, not just what sounds
bad.** Invoke the `create-dataset` skill; do not hand-roll this.

---

## 6. Experiment sequence

| ID | Question | Change | Gate |
|---|---|---|---|
| **F0** | Can we reproduce a known-good community fine-tune? | none (reproduction) | intelligible + stable output |
| **F1** | How much does **data** alone buy? | dataset + Stage-2 config, 82M fixed | blind A/B vs baseline |
| **F2** | Does **conversational** data specifically help? | + conversational mixture, best F1 recipe | blind A/B + no regression in pronunciation/identity |
| **F3** | Is the quality ceiling **data** or **capacity**? | + one scaled component (§3) | only if F2 plateaus |

**F0 is a hard gate.** If the pipeline cannot reproduce a known-good result, do not begin
architectural work — every downstream conclusion would be built on a broken instrument.

### F0 checklist
- [ ] `monotonic_align` compiled
- [ ] All four pretrained artifacts downloaded and checksummed
- [ ] Stage-2 mel loss ≈ **0.43**, not 8.3 (§4.1) — **the single most important early check**
- [ ] Voicepack extracted; acoustic norm **not** collapsed to 0.38 / NaN
- [ ] `accelerate` single-process, **not** DDP (§4.5)
- [ ] Per-epoch checkpoints from epoch 0
- [ ] Stock voicepack audition as a control (§4.3)

### F1/F2 starting config (from `Jeevav62`, the closest published English case)
```
epochs 10 · batch_size 2 · max_len 180 · joint_epoch 99 (GAN OFF, saves ~4 GB VRAM)
lambda_mel 5.0 · lambda_F0 2.0 · lambda_ce 20.0 · g2p en-gb
```
`joint_epoch 99` deliberately disables the adversarial phase initially — per §4.3 that is
where the metallic artifacts come from. Re-enable only after a clean F1 baseline.

### Per-checkpoint record (every meaningful one)
config · dataset version + licence · param count · step/epoch · validation metrics ·
fixed-corpus audio · objective metrics · **RTF** (measured through the **exported ONNX**,
not the PyTorch graph) · memory · qualitative notes.

**RTF must be measured on the exported `model.onnx` + regenerated `voices.bin` + `tokens.txt`
through sherpa-onnx.** A PyTorch-side timing number is not comparable to production and
would be a false green.

---

## 7. Export + voicepack (unchanged from the original plan, still correct)

Reuse the sherpa-onnx Kokoro export path — no custom exporter needed:
`v1.0/export_onnx.py` → `model.onnx`; `v1.0/generate_tokens.py` → `tokens.txt`;
`v1.0/generate_voices_bin.py` → `voices.bin` (**must be modified** to emit your trained
voices). All are in `sherpa-onnx/scripts/kokoro/v1.0/`.

**Deployment is a 3-file swap:** `model.onnx` + `voices.bin` + `tokens.txt`. The
multi-language assets (`espeak-ng-data/`, `lexicon-*.txt`, `dict/`, `*-zh.fst`) are
**G2P-side and unaffected by fine-tuning.**

⚠️ `add_meta_data.py` writes `metadata_props` including `comment`. The incumbent artifact
carries `comment = "This is kokoro v0.19 and supports only English"` — which happens to be
**correct here**, but that field is a cosmetic free-text string that export tooling
overwrites freely. **Do not treat it as ground truth about the weights** (a prior agent
made exactly that mistake, in the opposite direction).

### Voicepack obligations
- [ ] Re-extract `voices.bin` for **every** changed backbone
- [ ] Validate **all 11** production voices with an audio compatibility test
- [ ] Preserve the voice→SID mapping contract
- [ ] Before/after samples for every voice
- [ ] Confirm the Sherpa-ONNX package loads

---

## 8. Deployment acceptance

1. Blind evaluation shows **improvement or meaningful parity** vs the current baseline
2. Measurably better prosody / pacing / conversational delivery
3. **No** regression in pronunciation, clipping, artifacts, or speaker consistency
4. **RTF < 1.0** on the target (4-core laptop), measured through the exported ONNX
5. Practical memory for local deployment
6. Exports through the existing Kokoro/Sherpa path, or exporter changes are documented
7. Validated `voices.bin` for the final backbone
8. Voice/SID contract preserved where possible
9. Reproducible from recorded dataset version + config + seed

---

## 9. Honest cost estimate

| Phase | Estimate | Confidence |
|---|---|---|
| F0 environment + reproduction | 2–4 days | medium |
| F1 data pipeline + first good Stage-2 | 1–2 weeks | medium |
| F2 conversational mixture | 1 week | medium |
| F3 capacity (only if F2 plateaus) | 1–2 weeks | low |
| Export + 11-voice validation | 2–3 days | high |

**Total to F2: 3–5 weeks.** Against that: ZipVoice reached **measured RTF parity in one
afternoon** and is waiting on a blind listen. **Run ZipVoice first.** This plan is the
fallback for the case where ZipVoice's licensing, streaming, or reference-asset constraints
prove binding — and note that all three are real, unresolved risks in the ZipVoice plan.

**Do not run both at once.** They share the same blind-listening gate and the same
evaluation corpus; interleaving them makes both unattributable.
