//! ============================================================================
//! evals/agentic-tool/web_search.rs — Web retrieval latency and evidence-quality baseline with embedder ablation
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================


use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

use crate::common::{
    metrics::{mean_pairwise_cosine, score_facts},
    stage_dump::{self, names},
};

/// Model variants the eval can run. `minilm-prod` reproduces today's shipped
/// behaviour (special tokens inside the mean); `minilm-fixed` excludes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub enum EmbedderArm {
    MinilmProd,
    MinilmFixed,
    BgeM3,
    None,
}

impl std::str::FromStr for EmbedderArm {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "minilm-prod" | "minilm" => Ok(Self::MinilmProd),
            "minilm-fixed" | "fixed" => Ok(Self::MinilmFixed),
            "bge-m3" | "bgem3" => Ok(Self::BgeM3),
            "none" => Ok(Self::None),
            o => Err(format!(
                "Unknown embedder arm '{}'. Expected minilm-prod, minilm-fixed, bge-m3, none.",
                o
            )),
        }
    }
}

impl EmbedderArm {
    pub fn label(self) -> &'static str {
        match self {
            Self::MinilmProd => "minilm-prod",
            Self::MinilmFixed => "minilm-fixed",
            Self::BgeM3 => "bge-m3",
            Self::None => "none",
        }
    }
}

/// ONNX embedder with explicit control over pooling, so the F5 pooling defect can
/// be measured rather than argued about.
pub struct EvalEmbedder {
    session: parking_lot::Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
    has_token_type_ids: bool,
    pooling: Pooling,
    dim: usize,
    max_seq_len: usize,
    special_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pooling {
    /// Mean over all non-padding tokens, including `[CLS]`/`[SEP]`.
    MeanAll,
    /// Mean excluding special tokens — the pooling MiniLM was trained with.
    MeanNoSpecial,
    /// First-token vector then L2-normalised — what bge-m3 expects.
    ClsNormalized,
}

impl EvalEmbedder {
    pub fn load(kind: EmbedderArm) -> Result<Option<Arc<Self>>> {
        let (dir, file, dim, pooling, max_seq) = match kind {
            EmbedderArm::None => return Ok(None),
            EmbedderArm::MinilmProd => (
                "minilm-l12-v2",
                "model_int8.onnx",
                384usize,
                Pooling::MeanAll,
                256usize,
            ),
            EmbedderArm::MinilmFixed => (
                "minilm-l12-v2",
                "model_int8.onnx",
                384,
                Pooling::MeanNoSpecial,
                256,
            ),
            EmbedderArm::BgeM3 => (
                "bge-m3",
                "model_quantized.onnx",
                1024,
                Pooling::ClsNormalized,
                512,
            ),
        };

        let base = paths_models_dir().join("embedding").join(dir);
        let model_path = base.join(file);
        let tok_path = base.join("tokenizer.json");
        if !model_path.exists() || !tok_path.exists() {
            return Err(anyhow!(
                "Model assets missing for arm {}: expected {:?} and {:?}. \
                 Run with --embedder none to skip dense ranking.",
                kind.label(),
                model_path,
                tok_path
            ));
        }

        let tokenizer = Tokenizer::from_file(&tok_path)
            .map_err(|e| anyhow!("Failed to load tokenizer {:?}: {}", tok_path, e))?;
        let session = ort::session::Session::builder()
            .map_err(|e| anyhow!("Session builder failed: {:?}", e))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("Failed to set optimization level: {:?}", e))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("Failed to set intra threads: {:?}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow!("Failed to load model {:?}: {}", model_path, e))?;

        let has_token_type_ids = session
            .inputs()
            .iter()
            .any(|i| i.name() == "token_type_ids");
        let special_ids = Self::collect_special_ids(&tokenizer);

        Ok(Some(Arc::new(Self {
            session: parking_lot::Mutex::new(session),
            tokenizer,
            has_token_type_ids,
            pooling,
            dim,
            max_seq_len: max_seq,
            special_ids,
        })))
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Runs one batch. Truncates to `max_seq_len` so a pathological page cannot
    /// blow up latency and make the ablation meaningless.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;

        let max_len: usize = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0)
            .min(self.max_seq_len);
        if max_len == 0 {
            return Ok(texts.iter().map(|_| vec![0.0; self.dim]).collect());
        }

        let n = texts.len();
        let mut ids = Array2::<i64>::zeros((n, max_len));
        let mut mask = Array2::<i64>::zeros((n, max_len));
        let mut types = self
            .has_token_type_ids
            .then(|| Array2::<i64>::zeros((n, max_len)));

        for (i, enc) in encodings.iter().enumerate() {
            for j in 0..max_len {
                let id = enc.get_ids().get(j).copied().unwrap_or(0);
                let m = enc.get_attention_mask().get(j).copied().unwrap_or(0);
                ids[[i, j]] = id as i64;
                mask[[i, j]] = m as i64;
                if let Some(ref mut t) = types {
                    t[[i, j]] = enc.get_type_ids().get(j).copied().unwrap_or(0) as i64;
                }
            }
        }

        // Captured before `ids` is moved into a tensor.
        let special_mask: Vec<Vec<bool>> = (0..n)
            .map(|i| {
                (0..max_len)
                    .map(|j| self.is_special_id(ids[[i, j]]))
                    .collect()
            })
            .collect();

        let ids_t = ort::value::Tensor::from_array(ids)?;
        let mask_t = ort::value::Tensor::from_array(mask.clone())?;
        // The output borrow is tied to the session lock, so pooling runs inside it.
        let mut session = self.session.lock();
        let out = if let Some(t) = types {
            let tt = ort::value::Tensor::from_array(t)?;
            session.run(ort::inputs![
                "input_ids" => ids_t, "attention_mask" => mask_t, "token_type_ids" => tt
            ])?
        } else {
            session.run(ort::inputs![
                "input_ids" => ids_t, "attention_mask" => mask_t
            ])?
        };

        let key = out
            .keys()
            .next()
            .ok_or_else(|| anyhow!("No model output"))?;
        let hidden = out[key].try_extract_array::<f32>()?;
        let shape = hidden.shape().to_vec();
        let seq = shape[1];
        let hidden_size = shape[2];

        let mut out_vecs = Vec::with_capacity(n);
        for i in 0..n {
            let mut v = vec![0.0f32; hidden_size];
            match self.pooling {
                Pooling::ClsNormalized => {
                    for d in 0..hidden_size {
                        v[d] = hidden[[i, 0, d]];
                    }
                    l2_normalize(&mut v);
                }
                Pooling::MeanAll | Pooling::MeanNoSpecial => {
                    let mut sum = vec![0.0f32; hidden_size];
                    let mut denom = 0.0f32;
                    for t in 0..seq.min(max_len) {
                        let m = mask[[i, t]] as f32;
                        if m <= 0.0 {
                            continue;
                        }
                        if self.pooling == Pooling::MeanNoSpecial && special_mask[i][t] {
                            continue;
                        }
                        denom += m;
                        for d in 0..hidden_size {
                            sum[d] += hidden[[i, t, d]] * m;
                        }
                    }
                    if denom > 0.0 {
                        for d in 0..hidden_size {
                            v[d] = sum[d] / denom;
                        }
                    }
                    l2_normalize(&mut v);
                }
            }
            out_vecs.push(v);
        }
        drop(out);
        Ok(out_vecs)
    }

    fn is_special_id(&self, id: i64) -> bool {
        self.special_ids.contains(&id)
    }

    fn collect_special_ids(t: &Tokenizer) -> Vec<i64> {
        ["[CLS]", "[SEP]", "[PAD]", "<s>", "</s>", "<pad>", "[MASK]"]
            .iter()
            .filter_map(|s| t.token_to_id(s))
            .map(|i| i as i64)
            .collect()
    }
}

fn l2_normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 && n.is_finite() {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

fn paths_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.vox/models")
}

/// Bridges the crate's `TextEmbedder` trait onto [`EvalEmbedder`].
pub struct EvalEmbedderAdapter(pub Arc<EvalEmbedder>);

#[async_trait::async_trait]
impl nexus::TextEmbedder for EvalEmbedderAdapter {
    async fn embed_text(&self, text: &str) -> std::result::Result<Vec<f32>, nexus::NexusError> {
        let mut v = self
            .0
            .embed_batch(&[text])
            .map_err(|e| nexus::NexusError::Embedding(e.to_string()))?;
        v.pop()
            .ok_or_else(|| nexus::NexusError::Embedding("empty embed batch".into()))
    }

    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> std::result::Result<Vec<Vec<f32>>, nexus::NexusError> {
        self.0
            .embed_batch(texts)
            .map_err(|e| nexus::NexusError::Embedding(e.to_string()))
    }
}

/// One corpus entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusEntry {
    pub id: String,
    pub category: String,
    pub query: String,
    #[serde(default)]
    pub expected_facts: Vec<String>,
    #[serde(default = "one")]
    pub min_distinct_sources: usize,
    #[serde(default)]
    pub max_acceptable_ms: Option<u64>,
    pub notes: String,
}

fn one() -> usize {
    1
}

/// Loads the corpus, resolving the same candidate paths `datasets.rs` uses.
pub fn load_corpus(explicit: Option<&Path>) -> Result<Vec<CorpusEntry>> {
    let candidates: Vec<PathBuf> = match explicit {
        Some(p) => vec![p.to_path_buf()],
        None => vec![
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evals/assets/web_search_corpus.json"),
            PathBuf::from("evals/assets/web_search_corpus.json"),
            PathBuf::from("app/src-tauri/evals/assets/web_search_corpus.json"),
        ],
    };
    let path = candidates
        .iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow!("Corpus not found. Tried: {:?}", candidates))?;
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read corpus at {:?}", path))?;
    serde_json::from_str(&raw).with_context(|| format!("Failed to parse corpus at {:?}", path))
}

/// Applies `--limit`, `--category` and `--filter`.
pub fn filter_corpus(
    entries: Vec<CorpusEntry>,
    limit: Option<usize>,
    category: Option<&str>,
    filter: Option<&str>,
) -> Vec<CorpusEntry> {
    let mut out: Vec<CorpusEntry> = entries
        .into_iter()
        .filter(|e| {
            category
                .map(|c| e.category.eq_ignore_ascii_case(c))
                .unwrap_or(true)
        })
        .filter(|e| {
            filter
                .map(|f| e.id.contains(f) || e.query.contains(f))
                .unwrap_or(true)
        })
        .collect();
    if let Some(l) = limit {
        out.truncate(l);
    }
    out
}

/// Per-case result metrics, all recomputable from the written artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseMetrics {
    pub query_id: String,
    pub category: String,
    pub ok: bool,
    pub error: Option<String>,

    pub total_pipeline_ms: u64,
    pub fanout_total_ms: u64,
    pub url_dedup_ms: u64,
    pub fetch_total_ms: u64,
    pub extraction_total_ms: u64,
    pub chunking_total_ms: u64,
    pub ranking_total_ms: u64,
    pub sparse_ranking_ms: Option<u64>,
    pub dense_ranking_ms: Option<u64>,

    pub raw_hits: usize,
    pub dedup_hits: usize,
    pub candidate_urls: usize,
    pub pages_attempted: usize,
    pub pages_extracted: usize,
    pub passages_generated: usize,
    pub passages_delivered: usize,

    pub fetch_success_rate: f64,
    pub distinct_domains_delivered: usize,
    pub source_diversity: f64,
    pub consensus_urls_populated: bool,

    pub facts_expected: usize,
    pub facts_matched: usize,
    pub answer_in_snippet_rate: f64,
    pub fact_similarity_mean: f64,
    pub fact_scores: Vec<f64>,
    pub evidence_tokens: usize,
    pub redundancy: f64,
    pub context_utilization: f64,

    pub engine_success: BTreeMap<String, bool>,
    pub engine_latency_ms: BTreeMap<String, u64>,
    pub fetch_failures: Vec<String>,
    pub blocked_fetches: usize,
}

impl CaseMetrics {
    /// A run where the pipeline "succeeded" but returned almost nothing is the
    /// failure mode this eval exists to catch, so it is scored explicitly.
    pub fn is_starved(&self) -> bool {
        self.ok && self.passages_delivered <= 1
    }
}

/// Evidence delivery constants mirrored from `web_search.rs`, so the eval measures
/// what production actually does.
pub const MAX_CONTEXT_SHARE_CAP: f32 = 0.30;
pub const HARD_TOKEN_CEILING: usize = 2000;
pub const AVERAGE_PASSAGE_TOKENS: usize = 250;

/// Reproduces production's Stage 3 budget clamp.
pub fn compute_effective_k(
    max_passages: usize,
    remaining_tokens: usize,
    corpus_len: usize,
) -> (usize, usize, usize) {
    let token_ceiling =
        (((remaining_tokens as f32) * MAX_CONTEXT_SHARE_CAP) as usize).min(HARD_TOKEN_CEILING);
    let budget_k = (token_ceiling / AVERAGE_PASSAGE_TOKENS).max(1);
    (
        max_passages.min(budget_k).min(corpus_len),
        token_ceiling,
        budget_k,
    )
}

/// Extracts sources and passages from a delivered observation.
///
/// Parses the same XML the model receives, so diversity and token metrics are
/// computed against real delivered content rather than internal structures.
pub type ParsedSource = (String, String, String);
pub type ParsedPassage = (usize, f32, String);

pub fn parse_evidence(observation: &str) -> (Vec<ParsedSource>, Vec<ParsedPassage>) {
    let mut sources: Vec<ParsedSource> = Vec::new();
    let mut passages: Vec<ParsedPassage> = Vec::new();

    #[derive(PartialEq)]
    enum State {
        Outside,
        InSource(usize),
        InPassage(usize),
    }
    let mut state = State::Outside;

    for line in observation.lines() {
        let t = line.trim();
        if t.starts_with("<web_search_evidence") || t.starts_with("</web_search_evidence") {
            continue;
        }
        if t.starts_with("<source ") {
            sources.push((
                attr(t, "url").unwrap_or_default(),
                attr(t, "title").unwrap_or_default(),
                String::new(),
            ));
            state = State::InSource(sources.len() - 1);
        } else if t.starts_with("<passage ") {
            passages.push((
                attr(t, "rank").and_then(|r| r.parse().ok()).unwrap_or(0),
                attr(t, "score").and_then(|r| r.parse().ok()).unwrap_or(0.0),
                String::new(),
            ));
            state = State::InPassage(passages.len() - 1);
        } else if t == "</passage>" {
            if let State::InPassage(_) = state {
                state = State::Outside;
            }
        } else if t == "</source>" {
            state = State::Outside;
        } else if !t.is_empty() && !t.starts_with('<') {
            match state {
                State::InPassage(i) => {
                    if passages[i].2.is_empty() {
                        passages[i].2 = t.to_string();
                    } else {
                        passages[i].2.push(' ');
                        passages[i].2.push_str(t);
                    }
                }
                State::InSource(i) => {
                    if sources[i].2.is_empty() {
                        sources[i].2 = t.to_string();
                    } else {
                        sources[i].2.push(' ');
                        sources[i].2.push_str(t);
                    }
                }
                State::Outside => {}
            }
        }
    }

    (sources, passages)
}

pub fn attr(tag: &str, key: &str) -> Option<String> {
    let pat = format!("{}=\"", key);
    let start = tag.find(&pat)? + pat.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Renders the delivered passages into the exact XML the model receives.
///
/// Duplicated here rather than imported because `render_evidence_xml` is private to
/// the tool module; keeping the format identical is what makes the artifact
/// byte-comparable with production output.
pub fn render_evidence_xml(
    query: &str,
    ranking_mode: nexus::RankingMode,
    passages: &[nexus::ScoredPassage],
    max_passages: usize,
    remaining_tokens: usize,
) -> String {
    let (effective_k, token_ceiling, budget_k) =
        compute_effective_k(max_passages, remaining_tokens, passages.len());
    let selected = &passages[..effective_k];

    struct Src<'a> {
        title: &'a str,
        url: &'a str,
        items: Vec<(usize, f32, &'a str)>,
    }
    let mut list: Vec<Src> = Vec::new();
    let mut idx: BTreeMap<&str, usize> = BTreeMap::new();

    for (i, p) in selected.iter().enumerate() {
        let rank = i + 1;
        let e = idx.entry(p.source_url.as_str()).or_insert_with(|| {
            list.push(Src {
                title: p.source_title.as_str(),
                url: p.source_url.as_str(),
                items: Vec::new(),
            });
            list.len() - 1
        });
        list[*e].items.push((rank, p.score, p.text.as_str()));
    }

    let mode = match ranking_mode {
        nexus::RankingMode::Sparse => "sparse",
        nexus::RankingMode::Dense => "dense",
        nexus::RankingMode::Hybrid => "hybrid",
    };

    let mut xml = format!(
        "<web_search_evidence query=\"{}\" ranking_mode=\"{}\" total_sources=\"{}\" total_passages=\"{}\" token_ceiling=\"{}\" budget_k=\"{}\">\n",
        xml_escape(query),
        mode,
        list.len(),
        selected.len(),
        token_ceiling,
        budget_k
    );
    for (i, s) in list.iter().enumerate() {
        xml.push_str(&format!(
            "  <source id=\"{}\" title=\"{}\" url=\"{}\">\n",
            i + 1,
            xml_escape(s.title),
            xml_escape(s.url)
        ));
        for (rank, score, text) in &s.items {
            xml.push_str(&format!(
                "    <passage rank=\"{}\" score=\"{:.3}\">\n      {}\n    </passage>\n",
                rank,
                score,
                xml_escape(text.trim())
            ));
        }
        xml.push_str("  </source>\n");
    }
    xml.push_str("</web_search_evidence>");
    xml
}

pub fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\u{0}'..='\u{8}' | '\u{0B}'..='\u{0C}' | '\u{0E}'..='\u{1F}' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// Runs one corpus entry end to end and writes every stage artifact.
pub async fn run_case(
    entry: &CorpusEntry,
    case_dir: &Path,
    cfg: &CaseConfig,
    engine: &nexus::NexusSearch,
) -> Result<CaseMetrics> {
    std::fs::create_dir_all(case_dir)?;

    let options = nexus::NexusSearchOptions {
        time_filter: cfg.time_filter,
        ranking_mode: cfg.ranking_mode,
        max_candidates: cfg.max_candidates,
        chunk_size_words: cfg.chunk_size_words,
        chunk_overlap_words: cfg.chunk_overlap_words,
        fetch_timeout_ms: cfg.fetch_timeout_ms,
        max_response_bytes: cfg.max_response_bytes,
    };

    let started = Instant::now();
    let outcome = engine.search(&entry.query, &options).await;
    let wall_ms = started.elapsed().as_millis() as u64;

    match outcome {
        Err(e) => {
            let m = CaseMetrics {
                query_id: entry.id.clone(),
                category: entry.category.clone(),
                ok: false,
                error: Some(e.to_string()),
                total_pipeline_ms: wall_ms,
                fanout_total_ms: 0,
                url_dedup_ms: 0,
                fetch_total_ms: 0,
                extraction_total_ms: 0,
                chunking_total_ms: 0,
                ranking_total_ms: 0,
                sparse_ranking_ms: None,
                dense_ranking_ms: None,
                raw_hits: 0,
                dedup_hits: 0,
                candidate_urls: 0,
                pages_attempted: 0,
                pages_extracted: 0,
                passages_generated: 0,
                passages_delivered: 0,
                fetch_success_rate: 0.0,
                distinct_domains_delivered: 0,
                source_diversity: 0.0,
                consensus_urls_populated: false,
                facts_expected: entry.expected_facts.len(),
                facts_matched: 0,
                answer_in_snippet_rate: 0.0,
                fact_similarity_mean: 0.0,
                fact_scores: Vec::new(),
                evidence_tokens: 0,
                redundancy: 0.0,
                context_utilization: 0.0,
                engine_success: BTreeMap::new(),
                engine_latency_ms: BTreeMap::new(),
                fetch_failures: vec![e.to_string()],
                blocked_fetches: 0,
            };
            write_case_artifacts(case_dir, entry, None, None, None, None, &m).await?;
            Ok(m)
        }
        Ok(res) => {
            let mx = &res.metrics;

            stage_dump::write_json(
                case_dir,
                names::RAW_NEXUS_METRICS,
                &serde_json::to_value(mx)?,
            )?;

            stage_dump::write_json(
                case_dir,
                names::STAGE_1_FANOUT,
                &serde_json::json!({
                    "query": entry.query,
                    "time_filter": format!("{:?}", cfg.time_filter),
                    "fanout_total_ms": mx.fanout_total_ms,
                    "raw_hits": mx.total_raw_hits,
                    "deduplicated_hits": mx.deduplicated_hits,
                    "engines": mx.engines,
                    "consensus_urls_populated": !res.raw_pages.is_empty() && mx.total_raw_hits > 0,
                }),
            )?;

            let fetch_json: Vec<_> = mx
                .pages_fetched
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "requested_url": p.requested_url,
                        "final_url": p.final_url,
                        "dns_resolution_ms": p.dns_resolution_ms,
                        "total_fetch_ms": p.total_fetch_ms,
                        "bytes_read": p.bytes_read,
                        "hop_count": p.hop_count,
                        "success": p.success,
                        "error": p.error,
                    })
                })
                .collect();
            stage_dump::write_json(
                case_dir,
                names::STAGE_1B_FETCH,
                &serde_json::json!({
                    "fetch_total_ms": mx.fetch_total_ms,
                    "attempted": fetch_json.len(),
                    "fetches": fetch_json,
                }),
            )?;

            let extract_json: Vec<_> = mx
                .pages_extracted
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "url": p.url,
                        "extraction_ms": p.extraction_ms,
                        "markdown_bytes": p.markdown_bytes,
                    })
                })
                .collect();
            let previews: Vec<_> = res
                .raw_pages
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "url": p.url,
                        "title": p.title,
                        "markdown": stage_dump::preview(&p.markdown, 2000),
                    })
                })
                .collect();
            stage_dump::write_json(
                case_dir,
                names::STAGE_1C_EXTRACT,
                &serde_json::json!({
                    "extraction_total_ms": mx.extraction_total_ms,
                    "extracted": extract_json.len(),
                    "pages": extract_json,
                    "previews": previews,
                }),
            )?;

            stage_dump::write_json(
                case_dir,
                names::STAGE_2_CHUNK,
                &serde_json::json!({
                    "chunking_total_ms": mx.chunking_total_ms,
                    "total_passages_generated": mx.total_passages_generated,
                    "chunk_size_words": cfg.chunk_size_words,
                    "chunk_overlap_words": cfg.chunk_overlap_words,
                    "passages": res.scored_passages.iter().map(|p| serde_json::json!({
                        "passage_index": p.passage_index,
                        "source_url": p.source_url,
                        "source_title": p.source_title,
                        "chars": p.text.chars().count(),
                        "words": p.text.split_whitespace().count(),
                        "preview": stage_dump::preview(&p.text, 300),
                    })).collect::<Vec<_>>(),
                }),
            )?;

            let remaining = cfg.context_window.saturating_sub(cfg.max_output_tokens) - 4;
            let xml = render_evidence_xml(
                &entry.query,
                cfg.ranking_mode,
                &res.scored_passages,
                cfg.max_passages,
                remaining,
            );
            stage_dump::write_verbatim(case_dir, names::EVIDENCE_XML, &xml)?;

            let (sources, passages) = parse_evidence(&xml);
            let mut domains: HashSet<String> = HashSet::new();
            for (url, _, _) in &sources {
                domains.insert(stage_dump::apex_domain(url));
            }

            let embedded: Vec<Vec<f32>> = match cfg.embedder.as_ref() {
                Some(emb) => {
                    let refs: Vec<&str> = passages.iter().map(|(_, _, t)| t.as_str()).collect();
                    emb.embed_batch(&refs).unwrap_or_default()
                }
                None => Vec::new(),
            };

            let redundancy = mean_pairwise_cosine(&embedded);
            let evidence_tokens = stage_dump::estimate_tokens(&xml);
            let token_ceiling =
                (((remaining as f32) * MAX_CONTEXT_SHARE_CAP) as usize).min(HARD_TOKEN_CEILING);
            let evidence_text: String = passages.iter().map(|(_, _, t)| t.as_str()).collect();
            let (fact_sim_mean, fact_scores, facts_matched) =
                score_facts(&evidence_text, &entry.expected_facts);

            let mut engine_success = BTreeMap::new();
            let mut engine_latency = BTreeMap::new();
            for e in &mx.engines {
                engine_success.insert(e.engine.to_string(), e.success);
                engine_latency.insert(e.engine.to_string(), e.latency_ms);
            }

            let fetch_failures: Vec<String> = mx
                .pages_fetched
                .iter()
                .filter(|p| !p.success)
                .map(|p| {
                    format!(
                        "{} :: {}",
                        p.requested_url,
                        p.error.as_deref().unwrap_or("unknown")
                    )
                })
                .collect();

            let attempted = mx.pages_fetched.len();
            let m = CaseMetrics {
                query_id: entry.id.clone(),
                category: entry.category.clone(),
                ok: true,
                error: None,
                total_pipeline_ms: mx.total_pipeline_ms,
                fanout_total_ms: mx.fanout_total_ms,
                url_dedup_ms: mx.url_dedup_ms,
                fetch_total_ms: mx.fetch_total_ms,
                extraction_total_ms: mx.extraction_total_ms,
                chunking_total_ms: mx.chunking_total_ms,
                ranking_total_ms: mx.ranking_total_ms,
                sparse_ranking_ms: mx.sparse_ranking_ms,
                dense_ranking_ms: mx.dense_ranking_ms,
                raw_hits: mx.total_raw_hits,
                dedup_hits: mx.deduplicated_hits,
                candidate_urls: cfg.max_candidates,
                pages_attempted: attempted,
                pages_extracted: mx.pages_extracted.len(),
                passages_generated: mx.total_passages_generated,
                passages_delivered: passages.len(),
                fetch_success_rate: if attempted == 0 {
                    0.0
                } else {
                    mx.pages_extracted.len() as f64 / attempted as f64
                },
                distinct_domains_delivered: domains.len(),
                source_diversity: if sources.is_empty() {
                    0.0
                } else {
                    domains.len() as f64 / sources.len() as f64
                },
                consensus_urls_populated: false,
                facts_expected: entry.expected_facts.len(),
                facts_matched,
                answer_in_snippet_rate: if entry.expected_facts.is_empty() {
                    0.0
                } else {
                    facts_matched as f64 / entry.expected_facts.len() as f64
                },
                fact_similarity_mean: fact_sim_mean,
                fact_scores,
                evidence_tokens,
                redundancy,
                context_utilization: if token_ceiling == 0 {
                    0.0
                } else {
                    evidence_tokens as f64 / token_ceiling as f64
                },
                engine_success,
                engine_latency_ms: engine_latency,
                fetch_failures,
                blocked_fetches: mx.pages_fetched.iter().filter(|p| !p.success).count(),
            };
            stage_dump::write_json(
                case_dir,
                names::STAGE_2_RANK,
                &serde_json::json!({
                    "ranking_mode": format!("{:?}", cfg.ranking_mode),
                    "ranking_total_ms": mx.ranking_total_ms,
                    "sparse_ranking_ms": mx.sparse_ranking_ms,
                    "dense_ranking_ms": mx.dense_ranking_ms,
                    "rrf_fusion_ms": mx.rrf_fusion_ms,
                    "passages": res.scored_passages.iter().map(|p| serde_json::json!({
                        "source_url": p.source_url,
                        "source_title": p.source_title,
                        "passage_index": p.passage_index,
                        "score": p.score,
                        "sparse_score": p.sparse_score,
                        "dense_score": p.dense_score,
                    })).collect::<Vec<_>>(),
                    "delivered_embeddings": embedded,
                }),
            )?;

            write_case_artifacts(
                case_dir,
                entry,
                Some(&res),
                Some(&xml),
                Some(&sources),
                Some(&passages),
                &m,
            )
            .await?;
            Ok(m)
        }
    }
}

async fn write_case_artifacts(
    case_dir: &Path,
    entry: &CorpusEntry,
    result: Option<&nexus::NexusSearchResult>,
    xml: Option<&str>,
    sources: Option<&Vec<(String, String, String)>>,
    passages: Option<&Vec<(usize, f32, String)>>,
    m: &CaseMetrics,
) -> Result<()> {
    stage_dump::write_json(case_dir, names::METRICS, &serde_json::to_value(m)?)?;

    if let Some(x) = xml {
        stage_dump::write_verbatim(case_dir, names::EVIDENCE_TXT, x)?;
    }
    let parsed = serde_json::json!({
        "query_id": entry.id,
        "category": entry.category,
        "notes": entry.notes,
        "expected_facts": entry.expected_facts,
        "min_distinct_sources": entry.min_distinct_sources,
        "max_acceptable_ms": entry.max_acceptable_ms,
        "sources": sources.map(|s| s.iter().map(|(u, t, _)| serde_json::json!({"url": u, "title": t})).collect::<Vec<_>>()).unwrap_or_default(),
        "passages": passages.map(|p| p.iter().map(|(r, s, t)| serde_json::json!({"rank": r, "score": s, "chars": t.chars().count()})).collect::<Vec<_>>()).unwrap_or_default(),
        "raw_page_count": result.map(|r| r.raw_pages.len()).unwrap_or(0),
    });
    stage_dump::write_json(case_dir, "parsed_evidence.json", &parsed)?;
    Ok(())
}

/// Per-run configuration shared by every case.
pub struct CaseConfig {
    pub ranking_mode: nexus::RankingMode,
    pub time_filter: nexus::TimeFilter,
    pub max_candidates: usize,
    pub chunk_size_words: usize,
    pub chunk_overlap_words: usize,
    pub fetch_timeout_ms: u64,
    pub max_response_bytes: usize,
    pub max_passages: usize,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub embedder: Option<Arc<EvalEmbedder>>,
}

/// Embedding probe used only for the redundancy metric, never for ranking.
#[allow(dead_code)]
pub fn embed_passages(emb: &EvalEmbedder, passages: &[(usize, f32, String)]) -> Vec<Vec<f32>> {
    if passages.len() < 2 {
        return Vec::new();
    }
    let refs: Vec<&str> = passages.iter().map(|(_, _, t)| t.as_str()).collect();
    emb.embed_batch(&refs).unwrap_or_default()
}
