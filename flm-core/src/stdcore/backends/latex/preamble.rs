//! Definition pieces: units of LaTeX definitions that a feature registers so
//! that the LaTeX its lowering rules write compiles, the service slot they are
//! contributed to, the own pieces of the LaTeX family ([`super`], the parts
//! every LaTeX backend shares), among them the LaTeX hooks, and the rules
//! every piece follows.
//!
//! A **definition piece** ([`LatexPreambleDef`]) is a unit of LaTeX
//! definitions that a feature registers under an identifier
//! `flm.latex.def.<name>`, together with the identifiers it needs in turn; a
//! lowering rule requires it by identifier, and the style file holds the text
//! of every piece the document requires, in registration order. A rule
//! requires a piece (for instance [`VERBATIM`], `flm.latex.def.verbatim`)
//! exactly as it requires a package ([`super::packages`]), and the style file
//! writes the pieces after the package requests ([`super::style`]). Pieces are
//! contributed to the service slot [`SLOT`] (`flm.latex.preamble`), which
//! accepts any number of them, kept in registration order. The `latex` feature
//! contributes the family's own pieces ([`standard_pieces`]: eleven written in
//! this module, and one for each block of declarations of the Unicode-to-LaTeX
//! table, [`chunk_pieces`]); a feature that needs definitions of its own
//! contributes its own piece and requires it from its rules. The concepts
//! overview places definition pieces among the other LaTeX requirements under
//! [LaTeX package requests, definition pieces and
//! hooks](crate::guide::concepts_overview#latex-package-requests-definition-pieces-and-hooks).
//!
//! # The LaTeX hooks
//!
//! A **LaTeX hook** is a macro that the recomposed LaTeX writes and the style
//! file defines with a plain default (`\flmHeading`, `\flmLabels`, `\flmRef`,
//! `\flmHyperref`, and one or more per feature such as `\flmXN<kind>`), which a
//! user redefines in the document's preamble to change what the recomposed
//! LaTeX does at that point. Five of the LaTeX family's own pieces define its
//! LaTeX hooks:
//!
//! - [`HEADING_COMMAND`] (`\flmHeading`), written on the line before every
//!   sectioning command: it runs the command and then makes the heading's
//!   labels refer to the heading, by its number where the document class
//!   numbered it and by its title where it did not. It takes a key-value
//!   argument of its own with the keys `toc` and `alt` ([`heading_piece`]).
//! - [`LABELS_COMMAND`] (`\flmLabels`), written around every group of labels:
//!   it runs them in text and, on a line of a display-math environment, makes
//!   every label after the first an alias of the first ([`labels_piece`]);
//!   together with [`LABELS_WITH_DISPLAY_TEXT_COMMAND`]
//!   (`\flmLabelsWithDisplayText`), the single command that makes labels
//!   refer to the current place and print a given text
//!   ([`label_display_text_piece`]).
//! - [`REF_COMMAND`] (`\flmRef`), written for every reference
//!   ([`refs_piece`]).
//! - [`HYPERREF_COMMAND`] (`\flmHyperref`), written for every reference whose
//!   displayed words are the document author's own ([`hyperref_piece`]).
//!
//! The render options
//! [`heading_hook`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::heading_hook)
//! and
//! [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
//! turn these hooks off. The recomposed LaTeX is then plain LaTeX, and the
//! user must provide what the hooks did; the rendering guide lists it under
//! [the LaTeX hooks](crate::guide::rendering#the-latex-hooks).
//!
//! # The features' pieces
//!
//! The standard features contribute pieces of their own, which define LaTeX
//! hooks that a user redefines in the same way. Each feature's `latex` module
//! documents its piece:
//!
//! - `xnotes`: one piece per kind of xnote, `flm.latex.def.xnotes.<kind>`,
//!   defining `\flmXN<kind>[<labels>]{<note>}`, the command an xnote of that
//!   kind is written as, which by default hands the note to LaTeX's
//!   `\footnote` or to the command the kind's configuration gives
//!   ([`xnotes::latex`](crate::stdcore::features::xnotes::latex)).
//! - `href`: `flm.latex.def.href`, defining the three hyperlink hooks
//!   `\flmHref{<address>}{<text>}`, `\flmUrl{<address>}` and
//!   `\flmEmail{<address>}` over `hyperref`'s `\href`, the last two deriving
//!   the displayed text from the address in LaTeX
//!   ([`href::latex`](crate::stdcore::features::href::latex)).
//! - `defterm`: `flm.latex.def.defterm`, defining the two hooks of a term
//!   link, `\flmTerm{<label>}{<shown>}` over `\flmHyperref` and
//!   `\flmTermInDefterm{<shown>}` for a term inside its own definition, and
//!   requiring what a definition's recomposition uses: `hyperref` for
//!   `\phantomsection`, and [`LABELS_WITH_DISPLAY_TEXT_COMMAND`] with its
//!   reference type. The environment `defterm` itself is written into the
//!   style file from the configured lead-in, as a definition of the feature's
//!   rather than as a piece
//!   ([`defterm::latex`](crate::stdcore::features::defterm::latex)).
//! - `theorems`: `flm.latex.def.theorems`, defining `\noproofref` as nothing
//!   where no package defined it first, and one piece per form of proof
//!   back-reference, each defining the proof title hook `\flmProofTitle` with
//!   the default that form's package needs and requiring that package
//!   (`phfthm` or `amsthm`, [`PHFTHM`](super::packages::PHFTHM),
//!   [`AMSTHM`](super::packages::AMSTHM)); the render option
//!   [`resolved_proof_refs`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::resolved_proof_refs)
//!   chooses the form. The theorem-like environments themselves are declared
//!   in the style file as definitions of the feature's, each guarded on its
//!   name ([`theorems::latex`](crate::stdcore::features::theorems::latex)).
//! - `quote`: `flm.latex.def.quote`, defining the four hooks of a quotation's
//!   sections, `\flmQuoteText`, `\flmQuoteLines`, `\flmQuoteAttributed` and
//!   `\flmQuoteBlock`, the command `\flmIndent{<length>}` for a line's
//!   indentation, and the environments `blockquote` and `address`, each with
//!   `\ProvideDocumentEnvironment`
//!   ([`quote::latex`](crate::stdcore::features::quote::latex)).
//! - `annotations`: no piece. Its commands are made by the `phfcc` package
//!   ([`PHFCC`](super::packages::PHFCC)), which its rule requests, with one
//!   `\phfMakeCommentingCommand` per configured annotator declared in the
//!   style file as a definition of the feature's, guarded on the command's
//!   name ([`annotations::latex`](crate::stdcore::features::annotations::latex)).
//! - `graphics`: `flm.latex.def.graphics`, defining the image hook
//!   `\flmIncludeGraphics[<keys>]{<path>}` over `\includegraphics`
//!   ([`graphics::latex`](crate::stdcore::features::graphics::latex)).
//! - `floats`: `flm.latex.def.floats`, defining the environment
//!   `\begin{flmFloat}{<type>}{<case>}` that a float is written as, with
//!   `\ProvideDocumentEnvironment`, and for each of the four cases the hooks
//!   `\flmFloatSetUseConfig<case>` and `\flmFloatPlacementArgs<case>`, and
//!   requiring the `caption` package ([`CAPTION`](super::packages::CAPTION));
//!   each float type the document uses is declared as a `zref-clever`
//!   reference type in the style file, as a definition of the feature's
//!   ([`floats::latex`](crate::stdcore::features::floats::latex)).
//! - `cells`: `flm.latex.def.cells`, defining the table hook
//!   `\flmCellsTable{<natural>}{<fitted>}{<options>}{<rows>}` over the `tblr`
//!   environment of `tabularray`, with the hooks `\flmCellsHeaderFont`,
//!   `\flmCellsBeginCenter`, `\flmCellsEndCenter` and `\flmCellsMaxWidth`, the
//!   table command `\flmCellsSet{<name>}` of a cell's custom style, and the
//!   registration of a style's cell keys, `\flmCellsStyleKeys{<name>}{<keys>}`,
//!   with the shipped colors registered; it requires the `tabularray` and
//!   `xcolor` packages ([`TABULARRAY`](super::packages::TABULARRAY),
//!   [`XCOLOR`](super::packages::XCOLOR);
//!   [`cells::latex`](crate::stdcore::features::cells::latex)).
//! - `citations`: `flm.latex.def.citations`, defining the three hooks of a
//!   citation, `\flmCite[<note>]{<keys>}` over LaTeX's `\cite`,
//!   `\flmCitePreSpace` as `\unskip~` and `\flmRefCite{<word>}{<citations>}` as
//!   the word followed by the citations, and adding `\flmCite` to the kernel's
//!   list of commands whose argument a change of case leaves unchanged; it
//!   requires no package, since the list of cited works is LaTeX's own
//!   ([`citations::latex`](crate::stdcore::features::citations::latex)).
//!
//! One of the family's own pieces defines no command: [`tab_width_piece`]
//! declares the length `\flmTabWidth`, the width of one tab. [`stage_length`],
//! the family's way of writing a length, writes a length in tabs as a multiple
//! of it and requires the piece.
//!
//! # How a piece is written
//!
//! Every piece is written for a LaTeX system of 2022 or later, the oldest
//! this library's output supports, which has `\NewDocumentCommand` and the
//! related commands, the kernel hooks and `\@currentcounter`. A piece is also
//! written so that a user can customize what it defines without editing the
//! generated file:
//!
//! - **Guarded definitions.** A macro or environment a user may want to
//!   change is defined only when nothing of that name exists when the style
//!   file is loaded, so a definition made in the user's own class or
//!   preamble *before* the style file is loaded wins. Alternatively, a user
//!   loads the style file and changes a definition *after* it with
//!   `\RenewDocumentCommand` (`\renewcommand` for a macro made with
//!   `\providecommand`). Each kind of definition has **one guarded form**, and
//!   no piece uses another:
//!
//!   | Kind | Form |
//!   |---|---|
//!   | a macro the piece offers a user | `\ProvideDocumentCommand` |
//!   | an expandable macro a piece defines: the blank test ([`if_blank_piece`]), the three that read a packed argument ([`user_args_piece`]), a float case's placement ([`floats::latex`](crate::stdcore::features::floats::latex)), which a float environment's bracket test must see | `\providecommand`, which makes a long expandable macro |
//!   | an environment the piece offers a user | `\ProvideDocumentEnvironment` |
//!   | a recomposed user definition written under its own prefix, a macro or an environment ([`super::names`]) | `\providecommand`, `\ProvideDocumentEnvironment` |
//!   | a recomposed user definition written under a name the document owns (the render option `bare_names`) | `\long\def`, `\DeclareDocumentEnvironment`: unconditionally, since the name is listed precisely so that the document's definition replaces LaTeX's own |
//!   | a recomposed user **macro** asked to be robust rather than expandable (the render option `robust_names`), under its own prefix | `\ProvideDocumentCommand\<name>{+m …}`, one `+m` per parameter: engine-protected, and `+` makes each argument long |
//!   | the same under a name the document owns (both options list it) | `\DeclareDocumentCommand\<name>{+m …}`: unconditionally, for the reason the row above gives |
//!   | a macro internal to a piece (not a customization point; `@` in its name, where it has one, keeps a document from writing it) | `\def` |
//!   | a counter internal to a piece | `\ifcsname c@<counter>\endcsname\else\newcounter{<counter>}\fi` |
//!   | a declaration that is not a command definition: a math alphabet, a symbol font or a text symbol, as the pieces derived from the Unicode-to-LaTeX table make ([`chunk_pieces`]); a variable of LaTeX's programming layer, as the heading hook's keys need ([`heading_piece`]) | `\ifcsname <name>\endcsname\else…\fi` |
//!
//!   The internals are not a customization point (a user redefines the hook,
//!   not its parts). `\def` never fails on a name already taken, and every
//!   declaration that *would* fail on one is guarded by the last two rows, so
//!   two style files loaded in one LaTeX run do not clash, as happens in a
//!   document that includes two recomposed documents. A command defined with
//!   `\ProvideDocumentCommand` is robust, so it survives being written to an
//!   auxiliary file and read back, as a macro inside a *moving argument* (a
//!   section title or a caption) must; an expandable macro survives it
//!   differently, by being replaced by what it stands for on the way there,
//!   as a recomposed user definition is by default. The four
//!   rows for a recomposed user definition belong to the emitter rather than
//!   to a piece: such a definition is written into the style file from the
//!   document's own declaration
//!   ([`latex_recomposed`](crate::stdcore::backends::latex_recomposed)), not
//!   from a piece. The table lists them so that the form of every definition
//!   in this family's LaTeX is given in one place.
//! - **No test of the installation.** A piece never asks what is installed
//!   (whether `\zcref` exists, which engine is running, which kernel defines
//!   `\NoCaseChange`), so that one document behaves the same way wherever it
//!   is compiled. What a piece needs, it requires as a package. The only
//!   question a piece may ask is about the **document's own preamble**:
//!   whether the document loaded a package. That is a fact of the document's
//!   source: it travels with the source and gives the same answer on every
//!   TeX installation, so asking it keeps the purpose of the rule. Two of the
//!   family's pieces ask it, both about `hyperref`, which none of the family's
//!   own pieces requires: [`hyperref_piece`], so that a reference showing the
//!   author's own words is a link where the document loaded that package and
//!   plain text where it did not, and [`verbatim_piece`], whose PDF-bookmark
//!   definitions tell `hyperref`, where it is loaded, what verbatim content
//!   contributes to a bookmark. Each of the two says at its own definition
//!   what it asks and why; nothing else in the family asks anything. (A
//!   feature's piece may require `hyperref`: the `href` feature's piece does,
//!   since its hooks are built on that package's macros
//!   ([`href::latex`](crate::stdcore::features::href::latex)). A recomposed
//!   document holding a hyperlink therefore loads `hyperref`, and its
//!   references showing the author's words are links there. That piece's
//!   PDF-bookmark definitions take the verbatim piece's form, for a document
//!   that skips the package request and loads the package itself.)
//! - **LaTeX hooks.** Where a piece's behavior is meant to be changed rather
//!   than replaced, it runs a LaTeX hook named `\flm…` with a default the
//!   user redefines: the per-kind formatting of verbatim content is one hook
//!   per kind.
//! - **Small and documented in the file.** Every piece is small, plain, and
//!   documented by comment lines inside its text that say what to redefine to
//!   change what.
//!
//! Every name that the eleven pieces written in this module define begins
//! with `flm`, the prefix this library owns, and never with `flmU`, which is
//! left to the document's definitions ([`super::names`]); the pieces derived
//! from the Unicode-to-LaTeX table ([`chunk_pieces`]) define names under
//! untechxt's prefix `UnxT`. A recomposed user definition is renamed under a
//! prefix beginning with `flmU`, or, where the author lists its name in the
//! render option `bare_names`, keeps that name, which cannot begin with `flm`
//! (the backend refuses such an entry of `bare_names`). A recomposed user
//! definition therefore cannot replace a definition of the eleven pieces
//! written in this module.
//!
//! The documentation of each piece below lists the macros and environments
//! it defines and what a user redefines to change what.

use techy::core::node::BuildId;

use std::collections::BTreeSet;

use crate::core::lowering::{LowerCx, LowerError, Lowerer};
use crate::core::services::Services;
use untechxt::preamble::ChunkPreamble;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::pres::{Length, LengthUnit, VerbatimKind};

use super::construct::{LatexRawPayload, RAW};
use super::packages;

/// The name of the service slot of definition pieces, `flm.latex.preamble`:
/// every contribution is a [`LatexPreambleDef`], kept in registration order.
/// The `latex` feature declares the slot ([`super::LatexFamily`]).
pub const SLOT: &str = "flm.latex.preamble";

/// The prefix of every piece identifier, `flm.latex.def.`.
pub const ID_PREFIX: &str = "flm.latex.def.";

/// Splits the requirement identifiers `ids` in two: the identifiers of
/// definition pieces (those beginning with [`ID_PREFIX`]) and every other
/// identifier, which is a package identifier ([`packages::ID_PREFIX`]).
///
/// # Examples
///
/// ```
/// use flm_core::model::names::Name;
/// use flm_core::stdcore::backends::latex::preamble::split_requirements;
///
/// let ids = [Name::from("flm.latex.def.refs"), Name::from("flm.latex.pkg.amsmath")];
/// let (pieces, packages) = split_requirements(&ids);
/// assert_eq!(pieces.into_iter().collect::<Vec<_>>(), vec![Name::from("flm.latex.def.refs")]);
/// assert_eq!(packages.into_iter().collect::<Vec<_>>(), vec![Name::from("flm.latex.pkg.amsmath")]);
/// ```
pub fn split_requirements<'n>(
    ids: impl IntoIterator<Item = &'n Name>,
) -> (BTreeSet<Name>, BTreeSet<Name>) {
    ids.into_iter().cloned().partition(|id| id.as_str().starts_with(ID_PREFIX))
}

/// `flm.latex.def.refs` — `\flmRef[<options>]{<label>}`, the hook every
/// reference is written through. See [`refs_piece`].
pub const REFS: &str = "flm.latex.def.refs";
/// `flm.latex.def.hyperref` — `\flmHyperref{<label>}{<text>}`, the hook a
/// reference displaying the author's own words is written through. See
/// [`hyperref_piece`].
pub const HYPERREF: &str = "flm.latex.def.hyperref";
/// `flm.latex.def.label-display-text` —
/// `\flmLabelsWithDisplayText{<text>}{<labels>}` and the private counter it
/// steps. See [`label_display_text_piece`].
pub const LABEL_DISPLAY_TEXT: &str = "flm.latex.def.label-display-text";
/// `flm.latex.def.zref-clever-type` — the `zref-clever` reference type of
/// the private counter, which makes a reference to a label of
/// [`LABEL_DISPLAY_TEXT`] print its text alone. See
/// [`zref_clever_type_piece`].
pub const ZREF_CLEVER_TYPE: &str = "flm.latex.def.zref-clever-type";
/// `flm.latex.def.labels` — `\flmLabels{<labels>}`, the hook every group of
/// labels is written through, with the aliasing that makes every label on a
/// line of a display-math environment resolve. See [`labels_piece`].
pub const LABELS: &str = "flm.latex.def.labels";
/// `flm.latex.def.heading` — `\flmHeading`, the hook written before every
/// sectioning command, with the key-value argument of its own that carries an
/// alternative title. See [`heading_piece`].
pub const HEADING: &str = "flm.latex.def.heading";
/// `flm.latex.def.verbatim` — the verbatim machinery: `\flmFmtVRB` and its
/// per-kind hooks and the `flmVerbatimCode` and `flmVerbatimText`
/// environments. See [`verbatim_piece`].
pub const VERBATIM: &str = "flm.latex.def.verbatim";
/// `flm.latex.def.math-environment` — `\flmMathEnvironment{<env>}{<args>}{<body>}`,
/// the macro a display-math environment is written through under the
/// `math` feature's option. See [`math_environment_piece`].
pub const MATH_ENVIRONMENT: &str = "flm.latex.def.math-environment";
/// `flm.latex.def.ifblank` — `\flmIfBlankTF`, the blank test a recomposed
/// user definition's own "is this blank?" test becomes. See
/// [`if_blank_piece`].
pub const IF_BLANK: &str = "flm.latex.def.ifblank";
/// `flm.latex.def.userargs` — `\flmArg`, `\flmIfArgTF` and `\flmIfArgBlankTF`,
/// which read one argument of a recomposed user definition that takes more
/// than nine of them and therefore receives them all in one group. See
/// [`user_args_piece`].
pub const USER_ARGS: &str = "flm.latex.def.userargs";

/// `flm.latex.def.tab-width` — the length `\flmTabWidth`, the width of one
/// tab, the unit a length in tabs is written in. See [`tab_width_piece`].
pub const TAB_WIDTH: &str = "flm.latex.def.tab-width";

/// The LaTeX length holding the width of one **tab**, `flmTabWidth`: the
/// unit in which the LaTeX family writes a length in tabs (each backend sets
/// the width of a tab in a setting of its own). [`stage_length`] writes a
/// length in tabs as a multiple of it, and [`tab_width_piece`] declares it,
/// `2em` by default.
pub const TAB_WIDTH_LENGTH: &str = "flmTabWidth";

/// Stages `length` as a LaTeX dimension, one raw node: a length in tabs as a
/// multiple of [`TAB_WIDTH_LENGTH`] (`1\flmTabWidth`, `2\flmTabWidth`,
/// `1.5\flmTabWidth`), a point as `bp` (`10bp`), and an em, a millimeter, a
/// centimeter and an inch as they are (`1.5em`, `3mm`, `2cm`, `0.25in`).
///
/// This is the family's only way of writing a length. For a length in tabs it
/// also requires the piece [`TAB_WIDTH`], which declares `\flmTabWidth`, so a
/// style file never lacks the declaration of a length its document uses.
///
/// The point of a presentation [`Length`] is 1/72 inch, which LaTeX calls a
/// big point, `bp`;
/// LaTeX's own `pt` is 1/72.27 inch and is never written for it. The amount
/// is written in the decimal the length projects as, the shortest that reads
/// back to the same exact amount, so the output is the same wherever it is
/// produced.
///
/// # Errors
///
/// Returns [`LowerError`] when the node could not be built, or when the
/// calling rule did not declare that it produces `flm.pres.latex.raw`.
pub fn stage_length(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    length: &Length,
) -> Result<BuildId, LowerError> {
    if length.unit == LengthUnit::Tabs {
        lw.require(&Name::from(TAB_WIDTH));
    }
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(latex_length(length)), Vec::new())
}

/// `length` spelled as a LaTeX dimension, as [`stage_length`] writes it.
fn latex_length(length: &Length) -> String {
    let amount = length.amount_decimal();
    match length.unit {
        LengthUnit::Tabs => format!("{amount}\\{TAB_WIDTH_LENGTH}"),
        LengthUnit::Points => format!("{amount}bp"),
        LengthUnit::Ems | LengthUnit::Millimeters | LengthUnit::Centimeters | LengthUnit::Inches => {
            length.to_string()
        }
    }
}

/// The identifier of the piece `name`: [`ID_PREFIX`] followed by the name.
pub fn piece_id(name: &str) -> Name {
    Name::from(format!("{ID_PREFIX}{name}"))
}

/// The prefix of the identifier of a definition piece derived from a chunk of
/// the Unicode-to-LaTeX table, `flm.latex.def.untechxt.`: it begins with
/// [`ID_PREFIX`], so the identifier is read as a piece identifier everywhere
/// ([`split_requirements`]), and the part after [`ID_PREFIX`] says where the
/// piece comes from.
pub const CHUNK_ID_PREFIX: &str = "flm.latex.def.untechxt.";

/// The identifier of the piece derived from the chunk whose identifier is
/// `chunk_id`: [`CHUNK_ID_PREFIX`] followed by it
/// (`flm.latex.def.untechxt.bbold-alphabet`).
pub fn chunk_piece_id(chunk_id: &str) -> Name {
    Name::from(format!("{CHUNK_ID_PREFIX}{chunk_id}"))
}

/// The command inline verbatim content is written through, `EscVerb`, which
/// the `fvextra` package defines (version 1.5, of 2022, and later).
pub const ESCVERB: &str = "EscVerb";

/// The heading hook, `flmHeading`, which the [heading
/// piece](heading_piece) defines.
pub const HEADING_COMMAND: &str = "flmHeading";

/// The labels hook, `flmLabels`, which the [labels piece](labels_piece)
/// defines.
pub const LABELS_COMMAND: &str = "flmLabels";

/// The display-text hook, `flmLabelsWithDisplayText`, which the [display-text
/// piece](label_display_text_piece) defines.
pub const LABELS_WITH_DISPLAY_TEXT_COMMAND: &str = "flmLabelsWithDisplayText";

/// The command of the `hyperref` package that places an anchor where there is
/// no sectioning command, `phantomsection`: written before the labels of a
/// place that LaTeX does not number and no sectioning command anchors (a
/// definition of a term, an unnumbered float), so that a link to those labels
/// leads there ([`label_display_text_piece`]).
pub const PHANTOMSECTION_COMMAND: &str = "phantomsection";

/// The reference hook, `flmRef`, which the [references
/// piece](refs_piece) defines.
pub const REF_COMMAND: &str = "flmRef";

/// The hook a reference displaying the author's own words is written
/// through, `flmHyperref`, which the [hyperref piece](hyperref_piece)
/// defines.
pub const HYPERREF_COMMAND: &str = "flmHyperref";

/// The command of the `hyperref` package the [hyperref
/// piece](hyperref_piece) links through, `hyperref`: also what the
/// recomposed LaTeX writes in place of the hook where the render option
/// `label_hooks` is off.
pub const HYPERREF_PACKAGE_COMMAND: &str = "hyperref";

/// The private counter the [display-text piece](label_display_text_piece)
/// steps, `flm@internal@refcnt`, and the `zref-clever` reference type the
/// [type piece](zref_clever_type_piece) maps it to,
/// `flm--internal--ref`. Both keep the names the Python implementation of FLM
/// gave them.
pub const INTERNAL_REF_COUNTER: &str = "flm@internal@refcnt";
/// See [`INTERNAL_REF_COUNTER`].
pub const INTERNAL_REF_TYPE: &str = "flm--internal--ref";

/// The kernel command that protects its argument from a command that changes
/// the case of the text around it (`\MakeUppercase`), `NoCaseChange`: the
/// `refs` feature's `protect` option writes it around a reference, and the
/// style file then requests [`packages::TEXTCASE`], which supplies it on every
/// kernel.
pub const NO_CASE_CHANGE: &str = "NoCaseChange";

/// The blank test a recomposed user definition's own blank test becomes,
/// `flmIfBlankTF`, which the [blank-test piece](if_blank_piece) defines.
///
/// The name is the LaTeX family's own: every name beginning with `flmU` is
/// left to the document's definitions ([`super::names`]), and nothing this
/// library writes may take a name there.
pub const BLANK_COMMAND: &str = "flmIfBlankTF";

/// The accessor that reads one argument of a recomposed user definition whose
/// arguments are handed over in one group, `flmArg`, which the
/// [packed-argument piece](user_args_piece) defines:
/// `\flmArg{<n>}{<arguments>}` gives the content of argument `<n>`, counting
/// from one.
///
/// The name is the LaTeX family's own, like [`BLANK_COMMAND`]: every name
/// beginning with `flmU` is left to the document's definitions
/// ([`super::names`]).
pub const ARG_COMMAND: &str = "flmArg";

/// The flag test of a recomposed user definition whose arguments are handed
/// over in one group, `flmIfArgTF`, which the [packed-argument
/// piece](user_args_piece) defines:
/// `\flmIfArgTF{<n>}{<arguments>}{<then>}{<else>}` asks LaTeX's own
/// `\IfBooleanTF` about argument `<n>`.
pub const IF_ARG_COMMAND: &str = "flmIfArgTF";

/// The blank test of a recomposed user definition whose arguments are handed
/// over in one group, `flmIfArgBlankTF`, which the [packed-argument
/// piece](user_args_piece) defines:
/// `\flmIfArgBlankTF{<n>,…}{<arguments>}{<then>}{<else>}` runs `<then>` where
/// the arguments listed hold nothing at all between them.
pub const IF_ARG_BLANK_COMMAND: &str = "flmIfArgBlankTF";

/// The macro that runs the per-kind formatting hook, `flmFmtVRB`, which the
/// [verbatim piece](verbatim_piece) defines.
pub const FMT_VRB: &str = "flmFmtVRB";

/// The environment a block of verbatim code is written as,
/// `flmVerbatimCode`, which the [verbatim piece](verbatim_piece) defines;
/// it takes the language tag as an optional argument.
pub const VERBATIM_CODE_ENVIRONMENT: &str = "flmVerbatimCode";

/// The environment a block of plain verbatim text is written as,
/// `flmVerbatimText`, which the [verbatim piece](verbatim_piece) defines.
pub const VERBATIM_TEXT_ENVIRONMENT: &str = "flmVerbatimText";

/// The command a block of verbatim content is written through where it
/// cannot be written out in full, `flmVerbatimBlock`, which the [verbatim
/// piece](verbatim_piece) defines:
/// `\flmVerbatimBlock[<options>]{<environment>}{<lines>}` sets the block
/// environment `<environment>` over content given in the escaped encoding
/// `\EscVerb` reads.
///
/// A verbatim block is written out in full — `\begin{flmVerbatimCode}[…]`,
/// the content on lines of its own, `\end{flmVerbatimCode}` — wherever it
/// reaches LaTeX as lines of the file, and through this command wherever it
/// reaches LaTeX inside a group LaTeX reads as tokens: a value group of a
/// call, the body a body-grabbing environment hands over, a definition's own
/// body in the style file. See [`verbatim_piece`] for what the three
/// arguments hold.
pub const VERBATIM_BLOCK_COMMAND: &str = "flmVerbatimBlock";

/// The name of the formatting hook of one kind of verbatim content, the
/// suffix the [verbatim piece](verbatim_piece) writes after `\flmFmtVRB`:
/// `verbtext`, `verbcode` or `verba`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::preamble::verbatim_hook;
/// use flm_core::stdcore::backends::pres::VerbatimKind;
///
/// assert_eq!(verbatim_hook(VerbatimKind::Code), "verbcode");
/// ```
pub fn verbatim_hook(kind: VerbatimKind) -> &'static str {
    match kind {
        VerbatimKind::Text => "verbtext",
        VerbatimKind::Code => "verbcode",
        VerbatimKind::Identifier => "verba",
    }
}

/// The optional argument written on `\EscVerb` for verbatim content of
/// `kind`: `formatcom=\flmFmtVRB{<hook>}`, which runs that kind's hook.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::preamble::escverb_options;
/// use flm_core::stdcore::backends::pres::VerbatimKind;
///
/// assert_eq!(escverb_options(VerbatimKind::Identifier), "formatcom=\\flmFmtVRB{verba}");
/// ```
pub fn escverb_options(kind: VerbatimKind) -> String {
    format!("formatcom=\\{FMT_VRB}{{{}}}", verbatim_hook(kind))
}

/// A definition piece: a unit of LaTeX definitions that a feature registers
/// under an identifier `flm.latex.def.<name>`, together with the identifiers
/// it needs in turn; a lowering rule requires it by identifier, and the style
/// file holds the text of every piece the document requires, in registration
/// order.
///
/// A feature builds a piece with [`new`](Self::new),
/// [`requires`](method@Self::requires) and, where the piece stands for a
/// style identifier, [`delivering`](Self::delivering), and contributes it
/// to the service slot [`SLOT`]; a lowering rule then requires the piece's
/// identifier (or a style identifier it delivers). Where several features
/// contribute a piece under one identifier, the piece of the feature placed
/// last in the feature order is the definition ([`definitions`],
/// [`piece_named`]). The LaTeX backends write the piece's
/// [`content`](Self::content) into the style file,
/// under a comment line that gives the identifier. The text follows the rules
/// of the [module documentation](self#how-a-piece-is-written): guarded
/// definitions, no test of the installation, comment lines saying what to
/// redefine, and names under `flm` but never under `flmU`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatexPreambleDef {
    /// The identifier, `flm.latex.def.<name>`.
    pub id: Name,
    /// The identifiers this piece needs in turn: package identifiers
    /// (`flm.latex.pkg.<name>`) for the packages its text calls into, which
    /// the style file requests beside the document's own requirements, and
    /// piece identifiers ([`ID_PREFIX`], `flm.latex.def.<name>`) for the
    /// pieces its text calls into, which the style file writes beside the
    /// pieces the document required. The two are told apart by their
    /// prefix ([`split_requirements`]), as a lowering rule's own requirements
    /// are. The `latex-recomposed` backend follows these requirements
    /// transitively, so a piece lists only what it needs directly.
    pub requires: Vec<Name>,
    /// The LaTeX text: complete lines, guarded definitions with comment
    /// lines saying what to redefine to change what.
    pub content: String,
    /// A short text, for a person reading the definition, saying what the
    /// piece defines and when a document needs it. It is not written into the
    /// style file, which gives the piece's identifier above its text.
    pub docs: String,
    /// The style identifiers this piece stands for on a LaTeX backend, empty
    /// by default: identifiers that are neither package nor piece identifiers,
    /// such as a feature's stylesheet identifier (`flm.theorems.style`),
    /// which a lowering rule requires once for every backend. The
    /// `latex-rendered` backend resolves such an identifier to every piece
    /// whose `delivers` lists it; the `latex-recomposed` backend never reads
    /// this field.
    pub delivers: Vec<Name>,
}

impl LatexPreambleDef {
    /// The piece `name` (its identifier is [`piece_id`]`(name)`) with the
    /// LaTeX text `content` and the description `docs`
    /// ([`docs`](field@Self::docs)), needing nothing.
    pub fn new(name: &str, content: impl Into<String>, docs: impl Into<String>) -> Self {
        LatexPreambleDef {
            id: piece_id(name),
            requires: Vec::new(),
            content: content.into(),
            docs: crate::kept_docs(docs),
            delivers: Vec::new(),
        }
    }

    /// The same piece needing the given identifiers ([`requires`](Self::requires)):
    /// packages, pieces, or both.
    pub fn requires(mut self, ids: impl IntoIterator<Item = impl Into<Name>>) -> Self {
        self.requires = ids.into_iter().map(Into::into).collect();
        self
    }

    /// The same piece standing for the given style identifiers on a LaTeX
    /// backend ([`delivers`](field@Self::delivers)): a requirement of any of
    /// them brings the piece into the style file of a `latex-rendered`
    /// document.
    pub fn delivering(mut self, ids: impl IntoIterator<Item = impl Into<Name>>) -> Self {
        self.delivers = ids.into_iter().map(Into::into).collect();
        self
    }
}

/// Every definition piece the registry holds, in contribution order.
pub fn pieces(services: &Services) -> impl Iterator<Item = &LatexPreambleDef> + '_ {
    services.services_named::<LatexPreambleDef>(SLOT)
}

/// The piece that defines each identifier, in contribution order: of the
/// pieces contributed under one identifier, the one of the feature placed last
/// in the feature order (of one feature's, its first), at its own place. One
/// piece contributed more than once is at the first of its places. The LaTeX
/// backends write the pieces a document requires in this order, one per
/// identifier.
pub fn definitions(services: &Services) -> Vec<&LatexPreambleDef> {
    let mut chosen: Vec<&LatexPreambleDef> = Vec::new();
    for piece in services.services_by_precedence_named::<LatexPreambleDef>(SLOT) {
        if !chosen.iter().any(|seen| seen.id == piece.id) {
            chosen.push(piece);
        }
    }
    let mut placed: BTreeSet<&Name> = BTreeSet::new();
    pieces(services)
        .filter(|piece| chosen.iter().any(|kept| std::ptr::eq(*kept, *piece)) && placed.insert(&piece.id))
        .collect()
}

/// The piece that defines the identifier `id`: of the pieces contributed
/// under it, the one of the feature placed last in the feature order (of one
/// feature's, its first), or `None` when no feature contributed one — which
/// a caller reports as an unknown requirement rather than fails on.
pub fn piece_named<'s>(services: &'s Services, id: &str) -> Option<&'s LatexPreambleDef> {
    services.services_by_precedence_named::<LatexPreambleDef>(SLOT).find(|def| def.id.as_str() == id)
}

/// Returns the definition piece [`REFS`], which defines the reference hook
/// `\flmRef`.
///
/// Defines [`REF_COMMAND`] (`\flmRef[<options>]{<label>}`), the LaTeX hook
/// every reference of the recomposed LaTeX is written with. Its default sets
/// the reference with `\zcref` of the `zref-clever` package, which prints the
/// name of its target the way FLM's `\ref` does ("Section 2"), unlike LaTeX's
/// own `\ref`, which prints a bare number.
///
/// # The options are FLM's, and the default translates them
///
/// A *reference option* is what a document writes in the brackets of
/// `\ref[…]{…}`, and the vocabulary is FLM's own: the key `noprefix`, which
/// turns off the name the reference puts before its target's number or title,
/// and a key written alone that gives the *prefix variant*, the spelling of
/// that name to use ([`ref_display_options`](crate::stdcore::features::refs::ref_display_options)).
/// The recomposed LaTeX writes those options as the document wrote them, and
/// **this default translates them** into `zref-clever`'s keys as it calls
/// `\zcref`:
///
/// | written in the document | handed to `\zcref` | what it asks for |
/// |---|---|---|
/// | `noprefix` | `noname` | print the target's number or title alone, with no name before it ("2", not "Section 2") |
/// | `S` | `S` | print the first name capitalized and unabbreviated, for a reference that opens a sentence; the two vocabularies spell it alike |
/// | `capital` — the variant of the capitalized words | `cap=true,abbrev=false` | print the names capitalized and unabbreviated ("Lemma 1") |
/// | `lowercase` — the variant of the lowercase words | `cap=false,abbrev=false` | print the names in lowercase and unabbreviated ("lemma 1") |
/// | `abbreviated` — the variant of the abbreviated words | `abbrev=true` | print the abbreviated names ("Lem. 1") where the reference type declares them, and the full names where it does not, which is the package's own fallback |
/// | any other key — a variant of a host's own naming | nothing | the option is dropped, and the reference prints the package's default wording |
///
/// The translation reads the **key** alone: a value written after it goes
/// nowhere, since FLM's own reader ignores it too (`\ref[noprefix=x]{…}`
/// turns the name off like the bare key). A configuration may declare a
/// variant under any name, and only the names the library ships have a
/// counterpart in the package. An option without one is dropped rather than
/// handed over, because `zref-clever` stops the compilation on a key it does
/// not know; FLM's other outputs likewise print a reference in its default
/// wording when its target does not declare the variant asked for.
///
/// The abbreviated words are the reference type's: the
/// [`theorems`](crate::stdcore::features::theorems) feature declares each
/// numbered kind's abbreviated words beside its full ones
/// ([`theorems::latex`](crate::stdcore::features::theorems::latex)).
///
/// The translation is made **here**, in the default definition, and nowhere
/// in the code that writes the recomposed LaTeX. A user who redefines `\flmRef` therefore
/// sees FLM's options as the document wrote them and maps them as their own
/// reference command wants them; and the render option
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
/// off, which writes `\zcref[…]{…}` in place of the hook, hands FLM's options
/// to `\zcref` untranslated — the options are then the user's to write in
/// `zref-clever`'s vocabulary, as everything the hooks did is.
///
/// To send references somewhere else — another package, a different
/// wording, no link — redefine `\flmRef`, before loading the style file or
/// after it. Nothing else in the recomposed LaTeX calls a reference command.
///
/// The piece is written in **expl3** ([`if_blank_piece`] says what that is and
/// why two lines surround the definitions), for two functions:
/// `\keyval_parse:NNn` reads an option list the way LaTeX reads any key-value
/// list, handing each key written alone to one macro and each key with its
/// value to another, and `\str_case:nnF` looks the key up in the table above,
/// doing nothing for a key the table does not hold. The dropping is this
/// filter, in the hook, rather than a handler of unknown keys set on
/// `zref-clever`'s own option set: such a handler would change the package's
/// vocabulary for every `\zcref` of the document, the author's own included.
/// The macros this piece defines beside the hook are its internals, not a
/// customization point.
///
/// Requires [`packages::ZREF_CLEVER`].
pub fn refs_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "refs",
        "\
% \\flmRef[<options>]{<label>}: a reference that gives the name of its target
% (\"Section 2\"), set by \\zcref of the zref-clever package.
%
% The options are the ones the document wrote, in FLM's own vocabulary. This
% definition translates each of them by its key -- a value written after the
% key is ignored, as in FLM -- and drops every key it has no translation for:
%
%   noprefix    -> noname                 the number or title alone, with no
%                                         name before it (\"2\", not
%                                         \"Section 2\")
%   S           -> S                      the first name capitalized and
%                                         unabbreviated, for a reference that
%                                         opens a sentence
%   capital     -> cap=true,abbrev=false  the names capitalized (\"Lemma 1\")
%   lowercase   -> cap=false,abbrev=false the names in lowercase (\"lemma 1\")
%   abbreviated -> abbrev=true            the abbreviated names (\"Lem. 1\")
%                                         where the reference type declares
%                                         them, the full names otherwise
%   anything else -> dropped              the default wording; a variant of
%                                         your own naming has no counterpart
%                                         in zref-clever, which would stop on
%                                         a key it does not know
%
% Redefine \\flmRef to set references differently -- another package, another
% wording, your own variants, no link. Your definition sees the options as the
% document wrote them, in FLM's vocabulary, and maps them as it likes.
%
% The translation uses expl3, LaTeX's programming layer, which is part of the
% format and needs no package; its names read only between \\ExplSyntaxOn and
% \\ExplSyntaxOff. \\keyval_parse:NNn reads the option list as LaTeX reads any
% key-value list: it hands a key written alone to the first macro below and a
% key with its value to the second, which drops the value. \\str_case:nnF looks
% the key up in the table and appends what \\zcref is to be given, or nothing.
% The macros with @ in their names are internals, not a customization point:
% redefine \\flmRef, not its parts.
\\ExplSyntaxOn
\\def\\flm@ref@add#1{\\tl_put_right:Nn \\flm@ref@options {#1,}}
\\def\\flm@ref@key#1{\\str_case:nnF {#1}
  {{noprefix}{\\flm@ref@add{noname}}
   {S}{\\flm@ref@add{S}}
   {capital}{\\flm@ref@add{cap=true,abbrev=false}}
   {lowercase}{\\flm@ref@add{cap=false,abbrev=false}}
   {abbreviated}{\\flm@ref@add{abbrev=true}}}
  {}}
\\def\\flm@ref@pair#1#2{\\flm@ref@key{#1}}
\\def\\flm@ref@zcref#1#2{\\zcref[#1]{#2}}
\\ProvideDocumentCommand\\flmRef{O{} m}{%
  \\def\\flm@ref@options{}%
  \\keyval_parse:NNn \\flm@ref@key \\flm@ref@pair {#1}%
  \\exp_args:No \\flm@ref@zcref {\\flm@ref@options} {#2}}
\\ExplSyntaxOff
",
        "`\\flmRef[options]{label}`, the LaTeX hook every reference is written with: sets the \
         reference with `\\zcref`, translating the reference options into those of \
         `zref-clever`; needed by every reference",
    )
    .requires([packages::ZREF_CLEVER])
}

/// The hook of a reference whose displayed words are the author's own, the
/// definition piece [`HYPERREF`].
///
/// Defines [`HYPERREF_COMMAND`] (`\flmHyperref{<label>}{<text>}`), the LaTeX
/// hook every `\hyperref` of the recomposed LaTeX is written with: it shows
/// `<text>` and links it to the place `<label>` refers to.
///
/// # Why the default asks whether the document loaded `hyperref`
///
/// Making a link in a PDF takes the `hyperref` package, and loading it
/// changes a whole document: every reference becomes a link, bookmarks are
/// built, and the package wants a place of its own among the document's other
/// packages. Whether a document is compiled with it is therefore the author's
/// decision and not this library's, and the recomposed LaTeX leaves it to
/// them: this piece requires no package, and its default links through
/// `\hyperref` where the document loaded that package and prints the text
/// alone where it did not. Either way the document compiles, and either way
/// the author's words appear.
///
/// This is the only question the family's pieces ask that changes what a
/// piece does, and it is a question about the **document's own preamble**,
/// not about the installation: one document therefore still reads the same
/// way wherever it is compiled, as the rule against testing the installation
/// requires. `\IfPackageLoadedTF` is the kernel's own way of
/// asking it, available since the LaTeX of 2021 and so within the oldest
/// LaTeX this family writes for; it is asked at the reference, so the answer
/// is right whether the style file was loaded before `hyperref` or after it.
///
/// The piece also says what such a reference contributes to a **PDF
/// bookmark**, which is plain text and can carry no link: the text alone,
/// through `hyperref`'s `\pdfstringdefDisableCommands`, as the verbatim piece
/// does for verbatim content ([`verbatim_piece`]). Without it the label would
/// appear in the bookmark beside the text.
///
/// To link differently — another package, a footnote, no link at all —
/// redefine `\flmHyperref`, before loading the style file or after it.
///
/// The render option
/// [`label_hooks`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::label_hooks)
/// off writes `\hyperref[{<label>}]{<text>}` in place of the hook and
/// requests [`packages::HYPERREF`], the package that defines it: the choice
/// the hook leaves to the author is then made, and, as with everything the
/// hooks did, the author is responsible for it.
pub fn hyperref_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "hyperref",
        "\
% \\flmHyperref{<label>}{<text>}: <text>, shown as a link to the place <label>
% refers to.
%
% Making a link in a PDF takes the hyperref package, and loading it changes a
% whole document -- every reference becomes a link, bookmarks are built. That
% is the author's decision, not this file's, so this file neither loads
% hyperref nor asks for it: the definition below links through it where the
% document loaded it and prints the text alone where it did not, so the
% document compiles either way. The question is about this document's own
% preamble, so the answer is the same wherever the document is compiled; it is
% asked here, at the reference, so it is right whether this file was loaded
% before hyperref or after it.
%
% Redefine \\flmHyperref to link differently -- another package, a footnote
% with the target's number, no link at all.
\\ProvideDocumentCommand\\flmHyperref{m m}{%
  \\IfPackageLoadedTF{hyperref}{\\hyperref[{#1}]{#2}}{#2}}
% What such a reference contributes to a PDF bookmark: the text alone. A
% bookmark is plain text and carries no link, and without this line the label
% would appear in it beside the text. The hook runs when the document begins
% and does nothing where hyperref is absent.
\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands
  \\pdfstringdefDisableCommands{%
    \\def\\flmHyperref#1#2{#2}%
  }\\fi}
",
        "`\\flmHyperref{label}{text}`, the LaTeX hook of a reference that shows the author's own \
         words: a link through `hyperref` where the document loaded it, the text alone \
         otherwise; needed by such references",
    )
}

/// The only command that makes labels refer to the current place with a
/// given text as what a reference to them prints, the definition piece
/// [`LABEL_DISPLAY_TEXT`].
///
/// Defines [`LABELS_WITH_DISPLAY_TEXT_COMMAND`]
/// (`\flmLabelsWithDisplayText{<text>}{<labels>}`) and the private counter
/// [`INTERNAL_REF_COUNTER`] it steps.
///
/// A LaTeX label records what the last numbered thing was: `\label` stores
/// `\@currentlabel`, which a class sets when it steps a counter. Something
/// that is not numbered — a starred heading, a heading below the class's
/// numbering depth, a defined term — has no number for a label to record, so
/// a reference to it would print the number of the last numbered item. This macro
/// gives such a place a label of its own: it steps a counter kept for the
/// purpose and sets `\@currentlabel` to `<text>`, then runs `<labels>`,
/// which records it. Everything happens inside a group, so nothing else in
/// the document is touched.
///
/// `<text>` is expanded to make `\@currentlabel`, and `\label` does not
/// survive that expansion: a heading's title or a defined term holding a
/// labeled footnote would stop the LaTeX run. So, for the label's text only,
/// a `\label` inside `<text>` is swallowed (`\let\label\@gobble` in a group
/// of its own) and the text is expanded into a private macro that
/// `\@currentlabel` is then set to. The footnote's own label is not lost: it
/// is written at the footnote's place, and the labels of this place are the
/// ones `<labels>` writes beside it.
///
/// It places **no anchor of its own**, so that a link leads to the heading's
/// title. Under `hyperref` a sectioning command, starred or not, already
/// places an anchor at the title and records it in `\@currentHref`; a
/// `\label` after it therefore points at the title. `\refstepcounter`,
/// LaTeX's usual interface, would place a *second* anchor here, below the
/// title, and point `\@currentHref` at it, so a link would lead below the
/// heading. So the counter is stepped with the kernel's plain `\stepcounter`
/// and `\@currentlabel` and `\@currentcounter` are set here — the two
/// things `\refstepcounter` does that this macro needs, and neither of them
/// an anchor. `\@currentcounter` is what `zref-clever` reads to give the
/// label the reference type [`zref_clever_type_piece`] sets up.
///
/// A caller that has no sectioning command before it (for example a defined
/// term in running text) has no anchor to reuse, and a link to it would lead
/// to the last anchor placed before it. Such a caller writes
/// `\phantomsection` ([`PHANTOMSECTION_COMMAND`]) at its own place, which is
/// `hyperref`'s own way of placing an anchor where there is no sectioning
/// command.
///
/// [`heading_piece`] uses it for an unnumbered heading; anything else that
/// needs a label with a display text uses the same macro. Redefine it to
/// change how such labels are made.
///
/// What a reference to such a label *prints* is [`zref_clever_type_piece`]'s
/// part.
pub fn label_display_text_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "label-display-text",
        "\
% \\flmLabelsWithDisplayText{<text>}{<labels>}: makes <labels> refer to this
% place, with <text> as what a reference to them prints. It is the only
% command for that: a heading the class did not number uses it, and so does
% anything else that has a name but no number.
%
% It places no anchor of its own. Under hyperref a sectioning command, starred
% or not, already places one at the title; a \\label after it points there.
% \\refstepcounter would place a second anchor here, below the title, and a
% link would lead below the heading. So the counter is stepped with
% the plain \\stepcounter and the two things this needs are set here:
% \\@currentlabel, which is what a \\label records, and \\@currentcounter,
% which is what zref-clever reads to give the label its reference type. The
% group keeps every change local.
%
% <text> is expanded to make \\@currentlabel, and \\label does not survive that
% expansion; so a \\label inside <text> (for example a footnote's) is dropped
% from the label's text only. At its own place in the document it still makes
% its own label.
%
% A caller with no sectioning command before it -- for example a defined term
% in running text -- writes \\phantomsection at its own place, so that a link
% has an anchor of its own to lead to.
%
% Redefine \\flmLabelsWithDisplayText to change how such labels are made.
\\ifcsname c@flm@internal@refcnt\\endcsname\\else\\newcounter{flm@internal@refcnt}\\fi
\\ProvideDocumentCommand\\flmLabelsWithDisplayText{m m}{%
  \\begingroup
    \\stepcounter{flm@internal@refcnt}%
    {\\let\\label\\@gobble\\protected@xdef\\flm@lwdt@text{#1}}%
    \\let\\@currentlabel\\flm@lwdt@text
    \\def\\@currentcounter{flm@internal@refcnt}%
    #2%
  \\endgroup}
",
        "`\\flmLabelsWithDisplayText{text}{labels}`: makes labels refer to the current \
         place, with `text` as what a reference to them prints; needed by unnumbered headings, \
         defined terms and the labeled captions of unnumbered floats",
    )
}

/// What a reference to a label of [`label_display_text_piece`] prints, the
/// definition piece [`ZREF_CLEVER_TYPE`].
///
/// `zref-clever` prints a reference as a name and a number — "Section 2" —
/// choosing the name from the *reference type* of the counter the label
/// recorded. This piece gives the private counter
/// [`INTERNAL_REF_COUNTER`] a type of its own,
/// [`INTERNAL_REF_TYPE`] (`flm--internal--ref`), and sets that type's names
/// to nothing, so that `\zcref` to such a label prints its text and nothing
/// before it. Every other label keeps the type and the name it had.
///
/// The mapping is made once, for the counter, with `zref-clever`'s
/// `countertype` option; nothing has to be set or reset where a label is
/// written.
///
/// Required together with [`heading_piece`]: the two together make a
/// reference to an unnumbered heading print the heading's title.
///
/// Requires [`packages::ZREF_CLEVER`] and [`LABEL_DISPLAY_TEXT`].
pub fn zref_clever_type_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "zref-clever-type",
        "\
% The zref-clever reference type of the counter \\flmLabelsWithDisplayText
% steps, with every name prefix empty, so that a reference to such a label
% prints its text alone (\"Introduction\", not \"Section Introduction\").
% Every other label keeps the name its own type gives it. Redefine these
% settings, or the type of the counter, to put something before the text.
\\zcsetup{countertype={flm@internal@refcnt=flm--internal--ref}}
\\zcRefTypeSetup{flm--internal--ref}{Name-sg = , name-sg = , Name-pl = , name-pl = ,}
",
        "the `zref-clever` reference type with which a reference to a label made by \
         `\\flmLabelsWithDisplayText` prints its text alone; needed by unnumbered headings, \
         defined terms and the labeled captions of unnumbered floats",
    )
    .requires([packages::ZREF_CLEVER, LABEL_DISPLAY_TEXT])
}

/// Returns the definition piece [`LABELS`], which defines the labels hook
/// `\flmLabels` and makes every label on a line of mathematics resolve.
///
/// Defines [`LABELS_COMMAND`] (`\flmLabels{<labels>}`), the LaTeX hook written
/// around every group of labels the recomposed LaTeX holds.
///
/// In text it runs them as written: LaTeX handles any number of `\label`s in
/// a row, all recording the same thing.
///
/// On a line of a display-math environment it does more. `amsmath` keeps one
/// label per line — the last one — and loses the ones before it: given two,
/// it warns "Multiple \\label's: label 'x' will be lost", and a reference to
/// the lost one prints `??`. So the macro
/// runs the first label as written and records every further one as an
/// *alias* of it: after the last page, one line per alias is written to the
/// auxiliary file, and on the next run — after every real label has been
/// read from that file — each line makes the alias name refer to what the
/// first label refers to, for LaTeX's own references and for
/// `zref-clever`'s alike. Two runs resolve it, as two runs resolve any
/// LaTeX label. Nothing of `amsmath` is touched.
///
/// Redefine `\flmLabels` to change what a group of labels does;
/// `\def\flmLabels#1{#1}` gives plain LaTeX behavior back, `amsmath`'s
/// message included.
///
/// The line is written from the kernel's `enddocument/afterlastpage` hook,
/// which runs after the last page has been shipped out and so after every
/// `\newlabel` of that page has reached the auxiliary file;
/// `\AtEndDocument` would run before them. The alias is made with a `\global`
/// assignment because the auxiliary file is read inside a group.
///
/// The piece needs nothing: the aliasing is plain LaTeX, and a group of
/// labels that needs a display text is wrapped by whoever knows the text
/// ([`heading_piece`]), which requires [`label_display_text_piece`] itself.
pub fn labels_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "labels",
        "\
% \\flmLabels{<\\label commands>}: the labels of one place in the document.
% In text it runs them as written. On a line of a display-math
% environment, where amsmath keeps the last label of a line and loses the
% ones before it, it runs the first and records every further one as an alias:
% a line per alias is written to the .aux file after the last page and read on
% the next run, making the alias refer to what the first label refers to
% (for \\ref and for \\zcref alike). Redefine \\flmLabels to change what a
% group of labels does; \\def\\flmLabels#1{#1} restores plain LaTeX behavior,
% amsmath's \"Multiple \\label's\" message included.
\\ProvideDocumentCommand\\flmLabels{m}{%
  \\ifmmode\\expandafter\\flm@labels@math\\else\\expandafter\\@firstofone\\fi{#1}}
% Reading the group: the first \\label as written, the rest as aliases of it.
\\def\\flm@labels@math#1{\\def\\flm@lbl@first{}\\flm@lbl@scan#1\\flm@lbl@stop}
\\def\\flm@lbl@scan{\\@ifnextchar\\label\\flm@lbl@take\\flm@lbl@rest}
\\def\\flm@lbl@take\\label#1{%
  \\ifx\\flm@lbl@first\\@empty
    \\def\\flm@lbl@first{#1}%
    \\label{#1}%
  \\else
    \\expandafter\\flm@recordalias\\expandafter{\\flm@lbl@first}{#1}%
  \\fi
  \\flm@lbl@scan}
\\def\\flm@lbl@rest#1\\flm@lbl@stop{#1}
% Collecting the aliases. A name is recorded once however often the line is
% typeset (amsmath measures a line before setting it).
\\def\\flm@aliaslist{}
\\def\\flm@recordalias#1#2{%
  \\ifcsname flm@alias@#2\\endcsname\\else
    \\expandafter\\gdef\\csname flm@alias@#2\\endcsname{}%
    \\g@addto@macro\\flm@aliaslist{\\flm@alias@entry{#2}{#1}}%
  \\fi}
% Writing them, from the hook that runs after the last page has been shipped
% out: every \\newlabel of that page has reached the .aux file by then, so the
% alias lines come after them. \\AtEndDocument would run too early.
\\def\\flm@alias@write#1#2{\\immediate\\write\\@auxout{\\string\\flm@aliaslabel{#1}{#2}}}
\\AddToHook{enddocument/afterlastpage}{\\let\\flm@alias@entry\\flm@alias@write\\flm@aliaslist}
% Reading them back: the alias refers to what the original refers to. The
% assignments are global because the .aux file is read inside a group.
\\def\\flm@aliaslabel#1#2{%
  \\ifcsname r@#2\\endcsname
    \\global\\expandafter\\let\\csname r@#1\\expandafter\\endcsname\\csname r@#2\\endcsname
  \\fi
  \\ifcsname Z@R@#2\\endcsname
    \\global\\expandafter\\let\\csname Z@R@#1\\expandafter\\endcsname\\csname Z@R@#2\\endcsname
  \\fi}
",
        "`\\flmLabels{...}`, the LaTeX hook every group of labels is written with: makes every \
         label on a line of display mathematics resolve; needed by every label",
    )
}

/// Returns the definition piece [`HEADING`], which defines the heading hook
/// `\flmHeading`.
///
/// Defines [`HEADING_COMMAND`]
/// (`\flmHeading[<keys>]<command><star>{<title>}{<labels>}`), the LaTeX hook
/// written before every sectioning command of the recomposed LaTeX, on a line
/// of its own so that the sectioning command starts the next line:
/// `\flmHeading[toc=true]`, a newline,
/// `\section*{Title}{\flmLabels{\label{ref1}}}`. The newline is for the
/// reader; to LaTeX a command name followed by a newline is the same token
/// sequence, and the sectioning command is the hook's first mandatory
/// argument.
///
/// It runs the sectioning command exactly as written (the command the
/// recomposed LaTeX writes, starred if it is starred, with its title) and then
/// makes the heading's labels refer to it. If the class *numbered* the
/// heading, the labels are run as written and record its number, as they
/// would without the hook. If it did not — a starred heading, or a level
/// below the class's numbering depth — the labels are made to refer to the
/// heading with its **title** as what a reference to them prints
/// ([`label_display_text_piece`]), so that a reference reads "Introduction"
/// instead of the number of the last numbered item.
///
/// **How it knows, without patching anything.** Every LaTeX class numbers a
/// heading by stepping a counter through `\refstepcounter`, which records
/// the counter's name in the kernel's `\@currentcounter` (LaTeX 2020-10 and
/// later; `hyperref`'s own `\refstepcounter` calls the kernel's). So the
/// macro empties `\@currentcounter`, runs the command, and looks: a name
/// there means the heading was numbered. This was compiled against
/// `article`, `report`, `book`, KOMA-Script, `memoir` and `titlesec`, with
/// and without `hyperref`, and holds on all of them, for starred headings
/// and for levels below `secnumdepth` alike.
///
/// With `hyperref`, the anchor a link to such a label jumps to is placed
/// where the labels are made, which is just after the heading: a link leads
/// to a point a line or so below the title.
///
/// # The optional argument: a list of keys
///
/// The hook takes a key-value list of its own, written **on the hook** and
/// never on the sectioning command: `\flmHeading[alt={Short title}]`, a
/// newline, `\section{A long and \emph{fancy} title}{\flmLabels{}}`. There are
/// two keys.
///
/// - **`toc`** — `true` or `false`: whether the heading is listed in the table
///   of contents. The recomposed LaTeX writes it on every heading. Absent, the
///   heading is listed as LaTeX lists it, as in a document written by hand.
///   After a **starred** sectioning command with `toc=true` — which LaTeX
///   itself never lists — the hook writes
///   `\addcontentsline{toc}{<level>}{<entry>}`, `<level>` the sectioning
///   command's own name (`section` for `\section`) and `<entry>` the
///   alternative title where `alt` was written, else the title. Under
///   `hyperref` the starred command has already placed its anchor, so the
///   contents line links to the title. On an **unstarred** heading `toc=false`
///   is not honored: the heading stays listed as LaTeX lists it, and the hook
///   emits a `\PackageWarning` saying so. `toc=true` on an unstarred heading,
///   and `toc=false` on a starred one, change nothing.
///
/// - **`alt`** — an *alternative title*: the shorter or plainer form of the
///   title that LaTeX puts in the table of contents, in the running head and
///   in the PDF bookmark, while the page keeps the full title. The hook relays
///   it as the sectioning command's own optional argument, so `alt` does
///   exactly what the optional argument of `\section` does and nothing more.
///   A **starred** heading does not hand the key to the sectioning command,
///   because a starred sectioning command takes no optional argument — and
///   handing one to `\section*` is not even an error: the command takes one
///   token as its heading, so `\section*[Short]{Long}` sets the opening
///   bracket alone as the heading and runs the rest — `Short`, the closing
///   bracket and `Long` alike — together into the following paragraph. Where
///   a starred heading is written `toc=true`, `alt` is its line in the table
///   of contents (see `toc` above). Because the keys are
///   written on the hook, a starred and an unstarred heading have the same
///   form.
///
/// **What counts is whether the key was written, not what it holds.**
/// `\flmHeading[alt={Short}]` relays `[Short]`; `\flmHeading[alt={}]` relays
/// `[]`, which is LaTeX's own way of asking for an *empty* table-of-contents
/// entry; and `\flmHeading` with no optional argument, or
/// with an empty one (`\flmHeading[]`), relays nothing at all and the
/// sectioning command is written exactly as it was before. Writing `alt`
/// without a value is LaTeX's own error, the key being declared as requiring
/// one.
///
/// An **unnumbered** heading's labels still take the *full* title as what a
/// reference to them prints, not the alternative one: `alt` is what the
/// sectioning command's optional argument does and nothing else, and that
/// argument never reaches a label's display text. So a reference to an
/// unnumbered heading reads what the page reads.
///
/// The list is read with the key-value machinery of LaTeX's own programming
/// layer; the values and the record that `alt` was written are cleared at
/// every call, so one heading's keys never reach the next. A key this piece
/// does not define is LaTeX's own error, reported at the heading that wrote
/// it; nothing in this library checks the list.
///
/// The recomposed LaTeX writes `alt` where the heading was given an
/// alternative title (the heading key `alt=`), as it writes `toc`; both keys
/// are also there for a document written by hand over a recomposed style file
/// and for a user who redefines the hook. FLM does not build a plain-text form
/// of the title for the PDF bookmark.
///
/// Redefine `\flmHeading` to change what a heading does. The render option
/// [`heading_hook`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::heading_hook)
/// keeps it out of the recomposed LaTeX altogether; the user must then
/// provide the display text of an unnumbered heading.
///
/// Requires [`LABELS`], [`LABEL_DISPLAY_TEXT`] and [`ZREF_CLEVER_TYPE`].
pub fn heading_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "heading",
        "\
% \\flmHeading[<keys>]<sectioning command><star>{<title>}{<labels>}: a heading
% and the labels that refer to it, written on two lines for readability, as in
%   \\flmHeading[toc=true]
%   \\section*{Title}{\\flmLabels{\\label{ref1}}}
% It runs the sectioning command as written, then makes the labels refer to
% the heading: by its number when the class numbered it, by its title when it
% did not (a starred heading, or a level below secnumdepth). It finds out
% which without patching anything: a class numbers a heading through
% \\refstepcounter, which records the counter's name in \\@currentcounter.
%
% The optional argument is a key-value list. It is written here, on the hook,
% and never on the sectioning command: a starred sectioning command takes no
% optional argument, and giving one to \\section* is not an error, it is
% silently wrong -- \\section*[Short]{Long} takes one token as its heading,
% so the opening bracket alone becomes the heading and Short, the closing
% bracket and Long all run together into the following paragraph. Two keys
% are defined:
%
%   toc=true|false     whether the heading is listed in the table of contents.
%                      Absent, it is listed as LaTeX lists it. After a starred
%                      sectioning command with toc=true, which LaTeX never
%                      lists, \\addcontentsline{toc}{<level>}{<entry>} adds it:
%                      <level> is the command's own name (section for
%                      \\section), <entry> the alternative title if alt was
%                      written, else the title; under hyperref the starred
%                      command has already placed the anchor the line links
%                      to. On an unstarred heading toc=false is not honored:
%                      the heading stays listed and a warning says so.
%
%   alt={Short title}  an alternative title: what goes into the table of
%                      contents, the running head and the PDF bookmark, while
%                      the page keeps the full title. It is handed to the
%                      sectioning command as its own optional argument, so it
%                      does what \\section[Short title]{Full title} does, and
%                      nothing else: a reference to an unnumbered heading still
%                      prints the full title. A starred heading does not hand
%                      it to the sectioning command; where it is written
%                      toc=true, alt is its line in the table of contents.
%
% What counts is whether the key was written, not what it holds. alt={} is an
% empty alternative title and is handed over as \\section[]{Full title}, which
% is how LaTeX asks for an empty table-of-contents entry; no optional argument
% at all, and an empty one ([]), hand nothing over and leave the sectioning
% command exactly as it was. Writing alt with no value is LaTeX's own error,
% as is a key that is not defined here, and a value of toc other than true or
% false. The values and the record that alt was written are cleared at every
% call, so one heading's keys never reach the next. The recomposed LaTeX writes
% toc on every heading and alt where the heading was given an alternative
% title; both keys are also there for a document written by hand and for a
% user who redefines the hook.
%
% Redefine \\flmHeading to change what a heading does. The block below uses
% expl3, LaTeX's programming layer, which is part of the format and needs no
% package; its names read only between \\ExplSyntaxOn and \\ExplSyntaxOff.
% \\keys_define:nn declares the keys and \\keys_set:nn reads a list of them;
% \\bool_if:NTF asks whether the key was written. The three declarations are
% guarded on their own names, as every declaration in this file is, so that two
% of these style files can be loaded in one LaTeX run. The macros with @ in
% their names are internals, not a customization point: redefine \\flmHeading,
% not its parts.
\\ExplSyntaxOn
\\ifcsname flm@heading@alt\\endcsname\\else\\tl_new:N \\flm@heading@alt\\fi
\\ifcsname flm@heading@given\\endcsname\\else\\bool_new:N \\flm@heading@given\\fi
\\ifcsname flm@heading@toc\\endcsname\\else\\tl_new:N \\flm@heading@toc\\fi
\\keys_define:nn {flm/heading}
  {alt .code:n = {\\tl_set:Nn \\flm@heading@alt {#1}
                  \\bool_set_true:N \\flm@heading@given} ,
   alt .value_required:n = true ,
   toc .choices:nn = {true, false} {\\tl_set_eq:NN \\flm@heading@toc \\l_keys_choice_tl} ,
   toc .value_required:n = true}
\\def\\flm@heading@withalt#1#2#3{#2[#1]{#3}}
% The contents line of a starred heading: <level> then <entry>.
\\def\\flm@heading@addtoc#1#2{\\addcontentsline{toc}{#1}{#2}}
% toc=false on an unstarred heading: the heading stays listed.
\\def\\flm@heading@notoc#1{\\PackageWarning{flm}{toc=false~on~the~unstarred~
  \\string#1~is~not~honored~by~the~default~\\string\\flmHeading;~the~heading~
  stays~in~the~table~of~contents}}
\\ProvideDocumentCommand\\flmHeading{O{} m s m m}{%
  \\tl_clear:N \\flm@heading@alt
  \\bool_set_false:N \\flm@heading@given
  \\tl_clear:N \\flm@heading@toc
  \\keys_set:nn {flm/heading} {#1}%
  \\def\\@currentcounter{}%
  \\IfBooleanTF{#3}
    {#2*{#4}%
     \\str_if_eq:VnT \\flm@heading@toc {true}
       {\\edef\\flm@heading@level{\\cs_to_str:N #2}%
        \\bool_if:NTF \\flm@heading@given
          {\\exp_args:NVV \\flm@heading@addtoc \\flm@heading@level \\flm@heading@alt}
          {\\exp_args:NV \\flm@heading@addtoc \\flm@heading@level {#4}}}}
    {\\str_if_eq:VnT \\flm@heading@toc {false} {\\flm@heading@notoc{#2}}%
     \\bool_if:NTF \\flm@heading@given
       {\\exp_args:NV \\flm@heading@withalt \\flm@heading@alt #2{#4}}
       {#2{#4}}}%
  \\ifx\\@currentcounter\\@empty
    \\flmLabelsWithDisplayText{#4}{#5}%
  \\else
    #5%
  \\fi}
\\ExplSyntaxOff
",
        "`\\flmHeading[toc=...,alt={...}]`, the LaTeX hook written before every sectioning \
         command: makes a reference to an unnumbered heading print its title, lists a starred \
         heading written with `toc=true` in the table of contents, and takes an alternative \
         title for the table of contents and the PDF bookmark; needed by every heading",
    )
    .requires([LABELS, LABEL_DISPLAY_TEXT, ZREF_CLEVER_TYPE])
}

/// The definitions that set verbatim content, built on the `fvextra`
/// package: the definition piece [`VERBATIM`].
///
/// Defines:
///
/// - `\flmFmtVRB{<kind>}`, which runs the LaTeX hook `\flmFmtVRB<kind>` (a
///   macro of the style file that a user redefines); the three hooks
///   `\flmFmtVRBverbtext` (nothing), `\flmFmtVRBverbcode`
///   (`\ttfamily`) and `\flmFmtVRBverba` (`\itshape`), one per kind of
///   verbatim content (`\verbtext`, `\verbcode`, `\verba`). Redefine a hook
///   to change how that kind is set; inline verbatim content is written as
///   `\EscVerb[formatcom=\flmFmtVRB{<kind>}]{<content>}`.
/// - the environment `flmVerbatimCode[<lang>]`, a `Verbatim` environment
///   (`fancyvrb`, loaded by `fvextra`) set with the `verbcode` hook; the
///   language tag is passed to the LaTeX hook `\flmVerbatimCodeSetup{<lang>}`,
///   which does nothing by default and is where a user attaches syntax
///   highlighting (the hook runs before the `Verbatim` environment opens,
///   so it must not open a group or an environment of its own); and the
///   environment `flmVerbatimText`, the same with the `verbtext` hook and
///   no language. Define either environment before loading the style file
///   to replace it.
///
///   The recomposed LaTeX **always writes the optional argument**, `[<lang>]` or
///   `[]`, never leaving it out. LaTeX looks for an optional argument by
///   reading ahead past spaces, and the line end after
///   `\begin{flmVerbatimCode}` is one: without the brackets that lookahead
///   swallows it, and `fancyvrb`, which needs the verbatim content to begin
///   on the next line, stops with "Extraneous input … between
///   \begin{flmVerbatimCode}[<key=value>] and line end". Because the
///   brackets are always written, every block has the same form, and a
///   user's own `flmVerbatimCode`, defined the natural way with an optional
///   argument, works too.
///
/// - `\flmVerbatimBlock[<options>]{<environment>}{<lines>}`
///   ([`VERBATIM_BLOCK_COMMAND`]), which sets one block of verbatim content
///   written as a single macro call. `<lines>` holds one brace group per line
///   of the block and, inside each line, one brace group per tab-separated
///   segment, each segment in the encoding `\EscVerb` reads. The command
///   un-escapes each segment with `fvextra`'s own un-escaper, joins the
///   segments of a line with a tab and the lines with a line feed, and hands
///   `\begin{<environment>}[<options>]` … `\end{<environment>}` to
///   `\scantokens`, which offers that text to TeX as if it were a file: the
///   environment is entered for real and `fancyvrb` reads its lines as it
///   reads them out of any file. The `latex-recomposed` output writes a block
///   this way wherever the block reaches LaTeX inside a group LaTeX reads as
///   tokens — a value group of a call, the body a body-grabbing environment
///   hands over, a definition's own body — and writes it out in full
///   everywhere else.
///
///   The command **leaves the mode it found**: a block is vertical material,
///   so called where a paragraph was under way it begins one again, and
///   called where LaTeX was already between paragraphs it does not. The first
///   half allows a block to end a section title (the link `hyperref` wraps
///   the table-of-contents entry in cannot be closed in vertical mode), and
///   the second keeps the two forms setting the same block, an empty line
///   included.
/// - two definitions inside `hyperref`'s `\pdfstringdefDisableCommands` hook,
///   which say what verbatim content contributes to a **PDF bookmark**: a
///   bookmark is plain text, so inline content gives its text and a block
///   gives its characters run together, with no line breaks and nothing
///   between the lines. The hook is called inside
///   `\AtBeginDocument{\ifdefined\pdfstringdefDisableCommands …}`, so the
///   piece neither requires `hyperref` nor cares whether it is loaded before
///   or after the style file.
///
/// Inline content is written with `\EscVerb`, which `fvextra` defines from
/// version 1.5 (2022) on; the piece does not define it and does not test for
/// it. An installation whose `fvextra` is older does not compile a
/// recomposed LaTeX holding verbatim content.
///
/// `\flmVerbatimBlock` needs no package of its own: `\scantokens` is an
/// e-TeX primitive, available in every engine LaTeX has run on since 2004,
/// and `\FVExtraDetokenizeREscVArg`, the un-escaper that `\EscVerb` itself
/// calls (so that inline content and a block share one escaping), comes with
/// `fvextra`.
///
/// Requires [`packages::FVEXTRA`].
pub fn verbatim_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "verbatim",
        "\
% Verbatim content. Inline content is written as
%   \\EscVerb[formatcom=\\flmFmtVRB{<kind>}]{<escaped content>}
% and block content as the flmVerbatimCode environment. \\EscVerb comes with
% fvextra 1.5 (2022) and later, which this file requires.
% \\flmFmtVRB{<kind>} runs the hook \\flmFmtVRB<kind>; redefine
% \\flmFmtVRBverbtext, \\flmFmtVRBverbcode or \\flmFmtVRBverba to change how
% that kind of verbatim content is set.
\\ProvideDocumentCommand\\flmFmtVRBverbtext{}{}
\\ProvideDocumentCommand\\flmFmtVRBverbcode{}{\\ttfamily}
\\ProvideDocumentCommand\\flmFmtVRBverba{}{\\itshape}
\\ProvideDocumentCommand\\flmFmtVRB{m}{\\csname flmFmtVRB#1\\endcsname}
% \\flmVerbatimCodeSetup{<lang>} runs at the start of a flmVerbatimCode
% environment with its language tag (empty when none was given); redefine
% it to attach syntax highlighting. It runs before the Verbatim environment
% opens, so it must not open a group or an environment of its own.
\\ProvideDocumentCommand\\flmVerbatimCodeSetup{m}{}
% flmVerbatimCode[<lang>]: a block of code, set with the verbcode hook. The
% recomposed LaTeX always writes the brackets, [<lang>] or [], because LaTeX's
% lookahead for an absent optional argument swallows the line end that the
% verbatim content must begin after. Define the environment before loading
% this file to replace it.
\\ProvideDocumentEnvironment{flmVerbatimCode}{O{}}{%
  \\flmVerbatimCodeSetup{#1}%
  \\VerbatimEnvironment
  \\begin{Verbatim}[formatcom=\\flmFmtVRB{verbcode}]}%
  {\\end{Verbatim}}
% flmVerbatimText[<lang>]: a block of plain verbatim text, set with the
% verbtext hook. It takes the same optional argument as flmVerbatimCode and
% ignores it, so that one command (\\flmVerbatimBlock below), which always
% writes the brackets, serves either environment.
\\ProvideDocumentEnvironment{flmVerbatimText}{O{}}{%
  \\VerbatimEnvironment
  \\begin{Verbatim}[formatcom=\\flmFmtVRB{verbtext}]}%
  {\\end{Verbatim}}
% \\flmVerbatimBlock[<options>]{<environment>}{<lines>}: one block of verbatim
% content written as a single macro call, for the places a block cannot be
% written out in full -- inside an argument of a call, inside the body a
% body-grabbing environment hands over, inside a definition's own body -- where
% LaTeX has read the characters long before anything can say they are verbatim.
% The three arguments:
%   <options>     the block environment's own optional argument, always
%                 written, [<lang>] or [];
%   <environment> the environment's name, flmVerbatimCode for a code block;
%   <lines>       one brace group per line of the block, and inside each line
%                 one brace group per tab-separated segment, every segment
%                 written in the encoding \\EscVerb reads (a backslash before
%                 every printable ASCII character that is not a letter or a
%                 digit). The line and segment structure is carried by braces
%                 rather than by characters because a line feed written into an
%                 auxiliary file comes back from it as a space, and a tab
%                 inside an argument is a space before anything can look at it;
%                 brace groups survive both.
% The command un-escapes every segment with fvextra's own un-escaper, joins the
% segments of a line with a tab and the lines with a line feed, and hands
% \\begin{<environment>}[<options>] ... \\end{<environment>} to \\scantokens,
% which offers that text to TeX as if it were a file: the environment is
% entered for real, and fancyvrb reads its lines the way it reads them out of
% any file. This is the only place this file uses \\scantokens, and it is the
% whole mechanism -- nothing is saved anywhere, no name is made up, nothing
% outlives the call. The \\begin and the \\end are written as plain characters
% (\\string\\begin\\string{...) because \\scantokens puts a space after every
% control word, and fancyvrb looks for the exact characters \\end{ on a line:
% written as control words they would read \\end {...} and the environment
% would never close.
% The call leaves the mode it found. A block is vertical material, so the
% environment leaves TeX in vertical mode regardless of what came before; a
% block that ends a section title would then leave hyperref's link open in
% vertical mode, which pdfTeX refuses on the run that reads the table of
% contents back. Called
% where a paragraph was under way, the command therefore begins one again
% (\\leavevmode); called where TeX was already in vertical mode, it does not,
% because starting a paragraph there would add an empty line the same block
% written out in full does not add.
\\begingroup
\\catcode`\\^^J=12 \\catcode`\\^^I=12
\\gdef\\flm@vrb@nl{^^J}\\gdef\\flm@vrb@tab{^^I}
\\endgroup
\\let\\flm@vrb@stop\\relax
\\let\\flm@vrb@leave\\relax
\\ProvideDocumentCommand\\flmVerbatimBlock{O{} m m}{%
  \\ifhmode\\let\\flm@vrb@leave\\leavevmode\\else\\let\\flm@vrb@leave\\relax\\fi
  \\begingroup
    \\gdef\\flm@vrb@acc{}\\def\\flm@vrb@lsep{}%
    \\flm@vrb@lines#3\\flm@vrb@stop
    \\xdef\\flm@vrb@src{%
      \\string\\begin\\string{#2\\string}[#1]\\flm@vrb@nl
      \\flm@vrb@acc\\flm@vrb@nl\\string\\end\\string{#2\\string}}%
  \\endgroup
  \\edef\\flm@vrb@restorenl{\\newlinechar=\\the\\newlinechar\\relax}%
  \\newlinechar=`\\^^J\\relax
  \\expandafter\\scantokens\\expandafter{\\flm@vrb@src}%
  \\flm@vrb@restorenl
  \\ifvmode\\flm@vrb@leave\\fi}
% The walk over the lines and their segments: one line group at a time, one
% segment at a time, the segments of a line joined with a tab and the lines
% with a line feed. \\FVExtraDetokenizeREscVArg is fvextra's own un-escaper,
% the macro \\EscVerb itself calls, so that inline verbatim content and a block
% share one escaping; it is also the only safe route for text that is not
% ASCII.
\\def\\flm@vrb@lines{\\@ifnextchar\\bgroup\\flm@vrb@line\\flm@vrb@linesdone}
\\def\\flm@vrb@line#1{%
  \\xdef\\flm@vrb@acc{\\flm@vrb@acc\\flm@vrb@lsep}\\let\\flm@vrb@lsep\\flm@vrb@nl
  \\def\\flm@vrb@ssep{}\\flm@vrb@segs#1\\flm@vrb@stop\\flm@vrb@lines}
\\def\\flm@vrb@linesdone#1\\flm@vrb@stop{}
\\def\\flm@vrb@segs{\\@ifnextchar\\bgroup\\flm@vrb@seg\\flm@vrb@segsdone}
\\def\\flm@vrb@seg#1{\\FVExtraDetokenizeREscVArg{\\flm@vrb@addseg}{#1}\\flm@vrb@segs}
\\def\\flm@vrb@segsdone#1\\flm@vrb@stop{}
\\def\\flm@vrb@addseg#1{%
  \\xdef\\flm@vrb@acc{\\flm@vrb@acc\\flm@vrb@ssep#1}\\let\\flm@vrb@ssep\\flm@vrb@tab}
% What a PDF bookmark holds where a heading contains verbatim content. A
% bookmark is plain text: it can carry neither the setting of inline verbatim
% content nor a block at all, so hyperref is told what to put there instead.
% Inline content gives its text -- without this, fvextra disables \\EscVerb
% without removing its optional argument and the option list appears in the
% bookmark -- and a block gives its content run together, with no line breaks
% and no separator between the lines. The two definitions are in a hook that
% runs when the document begins and does nothing where hyperref is absent, so
% this file neither requires hyperref nor cares whether it is loaded before or
% after it.
\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands
  \\pdfstringdefDisableCommands{%
    \\def\\EscVerb[#1]#2{#2}%
    \\def\\flmVerbatimBlock[#1]#2#3{#3}%
  }\\fi}
",
        "`\\flmFmtVRB` and its per-kind LaTeX hooks, the environments `flmVerbatimCode` and \
         `flmVerbatimText`, and `\\flmVerbatimBlock`; needed by verbatim content",
    )
    .requires([packages::FVEXTRA])
}

/// The macro a display-math environment is written with when the `math`
/// feature's option asks for one: the definition piece [`MATH_ENVIRONMENT`].
///
/// Defines `\flmMathEnvironment{<env>}{<args>}{<body>}`, which by default
/// sets `\begin{<env>}<args><body>\end{<env>}`. It is defined only where
/// nothing of that name exists. Redefine it to set every display-math
/// environment differently, or to reach its name, arguments and body in one
/// place.
///
/// Requires [`packages::AMSMATH`], whose environments the macro may be
/// given.
pub fn math_environment_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "math-environment",
        "\
% \\flmMathEnvironment{<env>}{<args>}{<body>}: a display-math environment
% written as one macro call. Redefine it to set the environments differently.
\\ProvideDocumentCommand\\flmMathEnvironment{m m m}{\\begin{#1}#2#3\\end{#1}}
",
        "`\\flmMathEnvironment{env}{args}{body}`: a display-math environment written as one \
         macro call; needed where display mathematics is written that way",
    )
    .requires([packages::AMSMATH])
}

/// The blank test a recomposed user definition's own blank test becomes: the
/// definition piece [`IF_BLANK`].
///
/// Defines [`BLANK_COMMAND`] (`\flmIfBlankTF{<content>}{<then>}{<else>}`),
/// which runs `<then>` where `<content>` holds nothing at all and `<else>`
/// otherwise. A user definition whose template asks `\ifblank` or `\notblank`
/// about one of its arguments is recomposed with this macro in that test's
/// place.
///
/// The name is the family's own. No name beginning with `flmU` belongs to
/// this library: that whole name space is the document's, since a recomposed
/// user definition is written under a prefix beginning with it
/// ([`super::names`]). A document that declares a macro `blank` is recomposed
/// as `\flmUDblank`, which a helper of that name would collide with.
///
/// It is **long** and **expandable**, as `\providecommand` makes it. It is
/// long so that an argument holding a paragraph break, which a recomposed
/// definition's own arguments may hold, is tested rather than stopping the
/// compilation. It is expandable so that the test never stops the expansion
/// of a definition that has to expand: a recomposed definition is a plain
/// expandable macro, and a test inside its body that stopped expansion would
/// stop the definition's. `\providecommand` also guards the name: it defines
/// it only where nothing of that name exists, so two style files loaded in one
/// LaTeX run do not clash. It is not a customization point: a user changes
/// their own definition, not the test inside it.
///
/// It is written in **expl3**, the programming layer that has been part of the
/// LaTeX format since 2020 and needs no package: its names are readable only
/// between `\ExplSyntaxOn` and `\ExplSyntaxOff`, which is why those two lines
/// surround the definition. [`user_args_piece`] and [`refs_piece`] are
/// written that way as well, each saying so in its own comment.
/// The function used is `\tl_if_empty:nTF`, which is true exactly when its
/// argument holds no token at all — the condition FLM's own blank test asks.
/// The neighboring `\tl_if_blank:nTF` would be wrong: it calls a single space
/// blank, where FLM counts whitespace as content. A comment needs no thought
/// either way, because TeX discards a comment and the line end after it while
/// it reads the argument, so a comment never becomes a token.
///
/// This is the family's **only** blank test:
/// [`user_args_piece`]'s own blank test calls this command rather than
/// spelling the test again, and requires this piece for it. A style file that
/// carries that piece therefore always carries this one.
///
/// Requires nothing.
pub fn if_blank_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "ifblank",
        "\
% \\flmIfBlankTF{<content>}{<then>}{<else>}: the branch a user definition's own
% \"is this blank?\" test selects — <then> where <content> holds nothing at
% all, <else> otherwise. Not a customization point: change your own
% definition, not this. It is long, so that an argument holding a paragraph
% break is tested rather than stopping the compilation, and expandable, so
% that the test never stops the expansion of a definition that has to expand.
%
% This block uses expl3, LaTeX's programming layer, which
% is part of the format and needs no package; its names read only between
% \\ExplSyntaxOn and \\ExplSyntaxOff. \\tl_if_empty:nTF is true exactly when its
% argument holds no token at all, which is the condition FLM's own blank test
% asks: a single space is content, and a comment is gone before the argument
% exists.
\\ExplSyntaxOn
\\providecommand\\flmIfBlankTF[3]{\\tl_if_empty:nTF {#1} {#2} {#3}}
\\ExplSyntaxOff
",
        "`\\flmIfBlankTF{content}{then}{else}`: tests whether an argument is blank; needed \
         where one of the document's own definitions tests an argument that way",
    )
}

/// Reading one argument of a recomposed user definition that takes more than
/// nine: the definition piece [`USER_ARGS`].
///
/// A recomposed user definition of more than nine *effective arguments* — the
/// values a call writes plus the "was this argument written?" flags it writes,
/// which count together — cannot refer to them as `#1` to `#9`, since a LaTeX
/// macro body has nine parameters. Such a call hands every argument over in one
/// group instead, one pair of braces around each — `\flmUDbig{{a}{b}{c}…}` —
/// and the definition's body reads part *n* of that group through the three
/// macros this piece defines:
///
/// - [`ARG_COMMAND`] (`\flmArg{<n>}{<arguments>}`) — the content of argument
///   `<n>`, counting from one; a placeholder of that argument becomes this
///   call in the body;
/// - [`IF_ARG_COMMAND`] (`\flmIfArgTF{<n>}{<arguments>}{<then>}{<else>}`) —
///   LaTeX's own `\IfBooleanTF` asked about argument `<n>`, which holds
///   `\BooleanTrue` or `\BooleanFalse`;
/// - [`IF_ARG_BLANK_COMMAND`]
///   (`\flmIfArgBlankTF{<n>,…}{<arguments>}{<then>}{<else>}`) — the blank test
///   of [`if_blank_piece`], which a definition of nine effective arguments or
///   fewer uses directly, asked of the listed arguments together. It **calls**
///   that command rather than repeating the test, so that what "blank" means
///   is written in one place regardless of a definition's size.
///
/// All three are long and expandable, as `\providecommand` makes them, for
/// the reasons [`if_blank_piece`] gives: an argument may hold a paragraph
/// break, so a macro that reads one must be long, and a recomposed definition
/// is a plain expandable macro whose body must not stop expanding. They are
/// not a customization point.
///
/// The piece is written in **expl3** ([`if_blank_piece`] says what that is and
/// why two lines surround the definitions). `\tl_item:nn`, the kernel's own
/// function, picks part *n* of a token list, counting from one, and returns
/// its content with the braces around it removed, so the accessor needs
/// nothing else.
///
/// **A test never reads an argument directly.** Written
/// `\IfBooleanTF{\flmArg{2}{#1}}`, LaTeX stops with "Invalid argument", and
/// written `\flmIfBlankTF{\flmArg{2}{#1}}` it takes the wrong branch without
/// a word: both look at the tokens of the accessor itself rather than at the
/// argument it stands for. The two tests therefore expand the accessor before
/// testing it (`\exp_args:Ne`, which expands its argument completely). The
/// expansion leaves the argument's *own* content untouched, because
/// `\tl_item:nn` hands the content back protected from further expansion: a
/// space in an argument stays content, as FLM counts it, and a macro in one is
/// not expanded early.
///
/// `\exp_args:Ne` is built on the `\expanded` primitive, which every TeX
/// engine has carried since 2019, before the LaTeX of 2022 that this family's
/// LaTeX requires, so the piece needs no test of what is installed (see "No
/// test of the installation" in the [module
/// documentation](self#how-a-piece-is-written)).
///
/// Requires [`if_blank_piece`] ([`IF_BLANK`]), whose `\flmIfBlankTF` the blank
/// test above calls. A definition of more than nine effective arguments
/// therefore always brings the blank test's piece into the style file with
/// this one, whether or not it asks a blank test.
pub fn user_args_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "userargs",
        "\
% \\flmArg{<n>}{<arguments>}, \\flmIfArgTF{<n>}{<arguments>}{<then>}{<else>} and
% \\flmIfArgBlankTF{<n>,...}{<arguments>}{<then>}{<else>}: reading one argument of
% a user definition that takes more than nine of them. Such a definition is
% called with all its arguments in one group, one pair of braces around each
% ( \\flmUDbig{{a}{b}{c}...} ), because a macro body has nine parameters; its
% body reads part <n> of that group through these. \\flmArg gives the argument's
% content, \\flmIfArgTF asks whether the argument holds \\BooleanTrue, and
% \\flmIfArgBlankTF asks whether the listed arguments hold nothing at all between
% them. Not a customization point: change your own definition, not this. Long,
% so that an argument holding a paragraph break is read rather than stopping the
% compilation, and expandable, so that reading an argument never stops the
% expansion of a definition that has to expand.
%
% \\tl_item:nn is the LaTeX kernel's own: it picks part <n>, counting from one,
% and strips the braces around it. A test never reads a part directly --
% \\IfBooleanTF{\\flmArg{2}{#1}} stops with \"Invalid argument\" and
% \\flmIfBlankTF{\\flmArg{2}{#1}} answers the wrong branch without a word, since
% both look at the accessor's own tokens -- so the two tests expand the accessor
% first with \\exp_args:Ne, which needs the \\expanded primitive: every TeX engine
% has carried it since 2019, so it is older than the LaTeX this file asks for.
% What the part itself holds is left untouched: a space in an argument stays
% content, and a command in one is not expanded early.
%
% \\flmIfArgBlankTF asks its question through \\flmIfBlankTF, the block above,
% rather than spelling the test again: what \"blank\" means is written in one
% place regardless of a definition's size. A definition taking more than nine
% arguments therefore always brings that block into this file along with this
% one, whether or not it asks a blank test.
\\ExplSyntaxOn
\\providecommand\\flmArg[2]{\\tl_item:nn {#2} {#1}}
\\providecommand\\flmIfArgTF[4]{\\exp_args:Ne \\IfBooleanTF {\\tl_item:nn {#2} {#1}} {#3} {#4}}
\\providecommand\\flmIfArgBlankTF[4]{\\exp_args:Ne \\flmIfBlankTF {\\clist_map_tokens:nn {#1} {\\tl_item:nn {#2}}} {#3} {#4}}
\\ExplSyntaxOff
",
        "`\\flmArg{n}{arguments}`, `\\flmIfArgTF` and `\\flmIfArgBlankTF`: read one argument \
         of a document's own definition that takes more than nine; needed by such a \
         definition",
    )
    .requires([IF_BLANK])
}

/// The definition piece [`TAB_WIDTH`], `flm.latex.def.tab-width`: declares
/// the length [`TAB_WIDTH_LENGTH`], `\flmTabWidth`, the width of one tab, as
/// `2em` (the width the HTML output gives a tab by default, and the text
/// output's default of four columns at two columns per em). The declaration
/// is guarded on its name, so that a length of that name the user declared
/// before the style file is loaded is kept. Set it with
/// `\setlength{\flmTabWidth}{…}`, before loading the style file (after
/// declaring it) or after, to change every length written in tabs, among them
/// the indentation of a line of a quotation.
///
/// Requires nothing. The family's way of writing a length
/// ([`stage_length`]) requires it for a length in tabs.
pub fn tab_width_piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "tab-width",
        "\
% \\flmTabWidth: the width of one tab, the unit a length in tabs is written in
% (1\\flmTabWidth, 2\\flmTabWidth) -- the indentation of a line of a quotation.
% Change it with \\setlength{\\flmTabWidth}{...}.
\\ifcsname flmTabWidth\\endcsname\\else\\newlength{\\flmTabWidth}\\setlength{\\flmTabWidth}{2em}\\fi
",
        "`\\flmTabWidth`: the width of one tab, `2em` by default; needed where the recomposed \
         LaTeX writes a length in tabs, such as the indentation of a line of a quotation",
    )
}

/// The family's own definition pieces, in the order the `latex` feature
/// contributes them, which is also the order the style file writes them in:
/// [`REFS`], [`HYPERREF`], [`LABEL_DISPLAY_TEXT`], [`ZREF_CLEVER_TYPE`],
/// [`LABELS`], [`HEADING`], [`VERBATIM`], [`MATH_ENVIRONMENT`], [`IF_BLANK`],
/// [`USER_ARGS`], [`TAB_WIDTH`], followed by the pieces derived from the
/// Unicode-to-LaTeX table ([`chunk_pieces`]).
///
/// The order puts a piece before the pieces that call it, so that the style
/// file reads from the parts to the whole; LaTeX itself needs only the
/// package requests, which the style file writes before every piece.
///
/// # Panics
///
/// Where [`chunk_pieces`], which this calls, panics: when untechxt's builtin
/// table breaks one of its invariants, which no document input can cause.
pub fn standard_pieces() -> Vec<LatexPreambleDef> {
    let mut pieces = vec![
        refs_piece(),
        hyperref_piece(),
        label_display_text_piece(),
        zref_clever_type_piece(),
        labels_piece(),
        heading_piece(),
        verbatim_piece(),
        math_environment_piece(),
        if_blank_piece(),
        user_args_piece(),
        tab_width_piece(),
    ];
    pieces.extend(chunk_pieces());
    pieces
}

/// One piece per chunk of declarations of the Unicode-to-LaTeX table — every
/// chunk that is not a package ([`packages::is_package_chunk`]) — in the
/// order of [`packages::table_chunks`]: the piece
/// [`chunk_piece_id`]`(<chunk id>)`, `flm.latex.def.untechxt.<chunk id>`,
/// whose text is the chunk's declarations, guarded on the first name they
/// declare, and whose [`docs`](LatexPreambleDef::docs) text says that
/// characters of the document need the declarations and gives the chunk's
/// identifier.
///
/// A spelling of that table needs declarations no package makes — a math
/// alphabet read from a font family whose package would change the document's
/// own fonts, a symbol font with the symbols read from it — and the table
/// carries them as the chunk itself. This function turns each into a piece of
/// the style file, so that the same code that writes definitions into a style
/// file writes these too. The chunk order matters: the style file writes every
/// package request before any piece, and a piece written here calls into the
/// packages of the chunks that the same profile lists. A piece holds no test
/// of the engine: the declarations are the same under every engine.
///
/// Every command and font identifier these pieces declare carries untechxt's
/// own prefix, `UnxT` (`\UnxTBbold`, `UnxTwasy`), a name space the LaTeX
/// family neither owns nor touches ([`super::names`]). A recomposed user
/// definition is renamed under a prefix beginning with `flmU` unless the
/// author lists its name in the render option `bare_names`; only such a
/// listed name can replace what a character's spelling relies on.
///
/// # Panics
///
/// If a declaration chunk of the table is not one block of declarations for
/// every engine — more than one case, or a case not for every engine the
/// family knows ([`packages::ENGINES`]) — or declares no name the guard can
/// test: no math alphabet, math symbol, text symbol or symbol font. Both are
/// invariants of untechxt's builtin table, checked by the tests of this module
/// over the table as shipped; no document input can cause either panic.
pub fn chunk_pieces() -> Vec<LatexPreambleDef> {
    packages::table_chunks()
        .into_iter()
        .filter(|chunk| !packages::is_package_chunk(chunk))
        .map(|chunk| {
            let latex = match chunk.cases() {
                [case]
                    if packages::ENGINES
                        .iter()
                        .all(|engine| case.engines().contains(*engine)) =>
                {
                    match case.preamble() {
                        ChunkPreamble::Snippet(latex) => latex,
                        // A chunk whose one case loads a package is a package
                        // chunk, filtered out above.
                        _ => unreachable!("a declaration chunk's one case is declarations"),
                    }
                }
                _ => panic!(
                    "the chunk {} is not one block of declarations for every engine; a \
                     declaration is written with no test of the engine",
                    chunk.id()
                ),
            };
            let guard = guard_name(latex).unwrap_or_else(|| {
                panic!("the chunk {} declares no name to guard on", chunk.id())
            });
            let content = format!(
                "% Declarations some characters of the document need. Declared only when \
                 nothing of that name exists, so that two style files in one LaTeX run do not \
                 clash.\n\
                 \\ifcsname {guard}\\endcsname\\else\n{latex}\n\\fi\n"
            );
            LatexPreambleDef {
                id: chunk_piece_id(chunk.id()),
                requires: Vec::new(),
                content,
                docs: crate::kept_docs(format!(
                    "declarations needed by characters of the document whose LaTeX spelling \
                     uses them (chunk `{}` of untechxt's Unicode-to-LaTeX table)",
                    chunk.id()
                )),
                delivers: Vec::new(),
            }
        })
        .collect()
}

/// The name a snippet chunk's guard tests: the first name its declarations
/// define — a math alphabet, a math symbol or a text symbol by its own name, a
/// symbol font by the `\sym…` command LaTeX defines for it — or `None` when
/// the snippet declares no such name.
fn guard_name(latex: &str) -> Option<String> {
    for line in latex.lines() {
        for (prefix, sym) in [
            ("\\DeclareMathAlphabet{\\", false),
            ("\\DeclareMathSymbol{\\", false),
            ("\\DeclareTextSymbol{\\", false),
            ("\\DeclareSymbolFont{", true),
        ] {
            if let Some(rest) = line.strip_prefix(prefix) {
                let name = rest.split('}').next().unwrap_or_default();
                if !name.is_empty() {
                    return Some(if sym { format!("sym{name}") } else { name.to_string() });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::names::USER_DEFINITION_PREFIX;
    use super::*;

    #[test]
    fn a_length_is_spelled_as_a_latex_dimension() {
        assert_eq!(latex_length(&Length::tabs(2)), "2\\flmTabWidth");
        assert_eq!(latex_length(&Length::parse("1.5tab").unwrap()), "1.5\\flmTabWidth");
        assert_eq!(latex_length(&Length::parse("10pt").unwrap()), "10bp");
        for spelled in ["1.5em", "3mm", "2cm", "0.25in"] {
            assert_eq!(latex_length(&Length::parse(spelled).unwrap()), spelled);
        }
    }

    /// The piece's LaTeX without its comment lines: what it actually does,
    /// as opposed to what its comments say about it.
    fn definitions(content: &str) -> String {
        content.lines().filter(|line| !line.trim_start().starts_with('%')).collect::<Vec<_>>().join("\n")
    }

    /// The same, without `hyperref`'s PDF-string hook.
    ///
    /// `\pdfstringdefDisableCommands{…}` is where a package says what its
    /// commands contribute to a PDF bookmark, which is plain text: the
    /// definitions inside it are local to that conversion, are never a
    /// command a user calls, and are written with `\def` because that is the
    /// interface `hyperref` documents. The hook stands inside
    /// `\AtBeginDocument{\ifdefined\pdfstringdefDisableCommands …}` because
    /// the family neither requires `hyperref` nor knows whether it is loaded
    /// before or after the style file: it asks, when the document begins,
    /// whether the document's own preamble loaded that package, and its
    /// answer changes nothing the family's own LaTeX does, only whether
    /// `hyperref` is told anything at all.
    ///
    /// Of the family's own pieces, two carry such a hook —
    /// [`verbatim_piece`] and [`hyperref_piece`] — and so do the pieces of
    /// the features whose commands can stand in a heading's title (`href`,
    /// `defterm`, `citations`). This helper exempts a hook's own block, and
    /// nothing else, from the two checks of
    /// [`every_piece_is_complete_lines_with_one_guard_spelling_per_kind`]:
    /// one guard spelling per kind, and the one question a piece may ask.
    /// What each block holds is checked on its own, by
    /// [`the_pdf_string_hook_says_what_verbatim_content_contributes_to_a_bookmark`],
    /// by
    /// [`the_hyperref_piece_asks_what_the_document_loaded_and_says_what_a_bookmark_holds`]
    /// and by each feature's own tests.
    /// The question `\flmHyperref` asks outside any hook — whose answer *does*
    /// change what the family's LaTeX does — is inside those checks, which
    /// allow it in that piece alone.
    fn definitions_outside_the_pdf_string_hook(content: &str) -> String {
        let text = definitions(content);
        let mut kept = Vec::new();
        let mut inside = false;
        for line in text.lines() {
            if line.trim_start().starts_with("\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands")
            {
                inside = true;
            }
            if !inside {
                kept.push(line);
            }
            if inside && line.trim_end().ends_with("\\fi}") {
                inside = false;
            }
        }
        assert!(!inside, "an unterminated \\pdfstringdefDisableCommands hook");
        kept.join("\n")
    }

    #[test]
    fn the_standard_pieces_carry_their_identifiers_and_requirements() {
        let pieces = standard_pieces();
        let ids: Vec<&str> = pieces.iter().map(|p| p.id.as_str()).collect();
        // The family's own eight come first, in this order; the pieces of the
        // table's snippet chunks follow them
        // (`the_chunk_pieces_follow_the_chunk_table_and_are_guarded`).
        assert_eq!(&ids[..8], [
            REFS,
            HYPERREF,
            LABEL_DISPLAY_TEXT,
            ZREF_CLEVER_TYPE,
            LABELS,
            HEADING,
            VERBATIM,
            MATH_ENVIRONMENT
        ]);
        let by_id = |id: &str| pieces.iter().find(|p| p.id.as_str() == id).unwrap();
        assert_eq!(by_id(REFS).requires, [Name::from(packages::ZREF_CLEVER)]);
        // The references' hyperref hook requires no package: it asks whether
        // the document loaded `hyperref` and prints the text alone where it did
        // not. (The hyperlinks' piece, a feature's, requests the package,
        // since its hooks are that package's own macros.)
        assert!(by_id(HYPERREF).requires.is_empty());
        assert!(by_id(LABEL_DISPLAY_TEXT).requires.is_empty());
        assert_eq!(by_id(ZREF_CLEVER_TYPE).requires, [
            Name::from(packages::ZREF_CLEVER),
            Name::from(LABEL_DISPLAY_TEXT)
        ]);
        assert!(by_id(LABELS).requires.is_empty());
        assert_eq!(by_id(HEADING).requires, [
            Name::from(LABELS),
            Name::from(LABEL_DISPLAY_TEXT),
            Name::from(ZREF_CLEVER_TYPE)
        ]);
        assert_eq!(by_id(VERBATIM).requires, [Name::from(packages::FVEXTRA)]);
        assert_eq!(by_id(MATH_ENVIRONMENT).requires, [Name::from(packages::AMSMATH)]);
        assert!(by_id(IF_BLANK).requires.is_empty());
        // The packed-argument piece's blank test calls the blank-test piece's
        // command, so what "blank" means is written in one place; requiring it
        // is what brings that block into a style file that holds this one.
        assert_eq!(by_id(USER_ARGS).requires, [Name::from(IF_BLANK)]);
    }

    /// The pieces of the declaration chunks come after the family's own, in
    /// the chunk table's order, and each is guarded on a name it declares.
    /// Every declaration chunk is one block of declarations for every engine,
    /// the contract [`chunk_pieces`] panics on.
    #[test]
    fn the_chunk_pieces_follow_the_chunk_table_and_are_guarded() {
        let declarations: Vec<&untechxt::preamble::Chunk> = packages::table_chunks()
            .into_iter()
            .filter(|chunk| !packages::is_package_chunk(chunk))
            .collect();
        for chunk in &declarations {
            let [case] = chunk.cases() else {
                panic!("the chunk {} does not have exactly one case", chunk.id());
            };
            for engine in packages::ENGINES {
                assert!(case.engines().contains(engine), "{} under {engine:?}", chunk.id());
            }
        }
        let pieces = chunk_pieces();
        assert_eq!(
            pieces.iter().map(|piece| piece.id.clone()).collect::<Vec<_>>(),
            declarations.iter().map(|chunk| chunk_piece_id(chunk.id())).collect::<Vec<_>>()
        );
        assert!(pieces.iter().all(|piece| piece.id.as_str().starts_with(CHUNK_ID_PREFIX)));
        // The standard roster holds them last, so the style file writes the
        // family's pieces first and these after.
        let standard: Vec<Name> = standard_pieces().iter().map(|p| p.id.clone()).collect();
        assert_eq!(&standard[standard.len() - pieces.len()..],
                   &pieces.iter().map(|p| p.id.clone()).collect::<Vec<_>>()[..]);
        for piece in &pieces {
            assert!(piece.content.contains("\\ifcsname "), "{}", piece.id);
            assert!(piece.content.ends_with("\\fi\n"), "{}", piece.id);
        }
        assert_eq!(guard_name(r"\DeclareMathAlphabet{\UnxTBbold}{U}{bbold}{m}{n}").as_deref(),
                   Some("UnxTBbold"));
        assert_eq!(guard_name("\\DeclareFontEncoding{LS1}{}{}\n\\DeclareSymbolFont{UnxTx}{LS1}{a}{m}{n}")
                       .as_deref(),
                   Some("symUnxTx"));
    }

    #[test]
    fn a_piece_requires_packages_and_pieces_told_apart_by_their_prefix() {
        for piece in standard_pieces() {
            for id in &piece.requires {
                let known = id.as_str().starts_with(ID_PREFIX)
                    || id.as_str().starts_with(packages::ID_PREFIX);
                assert!(known, "{}: {id}", piece.id);
            }
        }
    }

    /// Every piece a registry of the standard package holds under its default
    /// configuration: the family's own ([`standard_pieces`]) and those the
    /// features contribute, so that a feature's piece is held to the same
    /// shape as the family's.
    fn every_registered_piece() -> Vec<LatexPreambleDef> {
        use crate::core::registry::{RegistryBuilder, RegistryInputs};
        use crate::stdcore::{self, StdcoreConfig};
        let mut builder = RegistryBuilder::new();
        builder.use_package(stdcore::package(StdcoreConfig::new()));
        let registry = builder.build(&RegistryInputs::new()).expect("the standard registry builds");
        let registered: Vec<LatexPreambleDef> = pieces(registry.services()).cloned().collect();
        for piece in standard_pieces() {
            assert!(registered.iter().any(|p| p.id == piece.id), "{}", piece.id);
        }
        registered
    }

    #[test]
    fn every_piece_is_complete_lines_with_one_guard_spelling_per_kind() {
        for piece in every_registered_piece() {
            assert!(piece.content.ends_with('\n'), "{}", piece.id);
            assert!(piece.content.starts_with('%'), "{}", piece.id);
            // The guarded spellings are the recomposed product's convention,
            // where a definition may meet the author's own. The rendered
            // product's pieces (`flm.latex.def.flmx.*`) define their commands
            // once, with `\NewDocumentCommand` and expl3, and are checked by
            // that backend's own tests.
            if piece.id.as_str().starts_with("flm.latex.def.flmx.") {
                continue;
            }
            for line in definitions_outside_the_pdf_string_hook(&piece.content).lines() {
                // A macro or environment the piece offers a user is defined
                // with the one guarded spelling of its kind — including the
                // expandable one, whose guard is `\providecommand`, the one
                // spelling that makes a long expandable macro; the internals,
                // whose names hold `@`, are plain `\def`.
                for definer in [
                    "\\NewExpandableDocumentCommand",
                    "\\DeclareExpandableDocumentCommand",
                    "\\RenewExpandableDocumentCommand",
                    "\\ProvideExpandableDocumentCommand",
                    "\\newcommand",
                    "\\renewcommand",
                    "\\newenvironment",
                    "\\renewenvironment",
                    "\\DeclareRobustCommand",
                    "\\NewDocumentCommand",
                    "\\DeclareDocumentCommand",
                    "\\NewDocumentEnvironment",
                    "\\DeclareDocumentEnvironment",
                ] {
                    assert!(!line.contains(definer), "{}: {definer} in {line}", piece.id);
                }
                if let Some(rest) = line.trim_start().strip_prefix("\\def\\") {
                    let name: String =
                        rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '@').collect();
                    assert!(name.contains('@'), "{}: \\def of the public {name}", piece.id);
                }
            }
            // Nothing tests what is installed: one behavior everywhere. The
            // one question a piece may ask is what the *document's own
            // preamble* loaded, which is a fact of the document's source and
            // answers the same on every TeX installation; the kernel's
            // `\IfPackageLoadedTF` is its spelling, and it is allowed in the
            // pieces of `ASK_WHAT_THE_DOCUMENT_LOADED` and nowhere else. Every
            // other question fails here, whether it asks what is defined, what
            // a *class* loaded, which format is installed or which engine
            // runs. See decision rationale at
            // [§flm-dd-dr:rend-latex-style-conservative].
            let text = definitions_outside_the_pdf_string_hook(&piece.content);
            for forbidden in [
                "\\ifdefined",
                "\\@ifundefined",
                "\\@ifpackageloaded",
                "\\@ifclassloaded",
                "\\IfPackageLoadedWithOptionsTF",
                "\\IfClassLoadedTF",
                "\\IfClassLoadedWithOptionsTF",
                "\\IfFormatAtLeastTF",
                "\\IfPackageAtLeastTF",
                "\\IfFileExists",
                "\\IfFontExistsTF",
                "_if_exist:",
                "\\ifpdf",
                "\\ifxetex",
                "\\ifluatex",
                "\\ifPDFTeX",
                "\\ifXeTeX",
                "\\ifLuaTeX",
                "\\sys_if_engine",
                "\\sys_if_output",
            ] {
                assert!(!text.contains(forbidden), "{}: {forbidden}", piece.id);
            }
            if text.contains(ASKING_WHAT_THE_DOCUMENT_LOADED) {
                assert!(
                    ASK_WHAT_THE_DOCUMENT_LOADED.contains(&piece.id.as_str()),
                    "{}: {ASKING_WHAT_THE_DOCUMENT_LOADED} asked by a piece that is not one of \
                     the pieces allowed to ask it, {ASK_WHAT_THE_DOCUMENT_LOADED:?}; a piece that \
                     needs the question is a decision to be recorded, not a line of LaTeX",
                    piece.id
                );
                // And it asks it about the one package the family leaves to
                // the document, never about a package of its own: a piece that
                // needs a package requires it.
                let asked_of_hyperref =
                    format!("{ASKING_WHAT_THE_DOCUMENT_LOADED}{{{HYPERREF_PACKAGE_COMMAND}}}");
                assert!(
                    !text.replace(&asked_of_hyperref, "").contains(ASKING_WHAT_THE_DOCUMENT_LOADED),
                    "{}: the question is asked of a package other than \
                     `{HYPERREF_PACKAGE_COMMAND}`",
                    piece.id
                );
            }
        }
        // The pieces that ask the question are exactly the listed ones: a
        // piece that stops asking it is listed no more.
        let pieces = standard_pieces();
        let asking: Vec<&str> = pieces
            .iter()
            .filter(|piece| {
                definitions_outside_the_pdf_string_hook(&piece.content)
                    .contains(ASKING_WHAT_THE_DOCUMENT_LOADED)
            })
            .map(|piece| piece.id.as_str())
            .collect();
        assert_eq!(asking, ASK_WHAT_THE_DOCUMENT_LOADED);
    }

    /// The kernel's test of whether the document's preamble loaded a package.
    const ASKING_WHAT_THE_DOCUMENT_LOADED: &str = "\\IfPackageLoadedTF";

    /// The pieces allowed to ask what the document's own preamble loaded,
    /// outside a PDF-string hook: the one whose default links a reference
    /// through `hyperref` where the document loaded it ([`hyperref_piece`]).
    /// Every other piece asking anything at all fails
    /// [`every_piece_is_complete_lines_with_one_guard_spelling_per_kind`].
    const ASK_WHAT_THE_DOCUMENT_LOADED: &[&str] = &[HYPERREF];

    #[test]
    fn no_piece_defines_a_name_under_a_user_definitions_prefix() {
        // Every name beginning with `flmU` is the document's: a recomposed
        // user definition is written under a prefix that begins with those
        // four characters (`super::names`), so a document that declares a
        // macro `blank` has `\flmUDblank`. Nothing this library's own LaTeX
        // writes may take a name there, or the two would collide by
        // construction. This walks every piece the family contributes and
        // refuses such a name however it is defined. A feature that
        // contributes a piece of its own observes the same rule; the rule is
        // stated for every piece, this test reaches the family's roster, and
        // a feature's own tests check its pieces. Stating it over the four
        // characters rather than over the prefixes themselves is what lets the
        // prefixes grow without reopening it: the six a *mode twin* takes
        // (`flmUTD`, `flmUTX`, `flmUTE`, `flmUMD`, `flmUMX`, `flmUME`) are
        // covered here by construction, since each begins with `flmU`.
        for piece in standard_pieces() {
            for line in definitions(&piece.content).lines() {
                assert!(
                    !line.contains(USER_DEFINITION_PREFIX),
                    "{}: the name space `{USER_DEFINITION_PREFIX}` belongs to the document's own \
                     definitions, and this line reaches into it: {line}",
                    piece.id
                );
            }
        }
        // A package request names a package, never a command, so there is
        // nothing to reach into; the check is here so that a package gaining a
        // body of its own does not slip past it.
        for package in packages::standard_packages() {
            assert!(!package.package.contains(USER_DEFINITION_PREFIX), "{}", package.id);
            assert!(
                !package.options.iter().any(|option| option.contains(USER_DEFINITION_PREFIX)),
                "{}",
                package.id
            );
        }
    }

    #[test]
    fn the_hooks_are_defined_under_the_names_the_rules_write() {
        let by_id = |id: &str| {
            standard_pieces().into_iter().find(|p| p.id.as_str() == id).unwrap().content
        };
        assert!(by_id(REFS).contains(&format!("\\ProvideDocumentCommand\\{REF_COMMAND}{{O{{}} m}}")));
        assert!(by_id(LABELS).contains(&format!("\\ProvideDocumentCommand\\{LABELS_COMMAND}{{m}}")));
        assert!(
            by_id(LABEL_DISPLAY_TEXT)
                .contains(&format!("\\ProvideDocumentCommand\\{LABELS_WITH_DISPLAY_TEXT_COMMAND}"))
        );
        assert!(
            by_id(HEADING)
                .contains(&format!("\\ProvideDocumentCommand\\{HEADING_COMMAND}{{O{{}} m s m m}}"))
        );
        // The heading hook runs the sectioning command as written and tests
        // the kernel's record of the counter a class steps to number it.
        assert!(by_id(HEADING).contains("{#2*{#4}%"));
        assert!(by_id(HEADING).contains("\\ifx\\@currentcounter\\@empty"));
        // The private counter and the type it is mapped to carry the names
        // the reference implementation gave them.
        assert!(by_id(LABEL_DISPLAY_TEXT).contains(&format!("\\newcounter{{{INTERNAL_REF_COUNTER}}}")));
        assert!(by_id(ZREF_CLEVER_TYPE)
            .contains(&format!("countertype={{{INTERNAL_REF_COUNTER}={INTERNAL_REF_TYPE}}}")));
    }

    /// The heading hook's key-value argument: two keys, read on the hook, and
    /// never given to a starred sectioning command.
    #[test]
    fn the_heading_hook_takes_two_keys_and_never_stars_an_optional_argument() {
        let heading = definitions(&heading_piece().content);
        // The keys are declared once, under the family's own key path, and the
        // key's own code records both the value and the fact that the key was
        // written. Both are cleared at every call, so one heading's keys never
        // reach the next.
        assert!(heading.contains("\\keys_define:nn {flm/heading}"));
        assert!(heading.contains("alt .code:n = {\\tl_set:Nn \\flm@heading@alt {#1}"));
        assert!(heading.contains("\\bool_set_true:N \\flm@heading@given"));
        assert!(heading.contains("\\keys_set:nn {flm/heading} {#1}"));
        assert!(heading.contains("\\tl_clear:N \\flm@heading@alt"));
        assert!(heading.contains("\\bool_set_false:N \\flm@heading@given"));
        assert!(heading.contains("\\tl_clear:N \\flm@heading@toc"));
        // `alt` and `toc` are the two keys, and both require a value: nothing
        // else is declared, and nothing else is read. `toc` takes `true` or
        // `false` and nothing else. A third key would have to be added here
        // and to the comment block the style file carries.
        assert_eq!(heading.matches(".code:n").count(), 1);
        assert_eq!(heading.matches(".choices:nn").count(), 1);
        assert_eq!(heading.matches("\\keys_define:nn").count(), 1);
        assert!(heading.contains("alt .value_required:n = true"));
        assert!(heading.contains("toc .choices:nn = {true, false}"));
        assert!(heading.contains("toc .value_required:n = true"));
        // A starred heading written `toc=true` is added to the contents under
        // the sectioning command's own name, with the alternative title where
        // one was written; `toc=false` on an unstarred heading is a warning.
        assert!(heading.contains("\\def\\flm@heading@addtoc#1#2{\\addcontentsline{toc}{#1}{#2}}"));
        assert!(heading.contains("\\edef\\flm@heading@level{\\cs_to_str:N #2}"));
        assert!(heading.contains("\\str_if_eq:VnT \\flm@heading@toc {true}"));
        assert!(heading.contains("\\str_if_eq:VnT \\flm@heading@toc {false} {\\flm@heading@notoc{#2}}"));
        assert!(heading.contains("\\PackageWarning{flm}"));
        // Both ways of running the sectioning command are pinned. The star
        // branch hands it no optional argument: a starred sectioning command
        // takes none, and giving it one is silently wrong rather than an
        // error. The unstarred branch relays the alternative title where the
        // key was written and writes the plain call where it was not.
        assert!(heading.contains("{#2*{#4}%"));
        assert!(!heading.contains("#2*["));
        assert!(heading.contains("{#2{#4}}"));
        assert!(heading.contains("\\def\\flm@heading@withalt#1#2#3{#2[#1]{#3}}"));
        // What decides the relay is whether the key was written, never what
        // its value holds: an empty `alt` is an empty alternative title.
        assert!(heading.contains("\\bool_if:NTF \\flm@heading@given"));
        assert!(!heading.contains("\\tl_if_empty:NTF \\flm@heading@alt"));
        // Both declarations are guarded on their own names, so two of these
        // style files load in one LaTeX run; `\tl_new:N` and `\bool_new:N`
        // both stop with an error on a name already taken.
        for name in ["flm@heading@alt", "flm@heading@given", "flm@heading@toc"] {
            assert!(
                heading.contains(&format!("\\ifcsname {name}\\endcsname\\else")),
                "the declaration of {name} is not guarded: {heading}"
            );
        }
    }

    #[test]
    fn the_alias_line_is_written_after_the_last_page_and_read_back_globally() {
        let labels = definitions(&labels_piece().content);
        // `\AtEndDocument` would run before the last page's `\newlabel`
        // lines reach the auxiliary file.
        assert!(labels.contains("\\AddToHook{enddocument/afterlastpage}"));
        assert!(!labels.contains("\\AtEndDocument"));
        // The auxiliary file is read inside a group, so the alias is global.
        assert!(labels.contains("\\global\\expandafter\\let\\csname r@#1"));
        assert!(labels.contains("\\global\\expandafter\\let\\csname Z@R@#1"));
    }

    #[test]
    fn the_names_written_by_the_rules_are_the_ones_the_verbatim_piece_defines() {
        let content = definitions(&verbatim_piece().content);
        for kind in [VerbatimKind::Text, VerbatimKind::Code, VerbatimKind::Identifier] {
            let hook = format!("\\{FMT_VRB}{}", verbatim_hook(kind));
            assert!(content.contains(&hook), "{hook}");
        }
        assert!(
            content.contains(&format!("\\ProvideDocumentEnvironment{{{VERBATIM_CODE_ENVIRONMENT}}}"))
        );
        assert!(
            content.contains(&format!("\\ProvideDocumentEnvironment{{{VERBATIM_TEXT_ENVIRONMENT}}}"))
        );
        assert!(
            content.contains(&format!("\\ProvideDocumentCommand\\{VERBATIM_BLOCK_COMMAND}{{O{{}} m m}}"))
        );
        // `\EscVerb` comes from `fvextra`; the piece neither defines it nor
        // tests for it. The one line that names it is the PDF-string hook,
        // which says what it contributes to a bookmark rather than what it
        // does on the page.
        let outside = definitions_outside_the_pdf_string_hook(&verbatim_piece().content);
        assert!(!outside.contains(&format!("\\{ESCVERB}")));
        assert_eq!(escverb_options(VerbatimKind::Code), "formatcom=\\flmFmtVRB{verbcode}");
    }

    #[test]
    fn the_pdf_string_hook_says_what_verbatim_content_contributes_to_a_bookmark() {
        let content = definitions(&verbatim_piece().content);
        // A PDF bookmark is plain text. Inline verbatim content contributes
        // its text — `fvextra` disables `\EscVerb` for a PDF string without
        // removing its optional argument, so without this line the option
        // list itself lands in the bookmark — and a block contributes its
        // characters run together, with no line breaks and nothing between
        // the lines. Both definitions must be expandable: `hyperref` removes
        // an unexpandable token from a PDF string and warns, so a walk over
        // the line groups (which would need `\@ifnextchar`) is not an option.
        assert!(content.contains(&format!("\\def\\{ESCVERB}[#1]#2{{#2}}")));
        assert!(content.contains(&format!("\\def\\{VERBATIM_BLOCK_COMMAND}[#1]#2#3{{#3}}")));
        assert!(content.contains("\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands"));
        // The hook is the only place the piece asks what is installed.
        assert_eq!(content.matches("\\ifdefined").count(), 1);
        // And the verbatim piece is the **only** piece that carries such a
        // hook. The two invariants of `every_piece_is_complete_lines_…` — one
        // guard spelling per kind, and nothing tests what is installed — stop
        // at the hook's edge for every piece, so without this the exemption
        // would be wider than the reason for it: another piece could put an
        // unguarded `\def` of a public name, or a second `\ifdefined`, inside
        // a hook of its own and no test would see it. A piece that needs one
        // adds its own check of what its hook holds, as this test is for this
        // piece, and amends the count here.
        let with_a_hook: Vec<String> = standard_pieces()
            .iter()
            .filter(|piece| {
                definitions(&piece.content).contains("\\pdfstringdefDisableCommands")
            })
            .map(|piece| piece.id.to_string())
            .collect();
        assert_eq!(with_a_hook, [String::from(HYPERREF), String::from(VERBATIM)]);
    }

    #[test]
    fn the_hyperref_piece_asks_what_the_document_loaded_and_says_what_a_bookmark_holds() {
        let content = definitions(&hyperref_piece().content);
        // The default links through the package's own command where the
        // document loaded it and prints the text alone where it did not, so
        // the recomposed LaTeX compiles either way. The question is about the
        // document's preamble, which is why it is `\IfPackageLoadedTF` — the
        // kernel's own test, asked at the reference, and not a test of what
        // the installation has.
        assert!(content.contains(&format!(
            "\\ProvideDocumentCommand\\{HYPERREF_COMMAND}{{m m}}"
        )));
        let linked = format!("\\{HYPERREF_PACKAGE_COMMAND}[{{#1}}]{{#2}}");
        assert!(content.contains(&format!(
            "\\IfPackageLoadedTF{{{HYPERREF_PACKAGE_COMMAND}}}{{{linked}}}{{#2}}"
        )));
        assert!(!content.contains("\\RequirePackage"));
        // The second piece carrying a PDF-string hook, and what its hook
        // holds: the text alone, as an expandable definition, `hyperref`
        // dropping an unexpandable token from a PDF string with a warning.
        // The checks of `every_piece_is_complete_lines_…` stop at a hook's
        // edge, so each piece with one says here what its own holds.
        assert!(content.contains("\\AtBeginDocument{\\ifdefined\\pdfstringdefDisableCommands"));
        assert!(content.contains(&format!("\\def\\{HYPERREF_COMMAND}#1#2{{#2}}")));
        assert_eq!(content.matches("\\ifdefined").count(), 1);
    }

    #[test]
    fn the_escaped_block_is_the_style_file_s_only_use_of_scantokens() {
        // The escaped block's wrapper hands its rebuilt lines to
        // `\scantokens`, which offers them to TeX as if they were a file, and
        // the wrapper's comment lines tell a reader of the style file that
        // this is the file's only use of that primitive. It is also one of the
        // two engine assumptions the family allows itself, so a second use is
        // a decision and not an accident: one use, in the verbatim piece.
        let uses = |piece: &LatexPreambleDef| {
            definitions(&piece.content).matches("\\scantokens").count()
        };
        let pieces = standard_pieces();
        assert_eq!(pieces.iter().map(uses).sum::<usize>(), 1);
        assert_eq!(uses(&verbatim_piece()), 1);
    }
}
