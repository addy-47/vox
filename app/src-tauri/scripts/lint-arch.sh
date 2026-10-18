#!/usr/bin/env bash
#
# Static architectural invariant gate for Vox backend.
# Enforces non-negotiable architectural boundaries defined in docs/specs/
#
# Invariants checked:
#   1. RULE-DB-01: Single Boundary — No SQL statements outside src/persistence/
#   2. RULE-DB-01 & RULE-DB-03: Gateway Isolation — No direct db.connect() outside src/persistence/ and lib.rs bootstrap
#   3. RULE-STORAGE-01: Path SSOT — No hardcoded ~/.vox or vox.db paths outside src/utils/paths.rs
#   4. RULE-CONC-01: Named Threads — No bare std::thread::spawn without Builder::name()
#   5. RULE-DB-04 & RULE-NOTIF-08: Hot-Path Isolation — No DB or notification calls in services/audio/
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC_DIR="$ROOT_DIR/src"

python3 - << 'EOF'
import os
import re
import sys

src_dir = os.path.abspath("src")
failures = 0

def report_violation(rule, file_path, line_no, msg):
    global failures
    rel_path = os.path.relpath(file_path, os.getcwd())
    print(f"\033[1;31m✖ [{rule}]\033[0m {rel_path}:{line_no} — {msg}")
    failures += 1

# Patterns
SQL_REGEX = re.compile(
    r'\b(SELECT\s+[A-Za-z0-9_,\s\*\(\)]+\s+FROM|INSERT\s+INTO|UPDATE\s+[A-Za-z0-9_]+\s+SET|DELETE\s+FROM|BEGIN\s+(TRANSACTION|IMMEDIATE|CONCURRENT)|DROP\s+TABLE|CREATE\s+TABLE)\b',
    re.IGNORECASE
)
STORAGE_PATH_REGEX = re.compile(r'("~/\.vox|"/\.vox/|"\.vox/data|"\.vox/config|"\.vox/vox\.db")')
BARE_SPAWN_REGEX = re.compile(r'(\bthread::spawn\(|\bstd::thread::spawn\()')

print("==> Scanning backend architectural invariants...")

for root, _, files in os.walk(src_dir):
    for f in files:
        if not f.endswith(".rs"):
            continue
        file_path = os.path.join(root, f)
        rel_to_src = os.path.relpath(file_path, src_dir)

        # Skip tests or mocks
        if "test" in f or "mock" in f:
            continue

        with open(file_path, "r", encoding="utf-8", errors="ignore") as fp:
            content = fp.read()
            lines = content.splitlines()

        is_persistence = rel_to_src.startswith("persistence/")
        is_paths_rs = rel_to_src == "utils/paths.rs"
        is_lib_rs = rel_to_src == "lib.rs"
        is_audio = rel_to_src.startswith("services/audio/")

        # 1. RULE-DB-01: No SQL outside persistence/
        if not is_persistence:
            for i, line in enumerate(lines, 1):
                m = SQL_REGEX.search(line)
                if m:
                    report_violation("RULE-DB-01", file_path, i, f"Direct SQL query outside persistence layer: {line.strip()}")

        # 2. RULE-DB-01 & RULE-DB-03: No direct db.connect() outside persistence/ & lib.rs bootstrap
        if not is_persistence and not is_lib_rs:
            for i, line in enumerate(lines, 1):
                if ".connect()" in line:
                    # check previous 2 lines for state.db or db
                    window = " ".join([l.strip() for l in lines[max(0, i-3):i]])
                    if "db" in window or "state" in window:
                        report_violation("RULE-DB-01", file_path, i, f"Direct DB connection vending outside persistence layer: {line.strip()}")

        # 3. RULE-STORAGE-01: No hardcoded ~/.vox paths outside utils/paths.rs
        if not is_paths_rs:
            for i, line in enumerate(lines, 1):
                m = STORAGE_PATH_REGEX.search(line)
                if m:
                    report_violation("RULE-STORAGE-01", file_path, i, f"Hardcoded storage path outside paths.rs: {m.group(0)}")

        # 4. RULE-CONC-01: No bare thread::spawn
        for i, line in enumerate(lines, 1):
            m = BARE_SPAWN_REGEX.search(line)
            if m:
                report_violation("RULE-CONC-01", file_path, i, f"Bare thread spawn without Builder name/priority: {line.strip()}")

        # 5. RULE-DB-04 & RULE-NOTIF-08: Hot-path audio isolation
        if is_audio:
            for i, line in enumerate(lines, 1):
                if "db.connect" in line or "notify(" in line:
                    report_violation("RULE-DB-04", file_path, i, f"Database/Notification call in real-time audio hot-path: {line.strip()}")

print("------------------------------------------------------------------------")
if failures > 0:
    print(f"\033[1;31mFAILED: {failures} architectural invariant violation(s) detected.\033[0m")
    sys.exit(1)
else:
    print("\033[1;32mSUCCESS: All backend architectural invariants hold.\033[0m")
    sys.exit(0)
EOF
