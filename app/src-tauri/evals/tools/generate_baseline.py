#!/usr/bin/env python3
"""Generate the Gemma reference ceiling for every planned compaction slice.

Reads slice files written by the harness (`--dump-slices`), replays each through
gemma4:12b using the production compaction prompt with thinking enabled, chains
each slice's output into the next slice's <prior_summary>, and writes one
`baseline/slice_NN.json` per slice.

This script never decides a slice boundary and never tokenizes. It consumes the
boundaries the Rust harness planned. The system prompt is read from the Rust
source at runtime so the two cannot drift textually: if the prompt changes, the
script fails loudly instead of generating against a stale copy.

Usage:
    python3 evals/tools/generate_baseline.py --run-dir <run_dir> [--model gemma4:12b]
"""

import argparse
import json
import os
import re
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import requests

REPO = Path(__file__).resolve().parents[2]  # .../app/src-tauri
PROMPT_RS = REPO / "src" / "services" / "memory" / "compaction" / "prompt.rs"

CATEGORIES = ["personal", "objective", "workdone", "blocker", "next_step", "pitfall"]

# A golden baseline is only useful as a ruler if regenerating it gives the same
# document. Sampling at 0.2 produced two different case_01 baselines from one
# seed (18 facts vs 45 facts), so the ceiling moved by 2.5x between identical
# runs. Greedy decoding is the only setting that makes the ruler reproducible.
TEMPERATURE = 0.0

# Verified at startup against the Rust source. If build_compaction_request changes
# its task text, the assertion below fails and this script refuses to run.
EXPECTED_TASK_SENTINEL = "using ONLY the <user_turns> block"

COMPACTION_SCHEMA = {
    "type": "object",
    "properties": {
        name: {"type": "array", "items": {"type": "string"}} for name in CATEGORIES
    },
    "required": CATEGORIES,
    "additionalProperties": False,
}


def read_system_prompt() -> str:
    """Extract COMPACTION_SYSTEM_PROMPT verbatim from the Rust source."""
    src = PROMPT_RS.read_text()
    marker = "COMPACTION_SYSTEM_PROMPT: &str = r#\""
    start = src.find(marker)
    if start < 0:
        raise SystemExit(f"cannot locate COMPACTION_SYSTEM_PROMPT in {PROMPT_RS}")
    body_start = start + len(marker)
    # The raw string ends at the first "#; sequence at a line boundary context.
    # The prompt body never contains the two characters "# followed by ;.
    end = src.find('"#;', body_start)
    if end < 0:
        raise SystemExit("cannot find the end of COMPACTION_SYSTEM_PROMPT")
    prompt = src[body_start:end]
    for required in ("<role>", "<precision_rules>"):
        if required not in prompt:
            raise SystemExit(f"extracted prompt looks truncated (missing {required})")
    if EXPECTED_TASK_SENTINEL not in src:
        raise SystemExit(
            "build_compaction_request task text changed in Rust; update this script"
        )
    return prompt


def format_session_context(doc: dict) -> str:
    """Mirror UnifiedCompactionPayload::format_session_context (utils/json.rs).

    The baseline chains its own prior summaries, so this rendering must match
    what production feeds the next slice. Any divergence here would make the
    baseline's slice N+1 see different context than the runtime's slice N+1.
    """
    out = ""
    sections = [
        ("personal", "Personal:"),
        ("objective", "Objectives:"),
        ("workdone", "Completed Work:"),
        ("blocker", "Blockers:"),
        ("next_step", "Next Steps:"),
        ("pitfall", "Pitfalls & Constraints:"),
    ]
    for key, heading in sections:
        items = doc.get(key) or []
        if not items:
            continue
        if out:
            out += "\n"
        out += heading + "\n"
        for item in items:
            out += f"- {item}\n"
    return out


def build_user_content(slice_messages: list, prior_summary: str | None) -> str:
    """Mirror build_compaction_request (prompt.rs): <prior_summary> + speaker-separated blocks + <task>.

    User and assistant turns render in separate blocks with per-side numbering.
    This function must stay byte-identical in structure to the Rust builder;
    the startup assertion on EXPECTED_TASK_SENTINEL guards the task text.
    """
    prior_block = ""
    if prior_summary and prior_summary.strip():
        prior_block = f"<prior_summary>\n{prior_summary.strip()}\n</prior_summary>\n\n"
    user_lines: list[str] = []
    assistant_lines: list[str] = []
    turn_no = 0
    for m in slice_messages:
        if m["role"] == "user":
            turn_no += 1
            user_lines.append(f'<turn n="{turn_no}">{m["content"].strip()}</turn>')
        elif m["role"] == "assistant":
            assistant_lines.append(f'<turn n="{turn_no}">{m["content"].strip()}</turn>')
    return (
        f"{prior_block}"
        f"<user_turns>\n" + "\n".join(user_lines) + "\n</user_turns>\n\n"
        f"<assistant_turns>\n" + "\n".join(assistant_lines) + "\n</assistant_turns>\n\n"
        "<task>\n"
        "Analyze the turns above in light of <prior_summary> if present.\n"
        "Extract the user's profile facts into \"personal\" using ONLY the <user_turns> block: "
        "no personal fact may come from <assistant_turns>, and every personal fact must be traceable to a numbered user turn.\n"
        "End every \"personal\" fact with a citation of the user turn that establishes it, in the exact form \" [turn N]\".\n"
        "Extract the assistant's operational session state across \"objective\", \"workdone\", \"blocker\", \"next_step\", and \"pitfall\" "
        "using both blocks as context.\n"
        "Output ONLY the raw JSON object starting with { and ending with }.\n"
        "</task>"
    )


def call_ollama(base_url: str, model: str, system: str, user_content: str,
                num_ctx: int, num_predict: int, seed: int | None,
                timeout_s: int) -> dict:
    body: dict = {
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_content},
        ],
        "stream": False,
        "think": True,
        "options": {
            "temperature": TEMPERATURE,
            "num_ctx": num_ctx,
            "num_predict": num_predict,
        },
        "format": COMPACTION_SCHEMA,
    }
    if seed is not None:
        body["options"]["seed"] = seed
    resp = requests.post(f"{base_url}/api/chat", json=body, timeout=timeout_s)
    resp.raise_for_status()
    data = resp.json()
    content = (data.get("message") or {}).get("content", "")
    return {"content": content, "raw": data}


def call_nvidia(base_url: str, api_key: str, model: str, system: str,
                user_content: str, max_tokens: int, seed: int | None,
                timeout_s: int, retries: int = 4) -> dict:
    """Baseline via NVIDIA NIM (OpenAI-compatible /chat/completions).

    Same prompt, same temperature, generous output budget. Retries on
    rate-limit/transient errors with backoff; surfaces finish_reason so a
    truncated ceiling can never pass silently.
    """
    url = base_url.rstrip("/") + "/chat/completions"
    body: dict = {
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_content},
        ],
        "temperature": TEMPERATURE,
        "max_tokens": max_tokens,
        "stream": False,
    }
    if seed is not None:
        body["seed"] = seed
    headers = {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}
    last_err: Exception | None = None
    for attempt in range(retries):
        try:
            resp = requests.post(url, json=body, headers=headers, timeout=timeout_s)
            if resp.status_code in (429, 500, 502, 503, 529):
                last_err = RuntimeError(f"HTTP {resp.status_code}: {resp.text[:200]}")
                time.sleep(2 ** attempt * 5)
                continue
            resp.raise_for_status()
            data = resp.json()
            choice = (data.get("choices") or [{}])[0]
            if choice.get("finish_reason") == "length":
                raise ValueError("baseline truncated by max_tokens; raise --max-tokens")
            msg = choice.get("message") or {}
            content = msg.get("content") or ""
            # Reasoning models can burn the whole token budget on
            # `reasoning_content` and return an empty `content`. Falling back to
            # the reasoning prose here would feed non-JSON into the parser and
            # produce a garbage ceiling that still looks like a valid baseline.
            # Fail loudly instead: the budget is too small for this model.
            if not content.strip():
                raise ValueError(
                    "empty content with finish_reason="
                    f"{choice.get('finish_reason')!r}; reasoning tokens likely "
                    "exhausted max_tokens. Raise --max-tokens."
                )
            return {"content": content, "raw": data}
        except (ValueError, requests.HTTPError) as e:
            raise
        except Exception as e:  # noqa: BLE001 - retried below
            last_err = e
            time.sleep(2 ** attempt * 5)
    raise RuntimeError(f"NVIDIA baseline call failed after {retries} attempts: {last_err}")


def strip_turn_citation(text: str) -> str:
    """Mirror runner.rs strip_turn_citation: remove a trailing [turn N].

    The baseline must store the same clean text production stores, or the judge
    compares cited against uncited strings. Citation compliance is measured from
    the raw output before stripping.
    """
    import re as _re
    return _re.sub(r"\s*\[\s*[Tt][Uu][Rr][Nn]\s+\d+\s*\]\s*$", "", text.strip())


def coerce_document(content: str) -> dict:
    """Parse the model output into the 6-bucket document, tolerating fences."""
    text = content.strip()
    if "```" in text:
        parts = text.split("```")
        # Take the first fenced block that parses, else fall through.
        for part in parts:
            candidate = part.strip()
            if candidate.startswith("{"):
                try:
                    return json.loads(candidate)
                except json.JSONDecodeError:
                    continue
    start = text.find("{")
    end = text.rfind("}")
    if start < 0 or end < 0 or end < start:
        raise ValueError(f"no JSON object in model output: {text[:400]}")
    doc = json.loads(text[start:end + 1])
    if not isinstance(doc, dict):
        raise ValueError("model output is not a JSON object")
    for key in CATEGORIES:
        if key not in doc or not isinstance(doc[key], list):
            doc[key] = []
    # Store clean text, mirroring production. Citation compliance is counted
    # from the raw output in main() before this runs.
    if isinstance(doc.get("personal"), list):
        doc["personal"] = [strip_turn_citation(x) if isinstance(x, str) else x
                           for x in doc["personal"]]
    return doc


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate the Gemma baseline ceiling.")
    ap.add_argument("--run-dir", required=True, help="Run directory containing case dirs with slices/")
    ap.add_argument("--model", default="gemma4:12b")
    ap.add_argument("--ollama", default="http://127.0.0.1:11434")
    ap.add_argument("--num-ctx", type=int, default=32768)
    ap.add_argument("--num-predict", type=int, default=8192)
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--timeout", type=int, default=180)
    ap.add_argument("--cases", default=None,
                    help="Comma-separated case dir names to process (default: all with slices/)")
    ap.add_argument("--backend", choices=["ollama", "nvidia"], default="ollama",
                    help="Baseline provider. nvidia uses NIM OpenAI-compatible endpoint.")
    ap.add_argument("--nvidia-model", default=None,
                    help="NIM model id (required with --backend nvidia)")
    ap.add_argument("--nvidia-key", default=None,
                    help="NIM API key (default: NVIDIA_API_KEY env var)")
    ap.add_argument("--nvidia-url", default="https://integrate.api.nvidia.com/v1")
    ap.add_argument("--max-tokens", type=int, default=8192,
                    help="Output budget for the nvidia backend")
    ap.add_argument("--workers", type=int, default=6,
                    help="Concurrent cases. Cases are independent; slices within "
                         "a case stay sequential because each consumes the last.")
    args = ap.parse_args()

    use_nvidia = args.backend == "nvidia"
    if use_nvidia:
        if not args.nvidia_model:
            print("--nvidia-model is required with --backend nvidia", file=sys.stderr)
            return 1
        nvidia_key = args.nvidia_key or os.environ.get("NVIDIA_API_KEY", "")
        if not nvidia_key:
            print("NVIDIA key missing: pass --nvidia-key or set NVIDIA_API_KEY", file=sys.stderr)
            return 1
    else:
        nvidia_key = ""

    run_dir = Path(args.run_dir)
    system_prompt = read_system_prompt()
    print(f"system prompt: {len(system_prompt)} chars, read from {PROMPT_RS.name}", flush=True)

    case_dirs = sorted(p for p in run_dir.iterdir() if p.is_dir() and (p / "slices").is_dir())
    if args.cases:
        wanted = set(args.cases.split(","))
        case_dirs = [p for p in case_dirs if p.name in wanted]
    if not case_dirs:
        print("no case directories with slices/ found", file=sys.stderr)
        return 1

    # Cases are independent, so they run concurrently. Slices within a case stay
    # strictly sequential because each one consumes the previous slice's output —
    # that chain is the production compaction contract and cannot be parallelised.
    # The worker cap keeps us under the provider's per-key concurrency limit;
    # going wider just buys 429s.
    def process_case(case_dir: Path) -> tuple[int, list[str]]:
        slice_files = sorted((case_dir / "slices").glob("slice_*.json"))
        out_dir = case_dir / "baseline"
        out_dir.mkdir(exist_ok=True)
        # Provenance: the judge prompt and run manifest must state whose ceiling
        # this is. A baseline without a recorded source is unverifiable.
        meta = {
            "backend": args.backend,
            "model": args.nvidia_model if use_nvidia else args.model,
            "seed": args.seed,
            "temperature": TEMPERATURE,
            "max_tokens": args.max_tokens,
            "system_prompt_chars": len(system_prompt),
            "system_prompt_source": str(PROMPT_RS),
        }
        (out_dir / "_meta.json").write_text(json.dumps(meta, indent=2))
        case_failures: list[str] = []
        prior_summary: str | None = None
        n_facts = 0
        print(f"== {case_dir.name}: {len(slice_files)} slices ==", flush=True)
        for sf in slice_files:
            sl = json.loads(sf.read_text())
            user_content = build_user_content(sl["messages"], prior_summary)
            label = f"{case_dir.name}/{sf.stem}"
            try:
                t0 = time.time()
                if use_nvidia:
                    result = call_nvidia(args.nvidia_url, nvidia_key, args.nvidia_model,
                                         system_prompt, user_content, args.max_tokens,
                                         args.seed, args.timeout)
                else:
                    result = call_ollama(args.ollama, args.model, system_prompt, user_content,
                                         args.num_ctx, args.num_predict, args.seed, args.timeout)
                doc = coerce_document(result["content"])
            except Exception as e:  # noqa: BLE001 - surfaced per slice, run continues
                print(f"  {sf.stem}: FAILED ({e})", flush=True)
                case_failures.append(label)
                continue
            facts = sum(len(doc.get(k) or []) for k in CATEGORIES)
            n_facts += facts
            (out_dir / f"{sf.stem}.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False))
            dt = time.time() - t0
            print(f"  {case_dir.name}/{sf.stem}: {facts} facts in {dt:.0f}s "
                  f"(turns {sl['from_turn']}..{sl['to_turn']})", flush=True)
            # Chain this slice's output into the next, exactly as production does.
            prior_summary = format_session_context(doc)
        print(f"== {case_dir.name}: {n_facts} baseline facts ==", flush=True)
        return n_facts, case_failures

    t_start = time.time()
    failures: list[str] = []
    with ThreadPoolExecutor(max_workers=args.workers) as ex:
        futs = {ex.submit(process_case, c): c for c in case_dirs}
        for fut in as_completed(futs):
            _, case_failures = fut.result()
            failures.extend(case_failures)
    print(f"\nall cases finished in {time.time() - t_start:.0f}s", flush=True)

    if failures:
        print(f"\n{len(failures)} slice(s) failed:", file=sys.stderr)
        for f in failures:
            print(f"  {f}", file=sys.stderr)
        return 1
    print("\nbaseline complete", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
