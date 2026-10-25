//! Server-side store for opened previews, keyed by `PreviewId`.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use installer_domain::{InstallError, PackageFile, PackagePreview, PreviewId};

use crate::CancelToken;

/// Idle previews kept at most; the oldest idle one is dropped beyond this.
const MAX_IDLE: usize = 32;

#[derive(Debug, Clone)]
pub struct StoredPreview {
    pub file: PackageFile,
    pub preview: PackagePreview,
}

#[derive(Debug)]
struct Entry {
    stored: StoredPreview,
    /// Set while an install runs for this preview.
    active: Option<CancelToken>,
}

#[derive(Debug, Default)]
struct Inner {
    next: u64,
    entries: HashMap<PreviewId, Entry>,
}

#[derive(Debug, Default)]
pub struct PreviewRegistry {
    inner: Mutex<Inner>,
}

impl PreviewRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock only means another thread panicked; the map is still usable.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Allocate an id, let `build` make the preview with it, and store it.
    pub fn insert(
        &self,
        file: PackageFile,
        build: impl FnOnce(PreviewId) -> PackagePreview,
    ) -> PackagePreview {
        let mut g = self.lock();
        g.next += 1;
        let id = PreviewId::new(g.next);
        let preview = build(id);
        g.entries.insert(
            id,
            Entry {
                stored: StoredPreview {
                    file,
                    preview: preview.clone(),
                },
                active: None,
            },
        );
        let idle: Vec<PreviewId> = {
            let mut ids: Vec<_> = g
                .entries
                .iter()
                .filter(|(_, e)| e.active.is_none())
                .map(|(id, _)| *id)
                .collect();
            ids.sort();
            ids
        };
        if idle.len() > MAX_IDLE {
            for old in &idle[..idle.len() - MAX_IDLE] {
                g.entries.remove(old);
            }
        }
        preview
    }

    pub fn get(&self, id: PreviewId) -> Option<StoredPreview> {
        self.lock().entries.get(&id).map(|e| e.stored.clone())
    }

    /// Mark the preview as installing and hand out its cancel token.
    /// `NotFound` for unknown ids, `Busy` if any install is already running.
    pub fn begin(&self, id: PreviewId) -> Result<(StoredPreview, CancelToken), InstallError> {
        let mut g = self.lock();
        if !g.entries.contains_key(&id) {
            return Err(InstallError::NotFound);
        }
        // F-02: one install at a time, across all previews.
        if g.entries.values().any(|e| e.active.is_some()) {
            return Err(InstallError::Busy);
        }
        let entry = g.entries.get_mut(&id).ok_or(InstallError::NotFound)?;
        let token = CancelToken::new();
        entry.active = Some(token.clone());
        Ok((entry.stored.clone(), token))
    }

    /// End an install. Keeps the preview for a retry unless `keep` is false.
    pub fn finish(&self, id: PreviewId, keep: bool) {
        let mut g = self.lock();
        if keep {
            if let Some(e) = g.entries.get_mut(&id) {
                e.active = None;
            }
        } else {
            g.entries.remove(&id);
        }
    }

    /// Cancel a running install. `false` if nothing was running for `id`.
    pub fn cancel(&self, id: PreviewId) -> bool {
        match self.lock().entries.get(&id).and_then(|e| e.active.as_ref()) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{sample_file, sample_meta};
    use installer_domain::{Compatibility, PackageFormat};

    fn add(reg: &PreviewRegistry) -> PreviewId {
        let file = sample_file(PackageFormat::Rpm);
        reg.insert(file.clone(), |id| PackagePreview {
            id,
            size_bytes: file.size_bytes,
            meta: sample_meta(PackageFormat::Rpm),
            compat: Compatibility::Ok,
            needs_permission: true,
        })
        .id
    }

    #[test]
    fn ids_are_unique_and_lookup_works() {
        let reg = PreviewRegistry::new();
        let (a, b) = (add(&reg), add(&reg));
        assert_ne!(a, b);
        assert_eq!(reg.get(a).unwrap().preview.id, a);
        assert!(reg.get(PreviewId::new(999)).is_none());
    }

    #[test]
    fn begin_is_exclusive_and_finish_releases() {
        let reg = PreviewRegistry::new();
        let id = add(&reg);
        let (_, token) = reg.begin(id).unwrap();
        assert_eq!(reg.begin(id).unwrap_err(), InstallError::Busy);
        assert!(reg.cancel(id));
        assert!(token.is_cancelled());
        reg.finish(id, true);
        assert!(!reg.cancel(id), "nothing running now");
        assert!(reg.begin(id).is_ok(), "retry allowed");
        reg.finish(id, false);
        assert_eq!(reg.begin(id).unwrap_err(), InstallError::NotFound);
    }

    #[test]
    fn only_one_install_runs_at_a_time() {
        let reg = PreviewRegistry::new();
        let (a, b) = (add(&reg), add(&reg));
        reg.begin(a).unwrap();
        assert_eq!(reg.begin(b).unwrap_err(), InstallError::Busy);
        // Opening more previews while busy is fine.
        let c = add(&reg);
        assert!(reg.get(c).is_some());
        reg.finish(a, false);
        assert!(reg.begin(b).is_ok(), "free again after the first ends");
    }

    #[test]
    fn idle_previews_are_capped_but_running_ones_survive() {
        let reg = PreviewRegistry::new();
        let first = add(&reg);
        reg.begin(first).unwrap();
        for _ in 0..(MAX_IDLE + 5) {
            add(&reg);
        }
        assert!(reg.get(first).is_some(), "active entry never evicted");
        assert_eq!(reg.len(), MAX_IDLE + 1);
    }
}
