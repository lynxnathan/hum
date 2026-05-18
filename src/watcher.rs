use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{PollWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, new_debouncer_opt, DebounceEventResult, NoCache};
use tokio::sync::mpsc::Sender;

use crate::events::DaemonEvent;

/// Returns true if any watched path starts with "/mnt/", indicating a
/// Windows-mounted NTFS filesystem under WSL2 where inotify is silent.
fn paths_need_poll(paths: &[&Path]) -> bool {
    paths.iter().any(|p| {
        p.to_str()
            .map(|s| s.starts_with("/mnt/"))
            .unwrap_or(false)
    })
}

/// Start watching the given paths for changes.
///
/// Each detected change sends a `DaemonEvent::FileChanged` over `tx`.
/// Debounce collapses editor temp-rename save patterns (~80ms window).
///
/// If any path starts with `/mnt/`, all paths use `PollWatcher` instead of
/// inotify (inotify does not fire for NTFS mounts in WSL2).
///
/// The debouncer is leaked via `std::mem::forget` -- it runs until process exit.
pub fn start_watcher(
    paths: &[PathBuf],
    tx: Sender<DaemonEvent>,
) -> anyhow::Result<()> {
    let path_refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
    let debounce = Duration::from_millis(80);

    let callback = move |result: DebounceEventResult| {
        if let Ok(events) = result {
            for debounced_event in events {
                // DebouncedEvent derefs to notify::Event which has .paths: Vec<PathBuf>
                for path in &debounced_event.event.paths {
                    if tx.blocking_send(DaemonEvent::FileChanged(path.clone())).is_err() {
                        tracing::warn!("event channel full or closed -- dropping FileChanged event");
                    }
                }
            }
        }
    };

    if paths_need_poll(&path_refs) {
        tracing::warn!(
            "watching /mnt/ path -- using PollWatcher (inotify unavailable for NTFS)"
        );
        let config = notify::Config::default().with_poll_interval(Duration::from_millis(500));
        let mut debouncer =
            new_debouncer_opt::<_, PollWatcher, _>(debounce, None, callback, NoCache::new(), config)?;
        for p in paths {
            debouncer.watch(p, RecursiveMode::NonRecursive)?;
        }
        std::mem::forget(debouncer);
    } else {
        let mut debouncer = new_debouncer(debounce, None, callback)?;
        for p in paths {
            debouncer.watch(p, RecursiveMode::NonRecursive)?;
        }
        std::mem::forget(debouncer);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mnt_path() {
        let mnt = PathBuf::from("/mnt/c/Users/me/piece.hum");
        let refs: Vec<&Path> = vec![mnt.as_path()];
        assert!(paths_need_poll(&refs));
    }

    #[test]
    fn native_path_no_poll() {
        let native = PathBuf::from("/home/user/code/hum/piece.hum");
        let refs: Vec<&Path> = vec![native.as_path()];
        assert!(!paths_need_poll(&refs));
    }

    #[test]
    fn mixed_paths_force_poll() {
        let native = PathBuf::from("/home/user/code/hum/piece.hum");
        let mnt = PathBuf::from("/mnt/d/projects/piece.hum");
        let refs: Vec<&Path> = vec![native.as_path(), mnt.as_path()];
        assert!(paths_need_poll(&refs));
    }

    #[test]
    fn empty_paths_no_poll() {
        let refs: Vec<&Path> = vec![];
        assert!(!paths_need_poll(&refs));
    }
}
