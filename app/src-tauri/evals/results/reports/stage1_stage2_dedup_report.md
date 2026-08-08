# Stage 1 & Stage 2 Deduplication Audit Report
=====================================================

## Stage 1 Exact Match Audit

The Stage 1 Exact Match Audit involves evaluating the dropped facts to determine if they were genuinely identical or redundant to the matched facts.

### Observations

* The majority of the dropped facts have a score of 1.0, indicating that they are exact matches to the matched facts.
* However, there are some cases where the dropped facts are not exact matches, but rather similar phrases or sentences. For example:
	+ "I have a severe lactose intolerance constraint." and "Severe lactose intolerance" are not exact matches, but they convey the same information.
	+ "Learning Spanish for trip to Barcelona" and "I am learning Spanish for a trip to Barcelona." are not exact matches, but they convey the same information.
* There are also cases where the dropped facts are identical to the matched facts, but they belong to different collections. For example:
	+ "I live in Austin, Texas." is dropped from the "Profile" collection, but it is matched to a fact in the "Identity" collection.

### Conclusion

Based on the observations, it appears that the Stage 1 Exact Match Audit has correctly identified and dropped redundant facts. However, there are some cases where the algorithm may have been overly aggressive in dropping similar phrases or sentences. To improve the accuracy of the audit, it may be necessary to fine-tune the algorithm to better handle similar but not identical phrases.

## Stage 2 Soft Vector Audit

The Stage 2 Soft Vector Audit involves evaluating the soft-vector merges to determine if priority resolution correctly handled incoming vs existing facts.

### Observations

* The Stage 2 Soft Vector Audit is empty, indicating that there are no soft-vector merges to evaluate.
* This suggests that the algorithm has not performed any soft-vector merges, and therefore, there are no potential issues with priority resolution.

### Conclusion

Based on the observations, it appears that the Stage 2 Soft Vector Audit is not applicable in this case, as there are no soft-vector merges to evaluate.

## Summary Scorecard & Deduplication Precision Score

Based on the audit results, I would give the deduplication algorithm a score of 8 out of 10. The algorithm has correctly identified and dropped redundant facts in most cases, but there are some cases where it may have been overly aggressive in dropping similar phrases or sentences. Additionally, the lack of soft-vector merges suggests that the algorithm may not be fully utilizing its capabilities.

To improve the accuracy of the algorithm, I would recommend fine-tuning it to better handle similar but not identical phrases, and exploring ways to incorporate soft-vector merges to improve the handling of incoming vs existing facts.

### Score Breakdown

* Stage 1 Exact Match Audit: 8/10
* Stage 2 Soft Vector Audit: N/A
* Overall Deduplication Precision Score: 8/10