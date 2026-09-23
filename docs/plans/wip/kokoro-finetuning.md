I researched the current Kokoro training ecosystem, recent fine-tuning work, expressive/prosody research, and current teacher-model options. The PRD below is intentionally written as a handoff from a product/ML lead: it defines the target, experimental sequence, checkpoints, baseline, and acceptance criteria without prescribing implementation details that should be validated against the actual training stack.

# Vox Conversational TTS — Kokoro-Derived Model

## 1. Objective

Develop a Vox-optimized conversational TTS model derived from Kokoro/StyleTTS2.

The target is a model in the approximate **90–110M parameter range** that improves conversational naturalness, prosody, pacing and expressive variation over the current Kokoro baseline while maintaining **RTF < 1.0** under the target Vox inference environment.

The model should remain small enough to be practical as a local TTS engine rather than becoming a general-purpose large TTS model.

The primary optimization target is:

**Natural conversational speech + expressive prosody + low latency**

not parameter count.

---

## 2. Starting point

Use the released **Kokoro-82M** model as the primary baseline.

Kokoro is an 82M-parameter StyleTTS2 + iSTFTNet model operating at 24 kHz. The official v1.0 release was trained on a few hundred hours of data and reports 8 languages / 54 voices. [1]

The underlying StyleTTS2 approach combines text/acoustic modeling with learned style, duration/prosody prediction, style diffusion and adversarial training using a pretrained speech-language model discriminator. [2]

The existing community ecosystem now has reproducible Kokoro fine-tuning workflows covering dataset preparation, Stage 1, Stage 2 and voicepack extraction. [3][4]

The first experiment should therefore **not** modify the architecture.

Establish what can be gained from better data, training and Stage-2/prosody optimization while keeping the architecture at 82M.

---

## 3. Product definition

The resulting model is intended for:

* realtime conversational assistants
* short and medium conversational turns
* streaming clause-level TTS
* natural pauses and pacing
* varied sentence-level prosody
* conversational emphasis
* mild emotional/expression variation
* CPU/local deployment where possible
* GPU acceleration when available

It is not intended to become:

* a voice-cloning research model
* a multilingual foundation TTS model
* a long-form audiobook specialist
* a large diffusion TTS system
* a replacement for every existing Vox TTS provider

---

## 4. Baseline

Before changing anything, establish a reproducible Kokoro baseline.

Record:

**Model:** official Kokoro-82M
**Sample rate:** 24 kHz
**Voice:** fixed reference Vox voice
**Inference implementation:** current production implementation
**Hardware:** target deployment hardware
**Test corpus:** fixed evaluation set
**RTF:** measured end-to-end and model-only where practical
**Peak memory:** measured
**Audio quality:** human evaluation
**Prosody:** human evaluation
**Conversational naturalness:** human evaluation

The evaluation corpus should contain approximately 100–200 carefully selected utterances covering:

* short acknowledgements
* questions
* statements
* long sentences
* numbers
* punctuation
* interruptions / conversational fragments
* emphasis
* emotional language
* multi-clause sentences
* sentences requiring natural pauses

The exact corpus and scoring procedure should be proposed by the ML engineer after inspecting the available data and runtime.

Every subsequent checkpoint must be evaluated against this same corpus.

---

## 5. Quality evaluation

Do not use training loss as the primary quality metric.

StyleTTS2's architecture explicitly relies on adversarial and style/prosody objectives, and community Kokoro fine-tuning has demonstrated that checkpoints with better numerical validation metrics do not necessarily sound better in listening tests. [2][3]

The evaluation should therefore produce four separate dimensions:

**Naturalness** — does this sound like natural human speech?

**Prosody** — are duration, rhythm, stress, pitch movement and pauses natural?

**Conversational quality** — does it sound appropriate when spoken by an assistant rather than read from a page?

**Runtime** — RTF, first-audio latency, throughput and memory.

Keep the dimensions separate. Do not collapse them into one arbitrary score.

---

## 6. Dataset strategy

Use a two-layer dataset strategy.

### Layer A — acoustic foundation

Start with a large, clean, permissively licensed speech corpus suitable for general acoustic/prosodic learning.

**LibriTTS-R** is an important candidate. It contains approximately 585 hours of 24 kHz speech from 2,456 speakers and corresponding text. The published work reports improved audio quality over LibriTTS and TTS systems trained on LibriTTS-R achieving naturalness comparable to ground-truth recordings in their experiments. [5]

The engineer should evaluate whether its speaking style is sufficiently conversational for the target rather than assuming that scale alone solves conversational naturalness.

### Layer B — conversational / expressive data

Add a smaller, higher-quality subset emphasizing:

* spontaneous speech
* varied speaking rate
* natural pauses
* emphasis
* emotional variation
* conversational sentence structure
* expressive delivery

The engineer should investigate available public/licensed expressive and conversational corpora and recommend the final mixture based on licensing, transcription quality, speaker diversity and acoustic quality.

Do not blindly mix every available dataset.

Data quality and text/audio alignment are first-class requirements. The Kokoro model card itself notes that poor audio, insufficient per-voice data and text/audio misalignment directly affect inference quality. [6]

---

## 7. Training approach

Use the existing Kokoro/StyleTTS2 two-stage training structure as the starting point.

**Stage 1:** establish acoustic reconstruction, alignment and decoder quality.

**Stage 2:** optimize duration, pitch/energy/prosody, style and adversarial objectives.

The original StyleTTS2 implementation separates these stages, while current Kokoro fine-tuning recipes follow the same general structure. [2][3][4]

The engineer should first reproduce a known-good fine-tuning run before modifying the architecture.

This is an important checkpoint: if the training pipeline cannot reproduce a known-good result, architectural experiments should not begin.

---

## 8. Experiment sequence

### Experiment 0 — Infrastructure validation

Reproduce Kokoro inference and a known community fine-tuning recipe.

Deliver:

* reproducible environment
* dataset preprocessing pipeline
* Stage 1 training
* Stage 2 training
* checkpoint generation
* inference/export
* baseline evaluation

Success means the pipeline produces intelligible, stable speech and measurements can be reproduced.

---

### Experiment 1 — 82M data/training baseline

Keep the architecture unchanged.

Investigate:

* dataset composition
* audio filtering
* transcript quality
* phonemization
* speaker balancing
* utterance-length distribution
* Stage 1 configuration
* Stage 2 configuration
* diffusion/style training
* adversarial training
* checkpoint selection

The goal is to determine how much quality improvement is available without increasing model size.

Recent Kokoro community work is particularly relevant here: one documented fine-tuning effort found that Stage-2 configuration, diffusion training and crop length materially affected prosody and naturalness. [7]

---

### Experiment 2 — Conversational data mixture

Introduce the curated conversational/expressive dataset into the strongest 82M configuration.

Compare against Experiment 1 using the identical evaluation corpus.

The question is:

**Does better conversational data produce a measurable improvement in conversational naturalness and prosody without unacceptable degradation of pronunciation, speaker consistency or runtime?**

---

### Experiment 3 — 90–95M architecture

Only if Experiment 2 establishes a strong 82M ceiling, introduce a modest capacity increase.

Do not widen the entire network indiscriminately.

The engineer should profile the architecture and identify where additional capacity is most likely to benefit:

* style representation
* prosody/duration modeling
* text representation
* decoder capacity

The experiment should explicitly test whether the additional parameters improve perceptual quality enough to justify their runtime cost.

---

### Experiment 4 — 100–110M architecture

If the 90–95M model demonstrates a useful quality/latency tradeoff, test a larger configuration approaching 100–110M parameters.

The architecture should remain recognizably Kokoro/StyleTTS2-derived.

The engineer should compare at least:

**82M -> ~95M -> ~110M**

using the same dataset, evaluation set and inference implementation.

The objective is to find the smallest model that reaches the desired quality rather than automatically selecting the largest model.

---

## 9. Teacher models

Larger TTS models may be used as **references and teachers**, not as architectural dependencies of the final Vox model.

Candidate teacher/reference systems include:

* Qwen3-TTS
* OmniVoice
* other strong locally runnable TTS models identified during experimentation

Qwen3-TTS currently provides 0.6B and 1.7B variants and supports multilingual controllable speech generation. [8]

OmniVoice is a newer multilingual zero-shot TTS architecture with more than 600-language coverage and a 581k-hour training corpus; its published architecture is fundamentally different from Kokoro and therefore it should be treated as a quality/reference system rather than a direct architectural template. [9]

Teachers may be useful for:

* generating alternative reference speech
* producing expressive examples
* creating difficult evaluation cases
* comparing prosody
* identifying failure modes
* generating candidate synthetic training data where licensing and quality permit
* providing a secondary judge signal

Teacher output must not automatically be treated as ground truth.

---

## 10. LLM-assisted evaluation

A local language model such as Qwen3.5-9B may be used to:

* generate conversational test prompts
* generate controlled linguistic variations
* classify intended conversational context
* assist with evaluation metadata
* act as a secondary judge

LLM judgments should supplement, not replace, listening evaluation.

For example:

**same text + multiple TTS checkpoints -> blind audio comparison -> human evaluation + automated/LLM analysis**

The engineer should investigate whether the available model can reliably evaluate the desired dimensions before incorporating its score into experiment decisions.

---

## 11. Runtime target

Hard requirement:

**RTF < 1.0**

Preferred target:

**meaningfully below the current Kokoro runtime**

The existing Vox baseline is approximately 0.7 RTF, so increasing model size is only justified if the resulting quality improvement is substantial enough to justify the runtime cost.

Measure:

* time to first audio
* complete utterance RTF
* clause-level RTF
* peak VRAM
* peak RAM
* CPU utilization
* GPU utilization
* warm inference
* cold/startup behavior

Measurements must use the same inference path used by the intended Vox integration whenever possible.

The engineer should verify any environment-sensitive performance assumptions experimentally rather than treating published benchmark numbers as directly comparable.

---

## 12. Hardware

Primary training environment:

**RTX 5070-class GPU with 16 GB VRAM**

The engineer should first establish the largest stable batch/crop/configuration that fits the actual environment.

Mixed precision, gradient accumulation, checkpointing and other memory optimizations should be considered only after establishing a clean baseline.

The 16 GB GPU is an experimental constraint, not a reason to redesign the model prematurely.

Large teacher models can be run separately when necessary so teacher inference does not constrain the final model architecture.

---

## 13. Checkpoint policy

Never assume the final epoch is the best checkpoint.

Save checkpoints frequently enough to permit perceptual comparison.

For every meaningful checkpoint record:

* training configuration
* dataset version
* parameter count
* training step/epoch
* validation metrics
* audio samples
* RTF
* memory
* qualitative observations

Checkpoint selection should be based on the complete evaluation rather than a single loss.

---

## 14. Ablation matrix

The engineer should maintain a compact experiment matrix:

| Experiment | Architecture | Data             | Training          | Purpose                       |
| ---------- | -----------: | ---------------- | ----------------- | ----------------------------- |
| E0         |          82M | Existing         | Reproduction      | Establish baseline            |
| E1         |          82M | Clean foundation | Improved training | Measure training/data ceiling |
| E2         |          82M | + conversational | Best E1 recipe    | Measure data impact           |
| E3         |         ~95M | E2               | Best recipe       | Measure moderate scaling      |
| E4         |        ~110M | E2               | Best recipe       | Measure larger scaling        |

Additional ablations should be added only when they answer a specific uncertainty.

---

## 15. Acceptance criteria

The final candidate should satisfy all of the following:

1. **Naturalness:** clearly improves or meaningfully matches the existing Kokoro baseline under blind listening.

2. **Conversational prosody:** demonstrates more natural pacing, pauses, emphasis and pitch variation on conversational evaluation material.

3. **Stability:** no significant increase in pronunciation errors, clipping, artifacts or speaker inconsistency.

4. **Runtime:** RTF remains below 1.0 under the target inference environment.

5. **Memory:** remains practical for the intended local deployment environment.

6. **Reproducibility:** training and evaluation can be repeated from recorded configuration/data versions.

7. **Deployment:** the final checkpoint can be integrated into the existing Vox TTS worker without requiring changes to the conversational orchestration architecture.

---

## 16. Final deliverables

The ML engineer should return:

**Model:** final checkpoint + inference/export format

**Training:** reproducible training configuration and preprocessing pipeline

**Dataset:** exact dataset composition, licenses, filtering and splits

**Evaluation:** fixed benchmark corpus and results

**Ablations:** 82M / ~95M / ~110M comparison

**Runtime:** RTF, latency and memory measurements

**Audio:** blind-comparison samples for baseline and candidate checkpoints

**Recommendation:** identify the smallest model/configuration that satisfies the quality and runtime requirements, with evidence supporting the choice.

---

## 17. Guiding principle

Do not approach this as “make Kokoro bigger.”

Approach it as:

**Kokoro-82M baseline -> understand quality ceiling -> improve data/prosody/training -> measure -> add capacity only where evidence shows it helps -> select the smallest model that materially improves conversational speech while staying realtime.**

The key external references behind the PRD are:

1. **Kokoro-82M official model card** — 82M architecture, releases, training data and training compute. ([Hugging Face][1])
   [Kokoro-82M on Hugging Face](https://huggingface.co/hexgrad/Kokoro-82M?utm_source=chatgpt.com)

2. **StyleTTS2 paper** — style diffusion, adversarial training, duration modeling and the underlying rationale for naturalness. ([arXiv][2])
   [StyleTTS2 paper](https://arxiv.org/abs/2306.07691?utm_source=chatgpt.com)

3. **StyleTTS2 implementation** — actual Stage 1/Stage 2 training structure and configuration. ([GitHub][3])
   [StyleTTS2 GitHub](https://github.com/yl4579/StyleTTS2?utm_source=chatgpt.com)

4. **Kikiri-TTS** — current reproducible Kokoro fine-tuning workflow and its documented training fixes. ([GitHub][4])
   [Kikiri-TTS GitHub](https://github.com/semidark/kikiri-tts?utm_source=chatgpt.com)

5. **Kokoro French fine-tuning** — concrete Stage 1/Stage 2 fine-tuning workflow. ([GitHub][5])
   [Kokoro French GitHub](https://github.com/tterrasson/kokoro-french?utm_source=chatgpt.com)

6. **LibriTTS-R** — 585 hours, 2,456 speakers, 24 kHz and reported naturalness results. ([arXiv][6])
   [LibriTTS-R paper](https://arxiv.org/abs/2305.18802?utm_source=chatgpt.com)

7. **Kokoro Indic fine-tuning research notes** — useful evidence around Stage-2 crop length, diffusion and prosody training. ([GitHub][7])
   [Kokoro Indic fine-tuning](https://github.com/sammy4321/Kokoro-Indic-Fine-Tuning?utm_source=chatgpt.com)

8. **Qwen3-TTS** — current 0.6B/1.7B teacher/reference option. ([Hugging Face][8])
   [Qwen3-TTS 0.6B](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-Base?utm_source=chatgpt.com)

9. **OmniVoice** — current 600+ language teacher/reference system and its 2026 architecture/paper. ([arXiv][9])
   [OmniVoice GitHub](https://github.com/k2-fsa/OmniVoice?utm_source=chatgpt.com)

The important architectural decision in this PRD is deliberate: **82M → better data/training → ~95M → ~110M**, rather than starting by modifying Kokoro's architecture and hoping the larger model is better.

[1]: https://huggingface.co/hexgrad/Kokoro-82M/blob/main/README.md?utm_source=chatgpt.com "README.md · hexgrad/Kokoro-82M at main"
[2]: https://arxiv.org/abs/2306.07691?utm_source=chatgpt.com "StyleTTS 2: Towards Human-Level Text-to-Speech through Style Diffusion and Adversarial Training with Large Speech Language Models"
[3]: https://github.com/yl4579/styletts2?utm_source=chatgpt.com "GitHub - yl4579/StyleTTS2: StyleTTS 2: Towards Human-Level Text-to-Speech through Style Diffusion and Adversarial Training with Large Speech Language Models · GitHub"
[4]: https://github.com/semidark/kikiri-tts?utm_source=chatgpt.com "GitHub - semidark/kikiri-tts: This project provides a complete, documented training recipe for fine-tuning Kokoro-82M on a new language. In this case German · GitHub"
[5]: https://github.com/tterrasson/kokoro-french?utm_source=chatgpt.com "GitHub - tterrasson/kokoro-french: This project provides a complete, documented training recipe for fine-tuning Kokoro-82M on a new language. In this case French · GitHub"
[6]: https://arxiv.org/abs/2305.18802?utm_source=chatgpt.com "LibriTTS-R: A Restored Multi-Speaker Text-to-Speech Corpus"
[7]: https://github.com/sammy4321/Kokoro-Indic-Fine-Tuning/blob/main/docs/JOURNEY.md?utm_source=chatgpt.com "Kokoro-Indic-Fine-Tuning/docs/JOURNEY.md at main · sammy4321/Kokoro-Indic-Fine-Tuning · GitHub"
[8]: https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-Base?utm_source=chatgpt.com "Qwen/Qwen3-TTS-12Hz-0.6B-Base · Hugging Face"
[9]: https://arxiv.org/abs/2604.00688?utm_source=chatgpt.com "OmniVoice: Towards Omnilingual Zero-Shot Text-to-Speech with Diffusion Language Models"
