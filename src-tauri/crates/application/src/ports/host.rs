use installer_domain::HostSystem;

/// Facts about this machine (os-release, arch, FUSE).
pub trait HostProbe: Send + Sync {
    fn host(&self) -> HostSystem;
}

/// Wall clock, injected so tests are deterministic.
pub trait Clock: Send + Sync {
    /// Seconds since the Unix epoch.
    fn now_unix(&self) -> i64;
}
