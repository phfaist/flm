//! The FLM host library: it turns a document written in files into output files, by
//! running the library [`flm_core`] with everything the library leaves to its caller.
//!
//! FLM is a LaTeX-like markup language. Its library, [`flm_core`], turns the text of a
//! document into outputs (an HTML page, a plain text, a LaTeX document) and reports the
//! problems it finds; it reads no file and writes none. In `flm_core`, a *host* is any
//! program that embeds the library; this crate, `flm-main`, is the host library shipped with
//! the project: it finds and reads a document, reads its settings, chooses its packages and
//! workflow, prepares the registry and the plan, runs the library and writes the outputs.
//! It does not depend on a process: it parses no command-line argument, prints nothing and
//! sets no exit status. The `flm` program (the crate `flm-cli`) turns a command line into a
//! [`session::Request`], calls this crate and prints what comes back. The guide chapter
//! "The flm command-line tool", in the documentation of `flm_core`, describes the program
//! for its users.
//!
//! # Using the host: prepare, then build
//!
//! A program uses the host in two calls. [`session::prepare`] does the work that depends on
//! the request and on the document's front matter but not on its content: it reads the front
//! matter ([`seed`]) and the document's metadata there, finds the packages the document uses
//! ([`packages`]), combines the settings ([`settings`]), and builds the
//! [registry](flm_core::core::registry) (the record of everything the language and the
//! pipeline consult) and the [plan](flm_core::core::plan::Plan) (the list of stages that
//! produces the requested output). [`session::build`] then reads the file again, parses its
//! content (positions are counted from the top of the file, front matter included), makes
//! the document and executes one [run](flm_core::core::run::Run) of the plan.
//!
//! ```
//! use std::path::PathBuf;
//! use std::sync::Arc;
//!
//! use flm_main::files::MemoryFiles;
//! use flm_main::output::{MemorySink, Naming, Sink};
//! use flm_main::seed::MathChoice;
//! use flm_main::session::{Input, Request, build, prepare};
//!
//! let files = MemoryFiles::new().with("paper.flm", "---\ntitle: Notes\n---\nHello \\emph{world}.\n");
//! let sink = Arc::new(MemorySink::new());
//! let mut request = Request::new(Input::File(PathBuf::from("paper.flm")));
//! // The host's math processor: the library's package `math-passthrough`.
//! request.default_math = MathChoice::processor("passthrough");
//! request.profile = Some(String::from("text"));
//! request.naming = Naming::File { name: String::from("paper.txt") };
//! request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
//!
//! let prepared = prepare(request, Arc::new(files), &[]).map_err(|error| format!("{error:?}"))?;
//! let built = build(&prepared).map_err(|error| format!("{error:?}"))?;
//! assert!(!built.has_errors());
//! assert_eq!(sink.files()["paper.txt"].bytes, b"Notes\n\nHello world.\n");
//! # Ok::<(), String>(())
//! ```
//!
//! When the request gives a sink ([`output::Sink`], the receiver of the output files), the
//! run's last stage, `flm.write.files` of the feature `write` ([`stdpkg::write`]), gives
//! the files of every output their paths ([`output::Naming`]) and hands them to the sink in
//! one delivery. A
//! document with errors stops the run before that stage unless the request sets
//! [`continue_with_errors`](session::Request::continue_with_errors), which the host passes
//! to the library's
//! [`RunOptions::continue_with_errors`](flm_core::core::run::RunOptions::continue_with_errors)
//! (the library stops a run that recorded an error before its first stage that writes
//! outside the run). [`session::build`] returns
//! every diagnostic, the list of the files it read, and the run, whose tables hold each
//! output's [artifact set](flm_core::core::artifact::ArtifactSet). The named files exist
//! only in the sink, so a caller that wants them without a disk gives an
//! [`output::MemorySink`], as above.
//!
//! What *prepare* made serves any number of builds; [`session::Prepared::is_current`] says
//! whether it is still valid for the files as they are now, and the
//! [`session`] module documentation says when a change needs a new *prepare*.
//!
//! # The modules
//!
//! | Module | What it does |
//! |---|---|
//! | [`session`] | the two operations, *prepare* and *build*, and the request that drives them |
//! | [`seed`] | splits a file into its front matter and its content, and reads the front matter: a block of YAML at the top of a file, holding the author's metadata and, under the key `flm`, the document's settings |
//! | [`settings`] | combines a document's settings from ranked sources (the program's defaults, the packages of the extensions the document uses, the front matter, the workflow, the command line), each value keeping a record of where it was written |
//! | [`packages`] | finds the packages a document uses: the compiled ones, and the packages of extensions, through the loader of the library `flm_ext`; the schema of an extension's manifest |
//! | [`extensions`] | finds an extension by its locator in the folders of the search path, in either layout (`NAME.yaml`, `NAME/flmextension.yaml`), and reads its files |
//! | `bundled` | finds an extension shipped with FLM among those compiled into the program, last on the search path, and reads its files (cargo feature `bundled-extensions`) |
//! | [`inclusion`] | finds and reads, for the library, the file an `\input` command refers to, with its front matter |
//! | [`files`] | reads files for every other module, and records the list of the files one build read |
//! | [`output`] | turns an output's artifacts into named files and hands them to a sink, which writes them to the disk or keeps them in memory |
//! | [`report`] | lays out a diagnostic as text for a person, or as data |
//! | [`stdpkg`] | the host's own packages: `stdmain`, with the features `compose` (the library's feature that assembles an output through a compose template), `compose-standard` (the template engine `minijinja` and `stdmain`'s compose templates), `write` (hands the output's files to the sink), `preview` (the script and stylesheet of the live preview's page), `graphics-files` (cargo feature `graphics`) and `citations-standard` (cargo feature `citations`); `std`, with no features of its own, which uses `stdcore` and `stdmain` and is the default initial list of every document; and the program's default settings |
//! | `graphics` | image files: finds and reads the file of an image, reads its size, converts it to another format (cargo feature `graphics`) |
//! | `citations` | the details of cited works: fetched over the network or read from bibliography files, kept in a cache file, formatted into citation entries (cargo feature `citations`) |
//! | [`watch`] | the live preview: builds the document again when a file it read changes, compares each build with the one before, and serves the output to a browser on the same machine (the loop, the file watcher and the server need the cargo feature `watch`) |
//!
//! # Files and the disk
//!
//! Only [`files`] reads the files of a document and only [`output`] writes its outputs
//! ([`files::DiskFiles`], [`output::DiskSink`], and [`output::write_file`] for a file that is
//! not the output of a document), both under the cargo feature `fs`, which is on by default.
//! Without it the crate reads and writes no document file and compiles for WebAssembly: a
//! host running in a browser reads from [`files::MemoryFiles`] and writes to
//! [`output::MemorySink`]. One exception: under the cargo feature `citations`, the citations
//! provider keeps its cache file in the folder the program gives (the field `cache_folder`
//! of `citations::CitationsHost`) and reads and writes it directly.
//!
//! # The front matter and the settings
//!
//! A document's settings are written as data, one map per feature from a settings key to its
//! value ([`flm_core::core::settings`]); a key is always one that the feature declares. The
//! keys a front matter may hold under `flm` are documented in [`seed`]; the keys of an
//! extension's manifest and where one is looked for, in [`extensions`]; the settings sources and their ranks,
//! in [`settings`]; the defaults the program sets where they differ from the library's, in
//! [`stdpkg`]. The JSON Schema of a front matter ([`seed::front_matter_schema`]) and of a
//! extension's manifest ([`packages::extension_schema`]) are assembled from the same parts, each
//! feature's part from the feature's own settings declaration. The command-line guide
//! (section "Configuration files and document front matter") describes the same files for
//! a user of the `flm` program.
//!
//! # What the host adds to a run, as features
//!
//! Everything the host adds to a run is a [feature](flm_core::core::registry::FlmFeature) of
//! its package `stdmain`, registered like the features of any other package and configured
//! with settings in the same way. The package has six features:
//!
//! - the feature `compose`, the library's ([`flm_core::stdcore::features::compose`]),
//!   which `stdmain` registers, assembles each output's
//!   [output regions](flm_core::core::region::Regions) through a compose template (a text
//!   with named places for the regions); for an output it serves, [`session::prepare`] adds
//!   to the plan a preference for its stage over the backend's own
//!   final stage (the stage that assembles the output in the backend's own layout; see
//!   [`flm_core::core::region`]),
//!   and selects, as the [pieces](flm_core::core::piece::Piece) the output's backend builds
//!   (document-level values placed beside the content, such as the title or the list of
//!   footnotes), the pieces the chosen template accepts;
//! - the feature `compose-standard` ([`stdpkg::compose_standard`]) contributes the
//!   template engine `minijinja` and `stdmain`'s own compose templates to the feature
//!   `compose`;
//! - the feature `write` ([`stdpkg::write`]) hands the output's files to the sink, and
//!   contributes the naming scheme ([`output::Naming`]) to the library's
//!   [service slot](flm_core::core::services) `flm.artifacts.namer` (a declared name to
//!   which features contribute shared values), from which a stage of any feature can learn where an artifact
//!   will be written;
//! - under the cargo feature `graphics`, the feature `graphics-files` contributes the
//!   services of the module `graphics` that find, inspect and convert image files;
//! - the feature `preview` ([`stdpkg::preview`]) declares the script and the stylesheet of
//!   the live preview's page;
//! - under the cargo feature `citations`, the feature `citations-standard` contributes the
//!   host's provider of citation entries to the library's feature `citations`, and the
//!   outputs `bibtex` and `csl-json`.
//!
//! A [service](flm_core::core::services) of the registry is made once, by *prepare*, and
//! serves every build. It reads files through a [`files::ServiceFiles`], to which each build
//! attaches its own file reader ([`files::BuildFiles`]), so the files a service reads appear
//! on that build's list of files read. The output folder given by the request
//! ([`session::Request::output_folder`], an [`output::OutputFolder`]) is what the address is
//! computed from when an output refers to an image file instead of holding the image (the
//! graphics strategy `reference`). The package `stdmain` uses the library's standard
//! package `stdcore`; the package `std` has no features of its own and uses `stdcore` and
//! `stdmain`. Every document uses the packages of its initial list before those it lists
//! under `use`: the request's ([`session::Request::initial_use`]), else the front matter's
//! `initial_use`, else the host's default ([`session::Request::default_initial_use`]), which
//! is `std` unless the host sets another ([`packages`]). Neither `std` nor `stdmain` holds a
//! feature that processes mathematics, without which each formula of a document is an
//! Error. Such a feature belongs to a *math processor*, a package chosen apart from the
//! initial list ([`seed::MathChoice`]): by the request ([`session::Request::math`]), else by
//! the front matter's `math`, else by the host's default
//! ([`session::Request::default_math`]), which is no processor unless the host sets one, for
//! example `passthrough`, the library's package `math-passthrough`, which the catalog
//! carries. The processor's package is used after the packages of the initial list and
//! before those of `use`. The packages that the request adds ([`session::Request::uses`])
//! are used after those of `use`, as if the front matter listed them last there.
//!
//! # Documents of several files
//!
//! A document brings in other files with `\input`, which the library parses where the
//! command is written. The library reads no file: for each `\input` it asks an
//! [input resolver](flm_core::lang::InputResolver). [`session::build`] gives the library's
//! assembling operation ([`flm_core::model::assemble`]) the host's input resolver,
//! [`inclusion::Inclusion`], made over that build's file reader. It finds each file
//! relative to the file holding the command, reads it and its front matter through
//! [`files`] and [`seed`], and so every included file appears on the build's list of files
//! read.
//!
//! # Watching a document
//!
//! The module [`watch`] holds the live preview. Its loop, `watch::Watch` (cargo feature
//! `watch`), builds the document, then builds it again whenever a file the build read
//! changes or its caller asks for a build, and hands its caller each build as a value, mainly
//! an [`watch::update::Update`], which says what an open page must do to show the new
//! build. It keeps what *prepare* made as long as [`session::Prepared::is_current`] says it
//! is valid. The comparison of two builds, [`watch::update`], is compiled without the cargo
//! feature. `watch::server` serves the output the loop holds to a browser on the same
//! machine, keeps every open page up to date through a stream of messages, and opens a
//! document's file in the editor at a given position (`watch::editor`); the script of the preview's
//! page is declared by the feature [`stdpkg::preview`]. The command-line guide (section
//! "Watching a document: the live preview") describes the preview for a user of the `flm`
//! program.

#[cfg(feature = "citations")]
pub mod citations;
pub mod files;
#[cfg(feature = "graphics")]
pub mod graphics;
#[cfg(feature = "bundled-extensions")]
pub mod bundled;
pub mod extensions;
pub mod host_files;
#[cfg(feature = "fs")]
pub mod host_programs;
pub mod inclusion;
pub mod output;
pub mod packages;
pub mod report;
pub mod seed;
pub mod session;
pub mod settings;
pub mod stdpkg;
pub mod watch;
