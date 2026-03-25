use std::sync::atomic::{AtomicBool, Ordering};

/// Abstraction for cancellation signalling. Connectors use this trait
/// instead of depending on the app's `AssistantState` directly.
pub trait CancellationToken: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

/// Test utility — a simple cancellation token backed by an `AtomicBool`.
/// Public (not behind `#[cfg(test)]`) because connector crate tests need it.
pub struct MockCancellationToken {
    cancelled: AtomicBool,
}

impl MockCancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl Default for MockCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken for MockCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}
