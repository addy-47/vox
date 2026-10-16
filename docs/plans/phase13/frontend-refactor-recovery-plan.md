# Frontend Architecture Recovery Plan — Phase 13

> **Status:** Plan. No production code changed.
> **Date:** 2026-10-16
> **Scope:** `app/src/` (frontend, 66,687 LOC / 312 files) and the backend surfaces it depends on.
> **Spec status:** CONTAINS UNRESOLVED SPEC DRIFT — see §9. Wave 0 must close it before any code wave starts (AGENTS.md §4.3).

---

## 0. What this document is

Five independent audits were run against the Vox frontend. This document merges them, retires the
false positives, ranks what survives, and sequences the work.

It is **not** a "trim 50k lines" plan. The evidence does not support that framing, and §7 explains why
executing it would have wasted the effort. The plan is built around a different diagnosis: the
frontend is not fat, it is **un-enforced**. Every rule the automated invariant suite covers is clean;
every rule it does not cover is broken. That is a tooling failure, not a discipline failure.

---

## 1. Evidence Base

| Source | File | LOC | Scope |
|---|---|---:|---|
| Ledger verification | `temp/audit_verification.md` | 881 | Re-verified L-01…L-30 against HEAD; 5 new defect classes C1–C5 |
| UX critique | `temp/impeccable_critique.md` | 520 | Critique of 8 focus surfaces; health score **14/40** |
| Trim analysis | `temp/frontend_trim_analysis.md` | 790 | Dead code, duplication map, trim scenarios |
| Style guide audit | `temp/style_guide_violations.md` | 1,344 | 522 violations across 26 rules |
| Frontend/backend blame | `temp/frontend_backend_blame.md` | 631 | IPC surface audit, responsibility split |

**Parent spot-checks (read directly this session, CONFIRMED):**

| Claim | File | Verdict |
|---|---|---|
| Cloud provider endpoint hardcodes NVIDIA fallback | `services/llm/config.rs:216-221`, `core/defaults.rs:36` | **CONFIRMED** |
| `redact_setting_value` predicate misaligned with real key set | `ipc/settings.rs:70-79` vs `config/mutation.rs:14-49` | **CONFIRMED, corrected** |
| `tts` supported keys contain no cloud-credential key | `config/mutation.rs:26-33` | **CONFIRMED** |
| `realtime` classified `Hot`, no side-effect arm exists | `config/mod.rs:70-79` vs `config/dispatch.rs:356-388` | **CONFIRMED** |
| `llm.servers` / `tts.servers` carry `api_key`, hit the log | `services/llm/config.rs:43`, `services/tts/config.rs:63` | **CONFIRMED** |
| `("tts","speed")` dispatch arm unreachable | `config/dispatch.rs:464` vs `config/mutation.rs:26-33` | **CONFIRMED** |
| `ExpandableList` discards domain changes on close | `app/src/shared/ui/ExpandableList.tsx:90-97` | **CONFIRMED** |
| `trackRestartCompletion` polls a tick counter, not an event | `app/src/store/settingsStore.ts:1197-1216` | **CONFIRMED** |
| `package.json` declares `lint:dead` but ships no `lint`, no `knip` | `app/package.json:6-13` | **CONFIRMED** |

### 1.1 One correction to the input reports

`temp/frontend_backend_blame.md` finding #3 states that 6 of 7 redaction arms are dead **and** that
the 3 live secret keys are unredacted. Verified against `config/mutation.rs:14-49`:

- Predicate live arm: **`llm.cloud_keys`** — is in `supported_keys_for_domain("llm")`. Correctly redacted.
- Predicate dead arms: `tts.azure_tts`, `tts.provider`, `llm.cloud`, `llm.provider`, `stt.cloud`,
  `stt.provider` — none are in any supported-key list.
- **Unredacted and secret-bearing:** `llm.servers`, `tts.servers`, `realtime.api_keys`.

So the accurate framing is: **the predicate covers 1 of 4 secret-bearing keys, and 6 of its 7 arms are
unreachable.** The security finding stands. The arm count in the source report is wrong.

---

## 2. Retired Findings — Do Not Fix

The prior ledger carried 30 findings. Verification retired or downgraded 8. Fixing these wastes work
and adds churn.

| ID | Original claim | Retired because |
|---|---|---|
| **L-08** | Legacy `favorite_models` table survives v14 | **FALSE POSITIVE.** The DROP at `config/schema.rs:234-245` is correct — `SCHEMA_TABLE_STATEMENTS` never creates the table, so no DB can reach v14 holding it. The ledger inverted cause and effect. |
| **L-13** | Health-probe effect deps cause re-probes | **FALSE POSITIVE.** All 5 deps at `LlmConfigDesk.tsx:309-315` are primitives or a stable store function. API keys are held in local state and only reach the draft on Save, so keystrokes cannot trigger probes. |
| **L-25** | Unmemoized filter/sort in `LlmConfigDesk` | **FALSE POSITIVE.** `filteredProviders` at `LlmConfigDesk.tsx:197-227` is memoized, as are all 13 other derivations in the file. |
| **L-04** | Double `requestAnimationFrame` on modal open | **DOWNGRADED.** Real, but cosmetic and cheap. Not a P1. |
| **L-05** | Empty model id accepted at IPC boundary | **DOWNGRADED.** Guarded downstream. Defence-in-depth only. |
| **L-10** | 9 FPS on SessionPanel open | **UNVERIFIABLE, not false.** No profile was run. Neither confirm nor refute from source. Do not "fix" a number nobody reproduced. |
| **L-17** | Design-spec violations (pill indicators, nested SVG boxes) | **UNVERIFIABLE.** Requires the visual detector; it never ran. Re-audit after Wave 0 tooling exists. |
| **L-22** | `settingsStore` importer fan-in | **DOWNGRADED.** Selectors already mitigate. Structural, not a bug. |

Also: 8 of 30 cited `file:line` anchors do not resolve to the code they claim. **Every future finding
entering a ledger must have its anchor machine-checked before it is accepted.**

---

## 3. Root Cause — Why It Is a Mess

Four causes, in order of leverage.

**RC-1 — There is no enforcement net. (Primary cause.)**
`app/package.json` declares no `lint` script. Running `pnpm lint` executes `~/Android/Sdk/cmdline-tools/latest/bin/lint`
— the Android **Java** linter — and exits 2. ESLint and five plugins are installed but have no config
file, so knip reports all six as unused. There is no CI quality gate: `grep -E 'lint|knip|pnpm test'`
across `.github/workflows/` returns zero matches. The 64 green invariants never run on a PR.

The correlation is exact and is the single most useful finding in this document:

| Rule | Covered by invariant suite | Violations |
|---|---|---:|
| §4.2 Zustand selector discipline | Yes | **0** |
| §4.5 memoization | Yes | **0** |
| §4.9 context value split | Yes | **0** |
| §4.6 listener teardown | Yes | **0** |
| `any` prohibition | Yes | **0** |
| §2 Zero hardcoded text | No | **329** |
| §5 Glass elevation tokens | No | **111** |
| §2 Service layer boundary | No | **10** |
| §4.11 Pure utility isolation | No | **10** |
| §4.3 Async lifecycle guards | No | **6** |

Rules that were automated hold perfectly. Rules left as prose are all broken. Nobody is at fault —
the system never made prose enforceable.

**RC-2 — Eight policy tables are copy-pasted from the backend.**
The frontend re-declares `isRestartKey`, `SETTINGS_SCOPE_KEYS`, `MODEL_CATALOG_SCOPES`,
`CLOUD_PROVIDERS`, `REALTIME_PROVIDERS`, `TTS_MODAL_PROVIDERS`, `MIN_CONTEXT_WINDOW`, and the numeric
defaults. The backend source tables already exist at `config/mod.rs:55-110` and `config/mutation.rs:14-62`.
They were duplicated rather than published, so they drift. Today they already do — see B-09.

**RC-3 — Three checks that cannot fail read as coverage.**
Invariant 6 ("no provider id in the frontend") greps `model_groups[].id`, which matches model names
(`kokoro`), not provider names (`openai`). It passes green while `ProviderLogo.tsx` carries **50
provider-id string branches**. Invariant 5's `value={{` regex cannot catch `value={someVar}`. Invariant 1's
import regex cannot catch dynamic `import("...")` — which is exactly how `TitleBar` and `ResponsiveLayout`
violate it. **A green check that cannot fail is worse than no check, because it reads as coverage.**

**RC-4 — One backend DTO forced eleven defensive fallbacks.**
Eleven hand-written `?.servers ?? .providers?.?.servers` chains exist because the backend split provider
storage across two files and returns it in one DTO. This is not frontend sloppiness. It is a symptom
with a backend root cause.

---

## 4. Tiered Findings

Tiers are ordered by *user pain × blast radius*, not by effort.
**Effort:** S ≤ 0.5d · M 1–3d · L > 3d. **Regression risk:** probability × impact of shipping it wrong.

### TIER 0 — Security, silent wrong runtime, data loss. Fix before anything else.

| ID | Finding | Side | Evidence | Effort | Blast radius | Reg. risk | Approach |
|---|---|---|---|:---:|---|:---:|---|
| **B-01** | API keys written to the app log in cleartext | BE | `ipc/settings.rs:168` logs the mutated value; `llm.servers` (`services/llm/config.rs:43`), `tts.servers` (`services/tts/config.rs:63`) and `realtime.api_keys` all carry `api_key` and none is in the redaction predicate | **S** | Whole app log | **Very low** — pure predicate fix | Rewrite `redact_setting_value` against `supported_keys_for_domain` instead of a hand-maintained list. Delete the 6 dead arms. Add an invariant that asserts the predicate and the supported-key set agree. |
| **B-02** | Every cloud provider except `openrouter`/`groq` resolves to the NVIDIA NIM endpoint | BE | `services/llm/config.rs:216-221`, `core/defaults.rs:36` | **M** | All cloud LLM | **Medium** — needs catalog SSOT | Move the provider catalog to the backend and persist `base_url` per provider. Reject unknown provider ids at the mutation boundary instead of falling through to a default. |
| **B-03** | No IPC write path for Azure Speech / Google Cloud TTS credentials | BE | `TtsProvidersConfig.azure_tts` exists at `services/tts/config.rs:123`; `supported_keys_for_domain("tts")` = `active, last_selected, active_voice_id, voice, voice_id, servers` — no cloud-credential key. `providersCopy.tsx:132-139` advertises Azure as `statusLabel: "Ready"` | **M** | Cloud TTS | **Medium** | Add the keys to the supported set, wire `apply_tts_mutation`, then correct the frontend status label to reflect real state. |
| **B-04** | `realtime.*` classified `Hot`, but no side-effect handler exists | BE | `config/mod.rs:70-79` lists `realtime` in `ALL_HOT`; `config/dispatch.rs:356-388` has no `realtime` arm | **M** | Realtime sessions | **High** — live session behaviour | Either add the `realtime` arm to `handle_setting_side_effects`, or reclassify to `Restart`. Decide which, then make the classification table the single source. |
| **F-01** | Closing any settings modal silently discards unsaved work | FE | `ExpandableList.tsx:90-97` calls `discardDomainChanges(domainId)` on X, Escape **and** backdrop click | **S** | Every settings modal | **Medium** — changes close semantics | Make discard explicit: dirty → confirm dialog; clean → close. Wire at `LlmConfigDesk.tsx:1252`. |
| **F-02** | Selecting a TTS voice provider writes the LLM active selection | FE | `CloudProvidersModalView.tsx:1327-1337` calls the LLM-only handler passed from `LlmConfigDesk.tsx:1317`, which hardcodes `updateDraft("llm","active",…)` at `:338-342` | **S** | Cloud TTS + LLM | **Medium** | Pass a domain-bound handler instead of a hardcoded one. Resolve during Wave 3 decouple. |
| **F-03** | A restart-class edit clears the **global** autosave timer | FE | The restart-class branch clears `settingsAutoSaveTimer`, dropping a pending hot-key write and leaving a dirty card with **no footer and no button** — all six footer conditions at `SettingsCardWrapper.tsx:81-181` miss | **M** | Dictation hotkey + all settings | **Medium** | Scope the timer per domain. Add the missing footer condition. |
| **F-04** | `commitChanges` has no re-entrancy guard | FE | `isCommitting` is written but nothing refuses a second call; the `only`-scoped draft restore is computed from the entry-time snapshot, so overlapping commits erase each other | **S** | All settings writes | **Medium** | Guard on `isCommitting`, restore from the post-commit draft. Replaced wholesale by `apply_settings_batch` in Wave 2. |

### TIER 1 — Broken features and correctness

| ID | Finding | Side | Evidence | Effort | Blast radius | Reg. risk | Approach |
|---|---|---|---|:---:|---|:---:|---|
| **F-05** | "Restarting Engine" spinner is a fabricated fixed 12 s timer | FE/BE | `settingsStore.ts:1197-1216` counts ticks; its doc comment claims it polls `SettingsUpdated` — it does not. Backend owns `restart_in_flight` (`core/state.rs:174`) and exposes it nowhere | **S** | Every restart | **Low** | Add `get_restart_status`. Replace the timer. **Highest leverage single fix in the plan** — also retires C5-b. |
| **F-06** | Remote TTS custom-server probe is CORS-blocked | FE | `CloudProvidersModalView.tsx:426` issues `fetch` from the webview. Backend already has a stricter probe at `services/health.rs:238-254` and `check_provider_health` already accepts an unsaved config payload | **S** | Remote TTS | **Low** | Call the backend probe with a `TtsProviderConfig` payload. Also retires ledger L-02. |
| **F-07** | There is no benchmark feature | FE/BE | 80 Tauri commands, none benchmarking. The "Benchmark" button at `RemoteModelCard.tsx:244-257` calls `probe_model_capabilities`. `tts_rtf` is computed by 5 TTS providers and never surfaced. `invariants.test.ts:1012-1045` actively enshrines the mislabel | **M** | Model catalog | **Medium** | Decide: ship a real `benchmark_model` command, or relabel the button to "Probe capabilities". **User decision — see §9 Q1.** |
| **F-08** | Probe/benchmark results are invisible | FE | No toast, no timestamp, no result row. Failure reachable only via hover tooltip at `RemoteCatalogSection.tsx:351-365`, which lacks `mobileTapEnabled` while its sibling has it at `:407`. `probeAgeDays` has zero call sites | **S** | Model catalog | **Low** | Persistent inline "Measured live · 2m ago" row on the card. The tooltip-only design is why this reads as broken on touch. |
| **B-09** | Live-wrong numeric defaults | BE/FE | `DEFAULT_LLM_MAX_OUTPUT_TOKENS=120` vs UI `?? 300`; `DEFAULT_LLM_TEMPERATURE=0.6` vs `?? 0.7`; `DEFAULT_TTS_SPEED=1.05` vs `?? 1.0`; `DEFAULT_LLM_THREADS=4` vs `max(2, hardwareConcurrency−2)` | **S** | All model params | **Low** | Delete the UI fallbacks once the backend publishes defaults in `get_settings_contract`. |
| **B-10** | `update_model_parameters` validates nothing and does read-modify-write without a transaction | BE | `persistence/models.rs:122-197` | **M** | Model params | **Medium** | Validate the parameter schema, resolve against the model metadata table, write in one transaction. Linked to ledger L-01/L-28. |
| **F-09** | 5 confirmed races | FE | Includes missing `isMounted` guards (6 violations of §4.3) | **M** | Settings + memory | **Medium** | Fixed by the ESLint config in Wave 0, then sweep. |
| **F-10** | Service layer boundary violated — 10 raw `invoke`/`fetch` in components | FE | Sweep required; `CloudProvidersModalView.tsx:426` is the worst | **M** | Settings | **Low** | Mechanical. Enforce with an ESLint `no-restricted-import` rule. |
| **F-11** | 2 `src/pages/` files do business logic | FE | Violates §2 page responsibility | **M** | 2 pages | **Medium** | Extract to `services/` + `store/`. |
| **F-12** | 12 duplicated provider/model logic blocks | FE | Between `LlmConfigDesk.tsx` and `CloudProvidersModalView.tsx:1465-1578`. Divergence already exists: `LlmConfigDesk.tsx:336` omits the `clearRemoteModelsError()` that `CloudProvidersModalView.tsx:1509` calls | **M** | Settings | **Medium** | Collapse in Wave 3. |
| **F-13** | 4 different vocabularies for one provider | FE | Raw id in `PipelineProvidersDesk.tsx:73-87`, name in modal cards, `Cloud Gateway` in the footer, `Self-Hosted · Connected` elsewhere. TTS Edge has two ids (`"microsoft"` vs `"edge_speech"`) so selection never highlights across surfaces. Google/Azure are dead grey boxes in the desk while live in the modal | **M** | Settings IA | **Medium** | One provider registry, one display function. Blocked on Wave 2's catalog. |

### TIER 2 — Enforcement and architecture debt

| ID | Finding | Evidence | Effort | Blast radius | Reg. risk | Approach |
|---|---|---|:---:|---|:---:|---|
| **S-01** | No ESLint config; `pnpm lint` runs Android's Java linter and exits 2. 6 devDeps dead | `package.json:6-13`; `command -v lint` → Android SDK | **S** | Repo-wide | **Very low** | Add `eslint.config.js` + the `lint` script. ~1 hour. Revives six dead dependencies. **Do this first.** |
| **S-02** | No CI quality gate | `.github/workflows/` — 3 tag-triggered releases, 0 quality steps | **S** | Repo-wide | **Very low** | Add lint + knip + vitest + `cargo nextest` steps. |
| **S-03** | 329 hardcoded strings across 62 files; 114 already exist verbatim in `src/data/` | §2 violation | **M** | Repo-wide | **Low** | Extract to `src/data/`. Enforce with `no-literal-string` in JSX. |
| **S-04** | 111 ad-hoc `shadow-[…]` values | §5 violation | **M** | Settings | **Low** | Promote to elevation tokens. `no-arbitrary-value` rule. |
| **S-05** | 8 policy tables duplicated from the backend | §3 RC-2 | **M** | Settings | **Medium** | Delete all 8 after `get_settings_contract` lands. **~180 LOC removed.** |
| **S-06** | `shared/lib/dateTime.ts` defeated by a re-export route-around | `historyService.ts:193` re-exports it; the module's only importer is the route-around; all 10 real consumers import from the service, dragging `@tauri-apps/api` into their chunks | **S** | 10 files | **Low** | Delete the re-export. Pure win. |
| **S-07** | 149 duplicated strings in `src/data/` | — | **M** | Settings | **Low** | Collapse to the SSOT. |
| **S-08** | 3 vacuous invariants read as coverage | RC-3 | **M** | Test suite | **Low** | Repoint Invariant 6 at a provider registry. Replace the Invariant 5 regex with ts-morph. |
| **S-09** | `ProviderLogo.tsx` — 676 lines, 50 `norm.includes(…)` branches | — | **M** | Provider display | **Medium** | Data-driven registry lookup. Delete unused `@lobehub/icons` dep. |
| **S-10** | Unreachable `("tts","speed")` dispatch arm | `config/dispatch.rs:464` — `speed` is not a supported tts key | **S** | Backend | **Very low** | Either add `speed` to supported keys or delete the arm. Ledger L-09. |

### TIER 3 — Dead code and duplication

| ID | Finding | LOC | Effort | Reg. risk | Approach |
|---|---|---:|:---:|:---:|---|
| **D-01** | 10 fully dead files + ~116 LOC of dead exports. Largest: `shared/ui/VoiceCarousel.tsx` (591) — barrel-only importer, and its one apparent grep hit is a *different* component (`RealtimeVoiceSelector as VoiceCarousel`) | **1,274** | **S** | **Low** | Delete after S-01 lands so knip can prove it. |
| **D-02** | `ProvidersHeader.tsx` (342) is a line-for-line clone of `CatalogHeader.tsx` (398), single consumer, graph-confirmed | 342 | **S** | **Low** | Delete, redirect the one consumer. |
| **D-03** | `CloudProvidersModalView.tsx` and `RemoteModelsModalView.tsx` share 75 byte-identical lines including the same code comment. Both hand-roll a toolbar `CatalogHeader` already provides | 75 | **S** | **Low** | Collapse into `CatalogHeader`. |
| **D-04** | Unused dependencies: `@lobehub/icons`, `shadcn`, `@tauri-apps/plugin-opener`, `@tauri-apps/plugin-updater` | bundle | **S** | **Low** | Remove after confirming zero imports. |

---

## 5. Frontend vs Backend — Who Owns This

**Verdict: ≈ 69% backend contract, ≈ 31% frontend architecture.**
Method: 20 defects classified by share, weighted by share rather than counted per bucket. Under
equal-weight bucket counting the answer is 75% backend, so 69% is the conservative reading.

**Six defects are ≥80% backend and no frontend rewrite can touch them:** B-01, B-02, B-03, B-04,
B-05/logging, and the missing probe/benchmark IPC surface.

**The dominant single cause:** the cloud provider catalog lives **only** in the frontend
(`providersCopy.tsx:12-50`, 37 providers with base URLs) while the backend knows two plus a default
(B-02). No URL is persisted and none is derivable server-side. The frontend probes with *its own* URL,
so the probe passes and the card reads "Connected" — while the runtime sends the key to NVIDIA. **The
UI actively misreports the state of a security-relevant misconfiguration.**

**The highest-leverage backend addition:** `get_settings_contract()`. `config/mod.rs:55-110` and
`config/mutation.rs:14-62` are *already* the source tables — they only need publishing instead of
duplicating. One command retires `isRestartKey`, `SETTINGS_SCOPE_KEYS`, `MODEL_CATALOG_SCOPES`,
`CLOUD_PROVIDERS`, `REALTIME_PROVIDERS`, `TTS_MODAL_PROVIDERS`, `MIN_CONTEXT_WINDOW` and the four wrong
defaults — ~180 lines and 8 duplicated tables.

Second: `apply_settings_batch` removes 34 lines of hand-rolled transaction emulation in `commitChanges`.
`resolve_memory_revisions` already proves the pattern.

**Six defects stay frontend-only under any backend improvement:** the fabricated `speed_range` fallback
(`TtsVoiceManager.tsx:69`, which contradicts the stated rule 60 lines away at `settingsService.ts:54-56`),
the discarded `error_type` (1 consumer out of 81 commands), the mislabeled Benchmark, order-sensitive
`JSON.stringify` dirty checks, and the provider-config builder triplicated across `LlmConfigDesk.tsx:115-140`
and `ModelsCard.tsx:113-140`.

---

## 6. The UX Answer to "Why Is It So Messy"

Health score **14/40**. The five things that make it feel broken:

1. **Losing work silently.** F-01. You change something, press Escape, and it is gone. No warning.
2. **A voice picker that changes your LLM.** F-02. Selecting a TTS provider overwrites the active model.
3. **Restart with no explanation.** Saving an API key restarts the engine immediately
   (`LlmConfigDesk.tsx:369`; restart keys at `settingsStore.ts:507-522`). Changing the *provider* demands
   a manual "Apply & Restart" that lives on a *different card*. No reason, no scope, no loss warning.
   The only restart strings in the app are `settingsCopy.ts:160,162`.
4. **A benchmark that reports nothing.** F-07 + F-08. And the probe spinner (F-05) runs a fixed 12 s
   regardless of what the engine did.
5. **One provider, four names.** F-13.

Click counts, measured: add provider + key + activate = **8 clicks, 2 navigations, 2 engine restarts.**
Pick a model = 4. Probe = 3 with no confirmation. The desk form commits *every pending setting in the app*
(`LlmConfigDesk.tsx:519,582`) where the modal correctly scopes its commit (`CloudProvidersModalView.tsx:451`).

None of the 64 passing vitest tests touch these components.

---

## 7. The Trim Premise Is Wrong — Read This Before Wave 5

You asked to trim the frontend because 50k+ lines "doesn't make sense." The measurement says otherwise:

| Scenario | LOC removed | % of total |
|---|---:|---:|
| Conservative (dead code only) | 1,274 | 1.9% |
| Realistic (dead + duplication collapse) | ~2,900 | 4.3% |
| Aggressive (ZBC rewrite incl. god files) | 3,764 | **5.7%** |

`shared/ui/` is **7.1%** of the code, not the dumping ground you suspected. The duplication is **not**
"the rest of the app hand-rolling what `shared/ui` provides" — it is four provider/settings surfaces
duplicating *each other*.

**The 66k lines are feature work, not waste.** Chasing line count would have consumed the entire budget
and left every bug in §4 still broken. Waves 0–2 net **−2,900 LOC for +400 LOC of tests** — a
correctness win, not a size win. The trim is a *consequence* of the decoupling, not a project.

One genuine surprise: dead code is only 2.1% of the total, but `shared/ui/VoiceCarousel.tsx` alone is
591 dead lines. **Dead code grew because the gate that would have caught it (`lint:dead`) has never
worked** — `package.json:9` declares it, `knip` is absent from `devDependencies`.

---

## 8. Execution Waves

Each wave ends with a gate. A failed gate stops the wave. Waves 0–4 are strictly ordered —
later waves depend on earlier ones landing.

### Wave 0 — Build the net. Resolve spec drift. **M, low risk, blocks everything**
1. **Adjudicate spec drift** (§9). Nothing else starts until this closes — AGENTS.md §4.3.
2. Add `eslint.config.js` + `lint` script. Fixes S-01, revives 6 dead deps. ~1 hour.
3. Add `knip.json`, install `knip` properly, wire into CI. Fixes S-02.
4. Rewrite the 3 vacuous invariants. Fixes S-08.
> **Gate:** `pnpm lint` exits 0 and actually runs ESLint; `pnpm lint:dead` produces a report; CI runs on a PR.

### Wave 1 — Tier 0 backend. **M, very low regression risk**
B-01, B-05 (log redaction), S-10, B-04 (pick Hot or Restart, then implement), B-03.
> **Gate:** dual-target `./scripts/verify-targets.sh` clean; `cargo nextest run --release --test-threads=1` green;
> new test asserts the redaction predicate and supported-key set agree.

### Wave 2 — Publish the contract. **L, medium risk**
`get_settings_contract` · `apply_settings_batch` · `get_restart_status` · backend provider catalog +
persisted `base_url` · real probe/benchmark commands · write-path for Azure/Google TTS credentials.
> **Gate:** contract command returns the source tables unmodified; every frontend table has a mechanical
> consumer of the contract; new test asserts `isRestartKey` and `classify_known` agree.

### Wave 3 — Decouple UI from business logic. **L, medium-high risk**
This is the wave you asked to run first. It cannot run first: with no backend contract, "decoupling" means
moving duplicated business logic into frontend hooks and cementing the drift. Wave 2 gives it something to bind to.
1. Delete the 8 duplicated tables (S-05) — consume the contract.
2. Fix the 10 service-layer violations (F-10) and the 2 page-responsibility violations (F-11).
3. Collapse the 12 duplicated provider/model blocks (F-12).
4. Resolve F-02, F-13 via a single provider registry.
> **Gate:** `grep` for the deleted table names returns zero hits outside the contract consumer;
> `pnpm build` and `pnpm test` green; invariant asserts no component imports policy tables.

### Wave 4 — Frontend bug fixes. **M, medium risk**
F-01, F-03, F-04, F-05, F-06, F-08, F-09 races, and the restart UX copy/placement (F-05's parent complaint).
> **Gate:** new interaction tests for discard-on-dirty, restart-once semantics, and probe result rendering.
> **This is the wave that makes the app feel fixed.** It lands after Wave 3 so the fixes go into clean code.

### Wave 5 — Trim. **S-M, low risk**
D-01 (1,274 LOC), D-02 (342), D-03 (75), D-04, S-06, S-09.
> **Gate:** knip reports no newly-dead exports; build + test green.
> **Runs after Wave 3** because collapsing duplication changes what is dead. Running it first deletes the wrong things.

### Wave 6 — Zero style violations. **M, low risk**
S-03 (329 strings), S-04 (111 shadows), S-07 (149 strings). All now mechanically enforced by Wave 0's config,
so this is a mechanical sweep with a compiler-grade net.
> **Gate:** `pnpm lint` reports 0 violations of every rule in `frontend-style-guide.md`.

### Wave 7 — God file decomposition. **L, HIGH regression risk. Last.**
`CloudProvidersModalView.tsx` (1,578), `useMemoryGraphScene.ts` (1,349), `LlmConfigDesk.tsx` (1,338),
`settingsStore.ts` (1,269), `AdvancedOrb.tsx` (1,027). Only safe once Waves 0–6 hold the architecture steady.
> **Gate:** render tests for `shared/ui` (currently **zero** for a 4,688-LOC design system) before touching any of them.

---

## 9. Open Questions — Need Your Decision

These change scope. Per AGENTS.md §1 and §4.3 I have not guessed.

**Q1 — Benchmark (F-07).** Ship a real `benchmark_model` IPC command, or relabel the button to
"Probe capabilities" and delete `tts_rtf`? *Recommend:* relabel. A real benchmark is new feature work;
the mislabel is what makes it feel broken. **Your call.**

**Q2 — Restart semantics.** Should an API-key save restart the engine immediately (current behaviour,
`settingsStore.ts:507-522`), or should key saves become hot-reload with "Apply & Restart" reserved for
active-provider/model changes? *Recommend:* hot-reload keys, restart only on selection change. This is
the single biggest UX win available and it changes what "dirty" means.

**Q3 — Probe affordance (F-08).** Persistent inline "Measured live · 2m ago" row, or toast + timestamp
in the existing tooltip? *Recommend:* inline row. The tooltip-only route is why this reads as broken on touch.

**Q4 — `realtime` policy (B-04).** Add the missing side-effect arm, or reclassify to `Restart`?
*Recommend:* reclassify to `Restart` first — it is the honest answer for provider/voice changes and
removes a live-wrong-state bug with one table edit.

**Q5 — Spec drift items (§10).** These need adjudication, not a code fix.

---

## 10. Spec Drift — Must Resolve in Wave 0

| Item | Detail | Action |
|---|---|---|
| `frontend-style-guide.md` §4.5 | Mandates memoizing `SliderField`, `Button`, `SearchInput`. **None of them exist.** | Update the guide to name the primitives that do exist |
| `frontend-style-guide.md` §2 | Instructs agents to run `pnpm lint` after every change. **The script does not exist** and the command resolves to Android's Java linter. | Fix per Wave 0 |
| `frontend-style-guide.md` §3 | References `src/hooks/` as the extraction target. **The directory does not exist.** Logic lives in `src/shared/hooks/`. | Reconcile path |
| `backend-style-guide.md:87` | References `docs/specs/provider-model-catalog-spec.md`. **The file does not exist.** | Write it or delete the reference |
| `docs/specs/ipc-spec.md` | Documents 61 of 81 commands. The entire probe / provider-health surface is undocumented. | Document, and add a CI check asserting every name in `lib.rs:733-823` appears in the spec |
| `docs/specs/config-refactor-plan` | Referenced as a spec; the file is at `docs/plans/phase13/config-refactor-plan.md`. | Fix the reference |
| §4.3 spec-alignment | `providersCopy.tsx:132-139` advertises Azure TTS as `Ready` when no IPC write path exists (B-03). | Resolve during Wave 1 |

---

## 11. What I Disagree With In The Framing

Three challenges, stated plainly.

**1. "Trim the frontend" is the wrong project.** §7: the ceiling is 5.7%. It would consume the budget
and leave every Tier 0 and Tier 1 bug exactly where it is. The trim is a by-product of Waves 3 and 5.

**2. "Decouple UI from business logic first" cannot run first.** With no backend contract, decoupling
means relocating duplicated business logic into frontend hooks. That is the same drift, one layer down,
and it is harder to undo. The contract has to exist first — otherwise you have decoupled the UI from a
copy of the rules.

**3. "50k lines doesn't make sense" — the line count is not the disease.** The disease is that
522 rules were unenforced and 8 backend tables were copied. Line count is the symptom that is cheap to
count and expensive to chase.

**One thing I will not compromise on:** waves 1 and 2 are backend work inside a "frontend refactor."
B-01 logs your API keys in cleartext and B-02 sends Anthropic keys to NVIDIA. Those do not wait for a
UI architecture project.

---

## 12. Appendix — Verification Commands

```bash
# Wave 0
cd app && pnpm lint && pnpm lint:dead          # both must exist and exit 0 after Wave 0
cd app && pnpm test                             # 64 baseline

# Every wave, backend
cd app/src-tauri && ./scripts/verify-targets.sh   # dual-target compile gate, 0 errors/warnings
cd app/src-tauri && RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo nextest run --release --test-threads=1 --no-fail-fast    # ~45.5s, run strictly sequentially

# Proof that no component reimplements policy
cd app && grep -rn "isRestartKey\|CLOUD_PROVIDERS\|MODEL_CATALOG_SCOPES\|SETTINGS_SCOPE_KEYS" src/

# Proof the service layer boundary holds
cd app && grep -rn "invoke\|fetch(" src/shared/components src/pages src/wizard | grep -v "\.test\."
```
