### 7.1 Tool 1: `set_title` / `set_session_title`
- **Classification**: Universal Action / State Mutation.
- **Implementor**: `services/harness/stages/tools/respond_and_set_title.rs`.
- **Modular & Universal Specification (`set_title`)**:
  - **Description**: Assigns a concise 3 to 5 word title summarizing the user's intent to initialize this new conversation session.
  - **Parameters**:
    - `title` (String, required): A concise 3 to 5 word title summarizing the user's intent.
  - **Behavioral Invariants**:
    1. **Title-Unset Injection Gate**: Offered in the model request whenever the session title is unset or unassigned (`title_is_unset == true`), regardless of turn index.
    2. **Post-Title Suppression**: Permanently omitted from candidate tool lists once a non-placeholder title is assigned to the session.
    3. **Zero Speech Parameter Bloat**: The tool accepts only `title`. The model formulates spoken dialogue naturally via standard content tokens, streamed eagerly by the 3-Stream Demuxer.
    4. **Exclusive Barrier Execution**: Executes within an exclusive barrier to safely mutate session metadata in the database.
- **Realtime Specification (`set_session_title`)**:
  - **Description**: Assigns a concise 3 to 5 word title to initialize this new conversation session.
  - **Parameters**:
    - `title` (String, required): A concise 3 to 5 word title summarizing the user's intent.
  - **Behavioral Invariants**:
    1. **Continuous Availability**: Declared continuously in WebSocket session configuration frames.
    2. **Execution-Level Idempotency Guard**: If invoked when the session title is already set, the tool checks `ctx.is_title_already_set()`, skips database write and `IpcEvent::SessionsChanged`, logs invocation to `session_tool_calls` as ignored, and returns a graceful rejection (`{"status": "ignored", "message": "Session title has already been set and is locked for this session."}`).
    3. **Native Provider Audio**: Zero local TTS audio dispatch; the provider model manages vocal output natively over the WebSocket.
