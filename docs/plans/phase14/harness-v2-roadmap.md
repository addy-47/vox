# Harness v2 Architecture Master Roadmap

This document is the authoritative master tracking ledger for the Vox Harness v2 transformation.
Each domain represents an independent architectural layer executed in strict sequential order.

---

## Domain Progress Overview

```mermaid
flowchart LR
    D1["Domain 1: 3-Stream Demuxing"] --> D2["Domain 2: ReAct Step Loop"]
    D2 --> D3["Domain 3: Guardrails & Spillover"]
    D3 --> D4["Domain 4: Mid-Work Interruption"]
    D4 --> D5["Domain 5: Background Tasks"]
    D5 --> D6["Domain 6: Core Tool Registry"]
```

| # | Domain | Artifact Link | Status |
| :-: | :--- | :--- | :--- |
| **1** | **3-Stream Demuxing & Interim Speech** | [domain-1-3-stream-demuxing.md](file:///home/addy/.gemini/antigravity/brain/0c12a585-e6e0-4aad-bda8-dead1cbc64a6/domain-1-3-stream-demuxing.md) | **In Discussion** |
| **2** | **ReAct Step Loop & Concurrency Barriers** | `domain-2-react-loop.md` | Pending |
| **3** | **Loop Guardrails & Context Spillover** | `domain-3-guardrails.md` | Pending |
| **4** | **Mid-Work Interruption & Step Steering** | `domain-4-interruption.md` | Pending |
| **5** | **Background Tasks & Proactive Wake-Up** | `domain-5-background-tasks.md` | Pending |
| **6** | **Core Built-In Tool Registry** | `domain-6-tool-registry.md` | Pending |

---

## Domain Checklists

### Domain 1: 3-Stream Demuxing & Interim Speech
- [ ] **Discussion & Architecture Alignment:** Review v1 dropped prefix text vs v2 3-stream demuxing, `Thinking` isolation, and `InterimFiller`.
- [ ] **Spec Formalization:** Lock boundary types in [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) and [events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md).
- [ ] **Implementation Gate (User Team):** Orchestrate tickets, write unit tests, implement code, and run internal review.
- [ ] **Architect Verification & Parity Audit:** Principal architect verifies code diffs, target compilation gate (`verify-targets.sh`), and runtime parity.
- [ ] **Domain Completed**

### Domain 2: ReAct Step Loop, Concurrency Barriers & Step Commits
- [ ] **Discussion & Architecture Alignment:** Review removal of `ToolFlow::Terminal`, reentrant 20-step loop, `ToolExecutionMode`, and immediate step persistence.
- [ ] **Spec Formalization:** Update loop state machine in [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md).
- [ ] **Implementation Gate (User Team):** Tickets, tests, code, and review.
- [ ] **Architect Verification & Parity Audit:** Validate loop invariants, cancellation checks, and Turso DB step recording.
- [ ] **Domain Completed**

### Domain 3: Loop Guardrails & Context Budget Truncation
- [ ] **Discussion & Architecture Alignment:** Review 2-strike signature detector (`A -> B -> A -> B`), head/tail disk spillover, and inline budget evaluation.
- [ ] **Spec Formalization:** Lock guardrail rules and cache spillover layout in [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md) and [storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md).
- [ ] **Implementation Gate (User Team):** Tickets, tests, code, and review.
- [ ] **Architect Verification & Parity Audit:** Audit guardrail test assertions and file leak protections.
- [ ] **Domain Completed**

### Domain 4: Two-Phase Mid-Work Interruption & Step Steering
- [ ] **Discussion & Architecture Alignment:** Review `SpeechStart` hold in `Working` without killing in-flight tools, `StepInbox`, and empty speech recovery.
- [ ] **Spec Formalization:** Update interaction state transitions in [events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md) and [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md).
- [ ] **Implementation Gate (User Team):** Tickets, tests, code, and review.
- [ ] **Architect Verification & Parity Audit:** Verify zero audio clicks, safe cancellation token handling, and speech-hold concurrency.
- [ ] **Domain Completed**

### Domain 5: Background Tasks, Timeout Promotion & Router Wake-Up
- [ ] **Discussion & Architecture Alignment:** Review `BackgroundTaskManager`, 13-second auto-promotion on shell tasks, and proactive voice wake-ups on `Ready`.
- [ ] **Spec Formalization:** Lock task lifecycle events and router dispatch in [events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md) and [ipc-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md).
- [ ] **Implementation Gate (User Team):** Tickets, tests, code, and review.
- [ ] **Architect Verification & Parity Audit:** Validate cross-platform process trees (Linux, Android) and task log cleanup.
- [ ] **Domain Completed**

### Domain 6: Foundational Built-In Tool Registry
- [ ] **Discussion & Architecture Alignment:** Review the 8 core tools (`set_title`, `search_memory`, `web_search`, `read_file`, `write_file`, `bash`, `manage_task`, `ask_user`) and removal of `spoken_filler`.
- [ ] **Spec Formalization:** Author authoritative [tools-spec.md](file:///home/addy/projects/apps/vox/docs/specs/tools-spec.md).
- [ ] **Implementation Gate (User Team):** Tickets, tests, code, and review.
- [ ] **Architect Verification & Parity Audit:** Audit sandboxing, path traversal guards, secret isolation, and parameter schemas.
- [ ] **Domain Completed**
