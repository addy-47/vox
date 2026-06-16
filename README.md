<div align="center">
  <img src="app/public/Vox.png" width="480" alt="Vox Ambient Intelligence Banner">
  <h3>The Ambient Intelligence Layer for the Native Edge</h3>
  <p align="center">
    <a href="#-key-features">Key Features</a> •
    <a href="#-technical-architecture">Architecture</a> •
    <a href="#-product-tour">Product Tour</a> •
    <a href="#-model-zoo">Model Zoo</a> •
    <a href="#-getting-started">Getting Started</a>
  </p>
  <p align="center">
    <img src="https://img.shields.io/badge/Latency-Sub--500ms-blueviolet?style=flat-square" alt="Latency">
    <img src="https://img.shields.io/badge/Privacy-100%25--Local--First-success?style=flat-square" alt="Privacy">
    <img src="https://img.shields.io/badge/OS-Windows_%7C_macOS_%7C_Linux-blue?style=flat-square" alt="OS Support">
    <img src="https://img.shields.io/badge/Engine-Rust_%2F_C%2B%2B-orange?style=flat-square" alt="Stack">
  </p>
</div>

---

## 🌌 The Vision

**Vox** is a real-time, local-first voice assistant designed to disappear into your desktop environment. Rather than forcing you to interact with a chat window, Vox operates as a persistent, ultra-low-latency ambient listener. It responds to voice activity instantly, streams output on the fly, and gets out of your way the second you stop speaking. 

Vox prioritizes conversational rhythm and tactile edge execution over cloud dependence, ensuring complete data privacy and full end-to-end voice interaction pipeline.

---

## ✨ Key Features

*   **Vox Live (Passive HUD)**: An ephemeral overlay that slides in automatically when speech is detected and auto-fades when you finish speaking.
*   **Push-To-Talk (PTT)**: Precise capture mechanism with real-time waveform visualization for higher-intent queries.
*   **Intelligent Barge-In**: Immediate flushing of audio buffers and LLM generation halts when you interrupt the model.
*   **Local Inference Zoo**: Leverages optimized ONNX and GGUF models running CPU-first inside a strictly enforced memory budget (~5.5GB).
*   **Multi-Platform Native Support**: Seamless native integration across Windows, macOS, and Linux, complete with a positioning HUD and click-through capability.

---

## 🏗️ Technical Architecture

Vox is built on a sequential, multi-threaded pipeline using Rust. The pipeline processes complete utterances end-to-end: audio capture → VAD → full STT transcription → LLM generation → chunked TTS synthesis → playback.



```mermaid

graph TD

    Mic[Audio Input Device] --> VAD[Ten VAD / Earshot]

    VAD --> Pipeline[Orchestration Pipeline]

    Pipeline --> STT[Nemotron-3.5 ASR]

    STT --> HUD[Vox Live HUD / UI]

    STT --> LLM[Llama 3.2 1B Instruct / Gemma 4]

    LLM --> TTS[Supertonic 3 TTS]

    TTS --> Speaker[Audio Output Device]

```



*   **VAD Engine**: Powered by Ten VAD (ONNX FP32, ~15ms/frame) or Earshot (native energy-based, ~1ms/frame). Both dispatchable via `VadBackend` enum.

*   **Speech-To-Text**: Nemotron-3.5 (INT8 ONNX, ~1,265 MB) — optimized chunked transcription with 8960-sample windows preserving context across chunks.

*   **LLM**: Llama-3.2-1B-Instruct (GGUF Q6_K) — CPU-first, 2.5–4.4 TPS, 100% stability across benchmark suite. Language detection routes Devanagari STT to Hindi prompts.

*   **TTS**: Supertonic 3 (sherpa-onnx native, INT8, 99M params) — sole TTS engine with 31 languages, 10 voices, and `<laugh>/<breath>/<sigh>` emotion tag support.


---

## 🖥️ Product Tour

### Ephemeral Interaction Layer
The passive HUD overlays transcriptions directly on your active workspace without disrupting focus. Enable click-through mode to pass events straight to background windows while you talk.

<div align="center">
  <table border="0">
    <tr>
      <td align="center" width="50%">
        <img src="app/public/tray.png" alt="Passive HUD" width="90%">
        <p><b>Passive HUD</b><br><em>Real-time overlay sliding in during active speech</em></p>
      </td>
      <td align="center" width="50%">
        <img src="app/public/tray-ptt.png" alt="Push-to-Talk" width="90%">
        <p><b>Push-To-Talk (PTT)</b><br><em>Manual trigger with visual waveform feedback</em></p>
      </td>
    </tr>
  </table>
</div>

### Control Center & Telemetry
Configure system behaviors, inspect live telemetry, select models, and view transcription history.

<div align="center">
  <table border="0">
    <tr>
      <td align="center" width="50%">
        <img src="app/public/home.png" alt="Control Center Active" width="95%">
        <p><b>Command Center Dashboard</b><br><em>State change visualization during voice capture</em></p>
      </td>
      <td align="center" width="50%">
        <img src="app/public/settings.png" alt="Model Settings" width="95%">
        <p><b>Model Manager</b><br><em>Dynamic hot-swapping for STT, LLM, and TTS configurations</em></p>
      </td>
    </tr>
    <tr>
      <td align="center" width="50%">
        <img src="app/public/monitoring.png" alt="Telemetry" width="95%">
        <p><b>Performance Telemetry</b><br><em>Real-time resource logs (CPU, Memory, and Latency)</em></p>
      </td>
      <td align="center" width="50%">
        <img src="app/public/history.png" alt="Session History" width="95%">
        <p><b>Session History</b><br><em>Review recent interactions, transcripts, and model evaluations</em></p>
      </td>
    </tr>
  </table>
</div>

---

## 🦁 Model Zoo

Vox manages models locally using manifest validation. On first startup, the app validates your local models folder and downloads missing dependencies dynamically:

| Engine | Model | Format | File Size | Memory (RSS) | Latency | Required |

| :--- | :--- | :--- | :--- | :--- | :--- | :--- |

| **VAD** | Ten VAD | ONNX FP32 | ~330 KB | ~50 MB | ~15ms/frame | Yes (Core) |

| **VAD** | Earshot | Native Rust | Zero | ~0 MB | ~1ms/frame | Optional |

| **Translit** | Vox Translit RNN | ONNX | ~16 MB | ~16 MB | ~5ms | Yes (Core) |

| **STT** | Nemotron-3.5 (Default) | ONNX INT8 | ~756 MB | ~1,265 MB | 0.02–0.35× RTF | Yes (Core) |

| **STT** | Qwen3-ASR (Legacy) | ONNX INT8 | ~986 MB | ~1,177 MB | 0.38–4.63× RTF | Optional |

| **LLM** | Llama 3.2 1B Instruct | GGUF Q6_K | ~1.02 GB | ~970 MB | 2.5–4.4 TPS | Optional |

| **LLM** | Gemma 4 2B Instruct | GGUF Q4_K_M | ~3.46 GB | ~4,092 MB | 9 TPS | Optional |

| **LLM** | MiniCPM5-1B | GGUF Q4_K_M | ~688 MB | ~654 MB | 6.5 TPS | Optional |

| **TTS** | Supertonic 3 | ONNX INT8 (7 files) | ~144 MB | ~21 MB | 0.79–1.50× RTF | Yes (Core) |


---

## 🚀 Getting Started

### Installation
You can install Vox directly on supported platforms using our bootstrap script:

```bash
curl -fsSL https://addy-47.github.io/vox/install.sh | bash
```

### Updates
To update your package manager repository and download the latest desktop client:

```bash
sudo apt update && sudo apt upgrade vox
```

---

<div align="center">
  <img src="app/public/logo-square.png" width="64" alt="Vox Square Logo">
  <br>
  <sub>Designed for privacy and immediate ambient intelligence.</sub>
</div>