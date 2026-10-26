//! Adapters that implement the `installer-application` ports.
//!
//! Stub in W0. Lanes B and C add modules here (one `pub mod` line each):
//!
//! - `inspect/`            PackageInspector per format; `rpm.rs`, `deb.rs` (lane B), `appimage.rs`, `elf.rs` (lane C)
//! - `host/`               HostProbe, os-release, FUSE check (lane B)
//! - `packagekit/`         SystemPackages over D-Bus (lane B)
//! - `dnf5daemon/`         SystemPackages fallback (lane B)
//! - `appimage_store.rs`   UserApps (lane C)
//! - `desktop_entries.rs`  DesktopIntegration (lane C)
//! - `defaults.rs`         DefaultHandlerRegistry (lane C)
//! - `ledger.rs`           AppLedger (lane C)
//! - `launcher.rs`         Launcher (lane C)
//! - `appearance.rs`       AppearanceSource (lane D/S7)
//!
//! Never depend on `tauri` here. Never execute a package (ADR-0005).
#![forbid(unsafe_code)]

pub mod appearance; // lane S7
pub mod appimage_store; // lane S3
pub mod defaults; // lane S3b
pub mod desktop_entries; // lane S3
pub mod file_probe;
pub mod host;
pub mod inspect;
pub mod launcher; // lane S3
pub mod ledger; // lane S3
pub mod packagekit; // lane S2

// lane S2c
#[cfg(feature = "dnf5daemon")]
pub mod dnf5daemon;

#[cfg(test)]
mod review_r2; // lane S3 (R2 proof tests)

#[cfg(test)]
mod review_r2b; // lane S3 (R2 reverify proof tests)
