# Notification System Behavioral & Interface Specification (Vox v2)

## 1. Name & Concept
This specification defines the behavioral contract, storage invariants, correlation semantics, and presentation rules for the Vox desktop application's persistent notification center.

## 2. Purpose
The notification center provides users with an auditable, non-intrusive drawer of actionable tasks and historical system alerts. It bridges the gap between ephemeral voice turn toasts (which disappear after a few seconds) and long-running application state, ensuring that required user actions (such as compacting conversation memory or retrying a failed operation) and critical errors (such as expired API keys or audio disconnects) are never silently lost.

---

## 3. Core Concepts & Classification

Notifications in Vox fall into two distinct conceptual classes:

1. **Actionable Task Cards (Entity-Bound Tasks)**:
   - Represent an outstanding unit of work tied to a specific domain entity (for example, an uncompacted session or a missed daily consolidation).
   - Must be strictly idempotent: there can only ever be **one active task card** for a given entity at any time.
   - Present a clear primary action button (such as "Tidy Now" or "Consolidate") that initiates the task.
   - When the user triggers the action, the task initiates in the background without corrupting the card's read/unread lifecycle.

2. **System Event Alerts (Point-in-Time Telemetry)**:
   - Represent significant system occurrences, warnings, or failures (for example, LLM provider timeout, audio device disconnection, or context overflow).
   - Are historical records: if a failure occurs 5 times over an hour, all 5 occurrences represent real events.
   - Are correlated by an event signature so that repetitive alerts do not flood the user interface.

---

## 4. Must Be True (Invariants & Behavioral Contracts)

### 4.1 Storage & Auditability Invariants
1. **Append-Oriented Storage**: Every emitted notification must be recorded as an independent record with its own unique identifier and timestamp. Emitting a new alert must never overwrite or destroy the creation timestamp or message of past alerts.
2. **Correlation Key Required**: Every notification record must carry a correlation key that identifies its grouping domain:
   - Entity tasks use their entity identifier (e.g., session task keys include the session number; scheduled consolidation keys include the scheduled date).
   - System alerts use their error category and source (e.g., provider timeouts share an error signature key).
3. **Card Lifecycle Exclusivity**: A notification record has exactly three user-facing states:
   - `unread`: The notification has not yet been reviewed or acted upon by the user. Contributes to unread badge counts.
   - `read`: The notification has been viewed or marked as read. Does not contribute to unread badge counts, but remains visible in the notification drawer.
   - `dismissed`: The notification has been explicitly cleared by the user. Hidden from the active drawer.
4. **Decoupling from Background Job Progress**: Card status must strictly govern user attention (`unread`, `read`, `dismissed`). The progress of background jobs (such as whether a compaction is queued, running, succeeded, or failed) must never overwrite the user card status. Job progress belongs to auxiliary payload data or transient application state.

### 4.2 Producer & Idempotency Rules
5. **Task Card Idempotency**: Before creating an actionable task card for an entity, the producer must check whether an active (unread or read) card already exists with that correlation key. If an active card exists, a new card must not be created.
6. **Dismissal Respect**: If a user explicitly dismisses a task card for an entity, the system must not automatically re-create that same task card on every subsequent boot or turn, unless the underlying entity experiences a material change in state (e.g., new turns added to the session).
7. **Severity Levels**: Every notification must specify one of three severity levels:
   - `info`: Informational events and routine task recommendations.
   - `warning`: Recoverable issues, degraded operating conditions, or missed background schedules.
   - `critical`: Unrecoverable errors, missing credentials, hardware disconnects, or operations requiring immediate user intervention.
8. **Plain Language Contract**: Notification titles and descriptions must use human-readable plain language explaining what happened and what action is required. Internal system codes, raw stack traces, and technical category slugs must never be displayed as user-facing copy.

### 4.3 Presentation & Drawer Contracts
9. **Unified Chronological Stream**: The notification drawer must display notifications in a single unified timeline ordered newest first. It must not fragment the main drawer into separate isolated category folders or nested accordions.
10. **Visual Rollup for Repetitive Alerts**: When multiple notifications share the same correlation key and remain unread/read, the user interface must visually roll them up into a single representative card displaying:
    - The latest title and message.
    - The recency timestamp of the most recent occurrence.
    - An occurrence count indicator (e.g., `×4`) indicating that multiple instances occurred.
11. **Entity Task Distinction**: Tasks for different entities (such as Session A and Session B) carry different correlation keys and must always render as distinct cards, each with its own action controls.
12. **Navigation & Deep Linking**: Clicking a notification card or its primary action must navigate the user directly to the relevant context:
    - Session tasks navigate to that session in History.
    - Memory alerts navigate to the Memory graph view.
    - Provider, auth, or hardware alerts navigate to the relevant Settings tab.
13. **Bulk Actions**: The drawer must provide a global action to mark all unread notifications as read, and an action to dismiss cards.

---

## 5. Must Not Happen (Strict Negative Constraints)

1. **No Silent Destruction of History**: The system must not enforce a database-level uniqueness constraint on correlation keys that causes new events to overwrite and erase past notification timestamps or records.
2. **No Task Card Duplication**: The system must not create multiple concurrent active cards prompting the user to perform the exact same action on the exact same entity.
3. **No Alert Flooding**: The system must not present 50 identical cards in the drawer when a background service repeatedly fails in a tight loop. High-frequency alerts must roll up into a single aggregated card.
4. **No Category as Entity Folders**: The drawer must not lock notifications into rigid category buckets that destroy the cross-domain chronological context of what happened when.
5. **No Technical Slugs in UI**: Raw category identifiers (e.g., `session_compaction`, `auth_failure`) must never be rendered as text in the user interface; category determines visual accents, icons, and routing only.

---

## 6. Out of Scope

- **Ephemeral Voice Toasts**: Voice turn audio feedback toasts displayed on the desktop canvas are transient and governed by `events-spec.md`.
- **Operating System Desktop Banners**: Integration with OS-native notification centers (macOS NotificationCenter, Windows Action Center, Linux libnotify) is not covered by this specification and may be addressed in a future integration.
- **Compaction & Memory Execution**: The algorithms and execution loops for memory compaction and consolidation are governed by `memory-spec.md`.

---

## 7. Open Questions

*None. Architectural direction confirmed: append storage with correlation key, producer idempotency for tasks, and frontend rollup aggregation.*
