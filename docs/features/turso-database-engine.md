# Turso Database Engine — Technical Specification & Feature Reference

## 1. Engine Overview

**Turso** is an in-process, embeddable SQL database engine built as a clean-room Rust rewrite of SQLite (originally codenamed "Limbo"). Maintained by Turso (ChiselStrike), it combines SQLite file-format and SQL-dialect compatibility with a modern async-first, pure-Rust architecture designed for multi-threaded and distributed systems.

| Dimension | Turso Database Engine (`turso` v0.8.0-pre.11) | libSQL (Legacy SQLite C fork) | Standard SQLite (C) |
|---|---|---|---|
| **Language & Toolchain** | Pure Rust (`cargo` build, zero C toolchain) | C (SQLite fork) + Rust bindings | Pure C |
| **Async Architecture** | Native Tokio async I/O (`.await`) | Blocking C calls wrapped in thread pools | Blocking synchronous C calls |
| **Concurrency Model** | Multi-Version Concurrency Control (MVCC) via `BEGIN CONCURRENT` | Single-writer WAL lock | Single-writer WAL / rollback journal |
| **Vector Engine** | Native vector storage types (`F32_BLOB`, etc.) + distance functions | Custom vector extensions / DiskANN in C | None (requires SQLite extension) |
| **Full-Text Search** | Built-in [Tantivy](https://github.com/quickwit-oss/tantivy) index via `USING fts` | FTS5 C extension | FTS5 C extension |
| **Encryption at Rest** | AEGIS-256 local encryption | SQLCipher / custom codec | SQLite Encryption Extension (SEE, commercial) |
| **Cloud Replication** | Native local-first sync (`turso --features sync`) | Native replication protocol | None native |
| **Memory Management** | Rust borrow checker + optional `mimalloc` allocator | C `malloc` / `free` | C `sqlite3_malloc` |

---

## 2. Core Architecture & Lifecycles

### 2.1 Engine Primitives: `Database` vs `Connection`

The engine establishes a strict separation between database management and execution contexts:

```
                  ┌────────────────────────────────────────────────────────┐
                  │                 turso::Database                        │
                  │   - Thread-safe handle (Clone + Send + Sync)           │
                  │   - Shared Page Cache & Buffer Pool                    │
                  │   - Multi-Process WAL Coordinator (.tshm)              │
                  │   - Global Extension & Symbol Registry                 │
                  └──────────────────────────┬─────────────────────────────┘
                                             │
                                   .connect() │ produces independent contexts
                                             │
               ┌─────────────────────────────┼─────────────────────────────┐
               ▼                             ▼                             ▼
   ┌───────────────────────┐   ┌───────────────────────┐   ┌───────────────────────┐
   │   turso::Connection   │   │   turso::Connection   │   │   turso::Connection   │
   │  - Independent Gate   │   │  - Independent Gate   │   │  - Independent Gate   │
   │  - Dangling Tx Guard  │   │  - Dangling Tx Guard  │   │  - Dangling Tx Guard  │
   │  - Statement Cache    │   │  - Statement Cache    │   │  - Statement Cache    │
   └───────────────────────┘   └───────────────────────┘   └───────────────────────┘
```

#### `turso::Database`
- Created once via `turso::Builder::new_local(path).build().await?`.
- Implements `Clone + Send + Sync`. Cloning creates a cheap `Arc` reference to the same underlying storage engine, buffer pool, and lock manager.
- Acts as a **connection factory** via `db.connect() -> Result<Connection>`.
- Coordinates write-ahead logging, background checkpointing, and page cache flush policies across all child connections.

#### `turso::Connection`
- Represents an independent SQL execution context.
- Implements `Send + Sync`. Can be cloned, but clones share the internal `ConnectionOperationGate`.
- **Concurrency Gate (`ConnectionOperationGate`)**:
  - Uses an atomic state machine (`AtomicUsize`) to coordinate operations on the connection.
  - Read queries (`conn.query()`) acquire **shared operation guards** (`acquire_shared()`), enabling multiple concurrent read streams on the same connection.
  - Batches and explicit write transactions acquire an **exclusive operation guard** (`acquire_exclusive()`). Attempting to run concurrent exclusive operations on the same connection returns `Error::Misuse("connection is busy...")`.
- **Dangling Transaction Safety (`AtomicDropBehavior`)**:
  - If a transaction handle is dropped without calling `.commit()` or `.rollback()`, the connection tracks the dangling transaction and safely executes the configured drop action (`DropBehavior::Rollback` by default) on the next access.
- **Statement Caching**: Supports prepared statement caching via `conn.prepare_cached(sql)`.

---

## 3. Concurrency, Transactions & MVCC

### 3.1 Concurrency Modes

Turso supports four transaction behaviors defined by `turso::transaction::TransactionBehavior`:

| Mode | SQL Command | Concurrency Characteristics | Use Case |
|---|---|---|---|
| `Deferred` | `BEGIN DEFERRED` | Transaction does not acquire locks until the database is first accessed. Default behavior. | General read-heavy or single-connection operations |
| `Immediate` | `BEGIN IMMEDIATE` | Reserves write access immediately. Blocks other writers from beginning. | Predictable sequential writes |
| `Exclusive` | `BEGIN EXCLUSIVE` | Prevents other connections from reading or writing while active. | Schema migrations, system reconfigurations |
| `Concurrent` | `BEGIN CONCURRENT` | **MVCC mode**. Multiple writers proceed in parallel without table-level locks. | High-throughput concurrent multi-threaded writes |

### 3.2 Multi-Version Concurrency Control (MVCC)

When `PRAGMA journal_mode = 'mvcc'` is enabled:
1. Multiple connections can begin write transactions concurrently using `conn.execute("BEGIN CONCURRENT", ())` or `conn.transaction_with_behavior(TransactionBehavior::Concurrent)`.
2. Each connection operates against an optimistic snapshot of the database.
3. Write conflicts are detected atomically at `COMMIT` time:
   - If two concurrent transactions modify distinct rows/pages, both commit successfully.
   - If two transactions modify overlapping rows/pages, the first to commit succeeds; the second receives a conflict error and must rollback and retry.

#### Retry Protocol for MVCC Conflicts
```rust
fn is_retryable(e: &turso::Error) -> bool {
    matches!(e, turso::Error::Busy(_) | turso::Error::BusySnapshot(_))
        || matches!(e, turso::Error::Error(msg) if msg.contains("conflict"))
}
```

### 3.3 Phased Exponential Backoff (`busy_timeout`)

Turso provides native busy timeout management via `conn.busy_timeout(duration: Duration)`. Unlike standard SQLite's linear sleep:
- Sleeps in escalating phases (1ms, then 2ms, up to a 100ms cap per phase) until the total accumulated duration is reached.
- Non-blocking to the Tokio async runtime when yields are scheduled.

---

## 4. Query, Statement & Batch APIs

### 4.1 Statement Preparation & Caching
- `conn.query(sql, params).await -> Result<Rows>`: Executes query and returns asynchronous row stream.
- `conn.execute(sql, params).await -> Result<u64>`: Executes modification and returns affected row count.
- `conn.prepare(sql).await -> Result<Statement>`: Prepares a one-off statement.
- `conn.prepare_cached(sql).await -> Result<Statement>`: Prepares and caches the compiled statement within the connection's LRU cache.

### 4.2 Batch Execution Models

| Method | Transactional | Rollback on Error | Returns | Description |
|---|---|---|---|---|
| `conn.execute_batch(sql)` | Non-atomic | No (persists until failure) | `Result<()>` | Raw multi-statement SQL string execution (useful for DDL schemas). |
| `conn.batch(stmts)` | Non-atomic | No (completed statements persist) | `Result<Vec<BatchResult>>` | Parameterized batch. Execution halts at first error; returns `Error::BatchStatementFailed` with statement index. |
| `conn.transactional_batch(stmts, behavior)` | **Atomic** | **Yes (automatic `ROLLBACK`)** | `Result<Vec<BatchResult>>` | All-or-nothing execution wrapped in `BEGIN <behavior> / COMMIT`. Rejects manual transaction controls inside statements. |

### 4.3 Pragma Operations
- `conn.pragma_update(name, value).await -> Result<Vec<Row>>`: Formats and executes `PRAGMA name = value`.
- `conn.pragma_query(name, callback).await -> Result<()>`: Inspects pragma values through a typed row visitor closure.

---

## 5. Storage Engine, Data Types & Vector Search

### 5.1 Native Vector Primitives (Zero External Extensions)

Turso embeds vector types and distance functions directly into the SQL engine:

#### Supported Vector Types
| Type | SQL Dialect Alias | Storage Layout | Precision |
|---|---|---|---|
| `FLOAT64` | `F64_BLOB` | 8 bytes × dimensions + 1 byte header | 64-bit IEEE 754 double |
| `FLOAT32` | `F32_BLOB` | 4 bytes × dimensions | 32-bit IEEE 754 single |
| `FLOAT16` | `F16_BLOB` | 2 bytes × dimensions + 1 byte header | 16-bit half precision |
| `FLOATB16` | `FB16_BLOB` | 2 bytes × dimensions + 1 byte header | 16-bit BFloat16 |
| `FLOAT8` | `F8_BLOB` | 1 byte × dimensions + 14 bytes header | 8-bit quantized float |
| `FLOAT1BIT` | `F1BIT_BLOB` | ⌈dimensions / 8⌉ bytes + 3 bytes header | 1-bit binary quantized |

#### Vector SQL Functions
- `vector32(blob_or_json)`: Casts JSON string array (e.g. `'[0.1, 0.2, ...]'`) or raw binary blob into an `F32_BLOB`.
- `vector64(blob_or_json)`: Casts into an `F64_BLOB`.
- `vector_extract(vector_col)`: Serializes vector column to human-readable JSON string.
- `vector_distance_cos(a, b)`: Computes native cosine distance ($1.0 - \text{cosine\_similarity}$).
- `vector_distance_l2(a, b)`: Computes native Euclidean $L_2$ distance.

#### Exact Vector Retrieval Pattern
```sql
SELECT id, vector_distance_cos(embedding, vector32(?1)) AS distance
FROM memory_facts_vectors
WHERE status = 'active'
ORDER BY distance ASC
LIMIT 10;
```

---

## 6. Full-Text Search (Tantivy Integration)

Turso includes native BM25 full-text search powered by the [Tantivy](https://github.com/quickwit-oss/tantivy) search engine crate, enabled via the `fts` feature.

### 6.1 Index Definition
```sql
CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    title TEXT,
    content TEXT
);

CREATE INDEX idx_docs_fts ON documents USING fts (title, content);
```

### 6.2 Query Syntax
```sql
SELECT id, title
FROM documents
WHERE (title, content) MATCH 'performance AND indexing';
```
- Operates directly via the `MATCH` operator on indexed columns.
- Uses Tantivy's BM25 term weighting and inverted index representation.
- Automatically synchronizes with table row inserts, updates, and deletes.

---

## 7. Engine Configuration & Builder Flags

Turso provides granular control over experimental engine modules via `turso::Builder`:

```rust
let db = turso::Builder::new_local("local.db")
    .read_only(false)
    .experimental_index_method(true)        // Enables USING syntax (FTS, custom indexers)
    .experimental_encryption(true)          // Enables AEGIS-256 database encryption
    .with_encryption(encryption_opts)       // Encryption key configuration
    .experimental_materialized_views(true)  // Live incremental materialized views
    .experimental_custom_types(true)        // STRICT table user-defined types
    .experimental_generated_columns(true)   // Virtual & stored generated columns
    .experimental_without_rowid(true)       // WITHOUT ROWID clustered indexes
    .experimental_vacuum(true)              // Enhanced incremental vacuum engine
    .experimental_attach(true)              // Cross-database ATTACH DATABASE support
    .experimental_multiprocess_wal(true)    // Coordinates WAL across separate OS processes (.tshm)
    .experimental_mvcc_passive_checkpoint(true) // Non-blocking checkpointing in MVCC mode
    .build()
    .await?;
```

### 7.1 Multi-Process WAL (`experimental_multiprocess_wal`)
- Creates a `.tshm` shared-memory sidecar file alongside `.db` and `.db-wal`.
- Coordinates WAL read marks and write transactions across multiple distinct OS processes without corrupting database state.

### 7.2 Encryption at Rest (`experimental_encryption`)
- Uses AEGIS-256 authenticated symmetric encryption.
- Encrypts database pages at the VFS block boundary.
- Database file header is randomized; unreadable by standard SQLite or hex analysis without key.

### 7.3 Change Data Capture (CDC)
```sql
PRAGMA unstable_capture_data_changes_conn('session_token');
```
- When enabled on a connection, writes record operation type, table, row ID, and changes into the `turso_cdc` internal system catalog table for streaming consumers.

---

## 8. Cloud Replication & Sync (`sync` Feature)

When compiled with `turso = { version = "0.8.0-pre.11", features = ["sync"] }`, Turso provides bidirectional synchronization with remote Turso Cloud instances:

```rust
use turso::sync::Builder;

let db = Builder::new_remote("local.db")
    .with_remote_url("libsql://my-org.turso.io")
    .with_auth_token("authToken")
    .bootstrap_if_empty(true)
    .build()
    .await?;

// Bidirectional sync lifecycle
db.pull().await?; // Fetch latest remote changesets
db.push().await?; // Push locally committed changesets
```

- **Architecture**: Local-first embedded replica. Local reads are sub-millisecond local file queries. Writes commit locally and synchronize asynchronously over HTTP/2.

---

## 9. Error Taxonomy & Diagnostics

The `turso::Error` enum exposes structured diagnostic classifications:

| Variant | Description | Actionable Handling |
|---|---|---|
| `Error::Busy(msg)` | Database or table locked by concurrent process | Retry with exponential backoff (`busy_timeout`) |
| `Error::BusySnapshot(msg)` | MVCC snapshot conflict detected at commit time | Rollback current transaction and retry from start |
| `Error::Misuse(msg)` | Connection misused (e.g. concurrent exclusive operations on single handle) | Allocate separate connection via `db.connect()` |
| `Error::Constraint(msg)` | UNIQUE, NOT NULL, or FOREIGN KEY constraint violated | Inspect violated schema constraint |
| `Error::Readonly(msg)` | Modification attempted on read-only database | Verify builder flags or file system permissions |
| `Error::Corrupt(msg)` | Database file or WAL page integrity failure | Initiate backup recovery / integrity check |
| `Error::BatchStatementFailed { index, error, results }` | One statement in a batch failed | Inspect failing statement index and preceding partial results |
| `Error::BatchRollbackFailed { error, rollback_error }` | Transactional batch failed and rollback also encountered an error | Critical connection reset required |
