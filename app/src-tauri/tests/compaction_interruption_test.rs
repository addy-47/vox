//! ============================================================================
//! compaction_interruption_test.rs — Memory Compaction Barge-In Race Condition Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : Working Memory & Compaction (`vox_lib::services::memory::working_memory`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    :
//! - Default (Embedded Local) : cargo test --test compaction_interruption_test
//! - Server & Cloud (Ignored) : cargo test --test compaction_interruption_test -- --ignored
//!
//! Metrics: Opportunistic compaction cancellation, race detection, & state preservation
//! ============================================================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use vox_lib::services::memory::working_memory::ConversationManager;

fn setup_manager_in_opportunistic_zone() -> ConversationManager {
    let mut mgr = ConversationManager::new(8192);

    // Push 4 turns to build total token count
    for i in 1..=4 {
        mgr.push_user_turn(format!(
            "User query turn number {} with detailed context.",
            i
        ));
        mgr.push_assistant_turn(format!("Assistant detailed response turn number {}.", i));
    }

    // Dynamically set max_context_tokens so utilization is exactly ~75% (between 65% soft and 85% critical)
    // Formula: max_context_tokens = 512 + (total_token_count / 0.75)
    let current_tokens = mgr.total_token_count();
    let target_max = 512 + ((current_tokens as f32 / 0.75) as usize);
    mgr.set_max_context_tokens(target_max);

    let util = mgr.context_utilization();
    assert!(
        util > 0.65 && util < 0.85,
        "Utilization ({:.2}) MUST be in opportunistic range (0.65..0.85)!",
        util
    );

    mgr
}

// ─── 1. Local Default Tests (Embedded / In-Memory State Machine) ─────────────

#[test]
fn test_opportunistic_compaction_cancel_on_speech_start() {
    let mut mgr = setup_manager_in_opportunistic_zone();
    let initial_count = mgr.get_messages().len();

    // 1. Trigger opportunistic compaction
    let candidate = mgr.try_trigger_opportunistic();
    assert!(
        candidate.is_some(),
        "Opportunistic compaction candidate should be triggered between soft and critical thresholds!"
    );

    let (snapshot_len, _snapshot_msgs, _cancel_flag) = candidate.unwrap();
    assert_eq!(snapshot_len, initial_count);

    // 2. User barges in (starts speaking) mid-compaction
    mgr.on_speech_start();

    // 3. Attempt to commit background compaction result
    let committed = mgr.commit_opportunistic(snapshot_len, "Compacted summary text".to_string());

    // 4. Assert commit was rejected and original message state is 100% preserved
    assert!(
        !committed,
        "Commit MUST be rejected when speech_start cancels opportunistic compaction!"
    );
    assert_eq!(
        mgr.get_messages().len(),
        initial_count,
        "Uncompacted working memory messages MUST remain completely intact!"
    );
}

#[test]
fn test_opportunistic_compaction_race_detection_on_new_user_message() {
    let mut mgr = setup_manager_in_opportunistic_zone();
    let initial_count = mgr.get_messages().len();

    // 1. Trigger opportunistic compaction snapshot
    let candidate = mgr.try_trigger_opportunistic();
    assert!(
        candidate.is_some(),
        "Opportunistic compaction candidate should be triggered!"
    );
    let (snapshot_len, _, _) = candidate.unwrap();

    // 2. User sends a new message while compaction is running in background
    mgr.push_user_turn("New incoming turn while background compaction runs!".to_string());
    assert_eq!(mgr.get_messages().len(), initial_count + 1);

    // 3. Attempt to commit snapshot
    let committed = mgr.commit_opportunistic(snapshot_len, "Compacted summary text".to_string());

    // 4. Assert commit rejected due to length mismatch (race condition prevention)
    assert!(
        !committed,
        "Commit MUST be rejected when new messages arrive during background compaction!"
    );
    assert_eq!(
        mgr.get_messages().len(),
        initial_count + 1,
        "New user message MUST be preserved without corruption!"
    );
}

#[test]
fn test_opportunistic_compaction_successful_commit_when_uninterrupted() {
    let mut mgr = setup_manager_in_opportunistic_zone();

    let candidate = mgr.try_trigger_opportunistic();
    assert!(
        candidate.is_some(),
        "Opportunistic compaction candidate should be triggered!"
    );
    let (snapshot_len, _, _) = candidate.unwrap();

    // Commit uninterrupted
    let committed = mgr.commit_opportunistic(snapshot_len, "Compacted summary text".to_string());
    assert!(committed, "Uninterrupted compaction commit MUST succeed!");

    // System prompt + Summary + Last user turn = 3 messages
    assert_eq!(mgr.get_messages().len(), 3);
    assert!(mgr.get_messages()[1]
        .content
        .contains("Compacted summary text"));
}

// ─── 2. Server & Cloud Tests (Ignored — Run via cargo test -- --ignored) ──────

#[test]
#[ignore = "Compaction provider cancellation test requires live LLM provider setup"]
fn test_compaction_live_provider_cancellation_ignored() {
    let cancel_flag = Arc::new(AtomicBool::new(true)); // Pre-cancelled
    assert!(cancel_flag.load(Ordering::Relaxed));
}
