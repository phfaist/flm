//! Opening an address or a file for the person at the machine: the preview's address in a
//! browser ([`open_address`]), and a written output (`flm render -V`) in the system's
//! default application for it ([`open_file`]).
//!
//! The browser that opens the preview's address can be chosen with the environment variable
//! [`BROWSER_VARIABLE`]; a written output always opens in the application that the system
//! associates with its file.

use std::ffi::{OsStr, OsString};
use std::path::Path;

/// The environment variable that gives the program that opens the preview's address, in
/// place of the system's default browser: `BROWSER`, the variable that other command-line
/// tools which open a browser read (`cargo doc --open` among them). The program is run with
/// the address as its only argument. The variable plays no part in opening a written
/// output (`flm render -V`).
pub const BROWSER_VARIABLE: &str = "BROWSER";

/// Opens the web address `url` in a browser: the program `browser` (the value of
/// [`BROWSER_VARIABLE`]) when it is given and not empty, else the system's default browser.
/// Returns once the program has started, without waiting for it to end.
///
/// # Errors
///
/// A message holding the system's error when the program cannot be started.
pub fn open_address(url: &str, browser: Option<OsString>) -> Result<(), String> {
    match browser.filter(|program| !program.is_empty()) {
        Some(program) => match std::process::Command::new(&program).arg(url).spawn() {
            Ok(mut child) => {
                // Waited for on a thread of its own, so that it leaves nothing behind once it ends.
                std::thread::spawn(move || child.wait());
                Ok(())
            }
            Err(error) => Err(format!("`{}` cannot be run: {error}", program.to_string_lossy())),
        },
        None => open_with_system(OsStr::new(url)),
    }
}

/// Opens the file at `path` with the system's default application for it. Returns once the
/// application has started, without waiting for it to end.
///
/// # Errors
///
/// A message holding the system's error when no application can be started for the file.
pub fn open_file(path: &Path) -> Result<(), String> {
    open_with_system(path.as_os_str())
}

/// Hands `target`, an address or a file's path, to the system's default application for it
/// (through the crate `open`), without waiting for the application to end.
fn open_with_system(target: &OsStr) -> Result<(), String> {
    open::that_detached(target).map_err(|error| error.to_string())
}
