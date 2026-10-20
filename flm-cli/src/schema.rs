//! `flm schema`: prints the JSON Schema of a settings file, or writes it to a file.
//!
//! The schema describes, in JSON Schema, the keys that a settings file may hold and the
//! values that each key accepts, so that a text editor can check such a file while it is
//! written. `--of` chooses the kind of file ([`SchemaOf`]): an extension's manifest (the
//! default) or the front matter of a document. What the schema covers depends on whether a
//! document is given:
//!
//! - without a document, the features of the library's packages `stdcore` and
//!   `math-passthrough`, of the host's packages `stdmain` and `std`, of every compiled
//!   package that the executable carries, and of the packages that a document which
//!   specifies no package uses: `std` and the package of the program's math processor
//!   ([`crate::default_math`]), found with no folder on the search path; among them the
//!   bundled extension `math-mathjax`, when the program runs JavaScript extensions and
//!   carries the bundled extensions (cargo features `js` and `bundled-extensions`); one
//!   schema then serves every file of its kind;
//! - with a document (`flm schema --of front-matter paper.flm`), the features of the
//!   packages that this document uses: those of its initial list (by default `std`, with
//!   `stdcore` and `stdmain`, which it uses), the package of its math processor (its front
//!   matter's `math`, else the program's, [`crate::default_math`]) and the packages its
//!   front matter lists under `use`, compiled into the program or extensions found as `flm render`
//!   finds them ([`flm_main::session::document_packages`]), on the search path that
//!   `--extensions-dir`, `FLMPATH` and the user's folder give; a compiled package that the
//!   front matter does not list is left out.
//!
//! The schema suggests values for an entry of `use` and of `initial_use`, and for `math`,
//! which an editor offers as completions and which restrict nothing
//! ([`flm_main::packages::suggestions`]): the packages compiled into the program, the
//! extensions installed in the folders of the search path, by their locators (`style`,
//! `@acme/style`), and the extensions bundled into the program; for `math`, `NAME` for each
//! of them named `math-NAME`. No extension is loaded, so the packages an extension provides
//! besides its default one (`LOCATOR#PACKAGE`) are not suggested. The search path is the one
//! the command would use: with a document, the document's; without one, that of a document
//! given as text (`-c`): `_flmextensions` in the current folder, the folders of `FLMPATH`
//! and the user's folder. A folder that cannot be listed adds nothing.
//!
//! The texts of the schema come from their owners: each feature's descriptions, from the
//! declaration of its settings; the form of an extension's manifest, from
//! [`flm_main::packages::extension_schema`]; the form of a front matter, from
//! [`flm_main::seed::front_matter_schema`]. This module chooses the schema, prints it and
//! writes it. The preview (`flm watch`) starts its page's *Settings* tool, which edits the
//! features that the front matter sets, with the front matter's schema made without a
//! document ([`schema`]), suggesting what the document's search path holds; after each
//! build that prepared the document again, the tool takes the schema of the packages of
//! that preparation ([`flm_main::watch::Preparation::settings_schema`]), the schema that
//! this command prints for the document.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, ValueEnum};
use flm_core::core::registry::PackageFactory;
use flm_main::extensions::ExtensionFinder;
use flm_main::files::{BuildFiles, DiskFiles, Files, ServiceFiles};
use flm_main::output::{Naming, write_file};
use flm_main::packages::{ExtensionFolder, carried, extension_schema, suggestions};
use flm_main::seed::front_matter_schema;
use flm_main::session::{Input, Request, document_packages};
use flm_main::stdpkg::{self, StdParts};

use crate::args::{BuildArgs, environment_search_path};
use crate::{CANNOT_START, DOCUMENT_ERRORS, DONE, PROGRAM_FAILURE, cannot_start, reporter};

/// The kind of file that a schema describes: the values of `flm schema --of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaOf {
    /// An extension's manifest (`NAME.yaml`, `flmextension.yaml`).
    Extension,
    /// The front matter of a document: its metadata and the settings under `flm`.
    FrontMatter,
}

/// The arguments of `flm schema`.
#[derive(Debug, Clone, Args)]
pub struct SchemaArgs {
    /// The kind of file that the schema describes: an extension's manifest
    /// (`NAME.yaml`, `flmextension.yaml`), or the front matter of a document.
    #[arg(long = "of", value_enum, default_value_t = SchemaOf::Extension)]
    pub of: SchemaOf,
    /// A document whose packages the schema covers: those of its initial list (by default
    /// `std`, with `stdcore` and `stdmain`), the package of its math processor (its front
    /// matter's `math`, else the program's) and the packages its front matter lists under
    /// `use`, compiled into the program or extensions found as `flm render` finds them.
    /// Default: the compiled packages that the program carries, `std` and the package of
    /// the program's math processor.
    #[arg(value_name = "DOCUMENT")]
    pub document: Option<String>,
    /// A folder in which extensions (`NAME.yaml`, `NAME/flmextension.yaml`) are looked for
    /// by name, before the folders of the environment variable `FLMPATH` and
    /// `~/.flm/extensions`, and the extensions compiled into the program last. Repeatable;
    /// each folder must exist. Default: the folder `_flmextensions` beside the document.
    /// Requires a document.
    #[arg(long = "extensions-dir", value_name = "DIR", requires = "document")]
    pub extensions_dirs: Vec<PathBuf>,
    /// The file to write the schema to; `-` (the default) prints it on standard output.
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<String>,
}

/// Returns the schema of the kind of file `of`, covering the features of `stdcore`, of
/// `stdmain`, of `std`, of `math-passthrough`, of `packages`, the other compiled
/// packages that the executable carries, and of the package of the program's math
/// processor ([`crate::default_math`]), which is the bundled extension `math-mathjax` when
/// the program carries it; and suggesting for `use`, `initial_use` and `math` the compiled
/// packages, the extensions installed in the folders `folders` (a search path) and the
/// bundled extensions ([`flm_main::packages::suggestions`]). The same folders, holding the
/// same entries, give the same schema. The preview (`flm watch`) starts its page's
/// *Settings* tool with the front matter's schema, over the document's search path.
///
/// # Errors
///
/// A message when the factory of the package `stdcore`, `stdmain`, `std` or
/// `math-passthrough` fails, or when the package of the program's math processor cannot be
/// found or read.
pub(crate) fn schema(
    of: SchemaOf,
    packages: &[PackageFactory],
    folders: Vec<ExtensionFolder>,
) -> Result<serde_json::Value, String> {
    let mut factories = compiled(packages)?;
    // What is installed in the folders, listed as the preparation of a document lists its
    // search path; the listing reports nothing, so a missing folder adds nothing here.
    let files: Arc<dyn Files> = Arc::new(DiskFiles);
    let finder = ExtensionFinder::new(BuildFiles::new(Arc::clone(&files)), ServiceFiles::new(Arc::clone(&files)), folders);
    let suggested = suggestions(factories.iter().map(|factory| factory.name().as_str()), &finder);
    // The packages of a document that specifies none, `std` and the program's math
    // processor's, as an empty document finds them with no folder on its search path: the
    // compiled ones are listed already, the bundled extensions are not.
    let mut request = Request::new(Input::Text(String::new()));
    request.default_math = crate::default_math();
    let initial = document_packages(request, files, packages).map_err(|error| {
        error.problems.iter().map(|problem| problem.detail.as_str()).collect::<Vec<_>>().join("; ")
    })?;
    for factory in initial.factories {
        if !factories.iter().any(|known| known.name() == factory.name()) {
            factories.push(factory);
        }
    }
    Ok(schema_over(of, &factories, &suggested))
}

/// Returns the factories of every package compiled into the program: the library's
/// packages `stdcore` and `math-passthrough`, the host's packages `stdmain` and `std`, and
/// `packages`, the other compiled packages that the executable carries ([`carried`]).
/// `flm info` lists their names on the page `program/extensions`.
///
/// # Errors
///
/// A message when the factory of the package `stdcore`, `stdmain`, `std` or
/// `math-passthrough` fails.
pub(crate) fn compiled(packages: &[PackageFactory]) -> Result<Vec<PackageFactory>, String> {
    // The packages depend on no command: `stdmain`'s features declare the same settings
    // whatever the naming scheme and the sink a command would give it.
    let stdmain = stdpkg::stdmain_factory(StdParts::new(Arc::new(Naming::Folder))).map_err(|error| error.to_string())?;
    carried(stdmain, packages).map_err(|error| error.to_string())
}

/// Returns the schema of the kind of file `of` over the package factories `factories`,
/// suggesting `suggestions` for an entry of `use`.
fn schema_over(of: SchemaOf, factories: &[PackageFactory], suggestions: &[String]) -> serde_json::Value {
    let refs: Vec<&PackageFactory> = factories.iter().collect();
    match of {
        SchemaOf::Extension => extension_schema(&refs, suggestions),
        SchemaOf::FrontMatter => front_matter_schema(&refs, suggestions),
    }
}

/// Returns the schema that `args` ask for: without a document, [`schema`] over `packages`
/// and the search path of a document given as text; with one, the schema over the packages
/// that the document uses, suggesting what its search path holds ([`document_packages`]),
/// found with the request that `flm render` would make for it ([`BuildArgs::request`]).
///
/// # Errors
///
/// The exit status, once the problems are printed: 3 when the schema cannot be made, and
/// the status of [`cannot_start`] (2) when the document cannot be read or its packages
/// cannot be found.
fn schema_of_args(args: &SchemaArgs, packages: &[PackageFactory]) -> Result<serde_json::Value, u8> {
    let Some(document) = &args.document else {
        return schema(args.of, packages, environment_search_path(&[], &Input::Text(String::new()))).map_err(|detail| {
            reporter().error(&detail);
            PROGRAM_FAILURE
        });
    };
    let build = BuildArgs {
        input: Some(document.clone()),
        content: None,
        format: None,
        profile: None,
        uses: Vec::new(),
        initial_use: None,
        math: None,
        template: None,
        options: Vec::new(),
        layout: None,
        blocks: false,
        extensions_dirs: args.extensions_dirs.clone(),
        verbose: false,
    };
    let request = build.request().map_err(|message| {
        reporter().error(&message);
        CANNOT_START
    })?;
    let found = document_packages(request, Arc::new(DiskFiles), packages).map_err(|error| cannot_start(&error))?;
    Ok(schema_over(args.of, &found.factories, &found.suggestions))
}

/// Runs `flm schema`: prints the schema on standard output (indented JSON, ending with a
/// newline), or writes it to the file of `-o` (through [`write_file`], which keeps no
/// record of the file). Returns the exit status: 0 when done, 1 when the file cannot be
/// written, 2 when the document given cannot be read or its packages cannot be found, 3
/// when the schema cannot be made or standard output cannot be written.
pub(crate) fn run(args: &SchemaArgs, packages: &[PackageFactory]) -> u8 {
    let text = match schema_of_args(args, packages) {
        Ok(schema) => format!("{schema:#}\n"),
        Err(status) => return status,
    };
    match args.output.as_deref() {
        None | Some("-") => {
            let mut stdout = std::io::stdout().lock();
            if let Err(error) = stdout.write_all(text.as_bytes()).and_then(|()| stdout.flush()) {
                reporter().error(&format!("standard output cannot be written: {error}"));
                return PROGRAM_FAILURE;
            }
        }
        Some(path) => {
            if let Err(error) = write_file(std::path::Path::new(path), text.as_bytes()) {
                reporter().error(&error.detail);
                return DOCUMENT_ERRORS;
            }
        }
    }
    DONE
}
