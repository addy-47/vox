# Adding a Model — Definition of Done (Phase 12)

Adding a TTS/STT/LLM model means exactly two things:

1. **Edit the manifest** (`manifests/models_manifest.json`) — distribution facts only
   (bytes, hashes, topology flags). Never behavior.
2. **Write the engine** (`app/src-tauri/src/services/<domain>/providers/`) —
   capabilities declared beside the code that enforces them (`fn caps()`,
   ranges beside clamps).

Any additional file touched is a regression signal. Concretely, adding a model
MUST NOT require edits to:

- `app/src/**` (capabilities come over IPC; voice lists are backend-scoped;
  tiers derive from manifest flags — Invariant 6 fails the build otherwise)
- `core/settings.rs` capability tables (exhaustive dispatch fails to compile
  until the new variant is wired — that compiler error is the checklist)
- `persistence/voices.rs` seed tables (the pack directory seeds itself;
  display names derive from slugs)
- new `as any` / `any` (Invariant 8 fails the build)
- new settings fields without a consumer (capability-consumability rule)

The wire test: delete the new provider id from the backend — the frontend must
still build and behave identically for every remaining provider.
