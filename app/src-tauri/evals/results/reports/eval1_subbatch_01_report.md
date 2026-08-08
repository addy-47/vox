# Eval 1 Sub-Batch 01 Compaction Audit Report (Turns 1-90)

## Fact Quality & Self-Containment Audit (CRITICAL)
- **Low-Quality Extractions:** 13
	+ "User lives in San Francisco." (bare entity, incomplete statement)
	+ "PostgreSQL backend database service is used for the Vox project." (incomplete statement)
	+ "Sarah is the lead frontend engineer on our team." (bare entity, incomplete statement)
	+ "Recorded relationship: Sarah is the lead frontend engineer." (unnecessary wrapper)
	+ "The user's primary language is Rust." (bare entity, incomplete statement)
	+ "The user's secondary language is TypeScript." (bare entity, incomplete statement)
	+ "The user prefers dark mode theme across all development tools." (soft preference, misplaced)
	+ "User has severe shellfish allergy to shrimp and lobster." (incomplete statement)
	+ "Safety Constraint saved: Severe shellfish allergy to shrimp and lobster." (unnecessary wrapper)
	+ "PostgreSQL backend database service is being used by the user." (incomplete statement)
	+ "Sarah is the lead frontend engineer on the team and will review IPC commands at 4 PM PST." (incomplete statement)
	+ "The conversation involved discussing various tasks, meetings, and preferences related to a project called Vox..." (narrative summary, not self-contained declarative statement)
	+ "The user is working on the Vox project, a realtime voice AI desktop app using Tauri v2 and Rust." (incomplete statement)

## Information Coverage & Detail Density Audit
- **Silent Drops:** 5
	+ User's favorite coffee shop or restaurant in San Francisco.
	+ Exact timeline for Berlin trip or Rust conference schedule.
	+ Specific details about the Vox project's architecture or technical stack.
	+ User's preferred editor or IDE settings (beyond VS Code with Vim keybindings).
	+ Sarah's contact information or meeting notes.

## Local Redundancy & Over-Extraction Audit
- **Duplicate Extracted Facts:** 7
	+ "User lives in San Francisco." and "The user lives in San Francisco."
	+ "PostgreSQL backend database service is used for the Vox project." and "PostgreSQL backend database service is being used by the user."
	+ "Sarah is the lead frontend engineer on our team." and "Recorded relationship: Sarah is the lead frontend engineer."
	+ "The user's primary language is Rust." and "User's primary language is Rust"
	+ "The user prefers dark mode theme across all development tools." (soft preference, misplaced)
	+ "Safety Constraint saved: Severe shellfish allergy to shrimp and lobster." and "Severe shellfish allergy to shrimp and lobster."
	+ "Added task: Benchmark IPC payload serialization speed." and "Benchmark IPC payload serialization speed."

## Collection Disambiguation & Schema Placement
- **Misclassified Facts:** 3
	+ User's preferred editor or IDE settings (beyond VS Code with Vim keybindings) -> Profile.
	+ Sarah's contact information or meeting notes -> Entities.
	+ Soft preferences like "dark mode theme" -> Constraints.

## Precision & Hallucination Check
- **False, Unstated, or Hallucinated Statements:** 2
	+ "The conversation involved discussing various tasks, meetings, and preferences related to a project called Vox..." (narrative summary, not self-contained declarative statement).
	+ "Sarah is the lead frontend engineer on the team and will review IPC commands at 4 PM PST." (incomplete statement).