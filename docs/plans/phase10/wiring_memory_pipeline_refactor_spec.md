# Phase 10 — Dead & Uncalled Functions Resolution Master Spec

> **Status:** ACTIVE Socratic Review & Architectural Resolution Specification  
> **Location:** `docs/plans/phase10/uncalled_functions_resolution_spec.md`  
> **Source Audit:** `dead_code_audit.md` (ca250fa9-7d0c-424b-9f59-4f69d0b884ab)  
> **Master Sprints Checklist:** [`uncalled_functions_sprints.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/uncalled_functions_sprints.md)  
> **Methodology:** 1 function per sprint → (1) Exploration, (2) `/grill-me` Socratic interview, (3) Resolution entry in this SSOT spec.

---

## Sprint Index & Resolution Summary

| Sprint | Function | Module & Line | Resolution Category | Finalized Action |
|:---|:---|:---|:---:|:---|
| **01** | [`count_words`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/utils.rs#L64) | `services/utils.rs:64` | **Dead Code** | **Delete completely.** Callers use idiomatic `s.split_whitespace().count()`. |
| **02** | [`to_friendly_hinglish`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/utils.rs#L153) | `services/utils.rs:153` | **Dead Code** | **Delete completely.** Pipeline uses `transliterate_if_hi(text, is_final, settings.stt.transliterate_enabled)`. |
| **03** | [`set_max_context_tokens`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L107) | `services/memory/working_memory.rs:107` | **Missing Wiring** | **Retain & Wire.** Hook into `ipc/settings/mutation.rs` on `llm.context_window` / provider changes and add unit test. |
| **04** | [`load_identity_into_system_prompt`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L118) | `services/memory/working_memory.rs:118` | **Missing Wiring** | **Retain & Wire.** Hook into `start_session` across assistant domains at initial session boot. Frozen mid-session for 100% KV cache stability. |
| **05** | [`new_session`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L155) | `services/memory/working_memory.rs:155` | **Missing Wiring** | **Retain & Wire.** Call in-place on `start_session` (new session ID) and on conversation resets, followed by `load_identity_into_system_prompt`. |
| **06** | [`update_system_prompt`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L197) | `services/memory/working_memory.rs:197` | **Missing Wiring** | **Retain & Wire.** Hook into `ipc/settings/mutation.rs` under `("persona", "modular_prompt")` for hot persona updates while preserving `<user_profile>`. |
| **07** | [`push_assistant_turn`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L229) | `services/memory/working_memory.rs:229` | **Missing Wiring** | **Retain & Wire.** Accumulate streamed tokens and push full assistant turn on `on_llm_finished` across all assistant domains. Restores multi-turn memory. |
| **08** | [`build_context`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L338) | `services/memory/working_memory.rs:338` | **Missing Wiring** | **Retain & Wire.** Hook into `on_transcript_final` in `modular_passive.rs` and `modular_ptt.rs` with critical threshold compaction + transition speech playback. |
| **09** | [`try_trigger_opportunistic`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L511) | `services/memory/working_memory.rs:511` | **Missing Wiring** | **Retain & Wire.** Hook into `on_playback_finished` (Ready state), `pause_session` (Paused state), and 30s idle sweeps to pre-compact at 65%–85% utilization. |
| **10** | [`commit_opportunistic`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L536) | `services/memory/working_memory.rs:536` | **Missing Wiring** | **Retain & Wire.** Atomic commit callback paired with `try_trigger_opportunistic`; guarantees race-safe context compaction without corrupting history. |
| **11** | [`on_pipeline_idle`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L582) | `services/memory/working_memory.rs:582` | **Dead Code** | **Delete completely.** Empty NOOP stub; idle processing is owned by `try_trigger_opportunistic` and `memory_worker.rs`. |
| **12** | [`latest_summary`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L599) | `services/memory/working_memory.rs:599` | **Dead Code** | **Delete completely.** Superceded by structured `session_compaction_contexts` and `build_narrative_context_chain`. |
| **13** | [`l2_normalize`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/embedder.rs#L265) | `services/memory/embedder.rs:265` | **Eval/Test Helper** | **Relocate to `evals/` & delete from `src/`.** Keep `src/` 100% production-wired; production uses `l2_normalize_in_place`. |
| **14** | [`classify_pair`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs#L353) | `services/memory/classifiers/intra_edge_classifier.rs:353` | **Dead Code** | **Delete completely.** Redundant wrapper; production Stage 3 canonically uses `classify_batch(&pairs)`. |
| **15** | [`get_calibrated_class_mapping`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs#L417) | `services/memory/classifiers/intra_edge_classifier.rs:417` | **Dead Code** | **Delete completely.** Internal NliEngine detail encapsulated inside `classify_batch`. |
| **16** | [`get_calibrated_class_mapping_strings`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs#L423) | `services/memory/classifiers/intra_edge_classifier.rs:423` | **Dead Code** | **Delete completely.** Internal NliEngine string label helper; uncalled. |
| **17** | [`reset_samples_ingested`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/playback.rs#L158) | `services/audio/playback.rs:158` | **Dead Code** | **Delete completely.** Also delete `total_samples_ingested` getter & atomic field to streamline `ingest_chunk` audio path. |
| **18** | [`set_mode`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/router.rs#L88) | `services/audio/router.rs:88` | **Dead Code** | **Delete completely.** (Along with entire obsolete `services/audio/router.rs`). Phase 10 audio flows through `VadActor` to domain pipelines. |
| **19** | [`start_realtime`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/router.rs#L95) | `services/audio/router.rs:95` | **Dead Code** | **Delete completely.** Obsolete router method; streaming is managed by `AudioBridge` / `RealtimeEngine`. |
| **20** | [`stop_realtime`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/router.rs#L105) | `services/audio/router.rs:105` | **Dead Code** | **Delete completely.** Obsolete router method; session stoppage is managed by `RealtimeEngine`. |
| **21** | [`handle_cancel`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/dictation/controller.rs#L25) | `services/dictation/controller.rs:25` | **Dead Code** | **Delete completely.** (Along with entire redundant `services/dictation/controller.rs`). Hotkeys/tray route directly to `services::pipeline::dictation`. |
| **22** | [`barge_in`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/engine.rs#L93) | `services/realtime/engine.rs:93` | **Missing Wiring** | **Retain & Wire.** Wire into `realtime_ptt::handle_ptt_start` to send server interrupt + cancel playback. Wire `VoxEvent::Cancelled` into `realtime_passive`. |
| **23** | [`ingest_audio_i16`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/realtime_ptt.rs#L36) | `services/pipeline/realtime_ptt.rs:36` | **Dead Code** | **Delete completely.** Redundant overload; all PTT domains standardize on `ingest_audio(&[f32])`. |
| **24** | [`set_speech_detected`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/realtime_ptt.rs#L48) | `services/pipeline/realtime_ptt.rs:48` | **Test Seam / Dead Code** | **Delete from `src/`.** Tests trigger `handle_event(..., VoxEvent::SpeechStart)` naturally without backdoor test setters. |
| **25** | [`is_speech_detected`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/realtime_ptt.rs#L53) | `services/pipeline/realtime_ptt.rs:53` | **Test Seam / Dead Code** | **Delete from `src/`.** Tests assert on observable pipeline behavior (mock engine chunk counts & PTT status). |
| **26** | [`probe_top_k`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/probe.rs#L30) | `services/llm/probe.rs:30` | **Dead Code** | **Delete completely.** (Along with entire `services/llm/probe.rs`). `OpenAiCompatProvider` formats parameters per backend kind. |
| **27** | [`static_unsupported`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/capabilities.rs#L40) | `services/llm/capabilities.rs:40` | **Dead Code** | **Delete completely.** (Along with entire obsolete `services/llm/capabilities.rs`). `capability_probe.rs` is canonical SSOT. |
| **28** | [`get_or_insert_default`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/capabilities.rs#L117) | `services/llm/capabilities.rs:117` | **Dead Code** | **Delete completely.** Redundant method in obsolete `capabilities.rs`. |
| **29** | [`format_prompt`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L86) | `services/llm/llama_cpp.rs:86` | **Dead Code** | **Delete completely.** Superseded by multi-turn `format_conversation(&messages)`. |
| **30** | [`run_loop`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L345) | `services/llm/llama_cpp.rs:345` | **Dead Code** | **Delete completely.** Superseded by unified `spawn_llm_worker` in `services/llm/actor.rs`. |
| **31** | [`validate_wav`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/voices.rs#L61) | `ipc/voices.rs:61` | **Dead Code** | **Delete completely.** Unregistered IPC command; validation is executed inside `add_voice_from_file`. |
| **32** | [`preview_voice`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/voices.rs#L232) | `ipc/voices.rs:232` | **Dead Code** | **Delete completely.** (Along with `synthesize_preview_clip`). Unregistered in `lib.rs` and uncalled by UI. |
| **33** | [`update_theme`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/mutation.rs#L286) | `ipc/settings/mutation.rs:286` | **Dead Code** | **Delete completely.** Legacy convenience command; theme changes route through `update_setting("appearance", "theme", ...)`. |
| **34** | [`fetch_intra_subfloor_candidates`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/queries.rs#L381) | `persistence/queries.rs:381` | **Eval Seam** | **Relocate to `evals/` & Delete from `src/`.** 0 callers in `src/`; used exclusively in `eval_memory_pipeline.rs`. |
| **35** | [`fetch_inter_subfloor_candidates`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/queries.rs#L453) | `persistence/queries.rs:453` | **Eval Seam** | **Relocate to `evals/` & Delete from `src/`.** 0 callers in `src/`; used exclusively in `eval_memory_pipeline.rs`. |
| **36** | [`get_model`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/manifest.rs#L70) | `setup/manifest.rs:70` | **Dead Code** | **Delete completely.** 0 callers; model verification iterates `model_groups` directly. |
| **37** | [`nli_inverse_edge`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs#L288) | `core/constants.rs:288` | **Dead Code** | **Delete completely.** Redundant subset of `inverse_edge_for_relation` used in `stage3_eval.rs`. |
| **38** | [`latency_report`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/metrics.rs#L60) | `core/metrics.rs:60` | **Dead Code** | **Delete completely.** (Along with entire obsolete `core/metrics.rs`). Superseded by `src/monitoring/`. |
| **39** | [`write_artifact`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/bench_reporter.rs#L41) | `utils/bench_reporter.rs:41` | **Dead Code** | **Delete completely.** (Along with entire `src/utils/bench_reporter.rs`). |
| **40** | [`save_report`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/bench_reporter.rs#L46) | `utils/bench_reporter.rs:46` | **Dead Code** | **Delete completely.** Part of obsolete `bench_reporter.rs`. |
| **41** | [`vox_dir`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/paths.rs#L117) | `utils/paths.rs:117` | **Dead Code** | **Delete completely.** Standalone wrapper uncalled; callers use `paths::get().root`. |
| **42** | [`logs_dir`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/paths.rs#L125) | `utils/paths.rs:125` | **Dead Code** | **Delete completely.** Standalone wrapper uncalled; callers use `paths::get().logs`. |
| **43** | [`voices_dir`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/paths.rs#L152) | `utils/paths.rs:152` | **Dead Code** | **Delete completely.** Standalone wrapper uncalled; callers use `paths::get().voices` (and `temp_dir`). |
| **44** | [`deserialize_value_resilient`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/json.rs#L142) | `utils/json.rs:142` | **Dead Code** | **Delete completely.** 65-line uncalled Serde custom visitor. |
| **Special** | `transliterate_if_hi` | `services/utils.rs:115` | **Wiring Refactor** | **Relocate to `services/translit.rs` & Wire.** Gate Devanagari text streamed to UI and OS typing in `dictation.rs`, `modular_ptt.rs`, `modular_passive.rs`. |
| **Special** | `stitch_transcripts` | `services/utils.rs:288` | **Wiring Refactor** | **Relocate to `services/stt/` & Wire.** Wire into `services/stt/providers/embedded.rs::transcribe_chunk` for sliding-window transcript accumulation. |
| **Special** | `services/utils.rs` | Entire File | **Dead Code Purge** | **Delete file completely.** Purge `should_flush`, `count_words`, `to_friendly_hinglish`; move `is_devanagari` to `translit.rs`. |

---

## Detailed Sprint Resolutions

### Sprint 01 — `count_words`
- **Location:** `app/src-tauri/src/services/utils.rs:64-66`
- **Signature:** `pub fn count_words(s: &str) -> usize`
- **Implementation:**
  ```rust
  #[inline]
  pub fn count_words(s: &str) -> usize {
      s.split_whitespace().count()
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `should_flush` receives `word_count` precalculated as an argument from callers.
- **Architectural Trade-offs & Decision:**
  - Word counting in standard Rust is trivially expressed as `s.split_whitespace().count()`.
  - Wrapping a single stdlib method in a separate public function adds symbol bloat without semantic abstraction.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `count_words` from `services/utils.rs` and update documentation references in `docs/backend.md`.
- **Pipeline Invariants:** Zero ripple across audio/VAD/STT/LLM/TTS/Playback, zero IPC changes, zero lock changes.

---

### Sprint 02 — `to_friendly_hinglish`
- **Location:** `app/src-tauri/src/services/utils.rs:153-155`
- **Signature:** `pub fn to_friendly_hinglish(text: &str) -> String`
- **Implementation:**
  ```rust
  /// Converts Hindi text to friendly phonetic Hinglish using full final transliteration.
  pub fn to_friendly_hinglish(text: &str) -> String {
      transliterate_if_hi(text, true, true)
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Hardcodes `is_final = true` and `transliterate_enabled = true`.
- **Architectural Trade-offs & Decision:**
  - In the live voice pipeline (`dictation.rs`, `modular_passive.rs`, `stt/actor.rs`), Hindi-to-Roman transliteration must strictly adhere to the user's runtime toggle (`settings.stt.transliterate_enabled`) and turn completion status (`is_final`).
  - `transliterate_if_hi(text, is_final, settings.stt.transliterate_enabled)` is the canonical SSOT entry point that respects boundary detection and settings.
  - `to_friendly_hinglish` bypasses user configuration and is completely unused.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `to_friendly_hinglish` from `services/utils.rs`.
- **Pipeline Invariants:** Transliteration in domain pipelines continues to use `transliterate_if_hi`. Zero ripple across stages.

---

### Sprint 03 — `set_max_context_tokens`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:107-115`
- **Signature:** `pub fn set_max_context_tokens(&mut self, max_tokens: usize)`
- **Implementation:**
  ```rust
  /// Updates the maximum allowable context token budget.
  pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
      if max_tokens > 0 {
          self.max_context_tokens = max_tokens;
          log::info!(
              "[WorkingMemory] Updated max_context_tokens to {}",
              max_tokens
          );
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `ConversationManager` is initialized once at startup with `initial_ctx_size`. When users update `llm.context_window` or change LLM providers at runtime in settings, `ConversationManager` retains the initial token budget instead of syncing.
- **Architectural Trade-offs & Decision:**
  - Preserving conversational history across setting mutations and model switches is a core UX requirement. Re-instantiating `ConversationManager` would drop active messages and turn context.
  - Dynamically updating `max_context_tokens` via `set_max_context_tokens` allows `ConversationManager` to accurately enforce FIFO compaction and soft/critical thresholds (`0.65` / `0.85`) under newly configured model contexts.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Hook `state.conversation_manager.lock().set_max_context_tokens(val as usize);` in `app/src-tauri/src/ipc/settings/mutation.rs` when `("llm", "context_window")` or active LLM provider configurations change.
  2. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Stage ordering & thread models: Unchanged.
  - Audio hot path: Unaffected (zero locks on audio callback).
  - IPC Contract: Unchanged.

---

### Sprint 04 — `load_identity_into_system_prompt`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:118-152`
- **Signature:** `pub async fn load_identity_into_system_prompt(&mut self, conn: &turso::Connection) -> anyhow::Result<()>`
- **Implementation:**
  ```rust
  /// Preloads active Identity facts into the base system prompt block.
  pub async fn load_identity_into_system_prompt(
      &mut self,
      conn: &turso::Connection,
  ) -> anyhow::Result<()> {
      let active_identities =
          crate::persistence::queries::fetch_all_active_identity(conn).await?;
      if !active_identities.is_empty() {
          let mut base_prompt = self.system_prompt.content.clone();
          if let Some(start_idx) = base_prompt.find("\n\n<user_profile>") {
              base_prompt.truncate(start_idx);
          } else if let Some(start_idx) = base_prompt.find("<user_profile>") {
              base_prompt.truncate(start_idx);
          }

          let identity_lines: Vec<String> = active_identities
              .iter()
              .map(|f| format!("- {}", f.fact))
              .collect();
          let user_profile_block = format!(
              "\n\n<user_profile>\n{}\n</user_profile>",
              identity_lines.join("\n")
          );
          let updated_content = format!("{}{}", base_prompt.trim_end(), user_profile_block);
          self.system_prompt.content = updated_content.clone();
          if !self.messages.is_empty() && self.messages[0].role == Role::System {
              self.messages[0].content = updated_content;
          }
          self.total_token_count = estimate_tokens(&self.system_prompt.content);
          log::info!(
              "[WorkingMemory] Successfully preloaded {} Identity facts into System Prompt.",
              active_identities.len()
          );
      }
      Ok(())
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Spec requirement (`docs/features/memory-architecture.md §4 & §6`): `Identity` collection facts are strictly preloaded at session boot to eliminate per-turn SQL queries, embedding latency, and vector search overhead for identity context.
- **Architectural Trade-offs & Decision:**
  - *KV-Cache Prefix Stability:* Mutating the system prompt mid-session invalidates prefix token sequences in both local llama.cpp KV-cache and remote cloud provider prefix caches.
  - *Lifecycle Policy:* `load_identity_into_system_prompt` must run strictly once at initial session start (`start_session` across assistant domains).
  - *Structured Assembly:* To prevent substring truncation bugs when custom user prompts contain `<user_profile>` tags, store `base_system_prompt` and `identity_facts` as structured fields in `WorkingMemory` and assemble the combined system prompt deterministically via `assemble_system_prompt()`.
  - Mid-session pause/resume cycles freeze the system prompt, preserving 100% KV-cache stability across conversational turns. Any new identity facts committed by background memory workers take effect upon the next session start.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Wire `load_identity_into_system_prompt(&conn).await` into `start_session` in `modular_passive.rs`, `modular_ptt.rs`, `realtime_passive.rs`, and `realtime_ptt.rs`.
  2. Implement deterministic prompt assembly without fragile string slicing on `<user_profile>`.
  3. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Stage ordering: Preserves Audio → VAD → STT → LLM → TTS → Playback.
  - Audio hot path: Unaffected (runs during session setup before speech processing).
  - Per-turn latency: 0ms runtime DB hit for Identity during live conversational turns.

---

### Sprint 05 — `new_session`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:155-171`
- **Signature:** `pub fn new_session(&mut self, system_prompt: &str)`
- **Implementation:**
  ```rust
  /// Resets conversational history and initializes a new session.
  pub fn new_session(&mut self, system_prompt: &str) {
      let sys_msg = ChatMessage {
          role: Role::System,
          content: system_prompt.to_string(),
          timestamp_ms: current_timestamp_ms(),
      };
      let sys_tokens = estimate_tokens(system_prompt);

      self.system_prompt = sys_msg.clone();
      self.messages = vec![sys_msg];
      self.total_token_count = sys_tokens;
      self.kv_synced_index = 0;
      self.session_compaction_contexts.clear();
      self.latest_compaction_facts.clear();
      self.cancel_opportunistic();
      log::info!("[WorkingMemory] New session started. System prompt set.");
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In `modular_passive.rs`, `modular_ptt.rs`, etc., when `start_session` runs, a new `conversation_id` is generated, but `ConversationManager` messages and compaction contexts were never reset, bleeding old conversation history into new sessions.
- **Architectural Trade-offs & Decision:**
  - *Domain Placement Separation:* The high-level session initialization orchestration (minting conversation IDs, resetting state flags, re-initializing working memory, and loading identity into system prompt) does NOT belong buried inside `working_memory.rs`. It belongs in `services/pipeline/mod.rs` as `init_new_session(app, state, base_prompt)`.
  - *In-Place Working Memory Reset:* Inside `working_memory.rs`, the method is renamed/scoped to `reset_history(&mut self, system_prompt: &str)` (pure working memory buffer & compaction context reset).
  - *Sequence:* `services/pipeline/mod.rs::init_new_session` calls `state.conversation_manager.lock().reset_history(base_prompt)`, followed by `load_identity_into_system_prompt(&conn).await`.
- **Finalized Decision:** **REFACTOR & WIRE (MISSING WIRING / DOMAIN CLEANUP)**.
  1. Refactor working memory reset into `reset_history(&mut self, system_prompt: &str)` in `services/memory/working_memory.rs`.
  2. Implement unified `init_new_session` in `services/pipeline/mod.rs` called by `start_session` across assistant domains (`modular_passive`, `modular_ptt`, `realtime_passive`, `realtime_ptt`) and IPC reset commands.
  3. Add unit test coverage in `services/memory/working_memory.rs` and `services/pipeline/mod.rs`.
- **Pipeline Invariants:**
  - Stage ordering & thread models: Unchanged.
  - Architecture integrity: High-level pipeline session orchestration centralized in `services/pipeline/mod.rs`.
  - Context isolation: Guarantees 0% cross-session context pollution.

---

### Sprint 06 — `update_system_prompt`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:197-209`
- **Signature:** `pub fn update_system_prompt(&mut self, new_system_prompt: &str)`
- **Implementation:**
  ```rust
  /// Replaces the active system prompt content and updates token calculations.
  pub fn update_system_prompt(&mut self, new_system_prompt: &str) {
      if self.system_prompt.content != new_system_prompt {
          let sys_tokens = estimate_tokens(new_system_prompt);
          let old_sys_tokens = estimate_tokens(&self.system_prompt.content);
          self.system_prompt.content = new_system_prompt.to_string();
          if !self.messages.is_empty() && self.messages[0].role == Role::System {
              self.messages[0].content = new_system_prompt.to_string();
              self.total_token_count =
                  self.total_token_count.saturating_sub(old_sys_tokens) + sys_tokens;
          }
          self.kv_synced_index = 0;
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In `core/settings.rs:173`, persona settings updates are marked `SettingReloadPolicy::Hot`, but `ipc/settings/mutation.rs` only mutated the in-memory settings struct without updating `ConversationManager`'s active `system_prompt`.
- **Architectural Trade-offs & Decision:**
  - Hot reloading system prompt/persona text during development or user customization must take immediate effect on the active assistant without dropping conversation turns.
  - When the user edits the base persona in settings, the active `<user_profile>` identity block (if present) must be extracted from the current system prompt and appended to the new base prompt before calling `update_system_prompt`.
  - Setting `kv_synced_index = 0` correctly alerts local and cloud inference workers to re-evaluate the prompt prefix.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Hook `update_system_prompt` into `app/src-tauri/src/ipc/settings/mutation.rs` under `("persona", "modular_prompt")`.
  2. Preserve active `<user_profile>` blocks across the update.
  3. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Stage ordering: Preserved.
  - Thread safety: Safe under `state.conversation_manager.lock()`.
  - Latency: Sub-millisecond string token recalculation; zero audio pipeline impact.

---

### Sprint 07 — `push_assistant_turn`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:229-248`
- **Signature:** `pub fn push_assistant_turn(&mut self, text: String)`
- **Implementation:**
  ```rust
  /// Appends a new assistant turn to working memory.
  pub fn push_assistant_turn(&mut self, text: String) {
      if text.trim().is_empty() {
          return;
      }
      let tokens = estimate_tokens(&text);
      let msg = ChatMessage {
          role: Role::Assistant,
          content: text,
          timestamp_ms: current_timestamp_ms(),
      };
      self.messages.push(msg);
      self.total_token_count += tokens;
      self.kv_synced_index = self.messages.len();
      log::debug!(
          "[WorkingMemory] Assistant turn pushed. Total tokens: {} / {}. KV index: {}",
          self.total_token_count,
          self.max_context_tokens,
          self.kv_synced_index
      );
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - While `push_user_turn` was wired in all 4 domains (`modular_passive.rs`, `modular_ptt.rs`, `realtime_passive.rs`, `realtime_ptt.rs`), `push_assistant_turn` was omitted during the Phase 10 refactor. Consequently, `ConversationManager` accumulated user prompts without assistant responses, degrading multi-turn dialogues into isolated, one-sided turns and blinding compaction to previous assistant outputs.
- **Architectural Trade-offs & Decision:**
  - Tokens stream into the frontend (`EVENT_LLM_TOKEN`) and TTS chunker incrementally. To record the canonical assistant message, domain actors must accumulate tokens for the active `turn_id` into a local turn buffer.
  - On `on_llm_finished` (or server turn complete in realtime), the accumulated response string is passed to `state.conversation_manager.lock().push_assistant_turn(text)`.
  - This accurately increments `total_token_count`, updates `kv_synced_index`, and enables `ConversationManager` to evaluate context utilization thresholds (`0.65` / `0.85`) against the full conversation trace.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Accumulate generated tokens during streaming turns in `modular_passive.rs`, `modular_ptt.rs`, `realtime_passive.rs`, and `realtime_ptt.rs`.
  2. Call `state.conversation_manager.lock().push_assistant_turn(full_text)` on `on_llm_finished` (and server turn completion).
  3. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Stage ordering: Maintained (LLM → TTS → Playback).
  - Memory consistency: Restores bidirectional conversational memory integrity.
  - Audio hot path: Unaffected (zero audio thread locks).

---

### Sprint 08 — `build_context`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:338-408`
- **Signature:**
  ```rust
  pub fn build_context(
      &mut self,
      provider_kind: ProviderKind,
      is_devanagari: bool,
      llm_provider: Option<&dyn LlmProvider>,
      settings: Option<&crate::core::settings::LlmSettings>,
  ) -> (
      ConversationContext,
      Option<String>,
      HashMap<String, Vec<String>>,
  )
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In `modular_passive.rs` and `modular_ptt.rs`, `on_transcript_final` was using a naive shortcut (`state.conversation_manager.lock().get_messages().to_vec()`). This completely bypassed context budget validation, FIFO sliding window shift, critical threshold compaction, transition speech triggers, and session history XML consolidation.
- **Architectural Trade-offs & Decision:**
  - *Context Budget Enforcement:* Without `build_context`, conversations exceeding `max_context_tokens` trigger out-of-memory errors or token limit exceptions at the LLM provider.
  - *Critical Threshold Compaction:* When utilization hits `critical_threshold` ($\ge 0.85$), `build_context` executes `perform_compaction_maintenance`, extracts personal facts into `diff_to_enqueue`, and returns `transition_speech` so TTS can provide conversational filler while compaction processes. If compaction fails, it falls back cleanly to FIFO window shift.
  - *Consolidated Session History:* When compactions have occurred, `build_context` consolidates previous session context summaries into the system message.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Wire `build_context` into `on_transcript_final` in `modular_passive.rs` and `modular_ptt.rs`.
  2. If `transition_speech` is returned, dispatch as a non-blocking `TtsCommand::SynthesizeFiller { text, turn_id }` into the TTS actor channel with FIFO queue sequencing rather than uncoordinated synchronous synthesis.
  3. Enqueue extracted `personal_memory` facts into `personal_memory_queue` via `enqueue_personal_facts`.
  4. Pass the returned `ConversationContext.messages` to `GenerationRequest`.
  5. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Stage ordering: Strictly preserved (Audio → VAD → STT → RAG/Compaction → LLM → TTS → Playback).
  - Playback sequencing: Transition speech filler chunks queue cleanly ahead of main response audio in the TTS pipeline without playback state desync or abrupt turn clipping.
  - Memory bounds: Guarantees context utilization stays below 100% capacity.
  - Audio hot path: Unaffected.

---

### Sprint 09 — `try_trigger_opportunistic`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:511-533`
- **Signature:** `pub fn try_trigger_opportunistic(&mut self) -> Option<(usize, Vec<ChatMessage>, Arc<AtomicBool>)>`
- **Implementation:**
  ```rust
  /// Attempts to initiate an opportunistic background compaction when between soft and critical thresholds.
  pub fn try_trigger_opportunistic(
      &mut self,
  ) -> Option<(usize, Vec<ChatMessage>, Arc<AtomicBool>)> {
      if self.context_utilization() > self.soft_threshold
          && self.context_utilization() < self.critical_threshold
          && !self.opportunistic_active
          && self.messages.len() > 3
      {
          self.opportunistic_active = true;
          self.opportunistic_cancel = Arc::new(AtomicBool::new(false));
          log::info!(
              "[WorkingMemory] Triggering Opportunistic Compaction candidate at {:.1}% utilization.",
              self.context_utilization() * 100.0
          );
          Some((
              self.messages.len(),
              self.messages.clone(),
              Arc::clone(&self.opportunistic_cancel),
          ))
      } else {
          None
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Designed to perform zero-latency background compaction during idle pauses (between 65% `soft_threshold` and 85% `critical_threshold`), but was uncalled in the runtime event pump.
- **Architectural Trade-offs & Decision:**
  - *Zero User-Perceived Latency:* Spawning async background compaction during natural conversational pauses (after playback finishes, when paused, or during idle sweeps) compacts history before the live conversational turn ever hits the 85% critical threshold.
  - *Race & Cancellation Safety:* If user speech begins while opportunistic compaction is in flight, `on_speech_start` immediately cancels the background task via `opportunistic_cancel`.
  - *Idle Hooks:* Wired across `on_playback_finished` (transition to `Ready`), `pause_session` (transition to `Paused`), and the 30s background idle sweep in `persistence/memory_worker.rs`.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Wire `try_trigger_opportunistic` in `on_playback_finished` across assistant domains.
  2. Wire `try_trigger_opportunistic` in `pause_session` when the session is paused and audio capture is halted.
  3. Wire into the 30-second memory worker idle loop.
  4. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Thread safety: Clones message slice and shares `Arc<AtomicBool>` cancel handle; releases mutex lock before background LLM execution.
  - Latency: Completely asynchronous; zero blocking of audio capture or speech synthesis.

---

### Sprint 10 — `commit_opportunistic`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:536-579`
- **Signature:** `pub fn commit_opportunistic(&mut self, snapshot_len: usize, summary_text: String) -> bool`
- **Implementation:**
  ```rust
  /// Commits opportunistic compaction results if no user turns were added during processing.
  pub fn commit_opportunistic(&mut self, snapshot_len: usize, summary_text: String) -> bool {
      if !self.opportunistic_active {
          log::info!("[WorkingMemory] Commit rejected: Opportunistic compaction was inactive.");
          return false;
      }
      if self.opportunistic_cancel.load(Ordering::Relaxed) {
          self.opportunistic_active = false;
          log::info!("[WorkingMemory] Commit rejected: Opportunistic compaction was cancelled.");
          return false;
      }
      if self.messages.len() != snapshot_len {
          self.opportunistic_active = false;
          log::info!(
              "[WorkingMemory] Commit rejected: Race detected (expected {} items, current has {}).",
              snapshot_len,
              self.messages.len()
          );
          return false;
      }

      let last_user_turn = self.messages.pop().unwrap();
      let summary_msg = ChatMessage {
          role: Role::System,
          content: format!("[Summary of prior context: {}]", summary_text),
          timestamp_ms: current_timestamp_ms(),
      };

      self.messages = vec![self.system_prompt.clone(), summary_msg, last_user_turn];

      let mut count = 0;
      for msg in &self.messages {
          count += estimate_tokens(&msg.content);
      }
      self.total_token_count = count;
      self.kv_synced_index = 0;
      self.opportunistic_active = false;

      log::info!(
          "[WorkingMemory] Opportunistic Compaction COMMITTED successfully! Utilization now {:.1}%.",
          self.context_utilization() * 100.0
      );

      true
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Paired companion method to `try_trigger_opportunistic`.
- **Architectural Trade-offs & Decision:**
  - *Race Condition Protection:* Asynchronous background LLM generation can complete after the user has already resumed speaking or added new turns. `commit_opportunistic` validates `messages.len() == snapshot_len` and `!opportunistic_cancel.load()`. If any race condition or speech onset occurred, the commit is safely discarded without mutating the live message history.
  - *Atomic Context Compaction & Role Awareness:* On successful validation, seamlessly truncates intermediate turns into a concise context summary without assuming the last message is a user turn. It retains the system prompt, appends the summary system message, preserves the active trailing turn pair if uncompacted, updates `total_token_count`, and resets `kv_synced_index`.
- **Finalized Decision:** **RETAIN & WIRE (MISSING WIRING)**.
  1. Wire `commit_opportunistic` as the completion callback for the asynchronous background task spawned by `try_trigger_opportunistic`.
  2. Implement role-safe history reconstruction preserving the latest turn without blind `pop().unwrap()` role assumptions.
  3. Enqueue extracted `diff_to_enqueue` facts into `personal_memory_queue`.
  4. Add unit test coverage in `services/memory/working_memory.rs`.
- **Pipeline Invariants:**
  - Mutex bounds: Runs within a short, synchronous `state.conversation_manager.lock()` (<50µs) after background LLM generation completes.
  - Context integrity: 100% race-free; invalid commits reject cleanly without turn role corruption.

---

### Sprint 11 — `on_pipeline_idle`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:582-583`
- **Signature:** `pub fn on_pipeline_idle(&mut self)`
- **Implementation:**
  ```rust
  /// Handles idle pipeline transitions.
  pub fn on_pipeline_idle(&mut self) {}
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Empty NOOP function body `{}` left over from initial prototyping.
- **Architectural Trade-offs & Decision:**
  - Active idle workflows are already fully implemented and owned by:
    1. `try_trigger_opportunistic` (pre-compaction between 65% and 85% utilization).
    2. `persistence/memory_worker.rs` (30-second background queue sweeps & ONNX eviction).
    3. `services/llm/actor.rs::cool_down_llm` / `services/tts/actor.rs::cool_down_tts` (tiered auto-sleep).
  - An empty stub provides no architectural value and clutters the public API of `ConversationManager`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `on_pipeline_idle` from `services/memory/working_memory.rs`.
- **Pipeline Invariants:** Zero ripple across pipeline lifecycle or thread model.

---

### Sprint 12 — `latest_summary`
- **Location:** `app/src-tauri/src/services/memory/working_memory.rs:599-611`
- **Signature:** `pub fn latest_summary(&self) -> String`
- **Implementation:**
  ```rust
  /// Returns the most recent compaction summary string if present in history.
  pub fn latest_summary(&self) -> String {
      for msg in self.messages.iter().rev() {
          if msg.role == Role::System {
              if let Some(s) = msg.content.strip_prefix("[Compacted History Summary: ") {
                  return s.strip_suffix("]").unwrap_or(s).to_string();
              }
              if let Some(s) = msg.content.strip_prefix("[Summary of prior context: ") {
                  return s.strip_suffix("]").unwrap_or(s).to_string();
              }
          }
      }
      String::new()
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Ad-hoc reverse string parsing over chat messages.
- **Architectural Trade-offs & Decision:**
  - `ConversationManager` already stores structured compaction summaries in `self.session_compaction_contexts: Vec<String>` and formats them through `build_narrative_context_chain()`.
  - Re-parsing text strings with magic prefixes (`"[Compacted History Summary: "`, `"[Summary of prior context: "`) is brittle, redundant, and unused.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `latest_summary` from `services/memory/working_memory.rs`.
- **Pipeline Invariants:** Zero impact.

---

### Sprint 13 — `l2_normalize`
- **Location:** `app/src-tauri/src/services/memory/embedder.rs:265-269`
- **Signature:** `pub fn l2_normalize(v: &[f32]) -> Vec<f32>`
- **Implementation:**
  ```rust
  /// L2 normalizes a vector, returning a new normalized vector.
  pub fn l2_normalize(v: &[f32]) -> Vec<f32> {
      let mut out = v.to_vec();
      l2_normalize_in_place(&mut out);
      out
  }
  ```
- **Audit Findings:**
  - Production call sites in `src/`: 0
  - Test / Eval call sites: 1 (`app/src-tauri/evals/eval_retrieval.rs:193`)
  - The production embedding pipeline exclusively uses `l2_normalize_in_place(&mut sum_embeddings)` inside `mean_pool_and_normalize`.
- **Architectural Trade-offs & Decision:**
  - *Invariant — 100% Production Code in `src/`:* Helpers created strictly for evaluations or integration tests must reside in `evals/` or `tests/common/`, ensuring that all symbols in `src/` are 100% wired into the active production application.
  - `l2_normalize_in_place` remains in `embedder.rs` as the canonical high-performance in-place normalization method.
- **Finalized Decision:** **RELOCATE TO `evals/` & DELETE FROM `src/` (EVAL/TEST HELPER SEAM)**.
  1. Relocate `l2_normalize` to `app/src-tauri/evals/eval_retrieval.rs` (or `evals/common.rs`).
  2. Remove `pub fn l2_normalize` from `services/memory/embedder.rs`.
- **Pipeline Invariants:** Zero runtime production impact; maintains clean separation between production crate and evaluation harnesses.

---

### Sprint 14 — `classify_pair`
- **Location:** `app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs:353-358`
- **Signature:** `pub fn classify_pair(premise: &str, hypothesis: &str) -> Result<NliResult>`
- **Implementation:**
  ```rust
  /// Performs NLI classification between a single premise and hypothesis.
  pub fn classify_pair(premise: &str, hypothesis: &str) -> Result<NliResult> {
      let mut results = classify_batch(&[(premise, hypothesis)])?;
      results
          .pop()
          .ok_or_else(|| anyhow!("Empty prediction result"))
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In Stage 3 memory evaluation (`services/memory/pipeline/stage3_eval.rs:54`), all candidate NLI inferences are evaluated in batches via `classify_batch(&pairs)`.
- **Architectural Trade-offs & Decision:**
  - `classify_batch(&[(premise, hypothesis)])` already processes single or multiple pairs with equal efficiency without requiring a dedicated single-pair wrapper in `src/`.
  - Maintaining unused convenience wrappers in `src/` violates the workspace invariant that all code in `src/` must be 100% production-wired.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `classify_pair` from `services/memory/classifiers/intra_edge_classifier.rs` and its re-exports in `classifiers/mod.rs` and `services/memory/mod.rs`.
- **Pipeline Invariants:** Zero runtime impact; Stage 3 continues using `classify_batch`.

---

### Sprint 15 — `get_calibrated_class_mapping`
- **Location:** `app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs:417-420`
- **Signature:** `pub fn get_calibrated_class_mapping() -> Option<[NliLabel; 3]>`
- **Implementation:**
  ```rust
  /// Returns the calibrated class mapping ([index 0, index 1, index 2]) if the engine is loaded.
  pub fn get_calibrated_class_mapping() -> Option<[NliLabel; 3]> {
      let lock = NLI_ENGINE.read();
      lock.as_ref().map(|engine| engine.class_mapping)
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Getter written during model calibration development; `classify_batch` handles class index remapping internally.
- **Architectural Trade-offs & Decision:**
  - `class_mapping` is strictly an internal ONNX engine field that maps output logit indices to `NliLabel::{Contradiction, Entailment, Neutral}`.
  - Exposing internal engine calibration arrays publicly adds dead API surface to `src/`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `get_calibrated_class_mapping` from `services/memory/classifiers/intra_edge_classifier.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 16 — `get_calibrated_class_mapping_strings`
- **Location:** `app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs:423-432`
- **Signature:** `pub fn get_calibrated_class_mapping_strings() -> Option<Vec<&'static str>>`
- **Implementation:**
  ```rust
  /// Returns the calibrated class mapping as string labels if the engine is loaded.
  pub fn get_calibrated_class_mapping_strings() -> Option<Vec<&'static str>> {
      let lock = NLI_ENGINE.read();
      lock.as_ref().map(|engine| {
          engine
              .class_mapping
              .iter()
              .map(|label: &NliLabel| label.as_str())
              .collect()
      })
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - String conversion helper for calibrated class labels.
- **Architectural Trade-offs & Decision:**
  - Redundant debug getter for internal engine mapping; unreferenced across IPC, services, and tests.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `get_calibrated_class_mapping_strings` from `services/memory/classifiers/intra_edge_classifier.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 17 — `reset_samples_ingested`
- **Location:** `app/src-tauri/src/services/audio/playback.rs:158-160`
- **Signature:** `pub fn reset_samples_ingested(&self)`
- **Implementation:**
  ```rust
  /// Resets the total ingested sample counter back to zero.
  pub fn reset_samples_ingested(&self) {
      self.total_samples_ingested.store(0, Ordering::Relaxed);
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Associated `total_samples_ingested(&self)` getter and `total_samples_ingested: Arc<AtomicUsize>` were adding unnecessary `fetch_add(pushed, Ordering::SeqCst)` operations on every chunk during `ingest_chunk(&self, chunk_24khz)`.
  - Active monitoring in `monitoring/collector.rs` uses `buffer_len()` and `playback_active`, never `total_samples_ingested`.
- **Architectural Trade-offs & Decision:**
  - Eliminating dead tracking counters streamlines the high-throughput audio ingestion loop and reduces atomic contention on the playback ring buffer.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `reset_samples_ingested` and `total_samples_ingested` from `services/audio/playback.rs`.
  2. Remove `total_samples_ingested` atomic field and its increment from `PlaybackEngine::ingest_chunk`.
- **Pipeline Invariants:**
  - Audio hot path: Slightly improved efficiency (eliminates 1 redundant `SeqCst` atomic operation per 24kHz audio chunk).
  - Telemetry: Unaffected (active metrics in `collector.rs` use `buffer_len()`).

---

### Sprint 18 — `set_mode`
- **Location:** `app/src-tauri/src/services/audio/router.rs:88-92`
- **Signature:** `pub fn set_mode(&self, mode: RouteMode)`
- **Implementation:**
  ```rust
  /// Sets the active routing destination mode.
  pub fn set_mode(&self, mode: RouteMode) {
      if let Err(e) = self.cmd_tx.send(RouterCommand::SetMode(mode)) {
          log::warn!("[Audio::Router] Failed to dispatch SetMode command: {}", e);
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - The entire `AudioRouter` struct in `services/audio/router.rs` is an unspawned, unused leftover from Phase 9.
  - In Phase 10 architecture, audio capture from CPAL flows exclusively through `VadActor`, which performs VAD speech boundary detection and routes audio chunks directly to `services::pipeline::{dictation, realtime_ptt, modular_ptt}::ingest_audio`. Event routing is handled centrally by `services/pipeline/router.rs`.
- **Architectural Trade-offs & Decision:**
  - Bypassing VAD with a direct CPAL-to-websocket thread breaks Phase 10 invariants (ghost audio gating, energy calculations, and ducking).
  - `AudioRouter` is neither instantiated in `VoxEngine` nor stored in `AppState`.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `set_mode` from `services/audio/router.rs`.
  2. Mark the obsolete `services/audio/router.rs` file and `RouteMode` / `AudioRouter` exports in `services/audio/mod.rs` for complete deletion in Phase 10 cleanup.
- **Pipeline Invariants:** Zero runtime impact; preserves Phase 10 VAD-driven audio routing pipeline.

---

### Sprint 19 — `start_realtime`
- **Location:** `app/src-tauri/src/services/audio/router.rs:95-102`
- **Signature:** `pub fn start_realtime(&self, tx: UnboundedSender<Vec<i16>>)`
- **Implementation:**
  ```rust
  /// Enables realtime websocket audio streaming.
  pub fn start_realtime(&self, tx: UnboundedSender<Vec<i16>>) {
      if let Err(e) = self.cmd_tx.send(RouterCommand::StartRealtime(tx)) {
          log::warn!(
              "[Audio::Router] Failed to dispatch StartRealtime command: {}",
              e
          );
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Obsolete Phase 9 audio router method.
- **Architectural Trade-offs & Decision:**
  - In Phase 10, realtime audio streaming and resampling are owned exclusively by `RealtimeEngine` and `AudioBridge` (`services/realtime/audio_bridge.rs`), with speech audio routed through `VadActor`.
  - The CPAL router thread bypass is defunct and unused.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `start_realtime` from `services/audio/router.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 20 — `stop_realtime`
- **Location:** `app/src-tauri/src/services/audio/router.rs:105-112`
- **Signature:** `pub fn stop_realtime(&self)`
- **Implementation:**
  ```rust
  /// Disables realtime websocket audio streaming.
  pub fn stop_realtime(&self) {
      if let Err(e) = self.cmd_tx.send(RouterCommand::StopRealtime) {
          log::warn!(
              "[Audio::Router] Failed to dispatch StopRealtime command: {}",
              e
          );
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Companion shutdown method to `start_realtime` on the obsolete `AudioRouter`.
- **Architectural Trade-offs & Decision:**
  - In Phase 10, realtime session shutdown is handled directly via `RealtimeEngine::stop_session` and `AudioBridge::stop()`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `stop_realtime` from `services/audio/router.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 21 — `handle_cancel`
- **Location:** `app/src-tauri/src/services/dictation/controller.rs:25-38`
- **Signature:** `pub async fn handle_cancel(app: &AppHandle) -> Result<(), DictationError>`
- **Implementation:**
  ```rust
  /// Cancels active dictation recording.
  pub async fn handle_cancel(app: &AppHandle) -> Result<(), DictationError> {
      let state: State<'_, std::sync::Arc<AppState>> = app.state();
      state
          .pipeline
          .cancel_flag
          .store(true, std::sync::atomic::Ordering::Relaxed);
      if let Err(e) = app.emit(
          "ptt_status",
          serde_json::json!({ "state": "IDLE", "owner": "dictation" }),
      ) {
          log::warn!("[Dictation::Controller] Failed to emit ptt_status: {}", e);
      }
      Ok(())
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `DictationController` was an intermediary wrapper between global shortcut hooks and `services::pipeline::dictation`.
  - `ipc/tray.rs::cancel_active_dictation_turn` implemented ad-hoc cancel logic directly instead of dispatching to a pipeline function.
  - `services/pipeline/dictation.rs` was missing a canonical `handle_hotkey_cancel` function matching `modular_ptt.rs::handle_ptt_cancel` and `realtime_ptt.rs::handle_ptt_cancel`.
- **Architectural Trade-offs & Decision:**
  - In Phase 10, all domain lifecycles and event flows belong in `services/pipeline/` (e.g. `services::pipeline::dictation`).
  - Redundant wrapper files (`services/dictation/controller.rs`) violate the thin dispatch invariant and add unnecessary indirection.
- **Finalized Decision:** **DELETE (DEAD CODE & REFACTOR DOMAIN DISPATCH)**.
  1. Add canonical `handle_hotkey_cancel<R: tauri::Runtime>(app: &AppHandle<R>, state: &AppState)` to `services/pipeline/dictation.rs` which resets `IS_RECORDING`, clears `DICTATION_BUFFER`, sets `cancel_flag = true`, emits `EVENT_PTT_STATUS` to `WINDOW_TRAY`, and transitions state to `Idle`.
  2. Wire `services/dictation/hotkey.rs` and `ipc/tray.rs` directly to `services::pipeline::dictation::{handle_hotkey_press, handle_hotkey_release, handle_hotkey_cancel}`.
  3. Delete `services/dictation/controller.rs` entirely.
- **Pipeline Invariants:** Restores uniform `handle_hotkey_cancel` symmetry across Modular PTT, Realtime PTT, and Dictation PTT.

---

### Sprint 22 — `barge_in`
- **Location:** `app/src-tauri/src/services/realtime/engine.rs:93-102`
- **Signature:** `pub fn barge_in(&self, playback_engine: &PlaybackEngine)`
- **Implementation:**
  ```rust
  /// Cancels active speech playback and sends barge-in notification to the provider session.
  pub fn barge_in(&self, playback_engine: &PlaybackEngine) {
      log::info!("[RealtimeEngine] Interruption (barge-in) triggered.");
      playback_engine.cancel();

      if let Some(ref session) = self.session {
          if let Err(e) = session.cancel() {
              log::warn!("[RealtimeEngine] Cancel error during barge-in: {:?}", e);
          }
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In `realtime_ptt.rs::handle_ptt_start`, when user presses the PTT hotkey while playback is active, client playback was muted but `barge_in` was never called, failing to send `ControlEvent::Interrupt` to Gemini Live / Deepgram. In Gemini Live, without an explicit interrupt pulse, server-side monologue generation is only paused rather than terminated.
  - In `realtime_passive.rs`, native server-side VAD emits `VoxEvent::Cancelled` on Gemini `interrupted: true`, but `handle_event` had no match arm for `VoxEvent::Cancelled`.
- **Architectural Trade-offs & Decision:**
  - *PTT Barge-In Protocol:* In Push-To-Talk manual activity mode, Gemini Live requires `session.cancel()` -> `ControlEvent::Interrupt` (an `activityStart` + `activityEnd` pulse) to cleanly abort its ongoing turn buffer and transition to accepting the new user turn.
  - *Passive Interruption Handling:* In Passive mode, Gemini Live detects user speech natively on the streamed audio and sends `serverContent: { interrupted: true }` -> `VoxEvent::Cancelled`. `realtime_passive.rs` must handle `VoxEvent::Cancelled` via `on_server_interrupted` to immediately cancel local audio playback and transition the UI to `Listening`.
- **Finalized Decision:** **RETAIN & WIRE (CRITICAL MISSING WIRING)**.
  1. Wire `barge_in(&playback_engine)` into `realtime_ptt.rs::handle_ptt_start` whenever playback is active.
  2. Implement `on_server_interrupted` in `realtime_passive.rs` matching `VoxEvent::Cancelled` to cancel playback and transition UI state to `Listening`.
  3. Add unit test coverage in `realtime_ptt_test.rs` and `services/realtime/engine.rs`.
- **Pipeline Invariants:** Enforces clean turn lifecycle and complete interruption compliance with Gemini Multimodal Live API specifications.

---

### Sprint 23 — `ingest_audio_i16`
- **Location:** `app/src-tauri/src/services/pipeline/realtime_ptt.rs:36-40`
- **Signature:** `pub fn ingest_audio_i16(chunk: &[i16])`
- **Implementation:**
  ```rust
  /// Ingests i16 audio samples into the realtime Push-To-Talk buffer when recording is active.
  pub fn ingest_audio_i16(chunk: &[i16]) {
      if IS_RECORDING.load(Ordering::Relaxed) {
          REALTIME_PTT_BUFFER.lock().extend_from_slice(chunk);
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Redundant overload. All microphone ingestion paths (`AudioStream`, `VadActor`) and test harnesses supply `f32` slices via `ingest_audio(chunk: &[f32])`.
- **Architectural Trade-offs & Decision:**
  - Standardizing all Push-to-Talk domains (`modular_ptt`, `dictation`, `realtime_ptt`) on a single `pub fn ingest_audio(chunk: &[f32])` interface maintains architectural consistency and eliminates unused conversion entry points.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `ingest_audio_i16` from `services/pipeline/realtime_ptt.rs`.
- **Pipeline Invariants:** Zero runtime impact; preserves uniform `ingest_audio(&[f32])` ingestion interface.

---

### Sprint 24 — `set_speech_detected`
- **Location:** `app/src-tauri/src/services/pipeline/realtime_ptt.rs:48-50`
- **Signature:** `pub fn set_speech_detected(detected: bool)`
- **Implementation:**
  ```rust
  /// Sets whether speech activity has been detected during the current Push-To-Talk hold.
  pub fn set_speech_detected(detected: bool) {
      SPEECH_DETECTED.store(detected, Ordering::Relaxed);
  }
  ```
- **Audit Findings:**
  - Production call sites in `src/`: 0
  - Test call sites: 2 (`tests/realtime_ptt_test.rs:176, 217`)
  - In production, `SPEECH_DETECTED` is managed automatically by `on_speech_start` and `handle_ptt_start/stop/cancel`.
  - Exposing a public setter `set_speech_detected(bool)` in production `src/` creates an artificial test backdoor that bypasses normal event flows.
- **Architectural Trade-offs & Decision:**
  - In `tests/realtime_ptt_test.rs`, speech onset can be stimulated cleanly through the standard event pipeline by dispatching `handle_event(&app, &state, &playback_engine, VoxEvent::SpeechStart { turn_id })`.
  - Eliminating `set_speech_detected` restores the invariant that `src/` contains 100% production-wired code with zero backdoor test setters.
- **Finalized Decision:** **DELETE FROM `src/` (TEST SEAM REFACTOR)**.
  1. Remove `set_speech_detected` from `services/pipeline/realtime_ptt.rs`.
  2. Refactor `tests/realtime_ptt_test.rs` lines 176 & 217 from backdoor `set_speech_detected(true)` to canonical event dispatch:
     ```rust
     realtime_ptt::handle_event(&app, &state, &playback_engine, VoxEvent::SpeechStart { turn_id: 1 });
     ```
- **Pipeline Invariants:** Zero production runtime regression; eliminates backdoor mutation points in production memory.

---

### Sprint 25 — `is_speech_detected`
- **Location:** `app/src-tauri/src/services/pipeline/realtime_ptt.rs:53-55`
- **Signature:** `pub fn is_speech_detected() -> bool`
- **Implementation:**
  ```rust
  /// Returns true if speech activity was detected during the current Push-To-Talk hold.
  pub fn is_speech_detected() -> bool {
      SPEECH_DETECTED.load(Ordering::Relaxed)
  }
  ```
- **Audit Findings:**
  - Production call sites in `src/`: 0
  - Test call sites: 3 (`tests/realtime_ptt_test.rs:127, 186, 225`)
  - Inside `services/pipeline/realtime_ptt.rs`, `SPEECH_DETECTED` is checked internally via atomic load during `handle_ptt_stop_with_sender`.
  - Exposing `is_speech_detected()` in `src/` is a test-only inspector that violates the principle of black-box observable verification.
- **Architectural Trade-offs & Decision:**
  - Integration tests should assert on public domain outputs: mock engine chunk ingress (`push_counter`), buffer draining (`get_buffer_len() == 0`), and emitted state transitions (`EVENT_PTT_STATUS`), rather than inspecting internal atomic flags.
- **Finalized Decision:** **DELETE FROM `src/` (TEST SEAM REFACTOR)**.
  1. Remove `is_speech_detected` from `services/pipeline/realtime_ptt.rs`.
  2. Refactor `tests/realtime_ptt_test.rs` lines 127, 186, 225 from `is_speech_detected()` assertions to observable state and mock engine chunk assertions:
     ```rust
     assert!(!realtime_ptt::is_recording());
     assert!(mock_engine.received_chunks_count() > 0);
     ```
- **Pipeline Invariants:** Zero production runtime regression; enforces clean black-box integration testing.

---

### Sprint 26 — `probe_top_k`
- **Location:** `app/src-tauri/src/services/llm/probe.rs:30-105`
- **Signature:** `pub async fn probe_top_k(&self, base_url: &str, model: &str, api_key: Option<&str>, kind: ProviderKind) -> CapabilityObservation`
- **Implementation:**
  ```rust
  /// Run an isolated probe for `top_k` on an OpenAI-compatible endpoint.
  pub async fn probe_top_k(
      &self,
      base_url: &str,
      model: &str,
      api_key: Option<&str>,
      kind: ProviderKind,
  ) -> CapabilityObservation {
      // Sends dummy POST with max_tokens: 1, top_k: 40 to evaluate endpoint rejection
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `ActiveProbeEngine` in `services/llm/probe.rs` was a speculative dynamic HTTP probing utility designed to test if an endpoint supported `top_k` by issuing dummy completions.
  - In production, parameter formatting is handled deterministically by `OpenAiCompatProvider` via detected `LocalBackendKind` (`Ollama` and `LmStudio` receive `top_k`, whereas `StandardOpenAi` drops it to prevent 400 rejection).
- **Architectural Trade-offs & Decision:**
  - Dynamic runtime probing burns user API tokens/credits on startup, adds latency, and creates failure modes if the probe fails due to transient network issues.
  - Deterministic backend detection is already implemented and reliable in `openai_compat.rs`.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `probe_top_k` and delete the entire `services/llm/probe.rs` file.
  2. Remove `pub use probe::ActiveProbeEngine;` from `services/llm/mod.rs`.
  3. Clean up `CapabilitySource::ActiveProbe` if unused.
- **Pipeline Invariants:** Zero production runtime impact; eliminates speculative HTTP probing overhead.

---

### Sprint 27 — `static_unsupported`
- **Location:** `app/src-tauri/src/services/llm/capabilities.rs:40-47`
- **Signature:** `pub fn static_unsupported(detail: &str) -> Self`
- **Implementation:**
  ```rust
  /// Creates a static observation marked as unsupported.
  pub fn static_unsupported(detail: &str) -> Self {
      Self {
          support: Support::Unsupported,
          source: CapabilitySource::StaticProviderKnowledge,
          status_code: None,
          detail: Some(detail.to_string()),
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `services/llm/capabilities.rs` was an abandoned prototype that created a duplicate `ModelCapabilities` struct, `CapabilityObservation`, and `CapabilityRegistry`.
  - The canonical capability probe and data structures in Vox are `services/llm/capability_probe.rs` (`CapabilityProbeEngine`) and `core::settings::ModelCapabilities`, which power the Settings UI and IPC health checks (`ipc/settings/health.rs`).
- **Architectural Trade-offs & Decision:**
  - Retaining duplicate capability files creates severe architectural ambiguity.
  - `services/llm/capability_probe.rs` already provides complete active probing for Ollama, LM Studio, OpenAI, and Embedded GGUF models.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `static_unsupported` and delete the entire `services/llm/capabilities.rs` file.
  2. Remove `pub mod capabilities;` and re-exports from `services/llm/mod.rs`.
- **Pipeline Invariants:** Zero runtime impact; reinforces `services/llm/capability_probe.rs` as the single source of truth for LLM capability inspection.

---

### Sprint 28 — `get_or_insert_default`
- **Location:** `app/src-tauri/src/services/llm/capabilities.rs:117-128`
- **Signature:** `pub fn get_or_insert_default(&self, key: &str, kind: ProviderKind) -> ModelCapabilities`
- **Implementation:**
  ```rust
  /// Fetches capability matrix for key, initializing with kind baseline if missing.
  pub fn get_or_insert_default(&self, key: &str, kind: ProviderKind) -> ModelCapabilities {
      let read_guard = self.cache.read();
      if let Some(caps) = read_guard.get(key) {
          return caps.clone();
      }
      drop(read_guard);
      let mut write_guard = self.cache.write();
      let default_caps = ModelCapabilities::default_for_kind(kind);
      write_guard.entry(key.to_string()).or_insert(default_caps).clone()
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Method on `CapabilityRegistry` within the obsolete `services/llm/capabilities.rs` file.
- **Architectural Trade-offs & Decision:**
  - Capability caching for providers is managed in `AppState` settings and probed on-demand by `CapabilityProbeEngine`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `get_or_insert_default` along with the deletion of `services/llm/capabilities.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 29 — `format_prompt`
- **Location:** `app/src-tauri/src/services/llm/llama_cpp.rs:86-92`
- **Signature:** `pub fn format_prompt(&self, text: &str, system_prompt: &str) -> String`
- **Implementation:**
  ```rust
  /// Combines system and user prompt segments into a complete turn prompt.
  pub fn format_prompt(&self, text: &str, system_prompt: &str) -> String {
      format!(
          "{}{}",
          self.format_system_prompt(system_prompt),
          self.format_user_prompt(text)
      )
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Legacy single-turn prompt formatter.
  - Vox conversational memory is multi-turn (`ChatMessage` history). Generation in `LlmWorker::generate` (line 494) formats the full structured turn sequence using `self.family.format_conversation(&conv_ctx.messages)`.
- **Architectural Trade-offs & Decision:**
  - `format_conversation` supports all roles (`System`, `User`, `Assistant`) across all 4 supported prompt families (`Gemma`, `Qwen`, `Llama3`, `Nemotron`).
  - `format_prompt` does not handle assistant turns and is completely superseded.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `format_prompt` from `services/llm/llama_cpp.rs`.
- **Pipeline Invariants:** Zero runtime impact; preserves multi-turn `format_conversation` template formatting.

---

### Sprint 30 — `run_loop`
- **Location:** `app/src-tauri/src/services/llm/llama_cpp.rs:345-382`
- **Signature:** `pub fn run_loop(&self, rx: std::sync::mpsc::Receiver<super::actor::LlmCommand>, tx: std::sync::mpsc::Sender<VoxEvent>)`
- **Implementation:**
  ```rust
  /// Runs the persistent command worker loop for generation requests.
  pub fn run_loop(
      &self,
      rx: std::sync::mpsc::Receiver<super::actor::LlmCommand>,
      tx: std::sync::mpsc::Sender<VoxEvent>,
  ) {
      log::info!("[LLM Worker] Persistent loop started.");
      while let Ok(cmd) = rx.recv() {
          // match LlmCommand::Generate, LlmCommand::Shutdown
      }
      log::info!("[LLM Worker] Loop exited. Model will be dropped.");
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Legacy actor loop from Phase 8 prior to provider decoupling.
  - In Phase 10, all LLM actor execution is driven uniformly by `spawn_llm_worker` in `services/llm/actor.rs:23`, which hosts a Tokio current-thread runtime executing `provider.generate(...)` for both `EmbeddedProvider` and `OpenAiCompatProvider`.
- **Architectural Trade-offs & Decision:**
  - `LlmWorker::run_loop` is duplicate dead code that bypasses the unified `LlmProvider` trait lifecycle.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `run_loop` from `services/llm/llama_cpp.rs`.
- **Pipeline Invariants:** Zero runtime impact; ensures single unified actor thread in `services/llm/actor.rs`.

---

### Sprint 31 — `validate_wav`
- **Location:** `app/src-tauri/src/ipc/voices.rs:61-65`
- **Signature:** `pub async fn validate_wav(path: String, min_duration_secs: f32) -> Result<(u32, f32), String>`
- **Implementation:**
  ```rust
  /// Validate a WAV file's readability and minimum duration requirements.
  #[tauri::command]
  pub async fn validate_wav(path: String, min_duration_secs: f32) -> Result<(u32, f32), String> {
      tokio::task::spawn_blocking(move || validate_wav_file(&path, min_duration_secs))
          .await
          .map_err(|e| format!("Task panicked: {}", e))?
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `validate_wav` is an IPC command in `ipc/voices.rs` that is not registered in `lib.rs::generate_handler![]` and has no frontend caller.
  - The voice cloning workflow (`add_voice_from_file` at `ipc/voices.rs:79`) already performs full audio decoding, resampling, duration validation, and tensor pre-baking via `convert_and_validate_audio(&file_path, &dest)`.
- **Architectural Trade-offs & Decision:**
  - Exposing an uncalled separate validation IPC endpoint creates dead API surface. Validation errors are already surfaced directly to users via `add_voice_from_file`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `validate_wav` from `ipc/voices.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 32 — `preview_voice`
- **Location:** `app/src-tauri/src/ipc/voices.rs:232-264`
- **Signature:** `pub async fn preview_voice(id: String) -> Result<String, String>`
- **Implementation:**
  ```rust
  /// Synthesize a short preview audio clip using the specified cloned voice.
  #[tauri::command]
  pub async fn preview_voice(id: String) -> Result<String, String> {
      // Loads voice, calls synthesize_preview_clip, writes preview.wav, updates DB
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Unregistered in `lib.rs::generate_handler![]`.
  - The frontend `VoiceCarousel.tsx` does not trigger voice preview generation. Voice selection applies immediately to live speech synthesis.
  - Calling `preview_voice` spins up a cold `ChatterboxEngine` diffusion instance solely to write a static `preview.wav` to disk.
- **Architectural Trade-offs & Decision:**
  - Dead IPC surface that introduces heavy unnecessary compute dependencies into the voice settings API.
  - Associated backend helper `synthesize_preview_clip` in `services/tts/voice.rs:280` has no other callers.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `preview_voice` from `ipc/voices.rs`.
  2. Purge `synthesize_preview_clip` from `services/tts/voice.rs`.
- **Pipeline Invariants:** Zero runtime impact; preserves core dynamic TTS voice generation.

---

### Sprint 33 — `update_theme`
- **Location:** `app/src-tauri/src/ipc/settings/mutation.rs:286-300`
- **Signature:** `pub async fn update_theme(app: AppHandle, theme: String) -> Result<(), String>`
- **Implementation:**
  ```rust
  /// Convenience command for theme changes (kept for backward compat with existing frontend).
  #[tauri::command]
  pub async fn update_theme(app: AppHandle, theme: String) -> Result<(), String> {
      let state: State<'_, std::sync::Arc<AppState>> = app.state();
      {
          let mut settings = state.settings.write().map_err(|e| e.to_string())?;
          if settings.appearance.theme == theme {
              return Ok(());
          }
          settings.appearance.theme = theme.clone();
      }
      if let Err(e) = app.emit("theme-changed", theme) {
          log::warn!("[Settings::Mutation] Failed to emit theme-changed: {}", e);
      }
      schedule_debounced_save(state.clone()).await;
      Ok(())
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Unregistered in `lib.rs::generate_handler![]`.
  - The frontend (`settingsStore.ts:448`) executes all appearance updates via `updateSetting("appearance", key, value)`.
  - The backend `update_setting` handler (`ipc/settings/mutation.rs:334`) handles `("appearance", "theme")` with full validation, theme-changed emission, and debounced persistence.
- **Architectural Trade-offs & Decision:**
  - `update_theme` is obsolete legacy convenience code. All setting mutations in Vox are standardized under the single `update_setting` IPC endpoint.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `update_theme` from `ipc/settings/mutation.rs`.
- **Pipeline Invariants:** Zero runtime impact; preserves canonical `update_setting` schema.

---

### Sprint 34 — `fetch_intra_subfloor_candidates`
- **Location:** `app/src-tauri/src/persistence/queries.rs:381-449`
- **Signature:** `pub async fn fetch_intra_subfloor_candidates(conn: &Connection, collection: &str, query_embedding: &[f32], floor_threshold: f32, ceil_threshold: f32, limit: Option<i64>) -> Result<Vec<(String, String, f32)>>`
- **Implementation:**
  ```rust
  /// Fetches intra-collection candidates in the sub-floor window [floor_threshold, ceil_threshold).
  /// Used exclusively by eval_pipeline.rs post-pipeline audit pass.
  pub async fn fetch_intra_subfloor_candidates(
      // Executes sub-floor SQL query between floor_threshold and ceil_threshold
  )
  ```
- **Audit Findings:**
  - Production call sites in `src/`: 0
  - Eval call sites: 1 (`evals/eval_memory_pipeline.rs:346`)
  - Explicitly authored as an offline evaluation query to audit candidates falling just below the primary retrieval floor.
- **Architectural Trade-offs & Decision:**
  - In accordance with the SSOT rule that `src/` must contain 100% production-consumed code, evaluation-only query helpers must not pollute the production persistence layer.
- **Finalized Decision:** **RELOCATE TO `evals/` & PURGE FROM `src/`**.
  1. Relocate `fetch_intra_subfloor_candidates` into `evals/helpers/` (or directly inside `evals/eval_memory_pipeline.rs`).
  2. Purge `fetch_intra_subfloor_candidates` from `src/persistence/queries.rs`.
- **Pipeline Invariants:** Zero production runtime impact; ensures `src/` contains 100% production-wired code.

---

### Sprint 35 — `fetch_inter_subfloor_candidates`
- **Location:** `app/src-tauri/src/persistence/queries.rs:453-534`
- **Signature:** `pub async fn fetch_inter_subfloor_candidates(conn: &Connection, target_collections: &[&str], query_embedding: &[f32], floor_threshold: f32, ceil_threshold: f32, limit: Option<i64>) -> Result<Vec<(String, String, String, f32)>>`
- **Implementation:**
  ```rust
  /// Fetches inter-collection candidates in the sub-floor window [floor_threshold, ceil_threshold).
  /// Used exclusively by eval_pipeline.rs post-pipeline audit pass.
  pub async fn fetch_inter_subfloor_candidates(
      // Executes multi-collection sub-floor SQL query between floor_threshold and ceil_threshold
  )
  ```
- **Audit Findings:**
  - Production call sites in `src/`: 0
  - Eval call sites: 1 (`evals/eval_memory_pipeline.rs:364`)
  - Offline evaluation companion query to `fetch_intra_subfloor_candidates` for cross-collection candidate audit passes.
- **Architectural Trade-offs & Decision:**
  - `src/persistence/queries.rs` should exclusively contain queries required by the active production runtime.
- **Finalized Decision:** **RELOCATE TO `evals/` & PURGE FROM `src/`**.
  1. Relocate `fetch_inter_subfloor_candidates` into `evals/helpers/` (or directly inside `evals/eval_memory_pipeline.rs`).
  2. Purge `fetch_inter_subfloor_candidates` from `src/persistence/queries.rs`.
- **Pipeline Invariants:** Zero production runtime impact; ensures `src/` contains 100% production-wired code.

---

### Sprint 36 — `get_model`
- **Location:** `app/src-tauri/src/setup/manifest.rs:70-77`
- **Signature:** `pub fn get_model(&self, id: &str) -> Option<&ModelEntry>`
- **Implementation:**
  ```rust
  /// Finds a model entry by ID.
  pub fn get_model(&self, id: &str) -> Option<&ModelEntry> {
      for group in &self.model_groups {
          if let Some(m) = group.files.iter().find(|m| m.id == id) {
              return Some(m);
          }
      }
      None
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In `ipc/setup.rs` and `setup/runtime_check.rs`, model resolution and integrity verification iterate over `model_groups` and `group.files` directly.
- **Architectural Trade-offs & Decision:**
  - `get_model` is an uncalled lookup helper on `VoxManifest`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `get_model` from `setup/manifest.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 37 — `nli_inverse_edge`
- **Location:** `app/src-tauri/src/core/constants.rs:288-295`
- **Signature:** `pub fn nli_inverse_edge(relation: &str) -> &'static str`
- **Implementation:**
  ```rust
  /// Returns the deterministic inverse relation string for an NLI relation (spec §4.3.1).
  pub fn nli_inverse_edge(relation: &str) -> &'static str {
      match relation {
          PM_RELATION_SUPPORTS => "supported_by",
          PM_RELATION_SUPERSEDES => "superseded_by",
          PM_RELATION_CONFLICTS => "conflicts_with",
          _ => "related_to",
      }
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - In `services/memory/pipeline/stage3_eval.rs:195`, the memory evaluation stage uses `inverse_edge_for_relation(&pred_edge)` directly to resolve inverse relationships across all edge categories (`SHAPES`, `DEPENDS_ON`, `CONFLICTS`, `SUPPORTS`, `SUPERSEDES`).
  - `nli_inverse_edge` is an uncalled, strict subset duplicate of `inverse_edge_for_relation`.
- **Architectural Trade-offs & Decision:**
  - Maintaining multiple relation inversion functions in `core/constants.rs` introduces duplicate logic and confusion.
  - `inverse_edge_for_relation` is the single canonical source of truth for semantic edge inversions.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `nli_inverse_edge` from `core/constants.rs`.
- **Pipeline Invariants:** Zero runtime impact; preserves canonical `inverse_edge_for_relation` mappings.

---

### Sprint 38 — `latency_report`
- **Location:** `app/src-tauri/src/core/metrics.rs:60-208`
- **Signature:** `pub fn latency_report(&self, input_duration: f64, output_duration: f64, mode: crate::core::settings::PipelineMode, is_ptt: bool) -> serde_json::Value`
- **Implementation:**
  ```rust
  /// Computes JSON snapshot of latency, memory, and throughput metrics for a turn.
  pub fn latency_report(&self, input_duration: f64, output_duration: f64, mode: PipelineMode, is_ptt: bool) -> serde_json::Value {
      // Computes diffs across Instant timestamps and returns legacy metrics JSON
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `core/metrics.rs` (`PipelineMetrics`, `MetricField`, `latency_report`) was an early prototype for ad-hoc stage latency measurement.
  - In Phase 10, real-time metrics aggregation, sliding-window latency tracking, memory profiling, and IPC reporting are fully implemented in `src/monitoring/` (`collector.rs`, `snapshot.rs`, `profiler.rs`, `telemetry_emitter.rs`).
- **Architectural Trade-offs & Decision:**
  - `core/metrics.rs` is completely uncalled dead code superseded by `src/monitoring/`.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `latency_report` and delete the entire `src/core/metrics.rs` file.
  2. Remove `pub mod metrics;` from `src/core/mod.rs`.
- **Pipeline Invariants:** Zero runtime impact; enforces `src/monitoring/` as the single unified metrics and telemetry system.

---

### Sprint 39 — `write_artifact`
- **Location:** `app/src-tauri/src/utils/bench_reporter.rs:41-44`
- **Signature:** `pub fn write_artifact(&self, filename: &str, content: &str)`
- **Implementation:**
  ```rust
  pub fn write_artifact(&self, filename: &str, content: &str) {
      let path = self.run_dir.join(filename);
      fs::write(path, content).expect("Failed to write artifact");
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `src/utils/bench_reporter.rs` was an ad-hoc benchmark logging tool whose only external reference was the deleted `core/metrics.rs`.
  - Uses unhandled `.expect()` panics and outputs to relative `outputs/` folder.
- **Architectural Trade-offs & Decision:**
  - `src/` must contain 100% production-consumed code.
- **Finalized Decision:** **DELETE (DEAD CODE)**.
  1. Purge `write_artifact` and delete the entire `src/utils/bench_reporter.rs` file.
  2. Remove `pub mod bench_reporter;` from `src/utils/mod.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 40 — `save_report`
- **Location:** `app/src-tauri/src/utils/bench_reporter.rs:46-50`
- **Signature:** `pub fn save_report(&self, report: serde_json::Value)`
- **Implementation:**
  ```rust
  pub fn save_report(&self, report: serde_json::Value) {
      let path = self.run_dir.join("metrics.json");
      let json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");
      fs::write(path, json).expect("Failed to write metrics.json");
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Co-located in `src/utils/bench_reporter.rs` alongside `write_artifact`.
- **Architectural Trade-offs & Decision:**
  - Dead code belonging to obsolete `bench_reporter.rs`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Deleted together with `bench_reporter.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 41 — `vox_dir`
- **Location:** `app/src-tauri/src/utils/paths.rs:117-119`
- **Signature:** `pub fn vox_dir() -> PathBuf`
- **Implementation:**
  ```rust
  pub fn vox_dir() -> PathBuf {
      get().root.clone()
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Callers access `crate::utils::paths::get().root` directly.
- **Architectural Trade-offs & Decision:**
  - `VoxPaths` singleton is essential for dynamic OS home directory resolution and `VOX_HOME` overrides across 60+ call sites. Redundant wrapper functions that have 0 callers should be purged to keep `paths.rs` lean.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `vox_dir` from `src/utils/paths.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 42 — `logs_dir`
- **Location:** `app/src-tauri/src/utils/paths.rs:125-127`
- **Signature:** `pub fn logs_dir() -> PathBuf`
- **Implementation:**
  ```rust
  pub fn logs_dir() -> PathBuf {
      get().logs.clone()
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Logging initialization (`lib.rs:121`) accesses `crate::utils::paths::get().logs.clone()` directly.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `logs_dir` from `src/utils/paths.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 43 — `voices_dir` (& `temp_dir`)
- **Location:** `app/src-tauri/src/utils/paths.rs:141-143, 152-154`
- **Signature:** `pub fn voices_dir() -> PathBuf`, `pub fn temp_dir() -> PathBuf`
- **Implementation:**
  ```rust
  pub fn temp_dir() -> PathBuf {
      get().temp.clone()
  }
  pub fn voices_dir() -> PathBuf {
      get().voices.clone()
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - Voice directory lookups use `voice_dir(&id)` or `paths::get().voices`.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `voices_dir` and `temp_dir` from `src/utils/paths.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Sprint 44 — `deserialize_value_resilient`
- **Location:** `app/src-tauri/src/utils/json.rs:142-206`
- **Signature:** `pub fn deserialize_value_resilient<'de, D>(deserializer: D) -> Result<String, D::Error> where D: Deserializer<'de>`
- **Implementation:**
  ```rust
  pub fn deserialize_value_resilient<'de, D>(deserializer: D) -> Result<String, D::Error> {
      // 65-line custom serde ValueVisitor converting strings, numbers, bools
  }
  ```
- **Audit Findings:**
  - Production call sites: 0
  - Test call sites: 0
  - `clean_json_content` and `parse_compaction_json` handle resilient JSON cleanup directly without invoking this visitor.
- **Finalized Decision:** **DELETE (DEAD CODE)**. Purge `deserialize_value_resilient` from `src/utils/json.rs`.
- **Pipeline Invariants:** Zero runtime impact.

---

### Special Sprint — `services/utils.rs` Extraction & Complete File Purge
- **Location:** `app/src-tauri/src/services/utils.rs` (407 lines)
- **Deep Audit Findings:**
  1. `transliterate_if_hi` (and `tokenize_devanagari_slices`): Core logic for Devanagari Hindi-to-Latin Hinglish conversion with trailing incomplete word protection. Belongs directly in `services/translit.rs`.
  2. `is_devanagari`: Character range check used in `supertonic.rs:197`. Belongs directly in `services/translit.rs`.
  3. `stitch_transcripts` (and alignment helpers: `edit_distance`, `words_soft_match`, `is_soft_subslice`, `find_alignment_match`, `find_sequential_overlap`): Core stateful STT streaming overlap stitcher that prevents $O(N^2)$ transcript calculation scaling. Belongs directly in `services/stt/stitcher.rs` and wires into `EmbeddedSttProvider::transcribe_chunk`.
  4. `should_flush` (and `ends_at_word_boundary`, `lerp`): Prototype token chunker only called in its own unit tests. Phase 10 TTS handles streaming chunking directly in actors. Dead code.
  5. `count_words` & `to_friendly_hinglish`: Dead code (Sprints 01 & 02).
- **Architectural Extraction & Wiring Plan:**
  1. **Relocate to `services/translit.rs`:** Move `is_devanagari`, `tokenize_devanagari_slices`, and `transliterate_if_hi`.
     - Wire `transliterate_if_hi` into `services/pipeline/dictation.rs` (`on_transcript_partial`, `on_transcript_final`), `services/pipeline/modular_ptt.rs` (`on_transcript_partial`, `on_transcript_final`), and `services/pipeline/modular_passive.rs`.
  2. **Relocate to `services/stt/stitcher.rs`:** Move `stitch_transcripts`, `words_soft_match`, `edit_distance`, `is_soft_subslice`, `find_alignment_match`, `find_sequential_overlap`.
     - Wire `stitch_transcripts` into `services/stt/providers/embedded.rs::transcribe_chunk`.
  3. **Purge File:** Delete `app/src-tauri/src/services/utils.rs` completely and remove `pub mod utils;` from `services/mod.rs`.
- **Pipeline Invariants:** Enforces clean separation of concerns, zero dead utility files, and proper transcript streaming across all speech pipelines.

---











