# Stage 3 Batch 02 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard

Overall Batch Score: 8/10
Total Facts Audited: 16
Key Operational Observations:
- High-quality candidate evaluations with accurate intra-collection state transitions.
- Most inter-collection edges were correctly classified, but some required adjustment for confidence scores.
- Subfloor near-miss analysis revealed missed relationships that could be addressed by adjusting the similarity threshold.

## 2. NLI Intra-Collection State Transition Audit

The DeBERTa-v3 state transition evaluation performed well in this batch, with accurate `SUPERSEDES` and `CONFLICTS` edges formed for most target facts. However, there were some instances of false positives and negatives that require attention:

* [Fact 1/16] "Let me know the status of our Tauri v2 app build." was incorrectly superseded by a similar fact from earlier in the batch.
* [Fact 5/16] "Update directive: Sarah's IPC review meeting is moved to 4 PM PST" failed to conflict with an existing fact, potentially leading to outdated information.

## 3. ModernBERT Inter-Collection Edge Audit

The cross-collection graph relationships were mostly correctly classified, but some adjustments were needed for confidence scores:

* [Fact 10/16] "Scheduled: IPC command review with Sarah at 2 PM PST" had a low confidence score (0.70) and should be re-evaluated.
* [Fact 13/16] "Remind me to feed my cat, Luna, at 6 PM" correctly classified as `DEPENDS_ON` but had a lower-than-expected confidence score (0.85).

## 4. Subfloor Near-Miss Analysis

The subfloor near-miss analysis revealed some missed relationships that could be addressed by adjusting the similarity threshold:

* [Fact 8/16] "Recorded entity: PostgreSQL backend database service" and [Fact 9/16] "Recorded relationship: Sarah is the lead frontend engineer" had a high cosine similarity (0.34) but were not connected in the graph.

## 5. Actionable Engineering Recommendations

Based on this batch's evidence, we recommend adjusting the following:

* Adjust the DeBERTa-v3 threshold for state transitions to reduce false positives and negatives.
* Fine-tune confidence scores for ModernBERT inter-collection edges.
* Re-evaluate the similarity threshold in Subfloor near-miss analysis to capture missed relationships.