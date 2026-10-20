use std::{sync::OnceLock, thread::available_parallelism};

use sysinfo::System;

pub const FALLBACK_LOGICAL_CORES: usize = 4;

const VAD_THREADS: u32 = 1;
const MAX_STT_THREADS: u32 = 4;
const MAX_TTS_THREADS: u32 = 4;
const MAX_LLM_THREADS: u32 = 8;

static CAPACITY: OnceLock<Capacity> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workload {
    Vad,
    Stt,
    Llm,
    Tts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Execution {
    Embedded,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity {
    logical: usize,
    physical: usize,
}

impl Capacity {
    pub fn new(logical: usize, physical: usize) -> Self {
        Self {
            logical: logical.max(1),
            physical: physical.max(1),
        }
    }

    pub fn fallback() -> Self {
        Self::new(FALLBACK_LOGICAL_CORES, FALLBACK_LOGICAL_CORES)
    }

    pub fn logical(&self) -> usize {
        self.logical
    }

    pub fn physical(&self) -> usize {
        self.physical
    }

    fn usable(&self) -> usize {
        self.logical.min(self.physical).max(1)
    }

    fn budget(&self) -> u32 {
        let reserved = self.usable().saturating_sub(1).max(1);
        reserved.min(u32::MAX as usize) as u32
    }
}

pub fn detect_capacity() -> Capacity {
    *CAPACITY.get_or_init(detect_capacity_uncached)
}

pub fn resolve_threads(workload: Workload, execution: Execution) -> u32 {
    let threads = resolve_threads_for(workload, execution, detect_capacity());
    log::info!(
        "[Compute::Capacity] workload={:?} execution={:?} threads={}",
        workload,
        execution,
        threads
    );
    threads
}

pub fn resolve_threads_for(workload: Workload, execution: Execution, capacity: Capacity) -> u32 {
    if execution == Execution::Remote {
        return 0;
    }

    let budget = capacity.budget();
    match workload {
        Workload::Vad => VAD_THREADS,
        Workload::Stt => clamp_threads(budget / 2, MAX_STT_THREADS),
        Workload::Tts => clamp_threads(budget / 2, MAX_TTS_THREADS),
        Workload::Llm => clamp_threads(budget, MAX_LLM_THREADS),
    }
}

fn detect_capacity_uncached() -> Capacity {
    let logical = detect_logical_cores();
    let physical = detect_physical_cores().unwrap_or(logical);
    Capacity::new(logical, physical)
}

fn detect_logical_cores() -> usize {
    match available_parallelism() {
        Ok(count) => count.get(),
        Err(err) => {
            log::warn!(
                "[Compute::Capacity] Logical core detection failed ({}); using fallback of {}.",
                err,
                FALLBACK_LOGICAL_CORES
            );
            FALLBACK_LOGICAL_CORES
        }
    }
}

fn detect_physical_cores() -> Option<usize> {
    System::new().physical_core_count()
}

fn clamp_threads(value: u32, max: u32) -> u32 {
    value.clamp(1, max)
}
