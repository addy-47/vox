#!/usr/bin/env python3
"""
remote-mutate.py — Automated remote mutation testing runner for Vox.

Offloads mutation testing campaigns to the 32-core remote server (from temp/server.txt),
keeping local CPU and cargo locks completely free.

Usage:
  ./scripts/remote-mutate.py --mutants-file mutants.json
  ./scripts/remote-mutate.py --help
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
APP_SRC_TAURI = SCRIPT_DIR.parent
PROJECT_ROOT = APP_SRC_TAURI.parent.parent
SERVER_FILE = PROJECT_ROOT / "temp" / "server.txt"
REMOTE_WORKSPACE = "~/.vox-remote-workspace/app/src-tauri"


def parse_server_target(server_file: Path) -> str:
    if not server_file.is_file():
        raise FileNotFoundError(f"Server config not found at {server_file}")
    with open(server_file, "r", encoding="utf-8") as f:
        first_line = f.readline().strip()
    match = re.search(r"[a-zA-Z0-9_-]+@[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+", first_line)
    if not match:
        raise ValueError(f"Could not parse user@ip from: {first_line}")
    return match.group(0)


def run_remote_ssh(target: str, cmd: str, timeout: int = 180) -> subprocess.CompletedProcess:
    wrapper = f"""
source ~/.cargo/env 2>/dev/null || export PATH="$HOME/.cargo/bin:$PATH"
cd {REMOTE_WORKSPACE}
export RAYON_NUM_THREADS=$(nproc)
export OMP_NUM_THREADS=$(nproc)
export VOX_MODELS_DIR="${{VOX_MODELS_DIR:-$HOME/.vox/models}}"
{cmd}
"""
    return subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", target, f"bash -lc '{wrapper}'"],
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def sync_workspace(target: str):
    print(f"[remote-mutate] Syncing {APP_SRC_TAURI} to {target}:{REMOTE_WORKSPACE}...")
    mkdir_proc = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", target, f"mkdir -p {REMOTE_WORKSPACE}"],
        capture_output=True,
        text=True,
    )
    if mkdir_proc.returncode != 0:
        raise RuntimeError(f"Failed to create remote directory: {mkdir_proc.stderr}")

    rsync_cmd = [
        "rsync",
        "-az",
        "--delete",
        "--exclude=target/",
        "--exclude=.git/",
        "--exclude=node_modules/",
        "--exclude=temp/",
        "--exclude=*.log",
        f"{APP_SRC_TAURI}/",
        f"{target}:{REMOTE_WORKSPACE}/",
    ]
    t0 = time.time()
    res = subprocess.run(rsync_cmd, capture_output=True, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"Rsync failed: {res.stderr}")
    print(f"[remote-mutate] Workspace synced in {time.time() - t0:.2f}s.")


def cleanup_remote(target: str):
    print("[remote-mutate] Cleaning up remote working tree...")
    run_remote_ssh(target, "git checkout -- . 2>/dev/null || true")


def main():
    parser = argparse.ArgumentParser(description="Vox Remote Mutation Testing Tool")
    parser.add_argument(
        "--mutants-file",
        type=str,
        help="JSON file containing list of mutants: [{name, file, target, replacement, test_command}]",
    )
    parser.add_argument(
        "--report",
        type=str,
        default=str(PROJECT_ROOT / "docs" / "tests" / "mutation_report.md"),
        help="Output markdown report path",
    )
    parser.add_argument(
        "--skip-sync",
        action="store_true",
        help="Skip initial rsync if remote is already synced",
    )

    args = parser.parse_args()

    if not args.mutants_file:
        print("ERROR: --mutants-file is required. See --help for format.")
        sys.exit(1)

    with open(args.mutants_file, "r", encoding="utf-8") as f:
        mutants = json.load(f)

    if not isinstance(mutants, list) or len(mutants) == 0:
        print("ERROR: mutants file must contain a non-empty JSON array of mutant definitions.")
        sys.exit(1)

    target = parse_server_target(SERVER_FILE)
    print(f"[remote-mutate] Target remote 32-core server: {target}")

    if not args.skip_sync:
        sync_workspace(target)

    results = []
    try:
        # 1. Baseline verification (must be GREEN)
        first_test = mutants[0]["test_command"]
        print(f"[remote-mutate] Running baseline test: {first_test}...")
        t0 = time.time()
        base_res = run_remote_ssh(target, first_test, timeout=300)
        baseline_time = time.time() - t0
        if base_res.returncode != 0:
            print(f"[remote-mutate] FATAL: Baseline test failed (RED) before any mutation!\n{base_res.stderr}\n{base_res.stdout}")
            sys.exit(1)
        print(f"[remote-mutate] Baseline GREEN confirmed in {baseline_time:.2f}s.")

        # 2. Iterate mutants
        for idx, m in enumerate(mutants, 1):
            name = m.get("name", f"Mutant-{idx}")
            rel_file = m["file"]
            target_str = m["target"]
            replacement = m["replacement"]
            test_cmd = m["test_command"]

            print(f"\n[remote-mutate] [{idx}/{len(mutants)}] Testing: {name}")
            print(f"  File: {rel_file}")
            print(f"  Mutation: '{target_str.strip()}' -> '{replacement.strip()}'")

            # Apply mutation remotely via python one-liner on server
            py_mutator = (
                f"python3 -c \""
                f"path = '{rel_file}'; "
                f"data = open(path).read(); "
                f"assert '{target_str}' in data, 'Target string not found in ' + path; "
                f"open(path, 'w').write(data.replace('{target_str}', '{replacement}', 1))\""
            )
            apply_res = run_remote_ssh(target, py_mutator)
            if apply_res.returncode != 0:
                print(f"  ERROR: Failed to apply mutant: {apply_res.stderr}")
                results.append({"name": name, "status": "ERROR_APPLY", "killed": False, "time": 0})
                continue

            # Run test against mutant
            t_start = time.time()
            test_res = run_remote_ssh(target, test_cmd, timeout=300)
            elapsed = time.time() - t_start

            # Revert immediately
            revert_proc = run_remote_ssh(target, f"git checkout -- {rel_file}")
            if revert_proc.returncode != 0:
                print(f"  WARNING: Git checkout failed for {rel_file}, using rsync restore...")
                sync_workspace(target)

            if test_res.returncode != 0:
                print(f"  Result: KILLED (Red) in {elapsed:.2f}s [PASS]")
                results.append({"name": name, "file": rel_file, "status": "KILLED", "killed": True, "time": elapsed})
            else:
                print(f"  Result: SURVIVED (Green) in {elapsed:.2f}s [FAIL — Weak Test]")
                results.append({"name": name, "file": rel_file, "status": "SURVIVED", "killed": False, "time": elapsed})

    finally:
        cleanup_remote(target)

    # 3. Output Summary Table & Write Report
    total = len(results)
    killed = sum(1 for r in results if r.get("killed", False))
    score = (killed / total * 100) if total > 0 else 0.0

    print("\n" + "=" * 60)
    print(f"MUTATION CAMPAIGN COMPLETE: {killed}/{total} KILLED ({score:.1f}%)")
    print("=" * 60)
    for r in results:
        status_sym = "✅ KILLED" if r["killed"] else "❌ SURVIVED"
        print(f"  {r['name']:<35} | {status_sym} | {r.get('time', 0):.2f}s")
    print("=" * 60)

    # Write report
    report_path = Path(args.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    with open(report_path, "w", encoding="utf-8") as f:
        f.write("# Mutation Testing Campaign Report\n\n")
        f.write(f"- **Server**: {target} (32-core remote build server)\n")
        f.write(f"- **Total Mutants**: {total}\n")
        f.write(f"- **Killed**: {killed}\n")
        f.write(f"- **Mutation Score**: {score:.1f}%\n\n")
        f.write("| Mutant | File | Status | Elapsed |\n")
        f.write("| :--- | :--- | :--- | :--- |\n")
        for r in results:
            sym = "KILLED" if r["killed"] else "**SURVIVED**"
            f.write(f"| {r['name']} | `{r.get('file', '')}` | {sym} | {r.get('time', 0):.2f}s |\n")

    print(f"[remote-mutate] Report generated at {report_path}")
    sys.exit(0 if killed == total else 1)


if __name__ == "__main__":
    main()
