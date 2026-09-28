# Model Registry vs Frontend Coupling — Brutal Audit & Remediation Plan

- **Date:** 2026-09-28
- **Phase:** 12
- **Role:** Senior System Architect (audit direction only — no implementation)
- **Trigger:** Recurring friction — adding a TTS model (ZipVoice) forced 6 frontend file edits with no compiler or runtime signal that any of them were required.
- **Scope:** Frontend model-selection surface, capability resolution, voice resolution, and the manifest/catalog contract that feeds it.
- **Evidence tier:** Auditor (graph + full source reads + coverage caveats below).

---

## 1. The Direct Answer

**"Why does the frontend need to change when we add a TTS model?"**

Because the frontend does not have a model *registry*. It has six independent
hand-maintained copies of the model list, each with a different shape, each
written in TypeScript, each compiled to `any` at the boundary. Adding a model
means editing all of them, and nothing tells you which ones you missed.

The frontend never needs to know model names. It needs to know *capabilities*.
The capability question is already answered correctly by the backend
(`get_provider_caps`). The frontend ignores that answer and re-derives it from
string literals because the capability object is per-provider and the frontend
has decided it needs a static per-provider map instead.

**"Why are any model names even mentioned in the frontend?"**

Because five separate decisions that should be data-driven were implemented as
string equality. Each was individually reasonable. Together they are the
recurring tax:

| # | Decision | Should come from | Actually comes from |
|---|----------|------------------|---------------------|
| 1 | Which voice UI to render | `caps.voices` (backend) | partly `caps.voices`, partly `providerId === "zipvoice"` |
| 2 | Which settings key holds the voice | manifest group `id` | `customConfigMap` literal lookup |
| 3 | Which voices belong to this model | backend-scoped voice list | `v.id.startsWith("zipvoice_voice_")` string prefix |
| 4 | Whether a voice is a catalog voice | `caps.voices === "catalog"` | `activeTts?.id === "supertonic" \|\| === "kokoro"` |
| 5 | Which models count as resident | manifest `category`/`is_cloud`/`is_remote` | 3-term allowlist `supertonic \|\| kokoro \|\| chatterbox` |

**"Why is our manifest not SSOT?"**

Because the manifest **cannot** be SSOT today. It does not carry the data the
frontend needs. Verified — full key enumeration of
`manifests/models_manifest.json`:

```
group keys : category, description, files, id, is_built_in, is_cloud,
             is_remote, name, parameters, ram_usage, required, subcategory,
             tradeoffs, version
file keys  : archive, id, path, required, sha256, size

ANY cap/voice/speed/clone/guidance/step field? -> NONE
```

The manifest describes **distribution** (what bytes to fetch, how big, hashes).
It says nothing about **behavior** (voice source, tunable params, ranges). So
`caps_for_id` in Rust is a hand-written match, `FALLBACK_CAPS` in TS is a
hand-written copy of that match, and the third hand-written copy is the
allowlists in the components. The manifest is SSOT for downloads and a guest
book for behavior. That asymmetry is the root cause.

---

## 2. Root Cause — Three Structural Defects

### 2.1 The capability contract is unenforceable at every boundary

Three layers each hand-wave validation, so no layer catches a missing entry.

**Layer 1 — Backend silently fabricates capabilities.**
`app/src-tauri/src/core/settings.rs:711-716`:

```rust
_ => ProviderCaps {
    voices: TtsVoiceSource::Catalog,
    speed: true,
    quality_steps: false,
    clone: false,
},
```

An unknown model id returns plausible-looking catalog capabilities instead of
failing. The frontend therefore **cannot** distinguish "this model genuinely
has catalog voices" from "the backend has never heard of this model."

This is a **direct contradiction of the approved spec** —
`docs/specs/ipc-spec.md:257`:

> Unknown provider IDs MUST return an explicit error and never silently fall
> through to default/catalog capabilities.

And the test suite *asserts the violation* —
`app/src-tauri/src/core/settings.rs:1213`:

```rust
assert_eq!(caps_for_id("no_such_engine"), caps_for_id("supertonic"));
```

The spec was amended in this same change set to state the invariant. The code
and its test were left asserting the exact opposite. Per AGENTS.md §4.3 this is
live spec drift, not a style issue.

**Layer 2 — Frontend types are `string` / `any` at the mutation boundary.**
`app/src/store/settingsStore.ts:471-476`:

```ts
updateDraft: (
  domain: keyof VoxSettings,
  key: string,        // <- not keyof TtsSettings
  value: any,         // <- unchecked
  explicitDomainId?: SettingsDomainId
) => void;
```

`updateDraft("tts", "zipvocie", {...})` compiles clean, renders clean, and is
dropped by the backend.

**Layer 3 — The backend's rejection is a success response.**
`app/src-tauri/src/ipc/settings/core.rs:337-343` returns
`Ok(SettingUpdateResult { applied: false, message: "Unknown setting: tts.zipvoice" })`.
Not an error. A `200 OK` carrying a refusal.

And `applied` is read by **nobody**:

```
$ grep -rn "\.applied" app/src/ --include=*.ts --include=*.tsx
(no matches)
```

So the entire rejection path is invisible end to end. A key the backend does not
understand fails silently, permanently, with zero user-visible and zero
developer-visible signal.

### 2.2 Dirty-tracking and discard are gated on a hand-maintained key list

`app/src/data/settingsCopy.ts:54-65` hardcodes the TTS setting keys:

```ts
tts: ["active","voice_index","quality_steps","speed",
      "edge_tts","supertonic","kokoro","chatterbox","chatterbox_remote","zipvoice"],
```

That list is consumed by four functions in
`app/src/store/settingsStore.ts`:

- `isDomainDirty` (:596) — drives the "unsaved changes" indicator
- `isCategoryDirty` (:625) — drives per-tab dirty state
- `discardCategoryChanges` (:647) — drives Revert
- `discardDomainChanges` (:666) — drives Revert

`commitChanges` (:705) uses `for (const key in draftObj)` and is therefore
*immune* — persistence works. **The indicator and the undo path are not.**

Failure mode if the entry is forgotten — and it *was* forgotten by the pattern,
not by the author, which is the entire complaint:

1. User changes only the new model's voice.
2. Dirty indicator never lights. No "unsaved changes" affordance appears.
3. User clicks Revert. `discardDomainChanges` iterates the hardcoded list,
   skips the new key, **leaves the draft mutated**.
4. 600ms later the debounced auto-save at :566-577 fires `commitChanges`,
   whose `for...in` loop *does* see the key — and **persists the change the user
   just discarded.**

The undo button writes the user's rejected change to disk. The asymmetry
between `commitChanges` (`for...in`) and `discard*` (allowlist) is what turns a
missed list entry into data corruption rather than a cosmetic bug.

### 2.3 Voice resolution is string-prefix archaeology, with triplicated names

ZipVoice voices are defined **three times**, with **two different naming
schemes**, in **two languages**.

| Site | Content | Names |
|------|---------|-------|
| `app/src/shared/components/settings/models/TtsVoiceManager.tsx:139-148` | `defaultZipvoiceList` | `Atlas`, `Nova`, `Alfred`, `Vera`, `Sage`, `Maya`, `Claire`, `Iris` |
| `app/src-tauri/src/persistence/voices.rs:249-258` | `match slug` | `Atlas (Calm)`, `Nova (Warm)`, `Alfred (Formal)`, `Vera (Energetic)`, `Sage (Measured)`, `Maya (Approachable)`, `Claire (Polished)`, `Iris (Empathetic)` |
| `~/.vox/models/tts/zipvoice/voices/<slug>/` | on-disk directories | the real source |

The frontend list is the **fallback path** — it renders only when the DB seed
found nothing. So the *same user* sees `Atlas` on a fresh install and
`Atlas (Calm)` after the seeder runs. The two paths also disagree on identity:
the frontend hardcodes `id: "zipvoice_voice_atlas"`, the backend derives
`format!("zipvoice_voice_{}", slug)`, and the engine decodes it a third time at
`app/src-tauri/src/services/tts/providers/zipvoice.rs:472` via
`id.strip_prefix("zipvoice_voice_")`.

The id prefix is a **wire convention, not a data model**. It leaks into:

- `TtsVoiceManager.tsx:151` — `v.source_kind === "zipvoice" || v.id.startsWith("zipvoice_voice_")`
- `TtsVoiceManager.tsx:160` — the negative filter for the chatterbox branch
- `zipvoice.rs:472` — `strip_prefix("zipvoice_voice_")`

**The `source_kind === "zipvoice"` branch is dead code.** The backend writes
`"zipvoice_pack"` (`app/src-tauri/src/persistence/voices.rs:281`). Verified —
the only `source_kind` values written anywhere in the backend are
`zipvoice_pack`, `edge`, and `pre_baked`. The frontend filter's first
disjunct can never be true; correctness rests entirely on the string-prefix
fallback. The frontend is filtering on a value it knows nothing about, using a
convention it invented.

### 2.4 The scoping primitive exists on the backend and is ignored

`app/src-tauri/src/ipc/voices.rs:59-90`:

```rust
pub async fn list_voices(provider: Option<String>, ...) {
    if let Some(p) = provider.as_deref() {
        if p.to_lowercase() == "edge" || p.to_lowercase() == "edge_tts" { /* edge path */ }
    }
    // everything else: ALL voices from the DB, unscoped
}
```

`provider` is honoured **only** for Edge. For every other value it is ignored
and the full unfiltered voice table is returned. So the parameter is a
**phantom** — and the frontend type advertises five values that do nothing:

`app/src/services/voiceService.ts:27`

```ts
export function listVoices(provider?: "custom" | "edge" | "kokoro" | "supertonic" | "zipvoice")
```

Actual call sites (`ModelsCard.tsx:192,203`): `listVoices()` and
`listVoices("edge")`. Three of the five union members are never passed, and the
remaining two would be ignored anyway.

This is the causal chain in one line: **the backend never scoped voices by
model, so the frontend filters by model name.** The prefix hack in
`TtsVoiceManager.tsx:151` is not a design choice — it is a manual
reimplementation of a query the backend should have run.

---

## 3. Complete Defect Inventory

### 3.1 Spec contradictions (must be resolved before any refactor)

| # | Location | Finding |
|---|----------|---------|
| S1 | `docs/specs/ipc-spec.md:257` | Spec: unknown provider IDs **MUST** error. Code (`settings.rs:711`) and test (`settings.rs:1213`) assert silent catalog fallback. **Direct violation.** |
| S2 | `docs/specs/ipc-spec.md:256` | Spec enumerates 6 canonical TTS provider IDs. This closed list is the *second* SSOT that must be hand-edited per model — a direct cause of the recurring tax. |
| S3 | `docs/specs/ipc-spec.md:274` | Spec mandates the `zipvoice_voice_<slug>` id-prefix wire convention, institutionalising the string archaeology. Removing it is a **spec change**, not a code change. |
| S4 | `docs/specs/ipc-spec.md:252` | Spec declares `tts.zipvoice` carries `guidance_scale`. No UI exists to set it — see D9. |
| S5 | `.agents/rules/frontend-engineer.md:25` | "Components never ... embed hardcoded strings." Violated at 13 sites. |

### 3.2 Defects, ordered by blast radius

| # | Severity | Location | Defect | Trigger scenario |
|---|----------|----------|--------|------------------|
| **D1** | **Critical** | `mutation.rs:288` + `core.rs:337` + `settingsStore.ts` (no `.applied` read) | Unknown setting key returns `Ok(applied:false)`; frontend never checks. **Silent permanent settings loss.** | Any new `tts.<x>` key shipped before its backend arm. Data is accepted by the UI, acknowledged by IPC, never written. |
| **D2** | **Critical** | `settingsCopy.ts:54-65` + `settingsStore.ts:647,666` | Dirty-check and discard gated on hardcoded allowlist; `commitChanges` uses `for...in`. **Revert persists the discarded value** via 600ms auto-save. | Change one new-model key, hit Revert → the rejected change is written to disk. |
| **D3** | **High** | `settings.rs:711-716` | `caps_for_id` `_` arm fabricates catalog caps for unknown ids. Frontend cannot detect an unregistered model. | Any model present in the manifest but missing from `caps_for_id` renders with plausible-but-wrong UI. |
| **D4** | **High** | `settingsService.ts:43-50` | `FALLBACK_CAPS` is a hand-maintained TS copy of the Rust match. Used whenever the IPC throws. | Backend transiently unreachable → caps silently change → voice/speed/steps panes appear or vanish. Two SSOTs for one fact. |
| **D5** | **High** | `TtsVoiceManager.tsx:139-148` vs `voices.rs:249-258` | Voice names defined twice, **disagreeing**. | Fresh install shows `Atlas`; post-seed shows `Atlas (Calm)`. Fallback list and authoritative list both reachable. |
| **D6** | **High** | `ipc/voices.rs:63-80` | `list_voices` ignores `provider` for all non-Edge values. | Frontend compensates with prefix filtering. Adding a 2nd custom-voice model makes chatterbox and zipvoice voice lists cross-contaminate unless the prefix filter is extended by hand. |
| **D7** | **High** | `ModelStatusOverlay.tsx:82` | `isCatalogVoice` is a 2-term allowlist where `caps.voices === "catalog"` is already available. | Add any second catalog-voice TTS → overlay silently shows no voice chip. Currently correct **by luck** (zipvoice is `Custom`). |
| **D8** | **Medium** | `Monitoring.tsx:99` | `isTtsModel` is a 3-term allowlist (`supertonic\|kokoro\|chatterbox`), missing `zipvoice`, `chatterbox_remote`, `edge_tts`. | **Live bug today**: with zipvoice active, `totalResidentModelsCount` under-reports by 1 — the resident-model metric is wrong on the monitoring page. Not caught by any test. |
| **D9** | **Medium** | `settingsStore.ts:52,221` | `guidance_scale` typed in TS, defaulted and clamped in Rust (`mod.rs:84-85`), **no UI**. | Dead config surface. Spec declares it (§S4). Either build the control or drop the field. |
| **D10** | **Medium** | `TtsVoiceManager.tsx:167-172` | `customConfigMap` maps provider id → settings key by literal, then `(customConfig as any)` twice at :180,:197,:199 to defeat its own typing. | Any 4th custom-voice model needs a map entry **and** an `as any` escape. The `as any` at :180 is why TS never flagged the missing zipvoice key. |
| **D11** | **Medium** | `settingsStore.ts:39-52` vs `core/settings.rs:630-665` | Two hand-synced `TtsProviderConfig` unions. FE declares `Zipvoice { guidance_scale, quality_steps, speed }`; BE declares `Zipvoice { voice_id, guidance_scale }`. **Field sets disagree.** | `checkTtsProviderHealth`/`create_tts_provider` receive a payload whose `voice_id` the FE type omits and whose `quality_steps`/`speed` the BE type omits. Works only because both sides are unvalidated. |
| **D12** | **Medium** | `settingsStore.ts:236-237` | `provider?: TtsProviderConfig` and `voice?: number` on `TtsSettings`; neither exists on the backend `TtsSettings`. | Phantom FE fields. `voice` is the legacy pre-provider-config shape (ZBC violation). |
| **D13** | **Low** | `ModelStatusOverlay.tsx:81` | `m.id === ttsKind \|\| m.id.includes(ttsKind)` — substring match on ids. | A model named `kokoro_hd` matches `kokoro`; picks the wrong active chip. |
| **D14** | **Low** | `ProviderSelectorView.tsx:63-79` | TTS taxonomy is a hardcoded 3-way `embedded/server/cloud` unrelated to the 6-value `TtsActiveProvider`. Sublabel is literally `"Chatterbox GPU"`. | Parallel taxonomy that will never match the real enum. A model name in a UX label with no data binding. |
| **D15** | **Low** | `settingsStore.ts:534-544` | `requiresRestart` is a hand-maintained `(domain, key)` allowlist. New `tts.<x>` keys are not there. | Restart-requiring key added → hot-applied instead → stale worker. |
| **D16** | **Low** | `settingsStore.ts:548-562` | `domainMap` hardcodes `tts: "models"`. | Fine today; breaks if TTS ever gets its own settings domain. |

### 3.3 Capability Verification — `ProviderCaps` is wrong in 2 of 6 providers

Verified by enumerating every `TtsProvider` impl against `caps_for_id`. **The
trait's control setters and the declared caps disagree twice.**

| Provider | overrides `set_quality_steps`? | `caps.quality_steps` | |
|---|---|---|---|
| **supertonic** | **YES** — `supertonic.rs:174` | **`false`** | ❌ |
| kokoro | no | `false` | ✅ |
| chatterbox | YES — `chatterbox.rs:146` | `true` | ✅ |
| chatterbox_remote | YES — `chatterbox_remote.rs:229` | `true` | ✅ |
| edge_tts | no | `false` | ✅ |
| zipvoice | YES — `zipvoice.rs:103` | `true` | ✅ |

| Provider | overrides `set_speed`? | `caps.speed` | |
|---|---|---|---|
| **edge_tts** | **YES** — `edge_tts.rs:405` | **`false`** | ❌ |
| all others | YES | `true` | ✅ |

**D17 — Supertonic's quality_steps control is unreachable.**
Supertonic implements `set_quality_steps` with a live `AtomicU32`
(`supertonic.rs:163`), seeds it from settings clamped to
`MAX_QUALITY_STEPS_SUPERTONIC = 16` (`:164`), and imports that constant
specifically (`:23`). It is a fully working, persisted, hot-updatable control.
`caps_for_id` says `quality_steps: false` (`:687-692`), and the Steps knob is
gated on exactly that (`TtsVoiceManager.tsx:311,369`) — so **the knob never
renders for Supertonic.** The value is live in the engine and unreachable from
the UI.

**D18 — Edge's `speed` cap is wrong, and unenforced anyway.**
`EdgeTtsProvider` overrides `set_speed` (`edge_tts.rs:405`), but
`caps_for_id("edge_tts").speed == false` (`:699-704`). Separately, the Speed
tab is **not gated on `caps.speed` at all** — `TtsVoiceManager.tsx:270` renders
it unconditionally. So the knob appears for Edge despite the cap saying it
shouldn't. The declared cap and the actual UI disagree in the *opposite*
direction from Supertonic.

**D19 — `caps.speed` is read by nobody.** Frontend consumption of `ProviderCaps`
is exactly three sites: `caps?.clone` (`TtsVoiceManager.tsx:133`),
`caps?.quality_steps` (`:311`, `:369`). **`caps.speed` has zero consumers.** It
is declared in Rust, mirrored into `FALLBACK_CAPS`, and dead. `speed` in
`ProviderCaps` is a decorative field — a capability nobody reads.

**D20 — The Steps knob range is wrong for every provider, and sits below the
default.**

`DEFAULT_TTS_QUALITY_STEPS = 12` (`defaults.rs:45`) seeds
`TtsSettings.quality_steps = 12` (`settings.rs:742`). The knob renders
`min={4} max={8}` (`TtsVoiceManager.tsx:355-356`).

| Provider | default | backend clamp | knob max | Result |
|---|---|---|---|---|
| supertonic | 12 | 16 | **hidden** | control exists, entirely unreachable |
| chatterbox | 12 | 10 | **8** | knob shows 8; engine is actually at 10 — the UI never displays the real value |
| zipvoice | 12 | **8** | 8 | works, but only because the clamp ceiling coincidentally equals the UI max |

**Consequence for the design:** `ProviderCaps` is not currently derived from
anything. It is a hand-written table, wrong in 2 of 6 cases, with 1 of its 4
fields unconsumed. There is no verified behaviour to preserve.

**This falsifies the "the no-op setters already declare capability" idea.** The
overrides and the caps disagree in both directions, so `fn caps()` must be an
**explicit per-provider declaration** — never inferred from which setters are
overridden. The synthesis path and the UI surface are related but not
identical, and conflating them is what produced D17/D18.

**Corollary for the invariant:** D19 happened because a field was added to
`ProviderCaps` and given a `FALLBACK_CAPS` entry without a consumer. Any new
`ProviderCaps` field must be required to have a consuming frontend site, or it
does not get added.

### 3.4 The tax, quantified

Adding ZipVoice touched **19 files / 476 insertions**. The *irreducible* set is
one manifest group and one Rust engine file. Everything else is synchronization.

| Layer | Files | Irreducible? |
|-------|-------|--------------|
| Manifest | 1 | **Yes** — the SSOT |
| Rust engine | 1 new + 1 mod decl | **Yes** |
| Cargo | 1 | **Yes** |
| Rust settings types/arms (enum, config, caps, defaults, factory, health, mutation) | 7 | No — derivable |
| Rust constants duplicating manifest `files[].path` | 1 | No — see D-list §3.2 note below |
| Rust voice seeding + display names | 2 | No — belongs in manifest/DB |
| Specs | 3 | Partly — but see S1-S4 |
| **Frontend** | **6** | **No — all of it** |

Note on the Rust constants: `app/src-tauri/src/services/tts/mod.rs:73-78`
re-declares `MODEL_FILE_TTS_ZIPVOICE_ENCODER = "encoder.int8.onnx"` etc., which
the manifest **already declares** as `files[].id = "tts_zipvoice_encoder"` /
`files[].path = "tts/zipvoice/encoder.int8.onnx"`. The manifest carries stable
file ids; the backend ignores them and re-hardcodes the filenames. This is the
manifest-SSOT failure in its purest form — inside the backend.

**12 of 19 files, and ~100% of the frontend diff, are pure synchronization cost.**

---

## 4. The Correct End State

Three properties. All three are prerequisites; none is optional.

1. **The engine declares its own behaviour.** Every `TtsProvider` implements
   `fn caps()`, and declares its tunable ranges as `ParamRange` consts sitting
   beside the clamps that enforce them. One fact, one place, adjacent to the
   code that makes it true. The manifest stays a **distribution** artifact —
   bytes, hashes, topology — and gains **no new fields**.

2. **Unknown capabilities are errors, everywhere.** `caps_for_id` returns
   `Result` from an **exhaustive** match over `TtsProviderKind`. A typo'd
   provider id fails loudly at the IPC boundary; a new provider variant cannot
   exist without a `caps()` implementation. The frontend never sees a
   fabricated answer, so it never needs a fallback map.

3. **The frontend contains zero model identifiers.** Not "fewer" — zero. Every
   rendering decision is a pure function of a capability object and a
   backend-scoped voice list. Adding a model means: implement the provider, add
   one compiler-checked dispatch arm. **No frontend file is touched.**

**The test for "done":** delete any provider id string from `app/src/` and the
frontend must still build and behave identically for every remaining provider.

---

## 5. Remediation Plan (Phase 12)

Sequenced so each batch is independently verifiable and leaves the tree green.
Batches 1-2 are pure guards and pay for themselves immediately.

### Batch 0 — Fix the wrongly-declared caps (0.5 day, live bugs)

*Discovered during Batch 3 verification. These are user-facing now, so they do
not wait for the refactor — Batch 0 runs alongside Batch 1.*

- **0.1** **D17** — `caps_for_id("supertonic").quality_steps` → `true`
  (`settings.rs:687-692`). Restores a working, persisted, engine-backed control
  that the UI currently cannot reach. Fix the *data*; the trait move in 3.1
  makes it structural.
- **0.2** **D18** — `caps_for_id("edge_tts").speed` → `true`
  (`settings.rs:699-704`), matching `edge_tts.rs:405`.
- **0.3** **D19** — decide `ProviderCaps.speed`: gate the Speed tab on it
  (`TtsVoiceManager.tsx:270`) or delete the field. See Batch 3.6 — the same
  decision is needed there, so resolve it once, in Batch 0.
- **0.4** **D20** — expose each provider's real range. Minimum viable fix:
  raise the knob ceiling to `MAX_QUALITY_STEPS_CHATTERBOX` (10) and set the
  floor to `MIN_QUALITY_STEPS` (2) so the default 12 is at least clamp-visible.
  Proper fix is 3.3. Also reconcile `DEFAULT_TTS_QUALITY_STEPS = 12`
  (`defaults.rs:45`) — it exceeds the Chatterbox ceiling of 10, so the shipped
  default is silently clamped on first run.

**Exit:** Supertonic exposes quality steps; Chatterbox's knob reflects the
value the engine actually uses; no `ProviderCaps` field lacks a consumer.

### Batch 0.6 — Prune settings that no user can reach (1-2 days)

*Added 2026-09-28 after a field-level audit of `core/settings.rs`. Evidence
gathered by cross-referencing all 105 persisted leaf fields against the
frontend: which are named in an `updateDraft` call, and which are read by any
`.tsx` component. A type declaration alone does not count as reachability.*

**Finding — 13 of 105 leaf fields have no user-facing surface. 9 have no
frontend consumer at all:**

| Field | Problem |
|---|---|
| `TtsZipvoiceConfig.guidance_scale` | Defaulted, clamped `1.0..=3.0`, forwarded to the engine, **unreachable**. Doubly vestigial: ZipVoice is flow-distilled, and distillation exists specifically to eliminate classifier-free guidance (arXiv 2506.13053) — the one parameter worth tuning is the one the architecture removed. |
| `SystemSettings.log_level`, `telemetry_enabled` | `SETTINGS_SCOPE_KEYS.system` declares a "system" scope that **no component renders**. |
| `InteractionSettings.auto_sleep_timeout` | Declared in a scope key, never written, never read. |
| `LlmSettings.compaction_temperature`, `reasoning_enabled` | Same. |
| `VadSettings.max_speech_duration_s` | Same. |
| `GeminiRealtimeConfig.language_code` | Not settable. `RealtimeCard`'s dynamic `[voiceField]` resolves only to `voice_name`/`voice`. |
| `GeminiRealtimeConfig.resume_handle` | **Live session state persisted as configuration.** |
| `ElevenLabsConvaiConfig.agent_id`, `SttCloudConfig.credentials_json` / `credentials_path` | Declared only. |

- **0.6.1** Decide each of the 13: delete, or surface. `guidance_scale` →
  delete (0.6.3). `resume_handle` → move out of `VoxSettings` into live session
  state; it is not a preference. `credentials_json` / `credentials_path` →
  move to the credential store that already owns them, not the settings file.
- **0.6.2** Delete `SETTINGS_SCOPE_KEYS.system` if the scope stays unrendered, or
  add the card. A scope key the UI never reads is a promise nothing keeps.
- **0.6.3** Delete `TtsZipvoiceConfig.guidance_scale`, `DEFAULT_TTS_ZIPVOICE_GUIDANCE_SCALE`,
  `MIN/MAX_ZIPVOICE_GUIDANCE_SCALE`, the `set_guidance_scale` method, and the
  engine field. Fix the engine to the distilled model's actual behaviour.
- **0.6.4** **Split `core/settings.rs` (1217 lines) by responsibility.** It
  currently holds four unrelated things: the 13 config scopes (legitimate);
  `ModelCapabilities` (a *probe result* carrying `tested_at_epoch`/`ttft_ms`/
  `tps`); `LlmModelInfo` and `VoiceProfile` (*IPC catalog DTOs*); and
  `get_preset_colors()` — **five hardcoded UI accent hex colors**. The non-config
  items are imported by `services/llm/catalog/`, `services/llm/transport/` and
  `pipeline/assistant/session.rs`, so four subsystems reach into `core::settings`
  for things that are not settings. That absence of a single responsibility is
  why the file reads as ad-hoc.
  Proposed: `core/settings/` (scopes + reload policy), `core/model_probe.rs`
  (`ModelCapabilities`), `core/model_catalog.rs` (`LlmModelInfo`, `VoiceProfile`),
  and move `get_preset_colors` to the palette owner that should already exist.
- **0.6.5** Add the two invariants that keep the tree honest: a persisted field
  with no consumer (Batch 2.7 pattern, extended to the Rust side), and a
  `SETTINGS_SCOPE_KEYS` entry with no component that reads it.

**Exit:** every field in `VoxSettings` is either user-settable or deliberately
internal and named as such; `core/settings.rs` owns exactly one responsibility.

### Batch 1 — Close the silent-loss paths (1 day, no architecture change)

*Goal: make the current bugs loud before touching structure.*

- **1.1** `core/settings.rs:711` — change `caps_for_id` to return
  `Result<ProviderCaps, VoxIpcError>`; delete the `_` catalog arm. Update
  `catalog.rs:152` and `settings.rs:1213` (delete the
  `assert_eq!(caps_for_id("no_such_engine"), ...)` assertion — it encodes the
  spec violation).
- **1.2** `ipc/settings/core.rs:337` — return `VoxIpcError::InvalidArgument`
  for `applied == false` instead of `Ok(...)`.
- **1.3** `settingsStore.ts:711` — check `res.applied`; surface a console error
  and a failed-save toast. Removes the invisible-rejection class entirely.
- **1.4** Fix **D8** now (`Monitoring.tsx:99`) — derive `isTtsModel` from
  `modelCatalog.tts` + `is_cloud`/`is_remote` flags. One-line correctness fix
  for a live bug.
- **1.5** Add **D9** to the ledger: either add a `guidance_scale` control or
  remove the field from spec + both type unions. Decide, don't defer.
- **1.6** Spec reconciliation: correct **S1** in `ipc-spec.md` to match 1.1
  (the spec currently mandates the error; 1.1 makes the code obey). Per
  AGENTS.md §4.3, **update the spec first, then the code.**

**Exit:** unknown provider id produces a visible error; monitoring count is
correct; spec and code agree.

### Batch 2 — Add the invariant that makes this unrepeatable (1 day)

`app/src/test/invariants.test.ts` already exists with 5 static invariants and
walks every `.ts`/`.tsx` file. It has **no model-name invariant.** That is the
single missing guard — its absence is why this recurs.

- **2.1** **Invariant 6 — No model identifiers in the frontend.** Build the
  allowlist dynamically from `manifests/models_manifest.json`
  (`model_groups[].id`) plus the `TtsActiveProvider` enum. Flag any literal
  match in `app/src/**` outside an explicit `ALLOWED` set. Current file
  suppresses: `data/settingsCopy.ts`, `pages/Monitoring.tsx`,
  `services/voiceService.ts`, `services/settingsService.ts`,
  `store/settingsStore.ts`, `ModelStatusOverlay.tsx`, `TtsVoiceManager.tsx`,
  `VadWorkspace.tsx`, `ProviderSelectorView.tsx`, `LlmConfigDesk.tsx`,
  `ModelsCard.tsx`.
- **2.2** **Invariant 7 — No hand-maintained settings-key allowlist.**
  Assert `SETTINGS_SCOPE_KEYS.tts` ⊇ `Object.keys(backend TtsSettings)`, sourced
  from the Rust struct. Would have caught **D2** at review time.
- **2.3** **Invariant 8 — Parity: every `SETTINGS_SCOPE_KEYS` entry has an
  `apply_*_mutation` arm.** Would have caught **D1**.
- **2.4** **Invariant 9 — No `as any` in the settings store.** Kills **D10**'s
  escape hatch.
- **2.5** **Invariant 10 — Every `ProviderCaps` field has a frontend
  consumer.** Assert each field of the Rust `ProviderCaps` struct appears in a
  capability read (`caps?.<field>`) somewhere under `app/src/`. Would have
  caught **D19** (`speed`, zero consumers) at review time. This is the
  generalisation of D19 into a standing rule: **a capability that nothing reads
  is not a capability** — adding one without a consumer is how D19 happened.
- **2.6** **Invariant 11 — Declared caps match engine capability.** For each
  provider, if the engine overrides `set_<param>` or clamps a `MAX_*` constant,
  `caps.<param>` must be `true`. Cross-checked against the Rust impls, not the
  manifest. Would have caught **D17** and **D18**.

**Rationale:** the codebase already documents the correct pattern —
`ModelsCard.tsx:92-94`:

> Tier and health gating derive from manifest flags — never from id literals.

The convention exists and is written down. It was applied to 2 of the 5
decisions in §1. Invariants 6-9 make it total.

**Exit:** `pnpm test` fails on the current tree. That is the point — the guard
must be red before the fixes are green.

### Batch 3 — Capabilities become a trait contract (3-4 days, backend)

**Supersedes the original manifest-schema design.** Capabilities are properties
of the engine implementation, so they belong beside the code, not in a data
file that describes code. A JSON `capabilities` block would be a third copy of
ranges that already live in Rust next to the clamp that enforces them, and a
typo'd key (`"qualiy_steps"`) would be silently ignored.

- **3.1** Add `fn caps() -> ProviderCaps where Self: Sized` to `TtsProvider`
  (`providers/mod.rs:46`). **Explicit declaration — NOT derived from which
  setters are overridden.** §3.3 proves that inference is wrong in both
  directions (D17 Supertonic, D18 Edge).
- **3.2** Implement `caps()` on all 6 providers. **Fix D17 and D18 while
  doing it**: Supertonic gets `quality_steps: true`, Edge gets `speed: true`.
  Neither is a cosmetic change — both are currently-wrong user-facing
  behaviour.
- **3.3** Add a `ParamRange { min, max, step }` associated const next to each
  provider's existing clamp — `MAX_QUALITY_STEPS_SUPERTONIC` etc. already sit
  there (`tts/mod.rs:23-30`, `supertonic.rs:164,176`, `chatterbox.rs:52,147`,
  `zipvoice.rs:104,158`, `chatterbox_remote.rs:64,230`). Expose the range over
  IPC so the frontend knob and the backend clamp **cannot disagree**. This is
  the structural fix for **D20**.
- **3.4** Rewrite `caps_for_id` (`settings.rs:685`) as an **exhaustive** match
  over `TtsProviderKind` returning each type's `caps()`, with a `_ => Err(...)`
  arm that is now *unreachable* rather than *silently defaulting*. Satisfies
  the spec at `ipc-spec.md:257`; deletes the assertion at `settings.rs:1213`
  that encodes the violation.
- **3.5** Delete `FALLBACK_CAPS` and `DEFAULT_CAPS` (`settingsService.ts:43-52`).
  On IPC failure, render an explicit degraded state per
  `frontend-engineer.md:26` instead of inventing capabilities. Kills **D4**.
- **3.6** Resolve **D19** — either delete `ProviderCaps.speed` (it has no
  consumer and the Speed tab is ungated) or gate the Speed tab on it so the
  field becomes load-bearing. Do not leave a third state. This decision is
  load-bearing for 3.1: it determines whether `speed` is a real capability.
- **3.7** Delete the duplicate `MODEL_FILE_TTS_*` constants (`tts/mod.rs:73-78`),
  replaced by lookups against the manifest's stable `files[].id`. This part
  **stays manifest-facing** — file distribution genuinely is manifest data.
- **3.8** Backend integration test: every `TtsProviderKind` variant resolves via
  the exhaustive match, and every declared `ParamRange` matches the constants
  used in that provider's own clamp. A provider that declares a range it
  doesn't clamp with fails the suite.

**Explicitly NOT in this batch:** no new field on `ModelGroup`, no change to
`setup/manifest.rs`, no manifest schema migration. The manifest remains purely
a distribution artifact. The earlier `deployment` proposal is also out — see
§7.2.

**Exit:** `caps_for_id`'s match body is six exhaustive arms with no reachable
default; `zipvoice` capabilities live in exactly one place (`zipvoice.rs`); the
Steps knob range and the backend clamp come from the same declaration.

### Batch 4 — Scope the voice list (2-3 days, backend + thin frontend)

- **4.1** `list_voices` (`ipc/voices.rs:59`) — honour `provider` for all
  values. Resolve the model's voice scope server-side from the manifest.
  Kills **D6**.
- **4.2** Add `source_kind` as a declared enum in the voices schema; make
  `"zipvoice_pack"` a manifest-declared kind rather than an inline literal at
  `voices.rs:281`.
- **4.3** Seed ZipVoice display names **once**. Delete
  `defaultZipvoiceList` (`TtsVoiceManager.tsx:139-148`) and the `match slug`
  display-name block (`voices.rs:249-258`); the FS slug is the id, the DB row
  is the name. Kills **D5**.
- **4.4** Store the slug as a column. Delete `strip_prefix("zipvoice_voice_")`
  (`zipvoice.rs:472`) and the two `startsWith` filters
  (`TtsVoiceManager.tsx:151,160`). Kills the dead `source_kind === "zipvoice"`
  branch.
- **4.5** `voiceService.ts:27` — replace the 5-value phantom union with
  `provider?: string` (or drop the param entirely per 4.1's default).
- **4.6** Retire the `zipvoice_voice_<slug>` wire convention. **This is a
  spec change** (`ipc-spec.md:274`, §S3) — needs approval per AGENTS.md §4.3,
  and must be sequenced with a data migration for existing `voices.id` values.

**Exit:** `TtsVoiceManager` contains no `zipvoice` string. `grep -c zipvoice`
in `app/src/` returns 0.

### Batch 5 — Delete the frontend model knowledge (2 days)

Runs only after Batches 2-4 are green. Invariant 6 is the checklist.

- **5.1** `TtsVoiceManager.tsx` — delete `isZipvoice` (:137),
  `defaultZipvoiceList` (:139-148), `zipvoicePackagedVoices` (:150-152), the
  nested `isZipvoice` ternary in `localVoices` (:154-163),
  `customConfigMap` (:167-171), and both `as any` casts (:180, :197, :199).
  The settings key is `tts[providerId]` — the id is already the key, so the map
  is a no-op abstraction. Kills **D10**.
- **5.2** `ModelStatusOverlay.tsx:82` — `isCatalogVoice = caps.voices === "catalog"`.
  Kills **D7**. Also fix the substring match at :81 (**D13**).
- **5.3** `settingsCopy.ts:54-65` — derive the TTS key list from the boot
  payload's `tts` object keys. Kills **D2**'s allowlist permanently.
- **5.4** `settingsStore.ts:534-544,548-562` — derive `requiresRestart` from
  the backend's `reload_policy` (already returned in `SettingUpdateResult`
  and already used at :712) instead of a hardcoded list. Kills **D15**/**D16**.
- **5.5** `settingsStore.ts:39-52` — reconcile `TtsProviderConfig` with the
  Rust union; delete `provider`/`voice` phantom fields (:236-237). Kills
  **D11**/**D12**.
- **5.6** `ProviderSelectorView.tsx:63-79` — drive the TTS tier selector from
  the catalog's `is_cloud`/`is_remote` partition. Kills **D14**.
- **5.7** Consider generating `settingsStore.ts` types from the Rust structs
  (`ts-rs` or a build-time script) so **D11** cannot recur. Optional, but it is
  the only durable fix for the TS/Rust type-duplication class.

**Exit:** `rg 'supertonic|kokoro|chatterbox|zipvoice|edge_tts' app/src/` → 0
matches outside `test/`.

### Batch 6 — Ledger and enforcement (ongoing)

- **6.1** Update `docs/features/` model ledger: capability declarations are now
  manifest-owned.
- **6.2** Amend **S2** (`ipc-spec.md:256`): replace the closed 6-ID
  enumeration with "every `category: tts` group in the manifest is a valid
  provider id." Removes the second hand-edited SSOT.
- **6.3** Add a `docs/plans/phase12/model-registry-checklist.md` defining the
  post-refactor definition of done for adding a model: **edit the manifest,
  write the engine, done.** Any additional file touched is a regression signal.
- **6.4** Add the definition-of-done line to `AGENTS.md` §4 so it binds future
  phases.

---

## 6. Sequencing, Risk, and Evidence Caveats

**Dependency chain:** 0 → 1 → 2 → 3 → 4 → 5, with **0 and 1 parallelisable**
(0 is data-only, 1 is behavioural — they touch disjoint files). Batch 2 goes
red before 3-5 turn it green — that is intentional and is the whole point of a
guard. Batch 6 is continuous.

**Cheapest first proof (recommended, not a numbered batch):** the VAD fix —
`isVadModel = !modelCatalog.vad.find(g => g.id === vadBackend)?.is_built_in`
(`Monitoring.tsx:95`) and the matching default in `VadWorkspace.tsx:37`. Two
lines, no manifest change, no IPC change, no spec amendment. It validates
"derive from manifest flags, never id literals" on the smallest blast radius
before any structural work starts.

**Risk register:**

| Risk | Where | Mitigation |
|------|-------|-----------|
| **`DEFAULT_TTS_QUALITY_STEPS = 12` exceeds `MAX_QUALITY_STEPS_CHATTERBOX = 10`** | 0.1, 0.4 | Every Chatterbox install currently runs clamped from a default the user never chose. Raise the default per-provider, or clamp at seed time so the stored value is already valid. Needs a decision before 0.4 — a global default cannot be correct for per-model ceilings |
| Fixing D17 (Supertonic `quality_steps: true`) surfaces the Steps knob for a model with no validated range | 0.1 | Batch 0.4 must land together with 0.1 — do not ship the cap fix without the range fix, or Supertonic gets a 4-8 knob against a 16 ceiling |
| Deleting `ProviderCaps.speed` (D19) or gating the Speed tab on it — the two outcomes differ in UX | 0.3, 3.6 | Decide once, in Batch 0.3. Gating the tab on `caps.speed` is the smaller change and makes the field load-bearing; deleting it is the ZBC-clean option. **Do not leave it ungated with a declared cap** (the current state) |
| Deleting `caps_for_id`'s `_` arm breaks startup if any persisted `tts.active` is unrecognised | 1.1, 3.4 | Migration on load: map unknown `tts.active` → `supertonic` with a warn log, before caps resolution |
| Removing the id prefix orphans existing `voices.id` rows | 4.6 | Slug column migration; old ids read via fallback for one release. **Requires spec approval (S3)** |
| `FALLBACK_CAPS` removal surfaces degraded UI on transient IPC failure | 3.5 | Explicit loading/error state — required by `frontend-engineer.md:26`, not optional |
| Invariant 6 blocks legitimate non-model string literals | 2.1 | Drive the allowlist from the manifest; require an explicit inline `// cbm-allow: <reason>` escape, itself flagged in review |
| Invariant 11 has no single source to check against | 2.6 | Cross-check requires parsing the Rust impls. Either a build-time codegen fixture or a generated allowlist committed alongside — decide in Batch 2, do not leave it as a manual checklist |
| ~25-file refactor in one phase | all | Batches are independently shippable; each ends green |

**Hardware-tier invariant (per `system-architect.md:22`):** none of the seven
batches adds allocation, lock, or work to the audio hot path. Batch 3 leaves
`caps_for_id` a static `match` — it becomes *exhaustive*, not slower, and is
still resolved once at config time, never per chunk. Batch 4's `list_voices`
scoping adds one predicate on a config-time query, not the synthesis path.
**No pipeline stage order, threading model, or latency characteristic changes.**

**IPC contract changes (escalation per `system-architect.md:36`):** Batch 1.1
(`caps_for_id` → `Result`), 1.2 (`applied: false` → error), 3.3/3.4
(`ParamRange` exposure + exhaustive dispatch), and 4.1 (`list_voices` scoping)
all **change the backend→frontend IPC contract**. These require explicit
approval before implementation and corresponding `ipc-spec.md` amendments.

**Evidence caveats (Auditor tier, stated not hidden):**
- Graph index generation `2026-09-28T08:35:09Z`; the `zipvoice` provider file
  is **new and untracked** and therefore absent from the graph. All
  `zipvoice.rs` claims above come from direct source reads, not graph queries.
- §3.3 was verified by enumerating all 6 `impl TtsProvider for` sites against
  `caps_for_id` and the `MAX_*`/`MIN_*` constant usages. This is a complete
  enumeration of impls, not a sample — but it is text-derived, not
  graph-derived.
- Four files carry `parse_partial` ranges (`app/src/index.css:3-5`,
  `SettingsHelpContent.tsx:103`, `profiler/OverviewTab.tsx:144`,
  `profiler/PagesTab.tsx:108`). None is in the model-selection path; no claim
  depends on them.
- Negative claims ("`applied` is read by nobody", "`source_kind === "zipvoice"`
  is dead", "`caps.speed` has no consumer") are **bounded text-search findings**
  over `app/src/**` and `app/src-tauri/src/**` — not graph-absence proofs.
- `manifests/models_manifest.json` capability absence was verified by
  programmatic key enumeration over all 15 groups, not by sampling.
- **Not verified:** whether Supertonic's `quality_steps` *quality* is
  perceptually meaningful at 16 steps, or whether that ceiling was ever
  measured. 0.1 exposes a control the UI has never shown; whether the range is
  trustworthy is a separate question and was **not** established here.

---

## 7. Design Decision Record

### 7.1 Reversal: capabilities belong on the trait, not the manifest

The first draft of this plan put a `capabilities` block on `ModelGroup` in
`models_manifest.json`. **That was wrong, and reviewing it against the spec
prompted the correction.**

| Argument for the manifest | Verdict |
|---|---|
| The manifest is already the SSOT | ❌ It is the SSOT for **distribution**, not behaviour. `is_cloud` / `is_remote` / `is_built_in` are topology flags; capability is a property of the engine. |
| `edge_tts` has no engine, so it can't declare caps | ❌ **Factually false.** All 6 implement `TtsProvider` — `EdgeTtsProvider` at `edge_tts.rs:319`, `ChatterboxRemoteProvider` at `chatterbox_remote.rs:227`. |
| A JSON block is reviewable in a diff | ⚠️ True, but so is Rust. |
| Typo in a JSON key is caught | ❌ **Silently ignored.** A typo'd `"qualiy_steps"` renders a UI with no error. |
| The range and its clamp stay together | ❌ In JSON they'd be a 3rd copy. In Rust they're already adjacent — `MAX_QUALITY_STEPS_SUPERTONIC` sits 2 lines from its clamp at `supertonic.rs:23` / `:164`. |

**Decided:** `fn caps()` on `TtsProvider`, ranges as `ParamRange` consts beside
their clamps, `caps_for_id` as an exhaustive match over `TtsProviderKind`.

**Net effect on the plan:** Batch 3 loses the manifest schema change, the
`setup/manifest.rs` edit, and the schema migration. It shrinks.

### 7.2 Retraction: the `deployment` field

The first draft proposed `ModelGroup.deployment = { kind: "ssh" }` to replace
the hardcoded Chatterbox-remote SSH form. **Withdrawn.**

`is_remote` exists and is already consumed correctly in 6 places
(`ModelsCard.tsx:97`, `TtsModelWorkspace.tsx:39,48`,
`InteractionCard.tsx:103,146,149`, `Monitoring.tsx:163`). It correctly gates
tier behaviour. What it cannot carry is the 5 concrete values in the SSH form
(conn string, port, key path, remote path, server port).

But that is a **UI feature gap for exactly one model**, not a
registry-coupling bug: delete `chatterbox_remote` tomorrow and the form goes
with it. It does not make the frontend non-agnostic in any way that blocks
Batch 5. Deferred to Batch 6, conditional on a second remote model appearing.

### 7.3 The rule that replaces vigilance

Every failure in §3 has the same shape: **a fact that must agree across a
boundary, with no mechanism forcing agreement.** The fix is not discipline, it
is making disagreement *unrepresentable or loudly wrong*:

- Capable of silently defaulting → make the match **exhaustive** (3.4)
- Capable of being a 3rd copy → put it **beside the code that enforces it** (3.3)
- Capable of having no consumer → **require a consumer** (2.5)
- Capable of drifting from the engine → **assert against the engine** (2.6)
- Capable of being forgotten in the frontend → **static invariant** (2.1)

Batches 2 and 3 are what make this recurring class unrepeatable. Batches 1, 4
and 5 clean up what the invariants find.

---

## 8. Answer to the Recurring Complaint

It recurs because the codebase has a **documented convention that is only
partially applied**, and **no gate that can tell the difference**.

`ModelsCard.tsx:92-94` already states the rule: manifest flags, never id
literals. That rule governs card selection and tier gating. It was never
extended to capability resolution, voice resolution, dirty-tracking, or
resident-model counting — the four places where a new model actually bites.
Each omission looks reasonable in isolation. None is flagged by the compiler
(`key: string`, `value: any`, `as any`), by the backend (fabricated default
caps), or by the IPC layer (`applied:false` is a success nobody reads).

The result is that "add a TTS model" costs 19 files, of which 12 are
synchronization, and the only reason the number isn't larger is that
`ModelsCard.tsx` had already been fixed once by hand.

**The fix is not vigilance. It is making each provider declare its own
capability beside the code that enforces it, making an unknown provider
unrepresentable instead of silently defaulted, and making zero model
identifiers in the frontend a machine-checked invariant rather than a
convention.**

---

## 9. New Findings From Capability Verification

Chasing the Supertonic `quality_steps` question — the last open item from the
review — turned up more than a mismatch. See §3.3 for the full truth table.

**`ProviderCaps` was wrong in 2 of 6 providers, with 1 of its 4 fields read by
nobody.** `caps_for_id` is a hand-written table that was never validated
against the engines it describes. Supertonic's `quality_steps` control is fully
implemented, persisted, and hot-updatable — and invisible in the UI. Edge's
`speed` control is implemented and the cap says otherwise, and the Speed tab
is ungated so the cap isn't enforced either.

This matters for the plan in one specific way: it **falsifies the attractive
shortcut** of deriving `caps()` from which trait setters a provider overrides.
The overrides and the declared caps disagree in *both* directions. So `caps()`
must be an explicit, hand-declared per-provider function. Two new standing
invariants follow directly (§2.5, §2.6):

- every `ProviderCaps` field must have a frontend consumer — or it does not get
  added (this is how `speed` became dead)
- declared caps must agree with engine capability — cross-checked against the
  Rust impls, not the manifest

And a new **Batch 0**, because D17/D18/D19/D20 are user-facing now:

- `DEFAULT_TTS_QUALITY_STEPS = 12` (`defaults.rs:45`) exceeds the Chatterbox
  ceiling of 10 (`tts/mod.rs:24`) — **every Chatterbox install is silently
  clamped from a default the user never chose**
- the Steps knob renders `max={8}` (`TtsVoiceManager.tsx:355`) against ceilings
  of 10 (chatterbox) and 16 (supertonic) — **the UI never displays the value
  the engine is actually using**

**Correction to an earlier claim in this thread:** I proposed that the trait's
no-op setter defaults *already* constitute the capability declaration, so
`caps()` could be inferred. The enumeration above disproves it. Corrected
design: explicit declaration, no inference.

**Not verified:** whether Supertonic's 16-step ceiling was ever measured, or
whether the control is perceptually meaningful at that range. Batch 0.1
exposes a control the UI has never shown — that is a behaviour change that
deserves its own look, not just a cap flip.

