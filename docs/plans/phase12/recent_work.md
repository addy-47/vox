# Phase 12 — Recent Work & Agentic Runtime Ledger

This document tracks agentic turn execution, tool-calling architectures, MCP integrations, and test engineering for Phase 12.
For high-level workspace invariants and active guidelines, refer to [AGENTS.md](file:///home/addy/projects/apps/vox/AGENTS.md).

> 📖 **Phase 11 Archive:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

---

## Phase 11 Summary

> Compact summary of Phase 11 outcomes to provide context for Phase 12 work.

- **Full Keyboard Contract & Spatial Navigation:** Complete click-parity keyboard handling (`shortcuts.ts` SSOT), zone-bounded spatial navigation via `@noriginmedia/norigin-spatial-navigation`, context-aware drawer dispatcher (`PageDrawerContext.tsx`), and floating tooltip system (`@floating-ui/react`).
- **Memory Profiler Precision & Zero-Leak Soak:** Upgraded Linux kernel `/proc/<pid>/status` & `smaps_rollup` metrics (PSS, Anon Heap vs Shared isolation); verified 0-leak soak across 47 telemetry snapshots (164 constant DOM nodes, negative heap drift -0.465 MB/min).
- **Cloud Reasoning Multi-Provider Key Persistence:** Added persistent `cloud_keys: HashMap<String, String>` across 35+ providers, redesigned `LlmConfigDesk.tsx` 2-column grid, and clean underline tab system.

---

## Past Work (2026-09-21)

- **Phase 12 Initialized:** Transitioned from Phase 11 to Phase 12 ("Agentic Vox"). Scoped normalized LLM event stream, tool execution loop, duplex dialogue pipe extensions, MCP client integration, and initial tool capability (`generate_title`).
