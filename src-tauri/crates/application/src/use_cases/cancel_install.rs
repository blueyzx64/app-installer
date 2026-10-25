use std::sync::Arc;

use installer_domain::PreviewId;

use crate::PreviewRegistry;

/// Ask a running install to stop. Best effort: the backend decides if it still can.
pub struct CancelInstall {
    registry: Arc<PreviewRegistry>,
}

impl CancelInstall {
    pub fn new(registry: Arc<PreviewRegistry>) -> Self {
        Self { registry }
    }

    /// `true` if an install was running and was signalled.
    #[tracing::instrument(skip(self))]
    pub fn execute(&self, id: PreviewId) -> bool {
        self.registry.cancel(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_running_is_a_quiet_no() {
        let uc = CancelInstall::new(Arc::new(PreviewRegistry::new()));
        assert!(!uc.execute(PreviewId::new(1)));
    }
}
