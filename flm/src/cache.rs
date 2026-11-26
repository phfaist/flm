//! The program's cache: the folder in which the commands that build a document (`watch`,
//! `render` and `check`) keep the results of costly work from one run to the next, the
//! manager that keeps it ([`Manager`]), its events written as log lines, and its
//! maintenance.
//!
//! The folder of the cache, its *root*, is the first of these that is given
//! ([`root`]):
//!
//! 1. the option `--cache-dir DIR`;
//! 2. the environment variable [`CACHE_DIR_VARIABLE`], `FLM_CACHE_DIR`;
//! 3. the folder `flm` in the folder of the environment variable
//!    [`XDG_CACHE_HOME_VARIABLE`], `XDG_CACHE_HOME`, on every system, when the variable
//!    holds an absolute path;
//! 4. the folder `flm` in the system's folder of a user's caches ([`system_root`]):
//!    `~/.cache/flm` on Linux and other Unix systems, `~/Library/Caches/flm` on macOS, and
//!    `flm\cache` in the folder of local application data on Windows.
//!
//! An environment variable set to the empty text counts as not set. Under `--no-cache`, or
//! when none of the four gives a root, the program keeps its cache in memory, for the life
//! of the process ([`manager`]).

use std::ffi::OsString;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::progress::{LogLevel, LogRecord, LogSink};
use flm_host::cache::cachette::{
    Manager, ManagerError, ManagerSettings, Report, Reporter, Severity, SizeLimit, SystemClock,
};
use flm_host::files::system::{from_system, to_system};
use flm_host::session::Input;

/// The environment variable that names the folder of the program's cache, `FLM_CACHE_DIR`,
/// after the option `--cache-dir` and before [`XDG_CACHE_HOME_VARIABLE`].
pub const CACHE_DIR_VARIABLE: &str = "FLM_CACHE_DIR";

/// The environment variable of the XDG Base Directory Specification that names the folder
/// of a user's caches, `XDG_CACHE_HOME`: the program keeps its cache in the folder `flm` of
/// it, on every system, when it holds an absolute path.
pub const XDG_CACHE_HOME_VARIABLE: &str = "XDG_CACHE_HOME";

/// The source of the log lines of the cache ([`ProgramReporter`], [`maintain`]).
const SOURCE: &str = "cache";

/// Returns the root of the program's cache: `option`, the folder of `--cache-dir`; else
/// `variable`, the value of [`CACHE_DIR_VARIABLE`]; else the folder `flm` in `xdg`, the
/// value of [`XDG_CACHE_HOME_VARIABLE`], when it is an absolute path; else `system`, the
/// root in the system's folder of caches ([`system_root`]). A variable whose value is
/// empty counts as not set. `None` when none of the four gives a root.
pub(crate) fn root(
    option: Option<&Path>,
    variable: Option<OsString>,
    xdg: Option<OsString>,
    system: Option<PathBuf>,
) -> Option<PathBuf> {
    let given = |value: Option<OsString>| value.filter(|value| !value.is_empty()).map(PathBuf::from);
    option
        .map(Path::to_path_buf)
        .or_else(|| given(variable))
        .or_else(|| given(xdg).filter(|folder| folder.is_absolute()).map(|folder| folder.join("flm")))
        .or(system)
}

/// Returns the root of the program's cache in the system's folder of a user's caches, as
/// the crate `dirs` gives that folder (`dirs::cache_dir`): its folder `flm`, and on Windows,
/// where that folder is the folder of local application data, `flm\cache`. `None` when the
/// system gives no such folder.
pub(crate) fn system_root() -> Option<PathBuf> {
    let folder = dirs::cache_dir()?.join("flm");
    Some(if cfg!(windows) { folder.join("cache") } else { folder })
}

/// Returns the root of the program's cache for `option`, the folder of `--cache-dir`
/// ([`root`]), with the environment variables read from the process's environment.
pub(crate) fn environment_root(option: Option<&Path>) -> Option<PathBuf> {
    root(
        option,
        std::env::var_os(CACHE_DIR_VARIABLE),
        std::env::var_os(XDG_CACHE_HOME_VARIABLE),
        system_root(),
    )
}

/// Reads `text`, the value of `--cache-max-size`: a whole number of bytes, or a whole
/// number followed by `K`, `M` or `G` for that number of KiB, MiB or GiB, the powers of
/// 1024. The size is at least one byte.
///
/// # Errors
///
/// A message for a person when `text` has none of these forms, is zero, or is larger than
/// the program can count. The parser of the command line writes it after its own words,
/// which quote `text`, so the message does not quote it again.
pub(crate) fn parse_size(text: &str) -> Result<NonZeroU64, String> {
    let (digits, unit) = match text.char_indices().last() {
        Some((at, 'K')) => (&text[..at], 1u64 << 10),
        Some((at, 'M')) => (&text[..at], 1u64 << 20),
        Some((at, 'G')) => (&text[..at], 1u64 << 30),
        _ => (text, 1),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(String::from(
            "A size must be a whole number of bytes, or a whole number followed by ‘K’, ‘M’ or ‘G’",
        ));
    }
    let size = digits
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(unit))
        .ok_or_else(|| format!("A size must be at most {} bytes", u64::MAX))?;
    NonZeroU64::new(size).ok_or_else(|| String::from("A size must be at least one byte"))
}

/// Returns the manager of the program's cache, made once per command: a manager on the
/// folder `root` ([`open`]), or, when `root` is `None`, a manager in memory
/// ([`Manager::in_memory`]), which keeps the values for the life of the process and offers
/// no folder. Its clock is the system's ([`SystemClock`]); its size limit is `size_limit`
/// bytes, or cachette's own limit when `None`; its reports go to `log`
/// ([`ProgramReporter`]). When the manager on the folder cannot be made (the folder's path
/// is not valid Unicode text), `log` receives one warning and the manager is the one in
/// memory.
pub(crate) fn manager(root: Option<&Path>, size_limit: Option<NonZeroU64>, log: Arc<dyn LogSink>) -> Manager {
    if let Some(root) = root {
        match open(root, size_limit, Arc::clone(&log)) {
            Ok(manager) => return manager,
            Err(error) => {
                let text = format!("Failed to open the cache: {}", error.message());
                log.log(&LogRecord::new(LogLevel::Warn, SOURCE, &text));
            }
        }
    }
    // A manager in memory refuses no size limit of at least one byte.
    let reporter = ProgramReporter { log };
    Manager::in_memory(settings(size_limit), SystemClock::new(), reporter).unwrap_or_else(|_| Manager::none())
}

/// Returns the manager on the folder `root` ([`Manager::open_path`]), with the system's
/// clock, the size limit `size_limit` (cachette's own limit when `None`), and its reports
/// going to `log` ([`ProgramReporter`]).
///
/// # Errors
///
/// The error of [`Manager::open_path`]: the folder's path is not valid Unicode text.
pub(crate) fn open(root: &Path, size_limit: Option<NonZeroU64>, log: Arc<dyn LogSink>) -> Result<Manager, ManagerError> {
    Manager::open_path(root, settings(size_limit), SystemClock::new(), ProgramReporter { log })
}

/// The settings of the program's manager: the size limit `size_limit`, or cachette's own
/// limit when `None`, and cachette's defaults for the rest.
fn settings(size_limit: Option<NonZeroU64>) -> ManagerSettings {
    ManagerSettings {
        size_limit: size_limit.map_or(SizeLimit::Automatic, |size| SizeLimit::Bytes(size.get())),
        ..ManagerSettings::default()
    }
}

/// Returns the text that identifies the document `input` among the documents that use the
/// root of the cache: the canonical absolute path of its file, in the form in which the
/// host writes paths ([`from_system`]); `None` for a document given as text, and when the
/// path cannot be made canonical.
pub(crate) fn document_text(input: &Input) -> Option<String> {
    let Input::File(path) = input else {
        return None;
    };
    let system = to_system(path).ok()?;
    let canonical = std::fs::canonicalize(system).ok()?;
    from_system(&canonical).ok()
}

/// Carries out the maintenance of the cache of `manager` ([`Manager::maintain`]), which
/// does its work at most once a day; an error that it returns is one log line of `log` at
/// the level `Warn`.
pub(crate) fn maintain(manager: &Manager, log: &dyn LogSink) {
    if let Err(error) = manager.maintain() {
        let text = format!("Cache maintenance failed: {}", error.message());
        log.log(&LogRecord::new(LogLevel::Warn, SOURCE, &text));
    }
}

/// Carries out the maintenance of [`maintain`] on a thread of its own, for `flm watch`,
/// which goes on serving the preview meanwhile.
pub(crate) fn maintain_in_background(manager: Manager, log: Arc<dyn LogSink>) {
    std::thread::spawn(move || maintain(&manager, &*log));
}

/// The reporter of the program's cache ([`Reporter`]): it writes each report as one log
/// line with the source `cache`, at the level `Info` for a report of the severity
/// [`Severity::Information`] and `Warn` for every other, since no event of the cache fails
/// a command.
#[derive(Clone)]
pub(crate) struct ProgramReporter {
    /// Where the lines go: the program's log sink.
    log: Arc<dyn LogSink>,
}

impl Reporter for ProgramReporter {
    fn report(&self, report: &Report) {
        let level = match report.severity {
            Severity::Information => LogLevel::Info,
            Severity::Warning | Severity::Error => LogLevel::Warn,
        };
        self.log.log(&LogRecord::new(level, SOURCE, &report.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sizes_of_cache_max_size_are_whole_numbers_of_bytes_or_of_powers_of_1024() {
        let size = |text| parse_size(text).map(NonZeroU64::get);
        assert_eq!(size("1"), Ok(1));
        assert_eq!(size("1000"), Ok(1000));
        assert_eq!(size("2K"), Ok(2048));
        assert_eq!(size("512M"), Ok(512 << 20));
        assert_eq!(size("2G"), Ok(2 << 30));
        for refused in ["", "3k", "0", "0G", "K", "1.5G", "-1", "+1", "1 G", "1KB", "1T", "G1", "ten", "99999999999G"] {
            assert!(parse_size(refused).is_err(), "{refused:?} is accepted");
        }
    }

    #[test]
    fn the_root_is_the_first_of_the_option_the_variables_and_the_system_s_folder() {
        let option = Path::new("given");
        let variable = || Some(OsString::from("from-variable"));
        let xdg = if cfg!(windows) { "C:\\xdg" } else { "/xdg" };
        let system = || Some(PathBuf::from("system"));
        // Each source in turn, with every later one given.
        assert_eq!(root(Some(option), variable(), Some(xdg.into()), system()), Some(PathBuf::from("given")));
        assert_eq!(root(None, variable(), Some(xdg.into()), system()), Some(PathBuf::from("from-variable")));
        assert_eq!(root(None, None, Some(xdg.into()), system()), Some(Path::new(xdg).join("flm")));
        assert_eq!(root(None, None, None, system()), Some(PathBuf::from("system")));
        assert_eq!(root(None, None, None, None), None);
        // An empty variable is not set; `XDG_CACHE_HOME` counts only as an absolute path.
        assert_eq!(root(None, Some(OsString::new()), Some(xdg.into()), None), Some(Path::new(xdg).join("flm")));
        assert_eq!(root(None, None, Some(OsString::new()), system()), Some(PathBuf::from("system")));
        assert_eq!(root(None, None, Some("relative".into()), system()), Some(PathBuf::from("system")));
        // The system's folder: `flm` in the folder of caches (`flm\cache` on Windows).
        if let Some(folder) = system_root() {
            let expected = if cfg!(windows) {
                Path::new("flm").join("cache")
            } else if cfg!(target_os = "macos") {
                Path::new("Library").join("Caches").join("flm")
            } else {
                PathBuf::from("flm")
            };
            assert!(folder.ends_with(&expected), "{folder:?}");
        }
    }
}
