pub mod actor;
pub mod factory;
pub mod providers;
pub mod segmenter;

pub use actor::{
    spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles, VadCommand,
    VadOperationalMode, VadValidationResult,
};
pub use factory::create_vad_instance_from_settings;
pub use providers::{
    earshot_vad, silero_onnx, ten_onnx, EarshotVadEngine, SileroVadEngine, TenVadEngine,
    VadBackend, VadEngine,
};
pub use segmenter::PreRollBuffer;

pub const MODEL_DIR_VAD: &str = "vad";
pub const MODEL_FILE_VAD: &str = "ten_vad.onnx";
pub const MODEL_FILE_VAD_SILERO: &str = "silero_vad.onnx";

/// Input sample rate (Hz) of audio frames evaluated by the VAD actor.
pub const VAD_INPUT_SAMPLE_RATE: u32 = 16000;
pub const VAD_CHUNK_SIZE: usize = 256;
pub const VAD_PRE_ROLL_CAPACITY: usize = 8000;
pub const VAD_SPEECH_START_FRAMES: usize = 2;
pub const VAD_SPEECH_END_FRAMES: usize = 25;
pub const VAD_MIN_UTTERANCE_SAMPLES: usize = 4800;
pub const VAD_PARTIAL_INTERVAL_SAMPLES: usize = 12800;
pub const VAD_MAX_PARTIAL_WINDOW_SAMPLES: usize = 240000;

/// Idle sleep duration for the synchronous VAD actor thread when the ring buffer lacks a full chunk.
pub const VAD_ACTOR_IDLE_SLEEP_MS: u64 = 5;

/// Maximum timeout when awaiting synchronous windowed speech validation from the VAD actor.
pub const VAD_VALIDATION_TIMEOUT_MS: u64 = 500;

/// Earshot pure-Rust energy model noise gate calibration multiplier to compensate for dynamic range scale differences.
pub const EARSHOT_NOISE_GATE_MULTIPLIER: f32 = 1.5;
