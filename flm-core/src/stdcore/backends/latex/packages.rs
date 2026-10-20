//! Package requests: how a lowering rule asks for a LaTeX package, and how
//! all the requests of a document become the `\RequirePackage` lines of its
//! style file.
//!
//! A **package request** is a LaTeX package that a lowering rule or an
//! emitter requires by identifier (`flm.latex.pkg.<name>`), resolved through
//! the service slot `flm.latex.packages` to a package definition and merged
//! with every other request for that package into one `\RequirePackage` line
//! per package and engine. This module holds the package definition
//! ([`LatexPackageDef`]), the slot ([`SLOT`]) and its readers
//! ([`packages_named`], [`packages`]), the standard package definitions
//! ([`standard_packages`]), the definitions derived from the Unicode-to-LaTeX
//! table ([`chunk_packages`]), and the merge ([`merge_requests`]). The
//! concepts overview places package requests among the other LaTeX
//! requirements under [LaTeX package requests, definition pieces and
//! hooks](crate::guide::concepts_overview#latex-package-requests-definition-pieces-and-hooks).
//!
//! # Identifiers
//!
//! A **package identifier** is the name a lowering rule or an emitter puts
//! into a lowered tree's requirements to say "this document needs this LaTeX
//! package": `flm.latex.pkg.<name>`, for instance [`AMSMATH`]
//! (`flm.latex.pkg.amsmath`). The identifier resolves through the slot to a
//! [`LatexPackageDef`], which says which package to load and with which
//! options. Requirements carry identifiers rather than package names, so that
//! the same package needed with options by one feature and without options by
//! another is two separate declared entries; [`merge_requests`] then combines
//! the two into the one line LaTeX reads.
//!
//! The slot accepts any number of contributions, kept in registration order.
//! The `latex` feature contributes the standard package definitions
//! ([`standard_packages`]); another feature contributes its own package
//! definitions the same way and then requires them by identifier from its
//! rules.
//!
//! # One line per package
//!
//! LaTeX loads a package once: a second `\RequirePackage` for a package
//! already loaded with other options is a package option clash and stops the
//! compilation. So every request for one package (the LaTeX family's own, the
//! ones the document's content needs, and the ones the document's author added) is
//! merged into a single [`PackageRequest`] whose options are the union of the
//! requests' options, each option once, in the order it was first found. The
//! rule is the same for every package: there is no merge policy per package.
//! Two requests that give one option key *different* values cannot be merged;
//! that is a [`PackageOptionConflict`], which the caller reports, and no line
//! is written for that package under the engines where its requests conflict.
//!
//! # A request applies to a set of engines
//!
//! Every request an author or the family writes by hand applies to every
//! LaTeX engine; a request derived from a chunk of the Unicode-to-LaTeX table
//! applies to the engines of its case, so one identifier may resolve to one
//! definition per set of engines ([`engines`](field@LatexPackageDef::engines),
//! [`packages_named`]). The merge runs once per engine the family knows
//! ([`ENGINES`]) and gives one line per engine ([`PackageRequest::by_engine`]).
//! Where a package's line differs by engine, the style file writes the lines
//! inside a test of the engine ([`style_body`](super::style::style_body)), so
//! the style file is the same regardless of the engine that reads it.
//!
//! # Packages the author loads in their own preamble
//!
//! A package the document's author requests in their own preamble is not part
//! of the merge, and LaTeX's own rule applies to it: for most packages, a
//! request with an option the package's first request lacked is an option
//! clash. An author who loads a package that the style file also loads
//! therefore gives both requests the same options.
//!
//! # The packages of the Unicode-to-LaTeX table
//!
//! The packages the escaping needs ([`super::escape`]) are not chosen here:
//! the Unicode-to-LaTeX table of [`untechxt`] states, entry by entry, what
//! each spelling needs, as a set of *chunks* ([`Chunk`]). A **package chunk**
//! loads a package under every engine, with options that can differ by engine
//! ([`is_package_chunk`]); any other chunk is one block of declarations that
//! no package makes ([`ChunkPreamble`]). A package chunk resolves to the
//! package definitions derived from it ([`chunk_packages`]), under the
//! identifier [`chunk_package_id`] gives; a declaration chunk resolves to a
//! definition piece of [`super::preamble`], under the identifier
//! [`chunk_piece_id`](super::preamble::chunk_piece_id) gives. [`table_chunks`]
//! lists every chunk. None of this is kept in step with the table by hand: the
//! data is untechxt's, read when the registry is built.

use std::borrow::Cow;

use untechxt::builtin::needs_profiles::PROFILES;
use untechxt::preamble::{Chunk, ChunkPreamble, Engine, EngineSet, Profile};
use crate::core::services::Services;
use crate::model::names::Name;

/// The name of the service slot of package definitions, `flm.latex.packages`:
/// every contribution is a [`LatexPackageDef`], kept in registration order.
/// The `latex` feature declares the slot ([`super::LatexFamily`]).
pub const SLOT: &str = "flm.latex.packages";

/// The prefix of every package identifier, `flm.latex.pkg.`.
pub const ID_PREFIX: &str = "flm.latex.pkg.";

/// `flm.latex.pkg.amsmath` — the `amsmath` package: the display-math
/// environments other than `equation`, `\text` in mathematics and `\eqref`,
/// which the rules of the `math` and `math-passthrough` features request where
/// the document uses them. The
/// Unicode-to-LaTeX table's `amsmath` chunk is a definition of its own,
/// `flm.latex.pkg.untechxt.amsmath` ([`chunk_packages`]); where both are
/// requested, the merge combines them into one line,
/// `\RequirePackage{amsmath}`, under a comment line that gives both
/// identifiers ([`merge_requests`]).
pub const AMSMATH: &str = "flm.latex.pkg.amsmath";
/// `flm.latex.pkg.zref-clever` — the `zref-clever` package: `\zcref`, the
/// command that the default reference hook (`\flmRef`,
/// [`refs_piece`](super::preamble::refs_piece)) sets a reference with.
pub const ZREF_CLEVER: &str = "flm.latex.pkg.zref-clever";
/// `flm.latex.pkg.textcase` — the `textcase` package: `\NoCaseChange`, the
/// wrapper that keeps a reference out of a command that rewrites its
/// argument (`\MakeUppercase`), written around a reference under the `refs`
/// feature's `protect` option. The package supplies the command on every
/// kernel; on a kernel of 2022 or later, where the kernel defines it itself,
/// the package is a compatibility layer over the kernel's.
pub const TEXTCASE: &str = "flm.latex.pkg.textcase";
/// `flm.latex.pkg.hyperref` — the `hyperref` package: links in the PDF,
/// among them `\hyperref`, the command that shows a text of the document's
/// own as a link to a labeled place, and `\href`.
///
/// A reference that shows the author's own words requests the package only
/// where the render option
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
/// is off, since the recomposed LaTeX then writes `\hyperref` itself. With
/// that option on (the default), the LaTeX hook `\flmHyperref` links through
/// the package where the document loaded it and prints the text
/// alone where it did not, so such a reference leaves the choice to the
/// author ([`hyperref_piece`](super::preamble::hyperref_piece)). The definition
/// pieces of the `href` feature (hyperlink hooks built on `\href`;
/// [`href::latex`](crate::stdcore::features::href::latex)) and of the
/// `defterm` feature (which writes `\phantomsection`;
/// [`defterm::latex`](crate::stdcore::features::defterm::latex)) request it,
/// and so does the `floats` feature's rule, for the `\phantomsection` it
/// writes before the labels of an unnumbered float's caption
/// ([`floats::latex`](crate::stdcore::features::floats::latex)).
pub const HYPERREF: &str = "flm.latex.pkg.hyperref";
/// `flm.latex.pkg.fvextra` — the `fvextra` package (which loads
/// `fancyvrb`): `\EscVerb` and the `Verbatim` environment the verbatim
/// definition piece builds on.
pub const FVEXTRA: &str = "flm.latex.pkg.fvextra";
/// `flm.latex.pkg.newunicodechar` — the `newunicodechar` package:
/// `\newunicodechar`, with which the style file declares the characters
/// written through as UTF-8 under the `Declare` Unicode mode.
pub const NEWUNICODECHAR: &str = "flm.latex.pkg.newunicodechar";
/// `flm.latex.pkg.enumitem` — the `enumitem` package: the `label=` key of a
/// list environment's optional argument, which the recomposed LaTeX writes a
/// list's own label template as, and the commands `\arabic*`, `\alph*`,
/// `\Alph*`, `\roman*` and `\Roman*` that stand for the item's number in it.
pub const ENUMITEM: &str = "flm.latex.pkg.enumitem";

/// `flm.latex.pkg.amsthm` — the `amsthm` package: `\newtheorem`,
/// `\theoremstyle` and the `proof` environment, over which the recomposed
/// LaTeX declares the theorem-like environments where a proof's
/// back-reference is written resolved.
pub const AMSTHM: &str = "flm.latex.pkg.amsthm";

/// `flm.latex.pkg.phfthm` — the `phfthm` package: theorem and proof
/// environments over `amsthm` that read a proof's back-reference as it is
/// written (`\begin{proof}[*thm:x]`), `\phfMakeTheorem`,
/// `\phfMakeProofEnv` and `\noproofref`. Requested with the option
/// `thmset=empty`, `\RequirePackage[thmset=empty]{phfthm}`, so that the
/// package declares no theorem environment of its own: the style file
/// declares every theorem kind the document uses, from the `theorems`
/// feature's configuration, and an environment of that name the author
/// defines before the style file is loaded is kept. An author
/// who loads the package too gives it the same option (see the
/// [module documentation](self#packages-the-author-loads-in-their-own-preamble)).
/// It is not part of every TeX installation.
pub const PHFTHM: &str = "flm.latex.pkg.phfthm";

/// `flm.latex.pkg.phfcc` — the `phfcc` package: a reviewer's commenting
/// commands, each made by `\phfMakeCommentingCommand[<keys>]{<name>}`,
/// which defines `\<name>{…}` and `\<name> … \end<name>` (a highlighted
/// passage) and `\<name>[…]` (a comment), each colored in the reviewer's
/// color, the colors assigned in the order the commands are made. The
/// recomposed LaTeX of the `annotations` feature is written over it,
/// requested with no option. It is not part of every TeX installation.
pub const PHFCC: &str = "flm.latex.pkg.phfcc";

/// `flm.latex.pkg.graphicx` — the `graphicx` package: `\includegraphics`,
/// with which the `graphics` feature's LaTeX hook `\flmIncludeGraphics`
/// ([`preamble`](super::preamble#the-features-pieces)) shows a picture.
pub const GRAPHICX: &str = "flm.latex.pkg.graphicx";

/// `flm.latex.pkg.adjustbox` — the `adjustbox` package loaded with the option
/// `export`, `\RequirePackage[export]{adjustbox}`: the option makes its keys
/// available to `\includegraphics`, among them `max width`, with which the
/// `graphics` feature's LaTeX hook `\flmIncludeGraphics` keeps a picture
/// within the line's width.
pub const ADJUSTBOX: &str = "flm.latex.pkg.adjustbox";

/// `flm.latex.pkg.caption` — the `caption` package: `\caption*`, a caption
/// set with no number, with which the `floats` feature's LaTeX hook, the
/// environment `flmFloat`, sets the caption of a float that has one and no
/// number.
pub const CAPTION: &str = "flm.latex.pkg.caption";
/// `flm.latex.pkg.tabularray` — the `tabularray` package: the `tblr`
/// environment, in which the `cells` feature's LaTeX hook `\flmCellsTable`
/// sets a table of cells, and `\NewTableCommand`, with which the `cells`
/// feature's definition piece defines the table command a cell's custom style
/// is written as.
pub const TABULARRAY: &str = "flm.latex.pkg.tabularray";

/// `flm.latex.pkg.xcolor` — the `xcolor` package: `\providecolor`, with
/// which the `cells` feature's definition piece declares the background
/// colors of the cell styles the `cells` feature declares by default.
pub const XCOLOR: &str = "flm.latex.pkg.xcolor";

/// The identifier of the package `name`: [`ID_PREFIX`] followed by the name.
pub fn package_id(name: &str) -> Name {
    Name::from(format!("{ID_PREFIX}{name}"))
}

/// The LaTeX engines the LaTeX family ([`super`]) writes for, in this order:
/// pdfLaTeX, LuaLaTeX, XeLaTeX ([`Engine`]). A package request applies to a
/// set of them ([`LatexPackageDef::engines`](field@LatexPackageDef::engines));
/// the merge
/// gives one line per engine ([`PackageRequest::by_engine`], in this order);
/// and the style file writes the lines that differ inside a test of the
/// engine ([`style_body`](super::style::style_body)), whose last branch is
/// pdfLaTeX's and is also what an engine not in this list reads.
pub const ENGINES: [Engine; 3] = [Engine::PdfLatex, Engine::LuaLatex, Engine::XeLatex];

/// The prefix of the identifier of a package definition derived from a chunk
/// of the Unicode-to-LaTeX table, `flm.latex.pkg.untechxt.`: it begins with
/// [`ID_PREFIX`], so the identifier is read as a package identifier
/// everywhere, and the part after [`ID_PREFIX`] says where the definition
/// comes from.
pub const CHUNK_ID_PREFIX: &str = "flm.latex.pkg.untechxt.";

/// The identifier of the package definitions derived from the chunk whose
/// identifier is `chunk_id`: [`CHUNK_ID_PREFIX`] followed by it
/// (`flm.latex.pkg.untechxt.fontenc-t2a`).
pub fn chunk_package_id(chunk_id: &str) -> Name {
    Name::from(format!("{CHUNK_ID_PREFIX}{chunk_id}"))
}

/// Every chunk of the Unicode-to-LaTeX table: the chunks of [`untechxt`]'s
/// builtin profiles ([`PROFILES`]), each once by identifier, in order of first
/// appearance. It is everything a spelling of the table can need, and so what
/// the definitions derived from the package chunks ([`chunk_packages`]) and
/// the pieces of [`super::preamble`] cover between them.
pub fn table_chunks() -> Vec<&'static Chunk> {
    let mut chunks: Vec<&'static Chunk> = Vec::new();
    for chunk in PROFILES.iter().flat_map(Profile::chunks) {
        if !chunks.iter().any(|held| held.id() == chunk.id()) {
            chunks.push(chunk);
        }
    }
    chunks
}

/// Whether `chunk` is a package in this family: every case of it loads a
/// package, with or without options. Every other chunk is one block of
/// declarations no package makes, which the family writes as a definition
/// piece ([`chunk_pieces`](super::preamble::chunk_pieces)).
pub fn is_package_chunk(chunk: &Chunk) -> bool {
    chunk.cases().iter().all(|case| case.preamble().is_package())
}

/// The package definitions of the Unicode-to-LaTeX table's package chunks
/// ([`table_chunks`], [`is_package_chunk`]), in the chunks' order: for each
/// chunk, one definition per case, under the identifier [`chunk_package_id`]
/// gives the chunk's identifier, with the case's package name and option text
/// borrowed from untechxt's static data, applying to the engines of the case
/// that no earlier case of the chunk took — a chunk's first case that applies
/// to an engine is what the chunk is under it, so the cases are read in order
/// ([`Chunk::preamble_for`]). The [`docs`](LatexPackageDef::docs) text of
/// each says that characters of the document need the package and gives the
/// chunk's identifier.
///
/// The option text of a case is one string (`T2A,TU`), kept as one entry: the
/// merge reads an entry as LaTeX reads an option list ([`option_list`]).
pub fn chunk_packages() -> Vec<LatexPackageDef> {
    let mut defs = Vec::new();
    for chunk in table_chunks().into_iter().filter(|chunk| is_package_chunk(chunk)) {
        // The engines an earlier case of this chunk applies to.
        let mut taken = EngineSet::EMPTY;
        for case in chunk.cases() {
            // Cloning a borrowed `Cow` copies the pointer, not the text: the
            // definition borrows untechxt's static data.
            let (package, options) = match case.preamble() {
                ChunkPreamble::Package(name) => (name.clone(), Vec::new()),
                ChunkPreamble::PackageWithOptions(name, options) => {
                    (name.clone(), vec![options.clone()])
                }
                // Every case loads a package: `is_package_chunk` said so.
                ChunkPreamble::Snippet(_) => {
                    unreachable!("a package chunk loads a package under every case")
                }
            };
            defs.push(LatexPackageDef {
                id: chunk_package_id(chunk.id()),
                package,
                options,
                docs: crate::kept_docs(format!(
                    "needed by characters of the document whose LaTeX spelling uses this \
                     package (chunk `{}` of untechxt's Unicode-to-LaTeX table)",
                    chunk.id()
                )),
                // The case's engines without the earlier cases' ones;
                // `EngineSet` has no difference operator, so it is spelled
                // through the complement.
                engines: !(!case.engines() | taken),
            });
            taken = taken | case.engines();
        }
    }
    defs
}

/// The definition of one package request: the package to load and its
/// options, under the identifier a package request resolves through.
///
/// A feature contributes a definition to the slot [`SLOT`], and a lowering
/// rule requires it by its [`id`](Self::id). A definition is not by itself a
/// line of the style file: every request for one package is merged into the
/// single [`PackageRequest`] the file writes for that package
/// ([`merge_requests`]), whose options are the union of theirs and whose
/// comment line lists every identifier behind it. A request applies to the
/// engines of [`engines`](field@Self::engines): a request written by hand to
/// every engine, a request derived from a chunk of the Unicode-to-LaTeX table
/// to the engines of its case. [`requirepackage_line`](Self::requirepackage_line)
/// writes this one request on its own, for a caller that asks what a single
/// request says rather than what the style file holds.
///
/// The same value is also how a document's author asks for a package of their
/// own: the `latex-recomposed` render option
/// [`extra_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::extra_packages)
/// is a list of these, which join the document's own requests, after them,
/// before the merge. Such a request is built directly (in Rust with
/// [`new`](Self::new); in settings as a map with the keys `id`, `package`,
/// `options` and `docs`, all required) rather than contributed to the slot,
/// since no rule requires it by identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LatexPackageDef {
    /// The identifier, `flm.latex.pkg.<name>`.
    pub id: Name,
    /// The package's name as `\usepackage` takes it (`zref-clever`).
    pub package: Cow<'static, str>,
    /// The options, written comma-separated in brackets when not empty.
    pub options: Vec<Cow<'static, str>>,
    /// A short text, for a person reading the definition, saying what the
    /// package provides and when a document needs it. It is not written into
    /// the style file.
    pub docs: String,
    /// The engines the request applies to. A request an author writes, and
    /// every request the family declares by hand, is one case for every
    /// engine ([`EngineSet::ALL`]); a request derived from a chunk of the
    /// Unicode-to-LaTeX table applies to the engines of its case
    /// ([`chunk_packages`]), and one identifier then resolves to one
    /// definition per set of engines ([`packages_named`]). It is not part of
    /// the author-facing form of a request: the settings schema does not
    /// show it, and settings that give it are refused.
    #[serde(skip, default = "every_engine")]
    pub engines: EngineSet,
}

/// Every engine: what a request applies to unless it says otherwise.
fn every_engine() -> EngineSet {
    EngineSet::ALL
}

impl LatexPackageDef {
    /// The request of the package `package` under the identifier
    /// [`package_id`]`(package)`, with no options, applying to every engine;
    /// `docs` says what the package provides and when a document needs it
    /// ([`docs`](field@Self::docs)).
    pub fn new(
        package: impl Into<Cow<'static, str>>,
        docs: impl Into<String>,
    ) -> LatexPackageDef {
        let package = package.into();
        LatexPackageDef {
            id: package_id(&package),
            package,
            options: Vec::new(),
            docs: crate::kept_docs(docs),
            engines: EngineSet::ALL,
        }
    }

    /// The same request under another identifier, for a package requested
    /// twice with different options, each request under an identifier of its
    /// own.
    pub fn id(mut self, id: impl Into<Name>) -> LatexPackageDef {
        self.id = id.into();
        self
    }

    /// The same request with the given package options.
    pub fn options(
        mut self,
        options: impl IntoIterator<Item = impl Into<Cow<'static, str>>>,
    ) -> Self {
        self.options = options.into_iter().map(Into::into).collect();
        self
    }

    /// The same request applying to `engines` alone
    /// ([`engines`](field@Self::engines)).
    pub fn engines(mut self, engines: EngineSet) -> Self {
        self.engines = engines;
        self
    }

    /// The options in brackets, or nothing when there are none.
    fn bracketed_options(&self) -> String {
        if self.options.is_empty() {
            String::new()
        } else {
            format!("[{}]", self.options.join(","))
        }
    }

    /// The request as a document writes it: `\usepackage[opts]{package}`,
    /// without a newline. The line says nothing of the engines the request
    /// applies to ([`engines`](field@Self::engines)).
    pub fn usepackage_line(&self) -> String {
        format!("\\usepackage{}{{{}}}", self.bracketed_options(), self.package)
    }

    /// The request as a style file writes it: `\RequirePackage[opts]{package}`,
    /// without a trailing newline. The line says nothing of the engines the
    /// request applies to ([`engines`](field@Self::engines)): where a package's
    /// requests differ by engine, the style file's block writes the lines
    /// inside a test of the engine ([`style_body`](super::style::style_body)),
    /// and the style file is the same regardless of the engine that reads it.
    pub fn requirepackage_line(&self) -> String {
        format!("\\RequirePackage{}{{{}}}", self.bracketed_options(), self.package)
    }
}

/// One package of a style file's package block: under each engine the family
/// knows, the `\RequirePackage` line the block writes for it (the merged
/// options and the identifiers of the requests behind them), or nothing where
/// no request asks for the package under that engine.
///
/// [`merge_requests`] makes one of these from every request for one package,
/// and [`style_body`](super::style::style_body) writes it: each line under a
/// comment line that lists its [`ids`](PackageLine::ids), so that a reader of
/// the style file can find what asked for the line (every request, where
/// several were merged into one), once where the line is the same under every
/// engine and inside a test of the engine where it differs.
///
/// A caller may merge requests in **groups** and write the groups one after
/// another, as the `latex-recomposed` backend does: the library's requests
/// together with the author's requests
/// ([`extra_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::extra_packages))
/// form one group, and the author's requests for a package listed in the
/// render option
/// [`skip_packages`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::skip_packages)
/// form another. A package never appears in both groups, so the file still
/// holds one line per package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRequest {
    /// The package's name as `\RequirePackage` takes it (`fontenc`).
    pub package: String,
    /// The line under each engine of [`ENGINES`], in that order.
    pub by_engine: [Option<PackageLine>; 3],
}

/// The merged request for one package under one engine: the options and the
/// identifiers of the requests behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLine {
    /// The options, each entry one option, in the order the merge first found
    /// it.
    pub options: Vec<String>,
    /// The identifiers of the requests behind this line, in the order they
    /// were first found, each once.
    pub ids: Vec<Name>,
}

impl PackageRequest {
    /// The line under `engine`, or `None` where nothing requests the package
    /// under it.
    pub fn for_engine(&self, engine: Engine) -> Option<&PackageLine> {
        ENGINES
            .iter()
            .position(|known| *known == engine)
            .and_then(|at| self.by_engine[at].as_ref())
    }

    /// The line as a style file writes it under `engine`,
    /// `\RequirePackage[opts]{package}` without a trailing newline, or `None`
    /// where nothing requests the package under it.
    pub fn requirepackage_line(&self, engine: Engine) -> Option<String> {
        let line = self.for_engine(engine)?;
        let options = if line.options.is_empty() {
            String::new()
        } else {
            format!("[{}]", line.options.join(","))
        };
        Some(format!("\\RequirePackage{}{{{}}}", options, self.package))
    }
}

/// Two requests for one package that give one option key different values,
/// which no merge can resolve: `papersize=a4` against `papersize=letter`, or a
/// flag `draft` against `draft=false`.
///
/// [`merge_requests`] returns one of these per package and key and writes no
/// line for that package under the engines where the requests conflict:
/// either value would be wrong for one of the two requests. The
/// `latex-recomposed` backend reports each conflict as the Error
/// [`ConflictingPackageOptions`](crate::stdcore::backends::latex_recomposed::ConflictingPackageOptions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageOptionConflict {
    /// The package's name (`geometry`).
    pub package: String,
    /// The option key the two requests disagree on (`papersize`).
    pub key: String,
    /// The option as the request that was found first wrote it
    /// (`papersize=a4`).
    pub first: String,
    /// The option as the request that disagrees with it wrote it
    /// (`papersize=letter`).
    pub second: String,
}

/// Every entry of `options` split at its top-level commas (a comma inside
/// braces belongs to the value it is part of and does not split), each part
/// trimmed of the spaces around it, empty parts dropped.
///
/// This is how an option list is read wherever one is compared with another:
/// an author may write a whole list as one string
/// (`.options(["a4paper,margin=2cm"])`) or one string per option, and the two
/// must mean the same thing.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::packages::option_list;
///
/// assert_eq!(option_list(&[String::from("a4paper, margin=2cm")]), ["a4paper", "margin=2cm"]);
/// assert_eq!(option_list(&[String::from("keys={a,b}")]), ["keys={a,b}"]);
/// ```
pub fn option_list<S: AsRef<str>>(options: &[S]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for entry in options {
        let entry = entry.as_ref();
        let mut depth = 0usize;
        let mut start = 0usize;
        for (at, ch) in entry.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    let piece = entry[start..at].trim();
                    if !piece.is_empty() {
                        out.push(String::from(piece));
                    }
                    start = at + ch.len_utf8();
                }
                _ => {}
            }
        }
        let piece = entry[start..].trim();
        if !piece.is_empty() {
            out.push(String::from(piece));
        }
    }
    out
}

/// The key and the value of one option: everything before the first
/// top-level `=` is the key, everything after it the value, both trimmed of
/// the spaces around them; an option with no such `=` is a *flag*, whose key
/// is the whole option and whose value is `None`.
///
/// A flag and a `key=value` option with the same key are two different values
/// of that key: `draft` is not `draft=false` as far as this library can tell,
/// so the two conflict rather than merge.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::packages::option_key_value;
///
/// assert_eq!(option_key_value("papersize=a4"), ("papersize", Some("a4")));
/// assert_eq!(option_key_value("fleqn"), ("fleqn", None));
/// assert_eq!(option_key_value("keys={a=1,b=2}"), ("keys", Some("{a=1,b=2}")));
/// ```
pub fn option_key_value(option: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    for (at, ch) in option.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '=' if depth == 0 => {
                return (option[..at].trim(), Some(option[at + 1..].trim()));
            }
            _ => {}
        }
    }
    (option.trim(), None)
}

/// Merges `requests` into one [`PackageRequest`] per package, with one line
/// per engine of [`ENGINES`]: under each engine, the union of the options of
/// every request that applies to it ([`engines`](field@LatexPackageDef::engines)),
/// each option once, in the order it was first found, and the identifiers of
/// every request behind the line.
///
/// The packages come out in the order each was first requested, under any
/// engine, so a package keeps the place its first request had. Options are
/// read with [`option_list`] and compared by key with [`option_key_value`]:
/// an option already in the union with the same value is dropped, and one
/// with a *different* value is a [`PackageOptionConflict`], returned in the
/// second element of the result, once regardless of how many engines it was
/// found under. A package whose requests conflict under an engine gets **no** line
/// under that engine — and none at all where they conflict under every
/// engine, as two requests applying to every engine do — since either value
/// would be wrong for one of the two requests that asked. A package left with
/// no line under any engine is not in the result.
///
/// There is no merge policy per package or per engine: every standard package
/// definition and every package an author adds is merged this way, under
/// every engine. Where an ordering among the options would otherwise carry
/// meaning — `fontenc`, whose last encoding would become the document's
/// default — the style file states what it needs in a command of its own,
/// per engine ([`style_body`](super::style::style_body)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::LatexPackageDef;
/// use flm_core::stdcore::backends::latex::packages::merge_requests;
/// use flm_core::untechxt::preamble::{Engine, EngineSet};
///
/// let t1 = LatexPackageDef::new("fontenc", "Latin letters")
///     .id("a")
///     .options(["T1"])
///     .engines(!EngineSet::UNICODE);
/// let t2a = LatexPackageDef::new("fontenc", "Cyrillic letters").id("b").options(["T2A", "T1"]);
/// let (merged, conflicts) = merge_requests([&t1, &t2a]);
/// assert!(conflicts.is_empty());
/// assert_eq!(merged.len(), 1);
/// assert_eq!(
///     merged[0].requirepackage_line(Engine::PdfLatex).as_deref(),
///     Some("\\RequirePackage[T1,T2A]{fontenc}")
/// );
/// assert_eq!(
///     merged[0].requirepackage_line(Engine::LuaLatex).as_deref(),
///     Some("\\RequirePackage[T2A,T1]{fontenc}")
/// );
/// ```
pub fn merge_requests<'a>(
    requests: impl IntoIterator<Item = &'a LatexPackageDef>,
) -> (Vec<PackageRequest>, Vec<PackageOptionConflict>) {
    let requests: Vec<&LatexPackageDef> = requests.into_iter().collect();
    // One entry per package, in the order the packages were first met over
    // every engine. A linear scan over a handful of packages is cheaper than a
    // map, and it is what keeps the order the caller resolved the
    // requirements in.
    let mut merged: Vec<PackageRequest> = Vec::new();
    for def in &requests {
        if !merged.iter().any(|request| *request.package == *def.package) {
            merged.push(PackageRequest {
                package: def.package.to_string(),
                by_engine: Default::default(),
            });
        }
    }
    let mut conflicts: Vec<PackageOptionConflict> = Vec::new();
    for (at, engine) in ENGINES.into_iter().enumerate() {
        let (lines, met) = merge_under(&requests, engine);
        for (package, line) in lines {
            if let Some(request) = merged.iter_mut().find(|request| request.package == package) {
                request.by_engine[at] = Some(line);
            }
        }
        for conflict in met {
            if !conflicts.contains(&conflict) {
                conflicts.push(conflict);
            }
        }
    }
    merged.retain(|request| request.by_engine.iter().any(Option::is_some));
    (merged, conflicts)
}

/// The merge of [`merge_requests`] under the single engine `engine`, over the
/// requests that apply to it: each package's line, in the order the packages
/// were first found, and the conflicts found. A package with a conflict has
/// no line.
fn merge_under(
    requests: &[&LatexPackageDef],
    engine: Engine,
) -> (Vec<(String, PackageLine)>, Vec<PackageOptionConflict>) {
    let mut lines: Vec<(String, PackageLine)> = Vec::new();
    // Which packages turned out unmergeable, so that no line is written for
    // them however many further requests arrive.
    let mut refused: Vec<String> = Vec::new();
    let mut conflicts: Vec<PackageOptionConflict> = Vec::new();
    for def in requests.iter().filter(|def| def.engines.contains(engine)) {
        let at = match lines.iter().position(|(package, _)| **package == *def.package) {
            Some(at) => at,
            None => {
                lines.push((
                    def.package.to_string(),
                    PackageLine { options: Vec::new(), ids: Vec::new() },
                ));
                lines.len() - 1
            }
        };
        let line = &mut lines[at].1;
        if !line.ids.contains(&def.id) {
            line.ids.push(def.id.clone());
        }
        for option in option_list(&def.options) {
            let (key, value) = option_key_value(&option);
            let seen = line
                .options
                .iter()
                .find(|written| option_key_value(written).0 == key)
                .cloned();
            match seen {
                Some(written) if option_key_value(&written).1 == value => {}
                Some(written) => {
                    let conflict = PackageOptionConflict {
                        package: def.package.to_string(),
                        key: String::from(key),
                        first: written,
                        second: option.clone(),
                    };
                    if !conflicts.contains(&conflict) {
                        conflicts.push(conflict);
                    }
                    if !refused.iter().any(|package| **package == *def.package) {
                        refused.push(def.package.to_string());
                    }
                }
                None => line.options.push(option),
            }
        }
    }
    lines.retain(|(package, _)| !refused.contains(package));
    (lines, conflicts)
}

/// The standard package definitions the `latex` feature contributes: the
/// packages that the standard features' rules and definition pieces may
/// require —
/// [`AMSMATH`], [`ZREF_CLEVER`], [`TEXTCASE`], [`HYPERREF`], [`FVEXTRA`],
/// [`NEWUNICODECHAR`], [`ENUMITEM`], [`AMSTHM`], [`PHFTHM`], [`PHFCC`],
/// [`GRAPHICX`], [`ADJUSTBOX`], [`CAPTION`], [`TABULARRAY`], [`XCOLOR`] — in
/// that order, followed by the definitions derived from the Unicode-to-LaTeX
/// table's package chunks ([`chunk_packages`]), which the escaping requires.
pub fn standard_packages() -> Vec<LatexPackageDef> {
    let mut packages = vec![
        LatexPackageDef::new(
            "amsmath",
            "the display-math environments other than `equation`, `\\text` in mathematics and \
             `\\eqref`; needed where the document uses them",
        ),
        LatexPackageDef::new(
            "zref-clever",
            "`\\zcref`, which prints a reference with the name of its target (\"Section 2\"); \
             needed by every reference",
        ),
        LatexPackageDef::new(
            "textcase",
            "`\\NoCaseChange`, which keeps a reference unchanged inside a command that changes \
             the case of its argument; needed where references are protected that way",
        ),
        LatexPackageDef::new(
            "hyperref",
            "`\\hyperref`, `\\href` and `\\phantomsection`: links in the PDF; needed by \
             hyperlinks, defined terms, the labeled captions of unnumbered floats, and \
             references to the author's own words when the render option `label_hooks` is off",
        ),
        LatexPackageDef::new(
            "fvextra",
            "`\\EscVerb` and the `Verbatim` environment (version 1.5 or later); needed by \
             verbatim content",
        ),
        LatexPackageDef::new(
            "newunicodechar",
            "`\\newunicodechar`, with which the style file declares the non-ASCII characters \
             that the document holds as UTF-8; needed where the document holds such characters",
        ),
        LatexPackageDef::new(
            "enumitem",
            "the `label=` key of a list environment; needed by a list with a label of its own",
        ),
        LatexPackageDef::new(
            "amsthm",
            "`\\newtheorem`, `\\theoremstyle` and `proof`; needed by theorems and proofs when \
             a proof's back-reference is written resolved",
        ),
        LatexPackageDef::new(
            "phfthm",
            "`\\phfMakeTheorem` and `\\phfMakeProofEnv`; needed by theorems and proofs when a \
             proof's back-reference is written as the author wrote it",
        )
        .options(["thmset=empty"]),
        LatexPackageDef::new(
            "phfcc",
            "`\\phfMakeCommentingCommand`; needed by a reviewer's highlights and comments",
        ),
        LatexPackageDef::new("graphicx", "`\\includegraphics`; needed by pictures"),
        LatexPackageDef::new(
            "adjustbox",
            "the `max width` key and the other keys of `adjustbox` on `\\includegraphics`; \
             needed by pictures",
        )
        .options(["export"]),
        LatexPackageDef::new(
            "caption",
            "`\\caption*`, a caption without a number; needed by floats",
        ),
        LatexPackageDef::new(
            "tabularray",
            "the `tblr` environment and `\\NewTableCommand`; needed by tables of cells",
        ),
        LatexPackageDef::new(
            "xcolor",
            "`\\providecolor`, for the background colors of cells; needed by tables of cells",
        ),
    ];
    packages.extend(chunk_packages());
    packages
}

/// Every package definition the registry holds, in contribution order.
pub fn packages(services: &Services) -> impl Iterator<Item = &LatexPackageDef> + '_ {
    services.services_named::<LatexPackageDef>(SLOT)
}

/// Every package definition with the identifier `id`, in contribution order:
/// one for a request that is the same under every engine, and one per set of
/// engines for a request that differs between engines
/// ([`engines`](field@LatexPackageDef::engines)); none when no feature
/// contributed one — which a caller reports as an unknown requirement rather
/// than fails on.
pub fn packages_named<'s>(
    services: &'s Services,
    id: &'s str,
) -> impl Iterator<Item = &'s LatexPackageDef> + 's {
    packages(services).filter(move |def| def.id.as_str() == id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The package chunks of the table are fifteen and the declaration chunks
    /// five; the fifteen give twenty-one definitions, one per case, each
    /// applying to the engines its case took from the earlier ones.
    #[test]
    fn the_chunk_definitions_are_the_table_s_package_chunks_with_their_engine_cases() {
        let chunks = table_chunks();
        assert_eq!(chunks.iter().filter(|chunk| is_package_chunk(chunk)).count(), 15);
        assert_eq!(chunks.iter().filter(|chunk| !is_package_chunk(chunk)).count(), 5);
        let defs = chunk_packages();
        assert_eq!(defs.len(), 21);
        for def in &defs {
            let chunk_id = def.id.as_str().strip_prefix(CHUNK_ID_PREFIX).unwrap_or_else(|| {
                panic!("{} does not begin with {CHUNK_ID_PREFIX}", def.id)
            });
            #[cfg(feature = "docs")]
            assert_eq!(
                def.docs,
                format!(
                    "needed by characters of the document whose LaTeX spelling uses this \
                     package (chunk `{chunk_id}` of untechxt's Unicode-to-LaTeX table)"
                )
            );
            #[cfg(not(feature = "docs"))]
            assert_eq!(def.docs, "", "{chunk_id}");
        }
        let of = |chunk_id: &str| -> Vec<&LatexPackageDef> {
            let id = chunk_package_id(chunk_id);
            defs.iter().filter(|def| def.id == id).collect()
        };
        let t2a = of("fontenc-t2a");
        assert_eq!(t2a.len(), 2);
        assert_eq!(t2a[0].package, "fontenc");
        assert_eq!(t2a[0].options, ["T2A,TU"]);
        assert_eq!(t2a[0].engines, EngineSet::UNICODE);
        assert_eq!(t2a[1].package, "fontenc");
        assert_eq!(t2a[1].options, ["T2A,T1"]);
        assert_eq!(t2a[1].engines, !EngineSet::UNICODE);
        let t1 = of("fontenc-t1");
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0].options, ["T1"]);
        assert_eq!(t1[0].engines, !EngineSet::UNICODE);
        let dsfont = of("dsfont");
        assert_eq!(dsfont.len(), 1);
        assert!(dsfont[0].options.is_empty());
        assert_eq!(dsfont[0].engines, EngineSet::ALL);
        // The identifiers, each once in order of first appearance, are the
        // package chunks' in the table's order.
        let mut ids: Vec<Name> = Vec::new();
        for def in &defs {
            if !ids.contains(&def.id) {
                ids.push(def.id.clone());
            }
        }
        let expected: Vec<Name> = [
            "dsfont",
            "nicefrac",
            "fontenc-t2a",
            "fontenc-t1",
            "tipa",
            "amssymb",
            "amsmath",
            "mathrsfs",
            "bbm",
            "fontenc-x2",
            "fontenc-t2b",
            "fontenc-t2c",
            "fontenc-ot2",
            "fontenc-t2d",
            "tipx",
        ]
        .into_iter()
        .map(chunk_package_id)
        .collect();
        assert_eq!(ids, expected);
    }

    /// The declarations of the table's chunks define no command in the
    /// family's own name space (`\flm…`,
    /// [`FAMILY_PREFIX`](super::super::names::FAMILY_PREFIX)): the style file
    /// writes them as untechxt ships them, under untechxt's prefix `\UnxT…`,
    /// beside the family's own definitions.
    #[test]
    fn no_chunk_declaration_takes_a_family_name() {
        use crate::stdcore::backends::latex::names::FAMILY_PREFIX;
        // A negative assertion over the lines the scan recognizes would pass
        // over a table whose declarations it recognizes none of; the count
        // says the scan read something.
        let mut checked = 0;
        for chunk in table_chunks().into_iter().filter(|chunk| !is_package_chunk(chunk)) {
            let ChunkPreamble::Snippet(latex) = chunk.cases()[0].preamble() else {
                panic!("the declaration chunk {} does not hold declarations", chunk.id());
            };
            for line in latex.lines() {
                let Some(rest) = line.strip_prefix("\\DeclareMathAlphabet{\\")
                    .or_else(|| line.strip_prefix("\\DeclareMathSymbol{\\"))
                    .or_else(|| line.strip_prefix("\\DeclareTextSymbol{\\"))
                else {
                    continue;
                };
                let name = rest.split('}').next().unwrap_or_default();
                assert!(
                    !name.starts_with(FAMILY_PREFIX),
                    "the chunk {} defines `\\{name}`, a name in the family's own \
                     `\\{FAMILY_PREFIX}…` space",
                    chunk.id()
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "no declaration of the table was recognized, so none was checked");
    }

    /// A request's line carries its options and nothing of the engines it
    /// applies to: the two `fontenc-t2a` definitions write their own lines
    /// whatever their engines.
    #[test]
    fn the_request_lines_carry_their_options_and_say_nothing_of_the_engine() {
        let defs = chunk_packages();
        let id = chunk_package_id("fontenc-t2a");
        let t2a: Vec<&LatexPackageDef> = defs.iter().filter(|def| def.id == id).collect();
        assert_eq!(t2a[0].usepackage_line(), r"\usepackage[T2A,TU]{fontenc}");
        assert_eq!(t2a[0].requirepackage_line(), r"\RequirePackage[T2A,TU]{fontenc}");
        assert_eq!(t2a[1].usepackage_line(), r"\usepackage[T2A,T1]{fontenc}");
        assert_eq!(t2a[1].requirepackage_line(), r"\RequirePackage[T2A,T1]{fontenc}");
        let roster = standard_packages();
        let amsmath = roster.iter().find(|d| d.id.as_str() == AMSMATH).unwrap();
        assert_eq!(amsmath.requirepackage_line(), r"\RequirePackage{amsmath}");
    }

    /// Every identifier of the roster is a package identifier, and under each
    /// engine it resolves to at most one definition — the one that engine
    /// reads.
    #[test]
    fn the_roster_identifiers_are_prefixed_and_resolve_to_one_definition_per_engine() {
        let roster = standard_packages();
        assert!(roster.iter().all(|def| def.id.as_str().starts_with(ID_PREFIX)));
        let ids: BTreeSet<&Name> = roster.iter().map(|def| &def.id).collect();
        for engine in ENGINES {
            for id in &ids {
                let under = roster
                    .iter()
                    .filter(|def| def.id == **id && def.engines.contains(engine))
                    .count();
                assert!(under <= 1, "{id} under {engine:?}: {under} definitions");
            }
        }
        for id in [AMSMATH, ZREF_CLEVER, FVEXTRA, NEWUNICODECHAR, TEXTCASE] {
            assert!(ids.contains(&Name::from(id)), "{id}");
        }
        assert!(ids.contains(&chunk_package_id("dsfont")));
        // Two packages are requested under more than one identifier: `fontenc`,
        // once per encoding chunk, and `amsmath`, by the math features and by
        // the table.
        let shared: BTreeSet<&str> = roster
            .iter()
            .filter(|def| {
                roster.iter().any(|other| other.package == def.package && other.id != def.id)
            })
            .map(|def| &*def.package)
            .collect();
        assert_eq!(shared, BTreeSet::from(["amsmath", "fontenc"]));
        let amsmath: Vec<&str> = roster
            .iter()
            .filter(|def| def.package == "amsmath")
            .map(|def| def.id.as_str())
            .collect();
        assert_eq!(amsmath, [AMSMATH, chunk_package_id("amsmath").as_str()]);
    }

    /// The whole roster merges into one entry per package: the `fontenc`
    /// definitions become one line per engine, whose options are the
    /// encodings that engine's definitions load, each once, in the order the
    /// definitions stand in, and each line carries the identifiers behind it.
    #[test]
    fn the_rosters_fontenc_definitions_merge_into_one_line_per_engine() {
        let roster = standard_packages();
        let (merged, conflicts) = merge_requests(roster.iter());
        assert!(conflicts.is_empty(), "{conflicts:?}");
        let names: BTreeSet<&str> = roster.iter().map(|def| &*def.package).collect();
        assert_eq!(merged.len(), names.len());
        let fontenc = merged.iter().find(|request| request.package == "fontenc").unwrap();
        assert_eq!(
            fontenc.requirepackage_line(Engine::PdfLatex).as_deref(),
            Some(r"\RequirePackage[T2A,T1,X2,T2B,T2C,OT2,T2D]{fontenc}")
        );
        for engine in [Engine::LuaLatex, Engine::XeLatex] {
            assert_eq!(
                fontenc.requirepackage_line(engine).as_deref(),
                Some(r"\RequirePackage[T2A,TU,X2,T2B,T2C,OT2,T2D]{fontenc}")
            );
        }
        let chunk_ids = |chunk_ids: &[&str]| -> Vec<Name> {
            chunk_ids.iter().map(|id| chunk_package_id(id)).collect()
        };
        assert_eq!(
            fontenc.for_engine(Engine::PdfLatex).unwrap().ids,
            chunk_ids(&[
                "fontenc-t2a",
                "fontenc-t1",
                "fontenc-x2",
                "fontenc-t2b",
                "fontenc-t2c",
                "fontenc-ot2",
                "fontenc-t2d"
            ])
        );
        assert_eq!(
            fontenc.for_engine(Engine::LuaLatex).unwrap().ids,
            chunk_ids(&[
                "fontenc-t2a",
                "fontenc-x2",
                "fontenc-t2b",
                "fontenc-t2c",
                "fontenc-ot2",
                "fontenc-t2d"
            ])
        );
        // A package requested the same way under every engine has the same
        // line under every engine, here named by the two identifiers that
        // request it.
        let amsmath = merged.iter().find(|request| request.package == "amsmath").unwrap();
        for engine in ENGINES {
            assert_eq!(
                amsmath.requirepackage_line(engine).as_deref(),
                Some(r"\RequirePackage{amsmath}")
            );
            assert_eq!(
                amsmath.for_engine(engine).unwrap().ids,
                [Name::from(AMSMATH), chunk_package_id("amsmath")]
            );
        }
    }

    /// The union is in the order the options were first met, whatever order
    /// the requests arrive in, and a package keeps the place of its first
    /// request.
    #[test]
    fn the_merged_options_are_the_union_in_the_order_first_seen() {
        let first = LatexPackageDef::new("geometry", "the document's").options(["a4paper"]);
        let other = LatexPackageDef::new("xcolor", "the document's");
        let second = LatexPackageDef::new("geometry", "the author's")
            .id("flm.latex.pkg.geometry-2")
            .options(["margin=2cm", "a4paper"]);
        let (merged, conflicts) = merge_requests([&first, &other, &second]);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged[0].requirepackage_line(Engine::PdfLatex).as_deref(),
            Some(r"\RequirePackage[a4paper,margin=2cm]{geometry}")
        );
        assert_eq!(merged[1].package, "xcolor");
        let line = merged[0].for_engine(Engine::PdfLatex).unwrap();
        let ids: Vec<&str> = line.ids.iter().map(|id| id.as_str()).collect();
        assert_eq!(ids, ["flm.latex.pkg.geometry", "flm.latex.pkg.geometry-2"]);
        // Requests that apply to every engine give the same line under each.
        assert_eq!(merged[0].by_engine[0], merged[0].by_engine[1]);
        assert_eq!(merged[0].by_engine[1], merged[0].by_engine[2]);
    }

    /// One option key with two values is a conflict, and the package is then
    /// written not at all. A flag and a `key=value` option with the same key
    /// count as two values.
    #[test]
    fn one_key_with_two_values_is_a_conflict_and_the_package_is_left_out() {
        let a4 = LatexPackageDef::new("geometry", "the document's").options(["papersize=a4"]);
        let letter = LatexPackageDef::new("geometry", "the author's")
            .id("flm.latex.pkg.geometry-2")
            .options(["papersize=letter"]);
        let kept = LatexPackageDef::new("amsmath", "the document's");
        let (merged, conflicts) = merge_requests([&a4, &letter, &kept]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].package, "amsmath");
        assert_eq!(
            conflicts,
            [PackageOptionConflict {
                package: String::from("geometry"),
                key: String::from("papersize"),
                first: String::from("papersize=a4"),
                second: String::from("papersize=letter"),
            }]
        );
        // A flag against a value of the same key.
        let flag = LatexPackageDef::new("draftpkg", "one").options(["draft"]);
        let value =
            LatexPackageDef::new("draftpkg", "another").id("x").options(["draft=false"]);
        let (merged, conflicts) = merge_requests([&flag, &value]);
        assert!(merged.is_empty());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "draft");
    }

    /// Two requests that conflict under one engine alone leave the package
    /// without a line under that engine only; the conflict is reported once.
    #[test]
    fn a_conflict_under_one_engine_alone_leaves_the_package_out_there_only() {
        let a4 = LatexPackageDef::new("geometry", "every engine's").options(["papersize=a4"]);
        let letter = LatexPackageDef::new("geometry", "the Unicode engines'")
            .id("flm.latex.pkg.geometry-2")
            .options(["papersize=letter"])
            .engines(EngineSet::UNICODE);
        let (merged, conflicts) = merge_requests([&a4, &letter]);
        assert_eq!(
            conflicts,
            [PackageOptionConflict {
                package: String::from("geometry"),
                key: String::from("papersize"),
                first: String::from("papersize=a4"),
                second: String::from("papersize=letter"),
            }]
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].requirepackage_line(Engine::PdfLatex).as_deref(),
            Some(r"\RequirePackage[papersize=a4]{geometry}")
        );
        assert_eq!(merged[0].for_engine(Engine::LuaLatex), None);
        assert_eq!(merged[0].for_engine(Engine::XeLatex), None);
    }

    /// A request that applies to some engines gives no line under the others,
    /// and a package keeps the place its first request had, whatever engines
    /// that request applies to.
    #[test]
    fn a_request_absent_under_an_engine_gives_no_line_under_it() {
        let t1 = LatexPackageDef::new("fontenc", "pdfLaTeX's")
            .options(["T1"])
            .engines(!EngineSet::UNICODE);
        let (merged, conflicts) = merge_requests([&t1]);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            merged[0].requirepackage_line(Engine::PdfLatex).as_deref(),
            Some(r"\RequirePackage[T1]{fontenc}")
        );
        assert_eq!(merged[0].for_engine(Engine::LuaLatex), None);
        assert_eq!(merged[0].for_engine(Engine::XeLatex), None);
        assert_eq!(merged[0].requirepackage_line(Engine::LuaLatex), None);
        // A request for the Unicode engines alone, met first, keeps the first
        // place for its package.
        let unicode = LatexPackageDef::new("fontspec", "the Unicode engines'")
            .engines(EngineSet::UNICODE);
        let every = LatexPackageDef::new("amsmath", "every engine's");
        let (merged, conflicts) = merge_requests([&unicode, &every, &t1]);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        let order: Vec<&str> = merged.iter().map(|request| request.package.as_str()).collect();
        assert_eq!(order, ["fontspec", "amsmath", "fontenc"]);
    }

    /// An option list written as one string is read as a list, and a comma
    /// inside braces belongs to the value it stands in.
    #[test]
    fn an_option_list_is_read_at_its_top_level_commas() {
        let one = LatexPackageDef::new("geometry", "one").options(["a4paper, margin=2cm"]);
        let two = LatexPackageDef::new("geometry", "two")
            .id("x")
            .options(["margin=2cm", "headings={a,b}"]);
        let (merged, conflicts) = merge_requests([&one, &two]);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            merged[0].requirepackage_line(Engine::PdfLatex).as_deref(),
            Some(r"\RequirePackage[a4paper,margin=2cm,headings={a,b}]{geometry}")
        );
    }

    /// One request merged on its own — which is what a group of one is —
    /// gives the line the author asked for, its option list read as LaTeX
    /// reads one.
    #[test]
    fn a_group_of_one_request_gives_the_line_the_author_asked_for() {
        let def = LatexPackageDef::new("mypackage", "the author's")
            .options(["my,fixed,set,of,options"]);
        let (merged, conflicts) = merge_requests([&def]);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            merged[0].requirepackage_line(Engine::PdfLatex).as_deref(),
            Some(r"\RequirePackage[my,fixed,set,of,options]{mypackage}")
        );
        let line = merged[0].for_engine(Engine::PdfLatex).unwrap();
        assert_eq!(line.options, ["my", "fixed", "set", "of", "options"]);
        assert_eq!(line.ids.len(), 1);
    }
}
