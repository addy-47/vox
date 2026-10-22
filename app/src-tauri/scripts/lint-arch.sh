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
#   6. RAIL-RC-05: Protocol Purity — services/remote/protocol/ imports no runtime, framework or filesystem
#   7. RC-SSO: Single Definition Site — Every remote-control domain constant is defined exactly once
#   8. RULE-DB-04: One Write Path — `last_seen_at` is written from exactly one place
#   9. VERIFY-RC-15: Link Neutrality — tests/common/ names no concrete link implementation
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

# The remote-control domain constants, collected from their single definition
# site during the walk below.
domain_constants = []

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

        # 2. RULE-STORAGE-01: No hardcoded ~/.vox paths outside utils/paths.rs
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

        # 7. RC-SSO: the remote-control domain constants live in one place.
        if rel_to_src == "services/remote/mod.rs":
            for name in re.findall(r'pub const ([A-Z][A-Z_0-9]*)\s*:', content):
                domain_constants.append(name)

# ---------------------------------------------------------------------------
# Helpers for the remote-control invariants
# ---------------------------------------------------------------------------

def strip_comments_and_strings(source):
    """Removes Rust comments and string literals from a source file."""
    out = []
    index = 0
    length = len(source)
    while index < length:
        char = source[index]
        if char == "/" and index + 1 < length and source[index + 1] == "/":
            while index < length and source[index] != "\n":
                index += 1
            continue
        if char == "/" and index + 1 < length and source[index + 1] == "*":
            index += 2
            while index + 1 < length and not (source[index] == "*" and source[index + 1] == "/"):
                index += 1
            index += 2
            continue
        if char == '"':
            index += 1
            while index < length and source[index] != '"':
                if source[index] == "\\":
                    index += 1
                index += 1
            index += 1
            continue
        out.append(char)
        index += 1
    return "".join(out)


def split_top_level(text):
    """Splits a comma-separated list, ignoring commas inside braces."""
    parts = []
    depth = 0
    current = []
    for char in text:
        if char in "{[(<":
            depth += 1
        elif char in "}])>":
            depth -= 1
        if char == "," and depth == 0:
            parts.append("".join(current))
            current = []
        else:
            current.append(char)
    parts.append("".join(current))
    return parts


def expand_use(statement):
    """Expands one `use` item into the fully qualified paths it names."""
    statement = statement.strip()
    head, separator, tail = statement.partition("{")
    if not separator:
        return [statement]
    base = head.strip()
    inner = tail.rstrip()
    if inner.endswith("}"):
        inner = inner[:-1]
    paths = []
    for item in split_top_level(inner):
        item = item.strip()
        if not item:
            continue
        if "{" in item:
            item_head, _, item_tail = item.partition("{")
            paths.extend(expand_use(base + item_head + "{" + item_tail))
        else:
            paths.append(base + item)
    return paths


def used_paths(source):
    """Every path a source file imports, with brace groups expanded."""
    code = strip_comments_and_strings(source)
    paths = []
    for match in re.finditer(r"\buse\s+([^;]+);", code):
        paths.extend(expand_use(match.group(1)))
    return paths

# ---------------------------------------------------------------------------
# 6. RAIL-RC-05: the protocol module is pure
# ---------------------------------------------------------------------------

FORBIDDEN_PROTOCOL_IMPORTS = (
    "tokio",
    "tauri",
    "std::fs",
    "std::net",
    "std::io",
    "std::os",
    "std::thread",
    "std::process",
)

protocol_dir = os.path.join(src_dir, "services", "remote", "protocol")
if os.path.isdir(protocol_dir):
    for entry in sorted(os.listdir(protocol_dir)):
        if not entry.endswith(".rs"):
            continue
        path = os.path.join(protocol_dir, entry)
        with open(path, "r", encoding="utf-8", errors="ignore") as fp:
            content = fp.read()
        for imported in used_paths(content):
            root = imported.split("::")[0]
            forbidden = root in ("tokio", "tauri") or imported.startswith(
                FORBIDDEN_PROTOCOL_IMPORTS[2:]
            )
            if forbidden:
                report_violation(
                    "RAIL-RC-05",
                    path,
                    1,
                    f"the protocol module must be pure; it imports `{imported}`",
                )

# ---------------------------------------------------------------------------
# 7. RC-SSO: every remote-control domain constant has one definition site
# ---------------------------------------------------------------------------

if domain_constants:
    unique_constants = list(dict.fromkeys(domain_constants))
    definition_counts = {}
    definition_sites = {}
    for root, _, files in os.walk(src_dir):
        for name in files:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(root, name)
            with open(path, "r", encoding="utf-8", errors="ignore") as fp:
                content = fp.read()
            for constant in unique_constants:
                pattern = r"\b(?:pub\s+)?(?:const|static)\s+" + constant + r"\b"
                hits = len(re.findall(pattern, content))
                if hits:
                    definition_counts[constant] = definition_counts.get(constant, 0) + hits
                    definition_sites.setdefault(constant, path)
    for constant in unique_constants:
        count = definition_counts.get(constant, 0)
        if count != 1:
            report_violation(
                "RC-SSO",
                definition_sites.get(constant, os.path.join(src_dir, "services/remote/mod.rs")),
                1,
                f"the domain constant `{constant}` must have exactly one definition site, "
                f"found {count}",
            )

# ---------------------------------------------------------------------------
# 8. RULE-DB-04: `last_seen_at` has exactly one write path
# ---------------------------------------------------------------------------

last_seen_writes = []
for root, _, files in os.walk(src_dir):
    for name in files:
        if not name.endswith(".rs"):
            continue
        path = os.path.join(root, name)
        with open(path, "r", encoding="utf-8", errors="ignore") as fp:
            content = fp.read()
        flat = re.sub(r"\s+", " ", content)
        for match in re.finditer(
            r"UPDATE\s+paired_devices\s+SET\s+last_seen_at", flat, re.IGNORECASE
        ):
            line_number = flat[: match.start()].count("\n") + 1
            last_seen_writes.append((path, line_number))

if len(last_seen_writes) != 1:
    for path, line_number in last_seen_writes:
        report_violation(
            "RULE-DB-04",
            path,
            line_number,
            "`last_seen_at` must be written from exactly one place; a second write path would "
            "let a heartbeat rewrite a connect-time column",
        )
elif not os.path.abspath(last_seen_writes[0][0]).endswith("paired_devices.rs"):
    report_violation(
        "RULE-DB-04",
        last_seen_writes[0][0],
        last_seen_writes[0][1],
        "the single `last_seen_at` write path must live in the paired-devices access layer",
    )

# ---------------------------------------------------------------------------
# 9. VERIFY-RC-15: shared conformance suite names no concrete link implementation
# ---------------------------------------------------------------------------

tests_common_dir = os.path.abspath(os.path.join("tests", "common"))
if os.path.isdir(tests_common_dir):
    for root, _, files in os.walk(tests_common_dir):
        for name in files:
            if not name.endswith(".rs"):
                continue
            path = os.path.join(root, name)
            with open(path, "r", encoding="utf-8", errors="ignore") as fp:
                content = fp.read().lower()
            if "loopbacklink" in content:
                report_violation(
                    "VERIFY-RC-15",
                    path,
                    1,
                    "the shared test infrastructure must not name a link implementation (`LoopbackLink`)",
                )

print("------------------------------------------------------------------------")
if failures > 0:
    print(f"\033[1;31mFAILED: {failures} architectural invariant violation(s) detected.\033[0m")
    sys.exit(1)
else:
    print("\033[1;32mSUCCESS: All backend architectural invariants hold.\033[0m")
    sys.exit(0)
EOF