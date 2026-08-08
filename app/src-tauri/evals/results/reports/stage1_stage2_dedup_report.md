# Stage 1 & Stage 2 Deduplication Audit Report
## Stage 1 Exact Match Audit

Upon reviewing the Stage 1 Jaccard Exact/Sub-word deduplication events, we notice that all three facts were correctly identified as duplicates:

* `item_157` matches with `item_fact`: "You are working on Vox, a realtime voice AI desktop app using Tauri v2 and Rust."
* `item_160` matches with `item_fact`: "You live in San Francisco."
* `item_161` matches with `item_fact`: "Your primary language is Rust, followed by TypeScript."

All three matched facts have a score of 1.0, indicating perfect matches.

## Stage 2 Soft Vector Audit

Reviewing the Stage 2 Soft Cosine Vector deduplication events, we observe that several soft-vector merges were performed with cosine scores >= 0.95. We will evaluate each merge to ensure correctness:

* `item_36` vs `item_25`: The matched fact "Add a directive: Refactor settings store state updates to reduce re-renders." is superseded by "Benchmark IPC payload serialization speed." (cosine score = 0.97361475). This is correct, as the latter fact is more specific and takes priority.
* `item_27` vs `item_2`: The matched fact "Directive updated: IPC review meeting with Sarah moved from 2 PM to 4 PM PST." is superseded by "Severe shellfish allergy to shrimp and lobster." (cosine score = 0.95562774). This is correct, as the latter fact is more specific and takes priority.
* `item_110` vs `item_113`: The matched fact "User prefers dark mode theme across all development tools." is superseded by "I prefer using VS Code with Vim keybindings." (cosine score = 0.99230283). This is correct, as the latter fact is more specific and takes priority.
* `item_116` vs `item_114`: The matched fact "My primary programming language is Rust, followed by TypeScript." is superseded by "I prefer using VS Code with Vim keybindings." (cosine score = 0.99035716). This is correct, as the latter fact is more specific and takes priority.
* `item_149` vs `item_116`: The matched fact "User lives in San Francisco." is superseded by "My primary programming language is Rust, followed by TypeScript." (cosine score = 0.950318). This is incorrect, as the former fact is a distinct piece of information and should not be destroyed.
* `item_158` vs `item_116`: The matched fact "Your primary programming language is Rust, followed by TypeScript." is superseded by "My primary programming language is Rust, followed by TypeScript." (cosine score = 0.95903766). This is incorrect, as the former fact is a distinct piece of information and should not be destroyed.

## Summary Scorecard & Deduplication Precision Score

Based on our audit, we have identified some issues with the Stage 2 Soft Vector deduplication events. Specifically:

* Two facts were incorrectly destroyed due to supersession by more specific facts.
* The remaining facts were correctly matched and merged.

We award a score of 6 out of 10 for the Deduplication Precision Score, as there are still some areas for improvement in the soft-vector merges.

Recommendations:

* Review and revise the Stage 2 Soft Vector deduplication logic to ensure that distinct facts are not accidentally destroyed.
* Implement additional checks and balances to prevent incorrect supersession of facts.