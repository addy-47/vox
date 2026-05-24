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
| **Inference Latency** | ~0.9 ms per word (CPU, single-threaded) |
| **Model Size** | 16.2 MB (encoder + decoder + vocabularies) |
| **Format** | ONNX Runtime |

### Model Assets

Deployed to `models/translit/`:

| File | Description | Size |
|------|-------------|------|
| `encoder.onnx` | BiLSTM encoder graph | 9.5 MB |
| `decoder.onnx` | Attention decoder graph | 6.7 MB |
| `input_vocab.json` | Devanagari character → index (107 chars) | 1.1 KB |
| `target_vocab.json` | Latin character → index (67 chars) | 512 B |

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

## Training Pipeline & Structural Fixes

The model was retrained on the `vox-hinglish-rnn` repository to resolve character drops and expand coverage:

### 1. Corpus Expansion & Cleansing
* **Unified Transliteration Corpus**: Merged AI4Bharat's `Aksharantar` word-level dataset with aligned word-pairs from the public `sk-community/romanized_hindi` sentence-level dataset (~1.78M sentences).
* **Word Alignment & Filtering**: Extracted word pairs via 1-to-1 parallel sentence splitting, filtering out any mismatched lengths.
* **Length Capping Filter**: Enforced a strict character length filter ($\le 25$ characters) for all pairs, purging run-on outliers and ensuring 100% stable VRAM allocations.
* **Texting Normalization & Oversampling**: Enforced casual spelling standards (e.g., casing, nasalization, and texting contractions like `"achha"`, `"raha"`) and oversampled the curated conversational lexicon (`unique_word_pairs.json`) **10x** in the training split.
* **Final Corpus Size**: **1,403,617 unique, high-signal word pairs** (Train: 1,274,545, Val: 70,181, Test: 70,181).

### 2. Architectural Enhancements
* **Attention Padding Masking**: Integrated sequence-length comparison masks in PyTorch training. Scoring `<pad>` tokens as `-1e9` forces attention weights over dynamic pads to be mathematically zero, aligning training behavior with single-word inference.
* **Bucketed Batching**: Replaced standard random batching with a custom length-sorted `BucketedBatchSampler` using a high-throughput GPU batch size of **1024**. This packs similar sequence lengths together, reducing padding overhead to virtually zero and maintaining ONNX graph compatibility without PyTorch `PackedSequence` tracing failures.
* **Convergence**: Trained on the single NVIDIA RTX 5070 Ti for **15 epochs** with Cross-Entropy loss, reducing average Train Loss from **0.6864** to **0.1640**.

### 3. ONNX Inference Compatibility
* The attention padding mask is active only during PyTorch training and compiles out (`mask=None`) during ONNX tracing. This maintains a **zero-change, backward-compatible signature**, allowing immediate drop-in replacement in the Rust Tauri application without altering backend bindings.

## Example Output & Positional Stability

The structural fixes completely resolved the positional alignment issue where the first character of short words was dropped (e.g. `है` $\rightarrow$ `ai`, `क्या` $\rightarrow$ `yaa`, `में` $\rightarrow$ `ein`).

| Devanagari Input | Hinglish Output (Previous Model) | Hinglish Output (New Model) | Status |
|------------------|----------------------------------|-----------------------------|--------|
| नमस्ते | namaste | **namaste** | ✓ Stable |
| क्या हाल है? | yaa aal ai? | **kya haal hai?** | **✓ Fixed** |
| बाद में काम करेंगे | aad ein kam karenge | **baad mein kamm karenge** | **✓ Fixed** |
| मैं कल घर जाऊंगा | ain kal ghar jaaunga | **main kal ghar jaaunga** | **✓ Fixed** |
| तुम बहुत अच्छा काम करते हो | um bahut acha kam karte ho | **tum bahut achha kamm karate ho** | **✓ Fixed** |

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