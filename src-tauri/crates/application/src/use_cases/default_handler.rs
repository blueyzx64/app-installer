use std::sync::Arc;

use installer_domain::{BackendError, DefaultStatus};

use crate::ports::DefaultHandlerRegistry;

/// Are we the default handler for package files, and did we already ask?
pub struct GetDefaultHandlerStatus<R> {
    registry: Arc<R>,
}

impl<R: DefaultHandlerRegistry> GetDefaultHandlerStatus<R> {
    pub fn new(registry: Arc<R>) -> Self {
        Self { registry }
    }

    pub async fn execute(&self) -> Result<DefaultStatus, BackendError> {
        self.registry.status().await
    }
}

/// Become the default handler for all supported package types.
pub struct MakeDefaultHandler<R> {
    registry: Arc<R>,
}

impl<R: DefaultHandlerRegistry> MakeDefaultHandler<R> {
    pub fn new(registry: Arc<R>) -> Self {
        Self { registry }
    }

    pub async fn execute(&self) -> Result<(), BackendError> {
        self.registry.make_default().await
    }
}

/// Remember "Not Now" so we do not ask again.
pub struct DismissDefaultPrompt<R> {
    registry: Arc<R>,
}

impl<R: DefaultHandlerRegistry> DismissDefaultPrompt<R> {
    pub fn new(registry: Arc<R>) -> Self {
        Self { registry }
    }

    pub async fn execute(&self) -> Result<(), BackendError> {
        self.registry.dismiss_prompt().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeDefaults;

    #[tokio::test]
    async fn make_default_then_status_reports_it() {
        let reg = Arc::new(FakeDefaults::default());
        assert_eq!(
            GetDefaultHandlerStatus::new(reg.clone())
                .execute()
                .await
                .unwrap(),
            DefaultStatus {
                is_default: false,
                asked_before: false
            }
        );
        MakeDefaultHandler::new(reg.clone())
            .execute()
            .await
            .unwrap();
        let s = GetDefaultHandlerStatus::new(reg).execute().await.unwrap();
        assert!(s.is_default);
    }

    #[tokio::test]
    async fn dismiss_is_remembered() {
        let reg = Arc::new(FakeDefaults::default());
        DismissDefaultPrompt::new(reg.clone())
            .execute()
            .await
            .unwrap();
        let s = GetDefaultHandlerStatus::new(reg).execute().await.unwrap();
        assert!(s.asked_before && !s.is_default);
    }

    #[tokio::test]
    async fn errors_pass_through() {
        let reg = Arc::new(FakeDefaults::default());
        reg.fail(BackendError::Unavailable("gio".into()));
        let err = MakeDefaultHandler::new(reg).execute().await.unwrap_err();
        assert_eq!(err, BackendError::Unavailable("gio".into()));
    }
}
