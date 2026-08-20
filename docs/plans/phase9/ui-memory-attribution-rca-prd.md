# Vox UI Memory Attribution & RCA --- PRD

**Status:** Proposed\
**Date:** 2026-08-18\
**Scope:** Linux desktop runtime / Tauri + WebKitGTK frontend

## 1. Problem

Vox can show substantial RAM usage even when no inference models are
resident.

We need to answer:

> Where is Vox's UI memory actually going?

The profiler must break memory attribution down as far as technically
possible:

**Vox → WebKit/Tauri surface → Page → Component → Resource category →
Evidence → RCA**

It must distinguish measured data from estimates and must never invent
component-level RAM ownership.

## 2. Goal

Build a developer-facing **UI Memory Profiler** that can:

-   Separate Main WebView and Tray WebView usage.
-   Attribute memory growth to pages.
-   Trace component mount/unmount and instance counts.
-   Track JS heap, DOM, WebGL, images, fonts, CSS/compositing
    indicators, and WebKit/runtime memory where available.
-   Compare baseline, current, peak, delta, and retained memory.
-   Produce evidence-backed RCA findings.

## 3. Non-Goals

-   Inference/model memory profiling.
-   Replacing OS-level profilers.
-   A general React profiler.
-   Claiming exact RAM ownership for components when WebKit cannot
    expose it.
-   Production analytics.

## 4. Current UI Surfaces

### Main application

  --------------------------------------------------------------------------------------------------------
  Page                                Major workload
  ----------------------------------- --------------------------------------------------------------------
  **Home**                            AmbientBackground, PipelineField, AdvancedOrb, dialogue stream,
                                      LiveWaveform, interaction controls

  **History**                         2.5D orbit carousel, CentralClock, VoiceDial, session cards,
                                      DetailPanel, mobile HistoryListView

  **Memory**                          Three.js MemoryGraph, InstancedMesh nodes, LineSegments edges,
                                      search/filter UI, MemoryPipelineDrawer, telemetry

  **Settings**                        Radial settings hub, domain nodes,
                                      Appearance/Interaction/Memory/Models/Persona/Tray/Realtime/History
                                      cards

  **Monitoring**                      Runtime metrics, telemetry charts and controls

  **Wizard**                          Welcome, System Check, Model Setup, Audio Setup, Live Test,
                                      Completed
  --------------------------------------------------------------------------------------------------------

### Separate surface

  -----------------------------------------------------------------------
  Surface                             Major workload
  ----------------------------------- -----------------------------------
  **Tray HUD**                        TrayApp, Header,
                                      TranscriptRenderer, Footer,
                                      streaming transcript, ephemeral
                                      history, audio visualization

  -----------------------------------------------------------------------

The Tray HUD is a separate Tauri WebView and must be treated as an
independent memory surface.

## 5. Attribution Hierarchy

**Application → Window/WebView → Route/Page → Component → Resource
Category → Evidence → RCA**

### WebView level

Track:

-   Main WebView
-   Tray WebView
-   WebView/process identity
-   memory snapshot
-   baseline
-   peak
-   growth

### Page level

Track:

-   route
-   mount/unmount
-   active duration
-   memory/resource deltas
-   retained memory after navigation

### Component level

Components register their lifecycle:

-   component name
-   page
-   mount/unmount timestamps
-   instance count
-   optional resource counters

Component attribution is reported as **correlation** unless direct
measurement exists.

## 6. Resource Categories

### JavaScript

-   JS heap used
-   JS heap total
-   heap growth
-   available detached-object indicators

### DOM

-   node count
-   document/resource counts where available
-   other browser-observable DOM indicators

### WebGL

Especially important for **MemoryGraph** and **AdvancedOrb**.

Track where possible:

-   WebGL context lifecycle
-   geometry creation/disposal
-   buffers
-   textures
-   render targets
-   GPU object counts
-   approximate resource sizes when derivable

### Images

-   loaded images
-   dimensions
-   estimated decoded memory
-   large image count
-   persistent resources

### Fonts

-   loaded font faces/resources
-   duplicate or unexpected loading

### CSS / compositing

Track RCA indicators such as:

-   backdrop-filter
-   large blur surfaces
-   filters
-   animated gradients
-   compositing-heavy elements

These are evidence, not exact byte attribution.

### WebKit / runtime

Use whatever WebKitGTK/Tauri exposes for:

-   web-process memory
-   process identity
-   runtime-level counters

Anything that cannot be attributed remains:

**WebKit / Runtime --- Unattributed**

## 7. Measurement Modes

### Passive

Low-overhead periodic sampling for:

-   memory growth
-   page baselines
-   retention
-   surface comparison

### Diagnostic

Explicit developer mode for:

-   detailed lifecycle tracing
-   WebGL resource tracking
-   route snapshots
-   event timeline
-   deeper counters
-   diagnostic-only forced GC where supported

Diagnostic mode may add overhead.

## 8. Baseline / Delta / Retention

For every surface and page maintain:

-   `baseline`
-   `current`
-   `peak`
-   `delta`
-   `growth_rate`
-   `retained_after_unmount`

Example:

**Home → Memory**

`420 MB → 690 MB (+270 MB)`

After leaving Memory:

`675 MB`

That gives a strong retention signal:

`~255 MB retained`

The system must distinguish:

**allocated during page activity → released → retained**

## 9. RCA Engine

The profiler should turn measurements into evidence-backed findings.

Example:

**MemoryGraph --- suspicious WebGL retention**

Evidence:

-   Memory page mounted.
-   WebGL resource counters increased.
-   Memory page unmounted.
-   WebView memory did not return toward baseline.
-   WebGL resources remained elevated.

**Confidence:** High

Another example:

**Home --- high animation activity, no leak evidence**

Evidence:

-   Orb/AmbientBackground/PipelineField active.
-   High animation activity.
-   Memory remains stable after activity.

**RCA:** expensive active rendering, but no retention evidence.

The system must never claim exact ownership when only correlation is
available.

## 10. Standard Page Experiment

Support a repeatable diagnostic sequence:

**Cold startup → baseline → open page → stabilize → interact → peak →
leave page → stabilize → retained measurement**

Run this for each page and compare results.

This produces a memory profile per page rather than relying on a single
global RSS number.

## 11. Component Instrumentation

Provide lightweight instrumentation such as:

``` tsx
<MemoryTracked name="MemoryGraph">
  <MemoryGraph />
</MemoryTracked>
```

or:

``` ts
useMemoryTrace("MemoryGraph");
```

Record lifecycle and diagnostics, not fabricated RAM ownership.

Optional specialized instrumentation:

-   `useWebGLResourceTrace`
-   `useImageResourceTrace`
-   `useAnimationTrace`

Instrumentation should be disableable outside diagnostic builds.

## 12. Diagnostic UI

Add a developer-facing **UI Memory** surface.

### Overview

Show:

**Total Vox RAM → Main WebView → Tray WebView → Unattributed**

Then:

  Page           Current   Peak   Retained Risk
  ------------ --------- ------ ---------- ------
  Home               ---    ---        --- ---
  History            ---    ---        --- ---
  Memory             ---    ---        --- ---
  Settings           ---    ---        --- ---
  Monitoring         ---    ---        --- ---
  Tray               ---    ---        --- ---

### Page detail

**Page → Components → Resource categories → Timeline → RCA**

Example:

`Memory → MemoryGraph → WebGL → growth/retention → suspicious`

## 13. Timeline

Show diagnostic events horizontally:

**Startup → Home mount → History mount → Memory mount → Memory peak →
Memory unmount → retained memory**

The goal is to make accumulation and retention visually obvious.

## 14. RCA Severity

-   **Normal** --- expected usage
-   **Watch** --- elevated but not clearly problematic
-   **Suspicious** --- abnormal growth or retention
-   **Critical** --- strong evidence of uncontrolled resource
    accumulation

Severity must be evidence-driven.

## 15. Technical Constraints

### Accuracy

Every metric must be labelled as one of:

**Measured / Estimated / Correlated / Unattributed**

Never present an estimate as measured RAM.

### Runtime overhead

Passive profiling must be lightweight enough that it does not materially
alter the behavior being diagnosed.

### Platform

Initial target:

**Linux + Tauri + WebKitGTK**

### Existing architecture

Reuse where possible:

-   `useVoxFootprint`
-   Monitoring
-   Tauri IPC
-   React lifecycle
-   existing performance hooks
-   existing WebGL lifecycle

Do not create a parallel monitoring architecture unnecessarily.

## 16. Success Criteria

For a high-memory session, the profiler should answer:

1.  Which WebView is consuming the memory?
2.  Which page caused the growth?
3.  Which major component was active?
4.  Is the growth JS, DOM, WebGL, image/resource, or WebKit/runtime?
5.  Does memory return after leaving the page?
6.  What remains retained?
7.  What evidence supports the RCA?

A useful final finding should look like:

> **Memory page increased Main WebView footprint by \~X MB. The
> strongest correlated growth was MemoryGraph WebGL resources. After
> unmount, \~Y MB remained retained. This is a suspicious WebGL resource
> lifecycle issue.**

## 17. MVP Phases

**Phase 1 --- Attribution foundation:** Main/Tray WebView → route →
snapshots → baseline/delta/retention

**Phase 2 --- Component tracing:** mount/unmount → instance counts →
lifecycle timeline

**Phase 3 --- Resource attribution:** JS heap + DOM + WebGL + images +
fonts + CSS/compositing indicators

**Phase 4 --- RCA:** evidence correlation → severity → probable root
cause

**Phase 5 --- Diagnostic UI:** overview → page → component → resource →
timeline → RCA

## 18. Core Principle

The profiler should not try to manufacture an answer such as:

> "This React component owns exactly 137 MB."

It should answer the engineering question that is actually useful:

> **What changed, where did it change, what remained after cleanup, and
> what evidence points to the cause?**

WebKit process memory may not be perfectly attributable to React
components. The system must preserve that uncertainty instead of hiding
it.
