//! zbus proxies and the real `PkBus` over the system bus.
//!
//! Not unit tested (D-025: no system bus in CI). Covered by `#[ignore]` tests
//! at the bottom, run by a human on Fedora.

use std::collections::HashMap;

use futures_util::StreamExt;
use tokio::sync::{Mutex, OnceCell, mpsc};
use tokio::task::JoinHandle;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream, message};

use installer_domain::BackendError;

use super::enums::{PkError, PkExit, PkInfo, PkStatus};
use super::mapping::PkPackage;
use super::transaction::{CallError, PkBus, PkEvent, PkOp, PkTransaction};

const SERVICE: &str = "org.freedesktop.PackageKit";
const TX_IFACE: &str = "org.freedesktop.PackageKit.Transaction";

#[zbus::proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
pub trait PackageKit {
    fn create_transaction(&self) -> zbus::Result<OwnedObjectPath>;

    #[zbus(property)]
    fn version_major(&self) -> zbus::Result<u32>;
}

/// Methods and properties of one transaction. The signals (`Package`,
/// `Details`, `ErrorCode`, `Finished`) are read from one ordered message
/// stream instead of per-signal streams, so `ErrorCode` can never be seen after
/// `Finished`.
#[zbus::proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
pub trait Transaction {
    fn set_hints(&self, hints: &[&str]) -> zbus::Result<()>;
    fn install_files(&self, transaction_flags: u64, full_paths: &[&str]) -> zbus::Result<()>;
    fn resolve(&self, filter: u64, packages: &[&str]) -> zbus::Result<()>;
    fn get_details(&self, package_ids: &[&str]) -> zbus::Result<()>;
    fn remove_packages(
        &self,
        transaction_flags: u64,
        package_ids: &[&str],
        allow_deps: bool,
        autoremove: bool,
    ) -> zbus::Result<()>;
    fn cancel(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn status(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn remaining_time(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn allow_cancel(&self) -> zbus::Result<bool>;
}

fn call_error(e: zbus::Error) -> CallError {
    match &e {
        zbus::Error::MethodError(name, desc, _) => CallError {
            dbus_name: Some(name.to_string()),
            message: desc.clone().unwrap_or_default(),
        },
        zbus::Error::FDO(fdo) => CallError {
            dbus_name: Some(format!("org.freedesktop.DBus.Error.{}", fdo_name(fdo))),
            message: e.to_string(),
        },
        _ => CallError {
            dbus_name: None,
            message: e.to_string(),
        },
    }
}

fn fdo_name(e: &zbus::fdo::Error) -> &'static str {
    match e {
        zbus::fdo::Error::ServiceUnknown(_) => "ServiceUnknown",
        zbus::fdo::Error::NameHasNoOwner(_) => "NameHasNoOwner",
        zbus::fdo::Error::NoReply(_) => "NoReply",
        zbus::fdo::Error::AccessDenied(_) => "AccessDenied",
        _ => "Failed",
    }
}

fn backend(e: zbus::Error) -> BackendError {
    let c = call_error(e);
    BackendError::Unavailable(format!(
        "{}: {}",
        c.dbus_name.unwrap_or_default(),
        c.message
    ))
}

/// `LANG=de_DE.UTF-8` to the hint value. PackageKit wants `ll_CC.UTF-8`.
pub fn locale_hint(lang: Option<&str>) -> String {
    let raw = lang.unwrap_or("").trim();
    if raw.is_empty() || raw == "C" || raw == "POSIX" {
        return "C".into();
    }
    let base = raw.split('.').next().unwrap_or(raw);
    format!("{base}.UTF-8")
}

pub fn hints(lang: Option<&str>) -> Vec<String> {
    vec![
        "interactive=true".into(),
        format!("locale={}", locale_hint(lang)),
        "background=false".into(),
    ]
}

/// The system bus, connected on first use and shared.
pub struct SystemBus {
    conn: OnceCell<Connection>,
}

impl Default for SystemBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemBus {
    pub fn new() -> Self {
        SystemBus {
            conn: OnceCell::new(),
        }
    }

    async fn conn(&self) -> Result<&Connection, BackendError> {
        self.conn
            .get_or_try_init(|| async { Connection::system().await })
            .await
            .map_err(backend)
    }
}

impl PkBus for SystemBus {
    type Tx = ZbusTransaction;

    async fn begin(&self) -> Result<ZbusTransaction, BackendError> {
        let conn = self.conn().await?.clone();
        let pk = PackageKitProxy::new(&conn).await.map_err(backend)?;
        let path = pk.create_transaction().await.map_err(backend)?;

        // Subscribe before any method runs, so no signal is missed.
        let rule = MatchRule::builder()
            .msg_type(message::Type::Signal)
            .sender(SERVICE)
            .map_err(backend)?
            .path(path.as_ref())
            .map_err(backend)?
            .build();
        let mut stream = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .map_err(backend)?;
        let (send, recv) = mpsc::unbounded_channel();
        // Guard: any early return below aborts the task (and drops its match rule).
        let pump = PumpGuard(tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                for ev in decode(&msg) {
                    if send.send(ev).is_err() {
                        return;
                    }
                }
            }
        }));

        let proxy = TransactionProxy::builder(&conn)
            .path(path)
            .map_err(backend)?
            .build()
            .await
            .map_err(backend)?;
        let lang = std::env::var("LC_MESSAGES")
            .or_else(|_| std::env::var("LANG"))
            .ok();
        let hints = hints(lang.as_deref());
        let hint_refs: Vec<&str> = hints.iter().map(String::as_str).collect();
        proxy.set_hints(&hint_refs).await.map_err(backend)?;

        Ok(ZbusTransaction {
            proxy,
            events: Mutex::new(recv),
            _pump: pump,
        })
    }

    async fn is_available(&self) -> bool {
        let Ok(conn) = self.conn().await else {
            return false;
        };
        let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
            return false;
        };
        let Ok(name) = zbus::names::BusName::try_from(SERVICE) else {
            return false;
        };
        if dbus.name_has_owner(name).await.unwrap_or(false) {
            return true;
        }
        dbus.list_activatable_names()
            .await
            .map(|names| names.iter().any(|n| n.as_str() == SERVICE))
            .unwrap_or(false)
    }
}

pub struct ZbusTransaction {
    proxy: TransactionProxy<'static>,
    events: Mutex<mpsc::UnboundedReceiver<PkEvent>>,
    _pump: PumpGuard,
}

/// Aborts the signal pump when dropped.
struct PumpGuard(JoinHandle<()>);

impl Drop for PumpGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl PkTransaction for ZbusTransaction {
    async fn start(&self, op: PkOp) -> Result<(), CallError> {
        let r = match op {
            PkOp::InstallFiles { flags, paths } => {
                let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
                self.proxy.install_files(flags, &refs).await
            }
            PkOp::RemovePackages {
                flags,
                ids,
                allow_deps,
                autoremove,
            } => {
                let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
                self.proxy
                    .remove_packages(flags, &refs, allow_deps, autoremove)
                    .await
            }
            PkOp::Resolve { filter, names } => {
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                self.proxy.resolve(filter, &refs).await
            }
            PkOp::GetDetails { ids } => {
                let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
                self.proxy.get_details(&refs).await
            }
        };
        r.map_err(call_error)
    }

    async fn cancel(&self) {
        if let Err(e) = self.proxy.cancel().await {
            tracing::debug!("PackageKit Cancel failed: {e}");
        }
    }

    async fn next_event(&self) -> Option<PkEvent> {
        self.events.lock().await.recv().await
    }
}

/// Turn one bus message into events. Unknown messages give none.
fn decode(msg: &zbus::Message) -> Vec<PkEvent> {
    let header = msg.header();
    let (Some(iface), Some(member)) = (header.interface(), header.member()) else {
        return Vec::new();
    };
    let body = msg.body();
    match (iface.as_str(), member.as_str()) {
        (TX_IFACE, "Package") => body
            .deserialize::<(u32, String, String)>()
            .map(|(info, id, _summary)| {
                vec![PkEvent::Package(PkPackage {
                    info: PkInfo::from_u32(info),
                    id,
                })]
            })
            .unwrap_or_default(),
        (TX_IFACE, "Details") => body
            .deserialize::<HashMap<String, OwnedValue>>()
            .ok()
            .and_then(|m| {
                let id = String::try_from(m.get("package-id")?.try_clone().ok()?).ok()?;
                // F-27 is about downloads: prefer `download-size`; `size` is the installed size.
                let size = m
                    .get("download-size")
                    .or_else(|| m.get("size"))
                    .and_then(|v| u64::try_from(v.try_clone().ok()?).ok())?;
                Some(vec![PkEvent::Details {
                    package_id: id,
                    size,
                }])
            })
            .unwrap_or_default(),
        (TX_IFACE, "ErrorCode") => body
            .deserialize::<(u32, String)>()
            .map(|(code, details)| {
                vec![PkEvent::ErrorCode {
                    code: PkError::from_u32(code),
                    details,
                }]
            })
            .unwrap_or_default(),
        (TX_IFACE, "Finished") => body
            .deserialize::<(u32, u32)>()
            .map(|(exit, _runtime)| {
                vec![PkEvent::Finished {
                    exit: PkExit::from_u32(exit),
                }]
            })
            .unwrap_or_default(),
        ("org.freedesktop.DBus.Properties", "PropertiesChanged") => body
            .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            .map(|(changed_iface, props, _invalidated)| {
                if changed_iface != TX_IFACE {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for (k, v) in props {
                    let ev = match k.as_str() {
                        "Status" => u32::try_from(v)
                            .ok()
                            .map(|n| PkEvent::Status(PkStatus::from_u32(n))),
                        "Percentage" => u32::try_from(v).ok().map(PkEvent::Percentage),
                        "RemainingTime" => u32::try_from(v).ok().map(PkEvent::RemainingTime),
                        "AllowCancel" => bool::try_from(v).ok().map(PkEvent::AllowCancel),
                        _ => None,
                    };
                    out.extend(ev);
                }
                out
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_hint_table() {
        assert_eq!(locale_hint(Some("de_DE.UTF-8")), "de_DE.UTF-8");
        assert_eq!(locale_hint(Some("en_US")), "en_US.UTF-8");
        assert_eq!(locale_hint(Some("fr_FR.ISO-8859-1")), "fr_FR.UTF-8");
        assert_eq!(locale_hint(Some("C")), "C");
        assert_eq!(locale_hint(Some("")), "C");
        assert_eq!(locale_hint(None), "C");
    }

    #[test]
    fn hints_are_interactive() {
        let h = hints(Some("en_US.UTF-8"));
        assert!(h.contains(&"interactive=true".to_string()));
        assert!(h.contains(&"locale=en_US.UTF-8".to_string()));
    }

    /// HUMAN: run on Fedora with PackageKit: `cargo test -p installer-infrastructure -- --ignored packagekit`.
    #[tokio::test]
    #[ignore = "needs the system bus with PackageKit"]
    async fn real_bus_is_available() {
        assert!(SystemBus::new().is_available().await);
    }

    #[tokio::test]
    #[ignore = "needs the system bus with PackageKit"]
    async fn real_bus_resolve_bash_is_installed() {
        use crate::packagekit::enums::filter;
        use crate::packagekit::transaction::run;
        let bus = SystemBus::new();
        let tx = bus.begin().await.unwrap();
        let r = run(
            &tx,
            PkOp::Resolve {
                filter: filter::INSTALLED,
                names: vec!["bash".into()],
            },
            None,
            &installer_application::CancelToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(r.result(), Ok(()));
        assert!(r.packages.iter().any(|p| p.id.starts_with("bash;")));
    }
}

#[cfg(test)]
mod review_r4_proxy {
    use super::*;
    use zbus::zvariant::Value;

    /// PackageKit 1.4.0 pk-transaction.c:569-575 sends `size` (INSTALLED size, dnf5-backend-thread.cpp:360
    /// get_install_size) and a separate `download-size`. F-27 is about downloads.
    #[test]
    fn review_r4_details_uses_download_size() {
        let mut m: HashMap<&str, Value> = HashMap::new();
        m.insert("package-id", Value::from("dep;1;x86_64;fedora"));
        m.insert("size", Value::from(300_000_000u64));
        m.insert("download-size", Value::from(80_000_000u64));
        let msg = zbus::Message::signal(
            "/1_abc",
            "org.freedesktop.PackageKit.Transaction",
            "Details",
        )
        .unwrap()
        .build(&m)
        .unwrap();
        assert_eq!(
            decode(&msg),
            vec![PkEvent::Details {
                package_id: "dep;1;x86_64;fedora".into(),
                size: 80_000_000
            }]
        );
    }
}
