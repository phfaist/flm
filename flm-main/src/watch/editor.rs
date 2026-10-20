//! Opening a place of the document's files in an editor: finding the editors on this
//! machine, and turning a place written in the preview's page into a line and a column.
//!
//! An author reading the live preview wants to go from what is shown to where it is
//! written. A click on an element of the page with Ctrl or Cmd and Shift, or the button
//! *Open in editor* of a problem in the toolbar's list, opens the file in an editor at the
//! line and the column of that place. The page holds each place in the form the library
//! writes into each element (the HTML backend's render option `source_spans`): the name of a
//! source (the primary file as the command line gave it, or an included file) and a byte
//! offset into the text the library parsed, which, for a file with a front matter, is the
//! content below the front matter. Three types turn that into an editor opened at a line and
//! a column:
//!
//! - [`Places`] holds a build's sources, by name, and turns a byte offset into the line
//!   and the column an editor counts: the line as the file numbers it, front matter
//!   included, and the column in characters, not in bytes;
//! - [`Editors`] holds the editors found on this machine and the *default*, the editor a
//!   place is opened in when no other is asked for ([`Editors::find`]);
//! - [`Editor`] is one of the editors found; it starts the editor on a file, a line and a
//!   column ([`Editor::open`]) and reports how the editor ended ([`Started::wait`]).
//!
//! The editors the preview knows are those of [`EDITORS`], in this order: `code` (Visual
//! Studio Code), `zed` (Zed), `emacsclient` (Emacs, in the frame already open), `subl`
//! (Sublime Text) and `gvim` (the graphical Vim). Each is looked for by the name of its
//! program in the folders of a search path and, when it is not there, inside the
//! application bundle that installs it on macOS. Vim in a terminal is not among them: the
//! preview starts an editor without a terminal, where it could not show the file. The
//! preview's page lists the editors found in a menu, each by its label
//! ([`Editor::label`]), and asks for each place to be opened in the editor chosen there.
//!
//! The preview's server ([`super::server`]) checks which files may be opened: only a file
//! the last build read.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use flm_core::core::run::Run;
use flm_core::lang::{FlmSource, FlmSpan};

/// What the preview knows of one editor: the name of its program, the label the page's menu
/// shows, the arguments that open a file at a line and a column, and where its program lies
/// inside the macOS application bundles that install the editor.
#[derive(Debug)]
struct Known {
    /// The name of its program, one of [`EDITORS`].
    name: &'static str,
    /// The editor's name as the page's menu shows it.
    label: &'static str,
    /// The arguments that open a file at a line and a column ([`Editor::arguments`]).
    arguments: fn(&Path, usize, usize) -> Vec<OsString>,
    /// The paths of its program inside the application bundles that install the editor,
    /// relative to an application folder, in the order they are looked for.
    bundled: &'static [&'static str],
}

/// The editors the preview knows, in the order of [`EDITORS`].
// See decision rationale at [§flm-dd-dr:host-watch-editor-menu].
const KNOWN: [Known; 5] = [
    Known {
        name: "code",
        label: "VS Code",
        arguments: |file, line, column| vec![OsString::from("--goto"), located(file, line, column)],
        bundled: &["Visual Studio Code.app/Contents/Resources/app/bin/code"],
    },
    Known {
        name: "zed",
        label: "Zed",
        arguments: |file, line, column| vec![located(file, line, column)],
        bundled: &["Zed.app/Contents/MacOS/cli", "Zed Preview.app/Contents/MacOS/cli"],
    },
    Known {
        name: "emacsclient",
        label: "Emacs",
        arguments: |file, line, column| vec![OsString::from("-n"), OsString::from(format!("+{line}:{column}")), file.into()],
        bundled: &["Emacs.app/Contents/MacOS/bin/emacsclient"],
    },
    Known {
        name: "subl",
        label: "Sublime Text",
        arguments: |file, line, column| vec![located(file, line, column)],
        bundled: &["Sublime Text.app/Contents/SharedSupport/bin/subl"],
    },
    Known {
        name: "gvim",
        label: "gVim",
        arguments: |file, line, column| {
            vec![
                OsString::from(format!("+{line}")),
                OsString::from("-c"),
                OsString::from(format!("normal! {column}|")),
                file.into(),
            ]
        },
        bundled: &[],
    },
];

/// The editors the preview can open a file in, by the name of their program, in the order
/// [`Editors::find`] lists the editors it finds: `code`, `zed`, `emacsclient`, `subl` and
/// `gvim`.
pub const EDITORS: [&str; 5] = {
    let mut names = [""; 5];
    let mut at = 0;
    while at < KNOWN.len() {
        names[at] = KNOWN[at].name;
        at += 1;
    }
    names
};

/// `FILE:LINE:COLUMN`, one argument.
fn located(file: &Path, line: usize, column: usize) -> OsString {
    let mut located = file.as_os_str().to_owned();
    located.push(format!(":{line}:{column}"));
    located
}

/// One of the editors of [`EDITORS`], with the program found for it on this machine.
///
/// Found with [`Editors::find`]; [`Editor::open`] starts it on a file at a line and a
/// column.
#[derive(Debug, Clone)]
pub struct Editor {
    known: &'static Known,
    program: PathBuf,
}

/// Two editors are equal when they have the same name and the same program.
impl PartialEq for Editor {
    fn eq(&self, other: &Editor) -> bool {
        self.known.name == other.known.name && self.program == other.program
    }
}

impl Eq for Editor {}

/// The error of [`Editors::find`] when the preferred editor's name is not one of
/// [`EDITORS`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEditor {
    /// The name as given.
    pub name: String,
}

impl fmt::Display for UnknownEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the editor `{}` must be one of the editors `{}`", self.name, EDITORS.join("`, `"))
    }
}

impl std::error::Error for UnknownEditor {}

/// The editors of [`EDITORS`] found on this machine, in that order, and the *default*: the
/// editor a place is opened in when no other is asked for.
///
/// Made with [`Editors::find`], or [`Editors::none`] for no editor at all. The preview's
/// server ([`Server::start`](super::server::Server::start)) lists these editors for the
/// page's menu and opens each place in the editor the page asks for, or in the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editors {
    /// The editors found, in the order of [`EDITORS`].
    found: Vec<Editor>,
    /// The position of the default in `found`; `None` when nothing was found.
    default: Option<usize>,
    /// The application folders the editors were looked for in.
    folders: Vec<PathBuf>,
}

impl Editors {
    /// Looks for every editor of [`EDITORS`] on this machine, and returns those found, in
    /// the order of [`EDITORS`].
    ///
    /// An editor's program is looked for by its name in the folders of the search path
    /// `search_path` (a list of folders in the form of the variable `PATH`; `None` for an
    /// empty search path). When it is not there, it is looked for inside the macOS
    /// application bundle that installs the editor, in each folder of `application_folders`
    /// in turn (on macOS, `/Applications` and the `Applications` folder of the home folder):
    /// `Visual Studio Code.app/Contents/Resources/app/bin/code`, `Zed.app/Contents/MacOS/cli`
    /// (then, in each folder again, `Zed Preview.app/Contents/MacOS/cli`),
    /// `Emacs.app/Contents/MacOS/bin/emacsclient` and
    /// `Sublime Text.app/Contents/SharedSupport/bin/subl`; `gvim` is looked for on the
    /// search path only. A program counts only when it is an executable file.
    ///
    /// The default ([`Editors::default`]) is the editor `preferred` when it is given and
    /// found, else the first editor found; there is none when no editor is found.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownEditor`] when `preferred` is not one of [`EDITORS`].
    pub fn find(
        preferred: Option<&str>,
        search_path: Option<&OsStr>,
        application_folders: &[PathBuf],
    ) -> Result<Editors, UnknownEditor> {
        if let Some(name) = preferred
            && !EDITORS.contains(&name)
        {
            return Err(UnknownEditor { name: name.to_owned() });
        }
        let known: &'static [Known; 5] = &KNOWN;
        let found: Vec<Editor> = known
            .iter()
            .filter_map(|known| {
                let program = which::which_in(known.name, search_path, Path::new("."))
                    .ok()
                    .or_else(|| bundled_program(known, application_folders))?;
                Some(Editor { known, program })
            })
            .collect();
        let preferred = preferred.and_then(|name| found.iter().position(|editor| editor.name() == name));
        let default = preferred.or(if found.is_empty() { None } else { Some(0) });
        Ok(Editors { found, default, folders: application_folders.to_vec() })
    }

    /// No editor: a preview with these editors opens no place.
    pub fn none() -> Editors {
        Editors { found: Vec::new(), default: None, folders: Vec::new() }
    }

    /// The editors found, in the order of [`EDITORS`].
    pub fn iter(&self) -> std::slice::Iter<'_, Editor> {
        self.found.iter()
    }

    /// Whether no editor was found.
    pub fn is_empty(&self) -> bool {
        self.found.is_empty()
    }

    /// The editor found whose name is `name`, or `None` when `name` is not the name of an
    /// editor found.
    pub fn get(&self, name: &str) -> Option<&Editor> {
        self.found.iter().find(|editor| editor.name() == name)
    }

    /// The default: the editor a place is opened in when no other is asked for ([`Editors::find`]
    /// says which); `None` when no editor was found.
    pub fn default(&self) -> Option<&Editor> {
        self.default.map(|at| &self.found[at])
    }

    /// The message that says that no editor was found and where the editors were looked for:
    /// in the folders of `PATH` and, when [`Editors::find`] was given application folders, as
    /// an application in those folders.
    pub fn none_found(&self) -> String {
        let mut message = format!(
            "no editor was found for the places clicked on the page: none of the editors `{}` is in a folder of `PATH`",
            EDITORS.join("`, `")
        );
        if !self.folders.is_empty() {
            let folders: Vec<String> = self.folders.iter().map(|folder| format!("`{}`", folder.display())).collect();
            message.push_str(&format!(" or installed as an application in {}", folders.join(" or ")));
        }
        message
    }
}

/// The program of the editor `known` inside an application bundle that installs it, in
/// one of the folders `folders`: the first of its paths, each looked for in every folder in
/// turn, that is an executable file.
fn bundled_program(known: &Known, folders: &[PathBuf]) -> Option<PathBuf> {
    known
        .bundled
        .iter()
        .flat_map(|path| folders.iter().map(move |folder| folder.join(path)))
        .find(|candidate| executable(candidate))
}

/// Whether `path` is a file that can be executed: on Unix, a file with a permission to
/// execute it; elsewhere, any file.
fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    let permitted = std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0;
    #[cfg(not(unix))]
    let permitted = true;
    metadata.is_file() && permitted
}

/// The error of [`Editor::open`] when the editor's program could not be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorError {
    /// The editor's name, one of [`EDITORS`].
    pub name: &'static str,
    /// What the system said.
    pub detail: String,
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the editor `{}` cannot be started: {}", self.name, self.detail)
    }
}

impl std::error::Error for EditorError {}

impl Editor {
    /// The editor's name, the name of its program: one of [`EDITORS`].
    pub fn name(&self) -> &'static str {
        self.known.name
    }

    /// The editor's name as the preview's page shows it in its menu of editors: `VS Code`,
    /// `Zed`, `Emacs`, `Sublime Text` or `gVim`.
    pub fn label(&self) -> &'static str {
        self.known.label
    }

    /// The path of the program found for the editor.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Returns the command-line arguments that open `file` at the line `line` and the
    /// column `column` in this editor, both counted from 1, the column in characters:
    /// `--goto FILE:LINE:COLUMN` for `code`, `FILE:LINE:COLUMN` for `zed` and `subl`,
    /// `-n +LINE:COLUMN FILE` for `emacsclient` (in the frame already open), and
    /// `+LINE -c "normal! COLUMN|" FILE` for `gvim`.
    pub fn arguments(&self, file: &Path, line: usize, column: usize) -> Vec<OsString> {
        (self.known.arguments)(file, line, column)
    }

    /// Starts the editor on `file` at the line `line` and the column `column`, both counted
    /// from 1 ([`Editor::arguments`]), and returns as soon as it has started. The editor
    /// runs independently of this program, and ending this program does not end it; on
    /// Unix it is placed in a process group of its own, so that Ctrl+C in this program's
    /// terminal does not reach it. Its standard error
    /// is captured, so that, when it ends with failure, its message is known:
    /// [`Started::wait`] waits for its end.
    ///
    /// # Errors
    ///
    /// Returns an [`EditorError`] when the program cannot be started.
    pub fn open(&self, file: &Path, line: usize, column: usize) -> Result<Started, EditorError> {
        let mut command = Command::new(&self.program);
        command.args(self.arguments(file, line, column)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        // Apart from this program's terminal, so that its Ctrl+C does not reach the editor.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let child = command.spawn().map_err(|error| EditorError { name: self.name(), detail: error.to_string() })?;
        Ok(Started { name: self.name(), child })
    }
}

/// An editor started by [`Editor::open`], running independently of this program.
#[derive(Debug)]
pub struct Started {
    name: &'static str,
    child: Child,
}

impl Started {
    /// The editor's name, one of [`EDITORS`].
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Waits for the editor to end, and returns `Ok` when it ended with success. An editor
    /// that hands the file to an instance already open ends at once; one that stays open
    /// ends when the author closes it, so a caller waits on a thread of its own.
    ///
    /// # Errors
    ///
    /// Returns an [`EditorFailure`] when the editor ends with failure, with what it wrote
    /// on its standard error, or when its exit status cannot be read.
    pub fn wait(self) -> Result<(), EditorFailure> {
        let name = self.name;
        let output = self
            .child
            .wait_with_output()
            .map_err(|error| EditorFailure { name, end: format!("an exit status that cannot be read ({error})"), said: String::new() })?;
        if output.status.success() {
            return Ok(());
        }
        let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(EditorFailure { name, end: output.status.to_string(), said })
    }
}

/// An editor that was started and then ended with failure ([`Started::wait`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFailure {
    /// The editor's name, one of [`EDITORS`].
    pub name: &'static str,
    /// How it ended, in the system's words: its exit status or the signal that ended it;
    /// or, when its exit status could not be read,
    /// `an exit status that cannot be read (…)` with the system's reason.
    pub end: String,
    /// What it wrote on its standard error, without leading and trailing whitespace; empty
    /// when it wrote nothing.
    pub said: String,
}

impl fmt::Display for EditorFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the editor `{}` failed with {}", self.name, self.end)?;
        if !self.said.is_empty() {
            write!(f, ": {}", self.said)?;
        }
        Ok(())
    }
}

impl std::error::Error for EditorFailure {}

/// The sources of one build, by name, which turn a place written in the page (a source's
/// name and a byte offset into it) into the line and the column an editor counts: see the
/// [module documentation](self).
///
/// Made from a run with [`Places::of_run`]; [`Places::line_column`] does the conversion.
#[derive(Debug, Clone, Default)]
pub struct Places {
    sources: BTreeMap<String, Arc<FlmSource>>,
}

impl Places {
    /// Collects the sources of the document that `run` processed, each under its name
    /// (its canonical path): first the source of each tree of the document (the primary
    /// file, and every file brought in with a heading of its own); then, for a name that no
    /// tree has, the source of the first node that comes from it, in document order (a file
    /// brought in without a heading).
    ///
    /// A node that the library made (the expansion of a macro, text that a feature built
    /// during lowering) counts for the source it was made from, which is the source the
    /// HTML backend writes into the page for it.
    pub fn of_run(run: &Run) -> Places {
        // See decision rationale at [§flm-dd-dr:host-watch-gestures].
        let content = run.doc().content();
        let mut sources = BTreeMap::new();
        for (_, fragment) in content.trees() {
            if let Some(name) = fragment.source().origin().canonical_path() {
                sources.entry(name.to_owned()).or_insert_with(|| Arc::clone(fragment.source()));
            }
        }
        for (_, node) in content.walk_nodes() {
            if let Some(named) = named_source(node.span())
                && let Some(name) = named.origin().canonical_path()
                && !sources.contains_key(name)
            {
                sources.insert(name.to_owned(), named);
            }
        }
        Places { sources }
    }

    /// Whether a source has the name `name`.
    pub fn has(&self, name: &str) -> bool {
        self.sources.contains_key(name)
    }

    /// Returns the line and the column, both counted from 1, of the byte offset `offset` in
    /// the source named `name`: the line as the file numbers it (the front matter's lines
    /// counted), the column in characters from the start of the line. An offset inside a
    /// character counts as that character's offset. Sources of any length are handled.
    /// Returns `None` when no source has that name, or when the offset is past the end of
    /// the source.
    pub fn line_column(&self, name: &str, offset: usize) -> Option<(usize, usize)> {
        let source = self.sources.get(name)?;
        let content = source.content();
        if offset > content.len() {
            return None;
        }
        let offset = (0..=offset).rev().find(|at| content.is_char_boundary(*at)).unwrap_or(0);
        // Every source is indexed, whatever its length: the index's default limit, past
        // which it gives no line, is for displaying a diagnostic, where a byte position
        // stands in; an editor needs the line.
        let mut index = source.line_index();
        index.set_max_scan_len(content.len());
        let (line, range) = index.line_of(offset)?;
        let column = content[range.start..offset].chars().count() + source.column_number_offset();
        Some((line, column))
    }
}

/// The first source with a name on the chain of sources back from `span`: the span's own
/// source, or, for content the library made, the source it was made from.
fn named_source(span: &FlmSpan) -> Option<Arc<FlmSource>> {
    let mut span = span.clone();
    loop {
        if span.source().origin().canonical_path().is_some() {
            return Some(Arc::clone(span.source()));
        }
        let triggered = span.source().provenance().triggered_at()?.clone();
        span = triggered;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(name: &str) -> Editor {
        let known: &'static [Known; 5] = &KNOWN;
        let known = known.iter().find(|known| known.name == name).expect("a known editor");
        Editor { known, program: PathBuf::from(name) }
    }

    #[test]
    fn each_editor_is_given_the_line_and_the_column_its_own_way() {
        let file = Path::new("/doc/paper.flm");
        let words = |name| editor(name).arguments(file, 12, 5).into_iter().map(|word| word.into_string().unwrap()).collect::<Vec<_>>();
        assert_eq!(words("code"), ["--goto", "/doc/paper.flm:12:5"]);
        assert_eq!(words("zed"), ["/doc/paper.flm:12:5"]);
        assert_eq!(words("emacsclient"), ["-n", "+12:5", "/doc/paper.flm"]);
        assert_eq!(words("subl"), ["/doc/paper.flm:12:5"]);
        assert_eq!(words("gvim"), ["+12", "-c", "normal! 5|", "/doc/paper.flm"]);
    }

    #[test]
    fn each_editor_has_the_label_the_menu_shows() {
        let labels: Vec<&str> = EDITORS.iter().map(|name| editor(name).label()).collect();
        assert_eq!(labels, ["VS Code", "Zed", "Emacs", "Sublime Text", "gVim"]);
    }

    #[test]
    fn a_name_that_is_no_editor_is_refused() {
        for name in ["nano", "vim"] {
            assert_eq!(Editors::find(Some(name), None, &[]), Err(UnknownEditor { name: String::from(name) }));
        }
        let none = Editors::find(None, None, &[]).unwrap();
        assert!(none.is_empty());
        assert_eq!(none.default(), None);
        assert_eq!(none, Editors::none());
    }

    #[test]
    fn the_message_for_no_editor_says_where_the_editors_were_looked_for() {
        assert_eq!(
            Editors::none().none_found(),
            "no editor was found for the places clicked on the page: none of the editors `code`, `zed`, `emacsclient`, \
             `subl`, `gvim` is in a folder of `PATH`"
        );
        let folders = [PathBuf::from("/nowhere/Applications"), PathBuf::from("/nowhere/home/Applications")];
        assert_eq!(
            Editors::find(None, None, &folders).unwrap().none_found(),
            "no editor was found for the places clicked on the page: none of the editors `code`, `zed`, `emacsclient`, \
             `subl`, `gvim` is in a folder of `PATH` or installed as an application in `/nowhere/Applications` or \
             `/nowhere/home/Applications`"
        );
    }
}
