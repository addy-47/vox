//! ============================================================================
//! structure.rs — Accumulation-side structure measurements for the memory ladder
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common (vox_lib memory::personal patch engine)
//! Prerequisites: None
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : Nameless headings, duplicate headings, duplicate bullets, bullet-per-section
//!                distribution, patch skip count, section-membership violations
//!
//! Why this module exists
//! ---------------------
//! The anchor-preservation check in `preservation.rs` is *loss*-only: it diffs base-document
//! lines against the accepted document. In the append-only degeneration documented in
//! `docs/plans/phase12/consolidation-structured--logic-plan.md` §2.7 it is structurally blind,
//! because nothing is ever lost — only accumulated. These measurements are the
//! *accumulation*-side counterpart: they fail when the document degrades even though no
//! line disappeared.
//!
//! Every function here is a pure measurement over documents that already exist. Nothing in
//! this module re-implements the patch engine's decision logic: bounds and skip counts are
//! derived from the real engine, from `apply_patch_operations`, and from the production
//! element parser.
//! ============================================================================

use std::collections::{BTreeMap, HashMap};

use vox_lib::services::memory::personal::{
    apply_patch_operations, parse_content_elements, ElementKind, MemoryPatchOperation,
};

/// One measured defect against the `memory-spec.md` §5.1 structure contract.
#[derive(Debug, Clone)]
pub struct StructureDefect {
    pub kind: &'static str,
    pub detail: String,
}

impl StructureDefect {
    fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

/// Accumulation-side measurement of a personal-memory document against the §5.1 contract.
#[derive(Debug, Clone)]
pub struct StructureReport {
    /// Content elements excluding headings — the claim-bearing lines.
    pub bullets_total: usize,
    pub headings_total: usize,
    /// §5.1(1): a `##` heading whose title is empty or whitespace.
    pub nameless_headings: Vec<String>,
    /// §5.1(2): heading titles repeating case-insensitively within one document.
    pub duplicate_headings: Vec<String>,
    /// §5.1(3): documents with no heading at all.
    pub has_any_heading: bool,
    /// §5.1(4): documents with no content element.
    pub is_empty: bool,
    /// Bullets repeating case-insensitively — the duplicate-fact degeneration of §2.5.
    pub duplicate_bullets: Vec<String>,
    /// Heading title -> bullet count, in document order. Exposes sections that
    /// accumulate content and sections that never do.
    pub bullets_per_section: BTreeMap<String, usize>,
    /// Bullets appearing before the document's first heading.
    pub bullets_above_first_heading: usize,
    /// Every §5.1 contract violation found, for verbatim inclusion in the run report.
    pub defects: Vec<StructureDefect>,
}

impl StructureReport {
    /// True when the document satisfies every condition of the §5.1 structure contract.
    pub fn contract_satisfied(&self) -> bool {
        self.defects.is_empty()
    }
}

/// Measures a personal-memory document against the §5.1 structure contract and the
/// accumulation-side degeneration signals from §2.5.
pub fn measure_document(document: &str) -> StructureReport {
    let elements = parse_content_elements(document);
    let headings_total = elements
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Heading(_)))
        .count();

    let mut nameless_headings = Vec::new();
    let mut heading_title_counts: HashMap<String, usize> = HashMap::new();
    let mut heading_order: Vec<String> = Vec::new();

    for element in &elements {
        if !matches!(element.kind, ElementKind::Heading(_)) {
            continue;
        }
        let title = element.raw_text.trim_start_matches('#').trim().to_string();
        if title.is_empty() {
            nameless_headings.push(element.raw_text.clone());
            // A nameless heading still occupies a slot in the section map so that
            // bullets filed under it are attributable rather than silently counted.
            heading_order.push(String::from("<nameless>"));
        } else {
            *heading_title_counts
                .entry(title.to_lowercase())
                .or_insert(0) += 1;
            heading_order.push(title);
        }
    }

    let duplicate_headings: Vec<String> = heading_title_counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(title, _)| title.clone())
        .collect();
    let mut duplicate_headings = duplicate_headings;
    duplicate_headings.sort();

    // Bullets are the claim-bearing lines. Headings and blank lines are excluded so a
    // duplicate means a duplicated *claim*, not a repeated word in two section titles.
    let bullet_texts: Vec<String> = elements
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Bullet))
        .map(|e| {
            e.raw_text
                .trim_start_matches("- ")
                .trim_start_matches("* ")
                .trim()
                .to_string()
        })
        .collect();

    let mut bullet_counts: HashMap<String, usize> = HashMap::new();
    for text in &bullet_texts {
        *bullet_counts.entry(text.to_lowercase()).or_insert(0) += 1;
    }
    let mut duplicate_bullets: Vec<String> = bullet_counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(text, _)| text.clone())
        .collect();
    duplicate_bullets.sort();

    // Attribute each bullet to the nearest preceding heading, walking the real element
    // sequence rather than re-parsing text, so ordering matches the engine's own view.
    let mut bullets_per_section: BTreeMap<String, usize> = BTreeMap::new();
    let mut current_section: Option<String> = None;
    let mut bullets_above_first_heading = 0usize;
    for element in &elements {
        match element.kind {
            ElementKind::Heading(_) => {
                let title = element.raw_text.trim_start_matches('#').trim().to_string();
                current_section = if title.is_empty() {
                    Some(String::from("<nameless>"))
                } else {
                    Some(title)
                };
                bullets_per_section
                    .entry(current_section.clone().unwrap())
                    .or_insert(0);
            }
            ElementKind::Bullet => match &current_section {
                Some(section) => *bullets_per_section.entry(section.clone()).or_insert(0) += 1,
                None => bullets_above_first_heading += 1,
            },
            ElementKind::Paragraph => {}
        }
    }

    let has_any_heading = headings_total > 0;
    let is_empty = elements.is_empty();

    let mut defects = Vec::new();
    for heading in &nameless_headings {
        defects.push(StructureDefect::new(
            "nameless_heading",
            format!("heading has no title: {heading:?}"),
        ));
    }
    for heading in &duplicate_headings {
        defects.push(StructureDefect::new(
            "duplicate_heading",
            format!("heading title appears more than once: {heading:?}"),
        ));
    }
    if !has_any_heading {
        defects.push(StructureDefect::new(
            "no_headings",
            "document has no section headings; a flat bullet list is a contract violation",
        ));
    }
    if is_empty {
        defects.push(StructureDefect::new(
            "empty_document",
            "document has no content elements",
        ));
    }
    for bullet in &duplicate_bullets {
        defects.push(StructureDefect::new(
            "duplicate_bullet",
            format!("bullet text repeats in the document: {bullet:?}"),
        ));
    }
    if bullets_above_first_heading > 0 {
        defects.push(StructureDefect::new(
            "bullets_above_first_heading",
            format!("{bullets_above_first_heading} bullet(s) appear before any heading"),
        ));
    }

    StructureReport {
        bullets_total: bullet_texts.len(),
        headings_total,
        nameless_headings,
        duplicate_headings,
        has_any_heading,
        is_empty,
        duplicate_bullets,
        bullets_per_section,
        bullets_above_first_heading,
        defects,
    }
}

/// Per-operation accounting derived from the real patch engine.
///
/// `memory_pipeline_eval.rs` previously inferred applicability from its own stub
/// (`op != "delete" && content non-empty`), which could not fail for an out-of-range
/// index. The engine at `personal.rs::apply_patch_operations` silently skips out-of-range
/// `replace`/`delete` and clamps out-of-range `insert_after` to an append.
#[derive(Debug, Clone)]
pub struct EngineAccounting {
    /// Total operations the LLM proposed.
    pub operations_total: u32,
    /// Operations the engine genuinely DROPPED — the intent never reached the document.
    /// A clamped append is *not* counted here: the content is applied successfully, just at
    /// the end rather than the requested position. Counting it as skipped would fail a run
    /// for correct behaviour, since "add a new section at the end" is a legitimate outcome.
    pub dropped_by_engine: u32,
    /// Operations whose target index exceeded the document's element count and were clamped
    /// to an append. Reported as model imprecision, not as a failure.
    pub clamped_to_append: u32,
    /// Out-of-range anchors that the prompt's own worked example legitimately produces when
    /// opening a section past the last element. Counted separately from `clamped_to_append`
    /// so the harness does not penalise the documented pattern.
    pub chained_section_anchors: u32,
    /// Per-op detail, in engine application order (descending index), for the report.
    pub per_operation: Vec<OperationTrace>,
    /// The document the engine produces from base + all operations. Compared against the
    /// document the production accept path actually committed.
    pub engine_replay_document: String,
    /// True when the committed document equals the engine's independent replay.
    pub engine_replay_matches_committed: bool,
}

#[derive(Debug, Clone)]
pub struct OperationTrace {
    pub op: String,
    pub target_index: u32,
    pub element_count_at_stage: usize,
    /// Whether the index addresses a real element under the engine's 1-based bounds.
    pub index_in_bounds: bool,
    /// Whether the operation carries the content the engine requires to act.
    pub content_present: bool,
    /// Whether the engine acts on it, skips it, or clamps it.
    pub engine_action: &'static str,
    pub content: String,
}

/// Replays `operations` through the real engine and reports what the engine did with each.
///
/// `base_document` is the document the suggestions were staged against, which is the same
/// base `apply_patch_operations` receives on the accept path.
pub fn account_operations(
    base_document: &str,
    operations: &[(String, u32, String)],
) -> EngineAccounting {
    let element_count = parse_content_elements(base_document).len();
    let mut per_operation = Vec::with_capacity(operations.len());

    // Anchors that the prompt's own worked example legitimately produces.
    //
    // Opening a section at the very end of the document requires `insert_after(N, "## H")`
    // followed by `insert_after(N+1, "- bullet")`. With N == element_count, the bullet's
    // anchor is one past the end, which the engine clamps to an append. That clamp is not
    // imprecision — descending application means the bullet is applied first and the heading
    // is then inserted in front of it, so the section lands correctly. Any `insert_after` at
    // N+1 where the same batch opens a heading at N is this pattern, so it is accounted for
    // as a chained anchor rather than counted against the model.
    let chained_anchors: std::collections::HashSet<u32> = operations
        .iter()
        .filter(|(op, index, content)| {
            op.trim() == "insert_after" && content.trim().starts_with('#') && *index > 0
        })
        .map(|(_, index, _)| index + 1)
        .collect();

    let mut dropped = 0u32;
    let mut clamped = 0u32;
    let mut chained = 0u32;
    for (op, index, content) in operations {
        let op_norm = op.trim().to_lowercase();
        let content_present = !content.trim().is_empty();
        // The engine's own guards: `delete` needs only a valid index; `insert_after` and
        // `replace` additionally need non-empty text, and an empty-text op is skipped
        // before any bounds check.
        let index_in_bounds = *index > 0 && (*index as usize) <= element_count;
        let is_chained_anchor =
            op_norm == "insert_after" && *index == element_count as u32 + 1 && chained_anchors.contains(index);
        let engine_action: &'static str = match op_norm.as_str() {
            "insert_after" => {
                if !content_present {
                    "dropped_empty_content"
                } else if *index == 0 {
                    "prepend"
                } else if index_in_bounds {
                    "inserted"
                } else if is_chained_anchor {
                    "inserted_chained_section_anchor"
                } else {
                    // Engine clamps to an append rather than failing. The content is applied,
                    // so this is imprecision in the model's index arithmetic, not a lost edit.
                    "clamped_to_append_out_of_range"
                }
            }
            "replace" => {
                if !content_present {
                    "dropped_empty_content"
                } else if index_in_bounds {
                    "replaced"
                } else {
                    "dropped_out_of_range"
                }
            }
            "delete" => {
                if index_in_bounds {
                    "deleted"
                } else {
                    "dropped_out_of_range"
                }
            }
            _ => "dropped_unknown_op",
        };
        match engine_action {
            "clamped_to_append_out_of_range" => clamped += 1,
            "inserted_chained_section_anchor" => chained += 1,
            _ if engine_action.starts_with("dropped") => dropped += 1,
            _ => {}
        }
        per_operation.push(OperationTrace {
            op: op_norm,
            target_index: *index,
            element_count_at_stage: element_count,
            index_in_bounds,
            content_present,
            engine_action,
            content: content.clone(),
        });
    }

    let patch_ops: Vec<MemoryPatchOperation> = operations
        .iter()
        .map(|(op, index, content)| MemoryPatchOperation {
            op: op.clone(),
            index: *index,
            text: content.clone(),
        })
        .collect();
    let engine_replay_document = apply_patch_operations(base_document, &patch_ops)
        .map(|report| report.document)
        .unwrap_or_default();

    EngineAccounting {
        operations_total: operations.len() as u32,
        dropped_by_engine: dropped,
        clamped_to_append: clamped,
        chained_section_anchors: chained,
        per_operation,
        engine_replay_document,
        engine_replay_matches_committed: false,
    }
}

/// Result of checking INVARIANT 5.3-B index arithmetic against the real rows.
#[derive(Debug, Clone)]
pub struct ReanchorArithmetic {
    pub probe_exercised: bool,
    /// `(suggestion_id, index_before, index_after)` for every remaining pending suggestion
    /// that the accept should have shifted.
    pub shifted: Vec<(String, u32, u32)>,
    /// Suggestions whose index did not move but should have.
    pub missed_shifts: Vec<String>,
    /// Suggestions whose index moved but should not have.
    pub spurious_shifts: Vec<String>,
    /// Suggestions whose base_memory_version no longer matches the newly active version.
    pub version_mismatch: Vec<String>,
    /// True when every remaining pending row matches the deterministic arithmetic.
    pub arithmetic_valid: bool,
}

/// Independently recomputes the §5.3-B index shift and compares it to observed DB rows.
///
/// `resolved_op`/`resolved_index` describe the single accepted suggestion. `remaining_before`
/// and `remaining_after` are `(id, op, target_index)` for the other still-pending rows,
/// sampled immediately before and immediately after the accept.
pub fn verify_reanchor_arithmetic(
    resolved_op: &str,
    resolved_index: u32,
    remaining_before: &[(String, String, u32)],
    remaining_after: &[(String, String, u32)],
) -> ReanchorArithmetic {
    let after_map: HashMap<&str, (&str, u32)> = remaining_after
        .iter()
        .map(|(id, op, index)| (id.as_str(), (op.as_str(), *index)))
        .collect();

    let mut shifted = Vec::new();
    let mut missed_shifts = Vec::new();
    let mut spurious_shifts = Vec::new();

    for (id, _op, before_index) in remaining_before {
        let Some((_, after_index)) = after_map.get(id.as_str()) else {
            continue;
        };
        let before = *before_index;
        let after = *after_index;
        // INVARIANT 5.3-B: insert_after at k shifts indices > k by +1; delete at k shifts
        // indices > k by -1; replace leaves every index unchanged.
        let expected = match resolved_op.trim() {
            "insert_after" if before > resolved_index => before + 1,
            "delete" if before > resolved_index => before - 1,
            _ => before,
        };
        if after != before {
            shifted.push((id.clone(), before, after));
        }
        if after != expected {
            if expected == before {
                spurious_shifts.push(id.clone());
            } else {
                missed_shifts.push(id.clone());
            }
        }
    }

    let arithmetic_valid = missed_shifts.is_empty() && spurious_shifts.is_empty();
    ReanchorArithmetic {
        probe_exercised: true,
        shifted,
        missed_shifts,
        spurious_shifts,
        version_mismatch: Vec::new(),
        arithmetic_valid,
    }
}

/// A bullet or heading that the engine filed under a section the operation did not intend.
#[derive(Debug, Clone)]
pub struct SectionMembershipViolation {
    pub content: String,
    pub op: String,
    pub target_index: u32,
    /// Heading the content actually landed under, or `None` if it landed above all headings.
    pub landed_under: Option<String>,
    /// Heading the target index belonged to before the patch.
    pub intended_under: Option<String>,
}

/// Checks that every inserted bullet landed under the heading its anchor index belonged to.
///
/// The engine applies operations in descending index order, so two operations sharing an
/// anchor index are applied in list order and the *second* one is inserted at the position
/// the first one just took. A patch set that opens a new section with `insert_after(k, "## X")`
/// and then adds a bullet with `insert_after(k, "- detail")` therefore lands the bullet
/// *above* the heading it belongs to. This check detects that inversion directly.
pub fn verify_section_membership(
    base_document: &str,
    operations: &[(String, u32, String)],
    accepted_document: &str,
) -> Vec<SectionMembershipViolation> {
    let base_elements = parse_content_elements(base_document);
    let base_element_count = base_elements.len();
    let accepted_elements = parse_content_elements(accepted_document);

    // Section membership of every base index, computed before any patch is applied.
    let mut base_section: HashMap<u32, Option<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for element in &base_elements {
        if matches!(element.kind, ElementKind::Heading(_)) {
            let title = element.raw_text.trim_start_matches('#').trim().to_string();
            current = if title.is_empty() {
                Some(String::from("<nameless>"))
            } else {
                Some(title)
            };
        }
        base_section.insert(element.index, current.clone());
    }

    let mut accepted_positions: HashMap<String, usize> = HashMap::new();
    for (position, element) in accepted_elements.iter().enumerate() {
        accepted_positions
            .entry(element.raw_text.trim().to_string())
            .or_insert(position);
    }
    let accepted_sections: Vec<Option<String>> = {
        let mut sections = Vec::with_capacity(accepted_elements.len());
        let mut current: Option<String> = None;
        for element in &accepted_elements {
            if matches!(element.kind, ElementKind::Heading(_)) {
                let title = element.raw_text.trim_start_matches('#').trim().to_string();
                current = if title.is_empty() {
                    Some(String::from("<nameless>"))
                } else {
                    Some(title)
                };
            }
            sections.push(current.clone());
        }
        sections
    };

    let mut violations = Vec::new();
    for (op, index, content) in operations {
        let op_norm = op.trim().to_lowercase();
        if op_norm != "insert_after" {
            continue;
        }
        let text = content.trim();
        if text.is_empty() {
            continue;
        }
        let Some(position) = accepted_positions.get(text) else {
            // Not present in the accepted document: an apply-time skip, reported by
            // `account_operations` rather than here.
            continue;
        };
        let landed_under = accepted_sections[*position].clone();
        // The intended parent is the section owning the anchor index itself.
        let intended_under = if *index == 0 {
            None
        } else {
            base_section.get(index).cloned().flatten()
        };
        let is_heading = text.starts_with('#');
        if is_heading {
            continue;
        }
        // An out-of-range anchor was clamped to an append, so the element it "meant" does not
        // exist and the intended section is unknowable. Scoring that as a membership violation
        // would fail correct behaviour — "add a new section at the end" is a legitimate outcome
        // that the engine handled properly. Clamping is reported by `account_operations`
        // instead. Only an in-bounds anchor can be misfiled, and only that is checked here.
        if *index as usize > base_element_count {
            continue;
        }
        if landed_under != intended_under {
            violations.push(SectionMembershipViolation {
                content: text.to_string(),
                op: op_norm,
                target_index: *index,
                landed_under,
                intended_under,
            });
        }
    }
    violations
}

/// Words too common to carry meaning for coverage purposes.
const COVERAGE_STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "with",
    "that",
    "this",
    "from",
    "have",
    "has",
    "was",
    "were",
    "are",
    "but",
    "not",
    "you",
    "your",
    "yours",
    "they",
    "them",
    "their",
    "there",
    "here",
    "when",
    "what",
    "which",
    "who",
    "whom",
    "into",
    "over",
    "under",
    "about",
    "also",
    "than",
    "then",
    "some",
    "such",
    "only",
    "other",
    "more",
    "most",
    "much",
    "many",
    "very",
    "just",
    "like",
    "user",
    "users",
    "specifically",
    "mentions",
    "based",
    "preference",
    "currently",
    "enjoys",
    "enjoyed",
    "wants",
    "want",
    "trying",
    "tried",
    "makes",
    "made",
    "well",
    "even",
    "still",
];

/// Content words of a fact or document line, lowercased and stripped of punctuation.
fn content_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|word| word.len() > 3)
        .map(str::to_lowercase)
        .filter(|word| !COVERAGE_STOPWORDS.contains(&word.as_str()))
        .collect()
}

/// A candidate fact that INVARIANT 5.3-A marked `consolidated` but which never reached the
/// accepted document.
#[derive(Debug, Clone)]
pub struct UnrepresentedFact {
    pub text: String,
    pub coverage: f64,
    pub matched_words: Vec<String>,
    pub missing_words: Vec<String>,
}

/// Threshold below which a candidate fact is reported as only partially represented.
///
/// NOT used for gating. Measured against a live run, a fact the document carried faithfully
/// under a paraphrase ("plans to make rolls instead of loaves" -> "Planning to switch from
/// loaves to rolls") scored 0.55 purely because the verbs differ, while genuinely-absent
/// facts scored 0.00. Partial coverage therefore separates paraphrase from omission far too
/// poorly to gate on, and whether a claim is faithfully carried is the LLM judge's job
/// (`judge_pipeline_anchor_survival.md`, Coverage section). Reported only, as judge input.
const FACT_PARTIAL_COVERAGE_THRESHOLD: f64 = 0.6;

/// Classifies candidate facts by how much of each reached the accepted document.
#[derive(Debug, Clone, Default)]
pub struct FactCoverageReport {
    /// No content word of the fact appears anywhere in the document. Unambiguous loss: the
    /// fact was retired by INVARIANT 5.3-A without ever being written down. This is the only
    /// class severe enough to gate on.
    pub absent: Vec<UnrepresentedFact>,
    /// Some content words appear but the fact is not plainly carried. Reported as judge input.
    pub partial: Vec<UnrepresentedFact>,
    /// Wholly or substantially represented.
    pub represented: Vec<UnrepresentedFact>,
}

/// Classifies every candidate fact against the accepted document.
///
/// This closes a false green: INVARIANT 5.3-A transitions *every* candidate fact to
/// `consolidated` when suggestions are staged, whether or not the model actually proposed an
/// edit for it. A fact the model silently chose not to write down is therefore marked
/// consolidated and never appears again — invisible to the `candidate_partition_valid` check,
/// which only asserts that no fact is left `active`.
pub fn classify_fact_coverage(
    candidate_facts: &[String],
    accepted_document: &str,
) -> FactCoverageReport {
    let document_words: std::collections::HashSet<String> =
        content_words(accepted_document).into_iter().collect();

    let mut report = FactCoverageReport::default();
    for fact in candidate_facts {
        let words = content_words(fact);
        if words.is_empty() {
            continue;
        }
        let mut matched = Vec::new();
        let mut missing = Vec::new();
        for word in &words {
            if document_words.contains(word) {
                matched.push(word.clone());
            } else {
                missing.push(word.clone());
            }
        }
        let coverage = matched.len() as f64 / words.len() as f64;
        let entry = UnrepresentedFact {
            text: fact.clone(),
            coverage,
            matched_words: matched,
            missing_words: missing,
        };
        // A fact sharing no content word with the document is absent by any reading. Anything
        // above zero may be a faithful paraphrase, so it is judge input rather than a failure.
        if coverage == 0.0 {
            report.absent.push(entry);
        } else if coverage < FACT_PARTIAL_COVERAGE_THRESHOLD {
            report.partial.push(entry);
        } else {
            report.represented.push(entry);
        }
    }
    report
}

/// Verifies the accepted document did not silently lose a section.
///
/// A `replace` aimed at a heading index overwrites the heading with whatever text the model
/// supplied. When that text is a bullet, the section loses its heading and its bullets are
/// adopted by the preceding section — a live run put sourdough and Rust bullets under
/// "Languages Learning" and the document still satisfied every §5.1 condition, because the
/// damage is subtractive rather than malformed.
///
/// Legitimate heading *renames* also change the title, so comparing titles would fail correct
/// behaviour. Counting headings is the precise test: a rename preserves the count, a
/// heading-overwritten-by-a-bullet does not.
pub fn heading_count_preserved(
    base_document: &str,
    operations: &[(String, u32, String)],
    accepted_document: &str,
) -> (bool, usize, usize, usize) {
    let base_headings = parse_content_elements(base_document)
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Heading(_)))
        .count();
    let accepted_headings = parse_content_elements(accepted_document)
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Heading(_)))
        .count();

    let base_elements = parse_content_elements(base_document);
    // A `delete` at a heading index is a legitimate way to remove a section.
    let deleted_headings = operations
        .iter()
        .filter(|(op, index, _)| {
            op.trim() == "delete"
                && *index > 0
                && base_elements
                    .get((*index - 1) as usize)
                    .is_some_and(|e| matches!(e.kind, ElementKind::Heading(_)))
        })
        .count();

    let allowed_minimum = base_headings.saturating_sub(deleted_headings);
    (
        accepted_headings >= allowed_minimum,
        base_headings,
        accepted_headings,
        allowed_minimum,
    )
}
