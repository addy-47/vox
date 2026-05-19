# Hinglish Transliteration: Real-Time Hindi-to-Roman Conversion

## Overview

The Hinglish Transliteration feature provides real-time conversion of Devanagari Hindi text to Roman-script Hinglish within Vox's voice recognition pipeline. This enables users who speak Hindi to receive responses in a natural, SMS-style Latin script that preserves conversational phonetics while remaining compatible with downstream language models and text-to-speech systems.

## Problem Statement

Vox supports multimodal conversations including Hindi and Hinglish. When users speak Hindi, the automatic speech recognition (ASR) system outputs Devanagari script. However:

1. Most LLMs are trained primarily on Latin-script text and perform better with Romanized input
2. TTS systems may lack full Devanagari voice support or produce inconsistent results
3. English-Hindi code-switching (Hinglish) is the natural conversational mode for many users
4. Displaying Devanagari in transient UI elements creates readability overhead

The transliteration engine solves these by converting Hindi output to familiar Hinglish in real-time.

## Architecture

### Pipeline Position

```
Audio Input → VAD → ASR (Qwen-ASR) → Transliteration → LLM → TTS
                              ↓
                    Devanagari → Hinglish
```

The transliteration engine sits between ASR and the LLM, intercepting transcripts containing Devanagari characters and converting them before further processing.

### Engine Implementation

The `TransliterationEngine` is implemented in Rust (`app/src-tauri/src/services/translit.rs`):

- **ONNX Runtime Sessions**: Two separate sessions for encoder and decoder
- **Singleton Pattern**: Thread-safe initialization via `OnceLock`
- **Dedicated Thread**: Runs in isolated `vox-translit` thread for non-blocking operation
- **MPSC Channels**: Message passing for concurrent transcript processing

### Core Algorithm

The engine implements a sequence-to-sequence model with attention:

1. **Encoding**: Devanagari characters are tokenized and encoded through a 2-layer BiLSTM
2. **Attention**: Bahdanau attention mechanism aligns encoder outputs
3. **Decoding**: 2-layer LSTM generates Roman character sequence autoregressively

## Model Specifications

| Specification | Value |
|--------------|-------|
| **Architecture** | BiLSTM Encoder + LSTM Decoder with Bahdanau Attention |
| **Layers** | Encoder: 2 (bidirectional), Decoder: 2 |
| **Embedding Dimension** | 128 |
| **Hidden Dimension** | 256 |
| **Parameters** | ~4.1 Million |
| **Inference Latency** | ~2.9 ms per word (CPU, single-threaded) |
| **Model Size** | 16.2 MB (encoder + decoder + vocabularies) |
| **Format** | ONNX Runtime |

### Model Assets

Deployed to `models/translit/`:

| File | Description | Size |
|------|-------------|------|
| `encoder.onnx` | BiLSTM encoder graph | 9.5 MB |
| `decoder.onnx` | Attention decoder graph | 6.7 MB |
| `input_vocab.json` | Devanagari character → index (68 chars) | 946 B |
| `target_vocab.json` | Latin character → index (45 chars) | 346 B |

## Incomplete Word Protection

A critical UX consideration is preventing partial word transliteration artifacts. The `transliterate_if_hi()` function implements:

1. **Boundary Detection**: Identifies if text ends with whitespace, punctuation, or Devanagari danda (`।`)
2. **Tokenization**: Splits text into Devanagari and non-Devanagari segments
3. **Selective Processing**: Only transliterates complete words; skips incomplete final words
4. **Unicode Matching**: Detects Devanagari using range `\u{0900}`-`\u{097F}`

```rust
pub fn transliterate_if_hi(text: &str) -> String {
    // Skip if empty or no Devanagari
    if !is_devanagari(text) { return text.to_string(); }
    
    // Tokenize and process each word
    for token in tokenize(text) {
        if let DevanagariWord(word) = token {
            if is_complete(word) {
                word = transliterate(&word);
            }
            // Incomplete words remain in Devanagari
        }
    }
}
```

## Message Flow

The pipeline processes three message types via MPSC channels:

```rust
enum TranslitTask {
    Token { turn_id, target, token, local_transliterate_enabled },
    Partial { turn_id, target, text, owner, local_transliterate_enabled },
    Final { turn_id, target, text, owner, local_transliterate_enabled },
    Cancel { turn_id },
    Shutdown,
}
```

### Processing Stages

1. **Token Stage**: Individual words are transliterated for streaming LLM input
2. **Partial Stage**: Intermediate transcripts during speech are processed for UI display
3. **Final Stage**: Complete turn transcripts are finalized for response generation

## Settings Integration

The feature is controlled via Vox settings with hot-reload support:

```rust
pub struct VoxSettings {
    pub asr: AsrSettings,
    // ...
}

pub struct AsrSettings {
    pub transliterate_enabled: bool,  // default: true
}
```

Changes take effect immediately via `SettingReloadPolicy::Hot` without pipeline restart.

## Training Pipeline

The model was trained on the `vox-hinglish-rnn` repository:

- **Dataset**: Hugging Face Aksharantar corpus (1.2M Devanagari↔Roman pairs)
- **Lexicon**: `unique_word_pairs.json` with 1,129 Hinglish texting corrections
- **Hardware**: NVIDIA RTX 5070 Ti
- **Configuration**: Batch size 2048, Cross-Entropy loss, 5 epochs
- **Normalization**: Target replacements enforced conversational spelling (e.g., "achha" not "achchha")

## Example Output

| Devanagari Input | Hinglish Output |
|------------------|-----------------|
| नमस्ते | namaste |
| क्या हाल है? | kya haal hai? |
| बाद में काम करेंगे | baad mein kaam karenge |
| मैं आपके बारे में बात कर रहा हूँ | main aapke baare mein baat kar raha hoon |

## Source Repository

`vox-hinglish-rnn/` contains the complete training pipeline:

```
vox-hinglish-rnn/
├── corpus/
│   ├── dataset/
│   │   └── train/validation Arrow splits
│   └── unique_word_pairs.json      # Hinglish corrections
├── models/
│   ├── encoder.pt                   # PyTorch weights
│   ├── decoder.pt
│   ├── input_vocab.json
│   └── target_vocab.json
├── scripts/
│   ├── train_rnn.py                 # Training loop
│   ├── prepare_dataset.py           # Data cleansing
│   └── merge_vocabs.py
└── testing/
    ├── test_inference_onnx.py
    └── test_transcripts.py
```

## Testing

```bash
# Benchmark latency
python testing/test_inference_onnx.py

# Evaluate transcript quality
python testing/test_transcripts.py
```

## Diagnostics

The transliteration engine emits logs at these points:

- **Initialization**: Model loading success/failure
- **Failure**: Fallback to raw Devanagari on error
- **Performance**: Per-word processing time (debug mode)