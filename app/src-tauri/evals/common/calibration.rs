use std::collections::HashMap;

use anyhow::{Context, Result};

pub const CALIBRATION_ASSET: &str = "calibration/compaction_counts.json";

#[derive(Debug, Clone)]
pub struct CountCalibration {
    pub source: String,
    pub per_case: HashMap<String, u32>,
}

/// Per-model compaction-count baseline. Absent for a new executor model, in
/// which case counts are reported as a raw delta and never gate the run.
pub fn load_count_calibration(model: &str, context_window: u32) -> CountCalibration {
    let path = format!(
        "{}/evals/assets/{CALIBRATION_ASSET}",
        env!("CARGO_MANIFEST_DIR")
    );
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return CountCalibration {
            source: "uncalibrated:no_file".to_string(),
            per_case: HashMap::new(),
        };
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return CountCalibration {
            source: "uncalibrated:unparsable".to_string(),
            per_case: HashMap::new(),
        };
    };
    let entry = parsed
        .get("models")
        .and_then(|models| models.get(model))
        .and_then(|model_entry| {
            let window = model_entry
                .get("context_window")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX);
            if window != u64::from(context_window) {
                return None;
            }
            let counts = model_entry.get("counts")?.as_object()?;
            let per_case = counts
                .iter()
                .filter_map(|(case, value)| {
                    value.as_u64().map(|count| (case.clone(), count as u32))
                })
                .collect();
            Some(per_case)
        });

    match entry {
        Some(per_case) => CountCalibration {
            source: format!("calibrated:{model}"),
            per_case,
        },
        None => CountCalibration {
            source: format!("uncalibrated:{model}"),
            per_case: HashMap::new(),
        },
    }
}

/// Writes an unverified baseline proposal for the executor model. Never read by
/// the gate in the same run — a calibration must be reviewed before it becomes
/// the thing being compared against.
pub fn write_count_calibration(
    model: &str,
    context_window: u32,
    case_compactions: &[(String, u32)],
    source_run: &str,
) -> Result<()> {
    let path = format!(
        "{}/evals/assets/{CALIBRATION_ASSET}",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut root = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({ "note": "Model-specific compaction-count baselines. Counts depend on the executor model's summary size, not on pipeline correctness.", "models": {} }));

    let counts: serde_json::Map<String, serde_json::Value> = case_compactions
        .iter()
        .map(|(case, actual)| (case.clone(), serde_json::json!(actual)))
        .collect();

    if let Some(models) = root
        .get_mut("models")
        .and_then(serde_json::Value::as_object_mut)
    {
        models.insert(
            model.to_string(),
            serde_json::json!({
                "context_window": context_window,
                "source_run": source_run,
                "status": "unverified_proposal",
                "counts": counts,
            }),
        );
    }
    std::fs::write(&path, serde_json::to_string_pretty(&root)?)
        .with_context(|| format!("Failed to write calibration to {path}"))?;
    println!("[calibration] wrote unverified baseline proposal for {model}");
    Ok(())
}
