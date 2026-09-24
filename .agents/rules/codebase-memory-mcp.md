# Codebase Memory MCP Rules

This file defines the `exploration` hook for Vox. Read it before exploring the repository, looking up architecture, tracing relationships, or performing broad code searches.

## Purpose

Use the codebase-memory graph to answer structural questions efficiently and with bounded, verifiable evidence. The graph is a fast index over symbols and relationships; it is not a source-code replacement and its results are best effort.

Use the graph for questions such as:

- What symbols implement a feature?
- Who calls a function?
- What does a function call?
- How does data move between services?
- Which files are affected by a change?
- What are the entry points, boundaries, layers, or clusters?
- Which symbols have high fan-in or fan-out?
- Are there apparent dead-code candidates?

Use ordinary file search and reads for:

- Literal strings, configuration values, SQL, commands, and comments.
- Files or ranges reported as missed, stale, partial, or excluded from indexing.
- Exact implementation details after a graph result identifies the location.
- Tests, generated assets, and files intentionally excluded from the graph.

## Required first calls

Before structural exploration, identify the indexed project:

1. `list_projects`
2. `index_status` for the returned project
3. `get_graph_schema` when writing Cypher or when node/edge properties are unclear
4. `get_architecture` when the task needs repository-level orientation

Do not assume the project name, graph generation, coverage, or freshness. A repository can be indexed while individual files are stale, partially parsed, excluded, or missing from the graph.

## Recommended symbol workflow

Use this sequence for a targeted code question:

1. `search_graph` with a name pattern, semantic query, or bounded file pattern.
2. Inspect the returned qualified name, file path, line range, label, and degree.
3. `get_code_snippet` using the exact qualified name returned by the graph.
4. `trace_path` when callers, callees, data flow, or cross-service behavior matters.
5. `detect_changes` when the question concerns a local or branch diff.

Do not call `get_code_snippet` with a guessed qualified name. If the symbol is not found, broaden `search_graph` first and use the exact result.

## Recommended tracing workflow

For a known function:

1. Search for the exact function or a narrow name pattern.
2. Trace `inbound` for callers and impact.
3. Trace `outbound` for dependencies and behavior.
4. Trace `both` when the full lifecycle is required.
5. Use `data_flow` when argument-aware flow is required.
6. Use `cross_service` when service boundaries matter.
7. Add `risk_labels=true` for production or concurrency-sensitive changes.
8. Read exact source for terminal claims and edge cases.

Use the smallest direction and depth that answers the question. Start with `depth=2` or `depth=3`; increase depth only when the result requires it. Keep tests excluded unless tests are part of the question.

## Recommended architecture workflow

Use `get_architecture` with only the required aspects:

- `overview` for a compact repository summary.
- `structure` and `file_tree` for bounded layout discovery.
- `dependencies`, `packages`, or `layers` for module ownership.
- `entry_points` or `routes` for startup and request flow.
- `boundaries`, `clusters`, or `cycles` for architectural analysis.
- `hotspots` for fan-in and likely maintenance pressure.

Do not request `all` by default. It increases context and is rarely needed for a focused task.

## Query selection

Choose the narrowest tool for the question:

| Question | Preferred tool |
| --- | --- |
| Find a symbol by name | `search_graph` |
| Search a literal or SQL string | `search_code` or repository text search |
| Read exact symbol source | `get_code_snippet` |
| Find callers | `trace_path(direction="inbound")` |
| Find callees | `trace_path(direction="outbound")` |
| Inspect a Git diff | `detect_changes` |
| Aggregate, filter, or traverse multiple hops | `query_graph` |
| Repository overview | `get_architecture` |
| Index freshness and readiness | `index_status` |
| Exact path coverage | `check_index_coverage` |
| Multi-hop service analysis | `query_graph` or `trace_path(mode="cross_service")` |

`search_graph` is for symbols and indexed structure. `search_code` is for text and literals. Do not use one as a substitute for the other.

## Efficient Cypher

Use `query_graph` for questions that cannot be answered by a single graph lookup or trace:

- Count nodes or relationships by label.
- Find high-degree symbols or cross-boundary edges.
- Find symbols with multiple inbound and outbound relationships.
- Inspect edges filtered by relationship type.
- Aggregate results across files, packages, or services.

Always add a `LIMIT` unless the result is intentionally bounded by the tool budget. Prefer specific properties and paths over unconstrained `MATCH` clauses. Use `search_graph` pagination for large result sets instead of replacing it with an unbounded Cypher query.

Useful patterns:

```cypher
MATCH (f:Function) WHERE f.name =~ '.*Handler.*' RETURN f.name, f.file_path LIMIT 50
```

```cypher
MATCH (a)-[r:CALLS]->(b) WHERE a.name = 'main' RETURN a.name, b.name, r.line LIMIT 100
```

```cypher
MATCH (a)-[r:HTTP_CALLS]->(b) RETURN a.name, b.name, r.url_path, r.confidence LIMIT 50
```

## Coverage and freshness

After candidate paths are known, call `check_index_coverage` with every cited path. Include relevant scopes for negative, exhaustive, or dead-code claims.

Interpret coverage correctly:

- `no_recorded_issue` means no recorded index problem; it is not proof of complete indexing.
- `metadata_changed` means the index needs a source read and possibly reindexing.
- `known_gaps` requires direct source inspection of the reported scope.
- `missed` paths require source fallback before relying on graph absence.
- `has_more=true` requires continuation with the returned offset or cursor.

Use `index_repository` when the repository changed materially or the index is stale. Choose the narrowest mode that meets the task:

- `fast` for filtered structure without semantics.
- `moderate` for filtered structure plus semantic search.
- `full` for complete symbols, semantics, and relationships.
- `cross-repo-intelligence` only when service linking across repositories is required.

After reindexing, refresh the project generation and repeat `check_index_coverage` for evidence paths.

## Evidence tiers

Use the lightest tier that matches the claim:

- **Scout:** one or two graph lookups plus targeted source verification. Use for positive discovery only. Do not claim absence, dead code, or complete impact.
- **Verify:** task-directed searches, relevant trace directions, exact snippets for material claims, complete pagination, and coverage checks. This is the default for implementation work.
- **Auditor:** current graph generation, complete relevant pagination, both trace directions, broader relationships, exact source fallback, and explicit limitations. Use for reviews, audits, refactors, and high-risk changes.

For every tier:

1. Record the project and index generation.
2. Record queries, paths, ranges, pagination, and coverage gaps.
3. Verify material claims with source reads or text search.
4. State unresolved coverage rather than treating it as clean.

## Subagent handoff

Before delegating graph work, the parent must perform the initial graph lookup and coverage check. Pass the child:

- Project name and generation.
- Scout, Verify, or Auditor tier.
- Bounded paths and scopes.
- Exact qualified names and query results.
- Trace direction, depth, and continuation state.
- Coverage gaps and source fallback already performed.
- Unresolved questions.

A child without codebase-memory access must not claim to have used the graph. Give it the graph evidence and require source reads for every material claim.

## Common mistakes

- Searching the graph with a literal when `search_code` or text search is appropriate.
- Guessing a qualified name instead of searching first.
- Using outbound tracing when the question is about callers.
- Treating a zero-result graph query as proof that code does not exist.
- Ignoring `has_more`, `truncated`, `coverage_note`, or `metadata_changed`.
- Running broad queries without a limit or result budget.
- Re-running the same search because the first result was paginated or stale.
- Treating a clean coverage signal as a complete audit.
- Reading generated, ignored, excluded, or partially parsed files only through the graph.

## Completion checklist

Before relying on codebase-memory results, confirm:

- The correct project and generation were used.
- The query matched the question type.
- All material results were paginated.
- Cited paths passed `check_index_coverage`.
- Reported gaps were resolved with source reads or text search.
- Exact source was checked for behavioral and concurrency claims.
- Negative claims were phrased as bounded findings, not universal absence.
