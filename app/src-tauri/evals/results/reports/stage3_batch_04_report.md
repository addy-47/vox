# Stage 3 Batch 04 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard
- Overall Batch Score: 8/10
- Total Facts Audited: 16
- Key Operational Observations:
  - High precision observed in intra-collection state transitions.
  - Moderate issues with inter-collection edge classification.

## 2. NLI Intra-Collection State Transition Audit

### False Positives:

* Fact 05/16: New directive "Refactor settings store state updates to reduce re-renders" superseded an existing fact, despite the pipeline mistakenly treating it as a duplicate.
* Fact 11/16: Reminder to prepare release notes for v0.9.0 tomorrow was incorrectly marked as a duplicate.

### False Negatives:

* None identified in this batch.

### Supports Edges:
* All supports edges examined had entailment scores above the threshold of 0.85.

## 3. ModernBERT Inter-Collection Edge Audit

### Cross-Collection Relationship Analysis:
* Fact 02/16: Directive added "Prepare release notes for v0.9.0 tomorrow" was classified as a `DEPENDS_ON` relation with an edge score below the confidence threshold of 0.80.
* Fact 07/16: Directive added "Prepare release notes for v0.9.0 tomorrow" was incorrectly marked as a duplicate, indicating potential issues in cross-collection graph relationships.

## 4. Subfloor Near-Miss Analysis

### Candidates with cos_sim between 0.25 and 0.40:
* Fact 06/16: Update directive "Sarah's IPC review meeting is moved to 4 PM PST" had a near-miss candidate pair with a similarity of 0.35.
* Fact 13/16: Schedule a sync with Sarah at 2 PM PST to review IPC commands had a near-miss candidate pair with a similarity of 0.28.

## 5. Actionable Engineering Recommendations

* Adjust the threshold for `SUPERSEDES` edges in intra-collection state transitions to account for context-dependent updates.
* Refine inter-collection edge classification logic to improve accuracy, particularly for `DEPENDS_ON` relations.
* Consider implementing a more nuanced near-miss analysis approach to capture subtle semantic contradictions.