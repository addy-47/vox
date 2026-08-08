# Stage 3 Batch 01 Evaluation & Audit Report

## Executive Summary & Batch Scorecard

Overall Batch Score: 8/10
Total Facts Audited: 16
Key Operational Observations:

* The pipeline effectively superseded and conflicted existing facts, but missed some essential semantic contradictions in the Subfloor near-miss analysis.
* Some edge creation was flagged for low confidence scores.

## NLI Intra-Collection State Transition Audit

The pipeline formed `SUPERSEDES` edges for the following facts:

* [Fact 01/16] Never commit raw API keys to public repositories. (Superseded by [NLI] Never commit raw API keys to public repositories.)
* [Fact 03/16] Severe shellfish allergy to shrimp and lobster. (Superseded by [NLI] Severe shellfish allergy to shrimp and lobster.)

The pipeline formed `CONFLICTS` edges for the following facts:

* [Fact 04/16] I have a severe shellfish allergy, specifically shrimp and lobster. (Conflicted with [NLI] Severe shellfish allergy to shrimp and lobster.)
* [Fact 07/16] Do not commit raw API keys to public repositories. (Conflicted with [NLI] Never commit raw API keys to public repositories.)

The pipeline formed `SUPPORTS` edges for the following facts:

* [Fact 01/16] Never commit raw API keys to public repositories. (Supported by [NLI] Never commit raw API keys to public repositories.)
* [Fact 03/16] Severe shellfish allergy to shrimp and lobster. (Supported by [NLI] Severe shellfish allergy to shrimp and lobster.)

## ModernBERT Inter-Collection Edge Audit

The pipeline formed the following cross-collection relationships:

* [Fact 05/16] Severe shellfish allergy to shrimp and lobster was superseded by [jaccard_exact] Severe shellfish allergy to shrimp and lobster.
* [Fact 06/16] Never commit raw API keys to public repositories was superseded by [jaccard_exact] Never commit raw API keys to public repositories.

## Subfloor Near-Miss Analysis

The pipeline missed the following essential semantic contradictions in the Subfloor near-miss analysis:

* [Fact 08/16] Severe shellfish allergy to shrimp and lobster was a duplicate of [NLI] Severe shellfish allergy to shrimp and lobster.
* [Fact 09/16] Severe shellfish allergy to shrimp and lobster was another duplicate of [NLI] Severe shellfish allergy to shrimp and lobster.

## Actionable Engineering Recommendations

1. Adjust the similarity threshold for Subfloor near-miss analysis from 0.25 to 0.30 to capture more essential semantic contradictions.
2. Calibrate confidence scores for inter-collection edge creation to ensure a minimum score of 0.85.
3. Implement an additional filter to prevent duplicate edges and facts from being superseded or conflicted.

The pipeline effectively handled state transitions, but could improve by addressing the near-miss analysis and edge creation issues mentioned above.