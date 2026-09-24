# Phase 12 — Web Search Tool Plan (DRAFT)

> **Status: DRAFT — NOT APPROVED FOR EXECUTION.**
> Known issues remain; this document is intended for review by another agent before
> any implementation begins. Do not treat any batch below as authorized work.
>
> Target: `web_search` + `web_fetch` NonTerminal harness tools backed by the
> `kestrel-rs` crate (`kestrelsearch` lib), Modular pipeline only, keyless-only v1,
> gated by a single user-facing settings toggle.

---

## 0. Decisions already locked (pre-review)

| # | Decision | Resolution |
|---|----------|------------|
| 1 | Crate choice | `kestrel-rs = "=11.0.0"` (lib `kestrelsearch`), exact pin. Single crate for both search and fetch. |
| 2 | API keys | **None in v1.** All 9 `Engine` variants are keyless HTML providers. Brave/API-key providers deferred. |
| 3 | Pipeline domain | **`ToolDomain::Modular` only.** Realtime providers keep their own native retrieval (e.g. Gemini `googleSearchRetrieval`); no Realtime projection or dispatch work. |
| 4 | User gating | **One toggle: `web_search_enabled`** (default `false`). Gates *both* tools in the registry; the user never sees that two tools exist underneath. The earlier two-toggle proposal was wrong and is withdrawn. |
| 5 | Site allowlist/blocklist | Deferred to v2. SSRF guard (Batch 1) is the safety layer; list editor is convenience. |
| 6 | reqwest version | **Upgrade Vox `reqwest 0.12 → 0.13` first** (kestrel depends on 0.13; coexisting 0.12+0.13 means duplicate TLS stacks). Same feature set: `stream`, `rustls-tls`, `json`, `blocking`. |
| 7 | Version alignment | Vox app/Cargo version bumped to align with the crate line (`11.0.0`) per user instruction — see Batch 0 §Version. |
| 8 | Threshold authority | Model sets **only** `query` / `url` + required `spoken_filler`. Engines, region, time filter, result counts, char caps, timeouts, budgets are settings-owned (same precedent as `search_memory`). |

---

## 1. What kestrel-rs gives the harness (verified against docs.rs 11.0.0, 2026-09-23)

### 1.1 Search path — `web_search` tool

```rust
// kestrelsearch::search
pub async fn search_many(
    queries: &[String],
    options: &SearchOptions,
) -> Result<Vec<SearchResult>, KestrelError>
```
- Async Tokio — matches our runtime. `search_blocking` must never be used (would stall the async executor).
- `SearchMode` has exactly one variant: `Fanout` (compat shim — set it, don't read into it).

**`SearchOptions` (settings-owned, model never touches these):**

| Field | Type | Vox v1 value | Notes |
|---|---|---|---|
| `engines` | `Vec<Engine>` | `Duckduckgo, Bing, Mojeek` (initial subset) | Default fanout is 8 engines (all but `Yep`). Smaller subset = faster inside the 10s tool budget. Widen later from measured latencies. |
| `mode` | `SearchMode` | `Fanout` | Only variant. |
| `region` | `String` | `"us-en"` (default) | Locale for providers. |
| `time_filter` | `TimeFilter` | `Any` | Variants: `Any, D, W, M, Y` (day/week/month/year recency). Model does not get this in v1. |
| `max_concurrency` | `usize` | settings default (e.g. 4) | Provider parallelism. |
| `provider_quorum` | `Option<usize>` | `None` | **Legacy, ignored** by result-count fanout. Do not use. |
| `min_results` | `Option<usize>` | `Some(5)` | ⚠️ **Fanout stop threshold, NOT an output cap** (docs are explicit). Output size is controlled by how many results *we* format into the observation. |
| `search_budget` | `Option<Duration>` | `Some(7s)` | Discovery deadline **including retries**. `None` (default) disables deadlines — must not ship that way or a hung provider eats the 10s `TOOL_EXECUTION_TIMEOUT`. Quirk: budgets <15s enable up to two extra empty-query retries (+5s each, +2s backoff); budgets ≥15s use one attempt. 7s sits in the retry band — vet real latency in Batch 0. |

**`Engine` enum — all 9 keyless HTML providers:**
`Duckduckgo, Bing, Yahoo, Dogpile, Ecosia, Swisscows, Yep, Qwant, Mojeek`.

**`SearchResult` (what comes back):**
```rust
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
    pub content: Option<String>,       // extracted page text — MAY be None (see Open Issues)
    pub bm25_score: Option<f64>,
    pub engine: Option<Engine>,
    pub query: Option<String>,
    pub engine_rank: Option<usize>,
    pub sources: Vec<SourceOccurrence>, // cross-engine dedup provenance
}
```
Derives `Serialize`/`Deserialize`/`JsonSchema`.

**Observation format (plain text, matches `search_memory` precedent):**
```
Web search results for '<query>' (N results):
[1] <title> — <url>
    <snippet, truncated to ~500 chars>
[2] ...
```
Top-N (settings `max_results`, default 5) after dedup by URL. Empty → `Web search completed for '<query>'. No results found.`

### 1.2 Fetch path — `web_fetch` tool

```rust
// kestrelsearch::fetcher
pub async fn fetch_all(
    urls: &[String],
    options: &FetchOptions,
) -> Result<Vec<Option<String>>, KestrelError>
```
- Order-preserving; `None` per-slot = that page failed (no cross-item error propagation). Tool uses single-URL batches.
- "Each invocation owns independent capacity; reuse `KestrelClient` for a shared bound" — Vox holds one long-lived `KestrelClient` in `WebFetchService`.

**`FetchOptions` (the capping story — all settings-owned):**

| Field | Type | Vox v1 value | What it caps |
|---|---|---|---|
| `timeout` | `Duration` | ~7s | Per-fetch-batch deadline (under the 10s tool timeout). |
| `content_limit` | `usize` | settings default (e.g. 8000) | Retained extracted-text length. |
| `max_response_bytes` | `usize` | settings default (e.g. 512_000) | Stop downloading past N decoded bytes; extracts retained prefix only. |
| `max_concurrency` | `usize` | 1 (single-URL tool) | Download slots per batch. |
| `parse_concurrency` | `usize` | settings default | Parser slots; download slots hold backpressure while waiting. |
| `aggressive` | `bool` | `false` | Aggressive HTML cleanup. Standard preprocessing default; does not affect `text/plain` or `text/markdown`. |

Vox additionally truncates the returned text to the observation cap with a `[…truncated]` marker.

**Observation format:**
```
Fetched <final_url> (<title> if known>):
<extracted text, truncated to content_limit> […truncated]
```

### 1.3 SSRF layer (Vox-owned, NOT in kestrel)

Verified: kestrel-rs `transport` module is connection pools only — **no SSRF protection**. The guard runs *before* any URL reaches kestrel:

1. Scheme allow: `http` / `https` only.
2. DNS resolve host → reject loopback, private (RFC1918), link-local (incl. `169.254.169.254` cloud metadata), multicast, reserved ranges, IPv6 equivalents.
3. Redirect handling: follow manually, max 5 hops, **re-validate every hop** (reqwest's default policy does not).
4. Body size cap enforced by `FetchOptions.max_response_bytes`.
5. Violation → sanitized `is_error = true` observation (`"fetch blocked: destination not permitted"`); turn never aborts (existing NonTerminal failure contract).

### 1.4 Harness contract (both tools)

- `ToolFlow::NonTerminal` → `Thinking → Working` (`NonTerminalPhase`), `spoken_filler` required (Modular schema), observation appended to turn-local scratchpad, re-entrant loop re-entry, `TOOL_EXECUTION_TIMEOUT = 10s` + `CancellationToken`, `MAX_TOOL_ITERATIONS = 5` bound, persistence to `session_tool_calls`. All of this comes free from the existing NonTerminal path — zero harness-loop changes expected.
- `ToolDomain::Modular` → schemas never projected to Realtime providers.
- Errors (DNS fail, bot challenge, timeout, SSRF block) → `ToolResult { is_error: true }` with a sanitized one-liner the model can narrate. Never abort the turn.

---

## 2. Open issues (UNRESOLVED — for external agent review)

> These are the known issues referenced in the draft status. Nothing below is decided.

| # | Issue | Why it matters |
|---|-------|----------------|
| O-1 | **Does `search_many` populate `SearchResult.content`?** Docs don't say. If yes, `web_search` alone may answer most turns; if no, every useful turn needs an explicit `web_fetch` follow-up (two fillers / two Working cycles per turn). | Decides observation design, eval scenario design, and whether `web_fetch` should ever be auto-invoked by the harness (currently: no). Batch 0 measures this empirically. |
| O-2 | **`search_budget = 7s` sits inside the <15s retry band** (up to +10s of hidden retries). May defeat the 10s tool timeout or interact badly with it. Needs measured p90 fanout latency before the value is fixed. | Wrong value → intermittent tool timeouts presenting as flaky search to the user. |
| O-3 | **reqwest 0.12 → 0.13 upgrade risk.** ~10 files touch `reqwest::` (`services/llm/transport/*`, `catalog/*`, `health.rs`, `tts/*`, `setup/*`, incl. one `blocking::Client` user in `chatterbox_remote.rs`). If 0.13 changed APIs we use, fallout could be non-trivial; alternative is living with dual TLS stacks (rejected, but reversible). | Highest-blast-radius step in the plan and it lands first, before any web-tool value is delivered. |
| O-4 | **kestrel-rs dependency weight.** Unconditional non-feature-flagged deps include `clap` (CLI arg parsing in a library), `rmcp =3.4.0` (MCP server), full `opentelemetry` + `otlp` + `sdk` stack, `primp`/`primp-rustls` (its own HTTP transport alongside reqwest), `tempfile`, `home`, `toml_edit`. Only feature flag is `test-fixtures` (off by default). Binary-size and compile-time cost unmeasured. | Desktop-app binary bloat + OTel/rmcp code we never call. No mechanism to disable. If unacceptable → fallback candidates `web-search` (0.5.0) + `webfetch` (webtools-fetch), both behind the same trait boundary. |
| O-5 | **`~/.kestrelsearch/` dotfile writes.** `config` module docs: "Skill-installation state stored in `~/.kestrelsearch/config.toml`". Must confirm what triggers a write and that Vox's containment strategy prevents it (state belongs under `~/.vox`). Unverified whether writes are avoidable. | Stray dotfile in the user's home from a Tauri app is a packaging/cleanliness defect. |
| O-6 | **Crate volatility.** 47 versions in ~4 days (v6→v11 all in 2026-09-14…18), 430 total downloads, single maintainer (`rafaelpierre`), GitHub API returned no stats for the repo (possibly private/unreachable), docs coverage 47%. Exact pin `=11.0.0` mitigates drift but pins us to an unproven snapshot. | Exact pin means we own every upgrade decision; also means zero security-patch trickle-down without deliberate bumps. |
| O-7 | **Keyless scraping fragility.** All 9 engines are HTML-scraping providers (no APIs). They break when markup changes. No mitigation in v1 beyond multi-engine fanout (partial degradation) and the trait boundary (swap provider later). | Intermittent production degradation is an accepted v1 property — confirm acceptance explicitly. |
| O-8 | **No per-call approval flow exists** in Vox (and existing tools don't use one). `web_fetch` will fetch any model-chosen public URL automatically once the toggle is on. SSRF guard covers internal targets; it does not cover "fetching embarrassing/off-brand public pages". | Whether that's acceptable for v1, or a domain policy is actually required after all (revisit O-5/v2 site list). |
| O-9 | **Spec drift check.** `tools-spec.md` §7 catalog has no external-retrieval tool entries yet. Adding two tools is a spec addition (per §4.3 Non-Drift Hook, spec must be updated FIRST if behavior diverges). This plan assumes appending two catalog entries; exact wording not drafted. | Spec-first rule: no code until catalog entries are approved. |
| O-10 | **Version alignment ambiguity.** User instructed "update vox version to match the crate." Interpreted as: bump the Tauri app version to `11.0.0` to mirror the pinned crate line. If the intent was something else (e.g. a `kestrel`-aligned package metadata field), clarify before Batch 0. | Misreading this produces a wrong public version number on the app. |

---

## 3. Proposed changes — batched by shared blast radius

> Build expectation key (same as `implementation_plan.md`):
> 🟢 = green build throughout batch · 🟡 = red mid-batch, green on completion · ⛔ = blocked on dependency

---

### Batch 0 — reqwest upgrade, crate vetting, version alignment 🟡

**Blast radius**: `app/src-tauri/Cargo.toml`, ~10 files under `services/llm/`, `services/health.rs`, `services/tts/`, `setup/`, `tauri.conf.json`.
**Dependency**: None (first batch). **⛔ Halts everything downstream if red.**
**Addresses**: O-1, O-2, O-3, O-4, O-5, O-10.

#### Changes

##### [MODIFY] [`app/src-tauri/Cargo.toml`](file:///home/addy/projects/apps/vox/app/src-tauri/Cargo.toml)
- `reqwest` `0.12` → `0.13`, same features (`stream`, `rustls-tls`, `json`, `blocking`).
- Add `kestrel-rs = "=11.0.0"` (exact pin).
- Version field → `11.0.0` (see O-10 — confirm interpretation first).

##### [MODIFY] version surfaces
- `app/src-tauri/tauri.conf.json` → `11.0.0`.
- `app/package.json` version (if present and versioned in lockstep) → `11.0.0`.
- Verify no other version literals (updater config, About dialog) — update all in the same batch.

##### [FIX] reqwest 0.13 fallout
- `cargo check` the workspace; repair every breakage:
  - `services/llm/transport/mod.rs` (Client builder, RequestBuilder)
  - `services/llm/transport/chat_completions.rs`, `responses.rs`, `ollama.rs`
  - `services/llm/catalog/sync.rs`, `probe.rs`
  - `services/health.rs`
  - `services/tts/voice.rs`, `services/tts/providers/chatterbox_remote.rs` (`blocking::Client`)
  - `setup/manifest.rs`, `setup/model_manager.rs`
- ⚠️ If 0.13 removed/renamed APIs we rely on and fallout exceeds a day of work: STOP, report, decide against this plan whether to proceed (living with dual stacks) or drop kestrel for a 0.12-based fallback. Do not silently downgrade.

##### [VET — no Vox code] kestrel-rs empirical checks (throwaway scratch crate, results recorded back into this plan's Open Issues)
1. **O-1**: call `search_many` live; does `SearchResult.content` arrive populated? Record actual latencies per engine subset.
2. **O-2**: measure p50/p90 wall time for the 3-engine subset under `search_budget = 7s`; fix the budget number based on data.
3. **O-4**: record `cargo build --release` time + Tauri binary-size delta with/without `kestrel-rs`.
4. **O-5**: run `search_many`/`fetch_all` in a clean HOME; check whether `~/.kestrelsearch/` is created/written; determine the trigger and whether it can be disabled or redirected.
5. **O-1**: same for `fetch_all` — confirm `content_limit`/`max_response_bytes` behavior at boundaries.

**Exit criteria**: workspace green on reqwest 0.13; vetting results appended to §2 (issues marked resolved or plan amended); version surfaces aligned.

---

### Batch 1 — `services/web/` module 🟢

**Blast radius**: new files only under `app/src-tauri/src/services/web/`. Zero harness changes.
**Dependency**: Batch 0.
**Addresses**: O-1, O-5 (containment), O-8 (SSRF is here).

#### Changes

##### [NEW] `services/web/mod.rs`
- Module wiring + re-exports. Nothing else.

##### [NEW] `services/web/traits.rs`
```rust
pub struct VoxSearchHit { title, url, snippet }         // Vox-owned; no kestrel types cross the boundary
pub struct VoxFetchedPage { url, final_url, text, truncated: bool }

pub trait WebSearchProvider: Send + Sync {
    async fn search(&self, query: &str, limits: &SearchLimits) -> Result<Vec<VoxSearchHit>, WebError>;
}
pub trait WebFetcher: Send + Sync {
    async fn fetch(&self, url: &str, limits: &FetchLimits) -> Result<VoxFetchedPage, WebError>;
}
```
- `SearchLimits` / `FetchLimits` are Vox settings snapshots (max_results, snippet_chars, content_limit, max_response_bytes, budgets, engine subset…).
- Trait boundary exists so a future `web-search`/`webfetch`/Brave provider swaps in with zero harness edits.

##### [NEW] `services/web/ssrf.rs`
- The guard from §1.3: `validate(url) -> Result<ValidatedUrl, SsrfViolation>`.
- Manual redirect loop (max 5 hops), per-hop re-validation.
- Pure, synchronous, unit-testable with no network (resolution injected via a resolver fn for tests).

##### [NEW] `services/web/kestrel_provider.rs`
- **Only file that imports `kestrelsearch`.**
- Maps `SearchLimits` → `SearchOptions` (engines subset, region, `TimeFilter::Any`, `max_concurrency`, `min_results`, `search_budget`), calls `search_many`, dedups by URL, takes top-N, maps to `VoxSearchHit`.
- Maps `FetchLimits` → `FetchOptions` (`timeout`, `content_limit`, `max_concurrency = 1`, `parse_concurrency`, `max_response_bytes`, `aggressive = false`), calls `fetch_all` with one URL, maps to `VoxFetchedPage`.
- Timeouts set from limits so kestrel deadlines land under the 10s harness timeout.

##### [NEW] `services/web/service.rs`
- `WebSearchService` / `WebFetchService`: hold one long-lived `KestrelClient` (shared connection bound per crate docs), expose `search`/`fetch` with SSRF validation first (fetch only), metrics hooks (`turn_metrics` retrieval timing if applicable), `~/.kestrelsearch` containment per Batch 0 findings (O-5).

**Exit criteria**: `cargo test` unit tests for ssrf guard matrix (loopback / 10.x / 172.16–31 / 192.168 / 169.254 / IPv6 link-local / redirect-to-private / non-http scheme) all green, hermetic, no network.

---

### Batch 2 — Settings + IPC 🟢

**Blast radius**: `core/settings.rs`, `core/defaults.rs`, `ipc/settings/mutation.rs`. No frontend yet.
**Dependency**: Batch 1 (limits structs inform defaults; can parallelize if needed).
**Addresses**: threshold authority (§0 #8), single toggle (§0 #4).

#### Changes

##### [MODIFY] [`core/defaults.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/defaults.rs)
```
DEFAULT_WEB_SEARCH_ENABLED        = false   // opt-in: v1 sends queries to third-party engines
DEFAULT_WEB_SEARCH_MAX_RESULTS    = 5
DEFAULT_WEB_SEARCH_SNIPPET_CHARS  = 500
DEFAULT_WEB_SEARCH_BUDGET_MS      = <Batch 0 measured value>   // O-2
DEFAULT_WEB_SEARCH_REGION         = "us-en"
DEFAULT_WEB_FETCH_CONTENT_LIMIT   = 8000
DEFAULT_WEB_FETCH_RESPONSE_BYTES  = 512_000
DEFAULT_WEB_FETCH_TIMEOUT_MS      = 7000
```

##### [MODIFY] [`core/settings.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/settings.rs)
- New `WebSearchSettings` struct (fields above) + `Default` impl.
- Wired into the root settings struct alongside `PersonalMemorySettings` etc.

##### [MODIFY] [`ipc/settings/mutation.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/mutation.rs)
- Match arms for each key with validation: `web_search_enabled` must be bool; `max_results` 1–10; char/byte/timeout fields bounded (mirror the `top_k_facts` 1–100 style).

**Exit criteria**: settings round-trip test green; invalid values rejected with clear IPC errors.

---

### Batch 3 — The two tools 🟢

**Blast radius**: `services/harness/stages/tools/` (2 new files), `registry.rs`, `chassis.rs`, `steps.rs`/`loop.rs` (filter field wiring only).
**Dependency**: Batches 1 + 2.
**Addresses**: O-8, O-9 (tool behavior; spec edit itself is Batch 6).

#### Changes

##### [NEW] `stages/tools/web_search.rs`
- `WebSearchTool`: `ToolDomain::Modular`, `ToolFlow::NonTerminal`.
- Schema (Modular): required `query` (string), `spoken_filler` (string). Nothing else — threshold authority.
- Execute: read limits from settings → `WebSearchService::search` → format observation per §1.1 → `ToolResult::new(obs).with_spoken_filler(filler)`.
- Empty / error paths per NonTerminal failure contract (`is_error = true`, sanitized message).

##### [NEW] `stages/tools/web_fetch.rs`
- `WebFetchTool`: `ToolDomain::Modular`, `ToolFlow::NonTerminal`.
- Schema (Modular): required `url` (string), `spoken_filler` (string).
- Execute: SSRF `validate` → violation = `is_error` observation → else `WebFetchService::fetch` → format observation per §1.2.

##### [MODIFY] `stages/tools/registry.rs`
- Register both in `with_default_tools()`.
- `ToolFilter`: add `web_search_enabled: bool`.
- `active_definitions`: exclude **both** `web_search` and `web_fetch` when `!filter.web_search_enabled` (single flag, single log line each — mirrors the `search_memory` pattern at `registry.rs:71-77`).

##### [MODIFY] `services/harness/chassis.rs`
- Read `settings.web_search.enabled` → `web_search_enabled` (both Modular and Realtime constructors keep compiling; Realtime just never matches the `Modular` domain anyway).

##### [MODIFY] `services/harness/steps.rs`, `services/harness/loop.rs`
- Pass the new `ToolFilter` field at the existing filter-construction sites (same places `memory_retrieval_enabled` is passed today).

**Exit criteria**: `cargo nextest run` green; new unit tests: schema shape (required fields exactly as specified), registry gating on/off, formatter truncation, error-path observations.

---

### Batch 4 — Frontend 🟢

**Blast radius**: `app/src/store/settingsStore.ts`, `app/src/data/settingsCopy.ts`, one new settings card component, settings page mount.
**Dependency**: Batch 2 (IPC keys must exist).
**Addresses**: §0 #4 (the one toggle the user actually sees).

#### Changes

##### [MODIFY] `app/src/store/settingsStore.ts`
- Type the new `web_search` settings block on `draftSettings` (mirror `personal_memory` at ~line 300).

##### [MODIFY] `app/src/data/settingsCopy.ts`
- Card title + toggle copy entries (active/inactive labels + sublabels). Exact copy drafted in-batch; tone matches existing cards.

##### [NEW] `app/src/shared/components/settings/web_search/WebSearchCard.tsx`
- Clone the `PersonalMemoryCard` structure: reads `draftSettings.web_search`, **one `ToggleTile`** bound to `web_search_enabled` via `updateDraft("web_search", "web_search_enabled", !current)`.
- Numeric knobs (max results, content limit) are **not** exposed as UI in v1 — settings-only defaults (keeps the card honest: the user has one decision to make). Revisit if tuning is needed in practice.
- Card mounted on the settings page next to Personal Memory.

**Exit criteria**: toggle flips draft → save → IPC persists → reload reflects; `npm run build` (or repo's typecheck) green; no component calls `invoke`/`listen` directly (service boundary).

---

### Batch 5 — Tests + eval 🟡

**Blast radius**: `app/src-tauri/tests/`, eval harness files.
**Dependency**: Batches 1–4.
**Addresses**: O-7 (degradation visibility), general regression bar per AGENTS.md §3.

#### Changes

##### Hermetic unit tests
- SSRF guard matrix (already partial in Batch 1 — extend to redirect chains).
- Observation formatters: truncation boundaries, empty results, `is_error` messages.
- Registry gating: flag on/off × domain.

##### Integration (wiremock — no live network in CI)
- `search_many` response (canned HTML/JSON fixture) → observation golden text.
- `fetch_all` → extraction golden + `None` slot → `is_error` path.
- **Two-step chain**: model proposes `web_search`, observation returned, same turn re-enters, model proposes `web_fetch` on a result URL — verifies the re-entrant loop with two NonTerminal tools and `MAX_TOOL_ITERATIONS` interplay.

##### Eval (mirror set-title / search-memory pattern)
- New E2E suite: "answer a current-events style question requiring web search" scored 1/1.
- New E2E suite: "read this specific page and summarize" (fetch path) scored 1/1.
- Mutation run on new code, targeting ≥ your existing 73% kill-rate bar.

##### Live smoke
- One `#[ignore]` test hitting real DDG HTML through `web_search` — run manually only, with explicit user approval (AGENTS.md §3.4).

**Exit criteria**: full suite green under the standard nextest command from AGENTS.md §3.3; evals scored; mutation rate recorded.

---

### Batch 6 — Spec + docs + history hook 🟢

**Blast radius**: `docs/specs/tools-spec.md`, optionally `harness-spec.md`, `docs/plans/phase12/recent_work.md`, `AGENTS.md` §5.
**Dependency**: Batch 5 green (or batch-3 code complete if landing spec-first per O-9).
**Addresses**: O-9 (spec-first), repo post-task hook.

#### Changes
- **Spec-first (if executing O-9 correctly):** append `web_search` and `web_fetch` catalog entries to `tools-spec.md` §7 **before Batch 3 code lands** — classification (NonTerminal/Cognitive Observation), Modular-only domain, params, observation contracts, settings-owned limits. This plan's batch order puts code before spec for convenience; **the Non-Drift Hook says spec first — resolve this ordering in review.** (This is itself an open issue: O-9 explicitly.)
- No `harness-spec.md` change expected (no loop/lifecycle changes) — verify during spec pass.
- Post-task hook: append bullets to `AGENTS.md` §5; if line count ≥125, migrate delta to `docs/plans/phase12/recent_work.md` per §1.

**Exit criteria**: specs match shipped behavior; history hook applied.

---

## 4. Explicit non-goals (v1)

- Realtime / voice-mode exposure of either tool.
- API-key providers (Brave, etc.).
- Browser/JS-rendered fetch (no Servo, no Chromium) — plain HTTP only.
- Site allowlist/blocklist UI.
- Per-call user approval flow.
- Harness loop changes (no adaptive recursion, no compensating-action registry).
- MCP exposure of these tools.

## 5. Fallback

If Batch 0 vetting fails (O-4 weight, O-5 unwritable dotfiles, O-3 reqwest fallout unbounded): swap `kestrel_provider.rs`'s implementation for `web-search` 0.5.0 (search) + `webfetch` from `webtools-fetch` 0.6.0 (fetch). Both were evaluated; the trait boundary in Batch 1 exists precisely so this is a one-file change plus Cargo edits. `webtools-fetch` notably ships a real SSRF `guard` module and uses reqwest 0.12 — but adoption is tiny (321 downloads, scraper 0.19), so it is a fallback, not a promoted alternative.

---

*Draft authored 2026-09-23. Pending external-agent review of §2 Open Issues before any batch is executed.*



## a stupid idea too 
what is write a js playbook  that opens a broswer of user then send the model query to open llm websites with tmeprary chat like chatpgt , tell me it to do websarch and give reposnse , we scrape theat repsonse and send that to our local llm 

explore google ai search optoins , if gcp vertex ai asearch provices free qouata or not 
