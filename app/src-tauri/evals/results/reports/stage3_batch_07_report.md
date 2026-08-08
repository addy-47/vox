# Stage 3 Batch 07 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard
Overall Batch Score: 8/10
Total Facts Audited: 16
Key Operational Observations:

* The pipeline performed well on most facts, with only a few instances of false positives and near-misses.
* The similarity threshold for subfloor candidates seems to be too aggressive, missing some important relationships.

## 2. NLI Intra-Collection State Transition Audit

### Fact 11/16: `[Profile] I prefer using VS Code with Vim keybindings.`

The pipeline correctly superseded an existing fact with a duplicate candidate (`[jaccard_exact]`), indicating an exact Jaccard match (cos_sim = 1.000).

### Fact 14/16: `[Profile] You are learning German for your upcoming trip to Berlin.`

The pipeline failed to supersede the correct fact, instead suggesting a near-miss candidate with lower similarity (`[ModernBERT]` candidates). This suggests that the pipeline may have missed an important relationship.

## 3. ModernBERT Inter-Collection Edge Audit

### Fact 13/16: `[Profile] Primary language: Rust, secondary language: TypeScript.`

The pipeline created several cross-collection graph relationships, but only one had a high confidence score (edge_score = 0.991). The other candidates had lower similarity and were correctly rejected.

## 4. Subfloor Near-Miss Analysis

### Fact 12/16: `[Profile] I am learning German for my upcoming trip.`

The pipeline missed an important relationship between the fact and a near-miss candidate (`[ModernBERT]` candidate). The similarity threshold seems too aggressive, missing some vital semantic contradictions or relationships.

## 5. Actionable Engineering Recommendations

* Adjust the similarity threshold for subfloor candidates to capture more important relationships.
* Fine-tune the pipeline's logic to better handle duplicate facts and near-misses.
* Consider implementing a more sophisticated method for detecting exact Jaccard matches, such as using a library like `jaccard` in Python.

By addressing these issues, we can improve the overall performance of the pipeline and ensure that it accurately represents the knowledge graph.