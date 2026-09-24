### 7.1 Tool 1: `respond_and_set_title` (Modular) / `set_session_title` (Realtime)
- **Classification**: Action / State Mutation.
- **Implementor**: `services/harness/stages/tools/respond_and_set_title.rs`.
- **Modular Specification (`respond_and_set_title`)**:
  - **Classification**: Terminal.
  - **Description**: Responds conversationally to the user while assigning a concise 3 to 5 word title to initialize this new conversation session.
  - **Parameters**:
    - `spoken_response` (String, required): Your natural, concise conversational spoken response to the user's message.
    - `title` (String, required): A concise 3 to 5 word title summarizing the user's intent.
  - **Behavioral Invariants**:
    1. **Title-Unset Injection Gate**: Offered in the model request whenever the session title is unset or unassigned (`title_is_unset == true`), regardless of turn index.
    2. **Post-Title Suppression**: Permanently omitted from candidate tool lists once a non-placeholder title is assigned to the session.
    3. **Single-Pass Audio Delivery**: Dispatches `spoken_response` directly to local speech synthesis as `AudioIntent::TurnResponse`.
- **Realtime Specification (`set_session_title`)**:
  - **Classification**: Action.
  - **Description**: Assigns a concise 3 to 5 word title to initialize this new conversation session.
  - **Parameters**:
    - `title` (String, required): A concise 3 to 5 word title summarizing the user's intent.
  - **Behavioral Invariants**:
    1. **Continuous Availability**: Declared continuously in WebSocket session configuration frames.
    2. **Execution-Level Idempotency Guard**: If invoked when the session title is already set, the tool checks `ctx.is_title_already_set()`, skips database write and `IpcEvent::SessionsChanged`, logs invocation to `session_tool_calls` as ignored, and returns a graceful rejection (`{"status": "ignored", "message": "Session title has already been set and is locked for this session."}`).
    3. **Native Provider Audio**: Zero local TTS audio dispatch; the provider model manages vocal output natively.
