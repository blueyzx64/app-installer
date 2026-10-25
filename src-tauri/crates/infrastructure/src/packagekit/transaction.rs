//! Bus abstraction plus the transaction driver.
//!
//! `PkBus` / `PkTransaction` are the only things that touch D-Bus. Everything
//! here is tested with a fake (see `fake.rs`).

use std::future::Future;

use installer_application::CancelToken;
use installer_application::ports::InstallObserver;
use installer_domain::{BackendError, InstallPhase, InstallProgress};

use super::enums::{PkError, PkExit, PkStatus};
use super::mapping::{self, PkPackage};

/// A failed D-Bus method call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallError {
    /// D-Bus error name, e.g. `org.freedesktop.PackageKit.Transaction.RefusedByPolicy`.
    pub dbus_name: Option<String>,
    pub message: String,
}

impl CallError {
    pub fn closed() -> Self {
        CallError {
            dbus_name: Some("org.freedesktop.DBus.Error.Disconnected".into()),
            message: "signal stream ended before Finished".into(),
        }
    }
}

/// The one method a transaction runs (one transaction per operation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PkOp {
    InstallFiles {
        flags: u64,
        paths: Vec<String>,
    },
    RemovePackages {
        flags: u64,
        ids: Vec<String>,
        allow_deps: bool,
        autoremove: bool,
    },
    Resolve {
        filter: u64,
        names: Vec<String>,
    },
    GetDetails {
        ids: Vec<String>,
    },
}

/// Signals and property changes of one transaction, in bus order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PkEvent {
    Package(PkPackage),
    /// From `Details` (GetDetails): download size of one package.
    Details {
        package_id: String,
        size: u64,
    },
    ErrorCode {
        code: PkError,
        details: String,
    },
    Finished {
        exit: PkExit,
    },
    Status(PkStatus),
    Percentage(u32),
    RemainingTime(u32),
    AllowCancel(bool),
}

pub trait PkTransaction: Send + Sync {
    /// Call the operation. For methods that need authorization this does not
    /// return until polkit answered, so the driver polls it next to the events.
    fn start(&self, op: PkOp) -> impl Future<Output = Result<(), CallError>> + Send;
    fn cancel(&self) -> impl Future<Output = ()> + Send;
    /// Next event; `None` when the stream ended. Must be cancel safe.
    fn next_event(&self) -> impl Future<Output = Option<PkEvent>> + Send;
}

pub trait PkBus: Send + Sync {
    type Tx: PkTransaction;
    /// `CreateTransaction` + `SetHints`, with signal and property subscriptions
    /// already in place so no early event is missed.
    fn begin(&self) -> impl Future<Output = Result<Self::Tx, BackendError>> + Send;
    /// PackageKit is on the system bus (running or activatable).
    fn is_available(&self) -> impl Future<Output = bool> + Send;
}

/// What a finished transaction reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxReport {
    pub exit: PkExit,
    /// First `ErrorCode` signal, if any.
    pub error: Option<(PkError, String)>,
    pub packages: Vec<PkPackage>,
    pub sizes: Vec<(String, u64)>,
}

impl TxReport {
    pub fn result(&self) -> Result<(), installer_domain::InstallError> {
        mapping::outcome(
            self.exit,
            self.error.as_ref().map(|(c, d)| (*c, d.as_str())),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    phase: InstallPhase,
    percent: Option<u8>,
    eta: Option<u32>,
    allow_cancel: bool,
}

fn to_progress(s: &Shown) -> InstallProgress {
    let mut p = InstallProgress::phase(s.phase);
    p.percent = s.percent;
    p.eta_secs = s.eta;
    p.with_allow_cancel(s.allow_cancel)
}

/// Run one operation to `Finished`, forwarding progress and honouring cancel.
///
/// Cancel rule (F-28): `Cancel()` is only called while `AllowCancel` is true. A
/// cancel asked earlier is kept and sent the moment `AllowCancel` turns true.
pub async fn run<T: PkTransaction>(
    tx: &T,
    op: PkOp,
    observer: Option<&dyn InstallObserver>,
    cancel: &CancelToken,
) -> Result<TxReport, CallError> {
    let mut shown = Shown {
        phase: InstallPhase::Preparing,
        percent: None,
        eta: None,
        allow_cancel: false,
    };
    let mut last_emitted: Option<Shown> = None;
    let mut seen_work = false;
    let is_remove = matches!(op, PkOp::RemovePackages { .. });
    let emit = |s: &Shown, last: &mut Option<Shown>| {
        if *last != Some(*s) {
            *last = Some(*s);
            if let Some(o) = observer {
                o.on_progress(to_progress(s));
            }
        }
    };
    emit(&shown, &mut last_emitted);

    let mut report = TxReport {
        exit: PkExit::Unknown,
        error: None,
        packages: Vec::new(),
        sizes: Vec::new(),
    };

    let call = tx.start(op);
    tokio::pin!(call);
    let mut call_done = false;
    let mut cancel_armed = true;
    let mut cancel_requested = false;
    let mut cancel_sent = false;

    loop {
        tokio::select! {
            biased;
            res = &mut call, if !call_done => {
                call_done = true;
                res?;
            }
            ev = tx.next_event() => {
                let Some(ev) = ev else { return Err(CallError::closed()) };
                match ev {
                    PkEvent::Finished { exit } => {
                        report.exit = exit;
                        return Ok(report);
                    }
                    PkEvent::Package(p) => report.packages.push(p),
                    PkEvent::Details { package_id, size } => report.sizes.push((package_id, size)),
                    PkEvent::ErrorCode { code, details } => {
                        if report.error.is_none() {
                            report.error = Some((code, details));
                        }
                    }
                    PkEvent::Status(s) => {
                        let ctx = mapping::PhaseCtx {
                            current: shown.phase,
                            seen_work,
                            is_remove,
                        };
                        if let Some(p) = mapping::status_phase(s, ctx) {
                            shown.phase = mapping::next_phase(shown.phase, p);
                        }
                        seen_work |= mapping::implies_work(s);
                    }
                    PkEvent::Percentage(p) => shown.percent = mapping::percent(p),
                    PkEvent::RemainingTime(t) => shown.eta = mapping::eta(t),
                    PkEvent::AllowCancel(a) => shown.allow_cancel = a,
                }
                emit(&shown, &mut last_emitted);
            }
            () = cancel.cancelled(), if cancel_armed => {
                cancel_armed = false;
                cancel_requested = true;
            }
        }
        if cancel_requested && !cancel_sent && shown.allow_cancel {
            cancel_sent = true;
            tx.cancel().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packagekit::enums::PkInfo;
    use crate::packagekit::fake::{FakeTx, Step};
    use installer_application::testing::RecordingObserver;

    fn op() -> PkOp {
        PkOp::InstallFiles {
            flags: 0,
            paths: vec!["/tmp/a.rpm".into()],
        }
    }

    fn happy_script() -> Vec<Step> {
        vec![
            Step::Event(PkEvent::Status(PkStatus::Setup)),
            Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
            Step::Event(PkEvent::Status(PkStatus::Download)),
            Step::Event(PkEvent::Percentage(40)),
            Step::Event(PkEvent::RemainingTime(7)),
            Step::Event(PkEvent::AllowCancel(true)),
            Step::Event(PkEvent::AllowCancel(false)),
            Step::Event(PkEvent::Status(PkStatus::Install)),
            Step::Event(PkEvent::Percentage(101)),
            Step::Event(PkEvent::Package(PkPackage {
                info: PkInfo::Installing,
                id: "a;1;x86_64;local".into(),
            })),
            Step::Event(PkEvent::Status(PkStatus::Cleanup)),
            Step::Event(PkEvent::Finished {
                exit: PkExit::Success,
            }),
        ]
    }

    #[tokio::test]
    async fn forwards_progress_in_order_without_duplicates() {
        let tx = FakeTx::new(happy_script());
        let obs = RecordingObserver::default();
        let r = run(&tx, op(), Some(&obs), &CancelToken::new())
            .await
            .unwrap();
        assert_eq!(r.exit, PkExit::Success);
        assert_eq!(r.result(), Ok(()));
        assert_eq!(r.packages.len(), 1);
        use InstallPhase::*;
        assert_eq!(
            obs.phases(),
            vec![
                Preparing,
                WaitingForPermission,
                Downloading,
                Downloading,
                Downloading,
                Downloading,
                Downloading,
                Installing,
                Installing,
                Finishing
            ]
        );
        let p = obs.progress();
        assert_eq!(p[3].percent, Some(40));
        assert_eq!(p[4].eta_secs, Some(7));
        assert!(p[5].allow_cancel && !p[6].allow_cancel);
        assert_eq!(p[8].percent, None, "101 is unknown");
        assert!(tx.cancels() == 0);
    }

    #[tokio::test]
    async fn error_then_finished_is_reported() {
        let tx = FakeTx::new(vec![
            Step::Event(PkEvent::ErrorCode {
                code: PkError::NoNetwork,
                details: "x".into(),
            }),
            Step::Event(PkEvent::Finished {
                exit: PkExit::Failed,
            }),
        ]);
        let r = run(&tx, op(), None, &CancelToken::new()).await.unwrap();
        assert_eq!(r.result(), Err(installer_domain::InstallError::Offline));
    }

    #[tokio::test]
    async fn dismissed_polkit_signal_is_cancel() {
        let tx = FakeTx::new(vec![
            Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
            Step::Event(PkEvent::ErrorCode {
                code: PkError::NotAuthorized,
                details: "".into(),
            }),
            Step::Event(PkEvent::Finished {
                exit: PkExit::Failed,
            }),
        ]);
        let r = run(&tx, op(), None, &CancelToken::new()).await.unwrap();
        assert_eq!(r.result(), Err(installer_domain::InstallError::Cancelled));
    }

    #[tokio::test]
    async fn call_error_surfaces() {
        let tx = FakeTx::new(vec![]).with_call_error(CallError {
            dbus_name: Some("org.freedesktop.PackageKit.Transaction.RefusedByPolicy".into()),
            message: "no".into(),
        });
        let e = run(&tx, op(), None, &CancelToken::new()).await.unwrap_err();
        assert!(e.dbus_name.unwrap().ends_with("RefusedByPolicy"));
    }

    #[tokio::test]
    async fn closed_stream_is_an_error() {
        let tx = FakeTx::new(vec![Step::Event(PkEvent::Status(PkStatus::Setup))]);
        let e = run(&tx, op(), None, &CancelToken::new()).await.unwrap_err();
        assert_eq!(e, CallError::closed());
    }

    #[tokio::test]
    async fn cancel_only_while_allowed() {
        // Token fires while AllowCancel is false: nothing is sent until it turns true.
        let token = CancelToken::new();
        let tx = FakeTx::new(vec![
            Step::Event(PkEvent::Status(PkStatus::Download)),
            Step::CancelToken(token.clone()),
            Step::Yield,
            Step::Yield,
            Step::AssertCancels(0),
            Step::Event(PkEvent::AllowCancel(true)),
            Step::Yield,
            Step::Event(PkEvent::Finished {
                exit: PkExit::Cancelled,
            }),
        ]);
        let r = run(&tx, op(), None, &token).await.unwrap();
        assert_eq!(tx.cancels(), 1);
        assert_eq!(r.result(), Err(installer_domain::InstallError::Cancelled));
    }

    #[tokio::test]
    async fn cancel_sent_once_when_allowed() {
        let token = CancelToken::new();
        let tx = FakeTx::new(vec![
            Step::Event(PkEvent::AllowCancel(true)),
            Step::CancelToken(token.clone()),
            Step::Yield,
            Step::Yield,
            Step::Event(PkEvent::Percentage(10)),
            Step::Event(PkEvent::Percentage(11)),
            Step::Event(PkEvent::Finished {
                exit: PkExit::Cancelled,
            }),
        ]);
        run(&tx, op(), None, &token).await.unwrap();
        assert_eq!(tx.cancels(), 1);
    }

    #[tokio::test]
    async fn cancel_never_when_not_allowed() {
        let token = CancelToken::new();
        let tx = FakeTx::new(vec![
            Step::CancelToken(token.clone()),
            Step::Yield,
            Step::Yield,
            Step::Event(PkEvent::Finished {
                exit: PkExit::Success,
            }),
        ]);
        run(&tx, op(), None, &token).await.unwrap();
        assert_eq!(tx.cancels(), 0);
    }
}
