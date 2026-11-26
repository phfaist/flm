//! The `flm` program: one call of [`flm::run`] with the process's arguments and what
//! this executable carries ([`Carried`]): the compiled packages it carries in addition to
//! the host's own, and the tables of the extensions shipped with FLM
//! ([`flm::shipped_extensions`]: cargo feature `bundled-extensions`, empty tables
//! without it).
//!
//! The compiled packages are listed in the function `compiled_packages`. When one of them
//! cannot be made, the program prints `flm: ` and the error on standard error and exits
//! with status 1, before reading any argument.

use std::process::ExitCode;

use flm::Carried;
use flm_core::core::registry::{PackageError, PackageFactory};

fn main() -> ExitCode {
    match compiled_packages() {
        Ok(packages) => {
            let carried = Carried { packages, bundled: flm::shipped_extensions() };
            flm::run(std::env::args_os().skip(1), carried)
        }
        Err(error) => {
            eprintln!("flm: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Returns the compiled packages that the program carries besides the host's own: none.
/// The packages shipped with FLM come with the extensions compiled into the program
/// ([`flm::shipped_extensions`]). A program of its own over [`flm::run`] lists
/// its own package factories in a function of this shape.
///
/// # Errors
///
/// The error of a package factory that cannot be made.
fn compiled_packages() -> Result<Vec<PackageFactory>, PackageError> {
    Ok(Vec::new())
}
