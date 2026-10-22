use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;

use super::{supertonic::TtsEngine as SupertonicEngine, SynthesisContext, TtsProvider};
use crate::{
    core::{
        compute::{self, Execution, Workload},
        defaults::DEFAULT_TTS_SUPERTONIC_VOICE_ID,
    },
    services::tts::voice::{get_supertonic_voice_profiles, resolve_catalog_index},
};

pub const DEVANAGARI_ROUTE_PROVIDER: &str = "supertonic";

static LIVE_DEVANAGARI_ENGINES: AtomicUsize = AtomicUsize::new(0);

pub fn live_devanagari_engine_count() -> usize {
    LIVE_DEVANAGARI_ENGINES.load(Ordering::Relaxed)
}

struct CachedEngine {
    model_path: PathBuf,
    engine: SupertonicEngine,
}

pub struct DevanagariEngineCache {
    engine: Mutex<Option<CachedEngine>>,
    builds: AtomicUsize,
}

impl DevanagariEngineCache {
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(None),
            builds: AtomicUsize::new(0),
        }
    }

    pub fn build_count(&self) -> usize {
        self.builds.load(Ordering::Relaxed)
    }

    pub fn synthesize(
        &self,
        text: &str,
        ctx: &SynthesisContext<'_>,
        model_path: &Path,
        speed: f32,
    ) -> Result<()> {
        let mut guard = self.engine.lock();
        let stale = guard
            .as_ref()
            .is_some_and(|cached| cached.model_path.as_path() != model_path);
        if stale || guard.is_none() {
            if guard.take().is_some() {
                LIVE_DEVANAGARI_ENGINES.fetch_sub(1, Ordering::Relaxed);
            }
            let engine = self.build_engine(model_path, speed)?;
            self.builds.fetch_add(1, Ordering::Relaxed);
            LIVE_DEVANAGARI_ENGINES.fetch_add(1, Ordering::Relaxed);
            *guard = Some(CachedEngine {
                model_path: model_path.to_path_buf(),
                engine,
            });
        }

        let cached = guard
            .as_ref()
            .expect("the engine was just placed in the cache");

        cached.engine.set_speed(speed);
        cached.engine.synthesize_chunk(text, ctx)
    }

    fn build_engine(&self, model_path: &Path, speed: f32) -> Result<SupertonicEngine> {
        let voice = resolve_catalog_index(
            &get_supertonic_voice_profiles(),
            DEFAULT_TTS_SUPERTONIC_VOICE_ID,
        );
        let threads = compute::resolve_threads(Workload::Tts, Execution::Embedded);
        SupertonicEngine::new(model_path, voice, speed, threads).map_err(|e| {
            anyhow!(
                "[TTS] {} weights are not available at {}: {}",
                DEVANAGARI_ROUTE_PROVIDER,
                model_path.display(),
                e
            )
        })
    }
}

impl Default for DevanagariEngineCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DevanagariEngineCache {
    fn drop(&mut self) {
        if self.engine.lock().take().is_some() {
            LIVE_DEVANAGARI_ENGINES.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
