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
pub mod inspect;
