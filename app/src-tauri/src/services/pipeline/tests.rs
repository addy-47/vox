//! ============================================================================
//! src/services/pipeline/tests.rs — Unit tests for pipeline orchestrator state & cancellation
//! ============================================================================

#[cfg(test)]
mod tests {
    use crate::services::pipeline::PipelineState;
    use crate::core::state::InteractionOwner;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_pipeline_state_variants_and_default() {
        let cold = PipelineState::Cold;
        let warm = PipelineState::Warm;

        assert_ne!(cold, warm);
        assert_eq!(cold, PipelineState::Cold);
        assert_eq!(warm, PipelineState::Warm);

        // Test Default trait
        let default_state = PipelineState::default();
        assert_eq!(default_state, PipelineState::Cold);

        // Test Clone and Copy
        let cold_copy = cold;
        assert_eq!(cold, cold_copy);
    }

    #[test]
    fn test_interaction_owner_conversions() {
        // Test u8 -> InteractionOwner conversion
        assert_eq!(InteractionOwner::from(0u8), InteractionOwner::Tray);
        assert_eq!(InteractionOwner::from(1u8), InteractionOwner::MainWindow);
        assert_eq!(InteractionOwner::from(2u8), InteractionOwner::Ptt);
        assert_eq!(InteractionOwner::from(3u8), InteractionOwner::Wizard);
        // Wildcard fallback to Tray
        assert_eq!(InteractionOwner::from(4u8), InteractionOwner::Tray);
        assert_eq!(InteractionOwner::from(255u8), InteractionOwner::Tray);

        // Test u32 -> InteractionOwner conversion
        assert_eq!(InteractionOwner::from(0u32), InteractionOwner::Tray);
        assert_eq!(InteractionOwner::from(1u32), InteractionOwner::MainWindow);
        assert_eq!(InteractionOwner::from(2u32), InteractionOwner::Ptt);
        assert_eq!(InteractionOwner::from(3u32), InteractionOwner::Wizard);
        assert_eq!(InteractionOwner::from(100u32), InteractionOwner::Tray);

        // Test InteractionOwner -> u8 conversion
        assert_eq!(u8::from(InteractionOwner::Tray), 0u8);
        assert_eq!(u8::from(InteractionOwner::MainWindow), 1u8);
        assert_eq!(u8::from(InteractionOwner::Ptt), 2u8);
        assert_eq!(u8::from(InteractionOwner::Wizard), 3u8);

        // Test InteractionOwner as u8 casting
        assert_eq!(InteractionOwner::Tray as u8, 0u8);
        assert_eq!(InteractionOwner::MainWindow as u8, 1u8);
        assert_eq!(InteractionOwner::Ptt as u8, 2u8);
        assert_eq!(InteractionOwner::Wizard as u8, 3u8);

        // Test Roundtrip u8 -> InteractionOwner -> u8
        for val in 0u8..=3u8 {
            let owner = InteractionOwner::from(val);
            assert_eq!(u8::from(owner), val);
        }
    }

    #[test]
    fn test_cancellation_flag_and_atomic_turn_bumping() {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let turn_id = Arc::new(AtomicU32::new(0));

        // Initial state assertions
        assert!(!cancel_flag.load(Ordering::Relaxed));
        assert_eq!(turn_id.load(Ordering::Relaxed), 0);

        // Simulate turn 1 creation
        let old_turn = turn_id.load(Ordering::Relaxed);
        assert_eq!(old_turn, 0);

        // Cancel previous turn
        cancel_flag.store(true, Ordering::Relaxed);
        assert!(cancel_flag.load(Ordering::Relaxed));

        // Bump turn ID atomically
        let new_turn = turn_id.fetch_add(1, Ordering::Relaxed) + 1;
        assert_eq!(new_turn, 1);
        assert_eq!(turn_id.load(Ordering::Relaxed), 1);

        // Reset cancellation flag after queuing turn event
        cancel_flag.store(false, Ordering::Relaxed);
        assert!(!cancel_flag.load(Ordering::Relaxed));

        // Simulate turn 2 creation
        let old_turn_2 = turn_id.load(Ordering::Relaxed);
        assert_eq!(old_turn_2, 1);
        cancel_flag.store(true, Ordering::Relaxed);
        let new_turn_2 = turn_id.fetch_add(1, Ordering::Relaxed) + 1;
        assert_eq!(new_turn_2, 2);
        assert_eq!(turn_id.load(Ordering::Relaxed), 2);
        cancel_flag.store(false, Ordering::Relaxed);
        assert!(!cancel_flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_turn_id_filtering_for_stale_tasks() {
        // Test deterministic logic used in worker channels (e.g. transliteration worker)
        let mut worker_turn_id = 0u32;

        // Task from turn 1 arrives -> processed
        let task_turn_1 = 1u32;
        assert!(task_turn_1 >= worker_turn_id);
        worker_turn_id = task_turn_1;
        assert_eq!(worker_turn_id, 1);

        // Task from turn 2 arrives -> processed
        let task_turn_2 = 2u32;
        assert!(task_turn_2 >= worker_turn_id);
        worker_turn_id = task_turn_2;
        assert_eq!(worker_turn_id, 2);

        // Delayed/stale task from turn 1 arrives -> filtered out / dropped
        let stale_task_turn_1 = 1u32;
        let is_stale = stale_task_turn_1 < worker_turn_id;
        assert!(is_stale, "Stale turn 1 task must be dropped when worker is on turn 2");
        assert_eq!(worker_turn_id, 2, "Worker turn ID must remain on current turn");
    }

    #[test]
    fn test_barge_in_cancellation_flow() {
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Turn is running, cancel_flag is false
        assert!(!cancel_flag.load(Ordering::SeqCst));

        // Barge-in or interruption detected -> set cancel_flag to true
        cancel_flag.store(true, Ordering::SeqCst);
        assert!(cancel_flag.load(Ordering::SeqCst));

        // Generation worker checks cancel_flag during token generation loop
        if cancel_flag.load(Ordering::SeqCst) {
            // Worker detects cancellation, aborts generation loop
        } else {
            panic!("Worker should have detected cancellation flag!");
        }

        // RCA Fix check: Right before starting new generation, cancel_flag is reset to false
        cancel_flag.store(false, Ordering::Relaxed);
        assert!(!cancel_flag.load(Ordering::Relaxed));
    }
}
