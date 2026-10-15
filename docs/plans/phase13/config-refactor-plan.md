# Comprehensive Configuration Architecture, Model Provenance & Multi-Server Refactoring Plan

## 1. Executive Summary & Core Requirements

This plan comprehensively refactors the Vox configuration and model runtime system under **Zero Backward Compatibility (ZBC)**. It solves the parameter provenance defect, decouples runtime selection from per-model parameter storage, establishes multiple named custom servers for LLM and TTS, decouples realtime configuration into its own service domain, moves memory settings to `settings.jsonc` while keeping `agent.jsonc` thin for future MCP/skills, and creates an exhaustive checklist across every touched surface in the codebase.

**Zero New IPC Commands Needed:** All mutations, category switches, custom server updates, and parameter updates route through existing commands (`update_setting`, `get_settings`, `list_favorite_models`, `toggle_model_favorite`).

---

## 2. High-Level Design (HLD): Domain Boundaries & Provenance Model

### 2.1 The Parameter Provenance Problem
In the legacy codebase, global parameters like `context_window`, `temperature`, `threads`, `voice_id`, and `speed` were stored as top-level fields in `settings.jsonc`. 
- **The Defect:** When a user configured `context_window = 32768` on a cloud model (`meta/llama-3.1-8b-instruct`), and then switched to an embedded model (`qwen_3_5_0_8b`), the context window either crashed the embedded model or got overwritten by the embedded model's defaults. Switching back destroyed the user's tuned cloud parameters.
- **The Same Defect in TTS:** In TTS, `speed` and `voice_id` were stored at the root of `tts`. Switching from Kokoro (speed 1.15, voice "heart") to EdgeTTS (speed 1.0, voice "en-US-AriaNeural") overwrote each other.

### 2.2 The Solution: The `models` Table in Turso SQLite (`vox.db`)
We deprecate and drop `favorite_models` and introduce the `models` table.

#### Sparse On-Demand Storage Rule
Remote catalogs contain thousands of models (e.g. OpenRouter has 1000+). We **do not** pre-populate the database. 
A model becomes a row in the `models` table **strictly on-demand** when:
1. The user **activates** it (`last_active_at` is set).
2. The user **favorites** it (`is_favorite = 1`).
3. The user **mutates any parameter** from its catalog default (e.g. adjusts context window, temperature, threads, or speed).

If a model is untouched, it requires zero rows in the database and falls back to catalog defaults.

#### SQL Table Schema (Schema Version 14)
```sql
CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,                       -- e.g. "meta/llama-3.1-8b-instruct", "qwen_3_5_0_8b", "kokoro"
    model_type TEXT NOT NULL,                  -- "stt" | "llm" | "tts" | "realtime"
    category TEXT NOT NULL,                    -- "embedded" | "server" | "cloud"
    provider TEXT,                             -- "nvidia", "openrouter", server ID ("ollama", "home-gpu"), or NULL
    parameters TEXT NOT NULL DEFAULT '{}',     -- JSON metadata of per-model parameters (speed, context_window, threads, etc.)
    is_favorite INTEGER NOT NULL DEFAULT 0,    -- 0 or 1 (replaces favorite_models table)
    last_active_at INTEGER,                    -- Unix timestamp ms
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_models_type_cat ON models (model_type, category);
CREATE INDEX IF NOT EXISTS idx_models_fav ON models (is_favorite);
CREATE INDEX IF NOT EXISTS idx_models_last_active ON models (last_active_at DESC);
```

### 2.3 The 3-Way File Storage Matrix

| Storage Target | POSIX Mode | Domain Responsibilities | Contained Data Groups |
| :--- | :--- | :--- | :--- |
| **`config/settings.jsonc`** | `0644` (Public) | Hardware, VAD, UI theme, memory policies, and **minimal active & last-selected model pointers** | `appearance`, `vad`, `interaction`, `dictation`, `system`, `working_memory`, `personal_memory`, `stt` (active, last_selected, transliterate), `llm` (active, last_selected), `tts` (active, last_selected, active_voice_id), `realtime` (active, active_voice_id). |
| **`config/providers.jsonc`** | `0600` (Restricted) | Custom server inventory, cloud provider endpoints, and private API credentials | `llm.servers` (list of named custom servers), `llm.cloud_keys`, `stt.cloud` (stub), `tts.servers` (list of named custom servers), `realtime.api_keys`. |
| **`config/agent.jsonc`** | `0644` (Public) | Thin cognitive persona prompts (reserved for future MCP tools & agent skills) | `persona` (`modular_prompt`, `realtime_prompt`). |
| **Database (`vox.db`)** | `0700` | Relational, searchable, and append-heavy state | `models` (per-model parameters & favorites), `voices`, `sessions`, `turns`, `personal_memory` facts and vector embeddings. |

---

## 3. Configuration JSON Schemas: Pristine Target Layout

### 3.1 `config/settings.jsonc`
```jsonc
{
  "appearance": {
    "theme": "light",
    "accent_seed": "#6c2c52"
  },
  "vad": {
    "threshold": 0.72,
    "ptt_noise_gate": 0.04,
    "vad_backend": "earshot",
    "silence_duration_ms": 400,
    "speech_onset_ms": 120,
    "max_speech_duration_s": 30
  },
  "interaction": {
    "mode": "passive",
    "pipeline_mode": "modular"
  },
  "dictation": {
    "enabled": true,
    "interaction_mode": "ptt",
    "hotkey": "Ctrl+Alt+V",
    "output_mode": "tray",
    "silence_auto_stop_ms": 1200
  },
  "system": {
    "setup_completed": true
  },
  "working_memory": {
    "private_mode": true,
    "auto_compaction": false,
    "max_context_share": 0.4,
    "web_search_enabled": true
  },
  "personal_memory": {
    "context_retrieval_enabled": false,
    "pipeline_processing_enabled": true,
    "top_k_facts": 15,
    "semantic_similarity_cutoff": 0.4,
    "consolidation_cadence": "manual",
    "consolidation_time": "02:00",
    "suggestion_policy": "manual_review"
  },
  "stt": {
    "active": {
      "category": "embedded",
      "provider": null,
      "model": "nvidia_nemotron"
    },
    "last_selected": {
      "embedded": "nvidia_nemotron"
    },
    "transliterate_enabled": false
  },
  "llm": {
    "active": {
      "category": "server",
      "provider": "ollama",
      "model": "gemma4:12b"
    },
    "last_selected": {
      "embedded": "qwen_3_5_0_8b",
      "server": "gemma4:12b",
      "cloud": "meta/llama-3.1-8b-instruct"
    }
  },
  "tts": {
    "active": {
      "category": "embedded",
      "provider": null,
      "model": "kokoro"
    },
    "last_selected": {
      "embedded": "kokoro",
      "server": "chatterbox-gpu",
      "cloud": "edge_tts"
    },
    "active_voice_id": "heart"
  },
  "realtime": {
    "active": {
      "category": "cloud",
      "provider": "gemini_live",
      "model": "gemini-3.1-flash-live-preview"
    },
    "active_voice_id": "Aoede"
  }
}
```

---

### 3.2 `config/providers.jsonc`
```jsonc
{
  "llm": {
    "servers": [
      {
        "id": "ollama",
        "name": "Local Ollama",
        "url": "http://127.0.0.1:11434",
        "model": "gemma4:12b",
        "protocol": "ollama",
        "api_key": null
      },
      {
        "id": "home-gpu",
        "name": "Home RTX 4090",
        "url": "http://192.168.1.50:8000/v1",
        "model": "meta-llama/Llama-3.1-8B-Instruct",
        "protocol": "openai",
        "api_key": "vllm-secret-token"
      }
    ],
    "cloud_keys": {
      "nvidia": "",
      "openrouter": "",
      "groq": ""
    }
  },
  "stt": {
    // Cloud STT stub — coming soon.
  },
  "tts": {
    "servers": [
      {
        "id": "chatterbox-gpu",
        "name": "Chatterbox GPU Server",
        "url": "http://100.67.98.126:8000",
        "remote_path": "/opt/chatterbox",
        "api_key": null
      }
    ]
  },
  "realtime": {
    "api_keys": {
      "gemini_live": "",
      "deepgram_voice_agent": "",
      "openai_realtime": "",
      "elevenlabs_convai": ""
    }
  }
}
```

---

### 3.3 `config/agent.jsonc`
```jsonc
{
  "persona": {
    "modular_prompt": "You are Vox, a quick-witted, casual voice assistant like Jarvis or Friday.\nYou speak aloud through text-to-speech. Keep every response to 1 or 2 concise, natural sentences.\nBe direct, conversational, and sharp with an easygoing warmth.\nNever use markdown, bullet points, asterisks, numbered lists, XML tags, or speaker prefixes.",
    "realtime_prompt": "<persona>\nYou're Vox — always listening, never hovering. You talk like someone who's been trusted with the keys to the house: calm, capable, and not afraid to say what you think. You read the room. You know when to jump in, when to stay quiet, and when a well-placed one-liner will land.\n</persona>\n\n<core_rules>\n- Speak the user's language. Detect it, mirror it, never question it.\n- Hindi always gets Devanagari. No Romanized Hindi. Ever.\n- Hinglish is fine — it's how people actually talk. Match it naturally.\n</core_rules>\n\n<voice_rules>\n- Everything's spoken aloud. Make it flow. Short sentences. Breathe.\n- No lists. No bullets. No notation. Just conversation that moves.\n- Be warm like a friend who knows their stuff, not a manual that read one.\n</voice_rules>\n\n<edge_rules>\n- A dry joke is a superpower. Use it. But never at the cost of clarity.\n- If you don't know, say so. If you need more context, ask.\n- Silence is fine. You don't need to fill every gap.\n</edge_rules>\n\n<memory_context>\n- If [Compacted History Summary] is present as Message 1, it provides a chronological narrative summary of earlier turns in this session.\n- If <user_profile> is present, use it for personal context.\n- The <memory_manifest> shows total stored records per collection in database.\n</memory_context>"
  }
}
```

---

## 4. How Custom Servers & Provenance Switching Function

### 4.1 Custom Server Lifecycle
1. **Creation:** User adds a server in UI -> `update_setting("llm", "servers", [ ... ])`. Backend validates, appends to `providers.jsonc:llm.servers`.
2. **Selection:** User selects `"home-gpu"`. 
   `update_setting("llm", "active", { "category": "server", "provider": "home-gpu", "model": "..." })`.
   `update_setting("llm", "last_selected", { "server": "..." })`.
3. **Execution:** LLM service matches `active.provider == "home-gpu"` against `providers.llm.servers`, gets URL & API key, queries `models` table for per-model parameters, and connects.

### 4.2 Seamless Category Switching with Full Provenance
1. **User is on Cloud LLM (`meta/llama-3.1-8b-instruct`), tunes context_window to 32768:**
   - Parameter update routes to `models` table row for `meta/llama-3.1-8b-instruct`:
     `parameters: {"context_window": 32768, "temperature": 0.85}`.
2. **User switches to Embedded tab in UI:**
   - Frontend reads `llm.last_selected.embedded` -> `"qwen_3_5_0_8b"`.
   - Backend activates `"qwen_3_5_0_8b"` and queries its parameters from `models` table (`{"threads": 4, "n_gpu_layers": 28}`).
   - Qwen executes with its 4 threads and 28 layers. The 32k context window does not touch Qwen.
3. **User switches back to Cloud tab in UI:**
   - Frontend reads `llm.last_selected.cloud` -> `"meta/llama-3.1-8b-instruct"`.
   - Backend activates Llama and loads its saved parameters from `models` table (`{"context_window": 32768, "temperature": 0.85}`).
   - **Zero loss. 100% provenance.**

### 4.3 Same Provenance for TTS
- When user switches between Kokoro (embedded) and EdgeTTS (cloud):
  - Kokoro's speed (1.15) and voice ("heart") are preserved in `models` table for Kokoro.
  - EdgeTTS's speed (1.0) and voice ("en-US-AriaNeural") are preserved in `models` table for EdgeTTS.
  - `active_voice_id` in `settings.jsonc` reflects the currently playing voice.

---

## 5. IPC Architecture: Zero New Commands

All operations route through existing, strongly-typed IPC commands:
1. `get_settings()`: Returns `BootState` with `settings: VoxSettings`.
2. `update_setting(domain, key, value)`:
   - Category / active selection: `update_setting("llm", "active", { category, provider, model })`
   - Active voice ID: `update_setting("tts", "active_voice_id", "heart")`
   - Custom server list: `update_setting("llm", "servers", servers_array)`
   - Model parameters: `update_setting("model_parameters", model_id, params_json)`
3. `reset_settings()`: Restores system defaults.
4. `list_favorite_models()`: Queries `SELECT id FROM models WHERE is_favorite = 1`.
5. `toggle_model_favorite(model_id)`: Atomically toggles `is_favorite` on `models` table.

---

## 6. Comprehensive Implementation Checklist by File

### Phase 1: Specifications Alignment
- [x] [docs/specs/storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)
  - [x] Update Section 3.1 & 5.2: 3-way schema table matching Section 3 of this plan.
  - [x] Add Section 5.4: `models` table sparse provenance invariants.
  - [x] Document `ActiveSelection` and `LastSelectedModels` schema contracts.
  - [x] Document `llm.servers` and `tts.servers` custom server array contracts.
  - [x] Purge legacy voice ID aliases and obsolete backward-compatibility text.
- [x] [docs/specs/db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)
  - [x] Add Section 5.13: `models` table schema definition (Schema v14).
  - [x] Document deprecation and retirement of `favorite_models` table.
  - [x] Update Section 3 ER diagram to link models.
- [x] [docs/specs/ipc-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)
  - [x] Update Section 4.6 (Settings Domain): document `update_setting` routing for `model_parameters`, `servers`, and `active` objects.
  - [x] Update `list_favorite_models` and `toggle_model_favorite` behavioral contracts.

### Phase 2: Database Layer (`src-tauri/src/persistence/`)
- [x] [app/src-tauri/src/persistence/schema.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/schema.rs)
  - [x] Increment `SCHEMA_VERSION` from 13 to 14.
  - [x] Add `CREATE TABLE IF NOT EXISTS models` and indexes `idx_models_type_cat`, `idx_models_fav`, `idx_models_last_active`.
  - [x] In migration v14: copy any existing rows from `favorite_models` into `models (id, model_type, category, is_favorite, created_at, updated_at)` and `DROP TABLE favorite_models`.
- [x] [app/src-tauri/src/persistence/models.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/models.rs) [NEW]
  - [x] Implement `ModelRecord` struct.
  - [x] Implement `upsert_model_active(conn, id, model_type, category, provider)`.
  - [x] Implement `get_model(conn, id) -> Result<Option<ModelRecord>>`.
  - [x] Implement `get_model_parameters(conn, id) -> Result<serde_json::Value>`.
  - [x] Implement `update_model_parameters(conn, id, model_type, category, provider, params: &serde_json::Value) -> Result<()>`.
  - [x] Implement `toggle_model_favorite(conn, id, model_type, category, provider) -> Result<bool>`.
  - [x] Implement `list_favorite_models(conn) -> Result<Vec<String>>`.
- [x] [app/src-tauri/src/persistence/mod.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/mod.rs)
  - [x] Add `pub mod models;` and re-export public helpers.

### Phase 3: Domain Service Configs (`src-tauri/src/services/`)
- [ ] [app/src-tauri/src/services/realtime/config.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/config.rs) [NEW]
  - [ ] Implement `RealtimeSettings`, `RealtimeProvidersConfig`.
  - [ ] Top-down struct hierarchy.
- [ ] [app/src-tauri/src/services/realtime/mod.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/mod.rs)
  - [ ] Re-export from `services::realtime::config`.
  - [ ] Remove circular dependency on `crate::config::RealtimeProviderKind`.
- [ ] [app/src-tauri/src/services/realtime/actor.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/actor.rs)
  - [ ] Update `create_realtime_provider`: resolve API key from `providers.jsonc`, `model` from `settings.realtime.active.model`, `voice` from `settings.realtime.active_voice_id`, and parameters from `models` table.
- [ ] [app/src-tauri/src/services/llm/config.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/config.rs)
  - [ ] Re-architect with top-down hierarchy: `LlmSettings`, `LlmProvidersConfig`, `LlmCustomServer`.
  - [ ] Implement `active_model(&self) -> &str`.
  - [ ] Remove `LlmRemoteConfig` combining server and cloud.
- [ ] [app/src-tauri/src/services/llm/factory.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/factory.rs)
  - [ ] Update provider instantiation to accept resolved server/cloud endpoint and model parameters from DB.
- [ ] [app/src-tauri/src/services/harness/chassis.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/chassis.rs)
  - [ ] Resolve generation parameters (temperature, context_window, max_tokens, reasoning) dynamically from DB model record for active model.
- [ ] [app/src-tauri/src/services/tts/config.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/config.rs)
  - [ ] Re-architect with top-down hierarchy: `TtsSettings`, `TtsProvidersConfig`, `TtsCustomServer`.
  - [ ] Remove empty structs `TtsSupertonicConfig {}`, `TtsKokoroConfig {}`.
  - [ ] Remove offline engine configs (`chatterbox`, `zipvoice`) from providers file.
- [ ] [app/src-tauri/src/services/tts/factory.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/factory.rs)
  - [ ] Update `create_tts_provider` to read `speed` and `voice_id` from model parameters and `active_voice_id`.
  - [ ] Update `caps_for_id` to inspect model id directly.
- [ ] [app/src-tauri/src/services/tts/preview.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/preview.rs)
  - [ ] Update speed resolution.
- [ ] [app/src-tauri/src/services/stt/config.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/config.rs)
  - [ ] Re-architect with top-down hierarchy: `SttSettings`.
  - [ ] Stub cloud config.
- [ ] [app/src-tauri/src/services/stt/factory.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/factory.rs)
  - [ ] Ensure clean instantiation from `SttSettings`.

### Phase 4: App Configuration Layer (`src-tauri/src/config/`)
- [ ] [app/src-tauri/src/config/settings.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/config/settings.rs)
  - [ ] Define `ActiveSelection` and `LastSelectedModels`.
  - [ ] Move `WorkingMemorySettings` and `PersonalMemorySettings` into `settings.rs`.
  - [ ] Refactor `VoxSettings` to hold clean sub-structs for all 6 domains.
  - [ ] Remove `normalize_retired_voice_ids` and legacy shims.
- [ ] [app/src-tauri/src/config/files.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/config/files.rs)
  - [ ] Align `SettingsConfigFile`, `ProvidersConfigFile`, and `AgentConfigFile` with Section 3 schemas.
  - [ ] Implement clean `to_settings_config`, `to_providers_config`, `to_agent_config` and their `merge_*` counterparts.
- [ ] [app/src-tauri/src/config/persistence.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/config/persistence.rs)
  - [ ] Update atomic read/write logic for the 3 clean JSONC files.
  - [ ] Remove legacy alias recovery and section patching hacks.
- [ ] [app/src-tauri/src/config/mutation.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/config/mutation.rs)
  - [ ] Update `apply_setting_mutation` for each domain:
    - [ ] `appearance`, `vad`, `interaction`, `dictation`, `system`
    - [ ] `working_memory`, `personal_memory`
    - [ ] `stt`, `llm`, `tts`, `realtime` (`active`, `last_selected`, `active_voice_id`)
    - [ ] `model_parameters` mutation branch routing to `persistence::models::update_model_parameters`
    - [ ] `servers` mutation branch updating `providers.jsonc`
  - [ ] Remove obsolete legacy enum fallbacks.
- [ ] [app/src-tauri/src/config/dispatch.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/config/dispatch.rs)
  - [ ] Update side effect handlers for `active_voice_id` and active selections.
- [ ] [app/src-tauri/src/config/mod.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/config/mod.rs)
  - [ ] Clean up reload policies (`get_setting_reload_policy`) for the new schema paths.

### Phase 5: IPC Commands & Wire Contracts (`src-tauri/src/ipc/`)
- [ ] [app/src-tauri/src/ipc/settings.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings.rs)
  - [ ] Update `update_setting` handler to support `model_parameters` domain.
- [ ] [app/src-tauri/src/ipc/catalog.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/catalog.rs)
  - [ ] Rewrite `list_favorite_models` to query `models` table (`WHERE is_favorite = 1`).
  - [ ] Rewrite `toggle_model_favorite` to update/upsert `models` table.
- [ ] [app/src-tauri/src/ipc/voices.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/voices.rs)
  - [ ] Update active voice assignment to `settings.tts.active_voice_id`.

### Phase 6: Core State & Monitoring
- [ ] [app/src-tauri/src/core/state.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs)
  - [ ] Verify `resolve_base_prompt` reads from `settings.persona`.
- [ ] [app/src-tauri/src/monitoring/snapshots.rs](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/snapshots.rs)
  - [ ] Update active model snapshot inspection to read `active.category` and `active.provider`.

### Phase 7: Backend Integration Tests & Compile Gate
- [ ] [app/src-tauri/tests/settings_persistence_test.rs](file:///home/addy/projects/apps/vox/app/src-tauri/tests/settings_persistence_test.rs)
  - [ ] Update all subtests to test new schemas, round-trips, and mutations.
- [ ] [app/src-tauri/tests/model_eviction_test.rs](file:///home/addy/projects/apps/vox/app/src-tauri/tests/model_eviction_test.rs)
  - [ ] Update active provider assertions.
- [ ] [app/src-tauri/benches/pipeline_bench.rs](file:///home/addy/projects/apps/vox/app/src-tauri/benches/pipeline_bench.rs)
  - [ ] Update active provider setup.
- [ ] **Dual-Target Compile Gate**:
  - [ ] Run `cd app/src-tauri && ./scripts/verify-targets.sh` (must be 0 errors, 0 warnings on desktop + Android).
- [ ] **Full Test Suite Gate**:
  - [ ] Run `cargo nextest run --release --test-threads=1 --no-fail-fast`.

### Phase 8: Frontend State & UI Components (`app/src/`)
- [ ] [app/src/store/settingsStore.ts](file:///home/addy/projects/apps/vox/app/src/store/settingsStore.ts)
  - [ ] Define `ActiveSelection`, `LastSelectedModels`, `LlmCustomServer`, `TtsCustomServer`.
  - [ ] Update `LlmSettings`, `TtsSettings`, `SttSettings`, `RealtimeSettings`.
  - [ ] Move `working_memory` and `personal_memory` under `settings`.
- [ ] [app/src/data/settingsCopy.ts](file:///home/addy/projects/apps/vox/app/src/data/settingsCopy.ts)
  - [ ] Update `SETTINGS_SCOPE_KEYS`.
- [ ] [app/src/shared/components/settings/models/RemoteServerSetup.tsx](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/models/RemoteServerSetup.tsx)
  - [ ] Add form to create new custom server (`name`, `url`, optional `api_key`/`remote_path`).
- [ ] [app/src/shared/components/settings/interaction/LlmConfigDesk.tsx](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/interaction/LlmConfigDesk.tsx)
  - [ ] Wire server list from `providers.llm.servers`.
  - [ ] Wire category switcher using `active.category` and `last_selected`.
  - [ ] Connect `+ Custom` button to custom server creation form.
- [ ] [app/src/shared/components/settings/models/TtsModelWorkspace.tsx](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/models/TtsModelWorkspace.tsx)
  - [ ] Wire category switcher (`embedded | server | cloud`) using `active.category` and `last_selected`.
  - [ ] Bind active voice to `active_voice_id` and speed to model parameters.
- [ ] [app/src/shared/components/settings/models/AsrWorkspace.tsx](file:///home/addy/projects/apps/vox/app/src/shared/components/settings/models/AsrWorkspace.tsx)
  - [ ] Read active model from `stt.active.model`.
- [ ] [app/src/test/invariants.test.ts](file:///home/addy/projects/apps/vox/app/src/test/invariants.test.ts)
  - [ ] Update frontend invariant tests to match new backend type contracts.
- [ ] **Frontend Vitest Gate**:
  - [ ] Run `cd app && pnpm test`.

### Phase 9: End-to-End Verification
- [ ] Wipe `~/.vox/config/*.jsonc`.
- [ ] Launch Vox, verify clean boot and file generation.
- [ ] Add custom server, select it, verify persistence in `providers.jsonc` and `settings.jsonc`.
- [ ] Tune parameters on cloud model, switch to embedded, switch back, verify parameter provenance.

---

## 7. Verification Gate Commands

1. **Compilation Gate:**
   ```bash
   cd app/src-tauri && ./scripts/verify-targets.sh
   ```
2. **Backend Tests:**
   ```bash
   cd app/src-tauri && RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
3. **Frontend Tests:**
   ```bash
   cd app && pnpm test
   ```
