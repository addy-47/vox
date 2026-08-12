---
name: agy-subagent
description: Launch and manage persistent, role-based subagents via the agy CLI for isolated or non-backend tasks (QA, testing, review, frontend, ML research, architecture) inside the Antigravity IDE, which has no native subagent capability. Use when a task needs a distinct persona, scoped execution, or persistent multi-turn state separate from the main agent's own context.
---

# `agy-subagent` — CLI Subagent Orchestration

## 0. Why This Skill Exists

Antigravity has two distinct subagent mechanisms:

1. **Native runtime subagents** (`invoke_subagent` / `send_message`) — available in the Antigravity Agent Manager. Isolated tool permissions, async communication, no manual polling.
2. **CLI subagents via `agy`** — standalone OS processes, state persisted to disk. This is the *only* subagent mechanism available inside the **Antigravity IDE**, which has no native subagent support.

This skill governs case 2. If you're operating in a context where native subagents are available, use those instead — this skill is specifically the IDE workaround.

---

## 1. Role Roster

Roles map 1:1 to files in `.agents/rules/`. This list reflects the current roster — if a role file is added or removed, update this table, don't let it drift.

| Role | Rule File | Default Scope | Primary Skills (reach for mid-task) |
|---|---|---|---|
| Backend Engineer | `backend-engineer.md` | Write-capable | `rca`, `review`, `grill-me` |
| Frontend Engineer | `frontend-engineer.md` | Write-capable | `impeccable`, `intent-alignment`, `grill-me`, `review` |
| ML Research Engineer | `ml-research-engineer.md` | Write-capable, limited to data/eval artifacts — not production backend code | `feedback-review`, `create-dataset`, `create-eval`, `create-plan`, `create-loop` |
| Test Engineer | `test-engineer.md` | Write-capable — writes tests, runs harnesses | `test-plan`, `review`, `rca` |
| QA Engineer | `qa-engineer.md` | Read-only / sandboxed — never writes or executes tests | `review`, `rca`, `agy-subagent` (for batch-scaling large audits — see §5) |

**Open item, not yet solved:** neither this skill nor `agy` itself currently confirms a granular read-only execution flag — only the binary `--dangerously-skip-permissions` exists. Until a scoped flag is confirmed, "read-only" roles (System Architect, QA) are enforced by their own role file's invariants, not by OS-level sandboxing. Treat this as a real limitation, not a solved constraint — don't assume write access is actually blocked for these roles just because the table says "read-only."

---

## 2. Prompt Assembly — Template, Not Hardcoded Tasks

The previous version of this skill hardcoded a fixed task per role (e.g. "perform a full test audit"). That's wrong — the same role gets different tasks constantly. Every spawn is assembled from this template, never copied verbatim from an example:

**First turn (new subagent):**

```
agy -p "You are the {ROLE_NAME} for Vox. Read .agents/rules/{ROLE_FILE} first — that file defines your identity, invariants, and role boundaries. Follow it exactly for this and every future turn in this conversation.

You're running in the same working directory as the main agent, so AGENTS.md and the full codebase are already available to you directly — use them, don't wait for anything to be pasted in.

Skills available to you if you hit genuine uncertainty mid-task: {ROLE_SKILLS}.

Your task for this session:
{TASK_DESCRIPTION}" \
  --model {CURRENT_MODEL} \
  {PERMISSION_FLAGS}
```

**Subsequent turns (existing subagent, same role, same thread):**

```
agy --conversation {STORED_CONVERSATION_ID} \
  -p "{FOLLOW_UP_TASK_DESCRIPTION}" \
  --model {CURRENT_MODEL} \
  {PERMISSION_FLAGS}
```

`{ROLE_NAME}`, `{ROLE_FILE}`, and `{ROLE_SKILLS}` come from the table in §1. `{TASK_DESCRIPTION}` is written fresh, specific to the actual task at hand — never reused from a previous invocation, never a generic restatement of the role's job description.

---

## 3. Model Parameter

`{CURRENT_MODEL}` is a placeholder, not a literal. Before the first spawn in a session, confirm the model string you're about to pass is still current — do not carry forward a hardcoded string from a previous session or from this file. Model identifiers change; a skill that hardcodes one goes stale silently, exactly like the paths this project already moved away from. DEFAULT : gemini-3.6-flash-high

---

## 4. Lifecycle — Init vs. Reuse

Subagents are persistent across the full conversation thread. Never spawn a duplicate subagent for a role that already has an active conversation ID in this thread.

### Step 1 — Check for an existing conversation
Before spawning, check whether this thread already has a stored `{role: conversation_id}` mapping for the role you need. If yes, skip to Step 3 (reuse). If no, continue.

### Step 2 — Spawn and capture the ID
`agy` does not print its conversation ID to stdout on spawn — this was confirmed directly against the CLI, not assumed. The ID must be recovered from the filesystem:

```bash
ls -t ~/.gemini/antigravity-cli/conversations/*.db | head -n 1 | xargs -I {} basename {} .db
```

This is a temporal-ordering lookup — it has a real race condition if multiple new subagents are spawned close together. The only available mitigation right now (no `--print-conversation-id` flag exists yet) is discipline:

- **Never spawn more than one new subagent in the same turn.** Spawn, capture, confirm, then spawn the next.
- After capture, sanity-check the result looks like a plausible UUID before storing it. If it doesn't, or if you have reason to think another process might have written to that directory concurrently, stop and confirm rather than silently trusting it.

### Step 3 — Store and reuse
Keep the `{role: conversation_id}` mapping for the duration of the thread. All subsequent tasks for that role reuse the stored ID via `--conversation`. If you need the mapping to survive across sessions (not just this thread), persist it somewhere durable — but don't invent a new canonical path for that without confirming it with the user first.

---

## 5. Scaling Large Audits (Primarily QA)

When a subagent's task is too large to process directly — e.g. QA auditing 100 reports at 200 lines each — spawning a *further* subagent isn't the first move, judgment about the actual constraint is. The role doing this should be able to state afterward what it read directly, what was summarized, what was delegated, and why that adds up to real coverage rather than a shortcut. This skill is the mechanism for the delegation piece when delegation is actually the right call — it is not a substitute for that judgment.

---

## 6. Summary Checklist

1. Identify the role needed. Confirm it exists in the table in §1 — don't invent a role that has no rule file.
2. Check the thread for an existing conversation ID for that role.
3. **Existing:** reuse via `--conversation {STORED_ID}`, task description only.
4. **New:** assemble the full template in §2 — role file path, actual codebase access note, that role's real skills list, and a task description written for this specific task. Spawn. Immediately capture the conversation ID per §4 before doing anything else. Sanity-check it.
5. Confirm `{CURRENT_MODEL}` is actually current before using it — don't carry forward a stale literal.
6. Apply the role's default scope from §1. Remember the read-only distinction is currently prompt-enforced, not OS-enforced — don't treat it as a hard guarantee.