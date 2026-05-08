# FINETUNING.md

# Vox Speech Fine-Tuning Pipeline

## Goal

Build a Hindi + Hinglish speech corpus for:

1. STT fine-tuning

   * Qwen3-ASR-0.6B
   * Output target:

     * English → English
     * Hindi → Roman Hindi
     * Hinglish → Roman Hinglish

2. Future TTS fine-tuning

   * Piper Hindi voice
   * conversational Hindi/Hinglish synthesis

---

# Current Server

## Hardware

* GPU:

  * RTX 5070 Ti
  * 16GB VRAM

* CPU:

  * 32 cores

* RAM:

  * sufficient for parquet processing + training

---

# Core Requirement

IMPORTANT:

The final ASR model must NOT output Devanagari.

Example:

BAD:

```text
तुम कैसे हो
```

GOOD:

```text
tum kaise ho
```

This requires:

* transliteration pipeline
* Roman Hindi normalization
* Hinglish preservation

---

# Primary Datasets

## 1. IndicVoices (Main ASR dataset)

Source:
https://huggingface.co/datasets/ai4bharat/IndicVoices

Use:

* Hindi subset only
* conversational + extempore preferred

Purpose:

* primary STT fine-tuning corpus

---

## 2. Kathbath

Source:
https://huggingface.co/datasets/ai4bharat/Kathbath

Use:

* Hindi split only

Purpose:

* conversational robustness
* noisy Indian speech

---

## 3. HiACC

Purpose:

* Hinglish code-switching
* Roman Hindi behavior

High importance despite small size.

---

## 4. IndicVoices-R (Later)

Purpose:

* TTS fine-tuning
* speaker diversity
* Piper training

DO NOT process yet.

---

# Datasets To Ignore Initially

Ignore:

* podcasts
* manually transcribed audio
* random YouTube audio

These can be used later only for:

* augmentation
* evaluation
* conversational testing

---

# Corpus Strategy

We are NOT training multilingual ASR.

We are intentionally biasing toward:

* Hindi
* English
* Hinglish

All other languages should be filtered out aggressively.

---

# Final Corpus Structure

data/
├── raw/
│   ├── indicvoices/
│   ├── kathbath/
│   ├── hiacc/
│   └── audiobooks/
│
├── processed/
│   ├── audio_16khz/
│   ├── manifests/
│   ├── romanized/
│   └── cleaned/
│
├── exports/
│   ├── qwen_asr/
│   └── piper_tts/
│
└── cache/

---

# Final STT Dataset Format

JSONL:

```json
{
  "audio": "path/to/audio.wav",
  "text_raw": "तुम कैसे हो",
  "text_roman": "tum kaise ho",
  "language": "hi"
}
```

IMPORTANT:

* never overwrite raw transcript
* preserve both forms

---

# Audio Requirements

Convert ALL audio to:

* mono
* 16kHz WAV

Using ffmpeg.

---

# Dataset Cleaning Rules

Remove:

* corrupted audio
* music-heavy clips
* overlapping speakers
* clips shorter than 1 second
* clips longer than 20 seconds
* non-Hindi dominant clips

---

# Transliteration Pipeline

We will use:

* Gemini Flash API

NOT local LLM.

Reason:

* cheap
* highly accurate
* consistent transliteration

Task:

```text
Hindi/Hinglish → Roman Hindi
```

Example:

```text
"मुझे जाना है"
→
"mujhe jaana hai"
```

---

# Training Targets

## STT Fine-Tuning

Target:

* 100–300 hours high-quality Hindi/Hinglish

Preferred:

* quality over quantity

---

## Piper TTS Fine-Tuning

Target:

* 10–30 hours
* clean single-speaker audio

Use later.

---

# Important Processing Rules

DO:

* keep raw datasets untouched
* create processed copies
* preserve metadata
* preserve timestamps

DO NOT:

* overwrite original transcripts
* inject fake emotions
* inject fake laughter tags

---

# Required CLI Tools

Expected available:

* ffmpeg
* unzip
* duckdb
* python
* huggingface-cli

---

# Required Python Libraries

* datasets
* pyarrow
* duckdb
* pandas
* faster-whisper
* transformers
* soundfile

---

# Dataset Download Strategy

Use:

* huggingface-cli
* git-lfs only if necessary

Prefer:

* streaming parquet processing
* avoid extracting everything into RAM

---

# Initial IDE Tasks

1. Download Hindi subsets only
2. Extract parquet safely
3. Build unified manifest format
4. Normalize audio
5. Create transliteration pipeline
6. Export final JSONL
7. Generate train/val/test splits

---

# Immediate Focus

Immediate focus is ONLY:

* STT fine-tuning dataset creation

NOT:

* TTS training
* ONNX export
* inference optimization
