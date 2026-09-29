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

Governs physical devices, input/output hardware, audio DSP sensitivity, appearance, and selected engine backends. Mode `0644`.
- **`appearance`**: UI theme and accent styling.
- **`audio`**: Output mode (`Speaker` | `Headset`) and audio input device selection.
- **`vad`**: Sensitivity threshold, PTT noise gate, silence cutoff duration, speech onset pre-roll, and max speech duration.
- **`interaction`**: Interaction mode (`Passive` | `PTT`) and pipeline mode (`modular` | `realtime`).
- **`dictation`**: Enablement, interaction mode, global hotkey, output mode (`paste` | `clipboard` | `tray`), and auto-stop silence duration.
- **`system`**: First-run setup completion state.
- **`stt` / `llm` / `tts` Wiring**: Active provider selectors and local compute options (thread pools, GPU layers, model references).

---

### 4.3 `config/providers.jsonc` (Infrastructure & Credentials, Mode 0600)

Governs network connections to AI model hosts, API tokens, and provider parameters per subsystem. Strictly restricted to POSIX mode `0600`.
- **`llm`**: Local server endpoints (`ollama`) and cloud provider configurations (`base_url`, `model`, `api_key`).
- **`stt`**: Cloud STT provider settings and credentials.
- **`tts`**: Provider configs for EdgeTTS, Chatterbox, remote Chatterbox worker, and ZipVoice synthesis.
- **`realtime`**: Active provider selector and service credentials (`gemini_live`, `openai_realtime`, `deepgram_voice_agent`, `elevenlabs_convai`).

---

### 4.4 `config/agent.jsonc` (Cognitive Policy & Harness)

Governs prompt engineering, LLM reasoning parameters, context share, and memory policies. Mode `0644`.
- **`persona`**: System prompt definitions (`modular_prompt`, `realtime_prompt`).
- **`working_memory`**: Privacy mode, automatic background compaction, context budget limit (`max_context_share`), and web search enablement.
- **`personal_memory`**: Context retrieval enablement, fact count budgets (`top_k_facts`), similarity cutoffs, and consolidation schedule.
- **`cognitive`**: LLM generation temperature, compaction temperature, output token ceiling, effective context window, and reasoning flag.

---

## 5. Reload Policies & Concurrency Invariants

When any configuration file is updated via IPC or external file modification:

| Configuration Target | Reload Trigger | Action Taken by Runtime | Latency Budget |
| :--- | :--- | :--- | :--- |
| **`settings.jsonc` (Audio/VAD)** | IPC mutate | Sends `VadCommand` to audio DSP worker thread live. | $\le 100\text{ ms}$ |
| **`settings.jsonc` (Engine Switch)** | IPC mutate | Rebuilds pipeline subsystem on restart or engine re-init. | Restart / Rebind |
| **`providers.jsonc` (Credentials/URLs)** | IPC mutate | Updates provider records; LLM cloud provider constructed on engine boot. | Restart required |
| **`agent.jsonc` (Prompt/Temperature)** | IPC mutate | Read live per LLM chassis turn or prompt builder (`Hot`). | **$0\text{ ms}$** |
| **`agent.jsonc` (Memory Policies)** | IPC mutate | Updates working/personal memory subsystem live via side-effects (`Hot`). | Immediate |

---

## 6. Migration & Boot Protocol (`paths::migrate_legacy_layout`)

During startup, before any database engine or audio stream is mounted, Vox executes an automated one-way migration:

```
[Start Boot]
    │
    ▼
Is `~/.vox/vox.db` present in root?
    ├── YES ──► Create `data/db/` ──► Move `vox.db*` to `data/db/` (Zero Backward Compatibility — no symlinks)
    └── NO  ──► Proceed
    │
    ▼
Sweep stale config debris
    └── Remove `config/*.tmp` and `config/*.corrupt.*`
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
