# Stage 3 Batch 06 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard
- Overall Batch Score: 7/10
- Total Facts Audited: 16
- Key Operational Observations:
  * The batch exhibited a high level of precision in state transitions, with most `SUPERSEDES` and `CONFLICTS` edges correctly formed.
  * However, there were instances of false positives and false negatives in state resolutions.
  * Inter-collection graph relationships showed moderate accuracy, with some near-miss candidates.

## 2. NLI Intra-Collection State Transition Audit
### False Positives
* Fact 04/16: The pipeline mistakenly superseded an existing fact when the incoming fact was merely an additive update.
  `[ModernBERT] [Entities] Recorded relationship: Sarah is the lead frontend engineer.` (cos_sim: 0.928) should not have superseded `[jaccard_exact] [Entities] Sarah is the lead frontend engineer on our team.` (cos_sim: 1.000)
* Fact 11/16: The pipeline failed to conflict an existing fact when the new fact clearly contradicted it.
  `[ModernBERT] [Entities] Tauri v2 desktop application build` (cos_sim: 0.630) did not supersede or conflict with `[jaccard_exact] [Entities] PostgreSQL backend database service` (cos_sim: 1.000)

### False Negatives
* Fact 09/16: The pipeline failed to supersede an existing fact when the new fact clearly updated it.
  `[ModernBERT] [Entities] Recorded entity: PostgreSQL backend database service.` (cos_sim: 0.900) should have superseded `[jaccard_exact] [Entities] PostgreSQL backend database service` (cos_sim: 1.000)

## 3. ModernBERT Inter-Collection Edge Audit
* Fact 14/16: The pipeline incorrectly classified the relation between `[ModernBERT] [Entities] Sarah is the lead frontend engineer on our team.` and `[jaccard_exact] [Entities] Sarah is the lead frontend engineer on our team.` as a `SUPPORTS` edge, when it should have been a `SUPERSEDES` edge.
* Fact 15/16: The pipeline incorrectly classified the relation between `[ModernBERT] [Entities] PostgreSQL backend database service` and `[jaccard_exact] [Entities] PostgreSQL backend database service` as a `SUPPORTS` edge, when it should have been a `SUPERSEDES` edge.

## 4. Subfloor Near-Miss Analysis
* Fact 08/16: The pipeline missed a vital semantic contradiction between `[ModernBERT] [Entities] Sarah is the lead frontend engineer.` and `[jaccard_exact] [Entities] Sarah is the lead frontend engineer on our team.` due to similarity floor cutoff.
* Fact 12/16: The pipeline failed to detect a near-miss candidate pair in the 0.25-0.40 range between `[Subfloor] [Entities] PostgreSQL backend database service is used for the Vox project.` (cos_sim: 0.285) and `[ModernBERT] [Entities] Recorded entity: PostgreSQL backend database service.` (cos_sim: 0.900)

## 5. Actionable Engineering Recommendations
* Adjust the similarity threshold to improve detection of near-miss candidate pairs.
* Implement additional logic to detect false positives and false negatives in state resolutions.
* Refine inter-collection graph relationship classification to improve accuracy.