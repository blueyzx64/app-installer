use std::sync::Arc;

use installer_domain::Appearance;

use crate::ports::{AppearanceObserver, AppearanceSource};

/// Current desktop appearance. If the source fails, the safe defaults are returned
/// (light, default accent, `right: [close]`) so the window always renders.
pub struct GetAppearance<A> {
    source: Arc<A>,
}

impl<A: AppearanceSource> GetAppearance<A> {
    pub fn new(source: Arc<A>) -> Self {
        Self { source }
    }

    pub async fn execute(&self) -> Appearance {
        match self.source.snapshot().await {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(error = %e, "appearance unavailable; using defaults");
                Appearance::default()
            }
        }
    }
}

/// Push appearance changes to an observer.
pub struct WatchAppearance<A> {
    source: Arc<A>,
}

impl<A: AppearanceSource> WatchAppearance<A> {
    pub fn new(source: Arc<A>) -> Self {
        Self { source }
    }

    pub fn execute(&self, observer: Arc<dyn AppearanceObserver>) {
        self.source.watch(observer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeAppearance, RecordingAppearanceObserver};
    use installer_domain::{BackendError, ColorScheme, button_layout};

    #[tokio::test]
    async fn snapshot_is_returned() {
        let src = Arc::new(FakeAppearance::default());
        let dark = Appearance {
            color_scheme: ColorScheme::Dark,
            controls: button_layout::parse("close:"),
            ..Appearance::default()
        };
        src.set(dark.clone());
        assert_eq!(GetAppearance::new(src).execute().await, dark);
    }

    #[tokio::test]
    async fn failure_falls_back_to_defaults() {
        let src = Arc::new(FakeAppearance::default());
        src.fail(BackendError::Unavailable("portal".into()));
        assert_eq!(
            GetAppearance::new(src).execute().await,
            Appearance::default()
        );
    }

    #[test]
    fn changes_reach_observer() {
        let src = Arc::new(FakeAppearance::default());
        let obs = Arc::new(RecordingAppearanceObserver::default());
        WatchAppearance::new(src.clone()).execute(obs.clone());
        let next = Appearance {
            high_contrast: true,
            ..Appearance::default()
        };
        src.emit(next.clone());
        assert_eq!(obs.seen(), vec![next]);
    }
}
