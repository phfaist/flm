//! Watching files on the disk: a [`Watcher`] reports which paths of a set changed.
//!
//! The watch loop of the live preview ([`Watch`](super::Watch)) uses a `Watcher` to build
//! the document again when a file it read changes. The set of paths to watch, the
//! *watched set*, is a list of paths in the form [`crate::files::FileRead`] records them:
//! files read, files looked for and not found, and folders listed. The watch loop gives
//! the watcher the new set after each build ([`Watcher::watch`]).
//!
//! A `Watcher` watches the folders that hold those paths, and each listed folder itself,
//! and keeps only the changes that concern a path of the set: a file of the set written,
//! created, removed or renamed, or an entry of a listed folder appearing or disappearing.
//! A change to another file of those folders is ignored. The watcher watches folders
//! rather than files for two reasons: many editors save a file by writing a new file and
//! renaming it over the old one, which a watch on the file itself misses on some systems;
//! and a file looked for and not found can only be watched through its folder.
//!
//! The changes of one save arrive as several events from the system, often within a few
//! milliseconds. The watcher therefore waits for a *quiet period*, [`QUIET_PERIOD`] without
//! a further change, and then reports the changes together, once ([`Seen::Changed`]). The
//! crate `notify-debouncer-full`, over the crate `notify`, does the waiting, the joining of
//! events, and the matching of the two halves of a rename.
//!
//! A path of the set is matched with the system's events through its folder's real path
//! (absolute, with symbolic links resolved), so that a path reached through a symbolic
//! link, or written relative to the current folder, is recognized in the events about its
//! file.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::event::{AccessKind, EventKind, MetadataKind, ModifyKind};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::files::fold;

/// How long a [`Watcher`] waits after a change for a further change before it reports
/// the changes together: 100 milliseconds.
pub const QUIET_PERIOD: Duration = Duration::from_millis(100);

/// What a [`Watcher`] reports to the function given to [`Watcher::new`]: see the [module
/// documentation](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// These paths of the watched set changed: each path spelled as in the set, once, in the
    /// set's order.
    Changed(Vec<PathBuf>),
    /// The system reported a problem with watching (for example an event lost, or a limit
    /// of the system reached), in the system's words.
    Failed(String),
}

/// A problem with watching files: which operation failed ([`WatchCondition`]), and the
/// system's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchError {
    /// Which operation failed.
    pub condition: WatchCondition,
    /// The problem, as the system states it.
    pub detail: String,
}

/// The operation that failed, as a [`WatchError`] reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchCondition {
    /// The system's watching could not be started ([`Watcher::new`]).
    Start,
    /// This folder could not be watched ([`Watcher::watch`]).
    Folder(PathBuf),
    /// The system reported a problem while watching ([`Seen::Failed`]), for example an
    /// event lost or a limit of the system reached.
    Watching,
}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.condition {
            WatchCondition::Start => write!(f, "files cannot be watched: {}", self.detail),
            WatchCondition::Folder(folder) => {
                write!(f, "the folder `{}` cannot be watched: {}", folder.display(), self.detail)
            }
            WatchCondition::Watching => write!(f, "the system reported a problem while watching files: {}", self.detail),
        }
    }
}

impl std::error::Error for WatchError {}

/// The watched set, by the real path of each of its paths: see the [module
/// documentation](self).
#[derive(Debug, Default)]
struct WatchedSet {
    /// Each path of the set, by its real path, with its position in the set and the path
    /// as the set writes it.
    paths: BTreeMap<PathBuf, (usize, PathBuf)>,
    /// The folders of the set that exist, by their real path: an entry of one appearing or
    /// disappearing is a change of the folder.
    folders: BTreeMap<PathBuf, (usize, PathBuf)>,
}

impl WatchedSet {
    /// The paths of the set that the system's events `events` concern, each spelled as in
    /// the set, once each, in the set's order.
    fn named(&self, events: &[PathBuf]) -> Vec<PathBuf> {
        let mut named: BTreeMap<usize, PathBuf> = BTreeMap::new();
        for event in events {
            let place = real_place(event);
            if let Some((index, path)) = self.paths.get(&place) {
                named.insert(*index, path.clone());
            }
            if let Some((index, path)) = place.parent().and_then(|folder| self.folders.get(folder)) {
                named.insert(*index, path.clone());
            }
        }
        named.into_values().collect()
    }
}

/// The real path of `path`: absolute, `.` and `..` folded, its folder's symbolic links
/// resolved where the folder exists (and the path's own, when it is a folder).
fn real_place(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        fold(path)
    } else {
        std::env::current_dir().map(|current| fold(&current.join(path))).unwrap_or_else(|_| fold(path))
    };
    if absolute.is_dir() {
        return absolute.canonicalize().unwrap_or(absolute);
    }
    match (absolute.parent(), absolute.file_name()) {
        (Some(folder), Some(name)) => folder.canonicalize().map(|folder| folder.join(name)).unwrap_or(absolute),
        _ => absolute,
    }
}

/// Whether the system's event of kind `kind` may be a change: a read is none.
fn may_change(kind: &EventKind) -> bool {
    !matches!(
        kind,
        EventKind::Access(AccessKind::Any | AccessKind::Read | AccessKind::Open(_) | AccessKind::Other)
            | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))
    )
}

/// Watches a set of paths on the disk and reports which of them changed: see the [module
/// documentation](self).
///
/// Create it with [`Watcher::new`], which takes the function that receives the reports,
/// and give it its set with [`Watcher::watch`]. Dropping the watcher stops it.
pub struct Watcher {
    debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
    set: Arc<Mutex<WatchedSet>>,
    folders: BTreeSet<PathBuf>,
}

impl fmt::Debug for Watcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Watcher").field("folders", &self.folders).finish_non_exhaustive()
    }
}

impl Watcher {
    /// Creates a watcher with an empty set, which calls `report` on a thread of its own
    /// with each report ([`Seen`]); [`Watcher::watch`] gives it its set. Dropping the
    /// watcher stops it.
    ///
    /// # Errors
    ///
    /// Returns a [`WatchError`] of condition [`WatchCondition::Start`] when the system's
    /// file watching cannot be started.
    pub fn new(mut report: impl FnMut(Seen) + Send + 'static) -> Result<Watcher, WatchError> {
        let set = Arc::new(Mutex::new(WatchedSet::default()));
        let shared = Arc::clone(&set);
        let handler = move |result: DebounceEventResult| match result {
            Ok(events) => {
                let paths: Vec<PathBuf> = events
                    .iter()
                    .filter(|event| may_change(&event.kind))
                    .flat_map(|event| event.paths.iter().cloned())
                    .collect();
                let named = shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).named(&paths);
                if !named.is_empty() {
                    report(Seen::Changed(named));
                }
            }
            Err(errors) => {
                for error in errors {
                    report(Seen::Failed(error.to_string()));
                }
            }
        };
        let debouncer = new_debouncer(QUIET_PERIOD, None, handler)
            .map_err(|error| WatchError { condition: WatchCondition::Start, detail: error.to_string() })?;
        Ok(Watcher { debouncer, set, folders: BTreeSet::new() })
    }

    /// Makes `paths` the watched set, in place of the previous set: watches the folders
    /// that hold the paths and the folders among the paths, and stops watching the folders
    /// that no path of the set needs any more. A folder that does not exist is not watched;
    /// a path in it is watched once its folder exists and a later build gives the set
    /// again.
    ///
    /// # Errors
    ///
    /// Returns one [`WatchError`] of condition [`WatchCondition::Folder`] for each folder
    /// that could not be watched; the other folders are watched.
    pub fn watch(&mut self, paths: &[PathBuf]) -> Result<(), Vec<WatchError>> {
        let mut set = WatchedSet::default();
        let mut folders = BTreeSet::new();
        for (index, path) in paths.iter().enumerate() {
            let place = real_place(path);
            if place.is_dir() {
                folders.insert(place.clone());
                set.folders.entry(place.clone()).or_insert((index, path.clone()));
            }
            if let Some(folder) = place.parent().filter(|folder| folder.is_dir()) {
                folders.insert(folder.to_path_buf());
            }
            set.paths.entry(place).or_insert((index, path.clone()));
        }
        let mut errors = Vec::new();
        for folder in self.folders.difference(&folders) {
            // A folder removed from the disk is no longer watched by the system either.
            let _ = self.debouncer.unwatch(folder);
        }
        let added: Vec<PathBuf> = folders.difference(&self.folders).cloned().collect();
        for folder in added {
            if let Err(error) = self.debouncer.watch(&folder, RecursiveMode::NonRecursive) {
                folders.remove(&folder);
                errors.push(WatchError { condition: WatchCondition::Folder(folder), detail: error.to_string() });
            }
        }
        *self.set.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = set;
        self.folders = folders;
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// Returns the folders being watched, each as its real path.
    pub fn folders(&self) -> impl Iterator<Item = &Path> {
        self.folders.iter().map(PathBuf::as_path)
    }
}

#[cfg(test)]
mod tests {
    use super::{WatchCondition, WatchError};

    #[test]
    fn each_condition_states_its_own_text() {
        let error = |condition| WatchError { condition, detail: String::from("lost") }.to_string();
        assert_eq!(error(WatchCondition::Start), "files cannot be watched: lost");
        assert_eq!(error(WatchCondition::Folder("doc".into())), "the folder `doc` cannot be watched: lost");
        assert_eq!(error(WatchCondition::Watching), "the system reported a problem while watching files: lost");
    }
}
