---
name: architect
description: Lead a rigorous, Socratic system architecture session for Vox before authoring or updating specs in docs/specs/. Trigger on "design a system for", "how should we architect", "let's design", "re-architect", "system architecture for X", or before writing specs for multi-component features.
---

# Vox Architectural Design Harness

This skill governs interactive architectural design and refactoring sessions. It is **not** a generic boilerplate generator. Its purpose is to force first-principles systems thinking, expose hidden state/lifecycle couplings, and lock down exact boundary contracts before touching `docs/specs/`.

Always load and enforce `.agents/rules/architecture-invariants.md` and `.agents/rules/system-architect.md`.

---

## Step 1 — Deep Ground-Truth Recon (Mandatory First Step)

Before proposing a design or asking the user questions, inspect the actual ground truth:
1. Read the relevant spec(s) in `docs/specs/` (including adjacent specs like `ipc-spec.md`, `events-spec.md`, `db-spec.md`, `ownership-spec.md`).
2. If refactoring or fixing an existing feature, read the full implementation path across backend (`app/src-tauri/src/`) and frontend (`app/src/store/`, `app/src/services/`, `app/src/shared/`).
3. **Challenge, Don't Comply:** Never assume existing specs or code are sacred. Explicitly build a **Spec vs. Code vs. Reality Audit**:
   - Where does the current code violate the spec?
   - Where is the current spec mathematically incomplete, ambiguous, or over-coupled?

---

## Step 2 — Identify the Governing Architectural Questions (Socratic Frontier)

Do **not** ask generic filler questions ("Is this a POC or MVP? How many millions of users?"). Vox is a real-time native voice runtime and thin-client ecosystem.

Instead, identify the **governing architectural pivot points** and present any genuine design forks to the user concisely (with your recommended verdict and why):
- **Lifecycle Boundaries:** What are the distinct operational lifecycles here, and how do we guarantee they never shadow or auto-trigger each other?
- **Ownership & Authority:** Which peer/subsystem owns the single source of truth? Which surface is active vs. passive?
- **Hardware & Real-Time Budget:** How does this behave across Tier 1A (8GB CPU) through Tier 3 (Realtime S2S)? Does anything threaten the <500ms/800ms audio/voice latency budget or the 5.5GB RAM ceiling?

*(If the user has already stated their intent clearly, do not stall with unnecessary questions—validate their premise against the codebase and proceed directly to Step 3.)*

---

## Step 3 — Construct the State Machine & Payload Contract (No Ambiguous States)

Never propose a high-level design that hides the state enums or event payloads. Define the exact state space up front:

1. **Lifecycle State Enums:**
   - Write out the exact canonical enum(s) (e.g., `PairingState`, `RemoteLinkState`, `InteractionState`).
   - **Zero-State Audit:** Verify that "Disabled", "Unconfigured/Unpaired", and "Configured/Paired (Idle)" are separate variants—never lumped into a catch-all `Disconnected` or `Idle`.
   - **Role Occupancy Table:** If multiple peers exist (e.g., Host vs. Controller), show a table of which states each role is permitted to occupy.
2. **Canonical Event Payload (Single Projection Check):**
   - Write the exact DTO struct/interface emitted to the UI (e.g., `RemoteStateDto`).
   - Prove that a UI component can render every state (Unpaired, Paired Idle, Connecting, Connected, Reconnecting, Failed with reason) from **this single event payload alone**, with zero side-channel database queries or secondary events.
3. **Discrete Command Verbs:**
   - List every IPC command that triggers a transition. Verify that no single command (like `connect_host`) is overloaded across two different lifecycles.

---

## Step 4 — Trace the 4 Boundaries & Failure Matrix

For every core workflow (Setup/Pairing, Connect, Active Execution, Intentional Disconnect, Permanent Revoke/Teardown), provide a concrete trace across all 4 boundaries:

[Initiating UI] --(1. IPC Cmd + Payload)--> [Local Backend]
[Local Backend] --(2. State Transition + Local IPC Event)--> [Initiating UI]
[Local Backend] --(3. Wire Frame / Channel Msg)--> [Peer Backend Actor Loop]
[Peer Backend]  --(4. State Transition + Local IPC Event)--> [Peer UI]


Then explicitly stress-test the **Failure & Edge-Case Matrix**:
1. **Unannounced Cutoff (Fast & Slow Path):** Immediate socket EOF/RST vs. silent packet drop (Ping/Pong heartbeat timeout). Confirm both sides have an active read loop!
2. **Offline / Split-Brain Mutation:** Peer A revokes/mutates while Peer B is offline or asleep. Trace what happens when Peer B wakes up, hydrates (`get_link_state`), or attempts a handshake (`Hello` $\to$ `UnknownPeer`).
3. **Mid-Flow Abandonment & Rejection:** User closes modal mid-flow, token expires, or peer clicks "Deny". Verify that failure in Setup/Tier 1 never corrupts Transport/Tier 2 state.
4. **Concurrency & Admission Contention:** Simultaneous requests, busy host, or mid-turn barge-in.

---

## Step 5 — Iterate Surgical Refinements

Present the design clearly and wait for the user's pushback.
- When the user challenges a seam, inspect the exact failure mode they pointed out—do not defend a broken design or slap a band-aid on top of it.
- Update only the affected sections without regenerating unrelated boilerplate.

---

## Step 6 — Spec Formalization & Referential Integrity Sweep

Once the user approves the architecture:
1. Apply the changes to the authoritative spec(s) in `docs/specs/` in surgical batches.
2. **Mandatory Cross-Section Sweep:** Before declaring the spec update complete, search the entire spec (and sibling specs like `ipc-spec.md`, `events-spec.md`, `ownership-spec.md`) for any legacy enum variants, old boolean flags, or superseded event names. Update every diagram, table, vocabulary entry, decision log row, and `VERIFY-*` criterion so the specification has 100% internal consistency.