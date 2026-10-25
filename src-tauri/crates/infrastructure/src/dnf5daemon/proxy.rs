//! zbus implementation of `Dnf5Bus`. Thin: no decisions, only wire <-> plain data.
//!
//! ⚠ Everything below is from memory of `dnf5daemon-dbus-api(8)`, nothing was
//! run against a real daemon (D-025). HUMAN: verify on Fedora 45 with
//! `busctl --system tree org.rpm.dnf.v0` and `busctl introspect`:
//! - service `org.rpm.dnf.v0`, manager at `/org/rpm/dnf/v0`, iface `org.rpm.dnf.v0.SessionManager`
//! - `open_session(a{sv}) -> o`, `close_session(o) -> b`
//! - per session: `org.rpm.dnf.v0.rpm.Rpm` (`list`, `install`, `remove`) and `org.rpm.dnf.v0.Goal`
//!   (`resolve(a{sv}) -> (a(sssa{sv}a{sv}), u)`, `do_transaction(a{sv})`, `cancel() -> (b, s)`)
//! - signals (first arg = session path `o`): on `org.rpm.dnf.v0.Base`
//!   `download_add_new(o,s,s,x)`, `download_progress(o,s,x,x)`;
//!   on `org.rpm.dnf.v0.rpm.Rpm` `transaction_before_begin(o,t)`,
//!   `transaction_elem_progress(o,s,t,t)`, `transaction_after_complete(o,b)`
//! - `Rpm.install` accepting a local `.rpm` path as a spec (may instead need a
//!   dedicated call; this is the biggest unknown)

use std::collections::HashMap;

use futures_util::StreamExt;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, message::Type as MsgType};

use super::bus::{
    BusError, Dnf5Bus, Dnf5Signal, InstalledRpm, Resolution, ResolvedItem, SessionId,
};
use super::mapping::classify_dbus_error;

const SERVICE: &str = "org.rpm.dnf.v0";

type Opts<'a> = HashMap<&'a str, Value<'a>>;
type Attrs = HashMap<String, OwnedValue>;
/// `(object_type, action, reason, trans_item_attrs, pkg_attrs)`
type WireItem = (String, String, String, Attrs, Attrs);

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.SessionManager",
    default_service = "org.rpm.dnf.v0",
    default_path = "/org/rpm/dnf/v0"
)]
trait SessionManager {
    fn open_session(&self, options: Opts<'_>) -> zbus::Result<OwnedObjectPath>;
    fn close_session(&self, session: &ObjectPath<'_>) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.rpm.dnf.v0.rpm.Rpm",
    default_service = "org.rpm.dnf.v0"
)]
trait Rpm {
    fn list(&self, options: Opts<'_>) -> zbus::Result<Vec<Attrs>>;
    fn install(&self, pkg_specs: &[&str], options: Opts<'_>) -> zbus::Result<()>;
    fn remove(&self, pkg_specs: &[&str], options: Opts<'_>) -> zbus::Result<()>;
}

#[zbus::proxy(interface = "org.rpm.dnf.v0.Goal", default_service = "org.rpm.dnf.v0")]
trait Goal {
    fn resolve(&self, options: Opts<'_>) -> zbus::Result<(Vec<WireItem>, u32)>;
    fn do_transaction(&self, options: Opts<'_>) -> zbus::Result<()>;
    fn cancel(&self) -> zbus::Result<(bool, String)>;
    /// Upstream Goal.xml: `<arg name="problems" type="as" direction="out"/>`.
    fn get_transaction_problems_string(&self) -> zbus::Result<Vec<String>>;
}

fn err(e: zbus::Error) -> BusError {
    match &e {
        zbus::Error::MethodError(name, desc, _) => {
            classify_dbus_error(name.as_str(), desc.as_deref().unwrap_or(""))
        }
        zbus::Error::FDO(f) => {
            use zbus::DBusError as _;
            classify_dbus_error(f.name().as_str(), f.description().unwrap_or(""))
        }
        other => BusError::Failed(other.to_string()),
    }
}

fn str_attr(a: &Attrs, k: &str) -> Option<String> {
    a.get(k)
        .and_then(|v| <&str>::try_from(&**v).ok())
        .map(str::to_owned)
}

fn u64_attr(a: &Attrs, k: &str) -> Option<u64> {
    let v = a.get(k)?;
    u64::try_from(&**v)
        .ok()
        .or_else(|| i64::try_from(&**v).ok().and_then(|n| u64::try_from(n).ok()))
}

/// Real bus. Cheap to clone (the connection is shared).
#[derive(Clone)]
pub struct ZbusBus {
    conn: Connection,
}

impl ZbusBus {
    pub async fn system() -> Result<Self, BusError> {
        Ok(Self {
            conn: Connection::system().await.map_err(err)?,
        })
    }

    fn path(sid: &SessionId) -> Result<OwnedObjectPath, BusError> {
        OwnedObjectPath::try_from(sid.0.as_str()).map_err(|e| BusError::Failed(e.to_string()))
    }

    async fn rpm(&self, sid: &SessionId) -> Result<RpmProxy<'_>, BusError> {
        RpmProxy::builder(&self.conn)
            .path(Self::path(sid)?)
            .map_err(err)?
            .build()
            .await
            .map_err(err)
    }

    async fn goal(&self, sid: &SessionId) -> Result<GoalProxy<'_>, BusError> {
        GoalProxy::builder(&self.conn)
            .path(Self::path(sid)?)
            .map_err(err)?
            .build()
            .await
            .map_err(err)
    }
}

/// Is `org.rpm.dnf.v0` on the system bus (running or activatable)?
/// Never starts it. `false` when there is no system bus at all.
pub async fn is_available() -> bool {
    let Ok(conn) = Connection::system().await else {
        return false;
    };
    let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await else {
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

fn decode(msg: &zbus::Message, session: &str) -> Option<Dnf5Signal> {
    let hdr = msg.header();
    let member = hdr.member()?.as_str().to_owned();
    let body = msg.body();
    let mine = |p: &OwnedObjectPath| p.as_str() == session;
    match member.as_str() {
        "download_add_new" => {
            let (p, id, _desc, total): (OwnedObjectPath, String, String, i64) =
                body.deserialize().ok()?;
            mine(&p).then(|| Dnf5Signal::DownloadAdded {
                id,
                total: u64::try_from(total).unwrap_or(0),
            })
        }
        "download_progress" => {
            let (p, id, total, done): (OwnedObjectPath, String, i64, i64) =
                body.deserialize().ok()?;
            mine(&p).then(|| Dnf5Signal::DownloadProgress {
                id,
                total: u64::try_from(total).unwrap_or(0),
                downloaded: u64::try_from(done).unwrap_or(0),
            })
        }
        "transaction_before_begin" => {
            let (p, total): (OwnedObjectPath, u64) = body.deserialize().ok()?;
            mine(&p).then_some(Dnf5Signal::TransactionStarted { total })
        }
        "transaction_elem_progress" => {
            let (p, _nevra, processed, total): (OwnedObjectPath, String, u64, u64) =
                body.deserialize().ok()?;
            mine(&p).then_some(Dnf5Signal::ElemProgress { processed, total })
        }
        "transaction_after_complete" => {
            let (p, success): (OwnedObjectPath, bool) = body.deserialize().ok()?;
            mine(&p).then_some(Dnf5Signal::TransactionDone { success })
        }
        _ => None,
    }
}

impl Dnf5Bus for ZbusBus {
    async fn open_session(&self) -> Result<SessionId, BusError> {
        let mgr = SessionManagerProxy::new(&self.conn).await.map_err(err)?;
        let path = mgr.open_session(HashMap::new()).await.map_err(err)?;
        Ok(SessionId(path.as_str().to_owned()))
    }

    async fn close_session(&self, session: &SessionId) -> Result<(), BusError> {
        let mgr = SessionManagerProxy::new(&self.conn).await.map_err(err)?;
        mgr.close_session(&Self::path(session)?.into_inner())
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn list_installed(
        &self,
        session: &SessionId,
        name: &str,
    ) -> Result<Vec<InstalledRpm>, BusError> {
        let mut o: Opts<'_> = HashMap::new();
        o.insert(
            "package_attrs",
            Value::from(vec!["name", "epoch", "version", "release"]),
        );
        o.insert("patterns", Value::from(vec![name]));
        o.insert("scope", Value::from("installed"));
        let rows = self.rpm(session).await?.list(o).await.map_err(err)?;
        Ok(rows
            .iter()
            .filter(|r| str_attr(r, "name").as_deref() == Some(name))
            .filter_map(|r| {
                Some(InstalledRpm {
                    epoch: str_attr(r, "epoch")
                        .and_then(|e| e.parse().ok())
                        .or_else(|| u64_attr(r, "epoch"))
                        .unwrap_or(0),
                    version: str_attr(r, "version")?,
                    release: str_attr(r, "release")?,
                })
            })
            .collect())
    }

    async fn queue_install(&self, session: &SessionId, path: &str) -> Result<(), BusError> {
        self.rpm(session)
            .await?
            .install(&[path], HashMap::new())
            .await
            .map_err(err)
    }

    async fn queue_remove(&self, session: &SessionId, name: &str) -> Result<(), BusError> {
        self.rpm(session)
            .await?
            .remove(&[name], HashMap::new())
            .await
            .map_err(err)
    }

    async fn resolve(&self, session: &SessionId) -> Result<Resolution, BusError> {
        let goal = self.goal(session).await?;
        let (items, code) = goal.resolve(HashMap::new()).await.map_err(err)?;
        // ⚠ code: 0 ok, 1 warnings, 2 errors. On error keep the real cause.
        let problems = if code >= 2 {
            let text = goal
                .get_transaction_problems_string()
                .await
                .unwrap_or_default();
            let lines: Vec<String> = text
                .into_iter()
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty())
                .collect();
            if lines.is_empty() {
                vec!["problem resolving dependencies".to_owned()]
            } else {
                lines
            }
        } else {
            Vec::new()
        };
        Ok(Resolution {
            items: items
                .iter()
                .map(|(_ty, action, _reason, _t, pkg)| ResolvedItem {
                    action: action.clone(),
                    download_size: u64_attr(pkg, "download_size").unwrap_or(0),
                })
                .collect(),
            problems,
        })
    }

    async fn subscribe(
        &self,
        session: &SessionId,
    ) -> Result<UnboundedReceiver<Dnf5Signal>, BusError> {
        let rule = MatchRule::builder()
            .msg_type(MsgType::Signal)
            .sender(SERVICE)
            .map_err(err)?
            .build();
        let mut stream = MessageStream::for_match_rule(rule, &self.conn, None)
            .await
            .map_err(err)?;
        let (tx, rx) = unbounded_channel();
        let sid = session.0.clone();
        // Ends as soon as the receiver is dropped; dropping `stream` removes the
        // bus match rule. No per-call leak.
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = tx.closed() => break,
                    msg = stream.next() => {
                        let Some(Ok(msg)) = msg else { break };
                        if let Some(sig) = decode(&msg, &sid)
                            && tx.send(sig).is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Ok(rx)
    }

    async fn do_transaction(&self, session: &SessionId) -> Result<(), BusError> {
        self.goal(session)
            .await?
            .do_transaction(HashMap::new())
            .await
            .map_err(err)
    }

    async fn cancel(&self, session: &SessionId) -> Result<bool, BusError> {
        let (ok, _msg) = self.goal(session).await?.cancel().await.map_err(err)?;
        Ok(ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a Fedora box with dnf5daemon. HUMAN: `cargo test --features dnf5daemon -- --ignored`.
    #[tokio::test]
    #[ignore = "needs system bus with org.rpm.dnf.v0"]
    async fn real_bus_lists_and_closes_session() {
        assert!(is_available().await);
        let bus = ZbusBus::system().await.expect("system bus");
        let sid = bus.open_session().await.expect("open");
        let _ = bus.list_installed(&sid, "bash").await.expect("list");
        bus.close_session(&sid).await.expect("close");
    }
}
