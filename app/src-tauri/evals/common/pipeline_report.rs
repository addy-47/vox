use std::path::Path;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::Serialize;
use turso::Connection;
use vox_lib::core::defaults::DEFAULT_LLM_MAX_OUTPUT_TOKENS;

use super::report;

#[derive(Debug, Clone, Serialize)]
pub struct SuggestionObservation {
    pub op: String,
    pub target_index: u32,
    pub content: String,
    /// False when the op carries no usable content (replace or insert with empty content)
    pub engine_would_match: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JudgeObservation {
    pub model: String,
    pub verdict: String,
    pub latency_s: f64,
    pub coverage_score: Option<u32>,
    pub quality_score: Option<u32>,
    pub groundedness_score: Option<u32>,
    pub anchor_survival_score: Option<u32>,
    pub claimed_anchor_losses: Vec<String>,
    pub claimed_inventions: Vec<String>,
    pub report_markdown: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsolidationObservation {
    pub base_memory_version: i64,
    pub staged_suggestions: u32,
    pub candidate_facts: i64,
    pub candidate_fact_texts: Vec<String>,
    pub document_unchanged_after_staging: bool,
    pub candidate_partition_valid: bool,
    pub pending_anchored_to_base_version: bool,
    pub operations: Vec<SuggestionObservation>,
    pub skipped_operations: u32,
    pub reanchor_probe_exercised: bool,
    pub reanchor_valid: Option<bool>,
    pub accepted_memory_version: i64,
    pub expected_memory_version: i64,
    pub untargeted_lines_total: usize,
    pub untargeted_lines_preserved: usize,
    pub untargeted_preservation_valid: bool,
    pub unexplained_lost_lines: Vec<String>,
    pub suggestions_accepted: u32,
    pub linked_facts_consolidated: u32,
    pub linked_facts_rejected: u32,
    pub facts_left_active: i64,
    pub reject_probe: bool,
    pub document_unchanged_after_reject: bool,
    pub judge: Option<JudgeObservation>,
}

#[derive(Debug, Serialize)]
pub struct TurnTelemetry {
    pub turn: u32,
    pub tracked_tokens: usize,
    pub usable_budget: usize,
    pub utilization_percent: f32,
    pub budget_status: String,
    pub near_miss: bool,
    pub critical_eligible: bool,
    pub history_messages: usize,
    pub compaction_triggered: bool,
}

#[derive(Debug, Clone)]
pub struct ConsolidationOutcome {
    pub observation: ConsolidationObservation,
    pub accepted_document: String,
    pub latency_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct IngestionCycleReport {
    pub stage1_processed: usize,
    pub stage1_duplicates_deactivated: usize,
    pub stage1_errors: usize,
    pub stage2_processed: usize,
    pub stage2_inserted: usize,
    pub stage2_duplicates_deactivated: usize,
    pub stage2_errors: usize,
    pub latency_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct CompactionObservation {
    pub from_turn: u32,
    pub to_turn: u32,
    pub trigger_kind: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct CaseReport {
    pub case: String,
    pub expected_compactions_calibrated: Option<u32>,
    pub count_calibration_delta: Option<i64>,
    pub count_calibration_source: String,
    pub actual_compactions: u32,
    pub manual_compactions: u32,
    pub trigger_correctness_valid: bool,
    pub session_id: i64,
    pub session_start_ms: i64,
    pub input_turns: usize,
    pub crossing_turns: Vec<u32>,
    pub critical_turns: Vec<u32>,
    pub budget_invariants_valid: bool,
    pub compaction_latencies_ms: Vec<u128>,
    pub compaction_ledger: Vec<CompactionObservation>,
    pub ledger_ranges_contiguous: bool,
    pub final_watermark_valid: bool,
    pub empty_context_compactions: u32,
    pub ingestion_cycles: u32,
    pub ingestion_items_processed: usize,
    pub stage1_items_processed: usize,
    pub stage2_items_processed: usize,
    pub ingestion_items_inserted: usize,
    pub queue_accounting_complete: bool,
    pub ingestion_accounting_valid: bool,
    pub ingestion_cycle_reports: Vec<IngestionCycleReport>,
    pub consolidation_latency_ms: u128,
    pub queue_before: i64,
    pub queue_after_compaction: i64,
    pub queue_after: i64,
    pub failed_queue_items: i64,
    pub facts_before: i64,
    pub facts_after: i64,
    pub personal_memory_version_before: i64,
    pub personal_memory_chars_before: usize,
    pub personal_memory_chars_after: usize,
    pub personal_memory_injected: bool,
    pub personal_facts_before_consolidation: i64,
    pub memory_consolidation_exercised: bool,
    pub consolidation: Option<ConsolidationObservation>,
    pub base_document: String,
    pub accepted_document: String,
    pub turn_telemetry: Vec<TurnTelemetry>,
    pub status: String,
    pub error: Option<String>,
}

pub fn failed_setup_report(error: &str) -> CaseReport {
    CaseReport {
        case: "<setup>".to_string(),
        expected_compactions_calibrated: None,
        count_calibration_delta: None,
        count_calibration_source: "unavailable".to_string(),
        actual_compactions: 0,
        manual_compactions: 0,
        trigger_correctness_valid: false,
        session_id: 0,
        session_start_ms: 0,
        input_turns: 0,
        crossing_turns: Vec::new(),
        critical_turns: Vec::new(),
        budget_invariants_valid: false,
        compaction_latencies_ms: Vec::new(),
        compaction_ledger: Vec::new(),
        ledger_ranges_contiguous: false,
        final_watermark_valid: false,
        empty_context_compactions: 0,
        ingestion_cycles: 0,
        ingestion_items_processed: 0,
        stage1_items_processed: 0,
        stage2_items_processed: 0,
        ingestion_items_inserted: 0,
        queue_accounting_complete: false,
        ingestion_accounting_valid: false,
        ingestion_cycle_reports: Vec::new(),
        consolidation_latency_ms: 0,
        queue_before: 0,
        queue_after_compaction: 0,
        queue_after: 0,
        failed_queue_items: 0,
        facts_before: 0,
        facts_after: 0,
        personal_memory_version_before: 0,
        personal_memory_chars_before: 0,
        personal_memory_chars_after: 0,
        personal_memory_injected: false,
        personal_facts_before_consolidation: 0,
        memory_consolidation_exercised: false,
        consolidation: None,
        base_document: String::new(),
        accepted_document: String::new(),
        turn_telemetry: Vec::new(),
        status: "failed_setup".to_string(),
        error: Some(error.to_string()),
    }
}

pub fn failed_case_report(case_name: &str, input_turns: usize, error: &str) -> CaseReport {
    CaseReport {
        case: case_name.to_string(),
        expected_compactions_calibrated: None,
        count_calibration_delta: None,
        count_calibration_source: "unavailable".to_string(),
        actual_compactions: 0,
        manual_compactions: 0,
        trigger_correctness_valid: false,
        session_id: 0,
        session_start_ms: 0,
        input_turns,
        crossing_turns: Vec::new(),
        critical_turns: Vec::new(),
        budget_invariants_valid: false,
        compaction_latencies_ms: Vec::new(),
        compaction_ledger: Vec::new(),
        ledger_ranges_contiguous: false,
        final_watermark_valid: false,
        empty_context_compactions: 0,
        ingestion_cycles: 0,
        ingestion_items_processed: 0,
        stage1_items_processed: 0,
        stage2_items_processed: 0,
        ingestion_items_inserted: 0,
        queue_accounting_complete: false,
        ingestion_accounting_valid: false,
        ingestion_cycle_reports: Vec::new(),
        consolidation_latency_ms: 0,
        queue_before: 0,
        queue_after_compaction: 0,
        queue_after: 0,
        failed_queue_items: 0,
        facts_before: 0,
        facts_after: 0,
        personal_memory_version_before: 0,
        personal_memory_chars_before: 0,
        personal_memory_chars_after: 0,
        personal_memory_injected: false,
        personal_facts_before_consolidation: 0,
        memory_consolidation_exercised: false,
        consolidation: None,
        base_document: String::new(),
        accepted_document: String::new(),
        turn_telemetry: Vec::new(),
        status: "failed".to_string(),
        error: Some(error.to_string()),
    }
}

pub fn case_start_ms(case_index: usize) -> i64 {
    let day = NaiveDate::from_ymd_opt(2026, 1, 2)
        .expect("valid eval date")
        .checked_add_signed(ChronoDuration::days(case_index as i64))
        .expect("valid eval date range");
    let hour = 7;
    day.and_hms_opt(hour, 0, 0)
        .expect("valid eval time")
        .and_utc()
        .timestamp_millis()
}

pub async fn fetch_compaction_observations(
    conn: &Connection,
    session_id: i64,
) -> Result<Vec<CompactionObservation>> {
    let mut rows = conn
        .query(
            "SELECT from_turn_id, to_turn_id, trigger_kind, status FROM session_compactions WHERE session_id = ? ORDER BY id ASC;",
            (session_id,),
        )
        .await?;
    let mut observations = Vec::new();
    while let Some(row) = rows.next().await? {
        observations.push(CompactionObservation {
            from_turn: row.get::<i64>(0)? as u32,
            to_turn: row.get::<i64>(1)? as u32,
            trigger_kind: row.get(2)?,
            status: row.get(3)?,
        });
    }
    Ok(observations)
}

pub fn validate_compaction_ranges(observations: &[CompactionObservation]) -> bool {
    let mut previous_to = 0;
    for observation in observations {
        if !matches!(observation.trigger_kind.as_str(), "critical" | "manual")
            || observation.status != "completed"
            || observation.from_turn != previous_to + 1
        {
            return false;
        }
        previous_to = observation.to_turn;
    }
    true
}

#[derive(Debug, Clone)]
pub struct FinalReportConfig<'a> {
    pub eval_name: &'a str,
    pub run_id: &'a str,
    pub server_url: &'a str,
    pub server_model: &'a str,
    pub server_provider: &'a str,
    pub db_path: &'a Path,
    pub dataset_dir: &'a Path,
    pub context_window: u32,
    pub no_judge: bool,
    pub judge_model: Option<&'a str>,
    pub judge_url: Option<&'a str>,
    pub is_subset: bool,
    pub run_passed: bool,
    pub total_latency_s: f64,
    pub config_source: &'a str,
}

pub fn write_final_report(cfg: &FinalReportConfig, reports: &[CaseReport]) -> Result<()> {
    let payload = serde_json::json!({
        "eval": cfg.eval_name,
        "status": if reports
            .iter()
            .any(|report| report.status == "failed_setup")
        {
            "failed_setup"
        } else if cfg.is_subset {
            if cfg.run_passed && reports.iter().all(|report| report.status == "passed") {
                "passed_subset"
            } else {
                "failed_subset"
            }
        } else if cfg.run_passed && reports.iter().all(|report| report.status == "passed") {
            "passed"
        } else {
            "failed"
        },
        "complete_matrix": !cfg.is_subset,
        "run_passed": cfg.run_passed,
        "case_count": reports.len(),
        "memory_pipeline_exercised": reports
            .iter()
            .any(|report| report.memory_consolidation_exercised),
        "ingestion_items_inserted": reports
            .iter()
            .map(|report| report.ingestion_items_inserted)
            .sum::<usize>(),
        "dataset_dir": cfg.dataset_dir,
        "server": {
            "url": cfg.server_url,
            "model": cfg.server_model,
            "provider": cfg.server_provider,
        },
        "database": cfg.db_path,
        "context_window": cfg.context_window,
        "max_output_tokens": DEFAULT_LLM_MAX_OUTPUT_TOKENS,
        "auto_compaction": true,
        "retrieval_enabled": false,
        "tools_enabled": false,
        "judge": {
            "enabled": !cfg.no_judge,
            "model": cfg.judge_model.unwrap_or(cfg.server_model),
            "url": cfg.judge_url.unwrap_or(cfg.server_url),
            "gates_run": false,
        },
        "count_calibration": {
            "source": cfg.config_source,
            "semantics": "Compaction counts are model-specific calibration data. Trigger correctness (every compaction fired at a critical turn, contiguous ledger, valid watermark) is the model-independent gate.",
        },
        "consolidation_contract": "LLM proposes atomic patch operations; staging is non-mutating; the document changes only via the deterministic patch engine on accept.",
        "cases": reports,
        "total_latency_s": cfg.total_latency_s,
    });
    report::write_report(cfg.eval_name, cfg.run_id, payload)?;
    Ok(())
}
