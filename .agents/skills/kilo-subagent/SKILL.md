---
name: kilo-subagent
description: Launch and manage persistent, role-based subagents via the `kilo run` CLI (headless Kilo) for isolated or non-backend tasks (QA, testing, review, frontend, ML research, architecture). Use when the main agent (e.g., running in Antigravity IDE via `agy`) needs to spawn a Kilo subagent as a separate OS process with the full Kilo toolchain and project context.
---

# `kilo-subagent` — CLI Subagent Orchestration (Kilo)

## 0. Why This Skill Exists

Kilo has two distinct subagent mechanisms:

1. **Native runtime subagents** (`Task` tool / configured agents) — invoked inside the Kilo TUI. Async, permission-gated by config.
2. **CLI subagents via `kilo run`** — standalone headless OS processes, output returned to the caller, state persisted as a session on disk. This is the mechanism for the *main agent* (e.g. Antigravity/CLI caller) to spawn a Kilo subagent as a separate process.

This skill governs case 2. If you're already inside a Kilo TUI where native `Task`-tool subagents are convenient, prefer those. This skill is the headless CLI path for programmatic orchestration — e.g. when your main agent is Antigravity (`agy`) or OpenCode and you want to delegate a slice of work to a full Kilo instance.

**Hard invariant:** every spawn in this skill uses a **Kilo free model** and **`--variant minimal`**. No paid/high-reasoning models. Default model is `kilo/kilo-auto/free`.

---

## 1. Role Roster

Roles map 1:1 to files in `.agents/rules/` (relative to the current working directory — this is correct for the Vox workspace; adjust the path if used elsewhere). This list reflects the current roster — if a role file is added or removed, update this table, don't let it drift.

| Role | Rule File | Default Scope | Primary Skills (reach for mid-task) |
|---|---|---|---|
| System Architect | `system-architect.md` | Read-only strategy / gates | `architect`, `validate` |
| Backend Engineer | `backend-engineer.md` | Write-capable | `rca`, `review`, `refactor-arch`, `refactor-clean` |
| Frontend Engineer | `frontend-engineer.md` | Write-capable | `impeccable`, `intent-alignment`, `review` |
| ML Research Engineer | `ml-research-engineer.md` | Write-capable, limited to data/eval artifacts — not production backend code | `feedback-review`, `create-dataset`, `create-plan`, `create-loop` |
| Test Engineer | `test-engineer.md` | Write-capable — writes tests, runs harnesses | `test-plan`, `review`, `rca` |
| QA Engineer | `qa-engineer.md` | Read-only / sandboxed — never writes or executes tests | `review`, `rca` |

**Read-only caveat (real limitation, not a solved constraint):** `kilo run` with `--auto` auto-approves all non-explicitly-denied permissions. There is no scoped read-only flag. Until one lands, "read-only" roles (System Architect, QA) are enforced by their own role file's invariants + by you instructing the subagent to only use `read`/`glob`/`grep`/`webfetch` — not by OS-level sandboxing. Treat this as a real limitation; don't assume writes are actually blocked just because the table says "read-only".

---

## 2. Prompt Assembly — Template, Not Hardcoded Tasks

Every spawn is assembled from this template, never copied verbatim from an example. `{ROLE_NAME}`, `{ROLE_FILE}`, `{ROLE_SKILLS}` come from the table in §1. `{TASK_DESCRIPTION}` is written fresh, specific to the actual task — never reused.

**First turn (new subagent):**

```bash
kilo run \
  "You are the {ROLE_NAME} for this project. Read .agents/rules/{ROLE_FILE} first — that file defines your identity, invariants, and role boundaries. Follow it exactly for this and every future turn.

You are running in the same working directory as the main agent, so AGENTS.md and the full codebase are already available to you directly — use them, do not wait for anything to be pasted in.

Skills available to you if you hit genuine uncertainty mid-task: {ROLE_SKILLS}.

Your task for this session:
{TASK_DESCRIPTION}" \
  -m kilo/kilo-auto/free \
  --variant minimal \
  --format json \
  --auto \
  --dir /home/addy/projects/apps/vox
```

**Subsequent turns (same role, same thread, continue the session):**

```bash
kilo run \
  "{FOLLOW_UP_TASK_DESCRIPTION}" \
  -s {STORED_SESSION_ID} \
  -m kilo/kilo-auto/free \
  --variant minimal \
  --format json \
  --auto
```

Flags:
- `-m kilo/kilo-auto/free` — the Kilo free model id. The default is the lightweight Kilo auto model running in free tier. See §3 for the variant roster and fallback rules.
- `--variant minimal` — reasoning effort forced minimal. **Always present.** See §3 note on models that reject variants.
- `--format json` — required so the `sessionID` can be captured from the first event (§4 lifecycle).
- `--auto` — auto-approves tool permissions so the headless subagent can actually call `bash`/`edit`/`read`. Omit only for strictly read-only roles you have instructed to avoid writes.
- `--dir` — the working directory to run in. Use the project root by default.
- `-s {STORED_SESSION_ID}` — continue a prior session for multi-turn persistence.

---

## 3. Model Parameter — Kilo Free Models, Variant Minimal

`-m` is a placeholder. The Kilo free-model roster (provider `kilo/`), current as of the Kilo catalog:

| Model (human) | CLI id | Notes |
|---|---|---|
| Kilo Auto Free | `kilo/kilo-auto/free` | Default. The model you are running as right now — a capable free-tier reasoning model. |
| Kilo Auto Small | `kilo/kilo-auto/small` | Smaller, faster. |
| Cohere North Mini Code | `kilo/cohere/north-mini-code:free` | Free. |
| Dots Studio Dots 3 | `kilo/dots-studio/dots-3-note-preview:free` | Free. |
| Liquid LFM 2.5 | `kilo/liquid/lfm-2.5-2.6b:free` | Free; very small. |
| StepFun Step 3.7 Flash | `kilo/stepfun/step-3.7-flash:free` | Free. |
| Tencent Hy3 | `kilo/tencent/hy3:free` | Free. |
| NVIDIA Nemotron (various) | `kilo/nvidia/nemotron-3-*-*:free` | Free via NVIDIA endpoint. |

Full list: `kilo models` (pipe through `grep ":free"` or `grep "kilo/kilo-auto"` to filter).

**Selection rules:**
1. **Default to `kilo/kilo-auto/free`** unless you have a reason to prefer another. This is the model you are running as, so it is always available and consistent.
2. **If a model is unavailable** (spawn returns an auth/404/"model not found" / provider error), pick the next from the table and retry. The free roster changes without notice — never assume a specific id is still live; the error is the signal.
3. **Privacy:** For any task touching personal/confidential data, prefer `kilo/kilo-auto/free` (served via Kilo's zero-retention inference path) over the NVIDIA-endpoint models.

**`--variant minimal` on models that reject it:** Some free chat-completion models don't accept a reasoning-effort variant. If a spawn fails specifically because of `--variant`, retry the *same* command **without** `--variant minimal`. Treat variant rejection as a model-capability issue, not a reason to switch models.

---

## 4. Lifecycle — Init vs. Reuse

Subagents are persistent across the full conversation thread via Kilo session IDs. Never spawn a duplicate subagent for a role that already has an active session ID in this thread.

### Step 1 — Check for an existing session
Before spawning, check whether this thread already has a stored `{role: session_id}` mapping. If yes, skip to Step 3 (reuse). If no, continue.

### Step 2 — Spawn and capture the ID
`kilo run --format json` prints one JSON event per line. The `sessionID` field (format `ses_...`) is present on the **first** event (`step_start`). Capture it:

```bash
OUT=$(kilo run "..." -m kilo/kilo-auto/free --variant minimal --format json --auto --dir /home/addy/projects/apps/vox)
SID=$(echo "$OUT" | head -n 1 | grep -o '"sessionID":"[^"]*"' | head -n 1 | sed 's/"sessionID":"//;s/"//')
echo "Captured session: $SID"
```

Sanity-check `$SID` looks like `ses_...` before storing. If the run errored (no sessionID in output), the model may be unavailable — go to §3 rule 2 and retry with the next free model.

### Step 3 — Store and reuse
Keep `{role: session_id}` for the duration of the thread. All subsequent tasks for that role reuse the stored ID via `-s`. To list known sessions at any time:

```bash
kilo session list
```

To delete a stale session:

```bash
kilo session delete {SESSION_ID}
```

If you need the mapping to survive across sessions (not just this thread), persist it somewhere durable — but don't invent a new canonical path without confirming with the user first.

---

## 5. Scaling Large Audits (Primarily QA)

When a subagent's task is too large to process directly (e.g. QA auditing 100 reports at 200 lines each), spawning a *further* subagent isn't the first move — judgment about the actual constraint is. The role doing this should be able to state afterward what it read directly, what was summarized, what was delegated, and why that adds up to real coverage rather than a shortcut. This skill is the delegation mechanism when delegation is actually the right call — not a substitute for that judgment.

Headless `kilo run --format json` returns the full transcript as its stdout, so a subagent's complete result lands inline in the caller's context — no manual polling required.

---

## 6. Summary Checklist

1. Identify the role needed. Confirm it exists in the table in §1 — don't invent a role with no rule file.
2. Check the thread for an existing session ID for that role.
3. **Existing:** reuse via `kilo run "{task}" -s {STORED_ID} -m kilo/kilo-auto/free --variant minimal --format json --auto --dir /path/to/project`.
4. **New:** assemble the full template in §2 — role file path, codebase-access note, that role's real skills list, a task description written for this specific task, and `--dir` pointing at the project root. Spawn with `--format json`. Immediately capture `sessionID` from the first JSON line per §4 before doing anything else. Sanity-check it.
5. Model is always a **Kilo free** model (`kilo/kilo-auto/free` by default from §3); if unavailable, fall back down the table.
6. **`--variant minimal` is always present** unless the model rejects variants (then retry without it).
7. Apply the role's default scope from §1. Remember the read-only distinction is currently prompt-enforced, not OS-enforced — don't treat it as a hard guarantee.
