# Stage 3 Batch 02 Evaluation & Audit Report

## 1. Executive Summary & Batch Scorecard

* Overall Batch Score: 8.5/10
* Total Facts Audited: 16
* Key Operational Observations:
	+ The pipeline demonstrated strong performance in identifying supports and superseding relationships.
	+ However, there were instances of false negatives in state transition resolutions, particularly in cases where the incoming fact was an additive update or separate context.
	+ The inter-collection edge audit revealed some inconsistencies in relation classification and confidence score calibration.

## 2. NLI Intra-Collection State Transition Audit

The NLI state transition audit revealed several instances of false negatives, where the pipeline failed to supersede or conflict an existing fact when the new fact clearly contradicted it. For example:

* Fact 03/16: The incoming fact "Constraint saved: No meetings scheduled before 9 AM" should have superseded the existing fact "Do not schedule meetings before 9 AM" (cos_sim: 0.914), but the pipeline only formed a supports edge.
* Fact 08/16: The incoming fact "I am training for a half-marathon in November" should have conflicted with the existing fact "Learning Spanish for trip to Barcelona" (cos_sim: 0.298), but the pipeline did not form a conflicts edge.

On the other hand, the pipeline demonstrated strong performance in identifying supports relationships, with several instances of correct supports edges formed between incoming facts and existing facts.

## 3. ModernBERT Inter-Collection Edge Audit

The inter-collection edge audit revealed some inconsistencies in relation classification and confidence score calibration. For example:

* Fact 06/16: The pipeline formed a shapes edge between the incoming fact "I am using Rust and Python for side automation projects" and the existing fact "Uses Rust and Python for side projects" (cos_sim: 0.811), but the confidence score was only 0.939, which is below the threshold.
* Fact 11/16: The pipeline formed a shapes edge between the incoming fact "Profile recorded: Uses Rust and Python for side projects" and the existing fact "I am using Rust and Python for side automation projects" (cos_sim: 0.733), but the confidence score was only 0.983, which is below the threshold.

## 4. Subfloor Near-Miss Analysis

The subfloor near-miss analysis revealed several instances of vital semantic contradictions or relationships that were missed purely due to similarity floor cutoff. For example:

* Fact 04/16: The incoming fact "I am learning Spanish for a trip to Barcelona" had a cos_sim of 0.298 with the existing fact "I am training for a half-marathon in November", which is below the search threshold. However, the two facts are semantically unrelated, and the pipeline should have formed a conflicts edge.
* Fact 10/16: The incoming fact "Profile recorded: Has a pet cat named Miso" had a cos_sim of 0.344 with the existing fact "Profile recorded: Uses Rust and Python for side projects", which is below the search threshold. However, the two facts are semantically unrelated, and the pipeline should have formed a conflicts edge.

## 5. Actionable Engineering Recommendations

Based on the evidence in this batch, the following concrete logic or threshold adjustments are recommended:

* Adjust the NLI state transition resolution logic to better handle additive updates and separate contexts.
* Calibrate the confidence score threshold for inter-collection edge creation to ensure high-confidence edge creation.
* Adjust the similarity floor cutoff to capture more vital semantic contradictions or relationships.
* Implement a more robust relation classification system to handle inconsistencies in inter-collection edge creation.