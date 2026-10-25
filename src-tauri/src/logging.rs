//! Logging (F-60): `tracing` to stderr and a daily-rotated file under
//! `$XDG_STATE_HOME/<app-id>/log/`. Home paths are shown as `~`.
//! Never log env values or file contents.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub const APP_ID: &str = "io.github.blueyzx64.Installer";
const FILE_PREFIX: &str = "installer.log";
const KEEP_FILES: usize = 7;
/// Per-process cap on bytes written to the log file; later writes are dropped.
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const DEFAULT_FILTER: &str = "info";

/// Resolve the log directory. `env` is injected so tests need no real env.
pub fn log_dir(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |k: &str| env(k).filter(|v| !v.is_empty());
    let base = match non_empty("XDG_STATE_HOME") {
        Some(s) if Path::new(&s).is_absolute() => PathBuf::from(s),
        _ => PathBuf::from(non_empty("HOME")?).join(".local/state"),
    };
    Some(base.join(APP_ID).join("log"))
}

/// Replace every occurrence of `home` in `text` with `~`.
pub fn redact(text: &str, home: Option<&str>) -> String {
    match home.map(|h| h.trim_end_matches('/')) {
        Some(h) if !h.is_empty() => text.replace(h, "~"),
        _ => text.to_owned(),
    }
}

/// Stops writing after `cap` bytes (silently drops the rest).
struct Capped<W: Write> {
    inner: W,
    written: u64,
    cap: u64,
}

impl<W: Write> Write for Capped<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written >= self.cap {
            return Ok(buf.len());
        }
        self.written += buf.len() as u64;
        self.inner.write_all(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Shared sink that redacts the home directory from every chunk.
struct Maker<W: Write + Send + 'static> {
    sink: Arc<Mutex<W>>,
    home: Option<Arc<str>>,
}

struct Guard<'a, W: Write> {
    sink: MutexGuard<'a, W>,
    home: Option<Arc<str>>,
}

impl<W: Write> Write for Guard<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let out = redact(&text, self.home.as_deref());
        self.sink.write_all(out.as_bytes())?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.sink.flush()
    }
}

impl<'a, W: Write + Send + 'static> MakeWriter<'a> for Maker<W> {
    type Writer = Guard<'a, W>;
    fn make_writer(&'a self) -> Self::Writer {
        // A poisoned lock still holds a usable writer.
        let sink = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        Guard {
            sink,
            home: self.home.clone(),
        }
    }
}

fn maker<W: Write + Send + 'static>(w: W, home: &Option<Arc<str>>) -> Maker<W> {
    Maker {
        sink: Arc::new(Mutex::new(w)),
        home: home.clone(),
    }
}

/// Install the global subscriber. A second call is a no-op.
/// `RUST_LOG` overrides the default `info` filter.
pub fn init() {
    let env = |k: &str| std::env::var(k).ok();
    let home: Option<Arc<str>> = env("HOME").map(Arc::from);
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    let stderr = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(maker(io::stderr(), &home));

    let file = log_dir(&env).and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(FILE_PREFIX)
            .max_log_files(KEEP_FILES)
            .build(&dir)
            .ok()
    });
    let file_layer = file.map(|f| {
        let capped = Capped {
            inner: f,
            written: 0,
            cap: MAX_FILE_BYTES,
        };
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(maker(capped, &home))
    });

    // `try_init` also bridges `log` records (tauri internals) into tracing.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(file_layer)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    fn expected(base: &str) -> Option<PathBuf> {
        Some(PathBuf::from(format!("{base}/{APP_ID}/log")))
    }

    #[test]
    fn uses_xdg_state_home() {
        let e = env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")]);
        assert_eq!(log_dir(&e), expected("/s"));
    }

    #[test]
    fn falls_back_to_home() {
        let e = env(&[("HOME", "/h")]);
        assert_eq!(log_dir(&e), expected("/h/.local/state"));
    }

    #[test]
    fn ignores_empty_or_relative_xdg() {
        for x in ["", "rel/path"] {
            let e = env(&[("XDG_STATE_HOME", x), ("HOME", "/h")]);
            assert_eq!(log_dir(&e), expected("/h/.local/state"));
        }
    }

    #[test]
    fn no_dirs_no_file_log() {
        assert_eq!(log_dir(&env(&[])), None);
    }

    #[test]
    fn redacts_home() {
        assert_eq!(
            redact("open /home/sam/a /home/sam/b", Some("/home/sam")),
            "open ~/a ~/b"
        );
        assert_eq!(redact("x /home/sam/", Some("/home/sam/")), "x ~/");
        assert_eq!(redact("keep /home/sam", None), "keep /home/sam");
        assert_eq!(redact("keep", Some("")), "keep");
    }

    #[test]
    fn writer_redacts_and_caps() {
        let sink = Arc::new(Mutex::new(Capped {
            inner: Vec::new(),
            written: 0,
            cap: 7,
        }));
        let m = Maker {
            sink: sink.clone(),
            home: Some(Arc::from("/home/sam")),
        };
        m.make_writer().write_all(b"at /home/sam/x\n").unwrap();
        m.make_writer().write_all(b"dropped\n").unwrap();
        let got = sink.lock().unwrap().inner.clone();
        assert_eq!(String::from_utf8(got).unwrap(), "at ~/x\n");
    }
}
