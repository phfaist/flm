//! `flm cache`: lists the instances of the program's cache, and removes them.
//!
//! The cache is the one that `flm render`, `flm check` and `flm watch` use, in the folder
//! that `--cache-dir`, the environment variables and the system give, in that order
//! ([`crate::cache`]). Each cache in it, under its name (`flm.graphics-files.magick.converted`),
//! has *instances*, each with its own data, selected by an *instance key*, a JSON value (a
//! cache per document has one instance per document).
//!
//! - `flm cache list` prints the cache's folder, then one line per instance: the cache's
//!   name, its version, the instance key as JSON text, the instance's size, its last use
//!   (in UTC), and `damaged` for an instance whose data cannot be read. The instances are in
//!   the order of their caches' names, then of their keys.
//! - `flm cache clean NAME…` removes every instance of each cache named, and
//!   `flm cache clean --all` every instance of every cache; then it prints the number of
//!   instances removed and their total size. An instance that a running program uses is
//!   left in place, with a warning.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, Subcommand};
use flm_host::cache::cachette::{
    CleaningStatus, InstanceInfo, InstanceSelection, Manager, ManagerError, ManagerErrorKind,
};

use crate::{CANNOT_START, DOCUMENT_ERRORS, DONE, PROGRAM_FAILURE, ProgramLog, reporter};

/// The arguments of `flm cache`: what to do with the program's cache.
#[derive(Debug, Args)]
pub struct CacheArgs {
    /// What to do.
    #[command(subcommand)]
    pub action: CacheAction,
}

/// What `flm cache` does.
#[derive(Debug, Subcommand)]
pub enum CacheAction {
    /// Print the cache's folder, then one line per instance of a cache: the cache's name, its
    /// version, the instance key as JSON text, the size, the last use (UTC), and `damaged`
    /// for an instance whose data cannot be read.
    List(CacheListArgs),
    /// Remove every instance of each cache named, or of every cache with `--all`, and print
    /// the number of instances removed and their total size.
    Clean(CacheCleanArgs),
}

/// The arguments of `flm cache list`.
#[derive(Debug, Args)]
pub struct CacheListArgs {
    /// The folder of the cache. Default: as for `flm render`, the folder of the environment
    /// variable `FLM_CACHE_DIR`; else the folder `flm` of `XDG_CACHE_HOME`, when that
    /// variable holds an absolute path; else the system's folder of caches.
    #[arg(long = "cache-dir", value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,
}

/// The arguments of `flm cache clean`.
#[derive(Debug, Args)]
pub struct CacheCleanArgs {
    /// The names of the caches whose instances are removed, as `flm cache list` prints them.
    #[arg(value_name = "NAME", required_unless_present = "all", conflicts_with = "all")]
    pub names: Vec<String>,
    /// Remove every instance of every cache.
    #[arg(long = "all")]
    pub all: bool,
    /// The folder of the cache. Default: as for `flm cache list`.
    #[arg(long = "cache-dir", value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,
}

/// Runs `flm cache` with `args`, and returns the exit status: 0 when the command did what
/// was asked, also when it left an instance in use in place; 1 when the cache cannot be
/// read or an instance cannot be removed; 2 when no folder of the cache is given or found,
/// its path is not valid Unicode text, or a name is not a cache's name; 3 when standard
/// output cannot be written.
pub(crate) fn run(args: &CacheArgs) -> u8 {
    let cache_dir = match &args.action {
        CacheAction::List(list) => list.cache_dir.as_deref(),
        CacheAction::Clean(clean) => clean.cache_dir.as_deref(),
    };
    let Some(root) = crate::cache::environment_root(cache_dir) else {
        reporter().error("Missing folder of the cache: use ‘--cache-dir DIR’, or set ‘FLM_CACHE_DIR’");
        return CANNOT_START;
    };
    let log = Arc::new(ProgramLog { verbose: false });
    let manager = match crate::cache::open(&root, None, log) {
        Ok(manager) => manager,
        Err(error) => return failed(&error),
    };
    let done = match &args.action {
        CacheAction::List(_) => list(&manager, &root),
        CacheAction::Clean(clean) => remove(&manager, clean),
    };
    match done {
        Ok(text) => print(&text),
        Err(Failure::Manager(error)) => failed(&error),
        Err(Failure::Busy) => {
            reporter().error("Failed to clean the cache: another program is cleaning it or doing its maintenance");
            DOCUMENT_ERRORS
        }
    }
}

/// Why `flm cache` stopped.
enum Failure {
    /// The manager returned an error.
    Manager(ManagerError),
    /// Another program holds the root for its maintenance or a cleaning.
    Busy,
}

impl From<ManagerError> for Failure {
    fn from(error: ManagerError) -> Failure {
        Failure::Manager(error)
    }
}

/// Reports `error`, which the manager returned, and returns the exit status: 2 for an
/// error of the configuration (a path or a name that cachette refuses), 1 for every other.
fn failed(error: &ManagerError) -> u8 {
    reporter().error(error.message());
    if error.kind() == ManagerErrorKind::Configuration { CANNOT_START } else { DOCUMENT_ERRORS }
}

/// Writes `text` on standard output, and returns the exit status: 0, or 3 when standard
/// output cannot be written.
fn print(text: &str) -> u8 {
    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(text.as_bytes()).and_then(|()| stdout.flush()) {
        Ok(()) => DONE,
        Err(error) => {
            reporter().error(&format!("Failed to write standard output: {error}"));
            PROGRAM_FAILURE
        }
    }
}

/// One instance as `flm cache` shows it: what the listing states, and its size, or `None`
/// when its folder was removed after the listing or it cannot be measured.
struct Shown {
    info: InstanceInfo,
    size: Option<u64>,
}

/// The instances under the root of `manager` that `selected` accepts, given the name of
/// their cache, with their sizes, in the order of their caches' names, then of their keys
/// (an instance without a readable key first), then of their versions. An instance that
/// cannot be measured is warned about and shown with no size.
fn instances(manager: &Manager, selected: impl Fn(&str) -> bool) -> Result<Vec<Shown>, ManagerError> {
    let mut shown = Vec::new();
    for info in manager.instances()?.into_iter().filter(|info| selected(&info.cache)) {
        let size = match manager.instance_details(&info) {
            Ok(details) => details.map(|details| details.size),
            Err(error) => {
                let key = info.key.as_ref().map_or("-", |key| key.as_json());
                reporter().warning(&format!(
                    "Failed to measure instance ‘{key}’ of cache ‘{}’: {}",
                    info.cache,
                    error.message()
                ));
                None
            }
        };
        shown.push(Shown { info, size });
    }
    shown.sort_by(|a, b| {
        let order = |shown: &Shown| {
            let info = &shown.info;
            (info.cache.clone(), info.key.as_ref().map(|key| key.as_json().to_owned()), info.version)
        };
        order(a).cmp(&order(b))
    });
    Ok(shown)
}

/// The text of `flm cache list` for the cache of `manager`, whose folder is `root`: the
/// folder, then one line per instance, its columns aligned.
fn list(manager: &Manager, root: &Path) -> Result<String, Failure> {
    let rows: Vec<[String; 6]> = instances(manager, |_| true)?
        .into_iter()
        .map(|shown| {
            let info = &shown.info;
            [
                info.cache.clone(),
                info.version.map_or_else(|| String::from("-"), |version| version.to_string()),
                info.key.as_ref().map_or_else(|| String::from("-"), |key| key.as_json().to_owned()),
                shown.size.map_or_else(|| String::from("-"), size_text),
                info.last_use.map_or_else(|| String::from("-"), |time| utc_text(time.0)),
                String::from(if info.damaged.is_some() { "damaged" } else { "" }),
            ]
        })
        .collect();
    let mut widths = [0; 6];
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut text = format!("Cache folder: {}\n", root.display());
    for row in &rows {
        let mut line = String::new();
        for (cell, width) in row.iter().zip(widths) {
            line.push_str(&format!("{cell:width$}  "));
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }
    Ok(text)
}

/// Removes the instances of the cache of `manager` that `args` selects, once every name it
/// gives is checked (an invalid name removes nothing), warns about each selected instance
/// that a running program uses, and returns the text that tells the number of instances
/// removed and their total size, a lower bound (`at least`) when a removed instance has no
/// known size.
fn remove(manager: &Manager, args: &CacheCleanArgs) -> Result<String, Failure> {
    // Every name is checked before any instance is removed, so that an invalid name stops
    // the command with nothing removed. A manager that keeps nothing only checks the name.
    for name in &args.names {
        Manager::none().remove_instances(InstanceSelection::Cache(name))?;
    }
    let selected = |cache: &str| args.all || args.names.iter().any(|name| name == cache);
    let known: HashMap<String, Shown> =
        instances(manager, selected)?.into_iter().map(|shown| (shown.info.folder.clone(), shown)).collect();
    let selections: Vec<InstanceSelection<'_>> = if args.all {
        vec![InstanceSelection::All]
    } else {
        args.names.iter().map(|name| InstanceSelection::Cache(name)).collect()
    };
    let (mut count, mut size, mut unmeasured) = (0usize, 0u64, false);
    for selection in selections {
        let outcome = manager.remove_instances(selection)?;
        if outcome.status == CleaningStatus::Busy {
            return Err(Failure::Busy);
        }
        for folder in &outcome.removed {
            count += 1;
            match known.get(folder).and_then(|shown| shown.size) {
                Some(bytes) => size += bytes,
                None => unmeasured = true,
            }
        }
        for folder in &outcome.in_use {
            let (cache, key) = known.get(folder).map_or(("", "-"), |shown| {
                (shown.info.cache.as_str(), shown.info.key.as_ref().map_or("-", |key| key.as_json()))
            });
            reporter().warning(&format!("Instance ‘{key}’ of cache ‘{cache}’ not removed: the instance is in use"));
        }
    }
    let instances = if count == 1 { "instance" } else { "instances" };
    let bound = if unmeasured { "at least " } else { "" };
    Ok(format!("Removed {count} {instances} ({bound}{})\n", size_text(size)))
}

/// The size `bytes` for a person: in bytes below 1 KiB, otherwise in KiB, MiB, GiB or TiB,
/// the powers of 1024, with one decimal (`1.5 MiB`).
fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// The time `millis`, in milliseconds since the Unix epoch, as a date and a time of UTC to
/// the minute (`2026-11-24 13:05`).
fn utc_text(millis: i64) -> String {
    let minutes = millis.div_euclid(60_000);
    let (days, minute) = (minutes.div_euclid(1440), minutes.rem_euclid(1440));
    // The civil date of a day count (the algorithm `civil_from_days` of Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", minute / 60, minute % 60)
}
