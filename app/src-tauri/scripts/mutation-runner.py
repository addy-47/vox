#!/usr/bin/env python3
"""mutation-runner.py — generalized, reversible mutation campaign runner for Vox.

Usage:
  ./scripts/mutation-runner.py --mutants tests/assets/mutants.json --smoke
  ./scripts/mutation-runner.py --mutants tests/assets/mutants.json --mode local
  ./scripts/mutation-runner.py --mutants tests/assets/mutants.json --mode remote

Modes:
  local  (default) — run on this machine. This workspace is already the remote GPU server.
  remote            — read temp/server.txt and run via ssh in ~/.vox-remote-workspace/app/src-tauri.

Every mutant is applied as an exact one-line source edit, the bound test runs in release
mode, and the original bytes are restored in a finally block. A mutant that fails to
revert aborts the campaign. Results are written to docs/tests/mutation_report.md.
"""

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
APP_SRC_TAURI = SCRIPT_DIR.parent
PROJECT_ROOT = APP_SRC_TAURI.parent.parent
DEFAULT_MUTANTS = APP_SRC_TAURI / "tests" / "assets" / "mutants.json"
DEFAULT_REPORT = PROJECT_ROOT / "docs" / "tests" / "mutation_report.md"
SERVER_FILE = PROJECT_ROOT / "temp" / "server.txt"
REMOTE_WORKSPACE = "~/.vox-remote-workspace/app/src-tauri"


def log(msg):
    print(f"[mutation-runner] {msg}", flush=True)


def load_mutants(path: Path):
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("version") != 1 or not isinstance(data.get("mutants"), list):
        raise ValueError("mutants.json must be version 1 with a 'mutants' array")
    required = {"id", "name", "file", "target", "replacement", "test"}
    for m in data["mutants"]:
        if m.get("skip_reason"):
            missing = {"id", "name", "test"} - set(m)
        else:
            missing = required - set(m)
        if missing:
            raise ValueError(f"mutant {m.get('id', '?')} missing keys: {sorted(missing)}")
        if not m["test"].get("binary"):
            raise ValueError(f"mutant {m['id']} has no test.binary")
    return data["mutants"]


def remote_target() -> str:
    if not SERVER_FILE.is_file():
        raise FileNotFoundError(f"remote mode needs {SERVER_FILE}")
    first = SERVER_FILE.read_text(encoding="utf-8").splitlines()[0]
    match = re.search(r"[A-Za-z0-9_-]+@\d+\.\d+\.\d+\.\d+", first)
    if not match:
        raise ValueError(f"cannot parse user@ip from {first!r}")
    return match.group(0)


def test_command(mutant) -> str:
    binary = mutant["test"]["binary"]
    filter_name = mutant["test"].get("filter")
    cmd = ["cargo", "nextest", "run", "--test", binary, "--release", "--test-threads=1", "--no-fail-fast"]
    if filter_name:
        cmd += ["-E", f"test({filter_name})"]
    return " ".join(shlex.quote(part) for part in cmd)


def run_local(command: str, timeout: int):
    env = os.environ.copy()
    env.setdefault("VOX_MODELS_DIR", str(Path.home() / ".vox" / "models"))
    env["RAYON_NUM_THREADS"] = str(os.cpu_count() or 1)
    env["OMP_NUM_THREADS"] = env["RAYON_NUM_THREADS"]
    started = time.monotonic()
    proc = subprocess.run(
        command, shell=True, cwd=APP_SRC_TAURI, env=env,
        capture_output=True, text=True, timeout=timeout,
    )
    return proc.returncode, time.monotonic() - started, proc.stdout + proc.stderr


def run_remote(command: str, timeout: int):
    target = remote_target()
    wrapper = (
        "source ~/.cargo/env 2>/dev/null || export PATH=\"$HOME/.cargo/bin:$PATH\";\n"
        f"cd {REMOTE_WORKSPACE};\n"
        "export RAYON_NUM_THREADS=$(nproc); export OMP_NUM_THREADS=$(nproc);\n"
        f"{command}"
    )
    started = time.monotonic()
    proc = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", target,
         "bash", "-lc", wrapper],
        capture_output=True, text=True, timeout=timeout,
    )
    return proc.returncode, time.monotonic() - started, proc.stdout + proc.stderr


def apply_mutant(path: Path, target: str, replacement: str):
    original = path.read_bytes()
    text = original.decode("utf-8")
    count = text.count(target)
    if count != 1:
        raise ValueError(f"{path}: expected exactly one target occurrence, found {count}")
    path.write_bytes(text.replace(target, replacement, 1).encode("utf-8"))
    return original


def restore_mutant(path: Path, original: bytes):
    path.write_bytes(original)
    if path.read_bytes() != original:
        raise RuntimeError(f"FAILED TO REVERT {path} — stop immediately")


def display_path(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(PROJECT_ROOT))
    except ValueError:
        return str(path)


def write_report(results, mode, mutants_path, smoke, phase=None):
    lines = [
        "# Vox Mutation Report",
        "",
        "**Status:** " + (phase or ("SMOKE TEST ONLY — not a full campaign" if smoke else "FULL CAMPAIGN")),
        f"**Mode:** {mode}  ",
        f"**Mutant set:** `{display_path(mutants_path)}`  ",
        f"**Runner:** `app/src-tauri/scripts/mutation-runner.py`",
        "",
        "| Mutant | Test | Killed | Seconds | Note |",
        "| :--- | :--- | :---: | ---: | :--- |",
    ]
    killed = 0
    survived = 0
    skipped = 0
    for r in results:
        if r.get("skipped"):
            state = "skip"
            skipped += 1
        else:
            state = "yes" if r["killed"] else "NO"
        killed += int(r["killed"])
        survived += int(not r["killed"] and not r.get("skipped"))
        note = (r.get("error") or r.get("skip_reason") or r.get("note") or "").replace("\n", " ")[:120]
        lines.append(
            f"| {r['id']} — {r['name']} | `{r['test']}` | {state} | {r['seconds']:.1f} | {note} |"
        )
    total = killed + survived
    score = f"{100.0 * killed / total:.1f}%" if total else "n/a"
    lines += [
        "",
        f"**Mutation score:** {score} ({killed}/{total} killed; {survived} survived; {skipped} skipped as unmutatable).",
        "",
        "## Surviving mutants — RCA pending",
        "",
    ]
    survivors = [r for r in results if not r["killed"] and not r.get("skipped")]
    if survivors:
        for r in survivors:
            lines.append(f"- **{r['id']} — {r['name']}** (`{r['test']}`): {r.get('error', 'test passed under the mutant')}")
            tail = r.get("output_tail", "").strip()
            if tail:
                lines.append("  ```text")
                lines.extend("  " + line for line in tail.splitlines())
                lines.append("  ```")
    else:
        lines.append("None in this run.")
    lines += [
        "",
        "All mutants were restored byte-for-byte after execution. A survivor means the "
        "test is weak or the mutant is wrong; triage before merge. RCA text is added "
        "manually after triage.",
        "",
    ]
    DEFAULT_REPORT.parent.mkdir(parents=True, exist_ok=True)
    DEFAULT_REPORT.write_text("\n".join(lines), encoding="utf-8")
    log(f"report written to {DEFAULT_REPORT}")


def main():
    global DEFAULT_REPORT
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--mutants", type=Path, default=DEFAULT_MUTANTS)
    ap.add_argument("--mode", choices=["local", "remote"], default="local")
    ap.add_argument("--smoke", action="store_true", help="run only mutants tagged smoke: true")
    ap.add_argument("--dry-run", action="store_true", help="validate inputs without applying mutants")
    ap.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    ap.add_argument("--ids", help="comma-separated mutant ids to run (targeted re-run)")
    args = ap.parse_args()

    DEFAULT_REPORT = args.report
    mutants = load_mutants(args.mutants)
    if args.ids:
        wanted = {x.strip() for x in args.ids.split(",") if x.strip()}
        mutants = [m for m in mutants if m["id"] in wanted]
        if not mutants:
            raise SystemExit(f"no mutants matched ids: {wanted}")
    if args.smoke:
        mutants = [m for m in mutants if m.get("smoke")]
        if not mutants:
            raise SystemExit("no mutants tagged smoke: true")
    log(f"{len(mutants)} mutant(s); mode={args.mode}; dry_run={args.dry_run}")

    if args.dry_run:
        for m in mutants:
            if m.get("skip_reason"):
                log(f"SKIP {m['id']} — {m['skip_reason']}")
                continue
            path = APP_SRC_TAURI / m["file"]
            if not path.is_file():
                raise SystemExit(f"missing target file {path}")
            count = path.read_text(encoding="utf-8").count(m["target"])
            if count != 1:
                raise SystemExit(f"{m['id']}: target occurs {count} times in {path}")
            log(f"OK {m['id']} {m['name']}")
        return

    results = []
    for m in mutants:
        if m.get("skip_reason"):
            results.append({
                "id": m["id"], "name": m["name"], "test": m["test"]["binary"],
                "killed": False, "seconds": 0.0, "skipped": True,
                "skip_reason": m["skip_reason"], "output_tail": "",
            })
            log(f"{m['id']}: SKIPPED — {m['skip_reason']}")
            write_report(results, args.mode, args.mutants, args.smoke,
                         phase=f"CAMPAIGN IN PROGRESS — {len(results)}/{len(mutants)}")
            continue
        path = APP_SRC_TAURI / m["file"]
        command = test_command(m)
        original = None
        try:
            original = apply_mutant(path, m["target"], m["replacement"])
            log(f"{m['id']}: applied to {m['file']}; running {command}")
            runner = run_local if args.mode == "local" else run_remote
            code, seconds, output = runner(command, 900)
            killed = code != 0
            results.append({
                "id": m["id"], "name": m["name"], "test": m["test"]["binary"],
                "killed": killed, "seconds": seconds,
                "note": "expected-failure: " + m.get("expected", "test fails") if killed else "test passed under mutant",
                "output_tail": output[-1200:],
            })
            log(f"{m['id']}: {'KILLED' if killed else 'SURVIVED'} in {seconds:.1f}s")
        except subprocess.TimeoutExpired:
            results.append({
                "id": m["id"], "name": m["name"], "test": m["test"]["binary"],
                "killed": False, "seconds": 900.0,
                "error": "runner timeout — inconclusive, manual triage required",
                "output_tail": "",
            })
            log(f"{m['id']}: TIMEOUT")
        except Exception as exc:
            results.append({
                "id": m["id"], "name": m["name"], "test": m["test"]["binary"],
                "killed": False, "seconds": 0.0, "error": f"runner error: {exc}",
                "output_tail": "",
            })
            log(f"{m['id']}: ERROR {exc}")
        finally:
            if original is not None:
                restore_mutant(path, original)
                log(f"{m['id']}: production file reverted")
        write_report(results, args.mode, args.mutants, args.smoke,
                     phase=f"CAMPAIGN IN PROGRESS — {len(results)}/{len(mutants)}")
    write_report(results, args.mode, args.mutants, args.smoke)
    if any(not r["killed"] and "error" in r for r in results):
        raise SystemExit("campaign had inconclusive mutants — see report")


if __name__ == "__main__":
    main()
