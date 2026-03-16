use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct AssistantState {
    cancelled: AtomicBool,
}

impl AssistantState {
    pub fn clear_cancelled(&self) {
        self.cancelled.store(false, Ordering::Relaxed);
    }

    pub fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
