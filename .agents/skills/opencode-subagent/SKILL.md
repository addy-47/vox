---
name: opencode-subagent
description: Launch and manage persistent, role-based subagents via the `opencode` CLI (headless `opencode run`) for isolated or non-backend tasks (QA, testing, review, frontend, ML research, architecture). Use when a task needs a distinct persona, scoped execution, or persistent multi-turn state spawned by the main agent as a separate OS process. Always uses OpenCode Zen free models with `--variant low`.
---

# `opencode-subagent` — CLI Subagent Orchestration (OpenCode)

## 0. Why This Skill Exists

OpenCode has two distinct subagent mechanisms:

1. **Native runtime subagents** (`@`-mention / built-in `General`, `Explore`, `Scout`, or configured agents in `opencode.json`) — invoked inside the TUI/IDE. Async, permission-gated by config.
2. **CLI subagents via `opencode run`** — standalone headless OS processes, output returned to the caller, state persisted as a session on disk. This is the mechanism for the *main agent* to spawn a subagent as a separate process from the shell (CI, scripts, or when you want the result captured inline rather than a multi-turn TUI exchange).

This skill governs case 2. If you're already inside a TUI where native `@`-mention subagents are convenient, prefer those. This skill is the headless CLI workaround for programmatic orchestration.

**Hard invariant:** every spawn in this skill uses an **OpenCode Zen *free* model** and **`--variant low`**. No paid Zen models, no high/max reasoning effort. Free models rotate ("available for a limited time") — never hardcode a single model; pull from the roster in §4 and fall back when one is unavailable.

---

## 1. Role Roster

Roles map 1:1 to files in `.agents/rules/` (relative to the current working directory — this is correct for the Vox workspace; adjust the path if used elsewhere). This list reflects the current roster — if a role file is added or removed, update this table, don't let it drift.

| Role | Rule File | Default Scope | Primary Skills (reach for mid-task) |
|---|---|---|---|
| System Architect | `system-architect.md` | Read-only strategy / gates | `architect`, `validate` |
| Backend Engineer | `backend-engineer.md` | Write-capable | `rca`, `review`, `grill-me` |
| Frontend Engineer | `frontend-engineer.md` | Write-capable | `impeccable`, `intent-alignment`, `review` |
| ML Research Engineer | `ml-research-engineer.md` | Write-capable, limited to data/eval artifacts — not production backend code | `feedback-review`, `create-dataset`, `create-eval` |
| Test Engineer | `test-engineer.md` | Write-capable — writes tests, runs harnesses | `test-plan`, `review`, `rca` |
| QA Engineer | `qa-engineer.md` | Read-only / sandboxed — never writes or executes tests | `review`, `rca` |

**Read-only caveat (real limitation, not a solved constraint):** `opencode run` has no scoped read-only execution flag. The only approval escape is `--auto` (auto-approve everything — dangerous for write tasks). Until a scoped `--permissions` flag lands on `run`, "read-only" roles (System Architect, QA) are enforced by their own role file's invariants + by you instructing the subagent to only use `read`/`glob`/`grep`/`webfetch` — not by OS-level sandboxing. Treat this as a real limitation; don't assume writes are actually blocked just because the table says "read-only".

---

## 2. Prompt Assembly — Template, Not Hardcoded Tasks

Every spawn is assembled from this template, never copied verbatim from an example. `{ROLE_NAME}`, `{ROLE_FILE}`, `{ROLE_SKILLS}` come from the table in §1. `{TASK_DESCRIPTION}` is written fresh, specific to the actual task — never reused.

**First turn (new subagent):**

```bash
opencode run \
  "You are the {ROLE_NAME} for this project. Read .agents/rules/{ROLE_FILE} first — that file defines your identity, invariants, and role boundaries. Follow it exactly for this and every future turn.

You are running in the same working directory as the main agent, so AGENTS.md and the full codebase are already available to you directly — use them, do not wait for anything to be pasted in.

Skills available to you if you hit genuine uncertainty mid-task: {ROLE_SKILLS}.

Your task for this session:
{TASK_DESCRIPTION}" \
  -m opencode/{FREE_MODEL} \
  --variant low \
  --format json \
  --auto
```

**Subsequent turns (same role, same thread, continue the session):**

```bash
opencode run \
  "{FOLLOW_UP_TASK_DESCRIPTION}" \
  -s {STORED_SESSION_ID} \
  -m opencode/{FREE_MODEL} \
  --variant low \
  --format json \
  --auto
```

Flags:
- `-m opencode/{FREE_MODEL}` — the Zen free model id (see §4). Provider is always `opencode`.
- `--variant low` — reasoning effort forced low. **Always present.** See §4 note on models that reject variants.
- `--format json` — required so the `sessionID` can be captured from the first event (§4 lifecycle).
- `--auto` — auto-approves tool permissions so the headless subagent can actually call `bash`/`edit`/`read`. Omit only for strictly read-only roles you have instructed to avoid writes.
- `-s {STORED_SESSION_ID}` — continue a prior session for multi-turn persistence.

---

## 3. Model Parameter — Zen Free Models, Variant Low

`{FREE_MODEL}` is a placeholder. OpenCode Zen free models (provider `opencode/<id>`), current as of the Zen catalog:

| Model (human) | CLI id (`opencode/<id>`) | Notes |
|---|---|---|
| Hy3 Free | `hy3-free` | Default. General-purpose, fast. |
| DeepSeek V4 Flash Free | `deepseek-v4-flash-free` | Strong coding; limited-time. |
| MiMo-V2.5 Free | `mimo-v2.5-free` | Limited-time. |
| Laguna S 2.1 Free | `laguna-s-2.1-free` | Limited-time. |
| Nemotron 3 Ultra Free | `nemotron-3-ultra-free` | NVIDIA endpoint — trial use, do not send confidential data. |
| Nemotron 3.5 Lightning Free | `nemotron-3.5-lightning-free` | NVIDIA endpoint — trial use, do not send confidential data. |
| Big Pickle | `big-pickle` | Stealth free model; data may be used to improve it during free period. |

**Selection rules:**
1. **Default to `hy3-free`** unless you have a reason to prefer another.
2. **If a model is unavailable** (spawn returns an auth/404/"model not found" / provider error), pick the next from the table and retry. The free roster changes without notice — never assume a specific id is still live; the error is the signal.
3. **Privacy:** For any task touching personal/confidential data, avoid the two `nemotron-*-free` (NVIDIA trial) and `big-pickle` (data-improvement) models. Free models are served via `https://opencode.ai/zen/v1/chat/completions` and are zero-retention except where noted above.

**`--variant low` on models that reject it:** Some free chat-completion models don't accept a reasoning-effort variant. If a spawn fails specifically because of `--variant`, retry the *same* command **without** `--variant low`. Treat variant rejection as a model-capability issue, not a reason to switch models.

To confirm the live free roster at any time: `opencode models opencode` (or hit `https://opencode.ai/zen/v1/models`).

---

## 4. Lifecycle — Init vs. Reuse

Subagents are persistent across the full conversation thread via OpenCode session IDs. Never spawn a duplicate subagent for a role that already has an active session ID in this thread.

### Step 1 — Check for an existing session
Before spawning, check whether this thread already has a stored `{role: session_id}` mapping. If yes, skip to Step 3 (reuse). If no, continue.

### Step 2 — Spawn and capture the ID
`opencode run --format json` prints one JSON event per line. The `sessionID` field (format `ses_...`) is present on the **first** event. Capture it:

```bash
OUT=$(opencode run "..." -m opencode/hy3-free --variant low --format json --auto)
SID=$(echo "$OUT" | head -n 1 | grep -o '"sessionID":"[^"]*"' | head -n 1 | sed 's/"sessionID":"//;s/"//')
echo "Captured session: $SID"
```

Sanity-check `$SID` looks like `ses_...` before storing. If the run errored (no sessionID in output), the model may be unavailable — go to §3 rule 2 and retry with the next free model.

### Step 3 — Store and reuse
Keep `{role: session_id}` for the duration of the thread. All subsequent tasks for that role reuse the stored ID via `-s`. To survive across sessions (not just this thread), persist the mapping somewhere durable — but don't invent a new canonical path without confirming with the user first.

---

## 5. Scaling Large Audits (Primarily QA)

When a subagent's task is too large to process directly (e.g. QA auditing 100 reports at 200 lines each), spawning a *further* subagent isn't the first move — judgment about the actual constraint is. The role doing this should be able to state afterward what it read directly, what was summarized, what was delegated, and why that adds up to real coverage rather than a shortcut. This skill is the delegation mechanism when delegation is actually the right call — not a substitute for that judgment.

Headless `opencode run` returns the full transcript as its stdout, so a subagent's complete result lands inline in the caller's context — no manual polling required.

---

## 6. Summary Checklist

1. Identify the role needed. Confirm it exists in the table in §1 — don't invent a role with no rule file.
2. Check the thread for an existing `{role: session_id}` mapping.
3. **Existing:** reuse via `opencode run "{task}" -s {STORED_ID} -m opencode/{FREE_MODEL} --variant low --format json --auto`.
4. **New:** assemble the full template in §2 — role file path, codebase-access note, that role's real skills list, a task description written for this specific task. Spawn with `--format json`. Immediately capture `sessionID` from the first JSON line per §4 before doing anything else. Sanity-check it.
5. Model is always a **Zen free** model (`opencode/<id>` from §3); default `hy3-free`. If unavailable, fall back down the table. Avoid NVIDIA/`big-pickle` models for confidential data.
6. **`--variant low` is always present** unless the model rejects variants (then retry without it).
7. Apply the role's default scope from §1. Remember the read-only distinction is currently prompt-enforced, not OS-enforced — don't treat it as a hard guarantee.
