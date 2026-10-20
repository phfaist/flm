//! The live preview: a document built again whenever a file it read changes, each build
//! compared with the one before, and the result served to a browser.
//!
//! The *live preview* lets an author see a document beside the editor while writing it:
//! the `flm` program (`flm watch INPUT`) builds the document to an HTML page, serves the
//! page to a browser on this machine, and builds again whenever a file the document read
//! changes; the page in the browser is then brought up to date. This module holds the
//! parts of the preview that do not depend on the command line, so that another host can
//! offer a preview of its own. The guide page *The flm command-line tool*, section
//! *Watching a document: the live preview*, describes the preview as a user of the `flm`
//! program sees it.
//!
//! The parts:
//!
//! | Item | What it does | Compiled |
//! |---|---|---|
//! | [`Watch`] (the *watch loop*), with [`Event`], [`Built`], [`Handle`] | builds the document through [`crate::session`], waits for a change or a call to [`Handle::build_again`], builds again, and hands each event to its caller | cargo feature `watch` |
//! | [`build_settings`] | the settings the watch loop adds to every build | always |
//! | [`update`] | compares the outputs of two builds, and gives what an open page must change, as a value and as the message the page script (below) reads | always; it reads and writes nothing |
//! | [`watcher`] | reports which files of a set changed on the disk, after a quiet period | cargo feature `watch` |
//! | [`server`] | serves the last output from memory to a browser on this machine, with a stream of messages to every open page | cargo feature `watch` |
//! | [`editor`] | finds the editors on this machine and opens a file in one of them at a line and a column; turns a place of the page into that line and column | cargo feature `watch` |
//! | [`settings`] and [`front_matter`] | read the settings of the document's features for the page's *Settings* tool, and write them back into the document's [front matter](crate::seed) in place | cargo feature `watch` |
//!
//! The page script and its stylesheet, which keep the page in the browser up to date, are
//! declared by the feature [`preview`](crate::stdpkg::preview) of the standard package;
//! their source files are in the folder `watch/page/` of this crate.
//!
//! **One build at a time.** The watch loop runs one build at a time. Changes reported
//! while a build runs are collected, and one more build follows it; a call to
//! `Handle::build_again` made while a build runs counts as one such change. Each build:
//!
//! - prepares again (reads the front matter and the packages, builds the registry and the
//!   plan: [`crate::session::prepare`]) when the last preparation is out of date
//!   ([`Prepared::is_current`](crate::session::Prepared::is_current)); otherwise builds
//!   over the last preparation, so that its registry and plan are reused from one build to
//!   the next;
//! - continues through the document's errors, so that the output shows the document as it
//!   is now, with the HTML backend's error marks (what the page shows in place of
//!   something that could not be rendered: an element the backend cannot write, a command
//!   or environment that no feature handles, the mark a rule leaves where it dropped
//!   content; their stylesheet is
//!   [`STYLE_ERRORS`](flm_core::stdcore::backends::html::conventions::STYLE_ERRORS));
//! - writes each element's place in its source into the page (the HTML backend's render
//!   option `source_spans`), so that the page script can lead from an element to its
//!   source;
//! - requires three style and script definitions
//!   ([`StyleDef`](flm_core::core::registry::StyleDef)) for the page as a whole, through
//!   the HTML backend's render option `required_definitions`: the preview's page script
//!   and its stylesheet, and the HTML backend's stylesheet of the error marks. The page
//!   therefore carries the style of the error marks whether the document has an error or
//!   not, and an error that appears or disappears changes only the output regions it is
//!   in (an output region is one named part of an output's entry artifact, such as the
//!   document's content: [`Regions`](flm_core::core::region::Regions));
//!   [`build_settings`] lists the settings;
//! - produces every output of the workflow, and shows the *leading output*, the first of
//!   them ([`Prepared::outputs`](crate::session::Prepared::outputs)): its entry file is
//!   the page, and the files of the other outputs are served by path beside it;
//! - writes no file: the outputs' named files are handed to the watch loop in memory.
//!
//! **Which files are watched.** At every moment the watch loop watches the *watched set*:
//! the primary file (the session request's input file,
//! [`Request::input`](crate::session::Request::input), when the input is a file) and
//! every path the last build attempt read, found or not ([`crate::files`]). An attempt
//! that stopped early counts too: its error lists what it read up to that point
//! ([`PrepareError::files_read`](crate::session::PrepareError::files_read)). Before the
//! first build the watched set is the primary file alone, and it is watched before that
//! build starts, so that a save made while the first build runs causes one more build.
//!
//! **What the caller receives.** The watch loop prints nothing, handles no signal of the
//! process and does not depend on a terminal. It hands each [`Event`] to the function its
//! caller gave: a build starting, with the files that caused it; a build done, with its
//! [`update::Update`], its diagnostics and the output now current; a problem of the
//! watcher. It ends when its caller asks through a [`Handle`], which can also ask for a
//! build now. When a build gives no output, the output before it stays current, and the
//! next build is compared with it.
//!
//! ```no_run
//! # #[cfg(feature = "watch")] {
//! use std::path::PathBuf;
//! use std::sync::Arc;
//!
//! use flm_main::files::DiskFiles;
//! use flm_main::session::{Input, Request};
//! use flm_main::watch::{Event, Watch};
//!
//! let request = Request::new(Input::File(PathBuf::from("paper.flm")));
//! let watch = Watch::new(request, Arc::new(DiskFiles), Vec::new()).watching_files();
//! let handle = watch.handle();
//! watch.run(|event| {
//!     if let Event::Built(built) = event {
//!         println!("build {}: {} problems", built.update.build, built.update.problems.len());
//!         if built.update.build == 3 {
//!             handle.stop();
//!         }
//!     }
//! })?;
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[cfg(feature = "watch")]
pub mod editor;
#[cfg(feature = "watch")]
pub mod front_matter;
#[cfg(feature = "watch")]
pub mod server;
#[cfg(feature = "watch")]
pub mod settings;
pub mod update;
#[cfg(feature = "watch")]
pub mod watcher;

#[cfg(feature = "watch")]
pub use running::{Built, Event, Handle, Preparation, Watch};

use crate::settings::CommandLineSetting;

/// Returns the two command-line settings that the watch loop ([`Watch`]) adds to every
/// build, after the settings of its session request ([`Request`](crate::session::Request)),
/// in this order:
///
/// 1. `html.source_spans=true`: the HTML backend writes each element's place in its source
///    into the page
///    ([`source_spans`](flm_core::stdcore::backends::html::HtmlRenderOptions::source_spans));
/// 2. `html.required_definitions=[flm.preview.script, flm.preview.style, flm.html.style.errors]`:
///    the page as a whole requires the preview's page script and its stylesheet (declared
///    by the feature [`preview`](crate::stdpkg::preview) of the standard package) and the
///    HTML backend's stylesheet of the error marks
///    ([`STYLE_ERRORS`](flm_core::stdcore::backends::html::conventions::STYLE_ERRORS))
///    ([`required_definitions`](flm_core::stdcore::backends::html::HtmlRenderOptions::required_definitions)).
///
/// Without the second setting, the stylesheet of the error marks
/// ([`STYLE_ERRORS`](flm_core::stdcore::backends::html::conventions::STYLE_ERRORS): what
/// the page shows in place of something that could not be rendered) is required only by
/// the elements that carry an error mark, so a page that `flm render` writes holds it only
/// while its document has such a mark. The preview's page requires it always: the page's
/// head then stays the same when the document's first error mark appears or its last one
/// disappears, and the page can be brought up to date in place ([`update`]) instead of
/// being loaded anew.
///
/// Each setting's label, which a diagnostic about that setting quotes, is `the preview's`
/// followed by the setting. This function is available whether or not the cargo feature
/// `watch` is enabled, so that a host can build a document with the same settings, and
/// compare two such builds ([`update`]), without the watch loop.
pub fn build_settings() -> Vec<CommandLineSetting> {
    use crate::stdpkg::preview::{SCRIPT, STYLE};
    use flm_core::stdcore::backends::html::conventions::STYLE_ERRORS;
    [String::from("html.source_spans=true"), format!("html.required_definitions=[{SCRIPT}, {STYLE}, {STYLE_ERRORS}]")]
        .into_iter()
        .map(|setting| {
            let label = format!("the preview's `{setting}`");
            CommandLineSetting::new(setting, label)
        })
        .collect()
}

#[cfg(feature = "watch")]
mod running {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{Arc, Mutex};

    use flm_core::core::registry::PackageFactory;
    use flm_core::lang::FlmDiagnostic;

    use super::editor::Places;
    use super::update::{BuildOutput, Update};
    use super::watcher::{Seen, WatchCondition, WatchError, Watcher};
    use crate::files::{BuildFiles, FileRead, Files};
    use crate::output::{NamedFile, OutputError, Sink};
    use crate::report::{ContentOffsets, Reported};
    use crate::session::{Input, PrepareError, Prepared, Request, build, prepare};

    /// A message to the watch loop from its handle or from the watcher.
    enum Signal {
        /// These files of the watched set changed.
        Changed(Vec<PathBuf>),
        /// Build again now.
        Again,
        /// The watcher met a problem.
        Watcher(WatchError),
        /// End the loop.
        Stop,
    }

    /// An event of the watch loop, handed to the function given to [`Watch::run`]: a build
    /// starting, a build done, or a problem of the watcher. See the [module
    /// documentation](super), "What the caller receives".
    #[derive(Debug)]
    pub enum Event {
        /// A build starts. `changed` lists the files whose change caused it, each once,
        /// with each path spelled as in [`FileRead::path`](crate::files::FileRead::path);
        /// it is empty for the first build and for a build requested through
        /// [`Handle::build_again`] with nothing changed.
        Building {
            /// The files that changed.
            changed: Vec<PathBuf>,
        },
        /// A build is done ([`Built`]).
        Built(Box<Built>),
        /// The watcher has a problem: a folder could not be watched, or the system reported
        /// a problem while watching ([`WatchError`]). The watch loop continues.
        Watcher(WatchError),
    }

    /// A finished build, as the watch loop hands it to its caller in [`Event::Built`].
    #[derive(Debug)]
    pub struct Built {
        /// The files whose change caused the build, as [`Event::Building`] listed them.
        pub changed: Vec<PathBuf>,
        /// The packages, the workflow and the outputs that the preparation chose
        /// ([`crate::session::prepare`]), when this build prepared again and the
        /// preparation succeeded. A build prepares again the first time, and whenever the
        /// last preparation is out of date. `None` when the build reused an earlier
        /// preparation, or when the preparation failed ([`Built::outcome`] holds the error).
        pub prepared: Option<Preparation>,
        /// What changed from the last output to this build's output, and the build's
        /// problems ([`Update`]).
        pub update: Update,
        /// The current output: this build's output, or, when this build gave none, the
        /// last output a build gave; `None` until a build gives an output.
        pub output: Option<Arc<BuildOutput>>,
        /// The sources of the build that gave the current output, by name
        /// ([`Places`](super::editor::Places)), which turn a place written in the page (a
        /// source's name and a byte offset) into a line and a column; `None` until a build
        /// gives an output.
        ///
        /// The places and the problems ([`Built::update`]) usually come from the same
        /// build. A build that stops before the document is processed reports problems
        /// with no byte offset, which need no places. A build that runs hands its output
        /// over unless a stage of the output fails for a reason other than the document's
        /// errors (the backend cannot read its tables or its options, or no names can be
        /// derived for the output files); only then are the places those of an earlier
        /// build while the problems are this build's.
        pub places: Option<Arc<Places>>,
        /// The build's diagnostics, in the order reported, or the error that stopped the
        /// build ([`PrepareError`]).
        pub outcome: Result<Vec<FlmDiagnostic>, PrepareError>,
        /// Every path the build attempt read, found or not, in the order read, as
        /// [`crate::session`] lists them: the preparation's reads first, then the build's.
        pub files_read: Vec<FileRead>,
        /// The content of the input and of each file the build included, with the byte
        /// offset at which it starts in the file, as
        /// [`crate::session::Built::content_offsets`] gives them, for the records of the
        /// build's diagnostics
        /// ([`Record::of_diagnostic`](crate::report::Record::of_diagnostic)); empty when the
        /// build stopped before the document was processed, whose diagnostics and problems
        /// have no byte offset.
        pub content_offsets: ContentOffsets,
    }

    /// The choices of a preparation ([`crate::session::prepare`]) that the following builds
    /// use, as a build that prepared again reports them ([`Built::prepared`]); each field
    /// but the last copies the corresponding method of [`Prepared`].
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Preparation {
        /// The names of the packages in use, each listed after the packages it uses
        /// ([`Prepared::packages`]).
        pub packages: Vec<String>,
        /// The name of the workflow that runs ([`Prepared::workflow`]).
        pub workflow: String,
        /// The names of the outputs the build produces, in the workflow's order; the first
        /// is the leading output, the one the preview shows ([`Prepared::outputs`]). Empty
        /// for a workflow that delivers no output.
        pub outputs: Vec<String>,
        /// The JSON Schema of a front matter for the packages of this preparation
        /// ([`Prepared::factories`]), suggesting the packages and extensions listed on the
        /// document's search path ([`Prepared::suggestions`]), computed once by
        /// [`front_matter_schema`](crate::seed::front_matter_schema): the schema from which
        /// the page's *Settings* tool builds its forms, so that the settings of every
        /// package the document uses, an extension's included, appear there.
        pub settings_schema: Arc<serde_json::Value>,
    }

    impl Preparation {
        /// The choices of `prepared`.
        fn of(prepared: &Prepared) -> Preparation {
            Preparation {
                packages: prepared.packages().to_vec(),
                workflow: prepared.workflow().to_string(),
                outputs: prepared.outputs().to_vec(),
                settings_schema: Arc::new(crate::seed::front_matter_schema(
                    &prepared.factories().iter().collect::<Vec<_>>(),
                    prepared.suggestions(),
                )),
            }
        }
    }

    /// A handle to a watch loop, through which its caller stops the loop or asks for a
    /// build ([`Watch::handle`]). It is cheap to clone and can be used from any thread.
    /// Once the watch loop has ended, its methods do nothing.
    #[derive(Debug, Clone)]
    pub struct Handle {
        signals: Sender<Signal>,
    }

    impl Handle {
        /// Ends the watch loop: once the build under way, if any, is done, the watch loop
        /// stops watching files and [`Watch::run`] returns.
        pub fn stop(&self) {
            let _ = self.signals.send(Signal::Stop);
        }

        /// Asks for a build now, as if a file the document read had changed: one build, or,
        /// while a build runs, one more build after it.
        pub fn build_again(&self) {
            let _ = self.signals.send(Signal::Again);
        }
    }

    /// The sink of the watch loop's builds: it keeps the files of the last output handed
    /// to it until the watch loop takes them after a build, so that a build that handed no
    /// files is told apart from one that handed the same files again.
    #[derive(Debug, Default)]
    struct Received {
        files: Mutex<Option<Vec<NamedFile>>>,
    }

    impl Received {
        fn take(&self) -> Option<Vec<NamedFile>> {
            self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
        }
    }

    impl Sink for Received {
        fn receive(&self, files: &[NamedFile]) -> Result<(), Vec<OutputError>> {
            *self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(files.to_vec());
            Ok(())
        }
    }

    /// The watch loop of the live preview: builds a document, then builds it again at
    /// each change of a file it read and at each call to [`Handle::build_again`], and
    /// hands each [`Event`] to its caller. See the [module documentation](super).
    ///
    /// Create it with [`Watch::new`], add [`Watch::watching_files`] to watch the disk,
    /// keep a [`Handle`] ([`Watch::handle`]) to stop it, and start it with
    /// [`Watch::run`].
    pub struct Watch {
        request: Request,
        files: Arc<dyn Files>,
        compiled: Vec<PackageFactory>,
        received: Arc<Received>,
        watching: bool,
        signals: Sender<Signal>,
        incoming: Receiver<Signal>,
    }

    impl std::fmt::Debug for Watch {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Watch").field("input", &self.request.input).field("watching", &self.watching).finish_non_exhaustive()
        }
    }

    impl Watch {
        /// Creates the watch loop for the document of the session request `request`,
        /// reading its files through `files`; `compiled` lists the packages compiled into
        /// the program, as [`crate::session::prepare`] takes them.
        ///
        /// The watch loop changes three fields of `request`: it replaces the sink by one of
        /// its own (the output stays in memory), sets `continue_with_errors`, and appends the
        /// settings of [`build_settings`](super::build_settings) to `options`. Every other
        /// field is used as the caller set it. The watch loop watches no file until
        /// [`Watch::watching_files`] is called.
        pub fn new(mut request: Request, files: Arc<dyn Files>, compiled: Vec<PackageFactory>) -> Watch {
            let received = Arc::new(Received::default());
            request.sink = Some(Arc::clone(&received) as Arc<dyn Sink>);
            request.continue_with_errors = true;
            request.options.extend(super::build_settings());
            let (signals, incoming) = channel();
            Watch { request, files, compiled, received, watching: false, signals, incoming }
        }

        /// Returns the same watch loop, set to watch on the disk the files each build
        /// read and to build again when one changes ([`watcher`](super::watcher)).
        /// Without this call the watch loop builds once when it starts, and after that
        /// only when asked ([`Handle::build_again`]), which suits a test or a host whose
        /// files are not on a disk.
        pub fn watching_files(mut self) -> Watch {
            self.watching = true;
            self
        }

        /// Returns a handle that stops the watch loop or asks it for a build.
        pub fn handle(&self) -> Handle {
            Handle { signals: self.signals.clone() }
        }

        /// Runs the watch loop on the current thread until a [`Handle`] stops it, handing
        /// each event to `on_event`: builds the document once, then builds again at each
        /// change and each call to [`Handle::build_again`]. See the [module
        /// documentation](super).
        ///
        /// # Errors
        ///
        /// Returns a [`WatchError`] when the watch loop is set to watch files
        /// ([`Watch::watching_files`]) and the system's file watching cannot be started;
        /// nothing is built then.
        pub fn run(self, mut on_event: impl FnMut(Event)) -> Result<(), WatchError> {
            let mut watcher = if self.watching {
                let signals = self.signals.clone();
                Some(Watcher::new(move |seen| {
                    let _ = signals.send(match seen {
                        Seen::Changed(paths) => Signal::Changed(paths),
                        Seen::Failed(detail) => Signal::Watcher(WatchError { condition: WatchCondition::Watching, detail }),
                    });
                })?)
            } else {
                None
            };
            let mut state = State { prepared: None, last: None, places: None, count: 0, watched: self.watched_set(&[]) };
            let mut changed = Vec::new();
            watch(&mut watcher, &state.watched, &mut on_event);
            loop {
                on_event(Event::Building { changed: changed.clone() });
                let built = self.build(&mut state, changed);
                watch(&mut watcher, &state.watched, &mut on_event);
                on_event(Event::Built(Box::new(built)));
                match self.wait(&mut on_event) {
                    Some(next) => changed = next,
                    None => return Ok(()),
                }
            }
        }

        /// Waits for a change or a call to [`Handle::build_again`], and collects every
        /// signal already waiting with it: the files that changed, each once, or `None`
        /// when the watch loop is to end.
        fn wait(&self, on_event: &mut impl FnMut(Event)) -> Option<Vec<PathBuf>> {
            let mut changed: Vec<PathBuf> = Vec::new();
            let mut build = false;
            // The loop holds a sender itself, so the channel never closes while it waits.
            let mut next = self.incoming.recv().ok();
            while let Some(signal) = next {
                match signal {
                    Signal::Changed(paths) => {
                        for path in paths {
                            if !changed.contains(&path) {
                                changed.push(path);
                            }
                        }
                        build = true;
                    }
                    Signal::Again => build = true,
                    Signal::Watcher(error) => on_event(Event::Watcher(error)),
                    Signal::Stop => return None,
                }
                next = match self.incoming.try_recv() {
                    Ok(signal) => Some(signal),
                    Err(_) if build => None,
                    Err(_) => self.incoming.recv().ok(),
                };
            }
            build.then_some(changed)
        }

        /// One build, over what `state` holds, caused by the change of `changed`.
        fn build(&self, state: &mut State, changed: Vec<PathBuf>) -> Built {
            state.count += 1;
            let mut prepared_again = None;
            let prepared = match state.prepared.take().filter(Prepared::is_current) {
                Some(prepared) => prepared,
                None => match prepare(self.request.clone(), Arc::clone(&self.files), &self.compiled) {
                    Ok(prepared) => {
                        prepared_again = Some(Preparation::of(&prepared));
                        prepared
                    }
                    Err(error) => return self.failed(state, changed, None, error),
                },
            };
            // What an earlier build handed over and this one did not must not count as
            // this build's.
            self.received.take();
            let result = build(&prepared);
            let leading = prepared.outputs().first().cloned().unwrap_or_default();
            state.prepared = Some(prepared);
            let built = match result {
                Ok(built) => built,
                Err(error) => return self.failed(state, changed, prepared_again, error),
            };
            let output = self.received.take().map(|files| Arc::new(BuildOutput::of_build(&built.run, &leading, files)));
            state.watched = self.watched_set(&built.files_read);
            let problems = built.diagnostics.iter().map(Reported::of_diagnostic).collect();
            let update = Update::new(state.count, state.last.as_deref(), output.as_deref(), problems);
            if output.is_some() {
                state.last = output;
                state.places = Some(Arc::new(Places::of_run(&built.run)));
            }
            Built {
                changed,
                prepared: prepared_again,
                update,
                output: state.last.clone(),
                places: state.places.clone(),
                outcome: Ok(built.diagnostics),
                files_read: built.files_read,
                content_offsets: built.content_offsets,
            }
        }

        /// The build numbered `state.count` that gave no output, stopped by `error`, which
        /// lists what the attempt read.
        fn failed(
            &self,
            state: &mut State,
            changed: Vec<PathBuf>,
            prepared: Option<Preparation>,
            error: PrepareError,
        ) -> Built {
            state.watched = self.watched_set(&error.files_read);
            // The diagnostics found before the step that stopped, then its problems.
            let problems = error
                .diagnostics
                .iter()
                .map(Reported::of_diagnostic)
                .chain(error.problems.iter().map(Reported::of_problem))
                .collect();
            let update = Update::new(state.count, state.last.as_deref(), None, problems);
            let files_read = error.files_read.clone();
            Built {
                changed,
                prepared,
                update,
                output: state.last.clone(),
                places: state.places.clone(),
                outcome: Err(error),
                files_read,
                content_offsets: ContentOffsets::new(),
            }
        }

        /// The watched set after a build attempt that read `read`, by the rule of the
        /// [module documentation](super), "Which files are watched": the primary file, then
        /// each path read, each once; before the first build `read` is empty.
        fn watched_set(&self, read: &[FileRead]) -> Vec<PathBuf> {
            let mut watched: Vec<PathBuf> = Vec::new();
            if let Input::File(path) = &self.request.input {
                // Named as the build's reading of files names it, so that it is one path.
                watched.push(BuildFiles::resolve(Path::new(""), path));
            }
            for read in read {
                if !watched.contains(&read.path) {
                    watched.push(read.path.clone());
                }
            }
            watched
        }
    }

    /// Makes `paths` the set `watcher` watches, when the watch loop watches files, and hands
    /// a problem for each folder that could not be watched to `on_event`.
    fn watch(watcher: &mut Option<Watcher>, paths: &[PathBuf], on_event: &mut impl FnMut(Event)) {
        if let Some(Err(errors)) = watcher.as_mut().map(|watcher| watcher.watch(paths)) {
            for error in errors {
                on_event(Event::Watcher(error));
            }
        }
    }

    /// What the watch loop keeps from one build to the next.
    struct State {
        /// The last preparation, until it is out of date.
        prepared: Option<Prepared>,
        /// The last output a build gave.
        last: Option<Arc<BuildOutput>>,
        /// The places of the build that gave [`State::last`].
        places: Option<Arc<Places>>,
        /// The number of builds so far.
        count: u64,
        /// The watched set: the primary file and the paths the last build attempt read
        /// (`Watch::watched_set`).
        watched: Vec<PathBuf>,
    }
}
