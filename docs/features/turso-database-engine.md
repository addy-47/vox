# Turso Database Engine — What It Is, Features, and How Vox Uses It

## What Is Turso?

Turso is a **modern embeddable database engine** — a clean-room Rust rewrite of SQLite built from scratch (formerly codenamed "Limbo"). It is maintained by Turso (chiselstrike) and represents the next generation of the libSQL project.

| Layer | Turso Database (this engine) | libSQL |
|-------|------------------------------|--------|
| **Language** | Pure Rust (zero C deps) | SQLite C fork |
| **Async I/O** | Native tokio async | Blocking C calls |
| **Concurrent writes** | MVCC via `BEGIN CONCURRENT` | Single-writer lock |
| **Vector search** | Native types + index | Same vector support |
| **Production** | Beta | Production-ready |
| **Future** | All new development | Maintenance mode |

Vox uses the **`turso` crate v0.7.2** as its primary embedded persistence layer for local memory storage, graph relations, voice management, session tracking, and staging queues (tracking the upcoming `v0.8.0` MVCC stabilization milestone).

---

## 1. Core Features

### 1.1 Multi-Version Concurrency Control (MVCC)
- **`BEGIN CONCURRENT`** — Turso-specific transaction mode for optimistic concurrent multi-threaded writes, unlike SQLite's single-writer lock.
- **No disk lock contention** — readers and writers operate concurrently without blocking the audio hot-path or background compaction.

### 1.2 Native Async I/O
- Direct integration with `tokio` async runtime via `conn.query(...).await` and `conn.execute(...).await`.
- Non-blocking ingestion for background queue processing, vector search, and compaction.

### 1.3 Pure Rust & Zero C Dependencies
- Compiles natively with `cargo` — no external C compiler toolchain required.
- Memory safety via Rust's borrow checker eliminates entire classes of bugs common in C SQLite codebases.

### 1.4 SQLite Compatibility
- SQL dialect, file format, and SQLite C API compatibility (subset).
- Supports standard SQL: `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `JOIN`, aggregation, subqueries.

### 1.5 Postgres Frontend (Experimental)
- Turso can speak the Postgres wire protocol for Postgres-compatible clients. Not currently used in Vox.

---

## 2. Storage & Indexing Features

### 2.1 Native Vector Search (Built-in — No Extensions Required)

**Vector Types:**
| Type | SQL Alias | Storage | Precision |
|------|-----------|---------|-----------|
| `FLOAT64` | `F64_BLOB` | 8D + 1 bytes | IEEE 754 double |
| `FLOAT32` | `F32_BLOB` | 4D bytes | IEEE 754 single |
| `FLOAT16` | `F16_BLOB` | 2D + 1 bytes | Half precision |
| `FLOATB16` | `FB16_BLOB` | 2D + 1 bytes | BFloat16 |
| `FLOAT8` | `F8_BLOB` | D + 14 bytes | 8-bit quantized |
| `FLOAT1BIT` | `F1BIT_BLOB` | ⌈D/8⌉ + 3 bytes | 1-bit binary |

**Vector Functions:**
- `vector32(...)` / `vector64(...)` — Convert JSON array or binary to vector type
- `vector_extract(...)` — Extract vector as text representation
- `vector_distance_cos(a, b)` — Cosine distance (1 - cosine similarity)
- `vector_distance_l2(a, b)` — Euclidean distance

**Vector Index (DiskANN-based):**
```sql
CREATE INDEX idx_name ON table_name (libsql_vector_idx(column));
```
- Approximate nearest neighbor (ANN) search via `vector_top_k(idx_name, q_vector, k)` table-valued function
- Configurable: `metric` (cosine/l2), `max_neighbors`, `search_l`, `insert_l`
- Partial vector indexes with `WHERE` filters supported

### 2.2 Full-Text Search (Tantivy-Powered)
- Built-in FTS index powered by [tantivy](https://github.com/quickwit-oss/tantivy) with BM25 scoring. Not currently used in Vox.

### 2.3 Multi-Process WAL
- `experimental_multiprocess_wal(true)` — enables a `.tshm` sidecar for coordinating WAL access across multiple OS processes.

### 2.4 Standard B-Tree Indexes
- Full support for `CREATE INDEX`, `CREATE UNIQUE INDEX`, partial indexes, descending indexes.

---

## 3. Data Management Features

### 3.1 Change Data Capture (CDC)
```sql
PRAGMA unstable_capture_data_changes_conn('id');
```
- Records all INSERT, UPDATE, DELETE operations to the `turso_cdc` system table.

### 3.2 Encryption at Rest (AEGIS-256)
```rust
Builder::new_local("encrypted.db")
    .experimental_encryption(true)
    .with_encryption(EncryptionOpts {
        cipher: "aegis256".to_string(),
        hexkey: "<64-char-hex>".to_string(),
    })
```
- Encrypted databases are opaque blobs — cannot be read by standard SQLite tools.

### 3.3 Materialized Views
- Incrementally maintained via `experimental_materialized_views(true)`.
- Live query subscriptions for reactive applications.

### 3.4 Custom Types (STRICT Tables)
- `CREATE TYPE` for user-defined types via `experimental_custom_types(true)`.

### 3.5 Enhanced Vacuum
- `experimental_vacuum(true)` for better space reclamation.

### 3.6 Attach Database
- `experimental_attach(true)` for `ATTACH DATABASE` support across multiple DB files.

### 3.7 Aggregate Functions
- Standard: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, `GROUP_CONCAT`
- Extension: `stddev()` — standard deviation aggregate

---

## 4. Cloud Sync (Optional `sync` Feature)

Bidirectional synchronization with Turso Cloud via the `sync` Cargo feature.

```rust
use turso::sync::Builder;

let db = Builder::new_remote("local.db")
    .with_remote_url("libsql://your-database.turso.io")
    .with_auth_token("your-token")
    .bootstrap_if_empty(true)
    .build()
    .await?;

db.push().await?;  // Push local changes to cloud
db.pull().await?;  // Pull remote changes
```

**Architecture**: Local-first — reads are always local and sub-millisecond. Writes are batched and pushed. Encryption via AES-256-GCM for data in transit.

---

## 5. Engine Configuration & PRAGMA Options

```rust
let db = Builder::new_local("path/to/db.db")
    .experimental_index_method(true)        // Custom index methods (enabled)
    .experimental_encryption(true)          // Encryption at rest
    .experimental_materialized_views(true)  // Materialized views
    .experimental_custom_types(true)        // Custom types for STRICT tables
    .experimental_vacuum(true)              // Enhanced VACUUM
    .experimental_attach(true)              // ATTACH DATABASE
    .experimental_multiprocess_wal(true)    // Multi-process WAL
    .build()
    .await?;
```

| PRAGMA | Purpose |
|--------|---------|
| `journal_mode = WAL` | Write-Ahead Logging for concurrent reads |
| `busy_timeout = 5000` | Busy wait timeout in ms |
| `foreign_keys = ON` | Enforce foreign key constraints |
| `unstable_capture_data_changes_conn` | Enable CDC |

---

## 6. How Vox Currently Uses Turso

### 6.1 What Vox Uses Today

| Feature | Status | Location |
|---------|--------|----------|
| `Builder::new_local()` | ✅ Local database connection | `persistence/db.rs` |
| `db.connect()` | ✅ Single connection per worker | `persistence/db.rs` |
| `conn.query()` / `conn.execute()` | ✅ Primary query interface | All persistence/ipc files |
| `experimental_index_method(true)` | ✅ Required for `F32_BLOB` vector columns | `persistence/db.rs` |
| `PRAGMA journal_mode = WAL` | ✅ Concurrent read performance | `persistence/db.rs` |
| `PRAGMA busy_timeout = 5000` | ✅ Busy wait timeout | `persistence/db.rs` |
| `PRAGMA foreign_keys = ON` | ✅ Referential integrity | `persistence/db.rs` |
| Manual `BEGIN/COMMIT/ROLLBACK` | ✅ For atomic multi-table writes | `persistence/repository.rs` |
| `F32_BLOB(1024)` vector storage | ✅ BGE-M3 1024-dim embeddings | `persistence/schema.rs` |
| **`vector_distance_cos()` pushdown** | ✅ Cosine similarity in SQL — eliminates Rust-side O(n) vector decode loops | `persistence/queries.rs` |
| Schema migrations in Rust code | ✅ `run_migrations()` | `persistence/schema.rs` |

### 6.2 What Vox Is NOT Using (Gaps)

| Feature | Benefit | Priority | Why Deferred / Not Yet |
|---------|---------|----------|----------------------|
| **`BEGIN CONCURRENT`** (MVCC writes) | True concurrent multi-threaded writes instead of single-writer serialization | **HIGH** | Not yet adopted — requires confidence in MVCC correctness for the persistence worker |
| **Vector Index** (`CREATE INDEX ... libsql_vector_idx`) | Approximate nearest neighbor search (DiskANN) — sub-millisecond instead of `O(n)` scan | **HIGH** | `libsql_vector_idx` module not registered in Turso v0.7.1 runtime. Current `vector_distance_cos()` SQL pushdown eliminates Rust O(n) decode loops. Vector index will be adopted when available. |
| **Cloud Sync** (`sync` feature, `push/pull`) | Offline-first with cloud backup; database-per-user pattern | **MEDIUM** | Requires `turso --features sync` and Turso Cloud credentials — post-MVP |
| **Encryption at rest** | AEGIS-256 local database encryption | **MEDIUM** | Requires key management strategy — post-MVP |
| **Change Data Capture (CDC)** | Real-time change streaming for reactive UI | **MEDIUM** | Post-MVP for live memory view updates |
| **Full-Text Search (tantivy)** | Native BM25 FTS instead of `LIKE '%query%'` | **LOW** | Not yet needed |
| **Multi-process WAL** | Multiple processes sharing the same DB file | **LOW** | Vox is single-process |
| **Materialized Views** | Pre-computed query results | **LOW** | Not yet needed |
| **Encrypted Sync** | E2E encryption for cloud synced data | **LOW** | Depends on cloud sync adoption |

### 6.3 Assessment Summary

**Overall Utilization Score: ~35/100** — Up from ~25/100 after adopting `vector_distance_cos()` SQL pushdown in the memory pipeline.

**What's been won:**
- ✅ Native `vector_distance_cos()` SQL pushdown replaced manual Rust-side cosine similarity — eliminates `O(n)` vector decode loops from application memory (`queries.rs`).

**Remaining high-ROI opportunities:**
1. **`BEGIN CONCURRENT`** — Would unlock true concurrent writes, removing the single-writer bottleneck during peak memory pipeline processing.
2. **Vector Index (DiskANN)** — Would reduce ANN search from `O(n)` scan to sub-milliseconds. Blocked on Turso v0.7.1 runtime not registering `libsql_vector_idx` module.
3. **Cloud Sync** — Database-per-user architecture with offline-first backup and cross-device sync.
4. **Encryption at rest** — AEGIS-256 for sensitive memory data (Identity, Preferences, Goals).

**Important**: The deficit is in **utilization, not selection**. The `turso` crate is the correct engine choice — the gaps are features not yet turned on.

---

## 7. Migration Path (Adopted & Planned)

### Phase A ✅ (Adopted)
1. ✅ **`vector_distance_cos()` in retrieval queries** — `queries.rs` uses SQL-level cosine distance for seed fetch, intra-collection, and inter-collection candidate search. Eliminates Rust-side `O(n)` decode loops.

### Phase B (Next — After Gate 1)
2. **`BEGIN CONCURRENT` for persistence worker** — Replace `BEGIN TRANSACTION` with `BEGIN CONCURRENT` in `repository.rs` for concurrent write throughput.
3. **Vector Index** — When Turso runtime supports `libsql_vector_idx`, adopt `vector_top_k()` for ANN search.

### Phase C (Post-MVP)
4. **Encryption at rest** — Enable `experimental_encryption(true)` with derived key from device identity.
5. **Cloud Sync** — Add `turso --features sync`, configure `Builder::new_remote()`, implement `push/pull` lifecycle.
6. **CDC for reactive UI** — Stream changes to frontend via Tauri events for real-time memory view updates.

---

## 8. References

- **Turso Database GitHub**: https://github.com/tursodatabase/turso
- **Turso Rust SDK Reference**: https://docs.turso.tech/sdk/rust/reference
- **crates.io/turso**: https://crates.io/crates/turso
- **Vox v6 Memory Spec**: `docs/plans/v6-memory-architecture-spec.md`
- **Vox Memory Architecture**: `docs/features/memory-architecture.md`
- **Current Implementation**: `app/src-tauri/src/persistence/`
