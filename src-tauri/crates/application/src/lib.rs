//! Use cases and port traits. No IO, no framework types.
//!
//! Async ports use native `async fn` in traits made `Send` with
//! `#[trait_variant::make(Send)]`. They are not object safe, so use cases are
//! generic over their ports and hold them as `Arc<P>`. Sync callback ports
//! (`InstallObserver`, `AppearanceObserver`) are plain `dyn`-friendly traits.
//!
//! See `docs/architecture/ARCHITECTURE.md` section 6.
#![forbid(unsafe_code)]

pub mod identity;
pub mod ports;
pub mod registry;
pub mod use_cases;

#[cfg(feature = "testing")]
pub mod testing;

/// Cooperative cancellation. Adapters `select!` on `cancelled()` in long loops.
pub use tokio_util::sync::CancellationToken as CancelToken;

pub use registry::{PreviewRegistry, StoredPreview};
