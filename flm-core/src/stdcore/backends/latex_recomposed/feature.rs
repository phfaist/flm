//! The `latex-recomposed` feature ([`LatexRecomposed`]), which registers the
//! `latex-recomposed` backend: its native element set, its render options, its
//! two structural lowering rules, its stages and their tables, the
//! `latex-recomposed` output and the `latex-recomposed` workflow.
//!
//! A feature ([`FlmFeature`]) is a Rust value that contributes a related
//! bundle of things to a registry. This feature registers beside three others
//! that it relies on: the `lowering` feature
//! ([`LoweringFeature`](crate::stdcore::backends::lowering::LoweringFeature)),
//! which registers the shared presentation elements; the `latex` feature
//! ([`LatexFamily`](crate::stdcore::backends::latex::LatexFamily)), which
//! registers the LaTeX construct elements that this backend writes natively
//! and the service slots through which the regions stage
//! ([`LatexRecomposedRegionsStage`]) resolves package requests and definition
//! pieces; and the `check` feature
//! ([`check_feature`](crate::stdcore::check_feature)), which declares the
//! collection `flm.validated` that the `latex-recomposed` workflow demands
//! (see *What the workflow demands* below). A host normally obtains this
//! feature through [`feature()`].
//!
//! # The native element set
//!
//! The native set ([`native_elements`]) lists the elements that the backend's
//! emitter ([`emit_tree`](super::emit_tree)) writes as they are, without a
//! lowering rule. It is small: the backend
//! recomposes, so nearly everything reaches the emitter as a LaTeX construct
//! element that a feature's own lowering rule staged (built into the lowered
//! tree), and the emitter writes those construct elements and little else.
//!
//! - The four construct elements of the LaTeX family ([`construct::ALL`]): a
//!   macro call, an environment, raw LaTeX text, and a definition. Every
//!   lowering rule for this backend stages these.
//! - [`MATH_PASSTHROUGH`](crate::stdcore::backends::pres::MATH_PASSTHROUGH) and
//!   [`VERBATIM`](crate::stdcore::backends::pres::VERBATIM): mathematics and
//!   verbatim content are written to LaTeX byte for byte, so the shared
//!   elements that carry them as source need no translation; the emitter
//!   writes the delimiters and the source. Mathematics itself is written from
//!   its nodes by the `math-passthrough` feature's rules on this backend, so
//!   that a user definition used inside a formula is handled; a
//!   `flm.pres.math-passthrough` node reaches the emitter only from that
//!   feature's rule for an unknown command of math mode
//!   ([`OPAQUE_RULE`](crate::math_passthrough::OPAQUE_RULE)), when such
//!   a node is lowered on its own, outside a math group and outside a
//!   display-math environment, for instance in a math-mode argument of a user
//!   definition written as a call.
//! - [`UNKNOWN`](crate::stdcore::backends::pres::UNKNOWN): a macro or
//!   environment that no feature declares is content the author wrote in
//!   LaTeX syntax already; the emitter writes it as written, and the error
//!   reported for it is among the run's diagnostics.
//! - [`UNSUPPORTED`](crate::stdcore::backends::pres::UNSUPPORTED): the node
//!   that the lowering driver stages in place of an accepted unrenderable
//!   element (an element that the plan request accepts as one the backend
//!   need not render; see
//!   [`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported)).
//!   Every backend must write it natively; the emitter writes a comment line
//!   that gives the element's name.
//! - [`DROPPED`](crate::stdcore::backends::pres::DROPPED): the node that a
//!   lowering rule stages where it left out something it could not place
//!   (stray content of a table of cells, the later of two overlapping cells);
//!   the emitter writes a comment line that says what was left out.
//! - [`SUBTREE`](crate::stdcore::backends::pres::SUBTREE): the place of
//!   another tree of the document, such as the tree of a file included with a
//!   heading of its own; the recomposed LaTeX of that tree is written at that
//!   place.
//! - Four structural elements
//!   ([`StructuralElements`](crate::lang::structural::StructuralElements))
//!   whose meaning LaTeX needs no translation for ([`FLOW_THROUGH`]):
//!   `flm.text` (document text, which the emitter escapes; it must be native
//!   rather than lowered because escaping happens where the bytes are
//!   written), `flm.group` (a brace group), `flm.comment` and `flm.container`.
//!
//! The rest of the shared presentation elements (`par`, `span`,
//! `block-heading`, `runin-heading`, `link`, `list`, `lines`, `image`, the
//! decorations and their marks) are not in the set, because no rule of this
//! backend produces them (the features' recomposition rules stage construct
//! elements instead), and an element in the native set is an element the
//! emitter promises to write. `flm.parbreak` and `flm.paragraph` are not in
//! the set either: the two structural rules of [`mod@super::rules`] lower
//! them, and the paragraph-break rule is where the backend states that it
//! needs the raw tree. The set holds no element that a content feature owns.
//! Each content feature registers its own recomposition rule for this backend
//! ([`mod@super::rules`] explains how); this feature registers no lowering rule
//! but its own two.
//!
//! # What the workflow demands
//!
//! The `latex-recomposed` [workflow](crate::core::registry::WorkflowDef)
//! lists the `latex-recomposed` output and also *demands* one capability,
//! that is, asks the plan for the stages that provide it
//! ([`PlanRequest::demand`](crate::core::plan::PlanRequest::demand)). That
//! capability is [`flm.validated`](crate::stdcore::VALIDATED), a collection
//! ([`CollectionDef`](crate::core::registry::CollectionDef)): a name for a
//! set of capabilities that any feature may add to. Each check in the
//! registry adds its capability to it, and demanding the collection demands
//! all of them. A recomposition therefore runs the same
//! checks as every other workflow: a refused label type, a duplicate label or
//! an error in mathematics content is reported here exactly as by the
//! [`check` workflow](crate::stdcore::CHECK_WORKFLOW). The checks read the
//! document tree, in which a use site of a user definition that this backend
//! keeps is still a call of the definition; what the definition contains is
//! checked by LaTeX. The collection is declared by the `check` feature
//! ([`check_feature`](crate::stdcore::check_feature)), so a registry that
//! offers this backend must also hold that feature; a registry without it
//! fails to build, with an error that identifies this workflow's demands and
//! the missing name.
//!
//! Everything else that a run needs follows from the requirements of the
//! selected stages and lowering rules: the paragraph-break rule requires
//! `flm.blocks.raw`, and the reference rules read the table of safe labels,
//! so the stage that writes that table joins the plan.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::core::region::RegionsOutput;
use crate::core::settings::SettingsDecl;
use crate::stdcore::backends::lowering::REGIONS_SLOT;
use crate::core::registry::{
    BackendDef, BuildError, FeatureBuilder, FeatureInfo, FlmFeature, OutputDef, WorkflowDef,
};
use crate::lang::structural;
use crate::model::names::ElementName;
use crate::model::options::Bind;
use crate::stdcore::VALIDATED;
use crate::stdcore::backends::latex::construct;
use crate::stdcore::backends::latex::{ExtraPreamble, LatexPackageDef, UnicodeMode};
use crate::stdcore::backends::lowering::{LoweredTrees, lowered_table_name};
use crate::stdcore::backends::pieces::{LoweredPieces, PiecesStage, lowered_pieces_table_name};
use crate::stdcore::backends::pres;

use super::rules::rules;
use super::stages::{
    ARTIFACTS, DOCUMENT, EXTENSION, LatexRecomposedRegionsStage, MEDIA_TYPE, OUTPUT_TABLE, STYLE_FILE,
    PACKAGES_TABLE, PREAMBLE_TABLE, REGIONS, WORKFLOW, stages,
};

/// The registered name of the [`LatexRecomposed`] feature, `latex-recomposed`.
/// It is also the middle part of the identifiers of the conditions this
/// backend reports, `flm.latex-recomposed.*`.
pub const FEATURE_NAME: &str = "latex-recomposed";

/// The declaration of the feature's settings: one option type,
/// [`LatexRecomposedRenderOptions`], bound to the render request, whose fields
/// are the settings keys. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE_NAME)
        .options::<LatexRecomposedRenderOptions>(
            Bind::Render,
            "how a non-ASCII character of the text is written (`declare`, the default, writes \
            the character and declares it in the style file; `replace` writes its LaTeX \
            spelling), and the spellings that replace the library's own for given characters; \
            whether the document's source comments are left out; the LaTeX packages this \
            library never requests, and the packages the style file always requests, with \
            their options, merged with the requests that the document's content causes into \
            one `\\RequirePackage` line per package; three parts of LaTeX text written byte for \
            byte into the style file, before the packages, after them and at its end; the \
            user-definition names written as the document spells them rather than under a \
            prefix, and those written as robust commands rather than expandable macros; the \
            stem of the style file's name (default `flmrecomposed`), to which the program's \
            naming of files may add a prefix; when the style file is \
            written (`when-needed`, the default, writes it only where the document needs \
            something in it, `always` writes it for every document, and either way it is \
            written where packages to request always or LaTeX text to write into it are given); \
            whether headings are written through the LaTeX hook `\\flmHeading`, and labels \
            and references through the LaTeX hooks `\\flmLabels`, \
            `\\flmLabelsWithDisplayText` and `\\flmRef` (both on by default); whether the \
            labels of an equation line are left out of the recomposed LaTeX in a registry \
            without the references feature (off by default; no effect where that feature is \
            registered); and whether a proof's back-reference is written as the author wrote \
            it, for the `phfthm` package (the default), or resolved into the LaTeX hook \
            `\\flmProofTitle`, for `amsthm` alone. Set per render request.",
        )
}

/// The backend's registered name, `latex-recomposed`. A lowering rule marked
/// as the default on this backend refers to it
/// ([`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends)),
/// and the names of the backend's lowering stage and lowered-trees table
/// (`flm.lowered.latex-recomposed`) are derived from it.
pub const BACKEND: &str = "latex-recomposed";

/// The default stem of the style file's name, `flmrecomposed`; see
/// [`LatexRecomposedRenderOptions::style_stem`].
pub const DEFAULT_STYLE_STEM: &str = "flmrecomposed";

/// The structural elements that this backend's emitter writes natively,
/// without a lowering rule: `flm.text`, `flm.group`, `flm.comment` and
/// `flm.container` (see the module documentation and
/// [structural elements](crate::guide::concepts_overview#structural-elements)).
pub const FLOW_THROUGH: [&str; 4] =
    [structural::TEXT, structural::GROUP, structural::COMMENT, structural::CONTAINER];

/// The shared presentation elements ([`pres`]) that this backend's emitter
/// writes natively: `flm.pres.math-passthrough`, `flm.pres.verbatim`,
/// `flm.pres.unknown`, `flm.pres.unsupported`, `flm.pres.dropped`,
/// `flm.pres.subtree` and `flm.pres.span` (see the module documentation). The
/// span is here for the structural rule of `flm.math`
/// ([`MATH_RULE`](crate::stdcore::backends::lowering::rules::MATH_RULE)),
/// which every backend must be able to write; its content is written, its
/// role and kinds are not.
pub const SHARED_NATIVE: [&str; 7] = [
    pres::MATH_PASSTHROUGH,
    pres::VERBATIM,
    pres::UNKNOWN,
    pres::UNSUPPORTED,
    pres::DROPPED,
    pres::SUBTREE,
    pres::SPAN,
];

/// The backend's native element set, the elements its emitter writes without
/// a lowering rule: the four LaTeX construct elements ([`construct::ALL`]),
/// then [`SHARED_NATIVE`], then [`FLOW_THROUGH`], and nothing else. The
/// [`LatexRecomposed`] feature passes it to the backend's declaration
/// ([`BackendDef`]); see the module documentation for why each element is in
/// it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_recomposed::native_elements;
///
/// let native = native_elements();
/// assert_eq!(native.len(), 15);
/// assert!(native.iter().any(|element| element.as_str() == "flm.pres.latex.macro"));
/// assert!(native.iter().all(|element| element.as_str() != "flm.pres.par"));
/// ```
pub fn native_elements() -> Vec<ElementName> {
    construct::ALL
        .iter()
        .chain(SHARED_NATIVE.iter())
        .chain(FLOW_THROUGH.iter())
        .map(|name| ElementName::from(*name))
        .collect()
}
/// The render options of the `latex-recomposed` backend: how text is
/// escaped, which LaTeX packages and definitions the style file holds, how
/// user definitions, headings, labels, references and proofs are written, and
/// when the style file is written.
///
/// The option type is declared with [`Bind::Render`]: its value is set on the
/// render request
/// ([`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options())),
/// never on the document, so two runs that ask for different values use two
/// plans. The same fields are the keys of the feature's settings (see
/// [`crate::core::settings`]). Every field has a default, listed in its
/// documentation.
///
/// Several options are marked **You own this**: when you set them, the library
/// no longer guarantees that the recomposed LaTeX compiles or means what the
/// document says, and the documentation of the option states what you must
/// then provide yourself.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::UnicodeMode;
/// use flm_core::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions;
///
/// let options = LatexRecomposedRenderOptions::default();
/// assert_eq!(options.unicode, UnicodeMode::Declare);
/// assert!(!options.drop_comments);
/// assert_eq!(options.style_stem, "flmrecomposed");
/// assert!(!options.drop_math_labels_without_refs);
/// assert!(!options.resolved_proof_refs);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct LatexRecomposedRenderOptions {
    /// What is written for a non-ASCII character of document text:
    /// [`Declare`](UnicodeMode::Declare), the default, writes the character
    /// itself as UTF-8 and declares it once in the style file;
    /// [`Replace`](UnicodeMode::Replace) writes its LaTeX spelling in place
    /// (see [`escape`](crate::stdcore::backends::latex::escape)). Mathematics
    /// and verbatim content are written byte for byte in either mode.
    pub unicode: UnicodeMode,
    /// Per-character spellings that replace the spellings of the
    /// Unicode-to-LaTeX table of [`untechxt`] (the table that
    /// gives each character a LaTeX spelling and states what that spelling
    /// needs), in either Unicode mode: for each character, the
    /// LaTeX to write in its place, or to declare it with. Empty by default.
    ///
    /// **You own this when it is set.** Your spelling is written exactly as
    /// you gave it, and **nothing it needs is added to the style file**: the
    /// style file requests only what the Unicode-to-LaTeX table says its own
    /// spellings need. You must provide a spelling that is valid wherever the
    /// character occurs, in text and in mathematics alike, and load everything
    /// that spelling needs in your document's preamble (or through
    /// [`extra_packages`](Self::extra_packages) or
    /// [`extra_preamble`](Self::extra_preamble)). Neither is checked.
    pub unicode_overrides: BTreeMap<char, String>,
    /// Whether the document's source comments are left out of the recomposed
    /// LaTeX. Off by default: a comment is written with its comment prefix,
    /// its text and the whitespace that followed it.
    pub drop_comments: bool,
    /// LaTeX packages that this library never requests, each given by the
    /// name `\usepackage` takes (`amsmath`), for a user whose document class
    /// loads them already. Empty by default.
    ///
    /// An entry stops every request that this library makes for the package:
    /// the LaTeX family's own requests and the ones the document's content
    /// caused. The one exception is the style file's request for `iftex`, which
    /// the file makes wherever its package block tests which engine compiles
    /// the document, since the test needs that package. An entry matches the
    /// package, not the options it is loaded with: `fontenc` stops every
    /// request for it, the `T1` one and the `T2A` one alike, and one of them
    /// cannot be stopped without the other. Skipping `fontenc` also stops the
    /// command that sets the default font encoding
    /// ([`default_encoding_command`](crate::stdcore::backends::latex::style::default_encoding_command)),
    /// under every engine, except where one of your own
    /// [`extra_packages`](Self::extra_packages) requests loads `fontenc`
    /// with an engine's own encoding (`T1` for pdfLaTeX, `TU` for LuaLaTeX and
    /// XeLaTeX): the file writes that command, for that engine, for any
    /// `fontenc` line its package block holds.
    ///
    /// **An explicit request of yours takes precedence.** A package you list
    /// here *and* request through [`extra_packages`](Self::extra_packages) is
    /// still requested, on a line of its own, with your options and no
    /// others. Listing a package here and then requesting it is how you load a
    /// package with an option list that this library's own requests cannot
    /// add to. Your own requests for that package are still merged with **each
    /// other**, by the rule that applies to every other package
    /// ([`merge_requests`](crate::stdcore::backends::latex::packages::merge_requests)),
    /// so the style file holds one line for the package; two of them that
    /// give one option key two different values are reported as the same
    /// Error ([`ConflictingPackageOptions`](super::ConflictingPackageOptions)),
    /// and the package is then not requested.
    ///
    /// **You own this when it is set.** The recomposed LaTeX still uses what
    /// the skipped packages provide, so your preamble or your document class
    /// must load every package you list here; without them the document does
    /// not compile. Where you skip `fontenc`, you must also set the document's
    /// default font encoding.
    pub skip_packages: BTreeSet<String>,
    /// LaTeX packages that the style file always requests, each a package name
    /// with the options it is loaded with, as a [`LatexPackageDef`]. Empty by
    /// default.
    ///
    /// They join the requests that the document's content caused (the
    /// packages that this library's lowering rules, definition pieces and
    /// character spellings need), after them and in the order given. Every
    /// request for one package is then merged into the single `\RequirePackage`
    /// line that the style file writes for that package
    /// ([`merge_requests`](crate::stdcore::backends::latex::packages::merge_requests)).
    /// Requesting a package that this library requests itself therefore adds
    /// your options to its line, and requesting the same package twice adds
    /// your options once. A request of yours applies to every engine: the style
    /// file writes it the same way under pdfLaTeX, LuaLaTeX and XeLaTeX, and
    /// where this library's own request for that package differs by engine
    /// (`fontenc` does), your options join each engine's line. Two requests
    /// that give one option key two different values cannot be merged
    /// (`papersize=a4` against `papersize=letter`, or the flag `draft` against
    /// `draft=false`): that is the Error
    /// [`ConflictingPackageOptions`](super::ConflictingPackageOptions), and the
    /// style file then does not request that package at all.
    ///
    /// This option is where an author states what the parts of the document
    /// that this library writes through unchanged need in order to compile.
    /// Mathematics is written byte for byte, so a formula that uses
    /// `\boldsymbol` needs `amsmath`, and the author requests it here, once,
    /// rather than per definition or per use site.
    ///
    /// **You own this when it is set.** Nothing checks that the package
    /// exists, that the options are its own, or that loading it is compatible
    /// with what the document or your own preamble loads. A package that you
    /// also list in [`skip_packages`](Self::skip_packages) is still requested,
    /// on a line of its own, merged with your other requests for that package
    /// and with none of this library's; that is how you state an option list
    /// of your own.
    ///
    /// When this option holds a request, the style file is written regardless
    /// of [`style_file`](Self::style_file), since the file is where the
    /// requests are written.
    pub extra_packages: Vec<LatexPackageDef>,
    /// LaTeX text written into the style file exactly as given, in three
    /// parts ([`ExtraPreamble`]), all empty by default.
    ///
    /// The three parts differ only in where the style file places them, and
    /// each serves a different need:
    ///
    /// - [`before_packages`](ExtraPreamble::before_packages) is placed between
    ///   the file's header and the package block. `\PassOptionsToPackage` must
    ///   be written there to reach a package that the block requests, and a
    ///   package that must be loaded before the others is loaded there.
    /// - [`after_packages`](ExtraPreamble::after_packages) is placed directly
    ///   after the package block and the command that sets the default font
    ///   encoding, before everything this library defines. A definition made
    ///   there takes precedence over the LaTeX family's own: every definition
    ///   piece is guarded (`\ProvideDocumentCommand` and the related commands),
    ///   so a LaTeX hook defined there is the one the document uses, with
    ///   nothing patched.
    /// - [`at_end`](ExtraPreamble::at_end) is placed at the bottom, after the
    ///   character declarations, the definition pieces and the recomposed user
    ///   definitions, and before the `\endinput` that ends the file, so it can
    ///   use any of them: redefine a LaTeX hook, patch a definition piece, or
    ///   define a command that the document's own mathematics uses.
    ///
    /// Each part is written under a comment line of its own, and an empty part
    /// writes nothing at all. Where a part does not end in a line break, one is
    /// added so that the next line begins on a line of its own; the part's own
    /// bytes are written unchanged.
    ///
    /// **You own this when it is set.** Every part is written **byte for
    /// byte**: nothing in it is escaped, quoted, parsed or checked. A stray
    /// `\` or an unclosed group is yours to find, and so is a definition of a
    /// name that this library's own output relies on. It is the same text you
    /// would put in your document's preamble, placed where every recomposed
    /// output of this document includes it.
    ///
    /// When any part is not empty, the style file is written regardless of
    /// [`style_file`](Self::style_file), since the file is where the text is
    /// written.
    pub extra_preamble: ExtraPreamble,
    /// The user-definition names (macro and environment names) that the
    /// recomposed LaTeX writes exactly as the document spelled them, instead of
    /// under the prefix that every recomposed user definition otherwise takes.
    /// Empty by default; a name is given without a backslash, and one entry
    /// applies to a macro and an environment of that name alike.
    ///
    /// By default a kept user definition
    /// ([`kept`](crate::stdcore::features::userdefs::kept)) never keeps its
    /// own name: the macro
    /// `\emph` is recomposed as `\flmUDemph` and the environment `note` as
    /// `flmUEnote`
    /// ([`recomposed_name`](crate::stdcore::backends::latex::names::recomposed_name)),
    /// so that the definition cannot disturb LaTeX, a package, or the style
    /// file itself. Listing a name here asks for the opposite: the definition
    /// is written under that name and replaces LaTeX's own command of that
    /// name for the whole document.
    ///
    /// **You own this when it is set.** The document's definition becomes the
    /// definition of that command everywhere the style file is loaded: in your
    /// own text, in what a package writes, and in what the document class
    /// writes, and nothing warns you where that changes a result. You must
    /// provide a definition that can replace the command it overrides.
    ///
    /// Three kinds of entry are refused, each reported with the Error
    /// `flm.latex-recomposed.definition-name-refused`
    /// ([`DefinitionNameRefused`](super::DefinitionNameRefused)), which gives
    /// the reason ([`DefinitionNameRefusal`](super::DefinitionNameRefusal)).
    /// A name that begins with `flm` is refused, because that prefix is
    /// reserved for the LaTeX names that this library writes itself; the
    /// definition is then recomposed under its prefix as usual. An entry under
    /// which two of the document's own definitions would become one LaTeX
    /// command is refused too, because `\DeclareDocumentEnvironment{<name>}`
    /// defines the commands `\<name>` and `\end<name>` itself: a name declared
    /// as both a macro and an environment, or a macro named `end<name>` beside
    /// an environment named `<name>` when both are listed. Those definitions
    /// are still written as the option asked, and the author removes the
    /// entry or renames one of the two definitions. Last, a name that the
    /// document declares twice, once for text mode and once for mathematics
    /// mode, is refused, because the two per-mode definitions are written
    /// under two names of their own and cannot share one; both keep their
    /// prefixed names.
    pub bare_names: BTreeSet<String>,
    /// The user-definition names written as **robust** LaTeX commands rather
    /// than as the plain expandable macros that every recomposed definition
    /// otherwise becomes. Empty by default; a name is given without a
    /// backslash, and one entry applies to every definition of that name: a
    /// macro, an environment, and both per-mode definitions of a name declared
    /// once for text mode and once for mathematics mode.
    ///
    /// A *robust* command is one that LaTeX carries into a file of its own
    /// (the table of contents, the list of figures, the auxiliary file) as the
    /// call itself rather than as what the call stands for, and runs when that
    /// file is read back. The opposite is an *expandable* macro, which is
    /// replaced by what it stands for wherever text is expanded rather than
    /// typeset.
    ///
    /// By default a recomposed macro is expandable
    /// (`\providecommand\<name>[<n>]{<body>}`). A call of it therefore reaches
    /// a PDF bookmark as the definition's own words, and `\MakeUppercase`
    /// changes the case of those words. Listing a name here asks for the other
    /// behavior: the macro is written with LaTeX's document command interface,
    /// `\ProvideDocumentCommand\<name>{+m …}` with one `+m` per argument that
    /// a call writes, which makes it protected from expansion. The call then
    /// goes into a table of contents or an auxiliary file as itself;
    /// `hyperref` cannot expand it into a PDF bookmark and leaves it out of
    /// the bookmark with the log message "Token not allowed in a PDF string";
    /// and a case change around the call leaves the definition's own words
    /// exactly as the definition wrote them.
    ///
    /// **You own this when it is set.** Use it where a definition's body holds
    /// something that must not be expanded early, and where you prefer that
    /// the definition's own words survive a case change rather than appear in
    /// a PDF bookmark. Nothing checks which of the two a document needs, and
    /// the missing bookmark text is not reported.
    ///
    /// **No definition is refused for its form.** Every recomposed form can be
    /// written either way: a call writes one brace group per argument
    /// regardless of how the definition was written, the arguments of a
    /// definition with more than
    /// [`PLAIN_ARGUMENT_LIMIT`](crate::stdcore::backends::latex::construct::PLAIN_ARGUMENT_LIMIT)
    /// arguments are passed in one group, and a verbatim argument's value is a
    /// brace group like any other.
    ///
    /// **An entry that has nothing to act on is accepted and does nothing**,
    /// so listing every name a document defines is safe. There are three such
    /// entries. An **environment** listed here does not change, because an
    /// environment is robust already: it is written with
    /// `\ProvideDocumentEnvironment` and cannot be written otherwise, since
    /// `\begin` opens a group, runs the begin code, sets the body and runs the
    /// end code, and expansion alone could produce none of that. A name that
    /// **no definition of the document** has does not change anything: an
    /// entry states a wish about a name, not a claim that the document
    /// declares it. A definition that **cannot be written as a LaTeX
    /// definition at all**, so that every use site of it is written expanded
    /// ([`NotRecomposable`](crate::lang::rewrites::userdefs::NotRecomposable)),
    /// does not change either, since there is no definition to make robust.
    ///
    /// One kind of entry is refused, with the same Error
    /// `flm.latex-recomposed.definition-name-refused` that
    /// [`bare_names`](Self::bare_names) uses
    /// ([`DefinitionNameRefusal::FamilyPrefix`](super::DefinitionNameRefusal::FamilyPrefix)):
    /// a name that begins with `flm`, the prefix reserved for the LaTeX names
    /// that this library writes itself. The entry is ignored and the
    /// definition is written as an expandable macro, as usual.
    pub robust_names: BTreeSet<String>,
    /// The stem of the style file's name: the style file's own name is
    /// `<stem>.<hash>`, where the hash is computed from the file's content
    /// ([`content_hash`](crate::stdcore::backends::latex::style::content_hash)).
    /// Without a naming of files by the program, the file is
    /// `<stem>.<hash>.sty`; the program's naming of files may add to that
    /// name, for instance a prefix from the name of the document's file
    /// (`paper-flmrecomposed.1a2b3c4d.sty`).
    /// The default is [`DEFAULT_STYLE_STEM`], `flmrecomposed`.
    pub style_stem: String,
    /// When the style file is written: [`WhenNeeded`](StyleFile::WhenNeeded),
    /// the default, only where the document needs something in it, or
    /// [`Always`](StyleFile::Always). In both cases the file is written when
    /// [`extra_packages`](Self::extra_packages) holds a request or any of the
    /// three parts of [`extra_preamble`](Self::extra_preamble) is not empty,
    /// because the author asked for something to be in it.
    pub style_file: StyleFile,
    /// Whether a heading is written through the LaTeX hook `\flmHeading` (a
    /// macro that the style file defines with a default and that a user may
    /// redefine; see [`preamble`](crate::stdcore::backends::latex::preamble)).
    /// On by default.
    ///
    /// When on, a heading is written as `\flmHeading[toc=true]`, a newline,
    /// and `\section*{Title}{\flmLabels{\label{ref1}}}`; the hook is on a line of
    /// its own so that the sectioning command is easy to read in the
    /// recomposed LaTeX, and LaTeX reads the same tokens either way. The hook
    /// runs the sectioning command as written and then makes the heading's
    /// labels refer to it: by its number when the document class numbered it,
    /// and by its **title** when it did not, so that a reference to a starred
    /// heading, or to a level below the class's numbering depth, reads
    /// "Introduction" instead of the number of the item LaTeX numbered last
    /// ([`heading_piece`](crate::stdcore::backends::latex::preamble::heading_piece)).
    /// The hook also takes a key-value argument of its own, with two keys:
    /// `toc`, written on every heading (whether the heading is listed in the
    /// table of contents), and `alt`, written where the heading was given an
    /// alternative title (the heading key `alt=`), which LaTeX puts in the
    /// table of contents, the running head and the PDF bookmark (on a starred
    /// heading, only its line in the table of contents; see `heading_piece`).
    /// When this option is off neither key is written.
    ///
    /// When off, the sectioning command is written on its own,
    /// `\section*{Title}`, with the heading's labels after it: still inside
    /// `\flmLabels` while [`label_hooks`](Self::label_hooks) is on, and as
    /// bare `\label` commands otherwise. Nothing then replaces the hook's
    /// test, and a reference to an unnumbered heading prints the last value
    /// LaTeX recorded, with no warning.
    ///
    /// **You own this when it is off.** The library no longer guarantees that
    /// a reference shows the right target. You must provide either a document
    /// in which no unnumbered heading is the target of a reference, or a
    /// mechanism of your own that gives such headings a display text.
    pub heading_hook: bool,
    /// Whether labels and references are written through the LaTeX hooks
    /// `\flmLabels`, `\flmLabelsWithDisplayText` and `\flmRef` (macros that
    /// the style file defines with a default and that a user may redefine;
    /// see [`preamble`](crate::stdcore::backends::latex::preamble)). On by
    /// default.
    ///
    /// When on, a group of labels is written `\flmLabels{\label{ref1}\label{ref2}}`
    /// and a reference `\flmRef[<options>]{ref1}`, where `ref1` and `ref2`
    /// are safe labels
    /// ([`SafeLabels`](crate::stdcore::features::refs::SafeLabels)), the keys
    /// the recomposed LaTeX writes for the document's labels. `\flmLabels`
    /// makes every label after the first on a line of a display-math
    /// environment refer to that line's number, which `amsmath` alone does
    /// not do
    /// ([`labels_piece`](crate::stdcore::backends::latex::preamble::labels_piece)),
    /// and `\flmRef` is the macro that a user redefines to use a different
    /// reference command
    /// ([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)).
    ///
    /// When off, the recomposed LaTeX writes bare `\label{ref1}` commands and
    /// `\zcref[<options>]{ref1}`, and requests the `zref-clever` package.
    /// Several labels on one line of a display-math environment are then left
    /// to LaTeX: `amsmath` keeps the last label of a line, loses the ones
    /// before it, and warns `Multiple \label's`. A reference's options are
    /// then FLM's own option names passed unchanged to `\zcref`, since the
    /// hook's default is what translates them
    /// ([`refs_piece`](crate::stdcore::backends::latex::preamble::refs_piece)).
    ///
    /// **You own this when it is off.** A reference to a lost label prints
    /// `??`. You must provide either one label per line of a display-math
    /// environment or an aliasing mechanism of your own, and reference options
    /// that `zref-clever` accepts: `\ref[noprefix]{…}` stops the compilation,
    /// where the hook would have translated it to `noname`.
    pub label_hooks: bool,
    /// Whether the labels written on the lines of a display-math environment
    /// are left out of the recomposed LaTeX **in a registry that does not hold
    /// the references feature**. Off by default.
    ///
    /// The references feature ([`refs`](crate::stdcore::features::refs)) assigns
    /// the safe label of every label and resolves every `\ref`. A registry may
    /// be built without it (for example by a host that needs mathematics and
    /// little else), and an equation line may still carry a `\label` there.
    /// Such a label is written as the document wrote it, inside the
    /// `\flmLabels` group like every other label. This option says that it is
    /// not written at all, neither the `\label` command nor the group, for a
    /// host that keeps its mathematics free of anything but mathematics.
    ///
    /// **With the references feature registered, the option does nothing**,
    /// and nothing is reported: a label is then a reference point with a safe
    /// label, and a `\ref` in the document is written against that safe label,
    /// so removing the `\label` would leave the reference pointing at nothing.
    /// In such a registry the recomposed LaTeX is the same, byte for byte,
    /// whether this option is set or not.
    ///
    /// **Only mathematics is affected.** A heading's labels are written as
    /// they are, with or without the references feature, and so is a label
    /// written anywhere else. No option of this kind exists for headings.
    ///
    /// **You own this when it is on.** A label that the document wrote is then
    /// in no output: nothing in the recomposed LaTeX marks the equation line,
    /// so anything of yours that refers to it (LaTeX you write around the
    /// recomposed document, or a tool that reads the file) finds nothing.
    /// Nothing else about the line changes: a `\tag` and a `\nonumber` are not
    /// labels and are written as always.
    pub drop_math_labels_without_refs: bool,
    /// Whether a proof's **back-reference** is resolved into LaTeX hooks (on)
    /// or written as the author wrote it over the `phfthm` package (off, the
    /// default); the choice also decides which LaTeX package declares the
    /// theorem-like environments. The back-reference is the title
    /// `[*thm:x]`, which shows the statement that a proof proves, or
    /// `[**thm:x]`, which records the relation without showing it. Off by
    /// default.
    ///
    /// When off, the **authored form** is used: the back-reference is written
    /// as the author wrote it, `\begin{proof}[*ref1]` or `[**ref1]` around the
    /// statement's safe label, and the style file requests the `phfthm`
    /// package, which reads exactly this syntax on top of `amsthm`. With it,
    /// `[*ref1]` sets the heading "Proof of Theorem 1" where the document
    /// loads `hyperref` (the package refers to the statement through
    /// `hyperref`'s `\autoref`) and "Proof of 1" where it does not (through
    /// `\ref`), and `[**ref1]` sets "Proof"; the package also declares the
    /// theorem kinds through its own commands. The recomposed LaTeX does not
    /// request `hyperref` for this: whether a document loads it is its
    /// author's decision. The `phfthm` package is not part of every TeX
    /// installation: a document compiled where it is absent lists it in
    /// [`skip_packages`](Self::skip_packages) and defines the environments
    /// itself.
    ///
    /// When on, the **resolved form** is used, and the recomposed LaTeX needs
    /// `amsthm` alone. A back-reference that the proof shows is written inside
    /// the proof title hook `\flmProofTitle` around the reference hook
    /// `\flmRef` (LaTeX hooks, see
    /// [`preamble`](crate::stdcore::backends::latex::preamble)),
    /// `\begin{proof}[{\flmProofTitle{\flmRef{ref1}}}]`, which sets "Proof
    /// (theorem 1)" by default; `[**ref1]` writes no optional argument,
    /// `\begin{proof}`, and LaTeX sets its own "Proof".
    ///
    /// In both forms, a title that the author wrote on a proof is written
    /// inside the proof title hook, `[{\flmProofTitle{Sketch}}]`, and
    /// `\noproofref` is written as the author wrote it
    /// ([`theorems::latex`](crate::stdcore::features::theorems::latex)).
    pub resolved_proof_refs: bool,
}

/// When the style file is written: the value of the render option
/// [`style_file`](LatexRecomposedRenderOptions::style_file).
///
/// The style file ([`style`](crate::stdcore::backends::latex::style)) is the
/// `.sty` file written beside the recomposed LaTeX document, holding exactly
/// the package requests, character declarations and definitions that the
/// document needs to compile. A document that needs none of them (plain text
/// with no heading, label, reference, verbatim content or declared character)
/// needs no style file, unless its author asked for one through
/// [`extra_packages`](LatexRecomposedRenderOptions::extra_packages) or
/// [`extra_preamble`](LatexRecomposedRenderOptions::extra_preamble), whose
/// content is written only into the style file.
///
/// The regions stage ([`LatexRecomposedRegionsStage`]) reads this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StyleFile {
    /// The style file is written only where it would hold something. A
    /// document with no package request, no declared character, no definition
    /// piece and no user definition, whose author set neither
    /// [`extra_packages`](LatexRecomposedRenderOptions::extra_packages) nor
    /// [`extra_preamble`](LatexRecomposedRenderOptions::extra_preamble), gets
    /// no style artifact, and its recomposed LaTeX begins without the comment
    /// line that gives the style file's name. The default.
    #[default]
    WhenNeeded,
    /// The style file is always written, even where it holds nothing, for a
    /// host that expects a style file beside every recomposed output.
    Always,
}

impl StyleFile {
    /// The value's stable name, as the settings write it: `when-needed` or
    /// `always`.
    pub fn as_str(self) -> &'static str {
        match self {
            StyleFile::WhenNeeded => "when-needed",
            StyleFile::Always => "always",
        }
    }

    /// The value whose stable name is `name` ([`as_str`](Self::as_str)), or
    /// `None` when `name` is neither `when-needed` nor `always`.
    pub fn parse(name: &str) -> Option<StyleFile> {
        match name {
            "when-needed" => Some(StyleFile::WhenNeeded),
            "always" => Some(StyleFile::Always),
            _ => None,
        }
    }
}

impl Default for LatexRecomposedRenderOptions {
    fn default() -> LatexRecomposedRenderOptions {
        LatexRecomposedRenderOptions {
            unicode: UnicodeMode::Declare,
            unicode_overrides: BTreeMap::new(),
            drop_comments: false,
            skip_packages: BTreeSet::new(),
            extra_packages: Vec::new(),
            extra_preamble: ExtraPreamble::default(),
            bare_names: BTreeSet::new(),
            robust_names: BTreeSet::new(),
            style_stem: String::from(DEFAULT_STYLE_STEM),
            style_file: StyleFile::WhenNeeded,
            heading_hook: true,
            label_hooks: true,
            drop_math_labels_without_refs: false,
            resolved_proof_refs: false,
        }
    }
}


/// The feature that registers the `latex-recomposed` backend (see the module
/// documentation).
///
/// Use [`feature()`] to obtain it as a value that a package entry can list.
/// When registered, it declares:
///
/// - the tables: the lowered trees that its lowering stage writes
///   (`flm.lowered.latex-recomposed`), the three tables that its emit stage
///   writes, the lowered pieces (document-level values such as the title,
///   placed beside the content; [`Piece`](crate::core::piece::Piece)), the
///   output regions, the style-file table,
///   the artifact table and the artifact set (see [`mod@super::stages`]);
/// - the settings, with the render options
///   ([`LatexRecomposedRenderOptions`]);
/// - the pieces stage ([`PiecesStage`]), which lowers only the pieces that
///   the render request asks to place, and the regions stage
///   ([`LatexRecomposedRegionsStage`]);
/// - its regions output ([`RegionsOutput`]), contributed to the service slot
///   `flm.output.regions` so that a host can assemble the output's regions
///   through a template of its own;
/// - the two structural lowering rules ([`super::rules()`]);
/// - the backend ([`BackendDef`]), with its native element set and its
///   lowering, emit and final stages ([`super::stages()`]);
/// - the `latex-recomposed` output ([`OutputDef`]) and the
///   `latex-recomposed` workflow ([`WorkflowDef`]).
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRecomposed;

impl FlmFeature for LatexRecomposed {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE_NAME, "LaTeX (recomposed) backend").docs(
            "the document recomposed into LaTeX from the raw tree: the `latex-recomposed` \
             output and workflow, a body and a style file",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table_named::<LoweredTrees>(
            lowered_table_name(&BACKEND.into()),
            "the recomposed backend's lowered trees, one per document tree",
        )?;
        b.table(
            OUTPUT_TABLE,
            "per tree: the recomposed LaTeX, the characters it declares, the requirement \
             identifiers it needs",
        )?;
        b.table(PACKAGES_TABLE, "the package identifiers the recomposed trees require")?;
        b.table(
            PREAMBLE_TABLE,
            "the definition pieces the recomposed trees require and the user definitions \
             they recomposed",
        )?;
        b.table(DOCUMENT, "the recomposed LaTeX and its style file as an artifact set")?;
        b.table_named::<LoweredPieces>(
            lowered_pieces_table_name(&BACKEND.into()),
            "the pieces that the render request places, lowered for the recomposed backend",
        )?;
        // A LaTeX document places its pieces where LaTeX wants them (the
        // title in `\title`), which a template knows and the fixed skeleton
        // does not: the backend places none unless a render names them.
        b.stage(Arc::new(PiecesStage::new(BACKEND).placing_only_named()))?;
        b.table(REGIONS, "the body, the pieces and the style file's package name")?;
        b.table(STYLE_FILE, "the style file beside the recomposed LaTeX, where it has one")?;
        b.table(
            ARTIFACTS,
            "files a stage produces beside the recomposed LaTeX for it to refer to (copies of images), \
             added to the document's set after the style file",
        )?;
        b.stage(Arc::new(LatexRecomposedRegionsStage))?;
        b.contribute_to::<RegionsOutput>(
            REGIONS_SLOT,
            Arc::new(RegionsOutput {
                output: BACKEND.into(),
                backend: BACKEND.into(),
                document: DOCUMENT.name().into(),
                capability: DOCUMENT.name().into(),
                regions: REGIONS.handle(),
                artifacts: vec![STYLE_FILE.handle(), ARTIFACTS.handle()],
            }),
        )?;
        for rule in rules() {
            b.lowering_rule(rule)?;
        }
        let (lowering, emit, final_stage) = stages();
        b.backend(
            BackendDef::new(BACKEND, native_elements(), lowering, emit, final_stage)
                .docs("LaTeX recomposed from the raw tree, with a style file"),
        )?;
        b.output(
            OutputDef::new(BACKEND, DOCUMENT.name(), DOCUMENT.name(), MEDIA_TYPE, EXTENSION)
                .docs("the document recomposed into LaTeX, with its style file"),
        )?;
        b.workflow(
            WorkflowDef::new(WORKFLOW)
                .output(BACKEND)
                .demand(VALIDATED)
                .docs(
                    "recompose the document into LaTeX, running every check and leaving \
                     every resolution to LaTeX",
                ),
        )?;
        Ok(())
    }
}

/// Returns the [`LatexRecomposed`] feature as a shared value, ready to be
/// listed in a package entry.
///
/// A package that lists it must also list three other features, or the
/// registry build fails with an error that identifies the missing name:
///
/// - the `lowering` feature
///   ([`lowering::feature`](crate::stdcore::backends::lowering::feature())),
///   which registers the shared presentation elements;
/// - the `latex` feature
///   ([`latex::feature`](crate::stdcore::backends::latex::feature())), which
///   registers the LaTeX construct elements that this backend writes natively
///   and the service slots that its regions stage reads;
/// - the `check` feature ([`check_feature`](crate::stdcore::check_feature)),
///   which declares the [`flm.validated`](crate::stdcore::VALIDATED)
///   collection that this backend's workflow demands beside its output.
///
/// It depends on no content feature: each standard feature registers its own
/// recomposition rule for this backend, and an absent feature contributes
/// none. When the registry holds an element that a document may contain and
/// that no selected rule leads to the native set, building a plan for this
/// backend fails ([`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete)),
/// unless the plan request accepts that element as unrenderable
/// ([`PlanRequest::unsupported`](crate::core::plan::PlanRequest::unsupported)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_recomposed;
///
/// assert_eq!(latex_recomposed::feature().info().name.as_str(), latex_recomposed::FEATURE_NAME);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(LatexRecomposed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_native_set_is_exactly_the_fifteen_elements_of_the_design() {
        let native = native_elements();
        let names: Vec<&str> = native.iter().map(|element| element.as_str()).collect();
        assert_eq!(
            names,
            [
                "flm.pres.latex.macro",
                "flm.pres.latex.environment",
                "flm.pres.latex.raw",
                "flm.pres.latex.definition",
                "flm.pres.math-passthrough",
                "flm.pres.verbatim",
                "flm.pres.unknown",
                "flm.pres.unsupported",
                "flm.pres.dropped",
                "flm.pres.subtree",
                "flm.pres.span",
                "flm.text",
                "flm.group",
                "flm.comment",
                "flm.container",
            ]
        );
    }

    #[test]
    fn the_options_default_as_the_design_says() {
        let options = LatexRecomposedRenderOptions::default();
        assert_eq!(options.unicode, UnicodeMode::Declare);
        assert!(options.unicode_overrides.is_empty());
        assert!(!options.drop_comments);
        assert!(options.skip_packages.is_empty());
        assert!(options.extra_packages.is_empty());
        assert!(options.extra_preamble.is_empty());
        assert!(options.bare_names.is_empty());
        assert!(options.robust_names.is_empty());
        assert_eq!(options.style_stem, DEFAULT_STYLE_STEM);
        assert_eq!(options.style_file, StyleFile::WhenNeeded);
        assert!(options.heading_hook);
        assert!(options.label_hooks);
        assert!(!options.drop_math_labels_without_refs);
        assert!(!options.resolved_proof_refs);
    }

    #[test]
    fn the_style_file_setting_round_trips_through_its_name() {
        for value in [StyleFile::WhenNeeded, StyleFile::Always] {
            assert_eq!(StyleFile::parse(value.as_str()), Some(value));
        }
        assert_eq!(StyleFile::parse("sometimes"), None);
    }
}
