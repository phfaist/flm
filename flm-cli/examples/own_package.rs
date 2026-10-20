//! A program of its own over the `flm` command line: the commands of `flm`, with the
//! package `phfpack` carried as a package of the program's own. Any other crate's
//! package factory goes in the list the same way.
//!
//! ```text
//! cargo run -p flm-cli --example own_package -- render paper.flm
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    let packages = vec![flm_phfpack::factory().expect("the package phfpack")];
    flm_cli::run(std::env::args_os().skip(1), packages)
}
