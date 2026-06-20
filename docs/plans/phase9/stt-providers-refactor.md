# STT Provider Architecture Refactor — Implementation Plan

> **Phase:** 9
> **Status:** Final
> **Date:** 2026-06-20
> **Context:** Refactoring monolithic STT engine into a provider-based architecture (mirroring `LlmProvider`/`TtsProvider` patterns), using generic `Cloud` terminology for future extensibility.

---

## Objective

Refactor the current `SttEngine` trait + `switch-on-engine-type` actor into a proper provider-based architecture. Add **Google Cloud STT** (REST API) as a new cloud provider implementation with non-streaming support. The `Cloud` provider kind is generic — identified by a `provider` string field (e.g., `"google"`) — keeping the abstraction layer provider-agnostic for future cloud providers.

---

## Source of Truth

- **Current STT code:** `app/src-tauri/src/services/stt/` (mod.rs, actor.rs, qwen_onnx.rs, nemotron_onnx.rs)
- **Provider patterns to mirror:** `services/llm/providers/mod.rs` (`LlmProvider` trait), `services/tts/providers/mod.rs` (`TtsProvider` trait)
- **Settings:** `core/settings.rs` (`AsrSettings`, `LlmProviderConfig`, `TtsProviderConfig`, `SttProviderConfig`)
- **Events:** `core/events.rs` (`VoxEvent::TranscriptPartial`, `TranscriptFinal`)
- **New files:**
  - `services/stt/providers/mod.rs` — trait, `SttProviderKind` enum, factory
  - `services/stt/providers/embedded.rs` — `EmbeddedSttProvider`
  - `services/stt/providers/google_stt.rs` — `GoogleSttProvider`
- **Research docs:**
  - `docs/plans/phase9/google-stt-research.md`

---

## Assumptions

1. **Embedded engines stay sync** — Qwen-ONNX and Nemotron-ONNX remain on the sync worker thread. Only cloud providers need async bridging.
2. **Async pattern** — The `block_on()` pattern used by `OpenAiCompatProvider` is acceptable and preferred for async-wrapping within the sync STT worker.
3. **No API key encryption now** — Keys stored in cleartext in `settings.json` (matching current `LlmProviderConfig.api_key` / Realtime patterns). Encryption deferred as cross-cutting concern.
4. **Google STT uses REST API** — `reqwest` for HTTP, `jsonwebtoken` for JWT auth, REST endpoint `https://speech.googleapis.com/v1/speech:recognize`. OAuth2 token exchange from a self-signed JWT. No feature flags needed.
5. **`Cloud` terminology is generic** — `SttProviderKind::Cloud` with a `provider: String` field (e.g., `"google"`) identifies the cloud provider, keeping the trait and config enums provider-agnostic for future additions.
6. **Backwards compatibility** — `asr.model = "nvidia_nemotron"` continues to work; new `asr.provider` field defaults to `Embedded`.

---

## Phases

### Phase A: Provider Trait + Embedded Refactor

**Goal:** Create `SttProvider` trait and refactor existing ONNX engines into proper provider wrappers.

#### A1: Define `SttProvider` Trait

New file: `services/stt/providers/mod.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderKind {
    Embedded,
    Cloud,
}

pub trait SttProvider: Send {
    /// Transcribe full audio buffer (batch mode).
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String>;

    /// Process a streaming chunk. `is_final=true` signals utterance end.
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String>;

    /// Reset internal streaming state.
    fn reset_state(&self) -> anyhow::Result<()>;

    /// Health check.
    fn health_check(&self) -> bool;

    /// Provider kind identifier.
    fn kind(&self) -> SttProviderKind;
}
```

**Design decisions:**
- Audio format is implicit: all providers accept `&[f32]` (normalized f32 at 16kHz). Providers internally convert to their required format.
- `Send` (not `Sync`) — matching TTS pattern; the STT actor owns the provider exclusively.
- No `transcribe_batch()` on the trait; batch is handled by the caller stacking `transcribe()` calls.

#### A2: Embedded Provider (Single Wrapper)

New file: `services/stt/providers/embedded.rs`

- Wraps both Qwen-ONNX and Nemotron-ONNX behind `EmbeddedSttProvider`.
- Constructor takes `model_path` and `model_type` (`"nvidia_nemotron"` or `"qwen3_asr"`).
- **CRITICAL:** Move stride buffering logic (`processed_samples`, `stt_audio_buffer`) from `actor.rs` into this provider. The actor must no longer know about stride sizes, buffer management, or per-engine branching.
- `health_check()` checks model directory existence.
- `kind()` returns `SttProviderKind::Embedded`.

#### A3: Module Structure Update

```
services/stt/
  mod.rs               -- re-exports, legacy SttEngine trait removed
  providers/
    mod.rs              -- SttProvider trait, SttProviderKind enum, factory fn
    embedded.rs         -- EmbeddedSttProvider (Qwen + Nemotron)
    google_stt.rs       -- GoogleSttProvider (Phase D)
  actor.rs              -- refactored to dispatch on Box<dyn SttProvider>
  nemotron_onnx.rs      -- implementation detail, imported by embedded.rs
  qwen_onnx.rs          -- implementation detail, imported by embedded.rs
```

#### A4: Provider Factory Function

In `services/stt/providers/mod.rs`:

```rust
pub fn create_stt_provider(
    provider_config: &SttProviderConfig,
    model_path: &Path,
    runtime_handle: Option<tokio::runtime::Handle>,
) -> anyhow::Result<Box<dyn SttProvider>> {
    match &provider_config {
        SttProviderConfig::Embedded { model_type } => {
            Ok(Box::new(EmbeddedSttProvider::new(model_path, model_type)?))
        }
        SttProviderConfig::Cloud { provider, .. } if provider == "google" => {
            Ok(Box::new(GoogleSttProvider::new(provider_config)?))
        }
        SttProviderConfig::Cloud { provider, .. } => {
            anyhow::bail!("Unknown cloud STT provider: {}", provider)
        }
    }
}
```

**Validation gate:** `cargo build` passes, test clips still work end-to-end, no behavioral change for embedded STT.

---

### Phase B: Settings + IPC Wiring

**Goal:** Define `SttProviderConfig` enum, add to `AsrSettings`, wire up IPC and reload policies.

#### B1: Define `SttProviderConfig`

In `core/settings.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttProviderConfig {
    Embedded {
        #[serde(default = "default_stt_model")]
        model_type: String,
    },
    Cloud {
        provider: String,
        #[serde(default)]
        credentials_path: Option<String>,
        #[serde(default)]
        credentials_json: Option<String>,
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default = "default_region")]
        region: String,
        #[serde(default = "default_cloud_model")]
        model: String,
        #[serde(default = "default_language_bcp47")]
        language: String,
        #[serde(default)]
        endpoint: Option<String>,
    },
}
```

Default: `SttProviderConfig::Embedded { model_type: "nvidia_nemotron" }`.

#### B2: Update `AsrSettings`

```rust
pub struct AsrSettings {
    pub model: String,
    pub transliterate_enabled: bool,
    pub provider: SttProviderConfig,  // NEW
}
```

#### B3: Backwards Compatibility

Old settings without `provider` field: `serde(default)` on `AsrSettings` transparently provides `SttProviderConfig::Embedded { model_type: model }`. No migration code needed.

#### B4: IPC Commands

Add `check_stt_provider_health` Tauri command mirroring `check_llm_provider_health` / `check_tts_provider_health`. For Embedded: checks model dir. For cloud: performs lightweight network check.

#### B5: Reload Policy

Add `("asr", "provider") => SettingReloadPolicy::Restart`.

**Validation gate:** Settings load/save round-trips with new field. Old settings load correctly.

---

### Phase C: Google STT Provider

**Goal:** Implement `GoogleSttProvider` using REST API (reqwest + jsonwebtoken).

#### C1: Dependencies

No feature flags needed. All dependencies are unconditional:

| Crate | Purpose |
|-------|---------|
| `reqwest` | Already present — HTTP client for REST API calls |
| `jsonwebtoken` | Added — JWT creation for OAuth2 token exchange |
| `base64` | Already present — encoding utility |

#### C2: Implementation

File: `services/stt/providers/google_stt.rs`

- Uses REST API `https://speech.googleapis.com/v1/speech:recognize` (non-streaming `Recognize` call — batch mode).
- `transcribe_chunk()`: Not supported for REST-based Google STT (returns error or concatenates to buffer; final flush triggers API call).
- `transcribe()`: Sends full audio buffer to REST endpoint, returns transcript.
- Audio conversion: f32 → s16le PCM (same `f32_to_s16le_pcm` helper).

**Audio conversion (f32 → s16le):**

```rust
fn f32_to_s16le_pcm(audio: &[f32]) -> Vec<u8> {
    audio.iter()
        .flat_map(|&s| {
            let clamped = s.clamp(-1.0, 1.0);
            let sample = (clamped * i16::MAX as f32) as i16;
            sample.to_le_bytes()
        })
        .collect()
}
```

#### C3: Auth Flow

Uses a self-signed JWT exchanged for an OAuth2 access token:

1. **Cached access token** — reuse cached token if still valid.
2. **`credentials_json`** — parse inline JSON, extract `private_key` and `client_email`, create JWT, exchange for access token.
3. **`credentials_path`** — read file, same JWT exchange flow.
4. **`GOOGLE_APPLICATION_CREDENTIALS`** env var — fallback if no explicit credentials provided.

JWT claims:
```json
{
  "iss": "<client_email>",
  "scope": "https://www.googleapis.com/auth/cloud-platform",
  "aud": "https://oauth2.googleapis.com/token",
  "exp": <now + 3600>,
  "iat": <now>
}
```

Token exchange POST to `https://oauth2.googleapis.com/token` with `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion=<jwt>`.

#### C4: Recognition Config

```json
{
  "config": {
    "encoding": "LINEAR16",
    "sampleRateHertz": 16000,
    "languageCode": "<language>",
    "model": "<model>",
    "enableAutomaticPunctuation": true
  },
  "audio": {
    "content": "<base64-encoded-s16le-audio>"
  }
}
```

**Validation gate:** Compiles without feature flags; successful transcription with valid credentials; graceful error handling with invalid creds.

---

### Phase D: Frontend Settings UI

**Goal:** Add STT provider selection UI matching LLM provider interaction card pattern.

#### D1: TypeScript Types

In `store/settingsStore.ts`:

```typescript
export type SttProviderKind = "embedded" | "cloud";

export interface SttProviderConfig {
  kind: SttProviderKind;
  model_type?: string;
  provider?: string;
  credentials_path?: string | null;
  credentials_json?: string | null;
  project_id?: string | null;
  region?: string;
  model?: string;
  language?: string;
  endpoint?: string | null;
}
```

#### D2: Provider Selector in ModelsCard (ASR tab)

1. **Provider selector** button group: `Embedded` | `Cloud`
2. **Embedded** — existing model cards (Qwen3-ASR, Nemotron-3.5)
3. **Cloud (Google)** — provider dropdown (with `"google"` selected), credentials file picker / inline JSON textarea, project ID, region, model dropdown, language, health check

#### D3: Draft Handler

Extend `updateDraft("asr", "provider", ...)`.

#### D4: Model Catalog

Add cloud STT model metadata to `get_asr_metadata()` for reference display.

---

### Phase E: Actor / Worker Refactoring

**Goal:** Restructure `actor.rs` to dispatch on `Box<dyn SttProvider>`.

#### E1: Updated `spawn_stt_worker` Signature

```rust
pub fn spawn_stt_worker(
    app: AppHandle,
    rx: Receiver<SttCommand>,
    provider: Box<dyn SttProvider>,  // pre-constructed
    pipeline_event_tx: Option<Sender<VoxEvent>>,
    cancel_flag: Arc<AtomicBool>,
    is_loaded: Arc<AtomicBool>,
    engine_shutdown: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String>
```

Worker no longer takes `model_path` and `engine_type`. Provider is constructed by caller (pipeline).

#### E2: Remove Per-Engine Branching

- Eliminate the entire `match engine_type { "nvidia_nemotron" => ..., _ => ... }` pattern.
- Remove `init_engine()` function entirely.
- Worker loop simplifies to calling `provider.transcribe_chunk()` regardless of provider kind.

#### E3: Constructor Pattern (Mirrors TTS)

Like `spawn_tts_worker` takes `Box<dyn TtsProvider>`, STT worker takes a pre-constructed provider. Pipeline constructs it:

```rust
fn create_stt_provider_from_settings(settings: &VoxSettings) -> Result<Box<dyn SttProvider>> {
    let provider_config = &settings.asr.provider;
    let model_path = resolve_model_path(&settings.asr.model);
    create_stt_provider(provider_config, &model_path, None)
}
```

#### E4: Pipeline Wiring

In `launch_engine()`: construct provider → pass to `spawn_stt_worker`.

**Validation gate:** `cargo build` passes, test clips work for all embedded models, turn detection and queue-coalescing remain correct.

---

### Phase F: Pipeline Integration

**Goal:** Wire provider construction into pipeline startup and warmup paths.

#### F1: Provider Construction in `launch_engine`

Read `settings.asr.provider`, resolve model path, call factory, pass to worker.

#### F2: Warmup / Lazy Loading

For Embedded: eager construction (as before). For cloud: construction is cheap — health check can be deferred.

#### F3: Error Handling

- Embedded init failure: emit `EVENT_MODEL_FAILED`, return error.
- Cloud connect failure: `health_check()` returns false; at runtime, `transcribe()` returns error, logged, worker continues.
- Auth expiry (Google): re-authenticate on next call (token refresh on 401).

#### F4: Provider Switching

`SttProviderConfig` change triggers `Restart` policy → `stop_engine()` + `launch_engine()` cycle.

---

### Phase G: Testing and Edge Cases

| Test | Embedded | Google STT |
|------|----------|------------|
| test_clip (batch transcribe) | ✅ | ✅* |
| Partial streaming (VAD) | ✅ | ❌ (REST, no streaming) |
| Final with empty audio | ✅ | ✅ |
| ResetStream mid-utterance | ✅ | ✅ |
| Turn switching | ✅ | ✅ |
| Cancel during transcription | ✅ | ✅ |
| Health check (online) | ✅ | ✅ |
| Health check (offline) | N/A | ✅ |
| Invalid credentials | N/A | ✅ |
| Server reconnect | N/A | ✅ |

\* Requires valid Google Cloud credentials

---

## Dependency Summary

| Crate | Phase | Purpose |
|-------|-------|---------|
| `reqwest` | C | Already in Cargo.toml — REST API calls |
| `jsonwebtoken` | C | Added — JWT creation for OAuth2 token exchange |
| `base64` | C | Already in Cargo.toml — audio encoding |

No feature flags needed. Google STT is always compiled in.

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Async in sync thread blocks worker | Medium | High | `block_on()` with 150ms timeout (proven in `OpenAiCompatProvider`) |
| Google 5-min stream limit | Low | Medium | Not applicable (REST `Recognize` is non-streaming, single request) |
| API key leak in settings.json | High | High | Stored cleartext like existing `api_key` fields; encryption deferred |
| Qwen vs Nemotron state moved wrong | Low | High | Move stride+buffer fully into `EmbeddedSttProvider` |
| Credentials token expiry during session | Low | Low | Re-authenticate on 401 response — transparent retry |

---

## Implementation Order

```
Phase A: Provider trait + embedded refactor (trait, wrapper, factory)
    ↓
Phase B: Settings + IPC (config enum, settings, comm commands)
    ↓
Phase C: Google STT provider (REST, reqwest + jsonwebtoken)
    ↓
Phase D: Frontend UI (types, selector, config forms)
    ↓
Phase E: Actor refactoring (worker dispatch, remove branching)
    ↓
Phase F: Pipeline integration (wiring, error handling)
    ↓
Phase G: Testing (matrix verification)
```

**Dependency constraints:**
- A → B → E → F (must be sequential)
- A → C (need trait, can parallel with B)
- B → D (need types)
- C → G (need working provider)
- E → F (must happen before pipeline wiring)

---

## Out of Scope

- API key encryption at rest
- V2 Google STT API with Recognizer resources
- Streaming speaker diarization
- Dynamic batch pricing for Google STT
- Client-side rate limiting
- Per-provider model download/manifest
- WebSocket-based STT providers
- Real-time streaming for cloud STT (REST-based batch only)
