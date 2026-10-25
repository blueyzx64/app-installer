use std::sync::Arc;

use installer_domain::{Appearance, BackendError};

/// Called on every appearance change.
pub trait AppearanceObserver: Send + Sync {
    fn on_change(&self, appearance: Appearance);
}

#[trait_variant::make(Send)]
pub trait AppearanceSource: Sync {
    async fn snapshot(&self) -> Result<Appearance, BackendError>;

    /// Start delivering changes to `observer` until the source is dropped.
    /// Does not block; the adapter spawns whatever it needs.
    fn watch(&self, observer: Arc<dyn AppearanceObserver>);
}
