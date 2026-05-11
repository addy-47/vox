# Vox — Phase 6 Plan
# Persistence, Configuration, Observability & Runtime Monitoring

---

# Objective

Convert Vox from a mostly hardcoded prototype into a configurable, observable, persistence-backed runtime platform without violating realtime audio constraints.

Phase 6 must preserve:
- low-latency audio pipeline
- dedicated realtime threads
- non-blocking inference
- event-driven architecture
- **Model assets localized to `~/.vox/models/`**

Persistence and monitoring systems must remain async observers/sinks and must NEVER enter hot audio paths.

---

# Core Constraints

## Realtime Constraints

The following systems are considered realtime-sensitive:
- CPAL audio callback
- playback callback
- VAD loop
- STT inference loop
- LLM token generation loop
- TTS synthesis loop

These systems:
- MUST NOT touch SQLite directly
- MUST NOT perform filesystem writes
- MUST NOT allocate excessively
- MUST NOT block on mutexes/channels
- MUST NOT perform synchronous logging

All persistence must happen asynchronously through background workers.

---

# Proposed Backend Structure

The backend structure must be reorganized into isolated domains, ( file names are refernces and not hard constraints but key focus is isolation and decouple of code logically )

src/
├── core/
│   ├── events.rs
│   ├── metrics.rs
│   ├── mod.rs
│   ├── settings.rs
│   ├── state.rs
│   └── constants.rs
│
├── services/
│   ├── audio.rs
│   ├── vad.rs
│   ├── stt.rs
│   ├── llm.rs
│   ├── tts.rs
│   ├── playback.rs
│   ├── pipeline.rs
│   ├── ptt.rs
│   └── mod.rs
│
├── persistence/
│   ├── db.rs
│   ├── schema.rs
│   ├── sessions.rs
│   ├── logs.rs
│   ├── telemetry.rs
│   └── mod.rs
│
├── telemetry/
│   ├── aggregator.rs
│   ├── system_monitor.rs
│   ├── metrics_store.rs
│   └── mod.rs
│
├── ipc/
│   ├── settings.rs
│   ├── history.rs
│   ├── monitoring.rs
│   ├── tray.rs
│   └── mod.rs
│
├── utils/
│   ├── paths.rs
│   ├── logging.rs
│   ├── cleanup.rs
│   └── mod.rs
│
├── tray.rs
├── lib.rs
└── main.rs

```
```

## Responsibilities

### core/
Shared application-level state:
- settings schema
- global events
- constants
- metrics types
- shared runtime state

### services/
Realtime runtime systems only:
- audio
- vad
- stt
- llm
- tts
- playback
- pipeline
- ptt

No persistence logic allowed here.

### persistence/
All disk/database systems:
- sqlite
- repositories
- session storage
- metrics storage
- log indexing

### telemetry/
Realtime monitoring aggregation:
- cpu/ram monitoring
- queue metrics
- runtime sampling
- telemetry aggregation

### ipc/
Frontend/backend bridge layer only:
- settings IPC
- history IPC
- monitoring IPC
- tray IPC

### utils/
Shared utilities:
- paths
- cleanup
- logging setup

---

# Standardized .vox Structure

```text
~/.vox/
├── settings.json
├── settings.json.bak
├── logs/
├── models/
├── sessions/
├── cache/
└── vox.db
```

---

# Phase Breakdown

---

# Phase 6.0 — Foundation Cleanup

## Goals

Prepare the codebase before introducing persistence.

## Tasks

- remove dead frontend settings state
- remove frontend localStorage usage
- remove fake telemetry values
- remove placeholder metrics
- remove disconnected UI controls
- centralize constants
- centralize path handling
- standardize naming
- isolate IPC commands from runtime systems

## Validation

- no localStorage remains
- all settings originate from backend state
- project compiles cleanly
- frontend builds cleanly

---

# Phase 6.1 — Settings Foundation & Runtime Config

## Goals

Create a single source of truth configuration system.

## Requirements

### Settings Source

Backend settings.json becomes the only configuration source.

Frontend must never own authoritative state.

---

## Settings Categories

At minimum support:
- audio
- VAD
- STT
- LLM
- TTS
- tray
- telemetry
- persistence
- monitoring
- logging

---

## Model Path Migration

All runtime model paths will now refer to:

```text
~/.vox/models/
```

The user has already moved the models to this location. Phase 6.0 will update all code references to use this new authoritative path via `utils::paths`.

---

## Settings Lifecycle Rules

Every setting must belong to one category:

### 1. Hot Reload
Can apply immediately.

Example:
- tray opacity
- VAD threshold
- telemetry toggle

### 2. Worker Restart
Requires rebuilding a subsystem.

Example:
- STT threads
- VAD engine
- TTS engine
- LLM context size

### 3. Full App Restart
Rare.

Example:
- audio device backend
- deep runtime initialization changes

---

## Required Handshake Flow

Frontend cannot immediately assume settings succeeded.

Required flow:

```text
UI stages setting
→ backend validates
→ backend returns requirement level
→ UI asks for confirmation if restart needed
→ UI commits
→ backend rebuilds subsystem
→ backend emits success/failure event
→ frontend updates state only after success
```

---

## Corruption Recovery

If settings.json fails to parse:
- rename corrupted file to settings.json.bak
- generate fresh defaults
- continue boot
- log failure

App must never crash on corrupted config.

---

## Validation

- runtime config centralized
- no duplicated settings state
- worker restarts function correctly
- corrupted settings recover safely

---

# Phase 6.2 — Logging Infrastructure

## Goals

Implement structured rotating logging.

## Required Libraries

- tracing
- tracing-subscriber
- tracing-appender

---

## Requirements

### Log Storage

Store logs in:

```text
~/.vox/logs/
```

### Rotation

- daily rotating logs
- maximum 3 retained files

### Logging Rules

Realtime systems:
- MUST NOT synchronously flush logs
- MUST NOT block on logging

Logging system must remain async/non-blocking.

---

## Log Categories

At minimum:
- startup
- pipeline
- vad
- stt
- llm
- tts
- playback
- persistence
- telemetry
- ipc
- errors

---

## Failure Handling

Disk IO failures:
- must not crash app
- must log fallback error
- realtime pipeline must continue running

---

## Validation

- logs rotate correctly
- app survives disk failures
- logs contain session correlation data

---

# Phase 6.3 — SQLite Persistence Layer

## Goals

Implement structured session persistence.

---

## Required Library

Use:
- rusqlite

Do NOT introduce:
- sqlx
- sea-orm
- async ORM layers

---

## SQLite Architecture

SQLite must run on a dedicated persistence worker thread.

All inserts occur via channel/event queue.

No direct DB writes allowed from:
- audio callbacks
- VAD
- STT
- LLM
- TTS
- playback

---

## SQLite Mode

Must enable:

```sql
PRAGMA journal_mode=WAL;
```

This is required to allow:
- concurrent UI reads
- background async writes

Without blocking.

---

## Required Tables

### sessions
Stores session lifecycle metadata.

### messages
Stores:
- user transcript
- assistant response

### interaction_metrics
Stores:
- TTFT
- STT latency
- TTS RTF

---

## Explicit Non-Goals

Do NOT store:
- realtime waveform data
- raw telemetry streams
- CPU graphs
- RAM graphs
- raw logs

Telemetry is ephemeral.

---

## Persistence Flow

Pipeline events
→ persistence queue
→ persistence worker
→ sqlite write

Never:
```text
realtime thread → sqlite
```

---

## Retention Rules

Implement:
- max DB growth protection
- cleanup policies
- session deletion support

Must avoid unbounded growth.

---

## Failure Handling

SQLite lock failure:
- log error
- drop persistence event
- continue runtime

Realtime pipeline must survive DB failures.

---

## Validation

- WAL mode active
- UI can read during writes
- no pipeline stalls during persistence
- history survives restarts

---

# Phase 6.4 — Telemetry & Monitoring Backend

## Goals

Create lightweight runtime observability.

---

## Required Metrics

At minimum:
- CPU usage
- RAM usage
- active threads
- VAD state
- queue depths
- playback activity
- TTFT
- TTS RTF
- STT latency

---

## Sampling Rules

Backend sampling:
- 10Hz internal sampling

Frontend IPC emission:
- 2Hz maximum
- one aggregated payload every 500ms

Do NOT spam React with high-frequency updates.

---

## Telemetry Rules

Telemetry must remain:
- ephemeral
- in-memory
- lightweight

Do NOT persist live telemetry streams.

Only final interaction metrics belong in SQLite.

---

## Aggregation Architecture

Required flow:

```text
runtime metrics
→ telemetry aggregator
→ averaged payload
→ IPC emission
```

Direct raw metric emission is prohibited.

---

## Validation

- telemetry updates smoothly
- no IPC flooding
- no UI-induced pipeline lag

---

# Phase 6.5 — Settings UI Rewrite

## Goals

Replace placeholder UI with fully runtime-backed settings.

---

## Required Changes

Remove:
- fake dropdowns
- fake latency values
- localStorage
- disconnected toggles
- placeholder state

Replace with:
- real backend state
- runtime validation
- restart indicators
- persistence status
- settings categories

---

## UI Requirements

Settings UI must clearly indicate:
- hot reload
- requires worker restart
- requires app restart

User must confirm restart-required changes.

---

## Validation

- frontend reflects backend state only
- restart flow works correctly
- settings survive reboot

---

# Phase 6.6 — History & Logs UI Rewrite

## Goals

Replace mocked history page with real persistence-backed UI.

---

## Required Features

### Session History

Show:
- sessions
- timestamps
- trigger type
- transcripts
- assistant replies

### Metrics

Display:
- TTFT
- STT latency
- TTS RTF

### Logs

Support:
- filtering
- searching
- session correlation

---

## Requirements

UI queries must not block persistence worker.

Use paginated/history-safe queries.

---

## Validation

- session history loads correctly
- large history does not freeze UI
- realtime pipeline unaffected

---

# Phase 6.7 — Monitoring Dashboard UI

## Goals

Create a lightweight runtime monitoring dashboard.

---

## Required Frontend Library

Use:
- recharts

---

## Required Metrics

At minimum:
- CPU
- RAM
- active threads
- VAD state
- playback state
- queue activity
- TTFT
- STT latency
- TTS RTF

---

## Rendering Rules

Monitoring components must:
- use React.memo
- avoid parent render storms
- render only on payload change

Realtime charts must not trigger full-page rerenders.

---

## UX Rules

Dashboard must remain:
- functional
- readable
- lightweight

Avoid decorative/fake cyberpunk telemetry.

Only show real actionable runtime metrics.

---

## Validation

- UI remains smooth during speech
- monitoring does not increase latency
- charts update at stable 2Hz

---

# Global Architectural Rules

## Event-Driven Only

All persistence and telemetry systems must remain event-driven.

Required pattern:

```text
runtime systems
→ events
→ async observers
→ persistence/UI/logging
```

---

## No Realtime Thread Violations

Realtime systems must never:
- block on disk
- wait on sqlite
- wait on frontend
- wait on logging

---

## Single Source of Truth

Backend owns:
- settings
- persistence
- runtime state
- **Model file paths**

Frontend reflects backend state only.

---

# Completion Criteria

Phase 6 is complete only when:

- settings are centralized
- runtime config lifecycle works
- models load from .vox/models
- rotating logs function correctly
- SQLite persistence is stable
- WAL mode verified
- monitoring dashboard works
- history UI is real
- no localStorage remains
- no fake telemetry remains
- no hot-path blocking introduced
- realtime latency remains stable

---

# Critical Failure Conditions

The phase is considered failed if any of the following occur:

- audio glitches introduced
- VAD lag increases noticeably
- playback underruns appear
- React rerender storms occur
- DB writes block inference
- telemetry floods IPC
- frontend owns authoritative settings
- persistence failures crash runtime
