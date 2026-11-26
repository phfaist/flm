//! Opening a place of the document's files in an editor: finding the editors on this
//! machine, the editor of the author's own command, and turning a place written in the
//! preview's pages into a line and a column.
//!
//! An author reading the live preview wants to go from what is shown to where it is
//! written. A click on an element of the page with Ctrl or Cmd and Shift, or *Open in
//! editor* on a problem, opens the file in an editor at the line and the column of that
//! place. A place comes in one of two forms ([`Spot`]):
//!
//! - a byte offset, as the library writes it into each element (the HTML backend's render
//!   option `source_spans`) and into a problem in a file's content: the name of a source
//!   (the primary file as the command line gave it, or an included file) and a byte offset
//!   into the text the library parsed, which, for a file with a front matter, is the content
//!   below the front matter. The [document snapshot](super::snapshot) holds a build's
//!   sources, by name ([`Places`]), which turn the offset into the line and the column an
//!   editor counts: the line as the file numbers it, front matter included, and the column
//!   in characters, not in bytes;
//! - a line and a column, as a problem gives them outside a file's content (in its front
//!   matter): the line as the file numbers it, and the column in bytes, which is turned into
//!   characters from the file's line as it is on the disk.
//!
//! [`Editors::locate`] turns either form into the file's path, a line and a column, and
//! [`Editors::open_place`] opens it. Only a file the last build read may be opened.
//!
//! **The editors** the preview knows are those of [`EDITORS`], in this order: `code`
//! (Visual Studio Code), `cursor` (Cursor), `codium` (VSCodium), `zed` (Zed), `emacsclient`
//! (Emacs, in the frame already open), `subl` (Sublime Text), `gvim` (gVim, the graphical
//! Vim) and `mvim` (MacVim), each in the window already open, `vim` (Vim) and `nvim`
//! (Neovim). Each is looked for by the name of its program in the folders of a search path
//! and, when it is not there, inside the application bundle that installs it on macOS
//! ([`Editors::find`]).
//!
//! The preview starts an editor without a terminal. A Vim or a Neovim in a terminal is
//! reached as `emacsclient` reaches an Emacs: the program started hands the place to the
//! editor already running, the Vim that listens as the server [`VIM_SERVER`] or the Neovim
//! that listens at [`NEOVIM_ADDRESS`], and ends with failure when none listens.
//!
//! **The editor command** is an editor of the author's own, named [`CUSTOM`]: a command
//! given on the program's command line or in the viewer settings ([`super::viewer`]), split
//! into words without a shell, in which `{file}`, `{line}` and `{column}` are replaced
//! ([`EditorCommand`]).
//!
//! **The editor a place opens in** is chosen by [`Editors::choose`], from the name the
//! viewer settings or the program's command line give; the request to open a place names
//! no editor.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use super::snapshot::Places;
use crate::files::{BuildFiles, DiskFiles, FileError};

/// What the preview knows of one editor: the name of its program, the label a page shows,
/// the arguments that open a file at a line and a column, and where its program lies
/// inside the macOS application bundles that install the editor.
#[derive(Debug)]
struct Known {
    /// The name of its program, one of [`EDITORS`].
    name: &'static str,
    /// The editor's name as a page shows it.
    label: &'static str,
    /// The arguments that open a file at a line and a column ([`Editor::arguments`]).
    arguments: fn(&Path, usize, usize) -> Vec<OsString>,
    /// The paths of its program inside the application bundles that install the editor,
    /// relative to an application folder, in the order they are looked for.
    bundled: &'static [&'static str],
}

/// The editors the preview knows, in the order of [`EDITORS`].
// An editor installed as a macOS application often puts no program on `PATH` (VS Code's
// `code` and Zed's `zed` get there only through a command of the editor's own), so its
// program is also looked for at fixed paths inside its application bundles. Cursor and
// VSCodium are made from VS Code's source: their programs take its arguments, and lie in
// their bundles where VS Code's lies in its own.
const KNOWN: [Known; 10] = [
    Known {
        name: "code",
        label: "VS Code",
        arguments: goto,
        bundled: &["Visual Studio Code.app/Contents/Resources/app/bin/code"],
    },
    Known { name: "cursor", label: "Cursor", arguments: goto, bundled: &["Cursor.app/Contents/Resources/app/bin/cursor"] },
    Known { name: "codium", label: "VSCodium", arguments: goto, bundled: &["VSCodium.app/Contents/Resources/app/bin/codium"] },
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
    Known { name: "gvim", label: "gVim", arguments: remote_silent, bundled: &[] },
    Known { name: "mvim", label: "MacVim", arguments: remote_silent, bundled: &["MacVim.app/Contents/bin/mvim"] },
    Known {
        name: "vim",
        label: "Vim",
        arguments: |file, line, column| {
            vec![OsString::from("--servername"), OsString::from(VIM_SERVER), OsString::from("--remote-send"), remote_keys(file, line, column)]
        },
        bundled: &[],
    },
    Known {
        name: "nvim",
        label: "Neovim",
        arguments: |file, line, column| {
            vec![OsString::from("--server"), OsString::from(NEOVIM_ADDRESS), OsString::from("--remote-send"), remote_keys(file, line, column)]
        },
        bundled: &[],
    },
];

/// The name of the Vim server that the editor `vim` reaches: `VIM`, under which a Vim started
/// with `vim --servername VIM` listens, and MacVim by itself.
pub const VIM_SERVER: &str = "VIM";

/// The address of the Neovim that the editor `nvim` reaches, at which a Neovim started with
/// `nvim --listen ADDRESS` listens: the socket `/tmp/nvimsocket`, or, on Windows, the named
/// pipe `\\.\pipe\nvimsocket`.
#[cfg(not(windows))]
pub const NEOVIM_ADDRESS: &str = "/tmp/nvimsocket";

/// The address of the Neovim that the editor `nvim` reaches, at which a Neovim started with
/// `nvim --listen ADDRESS` listens: the socket `/tmp/nvimsocket`, or, on Windows, the named
/// pipe `\\.\pipe\nvimsocket`.
#[cfg(windows)]
pub const NEOVIM_ADDRESS: &str = r"\\.\pipe\nvimsocket";

/// The editors the preview can open a file in, by the name of their program, in the order
/// [`Editors::find`] lists the editors it finds: `code`, `cursor`, `codium`, `zed`,
/// `emacsclient`, `subl`, `gvim`, `mvim`, `vim` and `nvim`.
pub const EDITORS: [&str; KNOWN.len()] = {
    let mut names = [""; KNOWN.len()];
    let mut at = 0;
    while at < KNOWN.len() {
        names[at] = KNOWN[at].name;
        at += 1;
    }
    names
};

/// The name of the editor of the author's own command ([`EditorCommand`]), `custom`.
pub const CUSTOM: &str = "custom";

/// The label a page shows for the editor of the author's own command.
pub const CUSTOM_LABEL: &str = "Custom command";

/// `--goto FILE:LINE:COLUMN`, the arguments of VS Code and of the editors made from its
/// source.
fn goto(file: &Path, line: usize, column: usize) -> Vec<OsString> {
    vec![OsString::from("--goto"), located(file, line, column)]
}

/// `--remote-silent "+call setcursorcharpos(LINE, COLUMN)" FILE`, the arguments of gVim and
/// MacVim: the file opens in the graphical Vim already open, else in a new one.
fn remote_silent(file: &Path, line: usize, column: usize) -> Vec<OsString> {
    vec![OsString::from("--remote-silent"), OsString::from(format!("+call setcursorcharpos({line}, {column})")), file.into()]
}

/// The keys that make a Vim or a Neovim already running open `file` at the line `line` and
/// the column `column`, in characters, as `--remote-send` sends them: from any mode, back to
/// Normal mode, then the command
/// `:execute 'drop ' . fnameescape('FILE') | call setcursorcharpos(LINE, COLUMN) | redraw`.
/// The file is written inside a string of Vim's, `'` doubled, and the keys in Vim's notation
/// of keys, `<` written `<lt>`.
// `--remote-send`, and not `--remote-silent`: with no editor listening, `--remote-silent`
// edits the file in the program started, which has no terminal; and not `--remote-expr`,
// which then ends with success in Vim.
fn remote_keys(file: &Path, line: usize, column: usize) -> OsString {
    let quoted = file.to_string_lossy().replace('\'', "''");
    let command = format!("execute 'drop ' . fnameescape('{quoted}') | call setcursorcharpos({line}, {column}) | redraw");
    OsString::from(format!(r"<C-\><C-N>:{}<CR>", command.replace('<', "<lt>")))
}

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

/// The error of [`Editors::find`] when the editor's name is neither one of [`EDITORS`] nor
/// [`CUSTOM`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEditor {
    /// The name as given.
    pub name: String,
}

impl fmt::Display for UnknownEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown editor ‘{}’ (editors: ‘{}’, ‘{CUSTOM}’)", self.name, EDITORS.join("’, ‘"))
    }
}

impl std::error::Error for UnknownEditor {}

/// The command of an editor of the author's own: words, the first of which is the program,
/// in which `{file}`, `{line}` and `{column}` stand for the file, the line and the column of
/// the place to open.
///
/// [`EditorCommand::parse`] splits a command into words as a person types it, without a
/// shell: words are separated by spaces, and a part between single or double quotation marks
/// belongs to the word it is in, spaces included, without the marks. No other character is
/// special: a backslash is kept as it is, as in a path of Windows, and nothing is expanded.
/// [`EditorCommand::arguments`] replaces `{file}`, `{line}` and `{column}` in each word,
/// wherever they are in it (`{file}:{line}` makes one word); when no word holds `{file}`,
/// the file is given as one more word at the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    /// The command as written.
    text: String,
    /// Its words, the program first.
    words: Vec<String>,
}

/// The error of [`EditorCommand::parse`]: the command as written, and what is wrong in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    /// The command as written.
    pub text: String,
    /// The quotation mark left open, or `None` when the command has no word.
    pub unclosed: Option<char>,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unclosed {
            Some(mark) => write!(f, "Unclosed quotation mark {mark} in editor command ‘{}’", self.text),
            None => f.write_str("Missing program in the editor command"),
        }
    }
}

impl std::error::Error for CommandError {}

impl EditorCommand {
    /// Splits the command `text` into words: see [`EditorCommand`].
    ///
    /// # Errors
    ///
    /// A [`CommandError`] when a quotation mark is left open or the command has no word.
    pub fn parse(text: &str) -> Result<EditorCommand, CommandError> {
        let mut words = Vec::new();
        let mut word: Option<String> = None;
        let mut quote: Option<char> = None;
        for character in text.chars() {
            match (quote, character) {
                (Some(mark), _) if character == mark => quote = None,
                (Some(_), _) => word.get_or_insert_with(String::new).push(character),
                (None, '\'' | '"') => {
                    quote = Some(character);
                    word.get_or_insert_with(String::new);
                }
                (None, _) if character.is_whitespace() => words.extend(word.take()),
                (None, _) => word.get_or_insert_with(String::new).push(character),
            }
        }
        if quote.is_some() {
            return Err(CommandError { text: text.to_owned(), unclosed: quote });
        }
        words.extend(word);
        if words.is_empty() {
            return Err(CommandError { text: text.to_owned(), unclosed: None });
        }
        Ok(EditorCommand { text: text.to_owned(), words })
    }

    /// The command as written.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The program: the first word, as written.
    pub fn program(&self) -> &str {
        &self.words[0]
    }

    /// The words that open `file`, a path in the system's form, at the line `line` and the
    /// column `column`, both counted from 1, the column in characters, the program first:
    /// each word with `{file}`, `{line}` and `{column}` replaced, and the file as the last
    /// word when no word holds `{file}`.
    pub fn arguments(&self, file: &Path, line: usize, column: usize) -> Vec<OsString> {
        let (line, column) = (line.to_string(), column.to_string());
        let mut named = false;
        let mut words: Vec<OsString> = self
            .words
            .iter()
            .map(|word| {
                named |= word.contains("{file}");
                let replaced = word.replace("{line}", &line).replace("{column}", &column);
                let mut parts = replaced.split("{file}");
                let mut joined = OsString::from(parts.next().unwrap_or_default());
                for part in parts {
                    joined.push(file.as_os_str());
                    joined.push(part);
                }
                joined
            })
            .collect();
        if !named {
            words.push(file.as_os_str().to_owned());
        }
        words
    }

    /// Starts the command on the file named `file` at the line `line` and the column
    /// `column`, as [`Editor::open`] starts an editor.
    ///
    /// # Errors
    ///
    /// An [`EditorError`] when the file names no file on this system, or when the program
    /// cannot be started.
    pub fn open(&self, file: &str, line: usize, column: usize) -> Result<Started, EditorError> {
        let file = system_path(file)?;
        let mut words = self.arguments(&file, line, column).into_iter();
        let program = words.next().unwrap_or_default();
        start(self.program(), &program, words)
    }
}

/// An editor as a page lists it ([`Editors::listed`]): its name, its label, and whether a
/// place can be opened in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed {
    /// The editor's name: one of [`EDITORS`], or [`CUSTOM`].
    pub name: &'static str,
    /// The editor's name as a page shows it.
    pub label: &'static str,
    /// Whether a page may choose it: the editor was found; always, for [`CUSTOM`].
    pub available: bool,
}

/// An editor chosen to open a place in ([`Editors::choose`]): one of the editors found, or
/// the editor command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen<'e> {
    /// One of the editors found.
    Editor(&'e Editor),
    /// The editor command ([`CUSTOM`]).
    Command(&'e EditorCommand),
}

impl Chosen<'_> {
    /// The editor's name: one of [`EDITORS`], or [`CUSTOM`].
    pub fn name(&self) -> &'static str {
        match self {
            Chosen::Editor(editor) => editor.name(),
            Chosen::Command(_) => CUSTOM,
        }
    }

    /// Starts the editor on the file named `file` at the line `line` and the column
    /// `column` ([`Editor::open`], [`EditorCommand::open`]).
    ///
    /// # Errors
    ///
    /// The [`EditorError`] of the editor's start.
    pub fn open(&self, file: &str, line: usize, column: usize) -> Result<Started, EditorError> {
        match self {
            Chosen::Editor(editor) => editor.open(file, line, column),
            Chosen::Command(command) => command.open(file, line, column),
        }
    }
}

/// The editors of [`EDITORS`] found on this machine, in that order, and what the program's
/// command line asks for: the editor a place opens in, and the editor command.
///
/// Made with [`Editors::find`], or [`Editors::none`] for no editor at all. The preview's
/// server ([`Server::start`](super::server::Server::start)) lists every editor to its pages,
/// those found and the editor command available ([`Editors::listed`]), and opens each place
/// in the editor that [`Editors::choose`] chooses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editors {
    /// The editors found, in the order of [`EDITORS`].
    found: Vec<Editor>,
    /// The name of the editor the command line asks for.
    preferred: Option<String>,
    /// The editor command the command line gives.
    command: Option<EditorCommand>,
    /// The application folders the editors were looked for in.
    folders: Vec<PathBuf>,
}

impl Editors {
    /// Looks for every editor of [`EDITORS`] on this machine, and returns those found, in
    /// the order of [`EDITORS`], with `preferred`, the name of the editor that the program's
    /// command line asks for ([`Editors::preferred`]).
    ///
    /// An editor's program is looked for by its name in the folders of the search path
    /// `search_path` (a list of folders in the form of the variable `PATH`; `None` for an
    /// empty search path). When it is not there, it is looked for inside the macOS
    /// application bundle that installs the editor, in each folder of `application_folders`
    /// in turn (on macOS, `/Applications` and the `Applications` folder of the home folder):
    /// `Visual Studio Code.app/Contents/Resources/app/bin/code`,
    /// `Cursor.app/Contents/Resources/app/bin/cursor`,
    /// `VSCodium.app/Contents/Resources/app/bin/codium`, `Zed.app/Contents/MacOS/cli`
    /// (then, in each folder again, `Zed Preview.app/Contents/MacOS/cli`),
    /// `Emacs.app/Contents/MacOS/bin/emacsclient`,
    /// `Sublime Text.app/Contents/SharedSupport/bin/subl` and `MacVim.app/Contents/bin/mvim`;
    /// `gvim`, `vim` and `nvim` are looked for on the search path only. A program counts only
    /// when it is an executable file.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownEditor`] when `preferred` is neither one of [`EDITORS`] nor
    /// [`CUSTOM`].
    pub fn find(
        preferred: Option<&str>,
        search_path: Option<&OsStr>,
        application_folders: &[PathBuf],
    ) -> Result<Editors, UnknownEditor> {
        if let Some(name) = preferred
            && !EDITORS.contains(&name)
            && name != CUSTOM
        {
            return Err(UnknownEditor { name: name.to_owned() });
        }
        let known: &'static [Known] = &KNOWN;
        let found: Vec<Editor> = known
            .iter()
            .filter_map(|known| {
                let program = which::which_in(known.name, search_path, Path::new("."))
                    .ok()
                    .or_else(|| bundled_program(known, application_folders))?;
                Some(Editor { known, program })
            })
            .collect();
        Ok(Editors { found, preferred: preferred.map(str::to_owned), command: None, folders: application_folders.to_vec() })
    }

    /// No editor: a preview with these editors and no editor command opens no place.
    pub fn none() -> Editors {
        Editors { found: Vec::new(), preferred: None, command: None, folders: Vec::new() }
    }

    /// These editors, with `command`, the editor command that the program's command line
    /// gives: it counts over the one of the viewer settings.
    pub fn with_command(self, command: Option<EditorCommand>) -> Editors {
        Editors { command, ..self }
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

    /// The name of the editor that the program's command line asks for: it counts over the
    /// one of the viewer settings, as the server reads them at start.
    pub fn preferred(&self) -> Option<&str> {
        self.preferred.as_deref()
    }

    /// The editor command that the program's command line gives.
    pub fn command(&self) -> Option<&EditorCommand> {
        self.command.as_ref()
    }

    /// Every editor a page lists, whether a place can be opened in it or not: the editors of
    /// [`EDITORS`], in their order, each available when it was found, then [`CUSTOM`], always
    /// available, since choosing it on a page writes an editor command when there is none
    /// ([`ViewerFile::change`](super::viewer::ViewerFile::change)).
    pub fn listed(&self) -> Vec<Listed> {
        let known: &'static [Known] = &KNOWN;
        let mut listed: Vec<Listed> =
            known.iter().map(|known| Listed { name: known.name, label: known.label, available: self.get(known.name).is_some() }).collect();
        listed.push(Listed { name: CUSTOM, label: CUSTOM_LABEL, available: true });
        listed
    }

    /// The editor a place opens in, `asked` being the names of the editors asked for, in
    /// order (by the program's command line, [`Editors::preferred`], then by the viewer
    /// settings), and `settings_command` the editor command of the viewer settings, over
    /// which the command line's counts. The first editor named that is available is chosen:
    /// an editor found, or [`CUSTOM`] when there is an editor command. Otherwise the editor
    /// command, when there is one, else the first editor found; `None` when there is neither.
    pub fn choose<'e>(&'e self, asked: &[&str], settings_command: Option<&'e EditorCommand>) -> Option<Chosen<'e>> {
        let command = self.command.as_ref().or(settings_command);
        let named = asked.iter().find_map(|&name| match name {
            CUSTOM => command.map(Chosen::Command),
            name => self.get(name).map(Chosen::Editor),
        });
        named.or(command.map(Chosen::Command)).or(self.found.first().map(Chosen::Editor))
    }

    /// The message that says that no editor was found and where the editors were looked for:
    /// in the folders of `PATH` and, when [`Editors::find`] was given application folders, as
    /// an application in those folders.
    pub fn none_found(&self) -> String {
        let mut message = format!("Missing editor: none of ‘{}’ found in the folders of ‘PATH’", EDITORS.join("’, ‘"));
        if !self.folders.is_empty() {
            let folders: Vec<String> = self.folders.iter().map(|folder| format!("‘{}’", folder.display())).collect();
            message.push_str(&format!(" or as an application in {}", folders.join(" or ")));
        }
        message
    }

    /// Turns the place `spot` of the source named `file` into the file's line and column,
    /// both counted from 1, the column in characters ([`Spot`] says how each form is
    /// counted).
    ///
    /// Only a file of the last build may be opened: a file that `read` lists (the files the
    /// build read, each resolved as [`BuildFiles`] resolves it), and, for a byte offset, a
    /// source that `places` knows (the build's sources; `None` before the first build).
    ///
    /// # Errors
    ///
    /// Returns a [`Refusal`] that says why the place is not one to open.
    pub fn locate(places: Option<&Places>, read: &[String], file: &str, spot: Spot) -> Result<(usize, usize), Refusal> {
        // The list of files read names each file resolved, as `BuildFiles` reads it: the
        // primary file's name is the path as it was given.
        let resolved = BuildFiles::resolve("", file);
        if !read.contains(&resolved) {
            return Err(Refusal::NotRead { file: file.to_owned() });
        }
        match spot {
            Spot::Offset(start) => {
                let Some(places) = places.filter(|places| places.has(file)) else {
                    return Err(Refusal::NotRead { file: file.to_owned() });
                };
                places.line_column(file, start).ok_or_else(|| Refusal::PastEnd { file: file.to_owned(), start })
            }
            Spot::LineColumn { line, column } => Ok((line, characters(file, line, column))),
        }
    }

    /// Opens the place `spot` of the source named `file` ([`Editors::locate`]) in the editor
    /// `chosen` ([`Editors::choose`]), and returns the editor started and the line and the
    /// column it was given.
    ///
    /// # Errors
    ///
    /// Returns a [`Refusal`] that says why nothing was opened: the refusal of
    /// [`Editors::locate`], or, once the place is located, [`Refusal::NoEditor`] when
    /// `chosen` is `None` and [`Refusal::Editor`] when the editor cannot be started; both of
    /// these give the place.
    pub fn open_place(
        &self,
        places: Option<&Places>,
        read: &[String],
        file: &str,
        spot: Spot,
        chosen: Option<Chosen<'_>>,
    ) -> Result<OpenedPlace, Refusal> {
        let (line, column) = Editors::locate(places, read, file, spot)?;
        let path = system_path(file).map_err(Refusal::Editor)?;
        let place = Box::new(Located { file: file.to_owned(), path, line, column });
        let Some(chosen) = chosen else {
            return Err(Refusal::NoEditor { place, reason: self.none_found() });
        };
        match chosen.open(file, line, column) {
            Ok(started) => Ok(OpenedPlace { started, line, column }),
            Err(error) => Err(Refusal::NotStarted { place, error }),
        }
    }
}

/// A place in a file, in one of the two forms a page gives ([module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// A byte offset into the source, as the library parsed it (below the front matter).
    Offset(usize),
    /// A line, counted from 1 at the top of the file, and a column, counted from 1 in bytes
    /// from the start of the line.
    LineColumn {
        /// The line.
        line: usize,
        /// The column, in bytes.
        column: usize,
    },
}

/// A place located in a file ([`Refusal::NoEditor`], [`Refusal::NotStarted`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The file, as the build named it.
    pub file: String,
    /// The file's absolute path in the system's form, as an editor is given it.
    pub path: PathBuf,
    /// The line, counted from 1.
    pub line: usize,
    /// The column, counted from 1, in characters.
    pub column: usize,
}

impl fmt::Display for Located {
    /// `PATH:LINE:COLUMN`, the path absolute and in the system's form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.path.display(), self.line, self.column)
    }
}

/// A place opened by [`Editors::open_place`]: the editor started, and the line and the
/// column, both counted from 1, the column in characters.
#[derive(Debug)]
pub struct OpenedPlace {
    /// The editor started.
    pub started: Started,
    /// The line.
    pub line: usize,
    /// The column.
    pub column: usize,
}

/// The error of [`Editors::locate`] and [`Editors::open_place`]: why a place was not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The file is not a source of the document that its last build read.
    NotRead {
        /// The file, as asked.
        file: String,
    },
    /// The offset is past the end of the file as the last build read it.
    PastEnd {
        /// The file, as asked.
        file: String,
        /// The byte offset, as asked.
        start: usize,
    },
    /// The file names no file on this system.
    Editor(EditorError),
    /// There is no editor to open the place in: neither an editor found nor an editor
    /// command.
    NoEditor {
        /// The place.
        place: Box<Located>,
        /// The message of [`Editors::none_found`].
        reason: String,
    },
    /// The editor chosen could not be started.
    NotStarted {
        /// The place.
        place: Box<Located>,
        /// Why.
        error: EditorError,
    },
}

impl Refusal {
    /// The place, for a refusal that comes once it is located ([`Refusal::NoEditor`],
    /// [`Refusal::NotStarted`]).
    pub fn place(&self) -> Option<&Located> {
        match self {
            Refusal::NoEditor { place, .. } | Refusal::NotStarted { place, .. } => Some(place),
            _ => None,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotRead { file } => write!(f, "Missing source file ‘{file}’ in the last build"),
            Refusal::PastEnd { file, start } => write!(f, "Byte offset {start} past the end of ‘{file}’"),
            Refusal::Editor(error) | Refusal::NotStarted { error, .. } => write!(f, "{error}"),
            Refusal::NoEditor { reason, .. } => f.write_str(reason),
        }
    }
}

impl std::error::Error for Refusal {}

/// The column `column`, counted in bytes, of the line `line` of the file named `file`,
/// counted in characters as the file's line is now on the disk; `column` itself when the
/// file or the line cannot be read.
fn characters(file: &str, line: usize, column: usize) -> usize {
    let text = DiskFiles::system(file).ok().and_then(|path| std::fs::read_to_string(path).ok());
    let Some(text) = text else {
        return column;
    };
    let Some(written) = line.checked_sub(1).and_then(|index| text.split('\n').nth(index)) else {
        return column;
    };
    let mut end = column.saturating_sub(1).min(written.len());
    while !written.is_char_boundary(end) {
        end -= 1;
    }
    written[..end].chars().count() + 1
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

/// The file named `file`, written as the host writes paths ([`crate::files`]), in its
/// system's form ([`to_system`](crate::files::system::to_system)), made absolute from the
/// current folder, so that an editor already open elsewhere finds the file.
fn system_path(file: &str) -> Result<PathBuf, EditorError> {
    let system = DiskFiles::system(file).map_err(EditorError::File)?;
    Ok(std::path::absolute(&system).unwrap_or(system))
}

/// Starts the program `program` with the arguments `arguments`, as the editor named `name`
/// ([`Editor::open`] says how).
fn start(name: &str, program: &OsStr, arguments: impl IntoIterator<Item = OsString>) -> Result<Started, EditorError> {
    let mut command = Command::new(program);
    command.args(arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    // Apart from this program's terminal, so that its Ctrl+C does not reach the editor.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let child = command.spawn().map_err(|error| EditorError::Start { name: name.to_owned(), detail: error.to_string() })?;
    Ok(Started { name: name.to_owned(), child })
}

/// The error of [`Editor::open`] and [`EditorCommand::open`]: the file to open names no file
/// on this system, or the editor's program could not be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    /// The file to open names no file on this system: a [`FileError`] of kind
    /// [`NotFound`](crate::files::FileErrorKind::NotFound), whose
    /// [`invalid`](FileError::invalid) says why.
    File(FileError),
    /// The editor's program could not be started.
    Start {
        /// The editor's name: one of [`EDITORS`], or the program of the editor command.
        name: String,
        /// What the system said.
        detail: String,
    },
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorError::File(error) => write!(f, "{error}"),
            EditorError::Start { name, detail } => write!(f, "Failed to start editor ‘{name}’: {detail}"),
        }
    }
}

impl std::error::Error for EditorError {}

impl Editor {
    /// The editor's name, the name of its program: one of [`EDITORS`].
    pub fn name(&self) -> &'static str {
        self.known.name
    }

    /// The editor's name as a page of the preview shows it: `VS Code`, `Cursor`, `VSCodium`,
    /// `Zed`, `Emacs`, `Sublime Text`, `gVim`, `MacVim`, `Vim` or `Neovim`.
    pub fn label(&self) -> &'static str {
        self.known.label
    }

    /// The path of the program found for the editor.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Returns the command-line arguments that open `file`, a path in the system's form, at
    /// the line `line` and the column `column` in this editor, both counted from 1, the
    /// column in characters:
    /// `--goto FILE:LINE:COLUMN` for `code`, `cursor` and `codium`,
    /// `FILE:LINE:COLUMN` for `zed` and `subl`,
    /// `-n +LINE:COLUMN FILE` for `emacsclient` (in the frame already open),
    /// `--remote-silent "+call setcursorcharpos(LINE, COLUMN)" FILE` for `gvim` and `mvim`
    /// (in the window already open, else in a new one), and, for `vim` and `nvim`,
    /// `--servername VIM` or `--server ADDRESS` ([`VIM_SERVER`], [`NEOVIM_ADDRESS`]) then
    /// `--remote-send KEYS`, the keys that make the editor listening open the file at the
    /// line and the column: from any mode, back to Normal mode, then
    /// `:execute 'drop ' . fnameescape('FILE') | call setcursorcharpos(LINE, COLUMN) | redraw`.
    pub fn arguments(&self, file: &Path, line: usize, column: usize) -> Vec<OsString> {
        (self.known.arguments)(file, line, column)
    }

    /// Starts the editor on the file named `file` at the line `line` and the column
    /// `column`, both counted from 1 ([`Editor::arguments`]), and returns as soon as it has
    /// started. `file` is written as the host writes paths ([`crate::files`]); the editor is
    /// given its system's form ([`to_system`](crate::files::system::to_system)), made
    /// absolute from the current folder, so that an editor already open elsewhere finds the
    /// file. The editor runs independently of this program, and ending this program does
    /// not end it; on Unix it is placed in a process group of its own, so that Ctrl+C in
    /// this program's terminal does not reach it. Its standard error is captured, so that,
    /// when it ends with failure, its message is known: [`Started::wait`] waits for its end.
    ///
    /// # Errors
    ///
    /// Returns an [`EditorError`] when the file names no file on this system, or when the
    /// program cannot be started.
    pub fn open(&self, file: &str, line: usize, column: usize) -> Result<Started, EditorError> {
        let file = system_path(file)?;
        start(self.name(), self.program.as_os_str(), self.arguments(&file, line, column))
    }
}

/// An editor started by [`Editor::open`] or [`EditorCommand::open`], running independently
/// of this program.
#[derive(Debug)]
pub struct Started {
    name: String,
    child: Child,
}

impl Started {
    /// The editor's name: one of [`EDITORS`], or the program of the editor command.
    pub fn name(&self) -> &str {
        &self.name
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
        let output = match self.child.wait_with_output() {
            Ok(output) => output,
            Err(error) => {
                return Err(EditorFailure { name, end: format!("unreadable exit status: {error}"), said: String::new() });
            }
        };
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
    /// The editor's name: one of [`EDITORS`], or the program of the editor command.
    pub name: String,
    /// How it ended, in the system's words: its exit status or the signal that ended it;
    /// or, when its exit status could not be read, `unreadable exit status: …` with the
    /// system's reason.
    pub end: String,
    /// What it wrote on its standard error, without leading and trailing whitespace; empty
    /// when it wrote nothing.
    pub said: String,
}

impl fmt::Display for EditorFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Editor ‘{}’ failed ({})", self.name, self.end)?;
        if !self.said.is_empty() {
            write!(f, ": {}", self.said)?;
        }
        Ok(())
    }
}

impl std::error::Error for EditorFailure {}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(name: &str) -> Editor {
        let known: &'static [Known] = &KNOWN;
        let known = known.iter().find(|known| known.name == name).expect("a known editor");
        Editor { known, program: PathBuf::from(name) }
    }

    fn words(arguments: Vec<OsString>) -> Vec<String> {
        arguments.into_iter().map(|word| word.into_string().unwrap()).collect()
    }

    #[test]
    fn each_editor_is_given_the_line_and_the_column_its_own_way() {
        let file = Path::new("/doc/paper.flm");
        let words = |name| words(editor(name).arguments(file, 12, 5));
        for name in ["code", "cursor", "codium"] {
            assert_eq!(words(name), ["--goto", "/doc/paper.flm:12:5"]);
        }
        assert_eq!(words("zed"), ["/doc/paper.flm:12:5"]);
        assert_eq!(words("emacsclient"), ["-n", "+12:5", "/doc/paper.flm"]);
        assert_eq!(words("subl"), ["/doc/paper.flm:12:5"]);
        for name in ["gvim", "mvim"] {
            assert_eq!(words(name), ["--remote-silent", "+call setcursorcharpos(12, 5)", "/doc/paper.flm"]);
        }
        // The keys that the Vim or the Neovim listening receives, the file inside a string of
        // Vim's (`'` doubled) and `<` in Vim's notation of keys.
        let keys = r"<C-\><C-N>:execute 'drop ' . fnameescape('/doc/it''s <new>.flm') | call setcursorcharpos(12, 5) | redraw<CR>";
        let keys = keys.replace("<new>", "<lt>new>");
        let file = Path::new("/doc/it's <new>.flm");
        let remote = |name| self::words(editor(name).arguments(file, 12, 5));
        assert_eq!(remote("vim"), ["--servername", "VIM", "--remote-send", &keys]);
        assert_eq!(remote("nvim"), ["--server", NEOVIM_ADDRESS, "--remote-send", &keys]);
    }

    #[test]
    fn each_editor_has_its_label() {
        let labels: Vec<&str> = EDITORS.iter().map(|name| editor(name).label()).collect();
        assert_eq!(labels, ["VS Code", "Cursor", "VSCodium", "Zed", "Emacs", "Sublime Text", "gVim", "MacVim", "Vim", "Neovim"]);
    }

    #[test]
    fn a_name_that_is_no_editor_is_refused() {
        for name in ["nano", "emacs"] {
            assert_eq!(Editors::find(Some(name), None, &[]), Err(UnknownEditor { name: String::from(name) }));
        }
        let none = Editors::find(None, None, &[]).unwrap();
        assert!(none.is_empty());
        assert_eq!(none.choose(&[], None), None);
        assert_eq!(none, Editors::none());
        assert!(Editors::find(Some(CUSTOM), None, &[]).is_ok());
    }

    #[test]
    fn the_message_for_no_editor_says_where_the_editors_were_looked_for() {
        // The application folders given get through.
        let folders = [PathBuf::from("/nowhere/Applications"), PathBuf::from("/nowhere/home/Applications")];
        let message = Editors::find(None, None, &folders).unwrap().none_found();
        for folder in &folders {
            assert!(message.contains(&folder.display().to_string()), "{message}");
        }
    }

    #[test]
    fn a_command_is_split_into_words_without_a_shell_and_each_word_is_filled_in() {
        let file = Path::new("/doc/my paper.flm");
        let filled = |text: &str| words(EditorCommand::parse(text).unwrap().arguments(file, 12, 5));
        assert_eq!(filled("ed --line {line} --column={column} {file}"), ["ed", "--line", "12", "--column=5", "/doc/my paper.flm"]);
        assert_eq!(filled("  myeditor   {file}:{line}:{column}  "), ["myeditor", "/doc/my paper.flm:12:5"]);
        // Quotation marks hold a word together and are removed; nothing else is special.
        assert_eq!(
            filled(r#""/Applications/My Editor.app/bin/ed" -c 'goto {line}' $HOME\x"{file}""#),
            ["/Applications/My Editor.app/bin/ed", "-c", "goto 12", r"$HOME\x/doc/my paper.flm"]
        );
        assert_eq!(filled("C:\\Tools\\ed.exe +{line}"), ["C:\\Tools\\ed.exe", "+12", "/doc/my paper.flm"]);
        assert_eq!(filled("ed ''"), ["ed", "", "/doc/my paper.flm"]);
        assert_eq!(EditorCommand::parse("ed {file}").unwrap().program(), "ed");
        assert_eq!(EditorCommand::parse("ed 'x").unwrap_err().unclosed, Some('\''));
        assert_eq!(EditorCommand::parse("   ").unwrap_err().unclosed, None);
    }

    #[test]
    fn a_line_and_a_column_in_bytes_are_located_in_characters_in_a_file_the_build_read() {
        let folder = std::env::temp_dir().join(format!("flm-editor-locate-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("paper.flm");
        std::fs::write(&path, "---\ntitle: Ça été\n---\nText.\n").unwrap();
        let file = crate::files::system::from_system(&path).unwrap();
        let read = [BuildFiles::resolve("", &file)];
        // Byte 12 of `title: Ça été` follows eleven bytes, ten characters.
        assert_eq!(Editors::locate(None, &read, &file, Spot::LineColumn { line: 2, column: 12 }), Ok((2, 11)));
        // A line past the end keeps its column.
        assert_eq!(Editors::locate(None, &read, &file, Spot::LineColumn { line: 9, column: 4 }), Ok((9, 4)));
        // A file the build did not read; an offset with no sources of the build.
        let refused = Editors::locate(None, &[], &file, Spot::LineColumn { line: 1, column: 1 });
        assert_eq!(refused, Err(Refusal::NotRead { file: file.clone() }));
        assert_eq!(Editors::locate(None, &read, &file, Spot::Offset(0)), Err(Refusal::NotRead { file }));
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
