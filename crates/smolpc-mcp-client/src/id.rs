use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct RequestIdGenerator {
    next_id: AtomicU64,
}

impl RequestIdGenerator {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
        }
    }

    pub fn next(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::RequestIdGenerator;

    #[test]
    fn request_ids_are_monotonic() {
        let generator = RequestIdGenerator::new();

        assert_eq!(generator.next(), 1);
        assert_eq!(generator.next(), 2);
        assert_eq!(generator.next(), 3);
    }
}
