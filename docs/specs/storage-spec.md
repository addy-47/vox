# Storage, Filesystem & Configuration Specification (v1)

---

- **Status:** Approved Target Spec (SSOT for Filesystem Layout, Configuration Decomposition & Runtime Paths)
- **Policy:** Zero Backward Compatibility (ZBC) with an automated one-way boot migration.
- **Scope:** Governs the entire on-disk footprint of Vox: directory hierarchy, runtime sockets, configuration decomposition, schema definitions, reload policies, and file format standards.

---

## 1. Executive Summary & Architectural Invariants

Historically, Vox accumulated all persistent state, caches, temporary sockets, crash dumps, and multi-gigabyte models directly in the unpartitioned root of `~/.vox/`. Furthermore, `settings.json` was an unscalable monolith that coupled OS audio device settings, external cloud API secrets, and LLM agent personas into a single rigid struct.

This specification establishes three fundamental architectural boundaries:
1. **Strict Lifecycle-Tiered Filesystem:** All on-disk files are segregated into six discrete functional tiers: `config/`, `data/`, `models/`, `cache/`, `diagnostics/`, and `run/`.
2. **Decomposed Configuration (3-Way Split):** Configuration is split by volatility, reload policy, and security into `settings.jsonc` (Host & Hardware), `providers.jsonc` (Infrastructure & Secrets), and `agent.jsonc` (Cognitive Policy & Tools).
3. **Format & Parsing Standards:** Lenient JSONC (JSON with comments and trailing commas) for human/machine configuration, and JSONL (JSON Lines) for streamable event history.

---

## 2. Mandatory Architectural Invariants

### Invariant 1: The Disposable Cache Invariant (`rm -rf ~/.vox/cache`)
The `cache/` directory contains strictly reproducible or disposable data. If a user or automated cleaner purges `cache/`, Vox **must** boot cleanly, reconstitute temporary assets, and suffer **zero** loss of user data, transcripts, or configuration. Dictation transcripts and dialogue turns are durable state and **must never** reside in `cache/`.

### Invariant 2: Heavy Storage Decoupling (`VOX_MODELS_DIR`)
Multi-gigabyte static weights (`models/`) are decoupled from user configuration and databases. Vox must support the `VOX_MODELS_DIR` environment variable to allow mounting or relocating models to high-speed NVMe drives or shared network volumes independently of `$VOX_HOME`.

### Invariant 3: Ephemeral Runtime Isolation
Unix domain sockets (`vox.sock`) and process locks (`vox.pid`) are transient OS primitives. They **must never** be stored in persistent user directories that could be mounted on network shares (NFS/CIFS) or synchronized via cloud drives. On Linux, `$XDG_RUNTIME_DIR` (`/run/user/<uid>/vox/`) is prioritized, with fallback to `~/.vox/run/` with strict `0700` POSIX permissions.

### Invariant 4: Zero Path Leaks & Single Source of Truth
Direct filesystem traversal via `dirs::home_dir().join(".vox")` anywhere outside `utils/paths.rs` is **strictly prohibited**. All subsystems must query `VoxPaths` accessors (`paths::db()`, `paths::models()`, `paths::config()`, etc.).

### Invariant 5: Plaintext Secret Isolation
Provider API keys and tokens must reside strictly in `providers.jsonc` (or system keychain), protected with `0600` file permissions. Diagnostic logs, telemetry, and UI settings exports must **never** serialize or log the contents of `providers.jsonc`.

---

## 3. Tiered Filesystem Layout (`~/.vox/`)

```
~/.vox/                              # Base Root ($VOX_HOME, default ~/.vox)
├── config/                          # Tier 1: User & Agent Configuration (Human/UI edited)
│   ├── settings.jsonc               # Host hardware, audio I/O, VAD, UI theme, active engine selectors
│   ├── providers.jsonc              # Model providers, base URLs, credentials & API keys (chmod 0600)
│   └── agent.jsonc                  # Cognitive persona, prompt, temperature, memory search, tools/MCP
│
├── data/                            # Tier 2: Durable User State (Backup & Sync Target)
│   ├── db/
│   │   ├── vox.db                   # Primary Turso/libsql DB (facts, graph, sessions, compactions)
│   │   ├── vox.db-wal
│   │   └── vox.db-log
│   ├── history/
│   │   └── dictation.jsonl          # Durable dictation transcript history (JSON Lines)
│   └── voices/                      # User custom/cloned voice assets
│       └── <voice_id>/
│           ├── source.wav           # Pristine audio reference sample
│           └── baked/               # Precomputed acoustic profiles & speaker embeddings
│
├── models/                          # Tier 3: Static Model Weights (Overridable via $VOX_MODELS_DIR)
│   ├── manifests/
│   │   └── models_manifest.json     # SSOT canonical manifest of verified local & remote models
│   ├── stt/                         # Streaming transducer & ASR ONNX models (Nemotron 3.5)
│   ├── tts/                         # Synthesis engines (Kokoro, ZipVoice, Chatterbox voice packs)
│   ├── vad/                         # Voice activity detectors (Earshot, Silero)
│   ├── embedding/                   # Dense vector embeddings (MiniLM ONNX)
│   ├── translit/                    # Script transliteration ONNX models
│   ├── classifier/                  # Intent & domain router models
│   └── llm/                         # Local language models (Qwen GGUFs)
│
├── cache/                           # Tier 4: Disposable Scratch (Safe to wipe anytime)
│   ├── catalog/                     # Remote provider model catalog cache
│   ├── updates/                     # Update check cache (app_manifest.json)
│   ├── temp/                        # Audio resampling scratch & transient IPC buffers
│   └── vox.png                      # Materialized desktop icon for Linux notify-send
│
├── diagnostics/                     # Tier 5: Observability & Diagnostics
│   ├── logs/
│   │   └── vox.log.YYYY-MM-DD       # Daily rolling application logs
│   └── crashes/
│       └── crash_<timestamp>.log    # Panic backtraces, minidumps & core reports
│
└── run/                             # Tier 6: Ephemeral Runtime State (chmod 0700)
    ├── vox.sock                     # Unix domain socket (Wayland hotkey trigger IPC)
    ├── vox.pid                      # Single-instance process lock (flock)
    └── bin/
        └── vox-trigger              # Shell script for OS keybind manager invoking vox.sock
```

---

## 4. Configuration Architecture (The 3-Way Split)

### 4.1 Schema Decomposition & Ownership

| Configuration File | Primary Domain | Volatility | Reload Policy | Security Tier |
| :--- | :--- | :--- | :--- | :--- |
| **`config/settings.jsonc`** | Hardware, Audio I/O, VAD, Theme, Active Engine Wiring | Low | **Restart / Heavy Reconstruct** | Public / Diagnostics-Safe |
| **`config/providers.jsonc`** | Cloud/Server Endpoints, Model IDs, API Keys | Medium | **Instant Hot-Reload** | **Restricted (`0600`)** |
| **`config/agent.jsonc`** | Cognitive Persona, Prompts, Tools, MCP, Memory Policy | High | **Turn-Hot (Per Turn / Session)** | Public / User Profile |

---

### 4.2 `config/settings.jsonc` (Hardware & Runtime Wiring)

Governs physical devices, input/output hardware, audio DSP sensitivity, and selected engine backends.

```jsonc
{
  "$schema": "./schemas/settings.schema.json",
  "appearance": {
    "theme": "dark",
    "accent_seed": "hyper-violet"
  },
  "audio": {
    "output_mode": "speaker",         // "speaker" | "headset"
    "input_device": null              // null = system default, or device name string
  },
  "vad": {
    "backend": "silero_vad",          // "silero_vad" | "earshot" | "ten_vad"
    "threshold": 0.5,
    "ptt_noise_gate": 0.005,
    "silence_duration_ms": 400,
    "speech_onset_ms": 32,
    "max_speech_duration_s": 30
  },
  "pipeline": {
    "mode": "modular",                // "modular" | "realtime"
    "stt_engine": "embedded",         // "embedded" | "cloud"
    "tts_engine": "kokoro",           // "kokoro" | "zipvoice" | "chatterbox"
    "llm_engine": "server"            // "embedded" | "server" | "cloud"
  },
  "dictation": {
    "enabled": true,
    "hotkey": "Ctrl+Alt+V",
    "interaction_mode": "ptt",        // "ptt" | "passive"
    "output_mode": "paste",           // "paste" | "clipboard" | "tray"
    "silence_auto_stop_ms": 800
  },
  "system": {
    "setup_completed": true
  }
}
```

---

### 4.3 `config/providers.jsonc` (Infrastructure & Credentials)

Governs network connections to AI model hosts, API tokens, and provider mappings.

```jsonc
{
  "$schema": "./schemas/providers.schema.json",
  "providers": {
    "ollama": {
      "kind": "ollama",
      "base_url": "http://100.67.98.126:11435",
      "api_key": null,
      "default_model": "qwen3.5:9b"
    },
    "nvidia": {
      "kind": "openai_compatible",
      "base_url": "https://integrate.api.nvidia.com/v1",
      "api_key": "nvapi-secret-key-goes-here",
      "default_model": "meta/llama-3.1-8b-instruct"
    },
    "google_cloud_stt": {
      "kind": "google_speech",
      "project_id": "vox-production",
      "credentials_path": null,
      "language": "en-US",
      "model": "chirp_3"
    },
    "deepgram": {
      "kind": "deepgram",
      "api_key": "dg-secret-key-goes-here",
      "default_model": "nova-2",
      "default_voice": "aura-asteria-en"
    }
  }
}
```

---

### 4.4 `config/agent.jsonc` (Cognitive Policy & Phase 13 Harness)

Governs prompt engineering, LLM reasoning parameters, context share, memory retrieval filters, and tool/MCP integrations.

```jsonc
{
  "$schema": "./schemas/agent.schema.json",
  "persona": {
    "name": "Vox Assistant",
    "system_prompt": "You are Vox, a high-performance voice-native assistant.",
    "temperature": 0.6,
    "max_output_tokens": 120,
    "context_window": 8192,
    "reasoning_enabled": false
  },
  "memory": {
    "working": {
      "auto_compaction": true,
      "max_context_share": 0.4,
      "private_mode": false,
      "web_search_enabled": false
    },
    "personal": {
      "context_retrieval_enabled": true,
      "top_k_facts": 5,
      "semantic_similarity_cutoff": 0.65,
      "consolidation_cadence": "Daily",
      "consolidation_time": "03:00"
    }
  },
  "capabilities": {
    "tools_enabled": true,
    "mcp_servers": [
      {
        "name": "codebase-memory",
        "transport": "stdio",
        "command": "codebase-memory-mcp",
        "args": []
      }
    ],
    "skills_paths": [
      "~/.vox/skills"
    ]
  }
}
```

---

## 5. Reload Policies & Concurrency Invariants

When any configuration file is updated via IPC or external file modification:

| Configuration Target | Reload Trigger | Action Taken by Runtime | Latency Budget |
| :--- | :--- | :--- | :--- |
| **`settings.jsonc` (Audio/VAD)** | File watch or IPC mutate | Drops active CPAL stream, rebinds OS audio device/DSP thread. | $\le 100\text{ ms}$ |
| **`settings.jsonc` (Engine Switch)** | File watch or IPC mutate | Unloads active ONNX model weights, loads requested model into RAM/VRAM. | $\le 1.5\text{ s}$ |
| **`providers.jsonc` (Credentials/URLs)** | File watch or IPC mutate | Updates HTTP client connection pool; no pipeline interruption. | **$0\text{ ms}$ (Lockless)** |
| **`agent.jsonc` (Prompt/Temperature)** | File watch or IPC mutate | Hot-swaps `Arc<AgentConfig>` applied on the immediate next LLM turn. | **$0\text{ ms}$ (Atomic pointer swap)** |
| **`agent.jsonc` (MCP / Tools)** | File watch or IPC mutate | Connects/disconnects MCP stdio processes in background; active turn uses current snapshot. | Non-blocking async |

---

## 6. Migration & Boot Protocol (`paths::migrate_legacy_layout`)

During startup, before any database engine or audio stream is mounted, Vox executes an automated one-way migration:

```
[Start Boot]
    │
    ▼
Is `~/.vox/vox.db` present in root?
    ├── YES ──► Create `data/db/` ──► Move `vox.db*` to `data/db/` ──► Create symlink `~/.vox/vox.db` -> `data/db/vox.db`
    └── NO  ──► Proceed
    │
    ▼
Is `~/.vox/settings.json` present in root?
    ├── YES ──► Create `config/`
    │           ├── Extract provider keys/urls to `config/providers.jsonc`
    │           ├── Extract system prompt/memory to `config/agent.jsonc`
    │           └── Migrate remaining fields to `config/settings.jsonc`
    │           └── Archive legacy `settings.json` -> `cache/settings.json.bak`
    └── NO  ──► Proceed
    │
    ▼
Is `~/.vox/cache/dictation_history.jsonl` present?
    ├── YES ──► Create `data/history/` ──► Move to `data/history/dictation.jsonl`
    └── NO  ──► Proceed
    │
    ▼
Is `~/.vox/crash_reports/` present?
    ├── YES ──► Create `diagnostics/crashes/` ──► Move crash logs ──► Remove `crash_reports/`
    └── NO  ──► Proceed
    │
    ▼
Clean up abandoned files (e.g. `~/.vox/lib/libonnxruntime.so`)
    │
    ▼
[Proceed to VoxDb::open(&paths::db()) and AppState initialization]
```

---

## 7. Security, Permissions & Access Matrix

| Path | POSIX Mode | Rationale |
| :--- | :--- | :--- |
| `~/.vox/run/` | `0700` (`drwx------`) | Contains Unix domain socket and PID files; strictly restricted to current user. |
| `~/.vox/config/providers.jsonc` | `0600` (`-rw-------`) | Contains private API keys and tokens. |
| `~/.vox/config/*.jsonc` | `0644` (`-rw-r--r--`) | User preferences readable by desktop utility scripts. |
| `~/.vox/data/db/` | `0700` (`drwx------`) | Local knowledge graph and personal memories. |
