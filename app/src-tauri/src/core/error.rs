use std::path::PathBuf;
use thiserror::Error;

/// Master unified error hierarchy for the Vox application.
#[derive(Error, Debug)]
pub enum VoxError {
    #[error("Audio subsystem error: {0}")]
    Audio(#[from] AudioError),

    #[error("STT engine error: {0}")]
    Stt(#[from] SttError),

    #[error("TTS engine error: {0}")]
    Tts(#[from] TtsError),

    #[error("LLM engine error: {0}")]
    Llm(#[from] LlmError),

    #[error("Memory subsystem error: {0}")]
    Memory(#[from] MemoryError),

    #[error("Persistence error: {0}")]
    Persistence(#[from] PersistenceError),

    #[error("Dictation error: {0}")]
    Dictation(#[from] DictationError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Audio subsystem domain error types.
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Device initialization failed: {message}")]
    DeviceInitFailed { message: String },

    #[error("Stream creation failed: {message}")]
    StreamError { message: String },

    #[error("Unsupported sample format or rate: {0}")]
    InvalidFormat(String),
}

/// Speech-to-Text (STT) domain error types.
#[derive(Error, Debug)]
pub enum SttError {
    #[error("Model asset missing at path: {path}")]
    ModelMissing { path: PathBuf },

    #[error("ASR engine initialization failed: {message}")]
    InitFailed { message: String },

    #[error("Transcription inference failed: {message}")]
    TranscriptionFailed { message: String },
}

/// Text-to-Speech (TTS) domain error types.
#[derive(Error, Debug)]
pub enum TtsError {
    #[error("TTS model asset missing at path: {path}")]
    ModelMissing { path: PathBuf },

    #[error("TTS synthesis failed: {message}")]
    SynthesisFailed { message: String },

    #[error("Mutex lock poisoned on TTS provider: {0}")]
    LockPoisoned(String),
}

/// Large Language Model (LLM) domain error types.
#[derive(Error, Debug)]
pub enum LlmError {
    #[error("GGUF model file not found at path: {path}")]
    ModelNotFound { path: PathBuf },

    #[error("llama.cpp backend error: {message}")]
    BackendError { message: String },

    #[error("Context window budget exhausted")]
    ContextOverflow,
}

/// Cognitive Memory subsystem domain error types.
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Model asset missing at path: {path}")]
    MissingAsset { path: PathBuf },

    #[error("ONNX inference failure: {message}")]
    InferenceFailed { message: String },

    #[error("Tokenizer error: {message}")]
    TokenizerError { message: String },

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Worker background queue error: {0}")]
    WorkerError(String),
}

/// Database & Persistence domain error types.
#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("Turso/SQLite database error: {0}")]
    Database(#[from] turso::Error),

    #[error("I/O error in persistence: {0}")]
    Io(#[from] std::io::Error),

    #[error("Schema migration failed for version {version}: {message}")]
    MigrationError { version: u32, message: String },

    #[error("Record not found: {id}")]
    NotFound { id: String },
}

/// Dictation subsystem domain error types.
#[derive(Error, Debug)]
pub enum DictationError {
    #[error("Clipboard operation failed: {message}")]
    ClipboardFailed { message: String },

    #[error("Input simulation failed: {message}")]
    InputSimulationFailed { message: String },

    #[error("Global hotkey registration failed: {message}")]
    HotkeyRegistrationFailed { message: String },

    #[error("Dictation engine not ready: {message}")]
    EngineNotReady { message: String },
}
