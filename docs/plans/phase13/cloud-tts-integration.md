# Cloud TTS Integration — Azure Speech + Google Cloud TTS (Phase 13 Feature Document)

> Status: ✅ COMPLETE — implemented, reviewed, gated green, committed (`a092a64`)
> and pushed to `origin/ft/cloud-tts`. Testing handoff in §6 is for the next agent.

## 1. How it started

Vox's TTS layer had six providers (`supertonic`, `kokoro`, `chatterbox`,
`chatterbox_remote`, `edge_tts`, `zipvoice`), all local except Edge (scraped,
zero-config) and Chatterbox-Remote (self-hosted GPU). Users with real Azure /
Google cloud accounts had no first-class path. A mini-PRD proposed adding
**Azure Speech** and **Google Cloud Text-to-Speech** as optional cloud
providers, reusing the existing `TtsProvider` abstraction and audio worker,
with no custom OAuth flow and no large credential-configuration UI.

Open questions at kickoff (from the PRD §8): which trait contract, which Rust
credential libraries, whether Azure resource discovery via CLI is reliable,
whether the Google Rust client supports `StreamingSynthesize`, which audio
encoding minimizes conversion, what concurrency limits fit clause-based Azure
synthesis, and which provider to build first.

## 2. What we researched (2026-10-14, live docs, two parallel research passes)

### 2.1 Google Cloud TTS

- **Chirp 3: HD is TTS, not STT-only.** The shared "Chirp" name caused the
  confusion (STT has `chirp` recognizer models). TTS families per
  `docs.cloud.google.com/text-to-speech/docs/list-voices-and-types`: Chirp 3:
  HD (GA), Studio multispeaker (Experimental), Studio single-speaker (GA),
  Neural2 / WaveNet / Standard (GA), plus Gemini-TTS models. **Streaming works
  only with Chirp 3: HD** (and Gemini-TTS via its own API).
- **Endpoint:** `texttospeech.googleapis.com` (regional `us-`/`eu-` prefixes
  for residency). REST `POST /v1/text:synthesize` (5000 B/req, full-blob
  response, no incremental audio). gRPC
  `google.cloud.texttospeech.v1.TextToSpeech/StreamingSynthesize` =
  bidirectional streaming, **Preview** status.
- **Half-close caveat (load-bearing):** the API buffers text and only starts
  synthesizing after client half-close. There is no per-token incremental
  audio today. Correct granularity = **one stream per clause**, half-close at
  clause end; first-audio wins come from short clauses.
- **Auth:** OAuth scope `https://www.googleapis.com/auth/cloud-platform`;
  ADC auto-discovery (`gcloud auth application-default login`), quota project
  via `gcloud auth application-default set-quota-project` or override field.
- **Quotas:** 100 concurrent streaming sessions, 200 Chirp-3 req/min, batch
  1000 req/min. Pricing: Standard/WaveNet ~4M chars/mo free → $4/1M; Neural2
  1M free → $16/1M; Chirp-3-HD 1M free → $30/1M; Studio $160/1M.
- **Rust:** maintained official crate `google-cloud-texttospeech-v1`
  (v1.16.0) exposes `streaming_synthesize()` + `synthesize_speech()` +
  `list_voices()` with default ADC — no hand-rolled `tonic`/`prost` needed.
  Risk: default features pull `aws-lc-rs` (C/asm); must disable defaults,
  use `ring`, and pass the dual-target gate or fall back to REST-per-clause.
- **Wire format:** streaming `PCM` @24000 Hz = headerless s16le, zero decode
  cost into the 24 kHz mono f32 pipeline. `voices:list`
  (`GET /v1/voices?languageCode=`) returns `name` / `languageCodes` /
  `ssmlGender` / `naturalSampleRateHertz` for live sync.
- **Sync filter:** `en-US-Chirp3-HD-{Kore,Charon,Aoede,Puck,Leda,Fenrir}`
  (all 24000 Hz; same styles exist for en-GB/en-AU/en-IN).

### 2.2 Azure AI Speech

- **Endpoint (stable, no 2025–26 break):**
  `POST https://{region}.tts.speech.microsoft.com/cognitiveservices/v1`
  (SSML body; headers `Content-Type: application/ssml+xml`,
  `X-Microsoft-OutputFormat`, `User-Agent`, auth via
  `Ocp-Apim-Subscription-Key` or `Authorization: Bearer`). Custom domain:
  `https://{name}.cognitiveservices.azure.com/cognitiveservices/v1`.
- **One REST request = one fixed SSML document.** No in-request incremental
  input (true text-streaming is SDK-WebSocket-only, `wss://…/websocket/v2`,
  no SSML). Response body **is** incrementally consumable
  ("played as it's transferred") — clause-per-request + chunked read gives
  low first-audio latency without the SDK.
- **Auth:** key (primary, 90% case) / STS 10-min host-scoped JWT / Entra
  `aad#resource#token` (needs custom subdomain + `Cognitive Services Speech
  User` role + resource ID). **No automatic endpoint discovery** — `az login`
  yields subscription/tenant only; the user must supply region or endpoint
  (+ resource ID for Entra).
- **Quotas:** S0 30 TPS default (→1000 on request), F0 20 tx/60 s; per-request
  10-min audio cap; 429s are usually per-voice capacity, not quota → backoff,
  `Retry-After`, voice-native region. Pricing: F0 0.5M neural chars/mo free
  (Neural only); S0 Neural ~$15–16/1M, HD ~$22/1M.
- **Wire format:** `raw-24khz-16bit-mono-pcm` = headerless s16le, zero decode
  cost. `voices/list`
  (`GET {custom-endpoint}/tts/cognitiveservices/voices/list`) returns
  `ShortName` / `Locale` / `Gender` / `VoiceType` / `Status` / string
  `SampleRateHertz` / `WordsPerMinute`. Filter: `Status==GA &&
  VoiceType==Neural && rate>=24000 && en*`, unioned with documented DragonHD
  GA voices (`en-US-{Ava,Andrew,Aria,Jenny,Emma2,Andrew2}:DragonHDLatestNeural`,
  colon syntax, may not appear in list endpoint).

## 3. Plan (agreed, single ticket)

- **Contract reuse, no new abstraction:** both providers implement existing
  `TtsProvider::synthesize_chunk(text, ctx)` on the dedicated OS TTS worker
  thread. Google = one bidi stream per call (config → text → half-close →
  PCM chunks). Azure = one blocking SSML POST per call with chunked PCM read.
  No trait change, no session API, no `actor.rs` change.
- **Files touched (~10 modified + 3 new):** `Cargo.toml`; `tts/config.rs`
  (ids `azure_tts`/`google_tts`, `TtsAzureConfig`, `TtsGoogleConfig`,
  `TtsProviderConfig` variants, `TtsVoiceSource::Cloud`); `tts/factory.rs`;
  `tts/mod.rs` (endpoint/timeout constants); `tts/providers/mod.rs`;
  `tts/voice.rs` (live-sync fetch+sync per provider); `core/error.rs`
  (`TtsError::{Auth,Quota,RateLimited,VoiceNotFound,Cancelled}`);
  `core/defaults.rs`; `ipc/voices.rs`; `ipc/catalog.rs`. New:
  `tts/providers/azure_tts.rs`, `tts/providers/google_tts.rs`,
  `tts/auth.rs` (token cache + redaction helper, not a trait).
- **New crates:** `google-cloud-texttospeech-v1` (default-features off,
  `ring` TLS) and `azure_identity` (Entra fallback only; `cfg`-gatable).
  Both must pass `verify-targets.sh` (desktop + `arm64-v8a`) or be reverted
  (Google → REST-per-clause; Entra → key-only on Android).
- **IPC/schema:** new provider ids + sub-configs in `TtsSettings`;
  `providers.jsonc` (0600) holds `tts.azure_tts.{endpoint,key_ref}` and
  `tts.google_tts.{project_override}`; keys never cross IPC; reused
  `check_provider_health` with classified errors; voices table reused with
  `provider_id` = own id (no migration).
- **Events/harness:** none new, zero harness effect (TTS downstream of final
  LLM text; clause boundaries unchanged; hot path untouched).
- **Specs:** update `provider-model-catalog-spec.md`, `ipc-spec.md`,
  `storage-spec.md`, `db-spec.md` (rows note). `events`/`harness`/
  `dictation`/`ownership` explicitly unchanged. (Spec-first rule: the grill
  + this plan are the user-approved divergence record; spec edits land in
  §5.4 before this ticket closes.)

## 4. Decisions log (grill)

- Q1: both providers, one ticket — agreed.
- Q2: "Chirp 3 is STT" challenge — refuted by live docs; PRD correct.
- Q3: ADC auto + key-first Azure + Entra fallback — approved.
- Q4: evidence-based transport (official crate streaming + REST-per-clause),
  latency/quality trade-off resolved via half-close finding — approved.
- Q5: live sync only, no hardcoded catalog — approved.
- Q6: new crates allowed if justified + gated — approved.
- Q7/Q8/Q9: schema shape, typed errors, no events/harness — approved.

## 5. Implementation log (updated as work lands)

### 5.1 Step 1 — config + error + defaults + auth skeleton ✅ DONE (2026-10-15)

Files changed: `core/defaults.rs` (+3 consts), `core/error.rs`
(`TtsError::{Auth,Quota,RateLimited,VoiceNotFound,Cancelled}`),
`services/tts/config.rs` (`TtsVoiceSource::Cloud`, `AzureTts`/`GoogleTts`
ids + sub-configs + `TtsProviderConfig` variants + `to_provider_config` +
tests), `services/tts/auth.rs` (NEW: `CachedToken`, `normalize_azure_endpoint`,
`classify_http_status`, `redact_secret`), `services/tts/providers/azure_tts.rs`
+ `google_tts.rs` (NEW scaffolds implementing `TtsProvider`, logic in Steps
2/3), `services/tts/providers/mod.rs`, `services/tts/mod.rs` (endpoint/timeout
constants), `services/tts/factory.rs` (caps + constructor arms + tests),
`services/tts/voice.rs` (`cloud_sync=true` for `Cloud`), `config/mutation.rs`
(sub-config + provider arms), `ipc/catalog.rs` (active-voice arms),
`services/health.rs` (reachability arms via voices endpoints).
Thread context: pure data types (no thread); network I/O reserved for the TTS
OS worker thread in Steps 2/3. Contract changes: `TtsSettings` schema +2
sub-configs (persisted; no IPC command signature changed yet).
Gates: `cargo check --lib` clean; clippy clean on touched files (3 pre-existing
warnings elsewhere left untouched); `cargo test --lib services::tts::` 4/5 —
new config/factory tests pass, `edge_tts_live_synthesis` fails on no-network
sandbox (environmental, pre-existing).

### 5.2 Step 2 — Azure provider + voice sync + wiring ✅ DONE (2026-10-15)

Full `AzureTtsProvider` in `services/tts/providers/azure_tts.rs`: blocking SSML
POST per clause (`raw-24khz-16bit-mono-pcm`, chunked s16le→f32 ingest),
deterministic auth resolution at construction (providers.jsonc key →
`AZURE_SPEECH_KEY` env → Entra via `azure_identity::DeveloperToolsCredential`
with 8-min cache + one 401 refresh), one bounded 429 backoff honoring
`Retry-After` (cap 10 s), typed errors, RTF telemetry, cancellable sleeps.
Runs on the TTS OS worker thread (blocking client); Entra fetch bridged via a
1-thread private Tokio runtime (Edge pattern). `voice.rs`:
`fetch/sync_azure_tts_voices_to_db` (GA + Neural + `en*` + ≥24 kHz filter);
`ipc/voices.rs` syncs Azure when active with key. `TtsAzureConfig.key` +
`resource_id` flow through `TtsProvidersConfig` (providers.jsonc 0600),
`to_providers_config`/`merge_providers_config`/recovery/`mutation.rs`/reload
policy.
New crates: `azure_identity` + `azure_core` (Entra only; key path is
`reqwest`-only).
Contract changes: `TtsProviderConfig::AzureTts` carries key + resource_id;
no new IPC command; `sync_remote_voices` extended, not duplicated.

### 5.3 Step 3 — Google provider + voice sync + wiring ✅ DONE (2026-10-15)

Full `GoogleTtsProvider` in `services/tts/providers/google_tts.rs` on
`google-cloud-texttospeech-v1` **1.16.0** (needed for `streaming_synthesize`;
required bumping `jiff` 0.2.24 → 0.2.38 within its compatible range after a
lockfile conflict with `google-cloud-auth`). One bidi stream per
`synthesize_chunk` (config → text → half-close → PCM chunks), `speaking_rate`
server-side, quota project via `RequestOptions`, >5000-byte clauses rejected
with actionable error, RPC codes → typed `TtsError`. Bridged via 2-thread
private Tokio runtime on the TTS OS worker thread (Edge pattern).
`voice.rs`: `fetch/sync_google_tts_voices_to_db` (`voices:list` per
en-{US,GB,AU,IN}, `Chirp3-HD` filter); syncs when active with ADC.
New crates: `google-cloud-texttospeech-v1` + `google-cloud-gax` (options +
error types). `PcmIngest` struct bundles chunk state per arg-bundling rule.

### 5.4 Step 4 — spec updates + gates ✅ DONE (2026-10-15)
`providers-spec.md` §6.1 + new §6.6; `db-spec.md` `provider_id` set + live-sync
note; `ipc-spec.md` caps ids + `sync_remote_voices` contract; `storage-spec.md`
RULE-STORAGE-05 cloud-TTS rule. `events`/`harness`/`dictation`/`ownership`
unchanged as planned.
Gates: `verify-targets.sh` GREEN both targets, 0 warnings/errors
(`GATE_EXIT:0`; includes `arm64-v8a` — all 4 new crates survive Android).
Gate history: first run failed on 3 pre-existing rust-1.96 clippy errors in
untouched files (`ipc/audio.rs`, `monitoring/resource_scope.rs`, `lib.rs`);
fixed mechanically and disclosed as out-of-task gate repair. Unit tests 4/5
(`edge_tts_live_synthesis` needs network — environmental).

### 5.5 Adversarial review disposition ✅ (2026-10-15)

Independent review found 5🔴 + 5🟠 + 8🟡. Applied: cloud preview misroute →
loud error arms in `preview.rs`; Azure key in `update_setting` logs/reasons →
`redact_setting_value` (also covers LLM/STT secrets; gives `redact_secret()`
a caller); transaction rollback via `in_voice_transaction` on all four voice
syncs (mine + pre-existing Edge/manifest, same-file hygiene); health arms now
authenticated and success-only (no false-green on 401); unused
`AZURE_TTS_STS_PATH` deleted; Azure rate moved server-side into SSML
`prosody` (no client stretch clicks); voice-name SSML validation; explicit
`SsmlVoiceGender` match; locale-qualified Google display names; dropped-voice
filter logging; Google speed clamp tied to `SPEED_RANGE`.
Overruled with evidence: reviewer claimed Entra `aad#` format wrong — refuted
verbatim by live MS doc (2026-05-21, "include the `aad#` prefix"); format kept
with doc citation. `sync_remote_voices → Ok(0)` kept: frontend ignores the
value (fire-and-forget is the established contract; changing it needs a
frontend change, out of scope) — recorded as known limitation.

## 6. Testing handoff (for the test agent; testing is out of scope here)
**User prerequisites.** Google: `gcloud` installed, `gcloud init`, billed
project, Text-to-Speech API enabled, `gcloud auth application-default login`,
quota project set if asked (or project override field); optional
`GOOGLE_APPLICATION_CREDENTIALS`. Azure: subscription + Speech resource +
region/endpoint + key (primary), or `az login` + custom subdomain + Speech
role + resource ID (Entra fallback).

**Steps.** (1) `./scripts/verify-targets.sh` green both targets.
(2) `cargo nextest run --release` full suite. (3) `check_provider_health`
per provider → Connected; negatives (no ADC, bad key, wrong region, API
disabled, quota) → actionable error, secrets never logged. (4) Short-clause
synthesis per provider: first-audio latency, continuity, mid-synth cancel,
3-clause ordering; local TTS unaffected without cloud config. (5) Voice sync:
rows keyed per provider, GA/24 kHz filters hold, re-sync idempotent.
(6) Soak ~10 min: 429 backoff + token refresh without restart.
