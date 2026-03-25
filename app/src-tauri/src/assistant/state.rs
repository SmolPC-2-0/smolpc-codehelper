use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct AssistantState {
    cancelled: AtomicBool,
}

impl AssistantState {
    pub fn clear_cancelled(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    pub fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl smolpc_connector_common::CancellationToken for AssistantState {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled()
    }
}
