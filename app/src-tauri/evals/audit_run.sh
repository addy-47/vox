#!/usr/bin/env bash
# ==============================================================================
# evals/audit_run.sh — Master Synthesis & Empirical Claim Verification Subagent
# ==============================================================================
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <run_id_or_path>"
    echo "Example: $0 20260930_152000_a1b2c3d4"
    exit 1
fi

TARGET="$1"
if [ -d "$TARGET" ]; then
    RUN_DIR="$TARGET"
else
    RUN_DIR="evals/results/memory_eval/$TARGET"
fi

if [ ! -d "$RUN_DIR" ]; then
    echo "Error: Directory '$RUN_DIR' does not exist."
    exit 1
fi

DB_FILE="$RUN_DIR/eval_vox.db"
if [ ! -f "$DB_FILE" ]; then
    echo "Warning: Database '$DB_FILE' not found in run directory."
fi

SESSION_ID="${2:-ses_f0a042890ffewjzCFWe6kq8h27}"

echo "================================================================================"
echo "Launching OpenCode QA Auditor Subagent for Master Synthesis"
echo "Target Run Dir : $RUN_DIR"
echo "Session ID     : $SESSION_ID"
echo "Model          : opencode/space-bunny-free (--variant max)"
echo "================================================================================"

opencode run \
  "You are the Vox Senior QA/Test Auditor. Read .agents/rules/qa-engineer.md first.
Your task:
1. Audit all per-case reports in $RUN_DIR/ (case_01/, case_02/, case_03/, ...).
2. For every claim made by the Compaction, Ingestion, and Consolidation judges:
   - Verify the claim against the real database at $DB_FILE using the tursodb CLI (e.g. 'tursodb $DB_FILE \"SELECT ...\"').
   - Cross-check against the runtime artifacts in case_*/raw_llm_traces.json (request parameters, conversation history, verbatim model outputs).
3. Check for false positives and false negatives in deduplication, and verify whether any dropped observation truly represents semantic loss.
4. Author a rigorous, evidence-based Master Evaluation Report saved to $RUN_DIR/master_synthesis_report.md covering cross-case evolution, causal failure breakdowns, threshold sensitivity, and actionable recommendations." \
  -s "$SESSION_ID" \
  -m opencode/space-bunny-free \
  --variant max \
  --format json \
  --auto
