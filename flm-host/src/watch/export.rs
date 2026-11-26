//! The export of the live preview: a build by the watch loop of the document as `flm render
//! -o` builds it, written beside the document or handed to the browser (cargo feature
//! `watch`).
//!
//! The preview's own output cannot be saved as it is: it carries each element's place in
//! its source and the preview's page script. An *export* is therefore a build of its own,
//! which the watch loop carries out between two builds of the preview
//! ([`Handle::export`](super::Handle::export)), with:
//!
//! - the session request as the program gave it to [`Watch::new`](super::Watch::new), none
//!   of the preview's own settings added ([`build_settings`](super::build_settings)), its
//!   profile set to the build type chosen and its format cleared, so that every output of
//!   the build type's workflow is written;
//! - the files named as `flm render -o STEM.EXT` names them ([`Naming::File`]), where
//!   `STEM` is the document's file name without its extension and `EXT` the extension of
//!   the entry file of the build type's *leading output*, the first output of its workflow
//!   ([`Prepared::outputs`]); the export's *name* is `STEM.EXT`
//!   ([`Handle::export_name`](super::Handle::export_name));
//! - the document's folder as the output folder, the program's current folder kept from the
//!   request's [`output_folder`](crate::session::Request::output_folder).
//!
//! **Where the files go** ([`ExportAction`]). *Save as sibling* writes the files into the
//! document's folder through [`DiskSink`], as `flm render -o` does: each delivery of the
//! build is handed to the sink as the build made it. *Download* writes nothing and gives
//! the files to the caller ([`Exported::Download`]).
//!
//! **When nothing is written** ([`ExportError`]): the build type declares no output; a file
//! of a save has the path of a file that the build of the document read (its own file, for
//! a document named like its export); and, unless the export continues with errors:
//!
//! - the document has errors under the build type, found before any file is produced or
//!   any external program runs: the run stopped at its error gate
//!   ([`RunOutcome::HaltedAtGate`]). This is the one
//!   refusal that an export continuing with errors lifts
//!   ([`ExportError::DocumentErrors`]);
//! - the build fails: it stops, gives no entry file for the leading output, or reports an
//!   error from a stage that writes files or runs an external program, such as a failed run
//!   of LaTeX for the build type `pdf-latex` ([`ExportError::Failed`]). Such an error leaves
//!   nothing written, whether or not the main file was produced.
//!
//! An export that continues with errors is never stopped at the gate, and writes what its
//! build delivered when the build gives the leading output's entry file. Each refusal for
//! errors carries the errors of the export's build in full as its *log*
//! ([`ExportError::log`]), each laid out by [`report::diagnostic`]: the error of a failed
//! run of an external program quotes that program's log.
//!
//! **The preparation is kept.** The watch loop keeps the preparation of the last build type
//! asked for, and builds every export of that build type over it while it is current
//! ([`Prepared::is_current`]); asking for the name of an export prepares the build type.

use std::fmt;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use flm_core::core::artifact::NamingScope;
use flm_core::core::registry::PackageFactory;
use flm_core::core::run::RunOutcome;
use flm_core::progress::Scope;
use flm_core::techy::error::Severity;

use crate::files::{BuildFiles, Files};
use crate::output::{DiskSink, NamedFile, Naming, OutputError, OutputFolder, Sink};
use crate::report;
use crate::session::{Input, PrepareError, Prepared, Request, build, prepare};

/// Where the files of an export go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportAction {
    /// Written into the document's folder, as `flm render -o STEM.EXT` writes them
    /// (*Save as sibling*).
    SaveAsSibling,
    /// Given to the caller, to be saved by the browser (*Download*).
    Download,
}

/// An export to carry out ([`Handle::export`](super::Handle::export)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOrder {
    /// The build type: the profile the export runs.
    pub profile: String,
    /// Where the files go.
    pub action: ExportAction,
    /// Whether the export is written even when the document has errors, as `flm render
    /// --continue-with-errors` writes it.
    pub continue_with_errors: bool,
}

/// What an export wrote or gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exported {
    /// The files were written into the document's folder.
    Saved {
        /// The export's name, `STEM.EXT`.
        name: String,
        /// The path of each file written, relative to the document's folder, each once, in
        /// the order written.
        files: Vec<String>,
        /// The path of the entry file of the leading output (the *main file*), as the host
        /// writes paths ([`crate::files`]): the document's folder joined with its path.
        main: String,
    },
    /// The files, for the caller to hand over.
    Download {
        /// The export's name, `STEM.EXT`.
        name: String,
        /// `STEM`, the document's file name without its extension.
        stem: String,
        /// Each file, once (a file two outputs share as the first output gave it), in the
        /// order the build gave them.
        files: Vec<NamedFile>,
    },
}

/// Why an export wrote nothing, or could not write all its files. Its `Display` text is one
/// line for a reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The document is not a file: an export is named after the document's file.
    NoFile,
    /// The build type declares no output.
    NoOutput {
        /// The build type.
        profile: String,
    },
    /// The document has errors under the export's build type, found before any file is
    /// produced or any external program runs (the run stopped at its error gate), and the
    /// export does not continue with errors: the one refusal that an export continuing with
    /// errors lifts.
    DocumentErrors {
        /// The number of errors.
        count: usize,
        /// The errors, in full.
        log: String,
    },
    /// A file of a save has the path of a file that the build of the document read.
    ReadByBuild {
        /// The file's path, relative to the document's folder.
        path: String,
    },
    /// The build type could not be prepared, the build failed, or it gave no entry file for
    /// the leading output. The build fails, too, when a stage that writes files or runs an
    /// external program reports an error and the export does not continue with errors.
    Failed {
        /// The export's name, `STEM.EXT`, when the build type was prepared; else the build
        /// type.
        name: String,
        /// What failed, one line.
        reason: String,
        /// The errors the build reported, in full, when it reported any.
        log: Option<String>,
    },
    /// The sink could not write some of the files ([`DiskSink`]).
    NotWritten {
        /// What could not be written.
        errors: Vec<OutputError>,
    },
    /// The watch loop has ended.
    Ended,
}

impl ExportError {
    /// The errors of the export's build, in full, one after the other with a blank line
    /// between two, for an export refused for the document's errors or for a failed build
    /// that reported errors: among them, the error of a failed run of an external program,
    /// which quotes that program's log. `None` for every other reason.
    pub fn log(&self) -> Option<&str> {
        match self {
            ExportError::DocumentErrors { log, .. } => Some(log),
            ExportError::Failed { log, .. } => log.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::NoFile => f.write_str("Missing document file"),
            ExportError::NoOutput { profile } => write!(f, "Missing output for build type ‘{profile}’: it writes no file"),
            ExportError::DocumentErrors { count, .. } => {
                let errors = if *count == 1 { String::from("1 error") } else { format!("{count} errors") };
                write!(f, "Document with {errors}: an export despite the errors must be confirmed")
            }
            ExportError::ReadByBuild { path } => {
                write!(f, "Invalid export file ‘{path}’: the document's build reads a file of this name")
            }
            ExportError::Failed { name, reason, .. } => write!(f, "Failed to build ‘{name}’: {reason}"),
            ExportError::NotWritten { errors } => {
                let detail: Vec<String> = errors.iter().map(|error| first_line(&error.detail).to_owned()).collect();
                write!(f, "Failed to write the export: {}", detail.join("; "))
            }
            ExportError::Ended => f.write_str("Missing watch loop: the preview has ended"),
        }
    }
}

impl std::error::Error for ExportError {}

/// The first line of `text`.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// A request to the watch loop about an export, with the channel on which it answers.
pub(super) enum Job {
    /// The name of the export of a build type.
    Name {
        /// The build type.
        profile: String,
        /// Where the answer goes.
        reply: Sender<Result<String, ExportError>>,
    },
    /// An export.
    Export {
        /// The export to carry out.
        order: ExportOrder,
        /// Where the answer goes.
        reply: Sender<Result<Exported, ExportError>>,
    },
}

/// The sink of the exports' builds: it keeps every delivery of the build under way, for the
/// watch loop to write or hand over once the build is done.
#[derive(Debug, Default)]
struct Collected {
    deliveries: Mutex<Vec<Vec<NamedFile>>>,
}

impl Collected {
    fn take(&self) -> Vec<Vec<NamedFile>> {
        std::mem::take(&mut *self.deliveries.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
    }
}

impl Sink for Collected {
    fn receive(&self, files: &[NamedFile], _progress: &Scope) -> Result<(), Vec<OutputError>> {
        self.deliveries.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(files.to_vec());
        Ok(())
    }
}

/// The preparation kept for the exports of one build type.
struct Kept {
    /// The build type.
    profile: String,
    /// The export's name, `STEM.EXT`.
    name: String,
    /// The preparation, its files named after `name`.
    prepared: Prepared,
}

/// What the watch loop keeps for its exports: see the [module documentation](self).
pub(super) struct Exports {
    /// The session request as the program gave it.
    given: Request,
    files: Arc<dyn Files>,
    compiled: Vec<PackageFactory>,
    collected: Arc<Collected>,
    kept: Option<Kept>,
}

impl Exports {
    /// The exports of the document of `given`, the session request as the program gave it,
    /// read through `files` with the compiled packages `compiled`.
    pub(super) fn new(given: Request, files: Arc<dyn Files>, compiled: Vec<PackageFactory>) -> Exports {
        Exports { given, files, compiled, collected: Arc::new(Collected::default()), kept: None }
    }

    /// Carries out `job` and sends its answer.
    pub(super) fn carry_out(&mut self, job: Job) {
        match job {
            Job::Name { profile, reply } => {
                let _ = reply.send(self.kept(&profile).map(|kept| kept.name.clone()));
            }
            Job::Export { order, reply } => {
                let _ = reply.send(self.export(&order));
            }
        }
    }

    /// The document's file and its folder, as the host writes paths.
    fn document(&self) -> Result<(&str, &str), ExportError> {
        match &self.given.input {
            Input::File(path) => Ok((path, BuildFiles::folder_of(path))),
            Input::Text(_) => Err(ExportError::NoFile),
        }
    }

    /// The preparation of the build type `profile`, its files named after the export's
    /// name: the one kept when it is of that build type and current, else a new one, which
    /// is kept.
    fn kept(&mut self, profile: &str) -> Result<&mut Kept, ExportError> {
        let current = self.kept.as_ref().is_some_and(|kept| kept.profile == profile && kept.prepared.is_current());
        if !current {
            self.kept = None;
            self.kept = Some(self.prepare(profile)?);
        }
        Ok(self.kept.as_mut().expect("kept above"))
    }

    /// Prepares the build type `profile`, its files named as `flm render -o STEM.EXT` names
    /// them. The extension comes from the preparation: the build type is first prepared
    /// with the files named after `STEM`, which names them alike unless `STEM` itself ends
    /// with the extension of an output; only then is it prepared again.
    fn prepare(&self, profile: &str) -> Result<Kept, ExportError> {
        let (path, folder) = self.document()?;
        let stem = stem_of(path);
        let prepared = self.prepare_named(profile, folder, Naming::File { name: stem.clone() })?;
        let Some(leading) = prepared.outputs().first() else {
            return Err(ExportError::NoOutput { profile: profile.to_owned() });
        };
        let extension = prepared.registry().output(leading).map(|output| output.def.entry.extension.clone()).unwrap_or_default();
        let name = if extension.is_empty() { stem.clone() } else { format!("{stem}.{extension}") };
        let naming = Naming::File { name: name.clone() };
        let scope = NamingScope::of(prepared.registry(), prepared.plan().demands());
        let alike = naming.stem(&scope) == Naming::File { name: stem }.stem(&scope);
        let prepared = if alike {
            prepared
        } else {
            self.prepare_named(profile, folder, naming)?
        };
        Ok(Kept { profile: profile.to_owned(), name, prepared })
    }

    /// Prepares the build type `profile` from the session request as the program gave it,
    /// the files named by `naming` in the folder `folder`, handed to the exports' sink.
    fn prepare_named(&self, profile: &str, folder: &str, naming: Naming) -> Result<Prepared, ExportError> {
        let mut request = self.given.clone();
        request.profile = Some(profile.to_owned());
        request.format = None;
        request.naming = naming;
        request.output_folder = OutputFolder::new(folder).with_current(self.given.output_folder.current.clone());
        request.sink = Some(Arc::clone(&self.collected) as Arc<dyn Sink>);
        prepare(request, Arc::clone(&self.files), &self.compiled, &Scope::none())
            .map_err(|error| failed_preparing(profile, &error))
    }

    /// Carries out `order`: see the [module documentation](self).
    fn export(&mut self, order: &ExportOrder) -> Result<Exported, ExportError> {
        let (path, folder) = self.document().map(|(path, folder)| (path.to_owned(), folder.to_owned()))?;
        let collected = Arc::clone(&self.collected);
        let kept = self.kept(&order.profile)?;
        kept.prepared.set_continue_with_errors(order.continue_with_errors);
        // What an earlier build delivered must not count as this one's.
        collected.take();
        let built = build(&kept.prepared, &Scope::none());
        let deliveries = collected.take();
        let name = kept.name.clone();
        let built = built.map_err(|error| ExportError::Failed { name: name.clone(), reason: reason_of(&error), log: None })?;
        let errors: Vec<_> =
            built.diagnostics.iter().filter(|diagnostic| diagnostic.severity() == Severity::Error).collect();
        let log = errors.iter().map(|error| report::diagnostic(error)).collect::<Vec<String>>().join("\n\n");
        let reason = errors.first().map(|error| first_line(&error.message()).to_owned());
        if built.run.outcome() == Some(RunOutcome::HaltedAtGate) {
            return Err(ExportError::DocumentErrors { count: errors.len(), log });
        }
        // A run not stopped at the gate had no error before it: an error it reports comes
        // from a stage that writes files or runs an external program.
        if let Some(reason) = reason.as_ref().filter(|_| !order.continue_with_errors) {
            return Err(ExportError::Failed { name, reason: reason.clone(), log: Some(log) });
        }
        let leading = kept.prepared.outputs().first().cloned().unwrap_or_default();
        let main = deliveries.iter().flatten().find(|file| file.entry && file.output == leading).map(|file| file.path.clone());
        let Some(main) = main else {
            let reason = reason.unwrap_or_else(|| format!("the build wrote no ‘{name}’"));
            let log = (!errors.is_empty()).then_some(log);
            return Err(ExportError::Failed { name, reason, log });
        };
        let mut files: Vec<NamedFile> = Vec::new();
        for file in deliveries.iter().flatten() {
            if !files.iter().any(|kept| kept.path == file.path) {
                files.push(file.clone());
            }
        }
        match order.action {
            ExportAction::Download => Ok(Exported::Download { name, stem: stem_of(&path), files }),
            ExportAction::SaveAsSibling => {
                let read: Vec<&str> = built.files_read.iter().filter(|read| read.found).map(|read| read.path.as_str()).collect();
                if let Some(file) = files.iter().find(|file| read.contains(&BuildFiles::resolve(&folder, &file.path).as_str())) {
                    return Err(ExportError::ReadByBuild { path: file.path.clone() });
                }
                let sink = DiskSink::new(folder.clone());
                for delivery in &deliveries {
                    sink.receive(delivery, &Scope::none()).map_err(|errors| ExportError::NotWritten { errors })?;
                }
                Ok(Exported::Saved {
                    name,
                    files: files.into_iter().map(|file| file.path).collect(),
                    main: BuildFiles::resolve(&folder, &main),
                })
            }
        }
    }
}

/// `STEM`: the last name of the path `path` without its extension (the part from its last
/// dot on, unless the name begins with that dot).
fn stem_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.rfind('.') {
        Some(dot) if dot > 0 => file[..dot].to_owned(),
        _ => file.to_owned(),
    }
}

/// The error of an export whose build type `profile` could not be prepared.
fn failed_preparing(profile: &str, error: &PrepareError) -> ExportError {
    ExportError::Failed { name: profile.to_owned(), reason: reason_of(error), log: None }
}

/// The first line of the first problem of `error`, or of its first diagnostic.
fn reason_of(error: &PrepareError) -> String {
    match (error.problems.first(), error.diagnostics.first()) {
        (Some(problem), _) => first_line(&problem.detail).to_owned(),
        (None, Some(diagnostic)) => first_line(&diagnostic.message()).to_owned(),
        (None, None) => String::from("the build stopped"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stem_is_the_file_name_without_its_extension() {
        assert_eq!(stem_of("doc/paper.flm"), "paper");
        assert_eq!(stem_of("notes.v2.flm"), "notes.v2");
        assert_eq!(stem_of("README"), "README");
        assert_eq!(stem_of("doc/.flm"), ".flm");
    }
}
