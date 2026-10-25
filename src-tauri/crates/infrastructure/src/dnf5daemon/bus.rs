//! The seam between the adapter and the real D-Bus. The adapter only talks to
//! `Dnf5Bus`; `proxy.rs` implements it over zbus, tests implement it with fakes.
//! Everything here is plain data. No zbus types leak out.

use tokio::sync::mpsc::UnboundedReceiver;

/// Object path of one dnf5daemon session (`/org/rpm/dnf/v0/session_N`). ⚠
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(pub String);

/// A bus call failed. Already classified, see `mapping::classify_dbus_error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusError {
    /// polkit said no or the person dismissed the dialog. Treated as a cancel.
    AccessDenied,
    /// Service is missing or cannot be started.
    Unavailable(String),
    /// The daemon reports the goal was cancelled.
    Cancelled,
    Failed(String),
}

/// One installed rpm, from `rpm.Rpm.list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledRpm {
    pub epoch: u64,
    pub version: String,
    pub release: String,
}

/// One row of the resolved transaction (`Goal.resolve`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedItem {
    /// dnf5 action name: `Install`, `Upgrade`, `Remove`, `Replace`, ... ⚠
    pub action: String,
    pub download_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resolution {
    pub items: Vec<ResolvedItem>,
    /// Human-readable resolver problems. Empty when the goal resolved.
    pub problems: Vec<String>,
}

/// Progress signals, normalised from the dnf5daemon signal zoo. ⚠ names in proxy.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dnf5Signal {
    DownloadAdded {
        id: String,
        total: u64,
    },
    DownloadProgress {
        id: String,
        total: u64,
        downloaded: u64,
    },
    TransactionStarted {
        total: u64,
    },
    ElemProgress {
        processed: u64,
        total: u64,
    },
    TransactionDone {
        success: bool,
    },
}

#[trait_variant::make(Send)]
pub trait Dnf5Bus: Sync {
    async fn open_session(&self) -> Result<SessionId, BusError>;
    async fn close_session(&self, session: &SessionId) -> Result<(), BusError>;
    async fn list_installed(
        &self,
        session: &SessionId,
        name: &str,
    ) -> Result<Vec<InstalledRpm>, BusError>;
    /// Add a local `.rpm` file to the goal.
    async fn queue_install(&self, session: &SessionId, path: &str) -> Result<(), BusError>;
    async fn queue_remove(&self, session: &SessionId, name: &str) -> Result<(), BusError>;
    async fn resolve(&self, session: &SessionId) -> Result<Resolution, BusError>;
    /// Start listening. Call BEFORE `do_transaction`.
    async fn subscribe(
        &self,
        session: &SessionId,
    ) -> Result<UnboundedReceiver<Dnf5Signal>, BusError>;
    /// Returns when the transaction is finished (polkit prompt happens inside).
    async fn do_transaction(&self, session: &SessionId) -> Result<(), BusError>;
    async fn cancel(&self, session: &SessionId) -> Result<(), BusError>;
}
