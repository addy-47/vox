---
title: "Vox Edge Panel Contract — Help, Notifications, and Sessions"
last_updated: 2026-09-09
owners: "frontend-engineer role"
related_docs:
  - "docs/design.md — Liquid Space overlay topology"
  - "docs/frontend.md — frontend architecture and shared layer"
  - ".agents/rules/frontend-style-guide.md — React/TS standards"
  - ".agents/rules/frontend-engineer.md — frontend role invariants"
  - "docs/specs/ipc-spec.md — IPC command and event contracts"
---

# Vox Edge Panel Contract

## Goal

Replace the fragmented Help drawer, Notifications popover, and Home conversation rail with one shared edge-panel primitive, one central visibility contract, and three focused panel-content modules.

The shipped main-app topology will be:

- Top-right corner: **Notifications, Help** (in that order, left to right).
- Top-right panels: full-height, resizable right-edge rails. Help and Notifications are mutually exclusive.
- Top-left corner on Home: Conversations trigger and a full-height, resizable left-edge rail.
- Bottom corners: popover surfaces such as Monitoring.
- Central cards/nodes: bottom drawers.
- Monitoring has no Help or Notifications controls.
- The memory profiler remains a debug surface and is not part of the shipped panel contract.

This is an established-world refinement: preserve Vox product behavior and Liquid Space tokens while replacing the local panel grammar. Do not create `PRODUCT.md`, replace the global visual world, change backend/IPC, or alter the profiler.

## Locked Decisions

| # | Decision | Resolved answer |
|---|---|---|
| 1 | Panel edges | Help and Notifications both open from the right edge. |
| 2 | Trigger order | Notifications is immediately left of Help; Help is the rightmost icon. |
| 3 | Page coverage | Home, History, Memory, and Settings use the same top-right cluster. Monitoring is excluded. |
| 4 | Settings placement | Remove per-card Help triggers. Put both icons in the Settings top-right corner on desktop and mobile. |
| 5 | Settings Help context | Settings Help always opens `settings:overview`; domain-specific Help links are removed. |
| 6 | Right-panel exclusivity | Opening one right rail closes the other. The trigger becomes part of the open rail, so the hidden trigger cannot be used to open a second panel. |
| 7 | Session rail | Home Conversations opens a left rail. It is independent of the right-side Help/Notifications group. |
| 8 | Geometry | Panels are full-height edge rails. The trigger remains embedded in app chrome and the rail collapses back into it. |
| 9 | Sizing | Desktop width is viewport-relative, defaulting to `22vw` with a `20vw`–`25vw` resize range. Compact/mobile viewports use a full-width rail while preserving the edge trigger. |
| 10 | Dismissal | Outside pointer-down closes the topmost rail through the global overlay authority. Escape remains FILO. No per-panel Escape or outside-click listeners. |
| 11 | Visual direction | Borrow the docked trigger-to-panel interaction from Cursor/Antigravity, but retain Liquid Space tokens, glass elevation, typography, and ambient behavior. |
| 12 | Content | Help, Notifications, and Conversations content may be restructured, but existing data, actions, deep links that remain in scope, and navigation targets must continue to work. |
| 13 | Spec alignment | Add a small overlay-topology section to `docs/design.md` before implementation. Do not rewrite the design system or create a new global visual world. |
| 14 | Product record | UI-only task; `PRODUCT.md` creation is explicitly out of scope. |

## The Six Files

Exactly **six files** are involved. No more, no less.

| # | File | Location | Purpose |
|---|---|---|---|
| 1 | `EdgePanel.tsx` | `app/src/shared/components/ui/` | Rail chrome — header, close, resize, body slot for children. Presentational only. |
| 2 | `TopRightCluster.tsx` | `app/src/shared/components/ui/` | Updated trigger cluster (Notifications left of Help). Already exists, just fix order and simplify triggers. |
| 3 | `HelpPanel.tsx` | `app/src/shared/components/common/` | Help content rendered inside EdgePanel. Uses `helpCopy.ts` data. |
| 4 | `NotificationPanel.tsx` | `app/src/shared/components/common/` | Notification content rendered inside EdgePanel. Uses `notificationStore` + `notificationCopy.ts`. |
| 5 | `SessionPanel.tsx` | `app/src/shared/components/home/` | Session content rendered inside EdgePanel (home only). New component, replaces `SessionRail`. |
| 6 | `usePanelState.ts` | `app/src/shared/hooks/` | Panel visibility state hook. Replaces `HelpDrawerProvider` and `notificationStore.isOpen`. |

Supporting hook: `useSessionPanel.ts` also goes in `app/src/shared/hooks/` — handles data fetching, project grouping, pinning, and create session/project calls.

### EdgePanel (`app/src/shared/components/ui/EdgePanel.tsx`)

Presentational rail chrome. Not content, not data.

```ts
interface EdgePanelProps {
  side: "left" | "right";
  open: boolean;
  onClose: () => void;
  title: string;
  headerActions?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}
```

- Full-height rail from top to bottom (below titlebar)
- Viewport-relative width (`22vw` default, `20vw`–`25vw` range)
- Full-width on compact/mobile viewports
- Registers with `useOverlay` + `overlayStack.ts` for global dismissal
- Focus entry on open, focus restoration on close
- `prefers-reduced-motion` respected
- Uses existing `glass-card` styling

### TopRightCluster (`app/src/shared/components/ui/TopRightCluster.tsx`)

Update the existing file in place:

1. Fix order: Notifications trigger left of Help trigger.
2. Simplify triggers — both call `usePanelState` to open the corresponding panel. No separate `HelpTriggerButton.tsx`.

`NotificationBell` keeps its data-fetching and listener lifecycle but the trigger button calls `openPanel("notifications")` instead of toggling its own `isOpen`.

### HelpPanel (`app/src/shared/components/common/HelpPanel.tsx`)

Renders inside EdgePanel as children. Presentational only — receives data from `helpCopy.ts`. Uses `helpService.deriveTier()` for tier filtering. All copy from `helpCopy.ts`.

### NotificationPanel (`app/src/shared/components/common/NotificationPanel.tsx`)

Renders inside EdgePanel as children. Receives data from `notificationStore`. Uses `markAllRead`, `dismiss`, `triggerCompaction` actions. All copy from `notificationCopy.ts`.

### SessionPanel (`app/src/shared/components/home/SessionPanel.tsx`)

Home-only. Renders inside EdgePanel (side="left"). Uses `useSessionPanel()` hook for data. Structure:

```
┌─ Header: "Conversations" + close button ─────┐
├─ [+] New Session (plus → createSession IPC) ──┤
├─ PINNED SESSIONS ────────────────────────────┤
│  ├─ Session row (pin icon)                    │
│  └─ ...                                       │
├─ PROJECTS ────────────────────────────────────┤
│  ├─ Project A (folder icon + [+] create proj) │
│  │  ├─ Session row (pin icon)                 │
│  │  └─ Session row                             │
│  ├─ Project B (folder icon + [+] create proj) │
│  │  ├─ Session row                             │
│  │  └─ ...                                     │
│  └─ ...                                        │
└────────────────────────────────────────────────┘
```

**SessionPanel internals**:
- Plus icon → `createSession()` IPC (from `historyService.createSession()`)
- Pinned sessions: `is_pinned === true` from `getSessions()`, displayed first
- Projects grouped by `project_id`, each with folder icon + plus icon → `createProject()` IPC (from `projectService.createProject()`)
- Pin icon on each session row → `updateSession()` to toggle `is_pinned`
- Drag-and-drop reordering of projects
- Session selection → `selectSession()` IPC, then close panel

### usePanelState (`app/src/shared/hooks/usePanelState.ts`)

Simple hook managing which panel is open.

```ts
type PanelId = "help" | "notifications" | "sessions";

interface PanelState {
  activePanel: PanelId | null;
  openPanel: (id: PanelId) => void;
  closePanel: (id: PanelId) => void;
  togglePanel: (id: PanelId) => void;
  closeRightGroup: () => void;
  isPanelOpen: (id: PanelId) => boolean;
}
```

Rules:
- `help` and `notifications` are one exclusive right-side group
- `sessions` is an independent left-side group
- Mounted once in `App.tsx`

### useSessionPanel (`app/src/shared/hooks/useSessionPanel.ts`)

Supporting hook for SessionPanel data.

- `projectService.getProjects()` → `ProjectRow[]`
- `historyService.getSessions()` → `SessionRow[]`
- `eventsService.onSessionsChanged()` → refresh
- `historyService.createSession()` → new session
- `projectService.createProject()` → new project
- `historyService.updateSession()` → toggle pin
- Groups: `pinnedSessions`, `projects: Map<projectId, { project, sessions }>`, `uncategorized`

## Service Layer (unchanged)

All data-fetching and IPC calls stay where they are:

| Service | Purpose |
|---|---|
| `historyService.ts` | `getSessions()`, `createSession()`, `updateSession()`, `sortSessionsNewestFirst()`, `resolveSessionTitle()` |
| `projectService.ts` | `getProjects()`, `createProject()`, `renameProject()`, `deleteProject()` |
| `notificationService.ts` | `getNotifications()`, `markNotificationsRead()`, `dismissNotification()`, `triggerSessionCompaction()` |
| `helpService.ts` | `deriveTier()` for help tier filtering |
| `eventsService.ts` | `onSessionsChanged()` for reactive updates |

## Store Layer (minimal changes)

| Store | Changes |
|---|---|
| `sessionStore.ts` | No changes |
| `notificationStore.ts` | Remove `isOpen: boolean` and `setIsOpen` fields. Keep `notifications`, `fetchNotifications`, `initListeners`, `markAllRead`, `dismiss`, `triggerCompaction` |
| `settingsStore.ts` | No changes |

## What Gets Deleted

### Help directory (entire directory deleted)
- `app/src/shared/components/help/HelpDrawer.tsx`
- `app/src/shared/components/help/HelpDrawerProvider.tsx`
- `app/src/shared/components/help/HelpTriggerButton.tsx`
- `app/src/shared/components/help/HelpContent.tsx`
- `app/src/shared/components/help/HelpArticle.tsx`
- `app/src/shared/components/help/HelpEmptyState.tsx`
- `app/src/shared/components/help/HelpToc.tsx`
- `app/src/shared/components/help/HelpPinnedCrumb.tsx`
- `app/src/shared/components/help/HelpTierBadge.tsx`
- `app/src/shared/components/help/` directory

### Home directory (specific files deleted)
- `app/src/shared/components/home/NotificationBell.tsx`
- `app/src/shared/components/home/NotificationsPopover.tsx`
- `app/src/shared/components/home/SessionRail.tsx`
- `app/src/shared/components/home/index.ts` — remove exports for NotificationBell, NotificationsPopover, SessionRail

### Settings per-card Help triggers
- `app/src/shared/components/settings/SettingsCardWrapper.tsx` — remove `HelpTriggerButton` import and per-card Help trigger block (lines 80-89)

### Store field removed
- `app/src/store/notificationStore.ts` — remove `isOpen`/`setIsOpen` from `NotificationStoreState`

### TopRightCluster move
- Move `app/src/shared/components/common/TopRightCluster.tsx` → `app/src/shared/components/ui/TopRightCluster.tsx`
- Remove from `app/src/shared/components/common/index.ts`
- Add to `app/src/shared/components/ui/index.ts`

### Keep these unchanged
- `app/src/data/helpCopy.ts`, `notificationCopy.ts`, `sessionCopy.ts`, `layoutCopy.ts`
- `app/src/shared/ui/Drawer.tsx`, `overlayStack.ts`, `useOverlay.ts`
- `app/src/services/` (all files)
- `app/src/store/sessionStore.ts`, `settingsStore.ts`

## Implementation Batches

### Batch 0 — Spec-first overlay topology

**Dependencies:** None.
**Expected build state:** Green; documentation only.

Add a concise section to `docs/design.md`:
1. Top-corner triggers open edge rails.
2. Bottom-corner triggers open popovers.
3. Central cards/nodes open bottom drawers.
4. Right-side Help/Notifications share one exclusive group.
5. Left/right groups are independent.
6. Outside-click and Escape use the global FILO authority.
7. The profiler is excluded from the shipped contract.

### Batch 1 — Foundation files

**Dependencies:** Batch 0.
**Expected build state:** Green; new files are additive, old files still present.

Create:
1. `app/src/shared/components/ui/EdgePanel.tsx` — Shared edge rail chrome
2. `app/src/shared/hooks/usePanelState.ts` — Panel state hook
3. `app/src/shared/hooks/useSessionPanel.ts` — Session panel data hook
4. Move `TopRightCluster.tsx` from `common/` to `ui/`

Update:
- `app/src/shared/components/common/TopRightCluster.tsx` — delete (moved)
- `app/src/shared/components/common/index.ts` — remove TopRightCluster export
- `app/src/shared/components/ui/index.ts` — add EdgePanel, TopRightCluster exports
- `app/src/store/notificationStore.ts` — remove `isOpen`/`setIsOpen`
- `app/src/App.tsx` — mount `usePanelState` provider once

### Batch 2 — Panel content modules

**Dependencies:** Batch 1.
**Expected build state:** May be temporarily red; must be green when complete.

Create:
5. `app/src/shared/components/common/HelpPanel.tsx` — Help content inside EdgePanel
6. `app/src/shared/components/common/NotificationPanel.tsx` — Notification content inside EdgePanel
7. `app/src/shared/components/home/SessionPanel.tsx` — Session content inside EdgePanel (home only)

### Batch 3 — Atomic consumer migration

**Dependencies:** Batches 1–2.
**Expected build state:** Must be green when complete. No old panel path or legacy export may remain.

In one atomic pass:

1. Replace Help drawer/provider/trigger with `EdgePanel` + `HelpPanel`, controlled by `usePanelState`.
2. Replace Notification bell/popover with `EdgePanel` + `NotificationPanel`, controlled by `usePanelState`.
3. Replace Home `SessionRail` with `EdgePanel` (side="left") + `SessionPanel`, controlled by `usePanelState`.
4. Remove `SessionRail` usage from `Home.tsx`, `useOverlay` call, and `railOpen` state.
5. Remove `HelpDrawerProvider` from `App.tsx`.
6. Move notification fetch/listener initialization to one app-level mount (inside `NotificationPanel`).
7. Remove `isOpen`/`setIsOpen` from all callers of `notificationStore`.
8. Mount the canonical cluster (`TopRightCluster`) on Home, History, Memory, and Settings.
9. Remove the cluster from Monitoring.
10. Remove Settings per-card Help triggers (`SettingsCardWrapper.tsx` HelpTriggerButton block).
11. Delete all help directory files, `NotificationsPopover.tsx`, `SessionRail.tsx`, `NotificationBell.tsx`.
12. Update barrels and delete legacy symbols; do not leave compatibility re-exports.

### Batch 4 — Content and visual redesign

**Dependencies:** Batch 3.
**Expected build state:** Green throughout; content/layout-only changes.

Redesign the three panel bodies within Liquid Space:
- **Help:** clear header, guide navigation, article content, empty state, stable scroll.
- **Notifications:** header with unread count, mark-all, dense list, dismiss controls, states.
- **Sessions:** plus button, pinned section, projects grouped with folder headers, pin icons, drag-and-drop.

Keep the existing bottom `Drawer` for central surfaces.

### Batch 5 — Verification and finish

**Dependencies:** Batches 0–4.

Run validation, inspect desktop and compact viewports, fix defects, rerun. Run Impeccable detector once over changed UI targets after final build.

## Validation Matrix

### Automated

From `app/`:
```bash
pnpm build
pnpm test
git diff --check
```

Also verify statically:
- no raw `@tauri-apps/api` calls in components;
- no new `any` types;
- no hardcoded visible strings;
- no panel has local Escape/outside-click listener;
- provider values are memoized;
- Zustand reads use atomic selectors;
- no legacy panel export remains;
- `pnpm lint` is clean (if available);
- `SettingsCardWrapper` no longer imports `HelpTriggerButton`.

### Manual behavior

1. Home, History, Memory, Settings show same top-right cluster.
2. Cluster order is Notifications then Help; Help is rightmost.
3. Monitoring shows neither trigger.
4. Settings has no per-card Help trigger; always opens `settings:overview`.
5. Opening Help closes Notifications; vice versa.
6. Open rail embeds its trigger; hidden trigger cannot open a second rail.
7. Home Conversations opens left rail, coexists with right rail.
8. Outside pointer-down closes topmost rail.
9. Escape closes overlays FILO.
10. Focus enters open rail, returns to trigger on close.
11. Rails resize `20vw`–`25vw` on desktop, full-width on compact.
12. Help deep links, notification actions, conversation selection, pin, create session, create project all work.
13. No panel obscures bottom navigation or Tauri chrome.
14. Reduced-motion mode disables nonessential panel motion.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Duplicate notification listeners during route changes | Initialize once at app level; remove page-trigger lifecycle effects. |
| Trigger and portaled rail treated as separate outside-click targets | Use existing `useOverlay` + `overlayStack.ts` composite boundary. |
| Full-height glass blur harms constrained hardware | Keep within existing closed glass system; bound animated surface; memoize children. |
| Settings cluster collides with radial-hub cards | Page-level top-right chrome with stable z-index and responsive spacing. |
| Mobile full-width rail covers navigation | Reserve deliberately; verify bottom-nav interaction at `<1024px`. |
| Legacy panel paths remain and split state | Migrate all consumers atomically; delete old files in Batch 3. |
| Hardcoded copy drifts into components | All copy in `layoutCopy.ts`/`helpCopy.ts`/`notificationCopy.ts`/`sessionCopy.ts`; enforce static review. |
| Drag-and-drop adds bundle size | Use HTML5 DnD API or lightweight solution; avoid heavy dependencies. |
| `useSessionPanel` hook complexity | Keep focused: fetch, group, provide actions. Complex logic in hook, not component. |
| TopRightCluster move breaks imports | Update all import paths; grep for old path after move. |

## Deliberately Out of Scope

- Backend, IPC, persistence, model, or audio changes.
- `PRODUCT.md` creation.
- Global Liquid Space replacement or new palette/typography.
- Tray HUD and setup wizard surfaces.
- Monitoring Help/Notifications controls.
- Memory profiler redesign or removal.
- New telemetry, localization, full-text Help search, or persistent per-user rail-width storage.
- Changes to bottom drawers used by central cards/nodes.
- Project deletion/renaming UI (only create is in scope).
- Session deletion (only pinning and creation are in scope).

## Open Questions

None. All material scope, placement, exclusivity, sizing, visual-direction, and spec-alignment decisions are resolved.

Implementation checklist: `.kilo/plans/1788947884359-edge-panel-contract-checklist.md`.
