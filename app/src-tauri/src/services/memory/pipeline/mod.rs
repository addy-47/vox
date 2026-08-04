pub mod batch_result;
pub mod metrics;
pub mod runner;
pub mod stage1_dedup;
pub mod stage2_embed;
pub mod stage3_eval;
pub mod stage4_commit;

pub use batch_result::{BatchEvaluationResult, RelationEdge};
pub use metrics::PipelineStageMetrics;
pub use runner::{drain_pipeline_queue, run_pipeline_cycle};
pub use stage1_dedup::run_stage1_dedup;
pub use stage2_embed::run_stage2_embed;
pub use stage3_eval::run_stage3_eval;
pub use stage4_commit::run_stage4_commit;
