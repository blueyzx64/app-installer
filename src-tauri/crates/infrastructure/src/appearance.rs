//! `AppearanceSource` for GNOME: the Settings portal plus a few GSettings keys.
//!
//! - colour scheme, accent colour, contrast: `org.freedesktop.portal.Settings`
//! - window button placement: `org.gnome.desktop.wm.preferences button-layout`
//!   (the portal does not expose it)
//! - reduced motion: `org.gnome.desktop.interface enable-animations` (off = reduce)
//!
//! Every source is optional. With no session bus (containers, CI) or no GNOME
//! schemas, each field falls back to its default and nothing fails.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ashpd::desktop::settings::{ColorScheme as PortalScheme, Contrast as PortalContrast, Settings};
use futures_util::{Stream, StreamExt};
use gio::prelude::SettingsExt;
use installer_application::ports::{AppearanceObserver, AppearanceSource};
use installer_domain::appearance::button_layout;
use installer_domain::{Appearance, BackendError, ColorScheme, Rgb};
use tokio::sync::{mpsc, watch};

const WM_SCHEMA: &str = "org.gnome.desktop.wm.preferences";
const WM_LAYOUT_KEY: &str = "button-layout";
const IFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const ANIMATIONS_KEY: &str = "enable-animations";
const TEXT_SCALE_KEY: &str = "text-scaling-factor";

const PORTAL_TIMEOUT: Duration = Duration::from_secs(2);
const DEBOUNCE: Duration = Duration::from_millis(50);

/// What we managed to read. `None` = source missing or unreadable.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawSettings {
    pub color_scheme: Option<PortalScheme>,
    pub accent: Option<(f64, f64, f64)>,
    pub contrast: Option<PortalContrast>,
    pub button_layout: Option<String>,
    pub enable_animations: Option<bool>,
    pub text_scale: Option<f64>,
}

/// Portal colour scheme to ours. Only an explicit "prefer dark" is dark.
pub fn map_color_scheme(s: PortalScheme) -> ColorScheme {
    match s {
        PortalScheme::PreferDark => ColorScheme::Dark,
        PortalScheme::NoPreference | PortalScheme::PreferLight => ColorScheme::Light,
    }
}

/// Portal accent (sRGB floats) to a colour. The portal reports out-of-range
/// channels when no accent is set; that means "use the default blue".
pub fn map_accent((r, g, b): (f64, f64, f64)) -> Option<Rgb> {
    let ok = |v: f64| (0.0..=1.0).contains(&v);
    (ok(r) && ok(g) && ok(b)).then(|| Rgb::from_unit(r, g, b))
}

pub fn map_contrast(c: PortalContrast) -> bool {
    matches!(c, PortalContrast::High)
}

/// Animations switched off in GNOME means reduced motion.
pub fn map_reduced_motion(enable_animations: bool) -> bool {
    !enable_animations
}

/// Merge raw readings into one value; missing parts take the defaults.
pub fn merge(raw: &RawSettings) -> Appearance {
    let d = Appearance::default();
    Appearance {
        color_scheme: raw.color_scheme.map_or(d.color_scheme, map_color_scheme),
        accent: raw.accent.and_then(map_accent),
        high_contrast: raw.contrast.is_some_and(map_contrast),
        reduced_motion: raw.enable_animations.is_some_and(map_reduced_motion),
        text_scale: raw
            .text_scale
            .map_or(d.text_scale, Appearance::clamp_text_scale),
        controls: raw
            .button_layout
            .as_deref()
            .map_or(d.controls, button_layout::parse),
    }
}

/// Read a string/bool key if the schema is installed. Never panics (a missing
/// schema aborts the process in GIO unless looked up first).
fn gsettings(schema: &str) -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    source.lookup(schema, true)?;
    Some(gio::Settings::new(schema))
}

fn has_key(schema: &str, key: &str) -> bool {
    gio::SettingsSchemaSource::default()
        .and_then(|s| s.lookup(schema, true))
        .is_some_and(|s| s.has_key(key))
}

fn read_gsettings() -> (Option<String>, Option<bool>, Option<f64>) {
    let layout = has_key(WM_SCHEMA, WM_LAYOUT_KEY)
        .then(|| gsettings(WM_SCHEMA))
        .flatten()
        .map(|s| s.string(WM_LAYOUT_KEY).to_string());
    let animations = has_key(IFACE_SCHEMA, ANIMATIONS_KEY)
        .then(|| gsettings(IFACE_SCHEMA))
        .flatten()
        .map(|s| s.boolean(ANIMATIONS_KEY));
    let scale = has_key(IFACE_SCHEMA, TEXT_SCALE_KEY)
        .then(|| gsettings(IFACE_SCHEMA))
        .flatten()
        .map(|s| s.double(TEXT_SCALE_KEY));
    (layout, animations, scale)
}

async fn within<T>(f: impl Future<Output = Result<T, ashpd::Error>>) -> Option<T> {
    tokio::time::timeout(PORTAL_TIMEOUT, f).await.ok()?.ok()
}

async fn read_raw() -> RawSettings {
    let mut raw = RawSettings::default();
    if let Ok(Ok(portal)) = tokio::time::timeout(PORTAL_TIMEOUT, Settings::new()).await {
        raw.color_scheme = within(portal.color_scheme()).await;
        raw.accent = within(portal.accent_color())
            .await
            .map(|c| (c.red(), c.green(), c.blue()));
        raw.contrast = within(portal.contrast()).await;
    }
    if let Ok((layout, animations, scale)) = tokio::task::spawn_blocking(read_gsettings).await {
        raw.button_layout = layout;
        raw.enable_animations = animations;
        raw.text_scale = scale;
    }
    raw
}

type Ticks = Pin<Box<dyn Stream<Item = ()> + Send>>;

/// Any change in the portal's settings. Pending forever when there is no portal.
async fn portal_ticks() -> Ticks {
    if let Ok(Ok(portal)) = tokio::time::timeout(PORTAL_TIMEOUT, Settings::new()).await
        && let Ok(stream) = portal.receive_setting_changed().await
    {
        return Box::pin(stream.map(|_| ()));
    }
    Box::pin(futures_util::stream::pending())
}

/// Watch GSettings on its own thread with its own GLib main loop. Each change
/// sends a tick. Returns a handle that quits the loop when asked.
fn spawn_gsettings_watch(ticks: mpsc::UnboundedSender<()>) -> Option<glib::MainLoop> {
    let (loop_tx, loop_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("appearance-gsettings".into())
        .spawn(move || {
            let ctx = glib::MainContext::new();
            let _ = ctx.with_thread_default(|| {
                let main_loop = glib::MainLoop::new(Some(&ctx), false);
                let mut keep = Vec::new();
                for (schema, key) in [
                    (WM_SCHEMA, WM_LAYOUT_KEY),
                    (IFACE_SCHEMA, ANIMATIONS_KEY),
                    (IFACE_SCHEMA, TEXT_SCALE_KEY),
                ] {
                    if !has_key(schema, key) {
                        continue;
                    }
                    if let Some(settings) = gsettings(schema) {
                        let tx = ticks.clone();
                        settings.connect_changed(Some(key), move |_, _| {
                            let _ = tx.send(());
                        });
                        keep.push(settings);
                    }
                }
                let _ = loop_tx.send(main_loop.clone());
                if !keep.is_empty() {
                    main_loop.run();
                }
            });
        })
        .ok()?;
    loop_rx.recv_timeout(Duration::from_secs(2)).ok()
}

async fn run_watch(observer: Arc<dyn AppearanceObserver>, mut stop: watch::Receiver<()>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let gs_loop = spawn_gsettings_watch(tx);
    let mut portal = portal_ticks().await;
    let mut last = merge(&read_raw().await);

    loop {
        tokio::select! {
            _ = stop.changed() => break, // sender dropped
            _ = rx.recv() => {}
            t = portal.next() => { if t.is_none() { portal = Box::pin(futures_util::stream::pending()); continue; } }
        }
        tokio::time::sleep(DEBOUNCE).await;
        while rx.try_recv().is_ok() {}
        let now = merge(&read_raw().await);
        if now != last {
            last = now.clone();
            observer.on_change(now);
        }
    }
    if let Some(l) = gs_loop {
        l.quit();
    }
}

/// GNOME adapter. Dropping it stops every watcher it started.
#[derive(Default)]
pub struct GnomeAppearanceSource {
    stops: Mutex<Vec<watch::Sender<()>>>,
}

impl GnomeAppearanceSource {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AppearanceSource for GnomeAppearanceSource {
    async fn snapshot(&self) -> Result<Appearance, BackendError> {
        Ok(merge(&read_raw().await))
    }

    fn watch(&self, observer: Arc<dyn AppearanceObserver>) {
        let (stop_tx, stop_rx) = watch::channel(());
        let built = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let Ok(rt) = built else {
            tracing::warn!("appearance watch: cannot start runtime; using defaults");
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("appearance-watch".into())
            .spawn(move || rt.block_on(run_watch(observer, stop_rx)));
        match spawned {
            Ok(_) => self.stops.lock().expect("stops lock").push(stop_tx),
            Err(e) => tracing::warn!("appearance watch: cannot start thread: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use installer_domain::{ControlButton, ControlSide};

    #[test]
    fn scheme_mapping() {
        assert_eq!(
            map_color_scheme(PortalScheme::PreferDark),
            ColorScheme::Dark
        );
        assert_eq!(
            map_color_scheme(PortalScheme::PreferLight),
            ColorScheme::Light
        );
        assert_eq!(
            map_color_scheme(PortalScheme::NoPreference),
            ColorScheme::Light
        );
    }

    #[test]
    fn accent_mapping() {
        // GNOME blue
        let c = map_accent((0.2078, 0.5176, 0.8941)).unwrap();
        assert_eq!(c.to_hex(), "#3584E4");
        assert_eq!(map_accent((0.0, 0.0, 0.0)).unwrap().to_hex(), "#000000");
        assert_eq!(map_accent((1.0, 1.0, 1.0)).unwrap().to_hex(), "#FFFFFF");
        // unset or garbage
        assert_eq!(map_accent((-1.0, -1.0, -1.0)), None);
        assert_eq!(map_accent((2.0, 0.5, 0.5)), None);
        assert_eq!(map_accent((f64::NAN, 0.5, 0.5)), None);
    }

    #[test]
    fn contrast_and_motion_mapping() {
        assert!(map_contrast(PortalContrast::High));
        assert!(!map_contrast(PortalContrast::NoPreference));
        assert!(map_reduced_motion(false));
        assert!(!map_reduced_motion(true));
    }

    #[test]
    fn nothing_readable_gives_defaults() {
        assert_eq!(merge(&RawSettings::default()), Appearance::default());
    }

    #[test]
    fn merge_full() {
        let a = merge(&RawSettings {
            color_scheme: Some(PortalScheme::PreferDark),
            accent: Some((1.0, 0.0, 0.0)),
            contrast: Some(PortalContrast::High),
            button_layout: Some("close,minimize,maximize:".into()),
            enable_animations: Some(false),
            text_scale: Some(1.25),
        });
        assert_eq!(a.color_scheme, ColorScheme::Dark);
        assert_eq!(a.accent.unwrap().to_hex(), "#FF0000");
        assert!(a.high_contrast && a.reduced_motion);
        assert_eq!(a.text_scale, 1.25);
        assert_eq!(a.controls.side, ControlSide::Left);
        assert_eq!(
            a.controls.buttons,
            vec![
                ControlButton::Close,
                ControlButton::Minimize,
                ControlButton::Maximize
            ]
        );
    }

    #[test]
    fn text_scale_is_clamped() {
        let at = |v| {
            merge(&RawSettings {
                text_scale: Some(v),
                ..Default::default()
            })
            .text_scale
        };
        assert_eq!(at(1.5), 1.5);
        assert_eq!(at(0.1), 0.5);
        assert_eq!(at(9.0), 3.0);
        assert_eq!(at(f64::NAN), 1.0);
        assert_eq!(merge(&RawSettings::default()).text_scale, 1.0);
    }

    #[test]
    fn bad_layout_falls_back_to_right_close() {
        let a = merge(&RawSettings {
            button_layout: Some("nonsense".into()),
            ..Default::default()
        });
        assert_eq!(a.controls, installer_domain::WindowControls::default());
    }

    #[tokio::test]
    async fn snapshot_without_session_bus_returns_a_value() {
        // No bus in CI: must still succeed.
        let src = GnomeAppearanceSource::new();
        assert!(src.snapshot().await.is_ok());
    }
}
