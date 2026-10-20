//! The style file: a `.sty` file assembled for one document from the
//! requirements of its lowered trees, holding exactly the definitions that the
//! document's recomposed LaTeX needs to compile.
//!
//! A **style file** is the `.sty` file written beside the recomposed LaTeX
//! document. Instead of a preamble pasted into the document, every definition
//! the recomposed LaTeX relies on goes into one package file that the document
//! loads with `\usepackage`: the package requests ([`super::packages`]), the
//! declarations of the characters written as UTF-8 ([`super::escape`]), the
//! definition pieces the lowering rules required ([`super::preamble`]), the
//! definitions a feature writes from its configuration, and the user
//! definitions the recomposed LaTeX calls ([`super::names`]).
//!
//! A user customizes what the file defines without editing it. Every
//! definition piece and every definition a feature writes is guarded
//! ([`super::preamble`]), so a definition the user made before
//! loading the style file wins, and a `\renewcommand` or
//! `\RenewDocumentCommand` after loading replaces one. A user definition
//! written under its own prefix is guarded too; one written under the name the
//! document spells (the render option `bare_names`) is made unconditionally.
//! The comment line above each user definition gives the command that changes
//! it after the file is loaded.
//!
//! # The order of the file
//!
//! [`style_body`] writes the file's body from what the caller resolved, in
//! this order:
//!
//! 1. the LaTeX the document's author gives for the place before the packages
//!    ([`ExtraPreamble::before_packages`]);
//! 2. the package requests, one per package: the caller has merged every
//!    request for one package into one line per engine
//!    ([`merge_requests`](super::packages::merge_requests)), and a line is
//!    written once where it is the same under every engine and inside a test
//!    of the engine where it differs, with the package [`IFTEX`] requested
//!    first where such a test is written;
//! 3. the command that makes each engine's own font encoding the default,
//!    where the package block loads [`FONTENC`] with that encoding, inside a
//!    test of the engine of the same form as the block's;
//! 4. the author's LaTeX for the place after the packages
//!    ([`ExtraPreamble::after_packages`]);
//! 5. the declarations of the characters written as UTF-8, from the
//!    [`Declared`] record of the escaping;
//! 6. the definition pieces, in the order given;
//! 7. the definitions a feature writes from its configuration;
//! 8. the recomposed user definitions;
//! 9. the author's LaTeX for the end of the file
//!    ([`ExtraPreamble::at_end`]), followed by `\endinput`.
//!
//! The output is deterministic: the same inputs give the same bytes. Each
//! block begins with a comment line that gives its title, and each package request
//! and each definition piece follows a comment line that gives the
//! identifiers it was resolved under, so that a reader of the file can find
//! what asked for a line. A package request the document's author made
//! through the render options is written in the same form and carries an
//! identifier in the same way, but no requirement of the document is behind
//! it: the author asked for it directly.
//!
//! # The name of the file
//!
//! The file is named after its content: [`content_hash`] gives eight
//! hexadecimal digits of a digest of the body (everything after the
//! `\ProvidesPackage` line), so that the style files of two documents do not
//! clash in one directory and identical documents share one. The
//! `latex-recomposed` backend builds the body first ([`style_body`]), then
//! asks the host's naming service for the file's path, gives the package the
//! name of that path without `.sty`, and adds the header for that name
//! ([`with_header`]). The host's naming service prefixes the name with the
//! name of the document's file (`paper-flmrecomposed.1a2b3c4d.sty`); without
//! a naming service the package is `<stem>.<hash>`, after the render option
//! `style_stem` and the hash. The hash is computed before the name is known,
//! so it does not depend on the name. The backend's
//! [`stages`](mod@crate::stdcore::backends::latex_recomposed::stages) module
//! describes the rule in full, under "The style file's path".
//! [`assemble_style`] does both steps for a name the caller chose.

use std::collections::BTreeSet;

use untechxt::preamble::{Engine, EngineSet};

use crate::model::digest::Fnv1a64;

use super::escape::Declared;
use super::names::{LatexDefinitionMode, mode_of_twin_name};
use super::packages::{ENGINES, PackageRequest};
use super::preamble::LatexPreambleDef;

/// The package whose option list decides which font encoding a document's
/// text is set in, `fontenc`.
///
/// It is the only package this family requests whose option *order* has a
/// meaning: `fontenc` makes the last encoding of its option list the
/// document's default. A merged request has no meaningful last option, so
/// [`style_body`] writes the command that makes each engine's own encoding
/// the default ([`default_encoding_command`]) after the package block, per
/// engine, instead of relying on the position of an encoding in the option
/// list.
pub const FONTENC: &str = "fontenc";

/// The package whose tests of the engine the package block writes its
/// requests inside where they differ by engine, `iftex`. The block requests
/// it itself, first, where it writes such a test ([`style_body`]).
pub const IFTEX: &str = "iftex";

/// The font encoding the Unicode-to-LaTeX table's Latin spellings are written
/// for under pdfLaTeX, `T1`, and the one the style file makes the default
/// there.
pub const DEFAULT_ENCODING_8BIT: &str = "T1";

/// The font encoding of the Unicode fonts LuaLaTeX and XeLaTeX set text in,
/// `TU`, and the one the style file makes the default there: a document in
/// `T1` under those engines loses the characters typed into it directly.
pub const DEFAULT_ENCODING_UNICODE: &str = "TU";

/// The font encoding the style file makes the default under `engine`:
/// [`DEFAULT_ENCODING_UNICODE`] under the engines that use Unicode fonts
/// ([`EngineSet::UNICODE`]) and [`DEFAULT_ENCODING_8BIT`] under every other.
pub fn default_encoding(engine: Engine) -> &'static str {
    if EngineSet::UNICODE.contains(engine) {
        DEFAULT_ENCODING_UNICODE
    } else {
        DEFAULT_ENCODING_8BIT
    }
}

/// What the style file writes to make `encoding` the default font encoding,
/// regardless of the order in which the encodings were loaded:
/// `\renewcommand{\encodingdefault}{<encoding>}`.
///
/// `\encodingdefault` is the kernel's name for the encoding a font is
/// selected in when nothing asks for another, and `fontenc` sets it to the
/// last encoding of its own option list. Setting it decides the document's
/// text encoding: selecting a font in the preamble
/// (`\fontencoding{T1}\selectfont`) would not, because the default font is
/// selected again from `\encodingdefault` when the document begins.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::style::default_encoding_command;
///
/// assert_eq!(default_encoding_command("TU"), "\\renewcommand{\\encodingdefault}{TU}");
/// ```
pub fn default_encoding_command(encoding: &str) -> String {
    format!("\\renewcommand{{\\encodingdefault}}{{{encoding}}}")
}

/// One user definition on its way into a style file: the name it is defined
/// under, the name the document declared, and the LaTeX text.
///
/// A kept user definition is recomposed under a name of its own
/// ([`recomposed_name`](super::names::recomposed_name)), so the two names
/// usually differ; the style file's comment line above the definition gives
/// both, so that an author reading the file recognizes their own definition.
/// Where the document declared one name twice, once for text mode and once for
/// mathematics mode (per-mode definitions), the file holds two definitions
/// whose [`source_name`](Self::source_name) is the same authored name; the
/// comment line then also gives the parsing mode the definition is for, read
/// back from [`name`](Self::name) ([`mode_of_twin_name`]), so that the two
/// lines differ.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecomposedDefinition {
    /// The name defined, as a user writes it: `\flmUDhello` for a macro,
    /// `flmUEnote` for an environment. The style file holds one definition
    /// per name.
    pub name: String,
    /// The name the document declared, written the same way: `\hello`,
    /// `note`. Equal to [`name`](Self::name) for a definition written bare.
    pub source_name: String,
    /// The definition's LaTeX, as the emitter recomposed it.
    pub text: String,
    /// Whether the definition is a **robust** macro — one written through the
    /// document command interface, which the render option `robust_names` asks
    /// for — rather than the plain expandable macro a recomposed definition
    /// otherwise is. It decides one thing here: which command the comment line
    /// gives as the one that changes the definition after this file is loaded,
    /// `\RenewDocumentCommand` for a robust macro and `\renewcommand` for an
    /// expandable one. An environment is neither: it is always changed with
    /// `\RenewDocumentEnvironment`.
    pub robust: bool,
    /// The feature that wrote the definition from its own configuration, where
    /// the document declared none (`defterm` for the environment of a defined
    /// term): such a definition is guarded, and the style file writes it
    /// before the document's own definitions, its comment line giving the
    /// feature's name. `None` for a user definition.
    pub feature: Option<String>,
    /// Whether the definition is a feature's **declaration** — LaTeX that
    /// makes something other than a command definition, such as
    /// `\newtheorem`, written as `\ifcsname <name>\endcsname\else …\fi`
    /// ([`LatexDefinitionKind::Declaration`](super::LatexDefinitionKind::Declaration)).
    /// [`name`](Self::name) is then the control sequence the test checks, and
    /// the comment line above it says what the test does rather than which
    /// command changes the definition.
    pub declaration: bool,
}

impl RecomposedDefinition {
    /// A definition of `name`, declared in the document under `source_name`,
    /// whose LaTeX is `text`.
    pub fn new(
        name: impl Into<String>,
        source_name: impl Into<String>,
        text: impl Into<String>,
    ) -> RecomposedDefinition {
        RecomposedDefinition {
            name: name.into(),
            source_name: source_name.into(),
            text: text.into(),
            robust: false,
            feature: None,
            declaration: false,
        }
    }

    /// The same definition written by the feature `feature` from its own
    /// configuration ([`feature`](field@Self::feature)).
    pub fn of_feature(mut self, feature: impl Into<String>) -> RecomposedDefinition {
        self.feature = Some(feature.into());
        self
    }

    /// The same definition marked as a robust macro
    /// ([`robust`](field@Self::robust)).
    pub fn robust(mut self) -> RecomposedDefinition {
        self.robust = true;
        self
    }

    /// The same definition marked as a feature's declaration
    /// ([`declaration`](field@Self::declaration)).
    pub fn declaration(mut self) -> RecomposedDefinition {
        self.declaration = true;
        self
    }
}

/// `text` with every line break written as a space, so that it cannot end a
/// comment line it is written into.
fn one_line(text: &str) -> String {
    text.chars().map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch }).collect()
}

/// The LaTeX a document's author gives to be written into the style file
/// exactly as given, in the three places it can go: the render option
/// [`extra_preamble`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::extra_preamble)
/// of the `latex-recomposed` backend.
///
/// Each of the three texts is written **byte for byte**: nothing in it is
/// escaped, quoted, parsed or checked, and the only thing added is the line
/// break the next line needs where the text does not end in one. Each is
/// written under a comment line of its own, so that a reader of the file sees
/// where the library's own output stops. A text left empty writes nothing at
/// all — no comment line either.
///
/// The three differ only in where they are placed, and each place serves a
/// different need:
///
/// - [`before_packages`](Self::before_packages) is placed between the file's
///   header and the package block, which is where `\PassOptionsToPackage`
///   must be to reach a package the block requests, and where a package that
///   must be loaded before the others is loaded.
/// - [`after_packages`](Self::after_packages) is placed directly after the
///   package block and the default-encoding command, before everything the
///   library defines. A definition made there wins over the LaTeX family's
///   own, because every definition piece the family writes is guarded
///   (`\ProvideDocumentCommand` and the related commands); this is how a
///   LaTeX hook is replaced rather than patched.
/// - [`at_end`](Self::at_end) is placed at the bottom, after everything the
///   file holds and before `\endinput`, so it can use any of it: patch a
///   definition piece, redefine a LaTeX hook, define a command the
///   document's own mathematics uses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ExtraPreamble {
    /// Written after the file's header and before the package block. Empty by
    /// default.
    pub before_packages: String,
    /// Written after the package block and the default-encoding command, and
    /// before the character declarations, the definition pieces and the user
    /// definitions. Empty by default.
    pub after_packages: String,
    /// Written at the bottom of the file, after everything else and before
    /// `\endinput`. Empty by default.
    pub at_end: String,
}

impl ExtraPreamble {
    /// Whether all three texts are empty, so that nothing of the author's own
    /// is written into the style file.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::latex::style::ExtraPreamble;
    ///
    /// assert!(ExtraPreamble::default().is_empty());
    /// let mine = ExtraPreamble { at_end: String::from("\\relax"), ..Default::default() };
    /// assert!(!mine.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.before_packages.is_empty()
            && self.after_packages.is_empty()
            && self.at_end.is_empty()
    }
}

/// Writes one of the author's three texts under `heading`, unchanged,
/// with the one line break the next line needs where it does not end in one.
/// An empty text writes nothing.
fn write_author_piece(out: &mut String, heading: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str(heading);
    out.push_str(text);
    if !text.ends_with('\n') {
        out.push('\n');
    }
}

/// What one entry of the package block writes under each engine of
/// [`ENGINES`], in that order: its text, ending in a newline, or nothing.
type ByEngine = [Option<String>; 3];

/// The position of pdfLaTeX in a [`ByEngine`].
const PDFLATEX: usize = 0;
/// The position of LuaLaTeX in a [`ByEngine`].
const LUALATEX: usize = 1;
/// The position of XeLaTeX in a [`ByEngine`].
const XELATEX: usize = 2;

/// How the known engines divide over what one entry writes under each of
/// them, which decides the test of the engine the entry is written inside.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Split {
    /// The entry writes the same under every engine, and needs no test.
    Same,
    /// One test of the package `iftex` is enough: the engines for which the
    /// test is true share what the entry writes under the engine at the given
    /// position of a [`ByEngine`], and every other engine shares what it
    /// writes under pdfLaTeX.
    Test(&'static str, usize),
    /// The entry writes something different under each engine.
    Each,
}

impl Split {
    /// The split of one entry.
    fn of(entry: &ByEngine) -> Split {
        let [pdflatex, lualatex, xelatex] = entry;
        match (pdflatex == lualatex, lualatex == xelatex, pdflatex == xelatex) {
            (true, true, _) => Split::Same,
            (false, true, _) => Split::Test("\\iftutex", LUALATEX),
            (true, false, _) => Split::Test("\\ifxetex", XELATEX),
            (false, false, true) => Split::Test("\\ifluatex", LUALATEX),
            (false, false, false) => Split::Each,
        }
    }
}

/// Writes `entries` in order, each run of neighbors that split the engines
/// the same way ([`Split`]) inside one test of the engine, or with none where
/// they write the same under every engine.
fn write_by_engine(out: &mut String, entries: &[ByEngine]) {
    for run in entries.chunk_by(|a, b| Split::of(a) == Split::of(b)) {
        match Split::of(&run[0]) {
            Split::Same => write_branch(out, run, PDFLATEX),
            Split::Test(test, at) => write_conditional(out, run, &[(test, at)]),
            Split::Each => write_conditional(
                out,
                run,
                &[("\\ifluatex", LUALATEX), ("\\ifxetex", XELATEX)],
            ),
        }
    }
}

/// Writes the entries of `run` inside the tests of `tests`, each a test of
/// the package `iftex` and the position in a [`ByEngine`] of what to write
/// where the test is true. What the entries write under pdfLaTeX is written
/// where every test is false — where some entry writes anything under
/// pdfLaTeX — so that an engine other than the three the family
/// distinguishes ([`ENGINES`]) reads what pdfLaTeX reads.
fn write_conditional(out: &mut String, run: &[ByEngine], tests: &[(&str, usize)]) {
    for (number, (test, at)) in tests.iter().enumerate() {
        if number > 0 {
            out.push_str("\\else");
        }
        out.push_str(test);
        out.push('\n');
        write_branch(out, run, *at);
    }
    if run.iter().any(|entry| entry[PDFLATEX].is_some()) {
        out.push_str("\\else\n");
        write_branch(out, run, PDFLATEX);
    }
    for _ in tests {
        out.push_str("\\fi");
    }
    out.push('\n');
}

/// Writes what each entry of `run` writes under the engine at the position
/// `at` of a [`ByEngine`].
fn write_branch(out: &mut String, run: &[ByEngine], at: usize) {
    for text in run.iter().filter_map(|entry| entry[at].as_deref()) {
        out.push_str(text);
    }
}

/// The body of a style file: every block after the header, in the order the
/// [module documentation](self#the-order-of-the-file) lists, followed by
/// `\endinput`. The arguments are:
///
/// - `packages` are one entry per package, each holding the package's line
///   under each engine the family knows ([`PackageRequest::by_engine`]), in
///   the order given. The caller has resolved the requirement identifiers,
///   added the packages the pieces and the declared characters need
///   ([`LatexPreambleDef::requires`], [`Declared::packages`]), added the
///   packages the author asked for, and **merged every request for one
///   package into one line per engine**
///   ([`merge_requests`](super::packages::merge_requests)), since LaTeX loads
///   a package once and a second request with other options is a package
///   option clash. A package whose line is the same under every engine is
///   written once, as a `\RequirePackage` line
///   ([`PackageRequest::requirepackage_line`]) under a comment line that
///   lists every identifier behind it, separated by `, `. Where the lines
///   differ, they are written inside `\iftutex … \else … \fi` — or
///   `\ifluatex … \else\ifxetex … \else … \fi\fi` where all three differ —
///   each under the comment line that lists every identifier behind that
///   branch's line, and the package [`IFTEX`], which defines the tests, is
///   requested first. The last branch is pdfLaTeX's, and an engine other than
///   the three the family distinguishes ([`ENGINES`]) reads it too.
///
///   Where an engine's [`FONTENC`] line holds that engine's own encoding
///   ([`default_encoding`]), the command that makes it the default
///   ([`default_encoding_command`]) follows the block under a comment line of
///   its own, in a separate test of the engine of the same form: `fontenc`
///   makes the *last* encoding of its option list the document's default, and
///   a merged option list has no meaningful last element, so the file states
///   the default rather than arranging for it. Nothing is written for an
///   engine under which the block loads no `fontenc`, or loads one without
///   that engine's encoding: the style file states a default only for a
///   package it requests itself, and where it requests none the document's
///   default is the author's own to set.
/// - `declared` gives the characters to declare, each as
///   `\newunicodechar{<char>}{<spelling>}`, in character order.
/// - `pieces` are written in the order given, each one's text as it is, a
///   piece given twice written once.
/// - `definitions` are [`RecomposedDefinition`] values in the order given.
///   The comment line above each gives the definition's
///   [`name`](RecomposedDefinition::name), as a user writes it
///   (`\flmUDhello` for a macro, `flmUEnote` for an environment); where the
///   document declared it under another name
///   ([`source_name`](RecomposedDefinition::source_name)), that name too,
///   with any line break in it written as a space so that it stays one
///   comment line; and, where the definition is one of the two per-mode
///   definitions of a name the document declared once per mode, the parsing
///   mode it is for, so that the two lines differ. The comment line then
///   says how the definition is made (guarded, so that a definition of the
///   name made before this file is loaded wins, or unconditionally where the
///   definition is written under the name the document spells) and gives the
///   command that changes it after loading: `\renewcommand` for a macro,
///   `\RenewDocumentCommand` for a robust one
///   ([`robust`](field@RecomposedDefinition::robust)), and
///   `\RenewDocumentEnvironment` for an environment, which is a name written
///   without a backslash. A definition a feature wrote from its own
///   configuration ([`feature`](field@RecomposedDefinition::feature)) is
///   written first, in a block of its own, its comment line giving the
///   feature's name; it is always guarded. A feature's declaration
///   ([`declaration`](field@RecomposedDefinition::declaration)) is guarded by
///   the test `\ifcsname` it is written inside, and its comment line gives
///   the control sequence that the test checks.
///
/// - `extra_preamble` is the LaTeX the document's author gave to be written
///   as given ([`ExtraPreamble`]), in its three places: before the
///   package block, after the package block and the default-encoding command,
///   and at the bottom before `\endinput`. Each text is written **byte for
///   byte** under a comment line of its own — nothing in it is escaped,
///   quoted or checked — with the one line break the next line needs where
///   the text does not end in one. An empty text writes nothing at all.
///
/// A block with nothing in it is omitted entirely.
pub fn style_body(
    packages: &[PackageRequest],
    declared: &Declared,
    pieces: &[&LatexPreambleDef],
    definitions: &[RecomposedDefinition],
    extra_preamble: &ExtraPreamble,
) -> String {
    let mut out = String::new();

    // The author's first piece: everything the library writes is below it, so
    // this is where `\PassOptionsToPackage` reaches the requests below and
    // where a package that must load before the others is loaded.
    write_author_piece(
        &mut out,
        "% --- the author's own preamble: before the packages ---\n",
        &extra_preamble.before_packages,
    );

    // What each package writes under each engine: the comment line naming
    // the identifiers behind that engine's line, then the line itself;
    // nothing under an engine the package has no line under. Two lines that
    // differ only in their identifiers are two texts, and are written in two
    // branches: the comment line is part of what a branch says.
    let items: Vec<ByEngine> = packages
        .iter()
        .map(|request| {
            ENGINES.map(|engine| {
                let line = request.for_engine(engine)?;
                // An identifier a feature contributed is a dotted name and
                // holds no line break; one that came with an author's own
                // request is minted from the package name the author wrote,
                // so it can hold anything. A line break in it would end this
                // comment line and leave the rest as LaTeX, so line breaks
                // become spaces here, as they do in the comment line above a
                // user definition. The request line below carries the package
                // name as it was given — that part is the author's own LaTeX
                // and no byte of it is altered — and the options the merge
                // read out of the requests.
                let named: Vec<String> = line.ids.iter().map(|id| one_line(id.as_str())).collect();
                let requirepackage = request.requirepackage_line(engine)?;
                Some(format!("% {}\n{requirepackage}\n", named.join(", ")))
            })
        })
        .collect();
    // The one thing the block cannot say by its order: under each engine, the
    // command naming that engine's own encoding the default, where that
    // engine's `fontenc` line loads it. See the rustdoc of
    // `default_encoding_command` for why this command and not another.
    let commands: ByEngine = ENGINES.map(|engine| {
        let encoding = default_encoding(engine);
        packages
            .iter()
            .any(|request| {
                request.package == FONTENC
                    && request
                        .for_engine(engine)
                        .is_some_and(|line| line.options.iter().any(|option| option == encoding))
            })
            .then(|| format!("{}\n", default_encoding_command(encoding)))
    });
    if !items.is_empty() {
        out.push_str("% --- packages ---\n");
        if items.iter().chain([&commands]).any(|item| Split::of(item) != Split::Same) {
            out.push_str(
                "% Some lines below differ by TeX engine. They are written inside tests of the\n\
                 % engine, which the package iftex defines: \\iftutex is true under LuaLaTeX and\n\
                 % XeLaTeX, \\ifluatex under LuaLaTeX, and \\ifxetex under XeLaTeX. The last\n\
                 % branch of a test is for pdfLaTeX and any other engine.\n",
            );
            out.push_str(&format!("\\RequirePackage{{{IFTEX}}}\n"));
        }
        write_by_engine(&mut out, &items);
        if commands.iter().any(Option::is_some) {
            out.push_str("% --- the default font encoding ---\n");
            out.push_str(
                "% fontenc makes the last encoding of its option list the default, but the\n\
                 % options above may be merged from several requests, so their order carries\n\
                 % no meaning. The line below therefore sets the default for each engine whose\n\
                 % own encoding the request loads: T1 under pdfLaTeX, TU under LuaLaTeX and\n\
                 % XeLaTeX. A letter of another encoding switches to that encoding by itself.\n",
            );
            write_by_engine(&mut out, std::slice::from_ref(&commands));
        }
    }

    // The author's second piece: below the requests, above everything the
    // library defines. A definition made here wins over the family's own,
    // every piece below being guarded.
    write_author_piece(
        &mut out,
        "% --- the author's own preamble: after the packages ---\n",
        &extra_preamble.after_packages,
    );

    if !declared.chars().is_empty() {
        out.push_str("% --- unicode characters written as UTF-8 ---\n");
        for (ch, spelling) in declared.chars() {
            out.push_str("\\newunicodechar{");
            out.push(*ch);
            out.push_str("}{");
            out.push_str(spelling);
            out.push_str("}\n");
        }
    }

    let mut seen = BTreeSet::new();
    let pieces: Vec<&LatexPreambleDef> =
        pieces.iter().copied().filter(|def| seen.insert(def.id.clone())).collect();
    if !pieces.is_empty() {
        out.push_str("% --- commands and environments the library defines for the document ---\n");
        for def in pieces {
            out.push_str("% ");
            out.push_str(def.id.as_str());
            out.push('\n');
            out.push_str(&def.content);
            if !def.content.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    // A feature's definitions, written from its configuration, stand apart
    // from the document's own and before them, guarded like a piece.
    let (by_features, by_document): (Vec<&RecomposedDefinition>, Vec<&RecomposedDefinition>) =
        definitions.iter().partition(|definition| definition.feature.is_some());
    if !by_features.is_empty() {
        out.push_str("% --- definitions written from the configuration of FLM features ---\n");
        for definition in by_features {
            let RecomposedDefinition { name, text, robust, feature, declaration, .. } =
                definition;
            let renew = match (name.starts_with('\\'), robust) {
                (true, false) => "\\renewcommand",
                (true, true) => "\\RenewDocumentCommand",
                (false, _) => "\\RenewDocumentEnvironment",
            };
            out.push_str("% ");
            out.push_str(name);
            out.push_str(" (the ");
            out.push_str(feature.as_deref().unwrap_or_default());
            if *declaration {
                // A declaration defines no command a user redefines as a
                // whole; the comment says what its test does.
                out.push_str(" feature's, written from its configuration): declared unless \\");
                out.push_str(name);
                out.push_str(" is already defined, so a definition made earlier under that name is kept\n");
            } else {
                out.push_str(
                    " feature's, written from its configuration): defined unless the name is \
                     already taken, so a definition of it made before this file is loaded wins; ",
                );
                out.push_str(renew);
                out.push_str(" after loading this file changes it\n");
            }
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    if !by_document.is_empty() {
        out.push_str("% --- user definitions ---\n");
        for definition in by_document {
            let RecomposedDefinition { name, source_name, text, robust, .. } = definition;
            let is_macro = name.starts_with('\\');
            // What changes the definition after this file is loaded: an
            // environment is a document environment, a robust macro is a
            // document command, and an expandable macro is a plain macro.
            let renew = match (is_macro, robust) {
                (true, false) => "\\renewcommand",
                (true, true) => "\\RenewDocumentCommand",
                (false, _) => "\\RenewDocumentEnvironment",
            };
            // A definition written under a prefix of its own is guarded, so a
            // definition of that name the user made before this file is loaded
            // wins; one written under the name the document spelled is the
            // render option `bare_names` at work, and is made unconditionally,
            // since that name was listed exactly so that it replaces whatever
            // else is defined under it. The two are told apart by the names:
            // they are equal for a definition written bare and differ
            // otherwise.
            let bare = source_name == name;
            out.push_str("% ");
            out.push_str(name);
            if !bare {
                out.push_str(" (the document's ");
                // The one place the *document's own* text reaches the file
                // outside a definition body: a line break in it would end the
                // comment and leave the rest as LaTeX, so line breaks become
                // spaces. Every other character is harmless in a comment.
                // The package block above normalizes its comment line for the
                // same reason.
                out.push_str(&one_line(source_name));
                // A name the document declared once per parsing mode is two
                // definitions here, both carrying that one authored name, so
                // the authored name alone would give the two the same comment
                // line. The mode the twin is for is read back out of the name
                // the file defines, which is where the naming rule records it.
                match mode_of_twin_name(name.strip_prefix('\\').unwrap_or(name)) {
                    Some(LatexDefinitionMode::Text) => out.push_str(", in text"),
                    Some(LatexDefinitionMode::Mathematics) => out.push_str(", in mathematics"),
                    // `Single` is not a twin and `mode_of_twin_name` never
                    // answers it; the arm is here because the type allows it.
                    Some(LatexDefinitionMode::Single) | None => {}
                }
                out.push(')');
            }
            if bare {
                out.push_str(
                    ": defined unconditionally, under the name the document spells, as the \
                     render option bare_names asked; ",
                );
            } else {
                out.push_str(
                    ": defined unless the name is already taken, so a definition of it made \
                     before this file is loaded wins; ",
                );
            }
            out.push_str(renew);
            out.push_str(" after loading this file changes it\n");
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    // The author's last piece, after everything above and exactly as it was
    // given: it may use anything above it, and nothing about it is this
    // library's to second-guess.
    write_author_piece(
        &mut out,
        "% --- the author's own preamble: at the end ---\n",
        &extra_preamble.at_end,
    );

    out.push_str("\\endinput\n");
    out
}

/// The eight hexadecimal digits in the name of a style file, derived from its
/// content: the first eight digits of the FNV-1a digest ([`Fnv1a64`]) of
/// `body`, which is the file's content without its header (what
/// [`style_body`] returns).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::style::content_hash;
///
/// let hash = content_hash("\\endinput\n");
/// assert_eq!(hash.len(), 8);
/// assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
/// assert_eq!(hash, content_hash("\\endinput\n"));
/// ```
pub fn content_hash(body: &str) -> String {
    let mut hasher = Fnv1a64::new();
    hasher.write_bytes(body.as_bytes());
    format!("{:016x}", hasher.value())[..8].to_string()
}

/// The header of the style file named `name`, followed by `body`:
/// `\NeedsTeXFormat{LaTeX2e}` and `\ProvidesPackage{<name>}` on their own
/// lines, then the body. `name` is the package name without `.sty`, the
/// name a document loads with `\usepackage{<name>}`.
pub fn with_header(name: &str, body: &str) -> String {
    format!("\\NeedsTeXFormat{{LaTeX2e}}\n\\ProvidesPackage{{{name}}}\n{body}")
}

/// The whole style file named `name`: [`with_header`] over [`style_body`].
/// See [the module documentation](self) for what the parts are and how a
/// backend derives the file's name from its content.
pub fn assemble_style(
    name: &str,
    packages: &[PackageRequest],
    declared: &Declared,
    pieces: &[&LatexPreambleDef],
    definitions: &[RecomposedDefinition],
    extra_preamble: &ExtraPreamble,
) -> String {
    with_header(name, &style_body(packages, declared, pieces, definitions, extra_preamble))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::names::Name;
    use crate::stdcore::backends::latex::escape::{EscapeConfig, UnicodeMode, escape_into};
    use crate::stdcore::backends::latex::packages::{
        self, LatexPackageDef, merge_requests, standard_packages,
    };
    use crate::stdcore::backends::latex::preamble::{self, standard_pieces};

    /// The requests of `defs` merged as the caller merges them, for the
    /// tests below.
    fn merged(defs: &[&LatexPackageDef]) -> Vec<PackageRequest> {
        let (requests, conflicts) = merge_requests(defs.iter().copied());
        assert!(conflicts.is_empty(), "{conflicts:?}");
        requests
    }

    #[test]
    fn an_empty_requirement_set_gives_a_header_and_endinput() {
        let style = assemble_style("flmrecomposed.00000000", &[], &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert_eq!(
            style,
            "\\NeedsTeXFormat{LaTeX2e}\n\\ProvidesPackage{flmrecomposed.00000000}\n\\endinput\n"
        );
    }

    #[test]
    fn the_blocks_come_in_order_and_a_duplicate_is_written_once() {
        let roster = standard_packages();
        let amsmath = roster.iter().find(|d| d.id.as_str() == packages::AMSMATH).unwrap();
        let pieces = standard_pieces();
        let math = pieces.iter().find(|p| p.id.as_str() == preamble::MATH_ENVIRONMENT).unwrap();
        let mut declared = Declared::new();
        escape_into(&mut String::new(), "é", &EscapeConfig::new(UnicodeMode::Declare), &mut declared);
        let body = style_body(
            &merged(&[amsmath, amsmath]),
            &declared,
            &[math, math],
            &[
                RecomposedDefinition::new(
                    "\\flmUDhello",
                    "\\hello",
                    "\\providecommand\\flmUDhello{hi}",
                ),
                RecomposedDefinition::new("\\bare", "\\bare", "\\long\\def\\bare{hi}"),
                RecomposedDefinition::new(
                    "\\flmUDfirm",
                    "\\firm",
                    "\\ProvideDocumentCommand\\flmUDfirm{+m}{hi#1}",
                )
                .robust(),
            ],
            &ExtraPreamble::default(),
        );
        let package_at = body.find("% --- packages ---").unwrap();
        let chars_at = body.find("% --- unicode characters").unwrap();
        let pieces_at = body
            .find("% --- commands and environments the library defines for the document ---")
            .unwrap();
        let defs_at = body.find("% --- user definitions ---").unwrap();
        assert!(package_at < chars_at && chars_at < pieces_at && pieces_at < defs_at);
        assert_eq!(body.matches("\\RequirePackage{amsmath}").count(), 1);
        assert_eq!(body.matches("\\ProvideDocumentCommand\\flmMathEnvironment").count(), 1);
        assert!(body.contains("\\newunicodechar{é}{\\'e}\n"));
        // A definition written under its prefix is guarded and says so; one
        // written under the document's own name is made unconditionally and
        // its comment line names it once.
        assert!(body.contains(
            "% \\flmUDhello (the document's \\hello): defined unless the name is already \
             taken, so a definition of it made before this file is loaded wins; \
             \\renewcommand after loading this file changes \
             it\n\\providecommand\\flmUDhello{hi}\n"
        ));
        assert!(body.contains(
            "% \\bare: defined unconditionally, under the name the document spells, as the \
             render option bare_names asked; \\renewcommand after loading this file \
             changes it\n\\long\\def\\bare{hi}\n"
        ));
        // A robust macro is guarded in the same way and is changed after
        // loading with the document command interface it was made with.
        assert!(body.contains(
            "% \\flmUDfirm (the document's \\firm): defined unless the name is already \
             taken, so a definition of it made before this file is loaded wins; \
             \\RenewDocumentCommand after loading this file changes \
             it\n\\ProvideDocumentCommand\\flmUDfirm{+m}{hi#1}\n"
        ));
        assert!(body.ends_with("\\endinput\n"));
    }

    /// Every request for one package is one line, whatever identifiers and
    /// whatever options the requests carried, and the comment line above it
    /// names every identifier behind it.
    #[test]
    fn one_package_is_one_line_naming_every_identifier_behind_it() {
        let plain = LatexPackageDef::new("amsmath", "the author's");
        let under_another_id = LatexPackageDef::new("amsmath", "a feature's").id("flm.x.amsmath");
        let with_options = LatexPackageDef::new("amsmath", "the author's")
            .id("flm.y.amsmath")
            .options(["fleqn"]);
        let body = style_body(
            &merged(&[&plain, &under_another_id, &with_options]),
            &Declared::new(),
            &[],
            &[],
            &ExtraPreamble::default(),
        );
        assert_eq!(
            body,
            "% --- packages ---\n\
             % flm.latex.pkg.amsmath, flm.x.amsmath, flm.y.amsmath\n\
             \\RequirePackage[fleqn]{amsmath}\n\
             \\endinput\n"
        );
    }

    /// The comment lines the package block writes above a test of the engine
    /// and above the default-encoding command, for the tests below.
    const IFTEX_REQUEST: &str = "% Some lines below differ by TeX engine. They are written inside tests of the\n\
         % engine, which the package iftex defines: \\iftutex is true under LuaLaTeX and\n\
         % XeLaTeX, \\ifluatex under LuaLaTeX, and \\ifxetex under XeLaTeX. The last\n\
         % branch of a test is for pdfLaTeX and any other engine.\n\
         \\RequirePackage{iftex}\n";
    const ENCODING_HEADING: &str = "% --- the default font encoding ---\n\
         % fontenc makes the last encoding of its option list the default, but the\n\
         % options above may be merged from several requests, so their order carries\n\
         % no meaning. The line below therefore sets the default for each engine whose\n\
         % own encoding the request loads: T1 under pdfLaTeX, TU under LuaLaTeX and\n\
         % XeLaTeX. A letter of another encoding switches to that encoding by itself.\n";

    /// The roster's `fontenc` definitions are one line per engine, written
    /// inside a test of the engine, and the command naming each engine's own
    /// encoding the default follows the block, in a separate test of the
    /// engine of the same form — so that which encoding ends a merged list
    /// decides nothing. A block with no `fontenc` in it gets no such command,
    /// and one whose `fontenc` line lacks an engine's encoding gets none for
    /// that engine.
    #[test]
    fn the_default_encoding_command_follows_each_engine_s_own_fontenc_line() {
        let roster = standard_packages();
        let fontenc: Vec<&LatexPackageDef> = roster
            .iter()
            .filter(|def| def.package == FONTENC)
            .collect();
        assert_eq!(fontenc.len(), 13);
        let body = style_body(&merged(&fontenc), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert!(body.starts_with(&format!("% --- packages ---\n{IFTEX_REQUEST}")), "{body}");
        assert_eq!(body.matches("{fontenc}\n").count(), 2, "{body}");
        assert!(body.contains("\\RequirePackage[T2A,TU,X2,T2B,T2C,OT2,T2D]{fontenc}\n"), "{body}");
        assert!(body.contains("\\RequirePackage[T2A,T1,X2,T2B,T2C,OT2,T2D]{fontenc}\n"), "{body}");
        let commands = format!(
            "{ENCODING_HEADING}\\iftutex\n\\renewcommand{{\\encodingdefault}}{{TU}}\n\\else\n\
             \\renewcommand{{\\encodingdefault}}{{T1}}\n\\fi\n"
        );
        assert!(body.ends_with(&format!("{commands}\\endinput\n")), "{body}");
        // No fontenc: no test, no command.
        let amsmath = roster.iter().find(|d| d.id.as_str() == packages::AMSMATH).unwrap();
        let body = style_body(&merged(&[amsmath]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert!(!body.contains("iftex"), "{body}");
        assert!(!body.contains("\\if"), "{body}");
        assert!(!body.contains("\\encodingdefault"), "{body}");
        // A `fontenc` request with neither engine's own encoding names an
        // encoding the file never loaded, so no command is written.
        let other = LatexPackageDef::new(FONTENC, "the author's own").options(["T2A"]);
        let body = style_body(&merged(&[&other]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert!(!body.contains("\\encodingdefault"), "{body}");
        // An author's `T1` for every engine: one line for every engine, and
        // the command for pdfLaTeX alone, inside a test whose first branch is
        // empty.
        let t1 = LatexPackageDef::new(FONTENC, "the author's own").options(["T1"]);
        let body = style_body(&merged(&[&t1]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert_eq!(
            body,
            format!(
                "% --- packages ---\n{IFTEX_REQUEST}% flm.latex.pkg.fontenc\n\
                 \\RequirePackage[T1]{{fontenc}}\n{ENCODING_HEADING}\\iftutex\n\\else\n\
                 \\renewcommand{{\\encodingdefault}}{{T1}}\n\\fi\n\\endinput\n"
            )
        );
        // An author's `T1,TU` for every engine: the command for each.
        let both = LatexPackageDef::new(FONTENC, "the author's own").options(["T1,TU"]);
        let body = style_body(&merged(&[&both]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert!(body.ends_with(&format!("{commands}\\endinput\n")), "{body}");
    }

    /// A package whose line differs by engine is written inside `\iftutex`,
    /// the Unicode engines' line first and pdfLaTeX's after `\else`, each
    /// under the identifiers behind it; `iftex` is requested first, and the
    /// default-encoding command follows in the same shape.
    #[test]
    fn a_request_that_differs_by_engine_is_written_inside_an_iftex_test() {
        let roster = standard_packages();
        let by_id = |id: &Name| -> Vec<&LatexPackageDef> {
            roster.iter().filter(|def| def.id == *id).collect()
        };
        let mut defs = by_id(&Name::from(packages::NEWUNICODECHAR));
        defs.extend(by_id(&packages::chunk_package_id("fontenc-t1")));
        defs.extend(by_id(&packages::chunk_package_id("fontenc-t2a")));
        assert_eq!(defs.len(), 4);
        let body = style_body(&merged(&defs), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert_eq!(
            body,
            format!(
                "% --- packages ---\n{IFTEX_REQUEST}\
                 % flm.latex.pkg.newunicodechar\n\
                 \\RequirePackage{{newunicodechar}}\n\
                 \\iftutex\n\
                 % flm.latex.pkg.untechxt.fontenc-t2a\n\
                 \\RequirePackage[T2A,TU]{{fontenc}}\n\
                 \\else\n\
                 % flm.latex.pkg.untechxt.fontenc-t1, flm.latex.pkg.untechxt.fontenc-t2a\n\
                 \\RequirePackage[T1,T2A]{{fontenc}}\n\
                 \\fi\n\
                 {ENCODING_HEADING}\
                 \\iftutex\n\
                 \\renewcommand{{\\encodingdefault}}{{TU}}\n\
                 \\else\n\
                 \\renewcommand{{\\encodingdefault}}{{T1}}\n\
                 \\fi\n\
                 \\endinput\n"
            )
        );
    }

    /// Where the line is different under each of the three engines, the
    /// tests nest: `\ifluatex`, then `\ifxetex` after its `\else`, then
    /// pdfLaTeX's line. Where a package has no line under pdfLaTeX, the test
    /// has no `\else` branch.
    #[test]
    fn the_three_way_case_nests_ifluatex_and_ifxetex() {
        let x = |options: &'static str, engines: EngineSet| {
            LatexPackageDef::new("x", "a test's").options([options]).engines(engines)
        };
        let (a, b, c) = (
            x("a", EngineSet::PDFLATEX),
            x("b", EngineSet::LUALATEX),
            x("c", EngineSet::XELATEX),
        );
        let body = style_body(&merged(&[&a, &b, &c]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert_eq!(
            body,
            format!(
                "% --- packages ---\n{IFTEX_REQUEST}\
                 \\ifluatex\n% flm.latex.pkg.x\n\\RequirePackage[b]{{x}}\n\
                 \\else\\ifxetex\n% flm.latex.pkg.x\n\\RequirePackage[c]{{x}}\n\
                 \\else\n% flm.latex.pkg.x\n\\RequirePackage[a]{{x}}\n\
                 \\fi\\fi\n\\endinput\n"
            )
        );
        let unicode = x("b", EngineSet::UNICODE);
        let body = style_body(&merged(&[&unicode]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert_eq!(
            body,
            format!(
                "% --- packages ---\n{IFTEX_REQUEST}\
                 \\iftutex\n% flm.latex.pkg.x\n\\RequirePackage[b]{{x}}\n\\fi\n\\endinput\n"
            )
        );
    }

    /// Neighbors that differ by engine the same way share one test; a
    /// package the same under every engine between them ends the run.
    #[test]
    fn neighbors_that_differ_alike_share_one_test() {
        let split = |package: &'static str| {
            [
                LatexPackageDef::new(package, "the Unicode engines'")
                    .options(["u"])
                    .engines(EngineSet::UNICODE),
                LatexPackageDef::new(package, "pdfLaTeX's")
                    .options(["p"])
                    .engines(!EngineSet::UNICODE),
            ]
        };
        let [a_u, a_p] = split("a");
        let [b_u, b_p] = split("b");
        let same = LatexPackageDef::new("same", "every engine's");
        let body = style_body(
            &merged(&[&a_u, &a_p, &b_u, &b_p, &same]),
            &Declared::new(),
            &[],
            &[],
            &ExtraPreamble::default(),
        );
        assert_eq!(
            body,
            format!(
                "% --- packages ---\n{IFTEX_REQUEST}\
                 \\iftutex\n\
                 % flm.latex.pkg.a\n\\RequirePackage[u]{{a}}\n\
                 % flm.latex.pkg.b\n\\RequirePackage[u]{{b}}\n\
                 \\else\n\
                 % flm.latex.pkg.a\n\\RequirePackage[p]{{a}}\n\
                 % flm.latex.pkg.b\n\\RequirePackage[p]{{b}}\n\
                 \\fi\n\
                 % flm.latex.pkg.same\n\\RequirePackage{{same}}\n\
                 \\endinput\n"
            )
        );
        // The same requests with the shared package between the two: two runs,
        // two tests.
        let body = style_body(
            &merged(&[&a_u, &a_p, &same, &b_u, &b_p]),
            &Declared::new(),
            &[],
            &[],
            &ExtraPreamble::default(),
        );
        assert_eq!(body.matches("\\iftutex\n").count(), 2, "{body}");
    }

    /// The private positions of the writer name the engines of
    /// [`ENGINES`] they index.
    #[test]
    fn the_engine_positions_index_the_family_s_engines() {
        assert_eq!(ENGINES[PDFLATEX], Engine::PdfLatex);
        assert_eq!(ENGINES[LUALATEX], Engine::LuaLatex);
        assert_eq!(ENGINES[XELATEX], Engine::XeLatex);
        assert_eq!(default_encoding(Engine::PdfLatex), "T1");
        assert_eq!(default_encoding(Engine::LuaLatex), "TU");
        assert_eq!(default_encoding(Engine::XeLatex), "TU");
    }

    /// A feature's declaration stands in the block of feature definitions,
    /// before the document's own, as written; its comment line names the
    /// control sequence its test names and says that what is defined under
    /// that name first stands.
    #[test]
    fn a_feature_declaration_is_written_in_the_feature_block_under_its_test() {
        let body = style_body(
            &[],
            &Declared::new(),
            &[],
            &[
                RecomposedDefinition::new("\\flmUDa", "\\a", "\\providecommand\\flmUDa{hi}"),
                RecomposedDefinition::new(
                    "lemma",
                    "lemma",
                    "\\ifcsname lemma\\endcsname\\else\\newtheorem{lemma}[theorem]{Lemma}\\fi",
                )
                .of_feature("theorems")
                .declaration(),
            ],
            &ExtraPreamble::default(),
        );
        let expected = "% --- definitions written from the configuration of FLM features ---\n\
            % lemma (the theorems feature's, written from its configuration): declared unless \
            \\lemma is already defined, so a definition made earlier under that name is kept\n\
            \\ifcsname lemma\\endcsname\\else\\newtheorem{lemma}[theorem]{Lemma}\\fi\n\
            % --- user definitions ---\n";
        assert!(body.contains(expected), "{body}");
    }

    /// The comment line above a package request cannot be ended by the text
    /// it names: an author's request carries an identifier minted from the
    /// package name the author wrote, so a line break in it becomes a space,
    /// as it does above a user definition. The package name itself is written
    /// as it was given.
    #[test]
    fn a_line_break_in_an_identifier_cannot_end_the_comment_line() {
        let def = LatexPackageDef::new("two\nlines", "the author's");
        let body = style_body(&merged(&[&def]), &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert!(body.contains("% flm.latex.pkg.two lines\n"), "{body}");
        assert!(body.contains("\\RequirePackage{two\nlines}\n"), "{body}");
    }

    /// The author's three pieces stand where the design puts them — before the
    /// packages, after them, at the end — each under a comment line of its
    /// own and written as it stands, with a line break added only where the
    /// next line would otherwise share theirs.
    #[test]
    fn the_three_preamble_pieces_stand_in_their_places_byte_for_byte() {
        let roster = standard_packages();
        let amsmath = roster.iter().find(|d| d.id.as_str() == packages::AMSMATH).unwrap();
        let block = "\\newcommand\\mine{% not a comment line of ours\n  \\textbf{x}}";
        let author = ExtraPreamble {
            before_packages: String::from("\\PassOptionsToPackage{fleqn}{amsmath}"),
            after_packages: String::from("\\ProvideDocumentCommand\\flmRef{O{} m}{#2}"),
            at_end: String::from(block),
        };
        let body = style_body(
            &merged(&[amsmath]),
            &Declared::new(),
            &[],
            &[RecomposedDefinition::new("\\flmUDa", "\\a", "\\providecommand\\flmUDa{hi}")],
            &author,
        );
        let before_at = body.find("% --- the author's own preamble: before the packages ---\n\\PassOptionsToPackage{fleqn}{amsmath}\n").unwrap();
        let packages_at = body.find("% --- packages ---").unwrap();
        let after_at = body.find("% --- the author's own preamble: after the packages ---\n\\ProvideDocumentCommand\\flmRef{O{} m}{#2}\n").unwrap();
        let definitions_at = body.find("% --- user definitions ---").unwrap();
        let end_at = body.find("% --- the author's own preamble: at the end ---").unwrap();
        assert!(before_at < packages_at, "{body}");
        assert!(packages_at < after_at && after_at < definitions_at, "{body}");
        assert!(definitions_at < end_at, "{body}");
        assert!(body.ends_with(&format!("{block}\n\\endinput\n")), "{body}");
        // The first piece stands even where the package block is empty.
        let only_before = ExtraPreamble {
            before_packages: String::from("\\relax\n"),
            ..ExtraPreamble::default()
        };
        assert_eq!(
            style_body(&[], &Declared::new(), &[], &[], &only_before),
            "% --- the author's own preamble: before the packages ---\n\\relax\n\\endinput\n"
        );
        // A piece that ends in a line break gets no second one.
        let ends_in_break =
            ExtraPreamble { at_end: String::from("\\relax\n"), ..ExtraPreamble::default() };
        let body = style_body(&[], &Declared::new(), &[], &[], &ends_in_break);
        assert!(body.ends_with("\\relax\n\\endinput\n"), "{body}");
        // Empty pieces write no comment line and nothing at all.
        let body = style_body(&[], &Declared::new(), &[], &[], &ExtraPreamble::default());
        assert_eq!(body, "\\endinput\n");
    }

    #[test]
    fn the_content_hash_names_the_body_and_ignores_the_header() {
        let body = style_body(&[], &Declared::new(), &[], &[], &ExtraPreamble::default());
        let hash = content_hash(&body);
        let style = with_header(&format!("stem.{hash}"), &body);
        assert!(style.starts_with("\\NeedsTeXFormat{LaTeX2e}\n\\ProvidesPackage{stem."));
        assert_eq!(content_hash(&style[style.find("\\endinput").unwrap()..]), hash);
        assert_ne!(content_hash("other"), hash);
    }
}
