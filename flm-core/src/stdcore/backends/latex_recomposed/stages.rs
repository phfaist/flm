//! The emit, regions and final stages of the `latex-recomposed` backend, the
//! tables they write, the artifact set they produce, and the conditions they
//! report (a condition is a kind of diagnostic, identified by a name such as
//! `flm.latex-recomposed.stage-failed`; see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)).
//!
//! A stage ([`Stage`]) is one unit of work of the pipeline; the plan decides
//! whether and when it runs. This backend's output is produced by three
//! stages of its own, which run after its lowering stage (the
//! [`LoweringStage`] for `latex-recomposed`); the regions stage also reads
//! what the pieces stage
//! ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage)) lowered:
//!
//! 1. the **emit stage** ([`LatexRecomposedEmitStage`], `flm.latex-recomposed.emit`)
//!    writes each lowered tree as LaTeX;
//! 2. the **regions stage** ([`LatexRecomposedRegionsStage`],
//!    `flm.latex-recomposed.regions`) writes the placed pieces as LaTeX,
//!    assembles the style file, and writes the output regions;
//! 3. the **final stage** ([`LatexRecomposedFinalStage`],
//!    `flm.latex-recomposed.final`) assembles the entry artifact from the
//!    regions and writes the artifact set.
//!
//! # The emit stage
//!
//! The emit stage runs once per document tree (scope
//! [`Scope::PerTree`]). It writes the tree's recomposed LaTeX ([`emit_tree`])
//! and writes three tables:
//!
//! - per tree, the recomposed LaTeX with the characters it declares, the
//!   chunks of the Unicode-to-LaTeX table of [`untechxt`]
//!   (the table that gives each character a LaTeX spelling and states what
//!   that spelling needs) that their spellings need (package chunks, see
//!   [`packages`]), and the requirement identifiers it needs ([`LatexFragments`],
//!   `flm.output.latex-recomposed`);
//! - the identifiers of the LaTeX packages that the trees require
//!   ([`LatexPackages`], `flm.latex-recomposed.packages`);
//! - the definition pieces that the trees require, and the user definitions
//!   they recomposed ([`LatexPreamble`], `flm.latex-recomposed.preamble`).
//!
//! A requirement is an identifier that a lowering rule or the emitter records
//! to say that its output needs a definition: on LaTeX, a package request
//! (`flm.latex.pkg.<name>`, see
//! [`packages`]) or a definition
//! piece (`flm.latex.def.<name>`, see [`LatexPreambleDef`]).
//!
//! # The regions stage
//!
//! The regions stage runs once per document (scope [`Scope::Document`]). It:
//!
//! - checks the entries of the render options
//!   [`bare_names`](super::LatexRecomposedRenderOptions::bare_names) and
//!   [`robust_names`](super::LatexRecomposedRenderOptions::robust_names)
//!   (below);
//! - writes as LaTeX the [pieces](crate::core::piece) that the render request
//!   places
//!   (the document-level values such as the title, read from the lowered
//!   pieces table `flm.lowered.latex-recomposed.pieces`), each as a tree is
//!   written;
//! - reads the three tables of the emit stage and resolves every requirement
//!   identifier of the trees and of the pieces through the two service slots
//!   of the `latex` feature ([`packages`], [`preamble`]);
//! - assembles the **style file** and writes it into the style-file table
//!   `flm.output.latex-recomposed.style` ([`STYLE_FILE`]);
//! - writes the output regions ([`Regions`]) into
//!   `flm.output.latex-recomposed.regions` ([`REGIONS`]). An output region is
//!   one named part of an output's entry artifact. This backend writes three
//!   sorts of output region: the body, which is the recomposed trees in
//!   document order, each
//!   followed by the comment line `%%`; one region per placed piece; and the
//!   style file's package name when there is a style file. The comment line
//!   `%%` ends each tree's last line whether or not the tree ended with a
//!   newline, so that two trees never run into one line and no line break is
//!   added that the author did not write.
//!
//! # The final stage and the artifact set
//!
//! The final stage runs once per document. It is the backend's final stage:
//! it assembles the output's entry artifact from the output regions, in a
//! layout of the backend's own (see [output regions and the final
//! stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)),
//! and writes the artifact set `flm.output.latex-recomposed.document`
//! ([`DOCUMENT`]), the table of the `latex-recomposed` output, which the
//! workflow of that name lists. The set also holds the style file and the
//! artifacts of the artifact table `flm.output.latex-recomposed.artifacts`
//! ([`ARTIFACTS`]), such as the copies of images that the `graphics` feature
//! writes. A host that assembles the regions through a template of its own
//! provides the same table from the same regions, and the plan runs one of
//! the two. The set holds:
//!
//! - `latex-recomposed:document`, the entry artifact, with media type
//!   `application/x-latex` and extension `tex`. The final stage writes a
//!   comment line that gives the style file's name, then the body; a
//!   template writes its own text around the same regions.
//! - `latex-recomposed:<stem>.<hash>`, the style file, with media type
//!   `application/x-latex`, the extension `sty`, and a fixed path, from which
//!   its LaTeX package name is the path without `.sty`. The stem is the render
//!   option [`style_stem`](super::LatexRecomposedRenderOptions::style_stem);
//!   the hash is the first eight hexadecimal digits of the digest of the
//!   file's content without its header ([`content_hash`]). Two documents'
//!   style files therefore never clash in one directory, and identical
//!   documents share one. [`style_artifact`] finds it in a set.
//!
//! # The style file's path
//!
//! The document loads the style file with `\usepackage{<name>}`, so its path
//! is fixed when the document is written. The regions stage asks the host's
//! naming service ([`ArtifactNamer`], the service slot `flm.artifacts.namer`)
//! for the path of the identity `latex-recomposed:<stem>.<hash>` with the
//! extension `sty`, and sets it as the artifact's fixed path; the package name
//! is that path without `.sty`. A host's scheme may so prefix the name with
//! the name of the document's file (`paper-flmrecomposed.1a2b3c4d.sty`, loaded
//! as `\usepackage{paper-flmrecomposed.1a2b3c4d}`). Without a naming service
//! (a registry built without a host), the file is `<stem>.<hash>.sty`. LaTeX
//! loads the file from the document's folder by its name, so a path that is
//! not a bare file name ending in `.sty` is refused ([`StylePathRefused`]), and
//! the file is then named as without a naming service.
//!
//! # When the style file is written
//!
//! Under [`StyleFile::WhenNeeded`], the default, a document that requires no
//! package, declares no character, needs no definition piece and recomposes
//! no user definition gets no style artifact at all, and its recomposed LaTeX
//! has no comment line that gives a style file's name: the artifact set holds
//! the document alone. Under [`StyleFile::Always`] the file is written even
//! where it holds nothing, for a host that expects one beside every
//! recomposed output. With the render options
//! [`heading_hook`](super::LatexRecomposedRenderOptions::heading_hook) and
//! [`label_hooks`](super::LatexRecomposedRenderOptions::label_hooks) on (the
//! default), any heading,
//! label or reference requires a definition piece, so a document that holds
//! one of them always has a style file. So does a document whose author set
//! [`extra_packages`](super::LatexRecomposedRenderOptions::extra_packages) or
//! any part of
//! [`extra_preamble`](super::LatexRecomposedRenderOptions::extra_preamble),
//! since what these options ask for is written only into the style file.
//!
//! # What the author adds to the style file
//!
//! Two render options put the author's own LaTeX into the style file, and
//! neither is derived from the document.
//! [`extra_packages`](super::LatexRecomposedRenderOptions::extra_packages) is
//! a list of package requests ([`LatexPackageDef`] values, each a package
//! name with the options it is loaded with), which join the document's own
//! requests before they are merged, after them and in the order given.
//! [`extra_preamble`](super::LatexRecomposedRenderOptions::extra_preamble) is
//! LaTeX text written as given in three places (before the package block,
//! after it, and at the very end before `\endinput`), each part under a
//! comment line of its own, byte for byte and unchecked. The user who sets
//! either option is responsible for what it adds. **An explicit request takes
//! precedence over
//! [`skip_packages`](super::LatexRecomposedRenderOptions::skip_packages)**:
//! that option stops the LaTeX family's and the document's requests for a
//! package, never the author's own. The author's requests for a package that
//! `skip_packages` lists form a group of their own: they are merged with each
//! other by the rule that applies to every package, and with nothing that
//! this library requested, so the file holds one line for that package with
//! the author's options and no others. Listing a package in `skip_packages`
//! and requesting it in `extra_packages` is how an author states an option
//! list that this library's own requests cannot add to.
//!
//! # What the style file holds
//!
//! **A definition piece may need other definition pieces**, as well as
//! packages ([`LatexPreambleDef::requires`]). Before selecting definition
//! pieces in registration order, the regions stage adds every definition
//! piece that a required one needs, and so on recursively. A lowering rule
//! therefore requires only the definition piece its own output uses. The
//! definition piece of the `\flmHeading` hook, for example, brings in the
//! definition pieces for labels, for display texts and for the `zref-clever`
//! reference types.
//!
//! **The package block holds one `\RequirePackage` line per package.** The
//! requests that go into it are, in identifier order: every package
//! identifier that the trees required, every package that each required
//! definition piece needs, and every package that the declared characters'
//! spellings need, minus the packages that
//! [`skip_packages`](super::LatexRecomposedRenderOptions::skip_packages)
//! lists; then the author's own requests, in the order given. The requests
//! are merged per package and per engine
//! ([`merge_requests`](packages::merge_requests)). A request applies to a set
//! of engines: an author's request and the LaTeX family's own requests apply
//! to every engine, and one derived from a chunk of the Unicode-to-LaTeX table
//! of [`untechxt`] applies to the engines that chunk is for. The options of one package's
//! line under one engine are the union of the options of its requests that
//! apply to that engine, each once, in the order first seen, and the comment
//! line above the line lists every identifier behind it. A package whose
//! requests give one option key two different values is reported as
//! [`ConflictingPackageOptions`], an Error, and no line is written for it
//! under the engines to which both requests apply. The author's requests for a
//! package that `skip_packages` lists are merged among themselves by the same
//! rule, and their lines follow the others. A package whose line is the same
//! under every engine is written once; where the lines differ, the block first
//! requests the package `iftex` and writes them inside its test of the engine,
//! so that each engine reads its own line and the document is the same
//! regardless of the engine that compiles it. Where the block loads `fontenc` with an
//! engine's own default encoding (`T1` under pdfLaTeX, `TU` under LuaLaTeX and
//! XeLaTeX), the command that makes it the default follows the block, in a
//! test of the engine of its own, so that the order of the merged encodings
//! does not matter ([`style`](crate::stdcore::backends::latex::style)).
//!
//! After the package block come the character declarations of the `Declare`
//! Unicode mode; the required definition pieces (the trees' own, and the ones
//! a declared character's spelling needs where it needs declarations rather
//! than a package), in the order the registry holds them; the recomposed
//! user definitions, in order of first occurrence; and last the author's own
//! text for the end of the file. Everything this library writes into the file
//! is either guarded or a LaTeX hook, so a user can define a command before
//! the file is loaded or redefine it after
//! ([`style`](crate::stdcore::backends::latex::style)). The exceptions are the
//! author's own three parts of text, which are written as given, and the
//! definitions that `bare_names` puts under the document's own names, which
//! are written unconditionally.
//!
//! A requirement identifier for which no feature contributed a definition is
//! reported as [`UnknownRequirement`], an Error, and is left out of the file.
//!
//! # Checking the render options that list user definitions
//!
//! The regions stage also checks the *content* of the two render options that
//! list user definitions. An entry of
//! [`bare_names`](super::LatexRecomposedRenderOptions::bare_names) or of
//! [`robust_names`](super::LatexRecomposedRenderOptions::robust_names) that
//! cannot be honored is reported as [`DefinitionNameRefused`], an Error, once
//! per entry per document, with the option it belongs to
//! ([`DefinitionNameOption`]) and the reason ([`DefinitionNameRefusal`]). The
//! reasons are of two sorts, and only the first applies to `robust_names`:
//!
//! - A name that begins with `flm`, the prefix of every LaTeX name this
//!   library writes itself, is refused regardless of what the document holds,
//!   and the entry is ignored.
//! - An entry that the document's own definitions make impossible is detected
//!   from the definitions that the style file holds, where the whole document
//!   is known: a name declared as both a macro and an environment, or a macro
//!   `end<name>` beside an environment `<name>`, would become one LaTeX
//!   command, since `\DeclareDocumentEnvironment{<name>}` defines `\<name>`
//!   and `\end<name>` itself; and a name that the document declares twice,
//!   once per parsing mode, has two per-mode definitions, which are written
//!   under two different prefixed names and cannot share the one name the
//!   entry asks for.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use techy::error::{Diagnostic, DiagnosticInfo, DiagnosticValue, ToDiagnosticValue};
use techy::serialize::SerialValue;

use crate::core::artifact::{
    Artifact, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, RelPath,
};
use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::model::addr::TreeId;
use crate::model::names::{BackendName, Name};
use crate::lang::{nowhere, FlmOrigin};
use crate::stdcore::backends::latex::style::{content_hash, style_body, with_header};
use crate::stdcore::backends::latex::{
    Declared, LatexPackageDef, LatexPreambleDef, RecomposedDefinition, names, packages, preamble,
};
use crate::core::region::{CONTENT, Region, Regions, STYLE_PACKAGE};
use crate::stdcore::backends::lowering::{LoweringStage, NAMER_SLOT, lowered_table};
use crate::stdcore::backends::pieces::lowered_pieces_table;

use super::emit::{LatexFragment, NOWHERE_LABEL, emit_tree};
use super::feature::{BACKEND, LatexRecomposedRenderOptions, StyleFile};

/// The registered name of the `latex-recomposed` workflow, which lists the
/// `latex-recomposed` output and demands the collection `flm.validated` (see
/// the [`feature`](mod@super::feature) module documentation). It is
/// spelled like the backend's name but is a workflow name, the kind of name
/// that [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) takes.
pub const WORKFLOW: &str = "latex-recomposed";
/// The registered name of the emit stage ([`LatexRecomposedEmitStage`]).
pub const EMIT_STAGE: &str = "flm.latex-recomposed.emit";
/// The registered name of the regions stage ([`LatexRecomposedRegionsStage`]).
pub const REGIONS_STAGE: &str = "flm.latex-recomposed.regions";
/// The registered name of the final stage ([`LatexRecomposedFinalStage`]).
pub const FINAL_STAGE: &str = "flm.latex-recomposed.final";
/// The owner part of the identifiers of the entry artifact and the style
/// artifact, `latex-recomposed` ([`ArtifactId`]). The style artifact's name
/// part is `<stem>.<hash>` (see the module documentation); [`style_artifact`]
/// finds it in a set.
pub const ARTIFACT_OWNER: &str = "latex-recomposed";
/// The name part of the entry artifact's identifier,
/// `latex-recomposed:document`.
pub const DOCUMENT_ARTIFACT: &str = "document";
/// The media type of the entry artifact and the style artifact,
/// `application/x-latex`.
pub const MEDIA_TYPE: &str = "application/x-latex";
/// The file extension, `tex`, that a host writing the entry artifact to a
/// file should use.
pub const EXTENSION: &str = "tex";
/// The file extension of the style file, `sty`, which the style artifact
/// carries ([`Artifact::extension`]).
pub const STYLE_EXTENSION: &str = "sty";
/// What the regions stage writes into the body after each tree's recomposed
/// LaTeX: the comment line `%%`, which ends the tree's last line (see the
/// module documentation).
pub const TREE_END: &str = "%%\n";

/// The typed name of the per-tree output table, `flm.output.latex-recomposed`
/// ([`LatexFragments`]), which the emit stage writes.
pub const OUTPUT_TABLE: TableName<LatexFragments> = TableName::new("flm.output.latex-recomposed");
/// The typed name of the packages table, `flm.latex-recomposed.packages`
/// ([`LatexPackages`]), which the emit stage writes.
pub const PACKAGES_TABLE: TableName<LatexPackages> =
    TableName::new("flm.latex-recomposed.packages");
/// The typed name of the preamble table, `flm.latex-recomposed.preamble`
/// ([`LatexPreamble`]), which the emit stage writes.
pub const PREAMBLE_TABLE: TableName<LatexPreamble> =
    TableName::new("flm.latex-recomposed.preamble");
/// The typed name of the regions table, `flm.output.latex-recomposed.regions`,
/// which the regions stage writes: the output regions of the body, of each
/// placed piece, and of the style file's package name (see
/// [`crate::core::region`]).
pub const REGIONS: TableName<Regions> = TableName::new("flm.output.latex-recomposed.regions");
/// The typed name of the style-file table, `flm.output.latex-recomposed.style`:
/// an artifact set holding the style file where the document has one, and
/// nothing otherwise. The regions stage writes it, and the final stage adds
/// its artifact to the output's artifact set after the entry artifact.
pub const STYLE_FILE: TableName<ArtifactSet> = TableName::new("flm.output.latex-recomposed.style");
/// The typed name of the artifact table, `flm.output.latex-recomposed.artifacts`:
/// the files that a stage of another feature writes beside the document for
/// the document to refer to, such as the copies of images that the
/// `graphics` feature's publish stage writes. No stage of this backend writes
/// it; the final stage adds its artifacts to the output's artifact set after
/// the style file, when a stage of the plan wrote it.
pub const ARTIFACTS: TableName<ArtifactSet> =
    TableName::new("flm.output.latex-recomposed.artifacts");
/// The typed name of the artifact-set table,
/// `flm.output.latex-recomposed.document`, which the final stage writes: the
/// table of the `latex-recomposed` output, whose name is also the capability
/// that the final stage provides.
pub const DOCUMENT: TableName<ArtifactSet> = TableName::new("flm.output.latex-recomposed.document");

// --- the tables -----------------------------------------------------------------------------

/// The per-tree output table, `flm.output.latex-recomposed`
/// ([`OUTPUT_TABLE`]): one [`LatexFragment`] per document tree, written by the
/// emit stage and read by the regions stage.
#[derive(Debug, Clone, Default)]
pub struct LatexFragments {
    /// Each document tree's recomposed LaTeX and record.
    pub trees: BTreeMap<TreeId, LatexFragment>,
}

impl Table for LatexFragments {
    fn absorb(&mut self, piece: LatexFragments) {
        self.trees.extend(piece.trees);
    }

    /// One entry per tree: `{tree, text, declared: {char: spelling}, chunks:
    /// [id], requires: [id]}`.
    ///
    /// `chunks` lists what the tree's spellings need in the preamble: the
    /// identifiers of the chunks of the Unicode-to-LaTeX table of
    /// [`untechxt`]
    /// ([`PreambleNeeds`](crate::untechxt::preamble::PreambleNeeds)), in the
    /// order the tree first needed them. A dump therefore shows a change in
    /// what a document requires, not only in the characters it declares.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.trees
                .iter()
                .map(|(id, fragment)| {
                    SerialValue::Map(vec![
                        (String::from("tree"), cx.tree_ref(*id)),
                        (String::from("text"), SerialValue::Str(fragment.text.clone())),
                        (
                            String::from("declared"),
                            SerialValue::Map(
                                fragment
                                    .declared
                                    .chars()
                                    .iter()
                                    .map(|(ch, spelling)| {
                                        (ch.to_string(), SerialValue::Str(spelling.clone()))
                                    })
                                    .collect(),
                            ),
                        ),
                        (
                            String::from("chunks"),
                            SerialValue::List(
                                fragment
                                    .declared
                                    .needs()
                                    .chunks()
                                    .map(|chunk| SerialValue::Str(String::from(chunk.id())))
                                    .collect(),
                            ),
                        ),
                        (String::from("requires"), names(fragment.requires.iter())),
                    ])
                })
                .collect(),
        )
    }
}

/// The packages table, `flm.latex-recomposed.packages` ([`PACKAGES_TABLE`]):
/// every requirement identifier that the trees need and that is not a
/// definition piece, in identifier order. These are the package identifiers,
/// and any identifier of another form, which the regions stage reports as
/// [`UnknownRequirement`] when no package definition has it.
#[derive(Debug, Clone, Default)]
pub struct LatexPackages {
    /// The requirement identifiers.
    pub ids: BTreeSet<Name>,
}

impl Table for LatexPackages {
    fn absorb(&mut self, piece: LatexPackages) {
        self.ids.extend(piece.ids);
    }

    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        names(self.ids.iter())
    }

    fn piece_class(&self) -> PieceClass {
        if self.ids.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

/// The preamble table, `flm.latex-recomposed.preamble` ([`PREAMBLE_TABLE`]):
/// the definition pieces that the trees require, and the user definitions
/// that their definition nodes recomposed.
#[derive(Debug, Clone, Default)]
pub struct LatexPreamble {
    /// The definition-piece identifiers, in identifier order.
    pub pieces: BTreeSet<Name>,
    /// The recomposed user definitions, each with the name it defines, the
    /// name the document declared and its LaTeX text, once per name, in order
    /// of first occurrence across the trees.
    pub definitions: Vec<RecomposedDefinition>,
}

impl LatexPreamble {
    /// Adds `definition` unless one of that name is already recorded.
    fn add_definition(&mut self, definition: RecomposedDefinition) {
        if !self.definitions.iter().any(|seen| seen.name == definition.name) {
            self.definitions.push(definition);
        }
    }
}

impl Table for LatexPreamble {
    /// The definition pieces are joined as a set; when a definition of the
    /// same name is already recorded, the earlier one is kept.
    fn absorb(&mut self, piece: LatexPreamble) {
        self.pieces.extend(piece.pieces);
        for definition in piece.definitions {
            self.add_definition(definition);
        }
    }

    /// `{pieces: [id], definitions: [{name, source?, feature?, text}]}`.
    /// `source` is the name the document declared, present only when it
    /// differs from `name`; `feature` is the feature that wrote the definition
    /// from its configuration, present only for such a definition.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(vec![
            (String::from("pieces"), names(self.pieces.iter())),
            (
                String::from("definitions"),
                SerialValue::List(
                    self.definitions
                        .iter()
                        .map(|definition| {
                            let mut entries = vec![(
                                String::from("name"),
                                SerialValue::Str(definition.name.clone()),
                            )];
                            if definition.source_name != definition.name {
                                entries.push((
                                    String::from("source"),
                                    SerialValue::Str(definition.source_name.clone()),
                                ));
                            }
                            if let Some(feature) = &definition.feature {
                                entries.push((
                                    String::from("feature"),
                                    SerialValue::Str(feature.clone()),
                                ));
                            }
                            entries.push((
                                String::from("text"),
                                SerialValue::Str(definition.text.clone()),
                            ));
                            SerialValue::Map(entries)
                        })
                        .collect(),
                ),
            ),
        ])
    }

    fn piece_class(&self) -> PieceClass {
        if self.pieces.is_empty() && self.definitions.is_empty() {
            PieceClass::Empty
        } else {
            PieceClass::DocumentFact
        }
    }
}

/// A list of names as serial data.
fn names<'n>(names: impl Iterator<Item = &'n Name>) -> SerialValue {
    SerialValue::List(names.map(|name| SerialValue::Str(name.to_string())).collect())
}

// --- conditions -----------------------------------------------------------------------------

/// Condition `flm.latex-recomposed.stage-failed`, an Error: a stage of this
/// backend could not complete its work.
///
/// The emit, regions or final stage reports it when it cannot read or write a
/// table it declared (the plan does not match the stage's declarations) or
/// read its render options, and the regions or final stage when the artifact
/// set refuses an artifact that the stage adds to it. The emitter reports it
/// when it finds a node that breaks an invariant of the lowering rules (an
/// element that the backend neither writes natively nor expects, a construct
/// node without its payload, a packed call with an argument that is not a
/// group), or when raw text that a lowering rule assembled holds a `%`.
/// Recovery: the stage's output is absent (for the emit stage, for that
/// tree), or the emitter leaves out what it cannot write (see the
/// [`emit`](super::emit) module documentation).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.stage-failed",
    message = "the stage `{stage}` failed: {detail}"
)]
pub struct StageFailed {
    /// The registered name of the stage.
    pub stage: String,
    /// What failed, as text.
    pub detail: String,
}

/// Condition `flm.latex-recomposed.style-path`, an Error: the host's naming
/// service ([`ArtifactNamer`]) placed the style file at a path that LaTeX
/// cannot load by its name from the document's folder, one that is not a bare
/// file name ending in `.sty` (see the [module
/// documentation](self#the-style-files-path)). The regions stage reports it.
/// Recovery: the style file is named as without a naming service,
/// `<stem>.<hash>.sty`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.style-path",
    message = "the style file must be a file beside the LaTeX document, named `NAME.sty`, and the \
               program's naming of files places it at `{path}`"
)]
pub struct StylePathRefused {
    /// The path the naming service gave.
    pub path: String,
}

/// Returns the style file of the set `set`: its artifact of owner
/// `latex-recomposed` ([`ARTIFACT_OWNER`]) other than the entry artifact, or
/// `None` when the set holds no style file.
pub fn style_artifact(set: &ArtifactSet) -> Option<&Artifact> {
    set.iter().find(|artifact| artifact.id.owner.as_str() == ARTIFACT_OWNER && !artifact.entry)
}

/// Returns the path of the style file whose identity is `id` and whose name
/// without a naming service is `default` (`<stem>.<hash>`): the path the
/// registry's naming service gives, when it has one and the path is a bare
/// file name ending in `.sty`, else `<default>.sty`. Reports
/// [`StylePathRefused`] for a path that is not.
fn style_path(cx: &StageCx<'_>, id: &ArtifactId, default: &str, out: &mut StageOut<'_>) -> String {
    let fallback = format!("{default}.{STYLE_EXTENSION}");
    let Some(namer) = cx.services_all().service_named::<dyn ArtifactNamer>(NAMER_SLOT) else {
        return fallback;
    };
    let artifact = ArtifactRef {
        id,
        entry: false,
        media_type: MEDIA_TYPE,
        extension: Some(STYLE_EXTENSION),
        output: None,
        folder: false,
    };
    let path = namer.url_of(&artifact, &NamingScope::of(cx.registry(), cx.plan().demands()));
    let path = path.as_str();
    let loadable = path
        .strip_suffix(".sty")
        .is_some_and(|name| !name.is_empty() && !name.contains(['/', '\\']));
    if loadable {
        return path.to_string();
    }
    out.diags.push(Diagnostic::error(
        StylePathRefused { path: path.to_string() },
        nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
    ));
    fallback
}

/// Condition `flm.latex-recomposed.unknown-requirement`, an Error: a lowering
/// rule, the emitter or a definition piece required an identifier for which no
/// feature contributed a package definition
/// ([`LatexPackageDef`]) or a definition piece ([`LatexPreambleDef`]).
///
/// It usually means that the feature that defines the identifier is not
/// registered. Recovery: the style file leaves the identifier out, and the
/// recomposed LaTeX may then not compile.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.unknown-requirement",
    message = "the LaTeX output requires `{id}`, but no registered feature defines a LaTeX \
               package or LaTeX definitions under that identifier"
)]
pub struct UnknownRequirement {
    /// The requirement identifier.
    pub id: String,
}

/// Condition `flm.latex-recomposed.package-options-conflict`, an Error: two
/// requests for one LaTeX package give the same option key different values,
/// so the single `\RequirePackage` line that the style file writes for the
/// package cannot hold both.
///
/// A *flag* (an option written without a value, such as `draft`) and a
/// `key=value` option with that same key count as different values, because
/// the library cannot know what the package's own default for the key is.
/// Recovery: the style file does not request that package at all (under the
/// engines to which both requests apply), so the recomposed LaTeX may not
/// compile until one of the two requests is removed.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.package-options-conflict",
    message = "the LaTeX package `{package}` is requested both with `{first}` and with \
               `{second}`, which give its option `{key}` two different values"
)]
pub struct ConflictingPackageOptions {
    /// The package's name, as `\RequirePackage` takes it (`geometry`).
    pub package: String,
    /// The option key the two requests disagree on (`papersize`).
    pub key: String,
    /// The option as written by the request seen first (`papersize=a4`).
    pub first: String,
    /// The option as written by the request that disagrees with the first
    /// (`papersize=letter`).
    pub second: String,
}

/// The render option whose entry [`DefinitionNameRefused`] reports: the
/// option that listed a user definition which could not be written as the
/// option asked.
///
/// Two render options of this backend list user definitions by the name the
/// document declared them under, and both refuse a name that begins with
/// `flm`, the prefix reserved for the LaTeX names this library writes itself
/// ([`DefinitionNameRefusal::FamilyPrefix`]). One condition reports a refused
/// entry of either option, and this value says which option the entry
/// belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionNameOption {
    /// [`bare_names`](super::LatexRecomposedRenderOptions::bare_names): the
    /// definitions written under the document's own names rather than under a
    /// prefix.
    BareNames,
    /// [`robust_names`](super::LatexRecomposedRenderOptions::robust_names):
    /// the definitions written as robust commands rather than as expandable
    /// macros.
    RobustNames,
}

impl DefinitionNameOption {
    /// The option's name, `bare_names` or `robust_names`: the field name of
    /// [`LatexRecomposedRenderOptions`], used in the message and as the value
    /// in diagnostic data.
    pub fn as_str(&self) -> &'static str {
        match self {
            DefinitionNameOption::BareNames => "bare_names",
            DefinitionNameOption::RobustNames => "robust_names",
        }
    }
}

/// Diagnostic data: the option's name
/// ([`as_str`](DefinitionNameOption::as_str)).
impl ToDiagnosticValue for DefinitionNameOption {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        self.as_str().to_diagnostic_value()
    }
}

/// Why an entry of a render option that lists user definitions is refused:
/// the reason carried by [`DefinitionNameRefused`].
///
/// An entry of
/// [`bare_names`](super::LatexRecomposedRenderOptions::bare_names) asks for a
/// user definition to be written into the style file under the name the
/// document declared, instead of under the prefix that every recomposed
/// definition otherwise takes. Each variant is a reason why that name cannot
/// be used: the library reserves the name, two of the document's own
/// definitions would then be written under one LaTeX command, or the name
/// covers two per-mode definitions.
///
/// An entry of
/// [`robust_names`](super::LatexRecomposedRenderOptions::robust_names) — the
/// definitions written as robust commands rather than as expandable macros —
/// is refused for one reason only, [`FamilyPrefix`](Self::FamilyPrefix), which
/// concerns the name and not the definition: nothing else is refused there,
/// since every recomposed form can be written either way.
/// [`DefinitionNameRefused::option`] says which option an error concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionNameRefusal {
    /// The entry begins with `flm`, the prefix of every LaTeX name that this
    /// library writes itself, which no user definition may use.
    FamilyPrefix,
    /// The document declares the entry's name as both a macro and an
    /// environment. LaTeX keeps both kinds in one name space:
    /// `\DeclareDocumentEnvironment{<name>}` defines the command `\<name>`
    /// itself, so the environment's definition would replace the macro's.
    MacroAndEnvironment,
    /// The entry is a macro named `end<name>`, and the document also declares
    /// an environment named `<name>` that is written under its own name too.
    /// `\DeclareDocumentEnvironment{<name>}` defines the command
    /// `\end<name>` itself, so the environment's definition would replace
    /// the macro's.
    EndOfEnvironment,
    /// The document declares the entry's name twice, once for text mode and
    /// once for mathematics mode (two per-mode definitions). Each is written
    /// under a name of its own, so one name cannot hold both.
    ModeTwins,
}

impl DefinitionNameRefusal {
    /// The stable token that identifies the reason in diagnostic data:
    /// `family-prefix`, `macro-and-environment`, `end-of-environment` or
    /// `mode-twins`. A test or a tool matches on it; the prose is in the
    /// message only.
    pub fn kind(&self) -> &'static str {
        match self {
            DefinitionNameRefusal::FamilyPrefix => "family-prefix",
            DefinitionNameRefusal::MacroAndEnvironment => "macro-and-environment",
            DefinitionNameRefusal::EndOfEnvironment => "end-of-environment",
            DefinitionNameRefusal::ModeTwins => "mode-twins",
        }
    }
}

/// Diagnostic data: the [`kind`](DefinitionNameRefusal::kind) token, never the
/// message's prose, so that a test that checks the data does not depend on
/// the wording.
impl ToDiagnosticValue for DefinitionNameRefusal {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        self.kind().to_diagnostic_value()
    }
}

/// Condition `flm.latex-recomposed.definition-name-refused`, an Error: an
/// entry of a render option that lists user definitions,
/// [`bare_names`](super::LatexRecomposedRenderOptions::bare_names) or
/// [`robust_names`](super::LatexRecomposedRenderOptions::robust_names),
/// cannot be honored, for the reason that [`reason`](Self::reason) gives.
///
/// The regions stage reports it once per entry per document. Recovery depends
/// on the reason. An entry that begins with `flm` is ignored, and the
/// definition is recomposed as it would have been without the entry: under its
/// prefix for `bare_names`, as an expandable macro for `robust_names`. An entry
/// of `bare_names` under which two of the document's own definitions would
/// become one LaTeX command is found only once the whole document is
/// recomposed, so the definitions are written as the option asked, and LaTeX
/// keeps only the later one. An entry of `bare_names` for a name with two
/// per-mode definitions is ignored, and both definitions are written under
/// their prefixed names.
///
/// One condition reports an entry of either option, so that the rule about
/// reserved names is stated once; [`option`](Self::option) says which option
/// the entry belongs to, and the message gives the option's name.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.latex-recomposed.definition-name-refused")]
pub struct DefinitionNameRefused {
    /// The option that lists the refused entry.
    pub option: DefinitionNameOption,
    /// The refused entry, as written in the option.
    pub name: String,
    /// The reason the entry is refused.
    pub reason: DefinitionNameRefusal,
}

/// The message gives the option and the entry, states the reason, and, where
/// two definitions would become one command, how to resolve it. The wording
/// differs by reason and uses the entry's name, so it is written here rather
/// than as one format string.
impl std::fmt::Display for DefinitionNameRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.name;
        let option = self.option.as_str();
        write!(f, "the render option `{option}` lists `{name}`, but ")?;
        match self.reason {
            DefinitionNameRefusal::FamilyPrefix => write!(
                f,
                "a name listed there must not begin with `flm`, the prefix reserved for the \
                 LaTeX names that this library writes itself"
            ),
            DefinitionNameRefusal::MacroAndEnvironment => write!(
                f,
                "the document declares `{name}` both as a macro and as an environment, and \
                 `\\DeclareDocumentEnvironment{{{name}}}` defines `\\{name}` itself, so the \
                 two cannot both be written under the name `{name}`; remove the entry, or \
                 rename one of the two definitions"
            ),
            DefinitionNameRefusal::EndOfEnvironment => {
                let environment = name.strip_prefix("end").unwrap_or(name);
                write!(
                    f,
                    "the document declares the macro `{name}` and the environment \
                     `{environment}`, both listed here, and \
                     `\\DeclareDocumentEnvironment{{{environment}}}` defines `\\{name}` \
                     itself, so the two cannot both be written under their own names; remove \
                     the entry, or rename one of the two definitions"
                )
            }
            DefinitionNameRefusal::ModeTwins => write!(
                f,
                "the document declares `{name}` twice, once for text mode and once for \
                 mathematics mode, and the two definitions need two different LaTeX names, so \
                 they cannot be written under the one name `{name}`"
            ),
        }
    }
}

/// Returns the entries of the render option
/// [`bare_names`](super::LatexRecomposedRenderOptions::bare_names) that the
/// document's own definitions make impossible, each with its reason, in the
/// order of the entry's name.
///
/// A definition is written under the document's own name exactly when the
/// name the style file defines is the name the document declared
/// ([`RecomposedDefinition`]), so `definitions` says which entries this
/// document actually used. A macro `<name>` written under its own name defines
/// the LaTeX command `\<name>`; an environment `<name>` written under its own
/// name defines `\<name>` and `\end<name>` as well as the environment itself.
/// Two definitions whose commands coincide would be one command in the style
/// file, the later definition replacing the earlier, so the entry that brought
/// them together is refused.
///
/// The third refusal concerns definitions that are never written under the
/// document's own name, which is why `bare_names` itself is an argument: a
/// name that the document declares twice, once per parsing mode, is written
/// under two names of its own (two per-mode definitions) regardless of the
/// option, because one name cannot hold both. A per-mode definition is
/// recognized by the letter after `flmU` in the name that the style file
/// defines ([`names::mode_of_twin_name`]), so the entry is refused as soon as
/// one of the two definitions reaches the style file, without waiting for the
/// document to use both.
fn definition_name_clashes(
    definitions: &[RecomposedDefinition],
    bare_names: &BTreeSet<String>,
) -> Vec<(String, DefinitionNameRefusal)> {
    let mut macros: BTreeSet<&str> = BTreeSet::new();
    let mut environments: BTreeSet<&str> = BTreeSet::new();
    for definition in definitions {
        // Only a definition written bare can meet another under one name: a
        // feature's definition is guarded, as a prefixed one is.
        if definition.feature.is_some() || definition.name != definition.source_name {
            continue;
        }
        match definition.name.strip_prefix('\\') {
            Some(name) => macros.insert(name),
            None => environments.insert(definition.name.as_str()),
        };
    }
    let mut clashes: BTreeMap<&str, DefinitionNameRefusal> = BTreeMap::new();
    for name in &macros {
        if environments.contains(name) {
            clashes.insert(name, DefinitionNameRefusal::MacroAndEnvironment);
        } else if name.strip_prefix("end").is_some_and(|rest| environments.contains(rest)) {
            clashes.insert(name, DefinitionNameRefusal::EndOfEnvironment);
        }
    }
    // The third refusal: a listed name the style file writes under a twin
    // prefix. A twin is never written bare, so it never reaches the two scans
    // above; it is recognized by the letter after `flmU`, which classifies
    // every name in that space.
    for definition in definitions {
        let declared = definition.source_name.strip_prefix('\\').unwrap_or(&definition.source_name);
        let written = definition.name.strip_prefix('\\').unwrap_or(&definition.name);
        if bare_names.contains(declared) && names::mode_of_twin_name(written).is_some() {
            clashes.entry(declared).or_insert(DefinitionNameRefusal::ModeTwins);
        }
    }
    clashes.into_iter().map(|(name, reason)| (name.to_string(), reason)).collect()
}

/// Reports [`StageFailed`] for the stage `stage`, with `detail` as the
/// explanation and no position in the document.
fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: String::from(stage), detail: detail.to_string() },
        nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
    ));
}

// --- the emit stage -------------------------------------------------------------------------

/// The emit stage, `flm.latex-recomposed.emit` ([`EMIT_STAGE`]): it runs once
/// per document tree, writes the tree's recomposed LaTeX ([`emit_tree`]), and
/// writes the three tables [`OUTPUT_TABLE`], [`PACKAGES_TABLE`] and
/// [`PREAMBLE_TABLE`] (see the module documentation).
///
/// It reads the lowered-trees table `flm.lowered.latex-recomposed` and the
/// render options ([`LatexRecomposedRenderOptions`]). To the requirements that
/// the emitter recorded it adds those of the tree's lowering rules, then
/// splits them into definition pieces and the other identifiers. When the
/// emitter returns no LaTeX for the tree, the stage writes nothing for that
/// tree, and the emitter's diagnostics say why.
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRecomposedEmitStage;

impl Stage for LatexRecomposedEmitStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(EMIT_STAGE, Effect::Compute, Scope::PerTree)
            .writes(OUTPUT_TABLE)
            .writes(PACKAGES_TABLE)
            .writes(PREAMBLE_TABLE)
            .docs(
                "writes the recomposed backend's lowered tree as LaTeX: the recomposed LaTeX \
                 per tree, the package identifiers, the definition pieces and the recomposed \
                 user definitions",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(lowered_table(&BackendName::from(BACKEND)))
            .option::<LatexRecomposedRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let Some(id) = cx.tree_id() else { return };
        let trees = match cx.get(lowered_table(&BackendName::from(BACKEND))) {
            Ok(Some(trees)) => trees,
            Ok(None) => return,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let Some(lowered) = trees.get(id) else { return };
        let options = match cx.option::<LatexRecomposedRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let prefix = cx.registry().lang().parsing().comment_start();
        let emitted = emit_tree(&lowered.tree, options, prefix);
        for diagnostic in emitted.diags.iter() {
            out.diags.push(diagnostic.clone());
        }
        let Some(mut fragment) = emitted.fragment else { return };
        fragment.requires.extend(lowered.requires.iter().cloned());
        let (pieces, others) = preamble::split_requirements(&fragment.requires);
        let definitions = fragment.definitions.clone();
        let write = (|| -> Result<(), crate::core::data::store::AccessError> {
            out.table(OUTPUT_TABLE)?.trees.insert(id, fragment);
            out.table(PACKAGES_TABLE)?.ids.extend(others);
            let preamble = out.table(PREAMBLE_TABLE)?;
            preamble.pieces.extend(pieces);
            for definition in definitions {
                preamble.add_definition(definition);
            }
            Ok(())
        })();
        if let Err(error) = write {
            stage_failed(out, EMIT_STAGE, error);
        }
    }
}

// --- the regions stage ----------------------------------------------------------------------

/// The regions stage, `flm.latex-recomposed.regions` ([`REGIONS_STAGE`]): it
/// runs once per document, writes as LaTeX the pieces that the render places,
/// resolves every requirement of the trees and of the pieces, assembles the
/// style file into the style-file table [`STYLE_FILE`], and writes the output
/// [regions](crate::core::region) into [`REGIONS`]: the body, each piece, and
/// the style file's package name (see the module documentation).
///
/// It also checks the entries of the render options `bare_names` and
/// `robust_names` and reports [`DefinitionNameRefused`]; it reports
/// [`UnknownRequirement`] and [`ConflictingPackageOptions`] while assembling
/// the style file, and [`StylePathRefused`] when the naming service places
/// the style file where LaTeX cannot load it (see [the style file's
/// path](self#the-style-files-path)).
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRecomposedRegionsStage;

impl Stage for LatexRecomposedRegionsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(REGIONS_STAGE, Effect::Compute, Scope::Document)
            .writes(REGIONS)
            .writes(STYLE_FILE)
            .docs(
                "emits the pieces, assembles the style file from the requirements into \
                 `flm.output.latex-recomposed.style`, and writes the body, the pieces and \
                 the style file's name as the regions `flm.output.latex-recomposed.regions`",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(OUTPUT_TABLE)
            .reads(PACKAGES_TABLE)
            .reads(PREAMBLE_TABLE)
            .reads(lowered_pieces_table(&BackendName::from(BACKEND)))
            .option::<LatexRecomposedRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        // The one place the render options are checked for content rather
        // than for type: an entry inside the family's own name space is
        // refused here, once per document, whatever the document holds. One
        // rule about a name, over both options that name user definitions.
        // See decision rationale at [§flm-dd-dr:rend-latex-reserved-names]
        // and [§flm-dd-dr:rend-latex-userdef-robust-option].
        if let Ok(options) = cx.option::<LatexRecomposedRenderOptions>() {
            let listed = [
                (DefinitionNameOption::BareNames, &options.bare_names),
                (DefinitionNameOption::RobustNames, &options.robust_names),
            ];
            for (option, entries) in listed {
                for name in entries.iter().filter(|name| names::is_family_name(name)) {
                    out.diags.push(Diagnostic::error(
                        DefinitionNameRefused {
                            option,
                            name: name.clone(),
                            reason: DefinitionNameRefusal::FamilyPrefix,
                        },
                        nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
                    ));
                }
            }
        }
        let fragments = match cx.get(OUTPUT_TABLE) {
            Ok(Some(fragments)) => fragments,
            Ok(None) => return,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let package_ids = match cx.get(PACKAGES_TABLE) {
            Ok(Some(table)) => table.ids.clone(),
            Ok(None) => BTreeSet::new(),
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let mut recorded = match cx.get(PREAMBLE_TABLE) {
            Ok(Some(table)) => table.clone(),
            Ok(None) => LatexPreamble::default(),
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let options = match cx.option::<LatexRecomposedRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let services = cx.services_all();

        // The pieces the render places: each emitted as a tree is, and what
        // each needs joins what the trees need, so that the style file holds
        // it. A plan in which the pieces stage wrote nothing places none.
        let mut package_ids = package_ids;
        let mut declared = Declared::new();
        let mut piece_regions = Vec::new();
        let lowered_pieces = match cx.get(lowered_pieces_table(&BackendName::from(BACKEND))) {
            Ok(lowered) => lowered,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let prefix = cx.registry().lang().parsing().comment_start();
        for piece in lowered_pieces.iter().flat_map(|lowered| lowered.pieces.iter()) {
            let emitted = emit_tree(&piece.tree.tree, options, prefix);
            for diagnostic in emitted.diags.iter() {
                out.diags.push(diagnostic.clone());
            }
            let Some(mut fragment) = emitted.fragment else { continue };
            fragment.requires.extend(piece.tree.requires.iter().cloned());
            let (pieces, others) = preamble::split_requirements(&fragment.requires);
            recorded.pieces.extend(pieces);
            package_ids.extend(others);
            for definition in fragment.definitions {
                recorded.add_definition(definition);
            }
            declared.merge(&fragment.declared);
            let words = crate::core::region::piece_words(&piece.tree.tree);
            piece_regions.push(Region::piece(
                piece.name.clone(),
                piece.place,
                vec![fragment.text],
                words,
            ));
        }

        let LatexPreamble { pieces: piece_ids, definitions } = recorded;

        // The second content check of the render options, and the one that
        // needs the document: an entry the document's own definitions make
        // impossible — two of them that LaTeX would keep as one command, or one
        // name declared for both parsing modes. It is answered here, over the
        // definitions the style file will hold, because only the whole document
        // says which names met.
        for (name, reason) in definition_name_clashes(&definitions, &options.bare_names) {
            out.diags.push(Diagnostic::error(
                DefinitionNameRefused { option: DefinitionNameOption::BareNames, name, reason },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
        }

        // The trees in document order: the recomposed LaTeX, and the fold of what
        // they declared. A tree placed in another is written where it is
        // placed, and not again in its own right.
        let placed: BTreeSet<TreeId> =
            fragments.trees.values().flat_map(|fragment| fragment.placed.iter().map(|(_, tree)| *tree)).collect();
        let mut body = String::new();
        for (id, _) in cx.trees() {
            if placed.contains(&id) {
                continue;
            }
            write_tree(fragments, id, &mut body, &mut declared, &mut BTreeSet::new());
            body.push_str(TREE_END);
        }

        // The pieces, closed over the pieces they need in turn and taken in
        // the order the registry holds them, the piece of the feature placed
        // last defining an identifier several pieces have (see decision
        // rationale at [§flm-dd-dr:feat-order]); an identifier resolving to
        // none is reported. The declared characters' own pieces join the
        // trees': a spelling that needs declarations no package makes — a
        // math alphabet, a symbol font — carries them as a piece of its own.
        let mut wanted = piece_ids;
        wanted.extend(declared.pieces());
        let defining = preamble::definitions(services);
        let mut added = true;
        while added {
            added = false;
            for piece in &defining {
                if !wanted.contains(&piece.id) {
                    continue;
                }
                for id in preamble::split_requirements(&piece.requires).0 {
                    if wanted.insert(id) {
                        added = true;
                    }
                }
            }
        }
        let pieces: Vec<&LatexPreambleDef> =
            defining.into_iter().filter(|piece| wanted.contains(&piece.id)).collect();
        for id in &wanted {
            if !pieces.iter().any(|piece| piece.id == *id) {
                out.diags.push(Diagnostic::error(
                    UnknownRequirement { id: id.to_string() },
                    nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
                ));
            }
        }

        // The packages: the trees' identifiers, the pieces' needs, the
        // declared characters' needs; in identifier order; skipped by
        // package name.
        let mut all_package_ids = package_ids;
        for piece in &pieces {
            all_package_ids.extend(preamble::split_requirements(&piece.requires).1);
        }
        all_package_ids.extend(declared.packages());
        let mut package_defs: Vec<&LatexPackageDef> = Vec::new();
        for id in &all_package_ids {
            // One definition for a request that is the same under every
            // engine, one per set of engines for a request that differs.
            let defs: Vec<&LatexPackageDef> =
                packages::packages_named(services, id.as_str()).collect();
            if defs.is_empty() {
                out.diags.push(Diagnostic::error(
                    UnknownRequirement { id: id.to_string() },
                    nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
                ));
                continue;
            }
            package_defs.extend(
                defs.into_iter()
                    .filter(|def| !options.skip_packages.contains(def.package.as_ref())),
            );
        }
        // Then the author's own requests, after the document's and in the
        // order given. They resolve through no slot — nothing names them by
        // identifier. A request for a package the skip list also names is not
        // dropped: an explicit request wins. Such a request goes into a group
        // of its own, so that the skip list keeps this library's own requests
        // for that package out of the file and the author's options stand
        // alone — which is how an author states an option list this library's
        // own requests cannot add to.
        // See decision rationale at [§flm-dd-dr:rend-latex-author-preamble].
        let mut author_skipped: Vec<&LatexPackageDef> = Vec::new();
        for def in &options.extra_packages {
            if options.skip_packages.contains(def.package.as_ref()) {
                author_skipped.push(def);
            } else {
                package_defs.push(def);
            }
        }

        // One `\RequirePackage` line per package under each engine, its
        // options the union of every request's in the order first seen; two
        // requests naming one option key with different values cannot be
        // merged and the package is then written not at all under the engines
        // they meet under. The two groups are merged by the same
        // rule and written one after the other; a package is in one group or
        // the other and never in both, so the file holds one line per package.
        // See decision rationale at [§flm-dd-dr:rend-latex-package-block].
        let (mut package_requests, conflicts) = packages::merge_requests(package_defs);
        let (skipped_requests, skipped_conflicts) = packages::merge_requests(author_skipped);
        for conflict in conflicts.into_iter().chain(skipped_conflicts) {
            out.diags.push(Diagnostic::error(
                ConflictingPackageOptions {
                    package: conflict.package,
                    key: conflict.key,
                    first: conflict.first,
                    second: conflict.second,
                },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
        }
        package_requests.extend(skipped_requests);

        // Whether there is a style file at all: under `WhenNeeded` a
        // document that needs nothing in it gets none, and its recomposed LaTeX
        // carries no comment line naming one.
        // An author who set either of the two options asked for a file: the
        // requests and the block are written nowhere else, so the file exists
        // even where every request was skipped and the rest is empty.
        let needed = !package_requests.is_empty()
            || !declared.chars().is_empty()
            || !pieces.is_empty()
            || !definitions.is_empty()
            || !options.extra_packages.is_empty()
            || !options.extra_preamble.is_empty();
        let write_style = needed || options.style_file == StyleFile::Always;

        let mut regions = Regions::new(
            ArtifactId::new(ARTIFACT_OWNER, DOCUMENT_ARTIFACT),
            MEDIA_TYPE,
        );
        regions.regions.push(Region::new(CONTENT, vec![body]));
        regions.regions.extend(piece_regions);
        let mut beside = ArtifactSet::new();
        if write_style {
            let style_body = style_body(
                &package_requests,
                &declared,
                &pieces,
                &definitions,
                &options.extra_preamble,
            );
            let default = format!("{}.{}", options.style_stem, content_hash(&style_body));
            let style_id = ArtifactId::new(ARTIFACT_OWNER, default.clone());
            let path = style_path(cx, &style_id, &default, out);
            let name = path.strip_suffix(".sty").unwrap_or(&path).to_string();
            let style = with_header(&name, &style_body);
            let mut style_artifact =
                Artifact::bytes(style_id.clone(), MEDIA_TYPE, style.into_bytes())
                    .with_extension(STYLE_EXTENSION);
            style_artifact.fixed_path = Some(RelPath::new(path));
            if let Err(violation) = beside.insert(style_artifact) {
                return stage_failed(out, REGIONS_STAGE, violation);
            }
            regions.refs.push(style_id);
            regions.regions.push(Region::new(STYLE_PACKAGE, vec![name]));
        }
        let write = (|| -> Result<(), crate::core::data::store::AccessError> {
            *out.table(REGIONS)? = regions;
            *out.table(STYLE_FILE)? = beside;
            Ok(())
        })();
        if let Err(error) = write {
            stage_failed(out, REGIONS_STAGE, error);
        }
    }
}

// --- the final stage ------------------------------------------------------------------------

/// The final stage, `flm.latex-recomposed.final` ([`FINAL_STAGE`]): the
/// backend's final stage, which assembles the entry artifact from the output
/// regions in a layout of the backend's own (see [output regions and the
/// final stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
///
/// It writes the body, preceded by the comment line
/// `% flm: style file <name>.sty` when the output has a style file, as the
/// entry artifact of the set `flm.output.latex-recomposed.document`
/// ([`DOCUMENT`]), followed by the style file ([`STYLE_FILE`]) and every
/// artifact of the artifact table `flm.output.latex-recomposed.artifacts`
/// ([`ARTIFACTS`]) when a stage of the plan wrote one. It places no piece:
/// the pieces of the recomposed LaTeX are placed only by a host's template,
/// which knows where in a LaTeX document each belongs.
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRecomposedFinalStage;

impl Stage for LatexRecomposedFinalStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(FINAL_STAGE, Effect::Compute, Scope::Document).writes(DOCUMENT).docs(
            "writes the body and the style file as the artifact set \
             `flm.output.latex-recomposed.document`",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(REGIONS).reads(STYLE_FILE).reads_opt(ARTIFACTS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let regions = match cx.get(REGIONS) {
            Ok(Some(regions)) => regions,
            Ok(None) => return,
            Err(error) => return stage_failed(out, FINAL_STAGE, error),
        };
        let mut beside = Vec::new();
        for table in [STYLE_FILE, ARTIFACTS] {
            match cx.get(table) {
                Ok(set) => beside.extend(set),
                Err(error) => return stage_failed(out, FINAL_STAGE, error),
            }
        }
        let body = regions.get(CONTENT).map(|region| region.text()).unwrap_or_default();
        let document = match regions.get(STYLE_PACKAGE) {
            Some(name) => format!("% flm: style file {}.{STYLE_EXTENSION}\n{body}", name.text()),
            None => body,
        };
        let mut entry =
            Artifact::bytes(regions.entry.clone(), regions.media_type.clone(), document.into_bytes())
                .as_entry();
        entry.refs.extend(regions.refs.iter().cloned());
        match out.table(DOCUMENT) {
            Ok(set) => match set.insert_entry_with(entry, &beside) {
                Ok(reports) => reports.into_iter().for_each(|report| out.diags.push(report)),
                Err(violation) => stage_failed(out, FINAL_STAGE, violation),
            },
            Err(error) => stage_failed(out, FINAL_STAGE, error),
        }
    }
}

/// Returns the backend's three stages as shared values, in the order that a
/// backend declaration ([`BackendDef::new`](crate::core::registry::BackendDef::new))
/// takes them: the lowering stage ([`LoweringStage`] for `latex-recomposed`),
/// the emit stage ([`LatexRecomposedEmitStage`]) and the final stage
/// ([`LatexRecomposedFinalStage`]). The [`LatexRecomposed`](super::LatexRecomposed)
/// feature registers the pieces stage and the regions stage
/// ([`LatexRecomposedRegionsStage`]) as separate stages.
pub fn stages() -> (Arc<dyn Stage>, Arc<dyn Stage>, Arc<dyn Stage>) {
    (
        Arc::new(LoweringStage::new(BACKEND)),
        Arc::new(LatexRecomposedEmitStage),
        Arc::new(LatexRecomposedFinalStage),
    )
}

/// Writes the recomposed LaTeX of the tree `id` into `body`, with each tree
/// placed in it written at its position, and adds what those trees declared to
/// `declared`. `entered` holds the trees being written, so that a tree placed
/// inside itself is written only once.
fn write_tree(
    fragments: &LatexFragments,
    id: TreeId,
    body: &mut String,
    declared: &mut Declared,
    entered: &mut BTreeSet<TreeId>,
) {
    let Some(fragment) = fragments.trees.get(&id) else { return };
    if !entered.insert(id) {
        return;
    }
    let mut written = 0;
    for (at, tree) in &fragment.placed {
        body.push_str(&fragment.text[written..*at]);
        write_tree(fragments, *tree, body, declared, entered);
        written = *at;
    }
    body.push_str(&fragment.text[written..]);
    declared.merge(&fragment.declared);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stages_are_named_and_scoped_as_the_design_says() {
        let (lowering, emit, final_stage) = stages();
        assert_eq!(lowering.decl().name, "flm.latex-recomposed.lower");
        assert_eq!(emit.decl().name, EMIT_STAGE);
        assert_eq!(emit.decl().scope, Scope::PerTree);
        assert_eq!(final_stage.decl().name, FINAL_STAGE);
        assert_eq!(final_stage.decl().scope, Scope::Document);
        assert_eq!(OUTPUT_TABLE.name(), "flm.output.latex-recomposed");
        assert_eq!(DOCUMENT.name(), "flm.output.latex-recomposed.document");
    }

    #[test]
    fn the_preamble_table_keeps_the_first_definition_of_a_name() {
        let mut table = LatexPreamble::default();
        table.absorb(LatexPreamble {
            pieces: BTreeSet::new(),
            definitions: vec![RecomposedDefinition::new("\\a", "\\a", "one")],
        });
        table.absorb(LatexPreamble {
            pieces: [Name::from(preamble::VERBATIM)].into_iter().collect(),
            definitions: vec![
                RecomposedDefinition::new("\\a", "\\a", "two"),
                RecomposedDefinition::new("\\b", "\\b", "three"),
            ],
        });
        assert_eq!(table.definitions.len(), 2);
        assert_eq!(table.definitions[0].text, "one");
        assert_eq!(table.pieces.len(), 1);
        assert_eq!(table.piece_class(), PieceClass::DocumentFact);
        assert_eq!(LatexPreamble::default().piece_class(), PieceClass::Empty);
    }
}
