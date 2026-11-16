use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QueuedMessageDelivery {
    #[default]
    Immediate,
    TurnBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxMessage {
    pub text: String,
    pub delivery: QueuedMessageDelivery,
    pub received_at: u64,
}

impl InboxMessage {
    pub fn new(text: String, delivery: QueuedMessageDelivery, received_at: u64) -> Self {
        Self {
            text,
            delivery,
            received_at,
        }
    }
}

pub struct StepInbox {
    messages: Mutex<Vec<InboxMessage>>,
    speech_hold_active: AtomicBool,
    notify: Notify,
}

impl Default for StepInbox {
    fn default() -> Self {
        Self {
            messages: Mutex::new(Vec::new()),
            speech_hold_active: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }
}

impl StepInbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a message, releases any active speech hold, and wakes the step boundary.
    pub fn push(&self, message: InboxMessage) {
        self.messages.lock().push(message);
        self.speech_hold_active.store(false, Ordering::Release);
        self.notify.notify_waiters();
    }

    /// Engages the step-boundary hold when SpeechStart/PttStart occurs during Working.
    pub fn engage_speech_hold(&self) {
        self.speech_hold_active.store(true, Ordering::Release);
    }

    /// Releases the step-boundary hold (e.g., on empty speech / false VAD or PttCancel) and wakes waiters.
    pub fn release_speech_hold(&self) {
        self.speech_hold_active.store(false, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn is_speech_held(&self) -> bool {
        self.speech_hold_active.load(Ordering::Acquire)
    }

    /// Waits at the step boundary if the user is currently speaking (Listening/Thinking STT)
    /// until TranscriptFinal pushes a message, empty speech releases the hold, or the turn is cancelled.
    pub async fn wait_if_speech_held(&self, cancel: &CancellationToken) {
        while self.is_speech_held() {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = self.notify.notified() => {
                    if !self.is_speech_held() {
                        break;
                    }
                }
            }
        }
    }

    pub fn drain_immediate(&self) -> Vec<InboxMessage> {
        let mut guard = self.messages.lock();
        let (immediate, remaining): (Vec<_>, Vec<_>) = guard
            .drain(..)
            .partition(|m| m.delivery == QueuedMessageDelivery::Immediate);
        *guard = remaining;
        immediate
    }

    pub fn drain_all(&self) -> Vec<InboxMessage> {
        self.messages.lock().drain(..).collect()
    }

    pub fn clear(&self) {
        self.messages.lock().clear();
        self.speech_hold_active.store(false, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn is_empty(&self) -> bool {
        self.messages.lock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.lock().len()
    }
}
