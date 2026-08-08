# Stage 3 Batch 01 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard

* Overall Batch Score: 7.5/10
* Total Facts Audited: 16
* Key Operational Observations:
	+ The pipeline demonstrated strong performance in identifying `SUPPORTS` relationships, with high-confidence scores and accurate entailment detection.
	+ However, there were instances of false negatives in `SUPERSEDES` and `CONFLICTS` edge formation, indicating potential issues with contradiction detection.
	+ The `Subfloor` analysis revealed some near-miss candidates that may have been missed due to the similarity floor cutoff.

## 2. NLI Intra-Collection State Transition Audit

The NLI audit revealed some areas of concern:

* **False Negative:** Fact 10/16, where the pipeline failed to supersede an existing fact despite a clear contradiction. The incoming fact stated "I am a senior product manager in fintech," but the pipeline did not recognize the contradiction with the existing fact "I am learning Spanish for a trip to Barcelona."
* **False Positive:** Fact 14/16, where the pipeline mistakenly superseded an existing fact when the incoming fact was merely an additive update. The incoming fact stated "I have a severe lactose intolerance constraint," but the pipeline superseded the existing fact "Constraint saved: Severe lactose intolerance" despite the two facts being related but distinct.

## 3. ModernBERT Inter-Collection Edge Audit

The ModernBERT audit revealed some areas for improvement:

* **Confidence Score Calibration:** The pipeline formed several `SHAPES` edges with confidence scores below the 0.80 threshold. While these edges may still be accurate, it is essential to calibrate the confidence scores to ensure high-confidence edge creation.
* **Relation Classification:** The pipeline correctly classified most relations, but there were some instances where the relation classification was incorrect. For example, Fact 13/16, where the pipeline classified the relation as `DEPENDS_ON` instead of `restricted_by`.

## 4. Subfloor Near-Miss Analysis

The Subfloor analysis revealed some near-miss candidates that may have been missed due to the similarity floor cutoff:

* **Near-Miss Candidate:** Fact 5/16, where the pipeline evaluated a candidate with a cos_sim score of 0.378. While this candidate was not selected, it may have been a relevant match if the similarity floor cutoff were adjusted.

## 5. Actionable Engineering Recommendations

Based on the evidence in this batch, the following recommendations are made:

* **Adjust Contradiction Detection Threshold:** The pipeline's contradiction detection threshold may be too high, leading to false negatives. Adjusting this threshold may improve the pipeline's ability to detect contradictions.
* **Calibrate Confidence Scores:** The pipeline's confidence scores may need to be calibrated to ensure high-confidence edge creation. This may involve adjusting the confidence score threshold or retraining the model.
* **Adjust Similarity Floor Cutoff:** The pipeline's similarity floor cutoff may be too high, leading to near-miss candidates being missed. Adjusting this cutoff may improve the pipeline's ability to detect relevant matches.