use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    SessionStarted {
        id: u64,
        timestamp_ms: u64,
    },
    SessionEnded {
        id: u64,
        timestamp_ms: u64,
    },
    TurnCompleted {
        conversation_id: u64,
        turn_id: u32,
        user_text: String,
        assistant_text: String,
        stt_latency_ms: u32,
        ttft_ms: u32,
    },
    TurnCancelled {
        conversation_id: u64,
        turn_id: u32,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum MemoryWorkerEvent {
    SessionEnd {
        session_id: String,
        summary: String,
    },
    PersonalFactsReady {
        facts: HashMap<String, Vec<String>>,
        session_id: String,
    },
    ActiveSessionChanged {
        session_id: u64,
    },
    Shutdown,
}
