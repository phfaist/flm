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
//! | [`Watch`] (the *watch loop*), with [`Event`], [`Built`], [`Preparation`], [`FailedPreparation`], [`Handle`] | builds the document through [`crate::session`], waits for a change or a call to [`Handle::build_again`], builds again, and hands each event to its caller | cargo feature `watch` |
//! | [`build_settings`] | the settings the watch loop adds to every build | always |
//! | [`update`] | compares the outputs of two builds, and gives what an open page must change, as a value and as the message the page script (below) reads | always; it reads and writes nothing |
//! | [`snapshot`] | gives what the preview's pages need of the last processed document: the problems of the last build, the sources by name, the headings, the labels and the constructs written in each file | always; it reads and writes nothing |
//! | [`watcher`] | reports which files of a set changed on the disk, after a short delay | cargo feature `watch` |
//! | [`server`] | serves the last output and the tools page from memory to a browser on this machine, with a stream of messages to every open page | cargo feature `watch` |
//! | [`editor`] | finds the editors on this machine, and the editor of the author's own command, and opens a file in one of them at a line and a column | cargo feature `watch` |
//! | [`viewer`] | reads and writes the viewer settings, the author's own file `.flm/watch.json` in the home folder | cargo feature `watch` |
//! | [`export`] | builds the document as `flm render -o` builds it, between two builds of the preview, and writes it beside the document or gives its files | cargo feature `watch` |
//! | [`services`] | the trait through which the program running the preview gives the server what only it knows, such as the help pages | cargo feature `watch` |
//! | [`settings`] and [`front_matter`] | give the settings state of the document for the server's requests of settings, and write the changes of one request into the document's [front matter](crate::seed) in place | cargo feature `watch` |
//!
//! The page script and its stylesheet, which keep the page in the browser up to date, are
//! declared by the feature [`preview`](crate::stdpkg::preview) of the package `std`. The
//! tools page, which shows the page in a frame beside the preview's tools and sends most of
//! the requests to the server, is served by the [`server`]. The page script, its stylesheet
//! and the tools page are the *watch client*, the npm package `flm-watchclient` of FLM's
//! repository (folder `npm/flm-watchclient/`), whose build writes them into the folders
//! `watch/client/` and `watch/client/tools/` of this crate. The package's file `PROTOCOL.md`
//! specifies the messages and the requests that both parts exchange with the server, and
//! what the page script expects of the page.
//!
//! **One build at a time.** The watch loop runs one build at a time. A change reported
//! while a build runs makes that build useless: it is cancelled, the changes reported
//! meanwhile are collected, and one more build follows it. A call to
//! [`Handle::build_again`] or [`Handle::set_profile`] made while a build runs counts as
//! one such change, and a call to [`Handle::stop`] cancels the build too before the watch
//! loop ends. A cancelled build hands nothing to the caller's function, leaves the output
//! current as it is, and adds the paths it read to the watched set; the build that
//! follows it names the files whose change caused either. Each build:
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
//!   [`STYLE_ERRORS`](flm_core::stdlib::backends::html::conventions::STYLE_ERRORS));
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
//!   the page, and the files of the other outputs are served by path beside it; a
//!   profile that declares no output (`check`) gives the *output without files*, which has
//!   no entry file and replaces the last output as any output does;
//! - writes no file: the outputs' named files are handed to the watch loop in memory.
//!
//! **The progress of a build.** Each build reports on a progress sheet of its own
//! ([`flm_core::progress`]), whose log lines go to the log sink of the session request
//! ([`Request::log_sink`](crate::session::Request::log_sink)), and whose rows carry the time
//! elapsed since the build started. The root row is of the kind `flm.watch.build`; under
//! it, the build declares a row of the kind [`BUILD_KIND`](crate::session::BUILD_KIND)
//! before it prepares, gives the root's scope to [`crate::session::prepare`] when it
//! prepares again, and gives the scope of that row to [`crate::session::build`]. While a
//! build runs, [`Handle::progress`] gives the sheet's reading half and the table of
//! expected durations to estimate the build with ([`BuildProgress`]): the durations
//! measured on the last build handed over, laid over the table shipped with the host
//! library ([`expected_durations`](crate::report::expected_durations)), and that table
//! alone until a build is handed over.
//!
//! **The exports.** An export ([`export`]) is a build of the document as `flm render -o`
//! builds it, with the session request as its caller gave it, which the watch loop carries
//! out between two builds of the preview when asked through a [`Handle`]
//! ([`Handle::export`]). It hands nothing to the caller's function, and changes neither the
//! watched set nor the output current. A change does not cancel an export, and a request
//! for an export does not cancel the build under way: the export waits for it.
//!
//! **The profile.** The builds run the profile of the session request, resolved as
//! [`crate::session::prepare`] resolves it: the request's
//! [`profile`](crate::session::Request::profile), else the front matter's
//! `default_profile`, else `html`. A call to [`Handle::set_profile`] sets the request's
//! profile for the builds that follow and clears its
//! [`format`](crate::session::Request::format); the next build prepares again. The profile
//! chosen this way belongs to the watch loop alone: nothing is written into the document.
//! The *default profile* is the one the builds run when no profile is set this way: the
//! profile of the request the watch loop was created with, else the front matter's
//! `default_profile`, else `html`, as the last preparation that succeeded found them
//! ([`Built::default_profile`]).
//! The profiles a document can run are those its preparation lists
//! ([`Prepared::profiles`](crate::session::Prepared::profiles)); a profile whose
//! preparation fails gives a build that stops there, and the last output stays current.
//!
//! **Which files are watched.** At every moment the watch loop watches the *watched set*:
//! the primary file (the session request's input file,
//! [`Request::input`](crate::session::Request::input), when the input is a file), every
//! path the last build handed over read, found or not ([`crate::files`]), and every path
//! that the cancelled builds since then read. A build that stopped by its error counts as
//! handed over: its error lists what it read up to that point
//! ([`PrepareError::files_read`](crate::session::PrepareError::files_read)). A cancelled
//! build adds the paths it read to the watched set and removes none. Before the
//! first build the watched set is the primary file alone, and it is watched before that
//! build starts, so that a save made while the first build runs causes one more build.
//!
//! **What the caller receives.** The watch loop prints nothing, handles no signal of the
//! process and does not depend on a terminal. It hands each [`Event`] to the function its
//! caller gave: a build starting, with the files that caused it; a build done, with its
//! [`update::Update`], its diagnostics and the output now current; a problem of the
//! watcher. A build that was cancelled hands no build done. With a build done comes its
//! [document snapshot](snapshot::Snapshot), computed
//! on the watch loop's thread; and, when the build's preparation failed, the document's
//! packages as [`crate::session::document_packages`] finds them ([`FailedPreparation`]),
//! found on that thread too. It ends when its caller asks through a [`Handle`], which can
//! also ask for a build now. When a build does not hand over its output (it stops at the
//! preparation, fails as a program, or its output cannot be produced), the output before it
//! stays current, and the next build is compared with it. A build of a profile that
//! declares no output hands over the output without files, which becomes the current
//! output.
//!
//! ```no_run
//! # #[cfg(feature = "watch")] {
//! use std::sync::Arc;
//!
//! use flm_host::files::DiskFiles;
//! use flm_host::session::{Input, Request};
//! use flm_host::watch::{Event, Watch};
//!
//! let request = Request::new(Input::File(String::from("paper.flm")));
//! let watch = Watch::new(request, Arc::new(DiskFiles), Vec::new()).watching_files();
//! let handle = watch.handle();
//! watch.run(|event| {
//!     if let Event::Built(built) = event {
//!         let problems = built.snapshot.problems.len();
//!         println!("build {}: {problems} problems", built.update.build);
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
pub mod export;
#[cfg(feature = "watch")]
pub mod front_matter;
#[cfg(feature = "watch")]
pub mod server;
#[cfg(feature = "watch")]
pub mod settings;
#[cfg(feature = "watch")]
pub mod services;
pub mod snapshot;
pub mod update;
#[cfg(feature = "watch")]
pub mod viewer;
#[cfg(feature = "watch")]
pub mod watcher;

#[cfg(feature = "watch")]
pub use running::{BuildProgress, Built, Event, FailedPreparation, Handle, Preparation, Watch};

use crate::settings::CommandLineSetting;

/// Returns the two command-line settings that the watch loop ([`Watch`]) adds to every
/// build, after the settings of its session request ([`Request`](crate::session::Request)),
/// in this order:
///
/// 1. `html.source_spans=true`: the HTML backend writes each element's place in its source
///    into the page
///    ([`source_spans`](flm_core::stdlib::backends::html::HtmlRenderOptions::source_spans));
/// 2. `html.required_definitions=[flm.preview.script, flm.preview.style,
///    flm.html.style.errors]`: the page as a whole requires the preview's page script and
///    its stylesheet (declared by the feature [`preview`](crate::stdpkg::preview) of the
///    package `std`) and the HTML backend's stylesheet of the error marks
///    ([`STYLE_ERRORS`](flm_core::stdlib::backends::html::conventions::STYLE_ERRORS))
///    ([`required_definitions`](flm_core::stdlib::backends::html::HtmlRenderOptions::required_definitions)).
///
/// Without the second setting, the stylesheet of the error marks
/// ([`STYLE_ERRORS`](flm_core::stdlib::backends::html::conventions::STYLE_ERRORS): what
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
    use flm_core::stdlib::backends::html::conventions::STYLE_ERRORS;
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
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use flm_core::core::registry::PackageFactory;
    use flm_core::lang::FlmDiagnostic;
    use flm_core::progress::{Expected, Kind, Observer, Scope, Sheet, Task, measure};

    use super::export::{ExportError, ExportOrder, Exported, Exports, Job};
    use super::snapshot::{Processed, Snapshot};
    use super::update::{BuildOutput, Update};
    use super::watcher::{Seen, WatchCondition, WatchError, Watcher};
    use crate::files::{BuildFiles, FileRead, Files};
    use crate::output::{NamedFile, OutputError, OutputFolder, Sink};
    use crate::report::{ContentOffsets, Reported, expected_durations};
    use crate::session::{
        BUILD_KIND, DocumentPackages, Input, PrepareError, PrepareErrorKind, Prepared, Request, build, document_packages,
        prepare,
    };

    /// The kind of the root row of the progress sheet of each build of the watch loop,
    /// `flm.watch.build`, with no title.
    const ROOT_KIND: Kind = Kind::new("flm.watch.build");

    /// A message to the watch loop from its handle or from the watcher.
    enum Signal {
        /// These files of the watched set changed.
        Changed(Vec<String>),
        /// Build again now.
        Again,
        /// Build with this profile from now on.
        Profile(String),
        /// Carry out this request about an export, between two builds.
        Export(Job),
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
        /// with each path spelled as in [`FileRead::path`](crate::files::FileRead::path),
        /// those that caused a build cancelled before it included; it is empty for the
        /// first build and for a build requested through [`Handle::build_again`] or
        /// [`Handle::set_profile`] with nothing changed. A build that starts is followed by
        /// [`Event::Built`], unless it is cancelled.
        Building {
            /// The files that changed.
            changed: Vec<String>,
        },
        /// A build is done ([`Built`]). A cancelled build gives none.
        Built(Box<Built>),
        /// The watcher has a problem: a folder could not be watched, or the system reported
        /// a problem while watching ([`WatchError`]). The watch loop continues.
        Watcher(WatchError),
    }

    /// A finished build, as the watch loop hands it to its caller in [`Event::Built`]: a
    /// build that ran to its end or stopped by its error, never a cancelled one.
    #[derive(Debug)]
    pub struct Built {
        /// The files whose change caused the build, as [`Event::Building`] listed them.
        pub changed: Vec<String>,
        /// The build's preparation ([`Preparation`]): reused from an earlier build,
        /// prepared again, or failed. A build prepares again the first time, whenever the
        /// last preparation is out of date, and after a change of profile
        /// ([`Handle::set_profile`]).
        pub preparation: Preparation,
        /// The profile that the session's builds run: the profile the request names, set
        /// at start or by the last [`Handle::set_profile`]; else the profile of the last
        /// preparation that succeeded ([`Prepared::profile`]); `None` when neither exists.
        /// A failed preparation does not change it, and the following builds run it again.
        /// [`Built::output`] can then be the output of another profile, kept from an
        /// earlier build, when this build did not hand over its output.
        pub profile: Option<String>,
        /// The default profile: the profile that the request given to [`Watch::new`] names;
        /// else the front matter's `default_profile`, else `html`, as the last preparation
        /// that succeeded found them
        /// ([`FlmInEffect::default_profile`](crate::session::FlmInEffect::default_profile));
        /// `None` while the request names none and no preparation has succeeded. A failed
        /// preparation does not change it.
        pub default_profile: Option<String>,
        /// What changed from the last output to this build's output ([`Update`]). Its
        /// number ([`Update::build`]) counts every build that started, the cancelled ones
        /// included.
        pub update: Update,
        /// The current output: this build's output, or, when this build did not hand over
        /// its output (it stopped at the preparation, failed as a program, or its output
        /// could not be produced), the last output a build handed over; `None` until a
        /// build hands one over. A build of a profile that declares no output hands over
        /// the output without files: it has no entry file.
        pub output: Option<Arc<BuildOutput>>,
        /// The document snapshot ([`Snapshot`]): the build's problems, each as data, and
        /// what the run of the build that gave the current output knows of the document
        /// ([`Processed`]: its sources by name, which turn a place written in the page into
        /// a line and a column, its headings, its labels and its constructs), `None` until
        /// a build gives an output.
        ///
        /// The two parts usually come from the same build. A build that stops before the
        /// document is processed reports problems with no byte offset, which need no
        /// sources. A build that runs hands its output over unless a stage of the output
        /// fails for a reason other than the document's errors (the backend cannot read its
        /// tables or its options, or no names can be derived for the output files); only
        /// then is the processed document that of an earlier build while the problems are
        /// this build's.
        pub snapshot: Arc<Snapshot>,
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
        /// ([`Record::of_diagnostic`](crate::report::Record::of_diagnostic)); empty when
        /// the build stopped before the document was processed, whose diagnostics and
        /// problems have no byte offset.
        pub content_offsets: ContentOffsets,
        /// How long the build took: from its start, preparation included, to the end of its
        /// comparison with the last output.
        pub duration: Duration,
    }

    /// The preparation of a build ([`Built::preparation`]). Whatever the preparation, a
    /// build that stopped holds its error in [`Built::outcome`]: a build whose preparation
    /// failed, and also a build whose run failed after a preparation reused or prepared
    /// again.
    #[derive(Debug)]
    pub enum Preparation {
        /// The build reused the preparation of an earlier build, which was still current
        /// and which an earlier [`Built`] handed over.
        Reused,
        /// The build prepared again ([`crate::session::prepare`]), and the preparation
        /// succeeded; or the build reused a preparation that a cancelled build made, which
        /// no [`Built`] handed over before. It is what the following builds use until it is
        /// out of date, from which a caller reads the packages, the workflow, the outputs,
        /// the profiles the document can run ([`Prepared::profiles`]) and the schema of its
        /// settings.
        Prepared(Arc<Prepared>),
        /// The build prepared again, and the preparation failed: what the watch loop found
        /// of the document then ([`FailedPreparation`]). [`Built::outcome`] holds the
        /// error.
        Failed(FailedPreparation),
    }

    impl Preparation {
        /// The preparation this build hands over ([`Preparation::Prepared`]); else `None`.
        pub fn prepared(&self) -> Option<&Arc<Prepared>> {
            match self {
                Preparation::Prepared(prepared) => Some(prepared),
                Preparation::Reused | Preparation::Failed(_) => None,
            }
        }
    }

    /// What the watch loop finds of the document after a preparation that failed, in
    /// [`Preparation::Failed`]: the document's packages, so that a caller can keep
    /// what follows from them alone (the schema of its settings, the packages found, the
    /// math processors) as current as the document, although the preparation failed.
    #[derive(Debug)]
    pub struct FailedPreparation {
        /// The document's packages as [`document_packages`] finds them, which reads only
        /// the keys of the front matter that decide them; `None` when they cannot be found
        /// either (for example, a package that the front matter lists under `use` does not
        /// exist).
        pub packages: Option<DocumentPackages>,
    }

    /// The progress of the build under way, as [`Handle::progress`] gives it: the reading
    /// half of the build's progress sheet, the table of expected durations to estimate the
    /// build with, and the instant from which the sheet's times are counted. See the
    /// [module documentation](super), "The progress of a build".
    ///
    /// The fraction of the build done at this moment is
    /// `estimate(&progress.sheet.snapshot(), &progress.expected, progress.origin.elapsed())`
    /// ([`flm_core::progress::estimate`]).
    #[derive(Debug, Clone)]
    pub struct BuildProgress {
        /// The reading half of the build's progress sheet: [`Sheet::snapshot`] gives its
        /// rows.
        pub sheet: Sheet,
        /// The table of expected durations to estimate the build with: the durations
        /// measured on the last build handed over ([`measure`]), laid over the table
        /// shipped with the host library ([`expected_durations`]).
        pub expected: Arc<Expected>,
        /// The instant from which the times of the sheet's rows are counted: the start of
        /// the build.
        pub origin: Instant,
    }

    /// The progress of the build under way, shared by the watch loop and its handles.
    #[derive(Debug, Clone, Default)]
    struct UnderWay(Arc<Mutex<Option<BuildProgress>>>);

    impl UnderWay {
        fn lock(&self) -> std::sync::MutexGuard<'_, Option<BuildProgress>> {
            self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        /// Cancels the build under way, if any.
        fn cancel(&self) {
            if let Some(progress) = &*self.lock() {
                progress.sheet.cancel();
            }
        }
    }

    /// A handle to a watch loop, through which its caller stops the loop, asks for a
    /// build, sets the profile of the next builds, names and carries out an export
    /// ([`Watch::handle`]), or reads the progress of the build under way. It is cheap to
    /// clone and can be used from any thread. Once the watch loop has ended,
    /// [`stop`](Handle::stop), [`build_again`](Handle::build_again) and
    /// [`set_profile`](Handle::set_profile) do nothing, [`progress`](Handle::progress)
    /// returns `None`, and [`export_name`](Handle::export_name) and
    /// [`export`](Handle::export) return [`ExportError::Ended`].
    #[derive(Debug, Clone)]
    pub struct Handle {
        signals: Sender<Signal>,
        under_way: UnderWay,
    }

    impl Handle {
        /// Ends the watch loop: cancels the build under way, if any; the watch loop then
        /// stops watching files and [`Watch::run`] returns.
        pub fn stop(&self) {
            // Cancelled before the signal is sent, so that the build the signal could start
            // is never the one cancelled.
            self.under_way.cancel();
            let _ = self.signals.send(Signal::Stop);
        }

        /// Asks for a build now, as if a file the document read had changed: one build, or,
        /// while a build runs, cancels it and builds again.
        pub fn build_again(&self) {
            self.under_way.cancel();
            let _ = self.signals.send(Signal::Again);
        }

        /// The progress of the build under way ([`BuildProgress`]); `None` between two
        /// builds.
        pub fn progress(&self) -> Option<BuildProgress> {
            self.under_way.lock().clone()
        }

        /// Sets the profile of the next builds to `profile` and asks for a build now: the
        /// watch loop prepares the document again with the session request's
        /// [`profile`](crate::session::Request::profile) set to `profile` and its
        /// [`format`](crate::session::Request::format) cleared, so that every output of the
        /// profile's workflow is produced, then builds. The choice lasts until the next
        /// call and is never written into the document. While a build runs, the call
        /// cancels it, and the change applies to the build that follows; of several calls
        /// before that build, the last counts.
        ///
        /// A profile that cannot be prepared (unknown, or whose extension fails) gives a
        /// build that stops at the preparation, with its problems; the last output stays
        /// current ([`Built::output`]).
        pub fn set_profile(&self, profile: impl Into<String>) {
            self.under_way.cancel();
            let _ = self.signals.send(Signal::Profile(profile.into()));
        }

        /// Returns the name of the export of the build type `profile`, `STEM.EXT` ([module
        /// documentation of `export`](super::export)): the watch loop prepares the build
        /// type, between two builds, and keeps the preparation for the export. Waits until
        /// the watch loop answers, after the build under way, if any; never call it from
        /// the function given to [`Watch::run`], which the watch loop waits for.
        ///
        /// # Errors
        ///
        /// [`ExportError::NoFile`], [`ExportError::NoOutput`] or [`ExportError::Failed`]
        /// when the build type gives no name, and [`ExportError::Ended`] once the watch
        /// loop has ended.
        pub fn export_name(&self, profile: impl Into<String>) -> Result<String, ExportError> {
            let (reply, answer) = channel();
            let _ = self.signals.send(Signal::Export(Job::Name { profile: profile.into(), reply }));
            answer.recv().unwrap_or(Err(ExportError::Ended))
        }

        /// Carries out the export `order` ([module documentation of
        /// `export`](super::export)): the watch loop builds the document as `flm render -o
        /// STEM.EXT` builds it, between two builds of the preview, and writes the files
        /// beside the document or gives them. Waits until the export is done, after the
        /// build under way, if any; never call it from the function given to
        /// [`Watch::run`], which the watch loop waits for.
        ///
        /// # Errors
        ///
        /// An [`ExportError`] when nothing is written, or when some files could not be
        /// written; [`ExportError::Ended`] once the watch loop has ended.
        pub fn export(&self, order: ExportOrder) -> Result<Exported, ExportError> {
            let (reply, answer) = channel();
            let _ = self.signals.send(Signal::Export(Job::Export { order, reply }));
            answer.recv().unwrap_or(Err(ExportError::Ended))
        }
    }

    /// The sink of the watch loop's builds: it keeps the files of the last output handed
    /// to it until the watch loop takes them after a build, so that a build that did not
    /// hand over its output is told apart from one that handed the same files again. A
    /// build of a profile that declares no output hands nothing to the sink; the watch loop
    /// gives it the output without files.
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
        fn receive(&self, files: &[NamedFile], _progress: &Scope) -> Result<(), Vec<OutputError>> {
            *self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(files.to_vec());
            Ok(())
        }
    }

    /// The watch loop of the live preview: builds a document, then builds it again at
    /// each change of a file it read and at each call to [`Handle::build_again`] or
    /// [`Handle::set_profile`], and hands each [`Event`] to its caller. See the [module
    /// documentation](super).
    ///
    /// Create it with [`Watch::new`], add [`Watch::watching_files`] to watch the disk,
    /// keep a [`Handle`] ([`Watch::handle`]) to stop it, ask for a build or set the
    /// profile, and start it with [`Watch::run`].
    pub struct Watch {
        request: Request,
        /// The profile that the request given to [`Watch::new`] names.
        asked: Option<String>,
        files: Arc<dyn Files>,
        compiled: Vec<PackageFactory>,
        received: Arc<Received>,
        exports: Exports,
        watching: bool,
        signals: Sender<Signal>,
        incoming: Receiver<Signal>,
        under_way: UnderWay,
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
        /// The watch loop changes four fields of `request`: it replaces the sink by one of
        /// its own (the output stays in memory), sets `output_folder` to its default (an
        /// output in memory is read from the current folder, and a file it refers to keeps
        /// the path the build gives it, absolute or relative to the current folder:
        /// [`OutputFolder::address_of`]), sets `continue_with_errors`, and appends the
        /// settings of [`build_settings`](super::build_settings) to `options`. At each
        /// call to [`Handle::set_profile`], it also replaces `profile` by the name given
        /// and clears `format`. Every other field is used as the caller set it. The exports
        /// ([`Handle::export`]) start from `request` as given, its `output_folder`'s current
        /// folder included. The watch loop watches no file until [`Watch::watching_files`]
        /// is called.
        pub fn new(mut request: Request, files: Arc<dyn Files>, compiled: Vec<PackageFactory>) -> Watch {
            let exports = Exports::new(request.clone(), Arc::clone(&files), compiled.clone());
            let received = Arc::new(Received::default());
            request.sink = Some(Arc::clone(&received) as Arc<dyn Sink>);
            request.output_folder = OutputFolder::default();
            request.continue_with_errors = true;
            request.options.extend(super::build_settings());
            let (signals, incoming) = channel();
            let asked = request.profile.clone();
            Watch {
                request,
                asked,
                files,
                compiled,
                received,
                exports,
                watching: false,
                signals,
                incoming,
                under_way: UnderWay::default(),
            }
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

        /// Returns a handle to this watch loop, through which a caller stops the loop, asks
        /// it for a build, and reads the progress of the build under way
        /// ([`Handle::progress`]); see [`Handle`] for the other requests it sends.
        pub fn handle(&self) -> Handle {
            Handle { signals: self.signals.clone(), under_way: self.under_way.clone() }
        }

        /// Runs the watch loop on the current thread until a [`Handle`] stops it, handing
        /// each event to `on_event`: builds the document once, then builds again at each
        /// change and each call to [`Handle::build_again`] or [`Handle::set_profile`]. See
        /// the [module documentation](super).
        ///
        /// # Errors
        ///
        /// Returns a [`WatchError`] when the watch loop is set to watch files
        /// ([`Watch::watching_files`]) and the system's file watching cannot be started;
        /// nothing is built then.
        pub fn run(mut self, mut on_event: impl FnMut(Event)) -> Result<(), WatchError> {
            let mut watcher = if self.watching {
                let signals = self.signals.clone();
                let under_way = self.under_way.clone();
                Some(Watcher::new(move |seen| {
                    let _ = signals.send(match seen {
                        Seen::Changed(paths) => {
                            // Cancelled before the signal is sent, as by a handle.
                            under_way.cancel();
                            Signal::Changed(paths)
                        }
                        Seen::Failed(detail) => Signal::Watcher(WatchError { condition: WatchCondition::Watching, detail }),
                    });
                })?)
            } else {
                None
            };
            let mut state = State {
                prepared: None,
                unreported: None,
                profile: None,
                default_profile: self.asked.clone(),
                last: None,
                processed: None,
                count: 0,
                watched: self.watched_set(&[]),
                expected: Arc::new(expected_durations()),
            };
            let mut changed = Vec::new();
            watch(&mut watcher, &state.watched, &mut on_event);
            loop {
                on_event(Event::Building { changed: changed.clone() });
                let attempt = self.build(&mut state, changed, Instant::now());
                watch(&mut watcher, &state.watched, &mut on_event);
                // The changes that caused a cancelled build cause the next one too.
                changed = match attempt {
                    Attempt::Built(built) => {
                        on_event(Event::Built(built));
                        Vec::new()
                    }
                    Attempt::Cancelled { changed } => changed,
                };
                let Some(next) = self.wait(&mut on_event) else { return Ok(()) };
                for path in next.changed {
                    if !changed.contains(&path) {
                        changed.push(path);
                    }
                }
                if let Some(profile) = next.profile {
                    self.request.profile = Some(profile);
                    self.request.format = None;
                    state.prepared = None;
                }
            }
        }

        /// Waits for a change or a call to [`Handle::build_again`] or
        /// [`Handle::set_profile`], and collects every signal already waiting with it: what
        /// the next build is to do, or `None` when the watch loop is to end. A request
        /// about an export is carried out as it comes, between the two builds.
        fn wait(&mut self, on_event: &mut impl FnMut(Event)) -> Option<Next> {
            let mut changed: Vec<String> = Vec::new();
            let mut profile = None;
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
                    Signal::Profile(name) => {
                        profile = Some(name);
                        build = true;
                    }
                    Signal::Watcher(error) => on_event(Event::Watcher(error)),
                    Signal::Export(job) => self.exports.carry_out(job),
                    Signal::Stop => return None,
                }
                next = match self.incoming.try_recv() {
                    Ok(signal) => Some(signal),
                    Err(_) if build => None,
                    Err(_) => self.incoming.recv().ok(),
                };
            }
            build.then_some(Next { changed, profile })
        }

        /// One build, over what `state` holds, caused by the change of `changed`, started
        /// at `started`, on a progress sheet of its own, which the handles reach while it
        /// runs. When the build is handed over, the durations measured on its sheet are laid
        /// over the shipped table, to estimate the next build with.
        fn build(&self, state: &mut State, changed: Vec<String>, started: Instant) -> Attempt {
            state.count += 1;
            let observer = Observer {
                row_started: Some(Box::new(move |_row| Some(started.elapsed()))),
                row_ended: Some(Box::new(move |_row| Some(started.elapsed()))),
                ..Observer::default()
            };
            let (sheet, root) = Sheet::start(&ROOT_KIND, None, observer, self.request.log_sink.clone());
            *self.under_way.lock() =
                Some(BuildProgress { sheet: sheet.clone(), expected: Arc::clone(&state.expected), origin: started });
            let attempt = self.attempt(state, changed, started, root);
            *self.under_way.lock() = None;
            if let Attempt::Built(_) = attempt {
                state.expected = Arc::new(measure(&sheet.snapshot()).over(expected_durations()));
            }
            attempt
        }

        /// The build of [`Watch::build`], reporting under `root`, which it ends: prepares
        /// when the last preparation is out of date, then builds.
        fn attempt(&self, state: &mut State, changed: Vec<String>, started: Instant, root: Task) -> Attempt {
            let progress = root.scope();
            // The build's row is declared before *prepare* runs, so that an estimate counts
            // the build's work from the start.
            let build_row = progress.child(&BUILD_KIND, None);
            let mut preparation = Preparation::Reused;
            let current = state.prepared.take().filter(|prepared| prepared.is_current());
            if current.is_none() {
                // An out-of-date preparation is never handed over.
                state.unreported = None;
            }
            let prepared = match current {
                Some(prepared) => {
                    // A preparation that a cancelled build made is handed over by the
                    // build that reuses it.
                    if let Some(unreported) = state.unreported.take() {
                        preparation = Preparation::Prepared(unreported);
                    }
                    prepared
                }
                None => match prepare(self.request.clone(), Arc::clone(&self.files), &self.compiled, &progress) {
                    Ok(prepared) => {
                        let prepared = Arc::new(prepared);
                        state.default_profile =
                            self.asked.clone().or_else(|| Some(prepared.flm_in_effect().default_profile.value.clone()));
                        preparation = Preparation::Prepared(Arc::clone(&prepared));
                        prepared
                    }
                    Err(error) if error.kind == PrepareErrorKind::Cancelled => {
                        return self.cancelled(state, changed, &error);
                    }
                    Err(error) => {
                        if self.request.profile.is_some() {
                            state.profile.clone_from(&self.request.profile);
                        }
                        let packages = document_packages(self.request.clone(), Arc::clone(&self.files), &self.compiled).ok();
                        let preparation = Preparation::Failed(FailedPreparation { packages });
                        return Attempt::Built(self.failed(state, changed, preparation, error, started));
                    }
                },
            };
            state.profile = Some(prepared.profile().to_string());
            // What an earlier build handed over and this one did not must not count as
            // this build's.
            self.received.take();
            let build_task = build_row.start();
            let result = build(&prepared, &build_task.scope());
            let leading = prepared.outputs().first().cloned().unwrap_or_default();
            // A profile that declares no output hands no files to the sink, and its output
            // is the output without files, which has no labels either.
            let declares_none = prepared.outputs().is_empty();
            state.prepared = Some(prepared);
            let built = match result {
                Ok(built) => built,
                Err(error) if error.kind == PrepareErrorKind::Cancelled => {
                    if let Preparation::Prepared(prepared) = preparation {
                        state.unreported = Some(prepared);
                    }
                    return self.cancelled(state, changed, &error);
                }
                Err(error) => return Attempt::Built(self.failed(state, changed, preparation, error, started)),
            };
            build_task.finish();
            root.finish();
            let output = if declares_none {
                Some(Arc::new(BuildOutput::default()))
            } else {
                self.received.take().map(|files| Arc::new(BuildOutput::of_build(&built.run, &leading, files)))
            };
            state.watched = self.watched_set(&built.files_read);
            let problems: Vec<Reported> = built.diagnostics.iter().map(|entry| Reported::of_diagnostic(entry, &built.content_offsets)).collect();
            let update = Update::new(state.count, state.last.as_deref(), output.as_deref());
            if output.is_some() {
                state.last = output;
                state.processed = Some(Arc::new(Processed::of_run(&built.run)));
            }
            Attempt::Built(Box::new(Built {
                changed,
                preparation,
                profile: state.profile.clone(),
                default_profile: state.default_profile.clone(),
                update,
                output: state.last.clone(),
                snapshot: Arc::new(Snapshot { problems, processed: state.processed.clone() }),
                outcome: Ok(built.diagnostics),
                files_read: built.files_read,
                content_offsets: built.content_offsets,
                duration: started.elapsed(),
            }))
        }

        /// The build caused by the change of `changed`, cancelled with `error`, which
        /// lists what the attempt read: the watched set gains those paths, and nothing else
        /// of `state` changes.
        fn cancelled(&self, state: &mut State, changed: Vec<String>, error: &PrepareError) -> Attempt {
            for read in &error.files_read {
                if !state.watched.contains(&read.path) {
                    state.watched.push(read.path.clone());
                }
            }
            Attempt::Cancelled { changed }
        }

        /// The build numbered `state.count`, started at `started`, that did not hand over
        /// its output, stopped by `error`, which lists what the attempt read, after the
        /// preparation `preparation`.
        fn failed(
            &self,
            state: &mut State,
            changed: Vec<String>,
            preparation: Preparation,
            error: PrepareError,
            started: Instant,
        ) -> Box<Built> {
            state.watched = self.watched_set(&error.files_read);
            // The diagnostics found before the step that stopped, then its problems; none
            // has a byte offset counted from the top of its file.
            let none = ContentOffsets::new();
            let problems: Vec<Reported> = error
                .diagnostics
                .iter()
                .map(|entry| Reported::of_diagnostic(entry, &none))
                .chain(error.problems.iter().map(Reported::of_problem))
                .collect();
            let update = Update::new(state.count, state.last.as_deref(), None);
            let files_read = error.files_read.clone();
            Box::new(Built {
                changed,
                preparation,
                profile: state.profile.clone(),
                default_profile: state.default_profile.clone(),
                update,
                output: state.last.clone(),
                snapshot: Arc::new(Snapshot { problems, processed: state.processed.clone() }),
                outcome: Err(error),
                files_read,
                content_offsets: ContentOffsets::new(),
                duration: started.elapsed(),
            })
        }

        /// The watched set after a build attempt that read `read`, by the rule of the
        /// [module documentation](super), "Which files are watched": the primary file, then
        /// each path read, each once; before the first build `read` is empty.
        fn watched_set(&self, read: &[FileRead]) -> Vec<String> {
            let mut watched: Vec<String> = Vec::new();
            if let Input::File(path) = &self.request.input {
                // Named as the build's reading of files names it, so that it is one path.
                watched.push(BuildFiles::resolve("", path));
            }
            for read in read {
                if !watched.contains(&read.path) {
                    watched.push(read.path.clone());
                }
            }
            watched
        }
    }

    /// Makes `paths` the set `watcher` watches, when the watch loop watches files, and
    /// hands a problem for each folder that could not be watched to `on_event`.
    fn watch(watcher: &mut Option<Watcher>, paths: &[String], on_event: &mut impl FnMut(Event)) {
        if let Some(Err(errors)) = watcher.as_mut().map(|watcher| watcher.watch(paths)) {
            for error in errors {
                on_event(Event::Watcher(error));
            }
        }
    }

    /// How a build of the watch loop ended.
    enum Attempt {
        /// The build is handed over: done, or stopped by its error.
        Built(Box<Built>),
        /// The build was cancelled; `changed` lists the files whose change caused it.
        Cancelled {
            /// The files that changed.
            changed: Vec<String>,
        },
    }

    /// What the next build is to do, as the signals collected by `Watch::wait` say.
    struct Next {
        /// The files that changed, each once.
        changed: Vec<String>,
        /// The profile set through [`Handle::set_profile`], the last one when several were.
        profile: Option<String>,
    }

    // A caller may keep the preparation a build hands over ([`Preparation::Prepared`]) for the
    // threads of its own, as the server's requests are answered on.
    const _: fn() = || {
        fn shared<T: Send + Sync>() {}
        shared::<Prepared>();
    };

    /// What the watch loop keeps from one build to the next.
    struct State {
        /// The last preparation, until it is out of date or the profile changes.
        prepared: Option<Arc<Prepared>>,
        /// The last preparation when it succeeded in a build that was cancelled, and no
        /// build handed it over since ([`Preparation::Prepared`]).
        unreported: Option<Arc<Prepared>>,
        /// The profile in use ([`Built::profile`]).
        profile: Option<String>,
        /// The default profile ([`Built::default_profile`]).
        default_profile: Option<String>,
        /// The last output a build handed over.
        last: Option<Arc<BuildOutput>>,
        /// What the run of the build that gave [`State::last`] knows of the document.
        processed: Option<Arc<Processed>>,
        /// The number of builds so far.
        count: u64,
        /// The watched set: the primary file and the paths the last build attempt read
        /// (`Watch::watched_set`), with the paths that the cancelled attempts since read.
        watched: Vec<String>,
        /// The table of expected durations to estimate the next build with
        /// ([`BuildProgress::expected`]).
        expected: Arc<Expected>,
    }
}
