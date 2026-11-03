//! A program of its own over the `flm` command line: the commands of `flm`, with a package
//! of the program's own, `shout` (the module `shout` beside this file), whose macro
//! `\shout{text}` writes its argument in bold. A document that lists the package in its
//! front matter (`use: [shout]`) can write `\shout`. Any other crate's package factory goes
//! in the list the same way.
//!
//! ```text
//! cargo run -p flm-cli --example own_package -- render paper.flm
//! ```

mod shout;

use std::process::ExitCode;

fn main() -> ExitCode {
    let packages = vec![shout::factory().expect("the package shout")];
    flm_cli::run(std::env::args_os().skip(1), packages)
}
