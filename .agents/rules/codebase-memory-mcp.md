---
trigger: manual
description: Graph-first exploration, symbol tracing, Cypher queries, and evidence verification using codebase-memory MCP. Read before repository exploration.
---

# Codebase Memory MCP Rules

Authoritative guide for graph-first exploration, symbol tracing, and architecture mapping in Vox.

---

## 1. When to Use Graph vs. Text Search

- **Use the graph for structural questions:**
  - Finding symbols implementing a feature or interface.
  - Tracing callers, callees, or cross-service data flow.
  - Analyzing impact radius, fan-in/fan-out, or dead-code candidates.
- **Use direct text/file search for:**
  - Exact string literals, SQL queries, regex patterns, or configuration values.
  - Files intentionally excluded from the index (tests, generated code).
  - Inspecting precise implementation logic after locating target symbols.

---

## 2. Exploration Workflows

### 2.1 Initialization
Before querying the graph, determine active index metadata:
1. `list_projects` → obtain active project ID.
2. `index_status` → confirm generation and status.
3. `get_graph_schema` → check available node labels and edge properties.

### 2.2 Symbol Lookup Sequence
1. `search_graph` with a symbol name pattern or semantic term.
2. Note the qualified name, file path, and line range.
3. `get_code_snippet` using the exact qualified name.
4. `trace_path` (`inbound` for callers; `outbound` for dependencies).
5. Verify terminal claims with direct source file reads.

### 2.3 Architecture Exploration
Call `get_architecture` with targeted aspects only (never request `all` by default):
- `overview` for top-level system structure.
- `structure` or `file_tree` for directory boundaries.
- `layers` or `dependencies` for subsystem ownership.
- `hotspots` for high fan-in maintenance bottlenecks.

---

## 3. Query Tool Selection

| Investigation Goal | Preferred Tool |
| :--- | :--- |
| Find symbol by name | `search_graph` |
| Find callers / inbound impact | `trace_path(direction="inbound")` |
| Find dependencies / outbound calls | `trace_path(direction="outbound")` |
| Cross-service data movement | `trace_path(mode="cross_service")` |
| Git branch or working tree diff | `detect_changes` |
| Literal string or regex match | `grep_search` or `search_code` |
| Multi-hop relationship aggregation | `query_graph` |
| Index coverage validation | `check_index_coverage` |

---

## 4. Cypher Patterns for `query_graph`

Always apply `LIMIT` clauses to avoid unconstrained result sets:

```cypher
// Find symbol by naming convention
MATCH (f:Function) WHERE f.name =~ '.*Handler.*' RETURN f.name, f.file_path LIMIT 50;

// Trace direct callers
MATCH (a)-[r:CALLS]->(b) WHERE b.name = 'handle_ptt_event' RETURN a.name, r.line LIMIT 50;

// Cross-service invocation edges
MATCH (a)-[r:HTTP_CALLS|IPC_CALLS]->(b) RETURN a.name, b.name, r.line LIMIT 50;
```

---

## 5. Evidence Tiers & Coverage

1. **Scout Tier:** 1–2 graph queries plus targeted source confirmation. Used for initial orientation; never use to claim absence of code.
2. **Verify Tier:** Targeted symbol traces, exact snippet verification, and `check_index_coverage`. Default standard for implementation tasks.
3. **Auditor Tier:** Bidirectional path tracing, full result pagination, and explicit source fallback for unindexed scopes. Mandatory for refactorings and architecture sign-offs.

---

## 6. Completion Checklist

Before making definitive architectural claims based on graph data, verify:
- [ ] Confirmed project name and generation via `index_status`.
- [ ] Validated cited file paths with `check_index_coverage`.
- [ ] Read exact source files for behavioral, concurrency, and memory claims.
- [ ] Stated negative findings as bounded observations, not absolute absence.
