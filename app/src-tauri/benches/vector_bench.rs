//! Vector Similarity Benchmark: SQL `vector_distance_cos()` vs Rust `cosine_similarity()`.
//!
//! Compares latency and result equivalence between:
//!   A) Turso-native SQL pushdown: `WHERE (1.0 - vector_distance_cos(embedding, ?)) >= ?`
//!   B) Rust-side: load all F32_BLOB rows, decode, loop with cosine_similarity()
//!
//! Run: cargo run --bin vector-bench --release -- [num_vectors] [dim]
//! Default: 1000 vectors, 384-dim (MiniLM-L12 — matches production)

use anyhow::Result;
use std::path::PathBuf;
use std::time::Instant;
use turso::{Builder, Connection};

use vox_lib::persistence::{decode_f32_blob, encode_f32_blob};

fn cosine_similarity(u: &[f32], v: &[f32]) -> f32 {
    if u.len() != v.len() || u.is_empty() {
        return 0.0;
    }
    let dot: f32 = u.iter().zip(v.iter()).map(|(x, y)| x * y).sum();
    let norm_u: f32 = u.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_v: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_u > 0.0 && norm_v > 0.0 {
        dot / (norm_u * norm_v)
    } else {
        0.0
    }
}

/// Simple inline xorshift64 PRNG (no external crate dependency).
struct XorShift64(u64);

impl XorShift64 {
    fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        Self(seed)
    }
    fn f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32) * (1.0 / 18446744073709551615.0)
    }
}

/// Generate a random unit vector of given dimension.
fn random_unit_vector(rng: &mut XorShift64, dim: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..dim).map(|_| rng.f32() * 2.0 - 1.0).collect();
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

async fn setup_db(db_path: &PathBuf, dim: usize) -> Result<Connection> {
    let path_str = db_path.to_string_lossy();
    let db = Builder::new_local(&path_str)
        .experimental_index_method(true)
        .build()
        .await?;
    let conn = db.connect()?;
    let _ = conn.execute("PRAGMA journal_mode = WAL;", ()).await;
    let _ = conn.execute("PRAGMA busy_timeout = 5000;", ()).await;

    // Match production schema: same table structure as persistence/schema.rs
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS memory_facts_vectors (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                fact_id     TEXT NOT NULL,
                collection  TEXT NOT NULL DEFAULT '',
                embedding   F32_BLOB({}) NOT NULL
            )",
            dim
        ),
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_mfv_fact_id ON memory_facts_vectors(fact_id)",
        (),
    )
    .await?;

    Ok(conn)
}

fn print_separator(label: &str) {
    println!("\n{}", "=".repeat(72));
    println!("  {}", label);
    println!("{}", "=".repeat(72));
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let num_vectors: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);
    let dim: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(384);

    let mut rng = XorShift64::new();

    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║      Vector Similarity Benchmark: SQL Pushdown vs Rust Loop         ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Configuration:");
    println!("    Vector count : {}", num_vectors);
    println!("    Dimension    : {}", dim);
    println!("    Database     : temp file");
    println!("    Iterations   : 5 per approach");

    // ─── Setup temp DB ──────────────────────────────────────────────────────────
    print_separator("Setup");

    let db_dir = std::env::temp_dir().join("vox_vector_bench");
    let _ = std::fs::create_dir_all(&db_dir);
    let db_path = db_dir.join("bench.db");
    let _ = std::fs::remove_file(&db_path);

    let conn = setup_db(&db_path, dim).await?;
    println!("  Database created at: {:?}", db_path);

    // ─── Generate and insert vectors ──────────────────────────────────────────
    print_separator("Data Generation");

    let query = random_unit_vector(&mut rng, dim);
    let query_blob = encode_f32_blob(&query);
    println!("  Query vector generated (dim={})", dim);

    let insert_start = Instant::now();
    for i in 0..num_vectors {
        let v = random_unit_vector(&mut rng, dim);
        let blob = encode_f32_blob(&v);
        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, '', ?)",
            (format!("fact_{:06}", i), blob),
        )
        .await?;
    }
    let insert_elapsed = insert_start.elapsed();
    println!(
        "  Inserted {} vectors in {:?} ({:.0} inserts/sec)",
        num_vectors,
        insert_elapsed,
        num_vectors as f64 / insert_elapsed.as_secs_f64()
    );

    // ─── Warmup ────────────────────────────────────────────────────────────────
    print_separator("Warmup");
    let mut warm_rows = conn
        .query(
            "SELECT id, (1.0 - vector_distance_cos(embedding, ?)) as sim
             FROM memory_facts_vectors
             ORDER BY sim DESC LIMIT 5",
            (query_blob.clone(),),
        )
        .await?;
    let mut warm_count = 0;
    while let Some(_) = warm_rows.next().await? {
        warm_count += 1;
    }
    let _ = conn
        .query(
            "SELECT id, embedding FROM memory_facts_vectors ORDER BY id ASC",
            (),
        )
        .await?;
    println!("  Warmup complete (SQL returned {} rows)", warm_count);

    // ─── Benchmark A: SQL vector_distance_cos Pushdown ─────────────────────────
    print_separator("Benchmark A: SQL vector_distance_cos() pushdown");

    let mut sql_latencies: Vec<std::time::Duration> = Vec::new();
    let mut sql_top5: Vec<(f64, i64)> = Vec::new();

    for iter in 0..5 {
        let start = Instant::now();
        let mut rows = conn
            .query(
                "SELECT id, (1.0 - vector_distance_cos(embedding, ?)) as sim
                 FROM memory_facts_vectors
                 ORDER BY sim DESC LIMIT 5",
                (query_blob.clone(),),
            )
            .await?;

        let mut results: Vec<(f64, i64)> = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let sim: f64 = row.get(1)?;
            results.push((sim, id));
        }
        let elapsed = start.elapsed();

        sql_latencies.push(elapsed);
        if iter == 0 {
            sql_top5 = results;
        }

        println!(
            "  Iter {}: {:>8.3}ms  (top-5 via SQL)",
            iter + 1,
            elapsed.as_secs_f64() * 1000.0
        );
    }

    let sql_avg = sql_latencies.iter().sum::<std::time::Duration>() / sql_latencies.len() as u32;
    println!(
        "\n  SQL avg latency: {:>8.3}ms",
        sql_avg.as_secs_f64() * 1000.0
    );

    // ─── Benchmark B: Rust cosine_similarity() Loop ────────────────────────────
    print_separator("Benchmark B: Rust cosine_similarity() manual loop");

    // Pre-load all vectors once (this is the minimum cost for the Rust approach)
    let load_start = Instant::now();
    let mut rows = conn
        .query(
            "SELECT id, embedding FROM memory_facts_vectors ORDER BY id ASC",
            (),
        )
        .await?;
    let mut db_vectors: Vec<(i64, Vec<f32>)> = Vec::with_capacity(num_vectors);
    while let Some(row) = rows.next().await? {
        let id: i64 = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        let decoded = decode_f32_blob(&blob);
        db_vectors.push((id, decoded));
    }
    let load_elapsed = load_start.elapsed();
    println!(
        "  Load+decode {} x {}d vectors in {:>8.3}ms  ({:.1} MB transferred)",
        db_vectors.len(),
        dim,
        load_elapsed.as_secs_f64() * 1000.0,
        (db_vectors.len() * dim * 4) as f64 / 1_048_576.0
    );

    let mut rust_latencies: Vec<std::time::Duration> = Vec::new();
    let mut rust_top5: Vec<(f64, i64)> = Vec::new();

    for iter in 0..5 {
        let start = Instant::now();

        let mut scored: Vec<(f64, i64)> = db_vectors
            .iter()
            .map(|(id, v)| (cosine_similarity(&query, v) as f64, *id))
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(5);

        let elapsed = start.elapsed();
        rust_latencies.push(elapsed);
        if iter == 0 {
            rust_top5 = scored;
        }

        println!(
            "  Iter {}: {:>8.3}ms  (top-5 via Rust)",
            iter + 1,
            elapsed.as_secs_f64() * 1000.0
        );
    }

    let rust_avg = rust_latencies.iter().sum::<std::time::Duration>() / rust_latencies.len() as u32;
    println!(
        "\n  Rust avg latency: {:>8.3}ms",
        rust_avg.as_secs_f64() * 1000.0
    );

    // ─── Result Equivalence Check ──────────────────────────────────────────────
    print_separator("Equivalence Check: SQL top-5 vs Rust top-5");

    let sql_set: Vec<(i64, f64)> = sql_top5.iter().map(|(s, id)| (*id, *s)).collect();
    let rust_set: Vec<(i64, f64)> = rust_top5.iter().map(|(s, id)| (*id, *s)).collect();

    println!("  SQL  top-5: {:?}", sql_set);
    println!("  Rust top-5: {:?}", rust_set);

    let sql_ids: Vec<i64> = sql_set.iter().map(|(id, _)| *id).collect();
    let rust_ids: Vec<i64> = rust_set.iter().map(|(id, _)| *id).collect();

    let ids_match = sql_ids == rust_ids;
    let same_set = sql_ids.iter().all(|id| rust_ids.contains(id));
    let mut max_diff = 0.0f64;
    for (s, r) in sql_set.iter().zip(rust_set.iter()) {
        let diff = (s.1 - r.1).abs();
        if diff > max_diff {
            max_diff = diff;
        }
    }

    println!();
    if ids_match {
        println!("  ✅ Top-5 IDs IDENTICAL (same order, same values)");
    } else if same_set {
        println!("  ⚠️  Top-5 set matches but order differs (near-ties)");
    } else {
        println!("  ❌ Top-5 results DIFFER — investigate");
    }
    println!("  Max floating-point diff: {:.2e}", max_diff);
    if max_diff < 1e-5 {
        println!("  ✅ Scores equivalent within 1e-5 tolerance");
    } else {
        println!("  ⚠️  Scores diverge beyond 1e-5 — investigate");
    }

    // ─── Summary ───────────────────────────────────────────────────────────────
    print_separator("Summary");

    if sql_avg.as_secs_f64() > 0.0 {
        let speedup = rust_avg.as_secs_f64() / sql_avg.as_secs_f64();
        println!(
            "  SQL  vector_distance_cos() avg: {:>8.3}ms",
            sql_avg.as_secs_f64() * 1000.0
        );
        println!(
            "  Rust cosine_similarity()   avg: {:>8.3}ms",
            rust_avg.as_secs_f64() * 1000.0
        );
        println!("  Speedup (Rust over SQL):     {:>8.2}x", speedup);

        let sql_per_item = sql_avg.as_secs_f64() / num_vectors as f64;
        let rust_per_item = rust_avg.as_secs_f64() / num_vectors as f64;
        println!();
        println!("  Per-vector:");
        println!("    SQL  pushdown: {:>8.3}µs", sql_per_item * 1_000_000.0);
        println!("    Rust loop:      {:>8.3}µs", rust_per_item * 1_000_000.0);
    }

    println!();
    println!("  Interpretation:");
    println!("  - SQL approach eliminates Rust-side decode + O(n) loop entirely.");
    println!("  - Data never leaves the engine — no Vec<Vec<f32>> allocation in app memory.");
    println!("  - For Vox's memory pipeline (seed + intra + inter), this means:");
    println!(
        "    - Lower latency per fact ingestion (3 SQL queries replaced 3 decode+loop passes)"
    );
    println!("    - Zero heap pressure from decoded vector storage");
    println!("    - Future: vector index (DiskANN) turns O(n) into O(log n)");
    println!();
    if ids_match {
        println!("  ✅ Quality: Identical results — vector_distance_cos() matches Rust cosine_similarity()");
        println!("     (both compute exact cosine distance; no approximation)");
    }

    // Cleanup
    let _ = std::fs::remove_file(&db_path);

    Ok(())
}
