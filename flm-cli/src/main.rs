//! The `flm` program: one call of [`flm_cli::run`] with the process's arguments and the
//! compiled packages that this executable carries in addition to the host's own.
//!
//! The compiled packages are listed in the function `compiled_packages`. When one of them
//! cannot be made, the program prints `flm: ` and the error on standard error and exits
//! with status 1, before reading any argument.

use std::process::ExitCode;

use flm_core::core::registry::{PackageError, PackageFactory};

fn main() -> ExitCode {
    match compiled_packages() {
        Ok(packages) => flm_cli::run(std::env::args_os().skip(1), packages),
        Err(error) => {
            eprintln!("flm: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Returns the compiled packages that the program carries besides the host's own: none.
/// The packages shipped with FLM come with the extensions compiled into the program (the
/// cargo feature `bundled-extensions`). A program of its own over [`flm_cli::run`] lists
/// its own package factories in a function of this shape.
///
/// # Errors
///
/// The error of a package factory that cannot be made.
fn compiled_packages() -> Result<Vec<PackageFactory>, PackageError> {
    Ok(Vec::new())
}
