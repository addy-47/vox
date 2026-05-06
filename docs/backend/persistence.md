# 📄 `persistence.md`

---

# 1. Purpose

Introduce a proper **Persistence Layer + Unified App State Architecture** for Vox.

This system must:

* provide a single source of truth
* synchronize settings across all UI surfaces
* support future expansion cleanly
* remain completely decoupled from the realtime pipeline

---

# 2. Core Principle

> ❗ Runtime state lives in memory.
> Disk is only a persistence snapshot.

---

# 3. Persistence Boundary (CRITICAL)

The realtime pipeline MUST remain independent from storage.

---

## ❌ Forbidden

```text
audio callback → disk write
STT event → file write
VAD loop → JSON update
```

---

## ✅ Required

```text
Realtime Pipeline
    ↓
App State (memory)
    ↓
Async Persistence Layer
```

---

# 4. Cross-Platform Storage Paths

---

## ❌ Do NOT

Hardcode:

```text
~/.vox
```

---

## ✅ Required

Use Tauri-native paths:

```rust
app.path().app_config_dir()
```

or

```rust
app.path().app_data_dir()
```

---

## Expected OS Paths

| OS      | Path                                 |
| ------- | ------------------------------------ |
| Linux   | `~/.config/vox/`                     |
| Windows | `%APPDATA%/vox/`                     |
| macOS   | `~/Library/Application Support/vox/` |

---

# 5. Directory Structure

---

## Required Layout

```text
app_config_dir/
├── settings.json
├── logs/
├── history.db
├── sessions/
└── models/
```

---

## Notes

### `settings.json`

Persistent user configuration.

---

### `logs/`

Rotating application logs.

---

### `history.db`

Future SQLite persistence.

---

### `sessions/`

Optional future exports/debugging.

---

### `models/`

Optional future model downloads.

---

# 6. Startup Initialization

---

## Required Behavior

Persistence layer initializes during:

```text
tauri::Builder.setup()
```

NOT lazily later.

---

## Startup Flow

```text
App Start
↓
Resolve config directory
↓
Create directory structure
↓
Load settings.json
↓
Fallback to defaults if missing
↓
Hydrate AppState
↓
Expose to frontend
```

---

# 7. State Consolidation (CRITICAL)

Current architecture contains fragmented state:

```rust
HudVisibility
HudMenuItem
InteractionState
PttManager
EngineState
```

This will become unmaintainable.

---

# 8. Unified AppState Architecture

---

## Required

Create a single root application state:

```rust
pub struct AppState {
    pub settings: Arc<RwLock<Settings>>,
    pub interaction: Arc<Mutex<InteractionState>>,
    pub hud: Arc<Mutex<HudState>>,
    pub ptt: Arc<Mutex<PttState>>,
    pub engine: Arc<Mutex<Option<VoxEngine>>>,
}
```

---

# 9. State Separation Rules

---

## 9.1 Persistent State

Stored to disk:

```rust
Settings
```

---

## 9.2 Runtime State

Memory only:

```rust
InteractionState
HudState
PttState
EngineState
```

---

## ❗ Important

Do NOT serialize runtime state.

---

# 10. Settings System

---

# 10.1 Settings Struct

---

## Required

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub version: u32,
    pub theme: Theme,
    pub overlay_enabled: bool,
    pub ptt_enabled: bool,
    pub launch_on_startup: bool,
}
```

---

# 10.2 Theme Enum

---

```rust
#[derive(Serialize, Deserialize, Clone)]
pub enum Theme {
    Light,
    Dark,
    System,
}
```

---

# 10.3 Default Settings

---

## Required

```rust
impl Default for Settings
```

Fallback used when:

* config missing
* invalid config
* migration failure

---

# 11. Settings Manager

---

## Create

```rust
SettingsManager
```

---

## Responsibilities

* load settings
* save settings
* create defaults
* validate schema
* future migrations

---

# 12. JSON Versioning (MANDATORY)

---

## Required

```json
{
  "version": 1,
  "theme": "dark"
}
```

---

## Why

Prevents future migration hell.

---

# 13. Persistence Rules

---

## 13.1 Writes

Settings writes must:

* be async
* be atomic
* never block realtime threads

---

## 13.2 Recommended

Use:

```rust
serde_json
atomicwrites
```

---

## 13.3 Save Strategy

Current phase:

```text
Immediate save on setting change
```

Allowed because:

* tiny writes
* low frequency

---

## Future Optimization

Optional debounce:

```text
multiple rapid updates
→ single disk write
```

---

# 14. IPC Commands

---

## Required Commands

```rust
get_settings
update_settings
reset_settings
```

---

# 15. Settings Update Flow

---

## Correct Architecture

```text
Frontend
↓ invoke(update_settings)
Rust updates AppState.settings
↓
Rust persists settings.json
↓
Rust emits settings_updated
↓
All windows reactively update
```

---

# 16. Theme Synchronization (CRITICAL)

---

## Requirement

Changing theme in main UI must instantly update:

* tray UI
* overlay UI
* future windows

WITHOUT:

* reload
* remount
* recreation

---

## Correct Behavior

```text
settings_updated
↓
frontend updates CSS variables / class
```

---

## ❌ Forbidden

```text
tray reads settings.json directly
```

Only Rust reads/writes persistence.

---

# 17. Frontend Architecture

---

## Frontend Must NOT

* own persistent state
* directly write files
* become source of truth

---

## Frontend Role

```text
UI renderer + event subscriber
```

---

# 18. Event System

---

## Required Events

```text
settings_updated
theme_changed
```

---

## Payload Example

```json
{
  "theme": "light"
}
```

---

# 19. Tray Synchronization

---

## Flow

```text
Main UI theme toggle
↓
invoke(update_settings)
↓
Rust updates SettingsState
↓
emit(settings_updated)
↓
Tray receives event
↓
Theme applied instantly
```

---

# 20. AppState Breakdown

---

## 20.1 Settings

Persistent user configuration.

---

## 20.2 Interaction State

Tracks:

```rust
Passive
PTT
```

---

## 20.3 Hud State

Tracks:

* tray visibility
* manual HUD toggles

---

## 20.4 PTT State

Tracks:

* recording state
* waveform state
* session buffers

---

## 20.5 Engine State

Tracks:

* audio stream
* worker lifecycle
* channels

---

# 21. Realtime Safety Rules

---

## MUST Ensure

Persistence layer NEVER touches:

* audio callback
* ring buffers
* VAD loop
* STT worker

---

## Reason

Disk I/O in realtime path destroys latency guarantees.

---

# 22. Future Extensibility

This architecture must support future additions:

---

## Future Settings

```json
{
  "models": {},
  "voices": {},
  "overlay": {},
  "shortcuts": {}
}
```

---

## Future Storage

* SQLite history
* model cache
* onboarding state
* window layouts

---

# 23. Migration Strategy (Future)

When version mismatch occurs:

```text
old config
↓
migration
↓
new schema
```

---

## MUST NEVER

Crash on invalid config.

Fallback safely.

---

# 24. Recommended File Structure

---

```text
src-tauri/src/
├── state/
│   ├── app_state.rs
│   ├── settings.rs
│   ├── interaction.rs
│   └── persistence.rs
│
├── services/
│   └── settings_manager.rs
```

---

# 25. Implementation Priority

---

## Phase 1

* AppState consolidation
* settings.json
* theme sync

---

## Phase 2

* history.db
* log rotation
* migration support

---

# 26. Final Architecture

---

```text
Frontend
    ↓
IPC Commands
    ↓
Rust AppState
    ↓
Async Persistence Layer
    ↓
settings.json
```

---

# 27. Final Principle

> Vox runtime is memory-first.

Persistence exists to:

* restore preferences
* preserve configuration
* support future extensibility

NOT to drive the realtime system.

---

# 🧠 Evaluation

---

### 🐛 BUG (avoided)

Frontend-owned persistence
→ causes desync + stale state
**Confidence: 100%**

---

### ⚖️ TRADEOFF

Immediate saves vs debounced saves
→ immediate acceptable for v1
**Confidence: 85%**

---

### 💡 IMPROVEMENT

Unified AppState
→ prevents long-term state fragmentation
**Confidence: 95%**

---
