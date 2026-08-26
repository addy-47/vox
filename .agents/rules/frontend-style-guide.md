---
trigger: manual
description: Vox Frontend Code Style Guide and Engineering Standards for TypeScript/React (`app/src/`).
---

# Vox — Frontend Code Style Guide & Engineering Standards

This document contains durable coding standards for the Vox frontend (`app/src/`). **Agents doing write operations on frontend code must read this file before modifying code.**

---

## 1. Hardware Tiers & Feature Mapping

The frontend reflects the active hardware tier and dynamic model degradation/upgrades in the ambient UI.

| Tier | Hardware | Pipeline Mode | Memory Ingestion | Memory Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :--------------: | :--------------: | :----------: |
| **1A** | 8GB, CPU-only, no GPU | Modular (Local) | ❌ None (FIFO only) | ✅ Working Memory context window only | ❌ Unavailable |
| **1B** ⭐ | 8GB+, dedicated GPU | Modular (Local) | ✅ Full async ingestion | ✅ Full retrieval (episodic + semantic) | ⚠️ Depends on local LLM capability |
| **2A** ⭐ | Hybrid (Remote LLM + Local Audio) | Modular (Remote LLM) | ✅ Full async ingestion | ✅ Full retrieval | ⚠️ Depends on remote LLM capability |
| **2B** ⭐ default | Hybrid (Cloud LLM + Local Audio) | Modular (Cloud LLM) | ✅ Full async ingestion | ✅ Full retrieval | ✅ All cloud models support tool calling |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Provider-managed | ✅ Via early tool calls in provider | ✅ Via early tool calls |

---

## 2. Frontend Architecture & Boundaries

- **Package Manager:** Always use `pnpm`, never `npm` or `yarn`.
- **Zero Hardcoded Text / Labels:** Inline hardcoded strings, labels, select options, or mock objects inside components/pages are strictly banned. All static content must live in `src/data/` (e.g., `appData.ts`, `settingsDomains.ts`).
- **Strict Service Layer Boundary:** Raw `@tauri-apps/api` invoke calls or direct fetches inside React components are prohibited. All IPC/API calls MUST pass through dedicated service modules in `src/services/` (e.g. `pipelineService.ts`, `settingsService.ts`).
- **Page Responsibility (Layout Only):** Files in `src/pages/` MUST only define visual structure, routing, and layout composition. Business logic, state sync, and data transformations belong in `src/services/`, `src/hooks/`, or `src/store/`.
- **Modular Component Subdirectories:** `src/shared/components/` must be structured into logical feature/domain subdirectories (e.g., `layout/`, `home/`, `history/`, `settings/`, `monitoring/`, `common/`). Flat, uncategorized component directories are banned.
- **Type Safety:** Strict TypeScript. `any` is strictly prohibited — define explicit interfaces/types for all props and service returns.
- **Verification:** Run `pnpm lint` and `pnpm build` after every modification. Zero warnings/errors permitted.

---

## 3. State Management & Reactivity

- **Transient High-Frequency UI State:** Audio visualizers, waveform amplitudes, and 60fps animation states live in local component state or mutable refs (`useRef`). Never store high-frequency animation values in global React Context.
- **Low-Frequency App State:** App configuration, settings, active tier, and connection status live in React Context or Zustand (`src/store/`).
- **Reusable Stateful Logic:** Extract into custom hooks (`src/hooks/`) whenever logic appears in 2+ components.

---

## 4. Liquid Space Design System & Layout Rules

- **Desktop Layout:** Floating bottom `EdgeNav` capsule, monitoring panel as popover bottom-left.
- **Mobile Layout:** Monitoring moves to `/monitoring` route with solid background. Nav capsule gets a 4th tab.
- **Viewport Transitions:** Mobile → desktop redirects from `/monitoring` to `/` and relaunches popover. Desktop → mobile closes popover and routes to `/monitoring`.
- **Orb Responsive Scaling:** Mobile Orb scales to `min(92vw, 85vh)`. Desktop Orb is `min(70vw, 65vh)`.
- **Glass Elevation is a Closed System:** Use only defined design token elevation levels. Do not invent arbitrary ad-hoc shadows or glass backgrounds.
- **Mood Sync:** Visual elements must reflect backend pipeline states (`Idle`, `Ready`, `Listening`, `Thinking`, `Speaking`, `Paused`, `Error`).

---

## 5. Documentation Standards

Root architecture and feature docs in `docs/*.md` follow a uniform frontmatter + "How to read" convention:

### 5.1 Required Frontmatter (YAML)
```yaml
---
title: "Doc Title"
audience: "Internal — <who this is for>"
last_updated: YYYY-MM-DD
owners: "frontend-engineer role"
related_docs:
  - "docs/other.md — one-line relationship"
---
```

### 5.2 Required "How to read this doc" Section
Immediately after the title, include:
- **Audience:** who the doc is for.
- **Scope:** what it covers.
- **Convention:** how claims are cited (`path/file.ts` pointers; no invented code blocks).
- **Non-goals:** what it is explicitly NOT (with cross-links).
- **SSOT:** where the authoritative detail lives.
