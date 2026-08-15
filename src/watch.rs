use std::path::{Component, Path};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, bounded};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

pub(crate) struct RepoWatcher {
    receiver: Receiver<()>,
    _watcher: RecommendedWatcher,
}

impl RepoWatcher {
    pub(crate) fn new(root: &Path) -> Result<Self> {
        let (sender, receiver) = bounded(1);
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            let Ok(event) = result else {
                return;
            };
            if should_refresh(&event) && sender.try_send(()).is_err() {
                return;
            }
        })
        .context("failed to create filesystem watcher")?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", root.display()))?;

        Ok(Self {
            receiver,
            _watcher: watcher,
        })
    }

    pub(crate) const fn changes(&self) -> &Receiver<()> {
        &self.receiver
    }
}

fn should_refresh(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|path| !is_noisy_git_path(path))
}

fn is_noisy_git_path(path: &Path) -> bool {
    let components: Vec<_> = path.components().collect();
    let Some(git_index) = components
        .iter()
        .position(|component| component.as_os_str() == ".git")
    else {
        return false;
    };
    let tail = components.get(git_index + 1..).unwrap_or_default();
    if tail
        .first()
        .is_some_and(|component| component.as_os_str() == "objects")
    {
        return true;
    }
    tail.last().is_some_and(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "index.lock" || name.starts_with(".watchman-cookie-")
    }) || components
        .iter()
        .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn filters_git_object_and_lock_noise_but_not_refs() {
        assert!(is_noisy_git_path(Path::new("/repo/.git/objects/ab/cdef")));
        assert!(is_noisy_git_path(Path::new("/repo/.git/index.lock")));
        assert!(!is_noisy_git_path(Path::new("/repo/.git/HEAD")));
        assert!(!is_noisy_git_path(Path::new("/repo/.git/refs/heads/main")));
        assert!(!is_noisy_git_path(Path::new("/repo/src/main.rs")));
    }
}
