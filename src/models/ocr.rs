use std::sync::Arc;

/// Optional progress callback: receives a value in [0.0, 1.0] and an optional
/// human-readable detail string (e.g. current stage of a MinerU task).
/// Wrapped in `Arc` so it can be shared/cloned across fallback paths.
pub type ProgressCallback = Arc<dyn Fn(f32, Option<String>) + Send + Sync>;

/// Shared cooperative-cancellation flag. `cancelled()` becomes `true` once
/// any caller requests the running conversion to stop.
#[derive(Debug, Clone, Default)]
pub struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}
