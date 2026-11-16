use std::{sync::mpsc, time::Instant};

use tokio_util::sync::CancellationToken;

use crate::services::llm::{
    actor::{LlmCommand, LlmResponse},
    GenerationRequest,
};

pub type LlmSender = mpsc::Sender<LlmCommand>;

/// Dispatches an LlmCommand::Generate command over the actor channel.
pub fn dispatch_llm_generation(
    llm_tx: Option<&mpsc::Sender<LlmCommand>>,
    request: GenerationRequest,
    turn_id: u32,
    cancel: &CancellationToken,
) -> Result<mpsc::Receiver<LlmResponse>, String> {
    let Some(tx) = llm_tx else {
        return Err("No LLM channel available".to_string());
    };
    let start_dispatch = Instant::now();
    let (response_tx, response_rx) = mpsc::channel();
    let cmd = LlmCommand::Generate {
        request: Box::new(request),
        turn_id,
        cancel: cancel.clone(),
        response_tx,
    };
    tx.send(cmd)
        .map_err(|e| format!("Failed to dispatch LlmCommand: {}", e))?;
    log::info!(
        "[Harness::Dispatch] Turn {} dispatched to LLM actor in {}ms",
        turn_id,
        start_dispatch.elapsed().as_millis()
    );
    Ok(response_rx)
}
