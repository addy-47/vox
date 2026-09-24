# Vox Compaction + Memory Pipeline Eval Sessions

_Last Updated: 2026-09-24_

## 1. Purpose

Deterministic evaluation sessions for the production compaction, memory-ingestion, and personal-memory-consolidation seams.

The eval uses one shared database. Cases are processed as normal sessions in sequence with at least one simulated day between cases and different times of day. Personal memory, episodic state, and consolidation state must carry forward between cases.

No LLM judge is used. Evaluation measures database population, state continuity, compaction behavior, ingestion behavior, consolidation behavior, and failures through production seams only.

## 2. Source Corpus

The source corpus contains 5,200 conversation turns. Every source turn is used exactly once in the generated cases.

| Source dataset | Turns | Current location |
|---|---:|---|
| `100-turns/dataset_session-2.json` | 100 | `sandbox/datasets/legacy/dataset_session-2.json` |
| `100-turns/dataset_session-3.json` | 100 | `sandbox/datasets/legacy/dataset_session-3.json` |
| `dataset_session1.json` | 1,000 | `sandbox/datasets/legacy/dataset_session1.json` |
| `dataset_session2.json` | 1,000 | `sandbox/datasets/legacy/dataset_session2.json` |
| `dataset_session3.json` | 1,000 | `sandbox/datasets/legacy/dataset_session3.json` |
| `dataset_session4.json` | 1,000 | `sandbox/datasets/legacy/dataset_session4.json` |
| `dataset_session5.json` | 1,000 | `sandbox/datasets/legacy/dataset_session5.json` |
| **Total** | **5,200** | |

The original source JSON files were moved to `sandbox/datasets/legacy/` after generation. The generated cases are in `sandbox/datasets/eval-sessions/`.

## 3. Case Matrix

| Case | Turns | Intended class | Source allocation |
|---|---:|---|---|
| `case_01_under_threshold_035_turns.json` | 35 | Under threshold | `dataset_session-2`: 1–35 |
| `case_02_under_threshold_075_turns.json` | 75 | Under threshold | `dataset_session-2`: 36–100; `dataset_session-3`: 1–10 |
| `case_03_under_threshold_090_turns.json` | 90 | Under threshold | `dataset_session-3`: 11–100 |
| `case_04_one_crossing_180_turns.json` | 180 | One crossing | `dataset_session1`: 1–180 |
| `case_05_one_crossing_200_turns.json` | 200 | One crossing | `dataset_session1`: 181–380 |
| `case_06_one_crossing_220_turns.json` | 220 | One crossing | `dataset_session1`: 381–600 |
| `case_07_two_crossings_350_turns.json` | 350 | Two crossings | `dataset_session1`: 601–950 |
| `case_08_two_crossings_450_turns.json` | 450 | Two crossings | `dataset_session1`: 951–1000; `dataset_session2`: 1–400 |
| `case_09_three_crossings_500_turns.json` | 500 | Three crossings | `dataset_session2`: 401–900 |
| `case_10_three_crossings_550_turns.json` | 550 | Three crossings | `dataset_session2`: 901–1000; `dataset_session3`: 1–450 |
| `case_11_three_crossings_600_turns.json` | 600 | Three crossings | `dataset_session3`: 451–1000; `dataset_session4`: 1–50 |
| `case_12_three_crossings_620_turns.json` | 620 | Three crossings | `dataset_session4`: 51–670 |
| `case_13_three_crossings_650_turns.json` | 650 | Three crossings | `dataset_session4`: 671–1000; `dataset_session5`: 1–320 |
| `case_14_three_crossings_680_turns.json` | 680 | Three crossings | `dataset_session5`: 321–1000 |
| **Total** | **5,200** | **14 cases** | |

The intended crossing classes are workload targets. The production eval must record actual compaction counts and threshold crossings because compaction output changes the post-compaction token footprint.

## 4. Execution Contract

- One shared database stores every case and all resulting state.
- Cases run sequentially as normal sessions.
- Each case starts at a different simulated time and is separated from the previous case by at least one day.
- Existing personal memory and consolidation state must not be reset between cases.
- Each case runs production budgeting, compaction, memory ingestion, and personal-memory consolidation in order.
- Evaluation uses production constructors, boundaries, and functions.
- No mocks or LLM judge are permitted.
- The run must report missing records, failed stages, unexpected state resets, threshold mismatches, and invalid cross-case memory continuity.

## 5. Dataset Schema

Each generated file is a JSON array of conversation objects:

```json
{
  "turn": 1,
  "user": "...",
  "assistant": "..."
}
```

Generated `turn` values are renumbered from 1 within each case. Source boundaries are documented in the case allocation table above.
