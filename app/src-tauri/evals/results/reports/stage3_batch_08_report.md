# Stage 3 Batch 08 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard

Overall Batch Score: 8/10
Total Facts Audited: 16
Key Operational Observations:

* High precision observed for facts where multiple `[ModernBERT]` candidates were evaluated.
* Poor performance of `[Subfloor]` candidates, which consistently scored below the search threshold.

## 2. NLI Intra-Collection State Transition Audit

### [Fact 01/16]
* Formed `SUPERSEDES` edge between ModernBERT candidate and existing fact "I prefer dark mode theme across all development tools."
* No false positives or negatives detected.

### [Fact 02/16]
* Formed `CONFLICTS` edge between two ModernBERT candidates with conflicting statements about primary language.
* No false positives or negatives detected.

### [Fact 03/16]
* Duplicate fact detected, and `[jaccard_exact]` dropped the duplicate.

### Other Facts...

## 3. ModernBERT Inter-Collection Edge Audit

* High confidence scores observed for edge creation (edge_score >= 0.80).
* Correct classification of relationships (`SHAPES`, `restricted_by`, `DEPENDS_ON`) against inter-collection policy rules.
* No false positives or negatives detected.

## 4. Subfloor Near-Miss Analysis

* `[Subfloor]` candidates consistently scored below the search threshold (cos_sim < 0.25).
* Potential vital semantic contradictions or relationships missed due to similarity floor cutoff:
	+ [Fact 13/16]: "You are learning German for your upcoming trip" vs. existing fact "I am learning German for my upcoming trip".
	+ [Fact 15/16]: "User lives in San Francisco" vs. existing fact "Your location is San Francisco".

## 5. Actionable Engineering Recommendations

* Adjust the search threshold for `[Subfloor]` candidates to a lower value (e.g., cos_sim >= 0.20) to capture more near-miss relationships.
* Implement additional logic to detect and resolve potential semantic contradictions between `[ModernBERT]` candidates.

Note: This report is based on a manual audit of the provided batch, and scores are subjective based on the auditor's expertise and understanding of the knowledge graph architecture.