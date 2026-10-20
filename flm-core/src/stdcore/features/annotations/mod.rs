//! The `annotations` feature: a reviewer's marks in a document, passages
//! highlighted and remarks written into the text, under commands that a host
//! configures, one per reviewer.
//!
//! # What an author writes
//!
//! An **annotator** is one reviewer, configured with a command name and
//! optional initials ([`Annotator`], [`AnnotationsConfig`]). The annotators
//! are numbered from zero in configuration order, and an output can give
//! each number its own color. The default configuration declares no
//! annotator, and the feature then declares no command: `\phf` is an unknown
//! command until a host configures an annotator whose command is `phf`.
//!
//! Each annotator's command writes two kinds of annotation:
//!
//! - a **highlight**, a passage the annotator marks, which stays part of the
//!   text: written `\phf{…}`, or `\phf … \endphf`;
//! - a **comment**, a remark the annotator writes into the text, which is not
//!   part of it: written `\phf[…]`. (A comment in this sense is a reviewer's
//!   remark, not a source comment written with `%`.)
//!
//! ```text
//! The result \phf{holds in every case}, as \phf[cite the lemma here] shown.
//! \phf The second form needs no braces, which helps
//! with a long passage.\endphf
//! ```
//!
//! The command takes **one argument**, read by the feature's own parser
//! ([`AnnotationParser`]), which chooses the form by what follows the command
//! (past any whitespace and source comments): a `{` begins a braced group, a
//! `[` a bracketed group, and anything else begins a passage that ends at
//! `\endphf` (`end` followed by the command's name). The argument holds
//! inline content or paragraphs, as written. `\phf{x}[y]` is a highlight of
//! `x` followed by the text `[y]`.
//! The function [`form_of`] reads which form an annotation was written in.
//! The guide for authors describes annotations under
//! [Annotations](crate::guide::language_syntax#annotations).
//!
//! An annotation never becomes a block that holds paragraphs. A blank line
//! inside it is a paragraph break of the text around it, and the annotation
//! continues in the next paragraph; a list or a displayed formula inside it
//! is a block of the text around it, still part of the annotation (a cut part
//! around that block, see below). The annotator's initials are shown at the
//! start of the annotation only.
//!
//! # What the feature provides to the pipeline
//!
//! **The element.** Every annotator's command is one element value of the
//! element type [`ANNOTATION`] (`flm.annotations.annotation`), carrying an
//! [`AnnotationElement`]: a macro with one argument, [`TEXT_ARG`], declared in
//! text mode.
//!
//! **Cut parts.** The element is *block-transparent*: its content is laid
//! out as if it were written directly in the content around it
//! ([block-transparent elements](crate::guide::concepts_overview#block-transparent-elements)).
//! Before any paragraph is built, the block-structure pass
//! ([`blocks`](crate::lang::rewrites::blocks)) cuts an
//! annotation whose argument crosses a paragraph break or holds a block into
//! **cut parts**: one node of the annotation's element around each stretch of
//! inline content between paragraph breaks and blocks, and one around each
//! block. Each cut part carries an [`AnnotationPiece`] record, its
//! continuation record, which says which cut part of how many it is. The
//! lowering rules read it to show the initials on the first cut part only,
//! and the `text` backend's rule reads it to open a comment's bracket on the
//! first cut part and close it on the last.
//!
//! **Lowering rules.** A lowering rule ([`LoweringRule`]) says how an element
//! becomes output on a backend. The feature registers these rules. A rule
//! marked as the default for one backend ranks above a rule marked as the
//! default everywhere ([`DefaultMark`]):
//!
//! - [`RULE`] ([`AnnotationRule`]), the default on every backend that has no
//!   rule of its own for an annotation: an
//!   annotation, or a cut part of one, lowers to a `flm.pres.span` of the
//!   role [`HIGHLIGHT_ROLE`] or [`COMMENT_ROLE`] (a `flm.pres.block` of that
//!   role for a cut part around a block placed among blocks), with the kind
//!   tokens `<command>` and `annotator-<n>` ([`annotator_kind`]). A role is a
//!   feature-owned dotted name on a presentation payload that says what a
//!   container is, and kind tokens classify it further
//!   ([`payload`](crate::stdcore::backends::pres::payload)). A cut part also
//!   carries the kind token [`MARK_CONTINUES_KIND`] when a cut part follows
//!   it and [`MARK_CONTINUED_KIND`] when one precedes it. The annotator's
//!   initials are a margin decoration at the start of the annotation, holding
//!   a span of the role [`INITIALS_ROLE`]; a decoration is a node that the
//!   lowering driver moves out of the content into an attached slot of the
//!   enclosing presentation node
//!   ([`Decoration`](crate::stdcore::backends::pres::Decoration)).
//! - [`TEXT_RULE`] ([`AnnotationTextRule`]), the default on the `text`
//!   backend: it lowers an annotation as the general rule does, except that a
//!   comment is written with its brackets and initials as text and no
//!   decoration: `[PhF: remark]` for an annotator with the initials `PhF`.
//!   The text backend prints a highlight as `[PhF] passage`.
//! - On the `html` backend, the feature's stylesheet `flm.annotations.style`
//!   ([`html`]), which colors each annotation by its annotator's number; the
//!   general rule and the `text` backend's rule require it wherever they
//!   write an annotation.
//! - On the `latex-rendered` backend, the definition piece
//!   `flm.latex.def.flmx.annotations` ([`latex_rendered`]), which the same
//!   requirement brings into the style file.
//! - [`latex::LATEX_RULE`] ([`latex::AnnotationLatexRule`]), the default on
//!   the `latex-recomposed` backend: it writes the command back as the author
//!   wrote it, over the LaTeX package `phfcc`, which makes each annotator's
//!   command ([`latex`]).
//!
//! **Settings.** The feature's configuration, [`AnnotationsConfig`], is the
//! value the feature is constructed from; it is fixed when the registry is
//! built
//! ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
//! because the commands are part of the language. Written as data, its
//! settings key `annotators` maps each command name to the annotator's other
//! fields, and a host that combines several sources of settings combines
//! that map one annotator at a time. The feature feeds the configuration
//! into the [registry digest](crate::model::digest), the number that changes
//! whenever something about the registry that can change what processing
//! produces changes ([`FeatureBuilder::digest_input`]);
//! [`AnnotationsConfig::digest_bytes`] gives the bytes. The feature also
//! declares one option type, [`AnnotationsRenderOptions`], set per render
//! request: when its field [`hidden`](AnnotationsRenderOptions::hidden) is
//! true, every rule leaves every
//! annotation out of the output, for a clean copy: a highlight is shown as
//! its passage alone and a comment as nothing (the recomposed LaTeX keeps the
//! paragraph breaks a comment held).
//!
//! # Errors
//!
//! A passage opened by `\phf` and not closed by `\endphf` is reported as
//! [`Unterminated`] (`flm.annotations.unterminated`) while the document is
//! parsed. An unknown command (a stray `\endphf`, or `\phf` with no
//! annotator configured) and an unclosed group are reported by the language
//! itself.
//!
//! # Interactions with other features
//!
//! - **Block structure.** The element carries the block-transparent aspect
//!   ([`BlockTransparent`]) and a block role per occurrence
//!   ([`BlockRoleAspect`]); the block-structure pass
//!   ([`blocks`](crate::lang::rewrites::blocks)) cuts it into cut parts.
//! - **Text of a title.** The element carries the aspect [`ApartFromText`]:
//!   a comment's remark is not part of the text around it, so that reading
//!   the plain text of a run (for example to give an unlabeled heading an
//!   identifier from its title) leaves the remark out. A highlight's passage
//!   is part of the text.
//! - **Decorations.** The initials are a margin decoration
//!   (`flm.pres.deco.margin`); each backend writes it as it writes every
//!   margin decoration.
//! - **The `latex-recomposed` backend** lowers the tree before the
//!   block-structure pass, so an annotation reaches its rule uncut and is
//!   written back as one invocation ([`latex`]).
//! - **The standard package.** The [standard package](crate::stdcore)
//!   lists the feature under the configuration
//!   [`StdcoreConfig::annotations`](crate::stdcore::StdcoreConfig::annotations).
//!
//! # Items
//!
//! - [`AnnotationsFeature`] and [`feature`] — the feature, under a
//!   configuration or the default one.
//! - [`AnnotationsConfig`], [`Annotator`] — the configuration;
//!   [`AnnotationsRenderOptions`] — the render option.
//! - [`AnnotationElement`] — the data of each element value of
//!   [`ANNOTATION`].
//! - [`AnnotationParser`], [`Form`], [`form_of`] — the argument's parser and
//!   the reading of an annotation's form.
//! - [`AnnotationPiece`], [`payload_readers`] — the continuation record of a
//!   cut part, and its reader for serialized trees.
//! - [`AnnotationRule`], [`AnnotationTextRule`] — the general rule and the
//!   `text` backend's rule; [`html`] and [`latex`] — the rules of the `html`
//!   and `latex-recomposed` backends.
//! - [`Unterminated`] — the one condition the feature reports.
//! - The names: [`FEATURE`], [`ANNOTATION`], [`TEXT_ARG`], [`RULE`],
//!   [`TEXT_RULE`], the roles [`HIGHLIGHT_ROLE`], [`COMMENT_ROLE`],
//!   [`INITIALS_ROLE`], and the kind tokens [`annotator_kind`],
//!   [`MARK_CONTINUES_KIND`], [`MARK_CONTINUED_KIND`].

mod config;
mod cut;
mod diagnostics;
pub mod html;
pub mod latex;
pub mod latex_rendered;
mod lower;
mod parser;

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::settings::SettingsDecl;
use crate::core::lowering::{DefaultMark, LoweringRule};
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::lang::Flm;
use crate::lang::mint::{ArgDef, ArgLayout, ElementDef, SyntaxDef};
use crate::lang::rewrites::blocks::BlockTransparent;
use crate::model::aspects::{ApartFromText, BlockRoleAspect, ContentRef};
use crate::model::element::ElementSpec;
use crate::model::names::{FeatureName, Name};
use crate::model::options::Bind;
use crate::stdcore::backends::text;

pub use config::{AnnotationsConfig, AnnotationsRenderOptions, Annotator};
pub use cut::{payload_readers, AnnotationPiece};
pub use diagnostics::Unterminated;
pub use lower::{AnnotationRule, AnnotationTextRule};
pub use parser::{form_of, AnnotationParser, Form};

/// The feature's registered name, `annotations`.
pub const FEATURE: &str = "annotations";

/// The declaration of the feature's settings ([`crate::core::settings`]): the
/// fields of [`AnnotationsConfig`] and of [`AnnotationsRenderOptions`] are its
/// settings keys, and the key `annotators` is combined one annotator at a
/// time.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<AnnotationsConfig>(
            "The annotators, in order, the first numbered 0: for each, the name of its command \
            (without the backslash) and the initials shown at the start of its annotations. \
            Fixed when the registry is built: it cannot vary per document or per render.",
        )
        .options::<AnnotationsRenderOptions>(
            Bind::Render,
            "Whether every annotation is left out of the output, each highlight shown as its \
            passage alone and each comment not at all (`hidden`, false by default). Set per \
            render request.",
        )
        .merge_entries("annotators", || crate::core::settings::named::to_data(&AnnotationsConfig::default().annotators, "command"))
}

/// The element type of an annotation, `flm.annotations.annotation`: one
/// element value per configured annotator, each carrying an
/// [`AnnotationElement`].
pub const ANNOTATION: &str = "flm.annotations.annotation";

/// The name of an annotation's one argument, `text`: the highlighted passage
/// or the written remark, read by the [`AnnotationParser`].
pub const TEXT_ARG: &str = "text";

/// The role of the span or block a highlight is staged as,
/// `flm.annotations.highlight`.
pub const HIGHLIGHT_ROLE: &str = "flm.annotations.highlight";

/// The role of the span or block a comment is staged as,
/// `flm.annotations.comment`.
pub const COMMENT_ROLE: &str = "flm.annotations.comment";

/// The role of the span that holds an annotator's initials inside the margin
/// decoration at the start of a highlight or a comment,
/// `flm.annotations.initials`.
pub const INITIALS_ROLE: &str = "flm.annotations.initials";

/// The name of the general lowering rule of the element [`ANNOTATION`],
/// `flm.annotations.annotation.lower.default` ([`AnnotationRule`]), the
/// default on every backend.
pub const RULE: &str = "flm.annotations.annotation.lower.default";

/// The name of the lowering rule of the element [`ANNOTATION`] on the `text`
/// backend, `flm.annotations.annotation.lower.text` ([`AnnotationTextRule`]),
/// which writes a comment's brackets and initials as text.
pub const TEXT_RULE: &str = "flm.annotations.annotation.lower.text";

/// Returns the kind token of the annotator numbered `number`,
/// `annotator-<n>`: the second kind token of every annotation's span or block
/// and of its initials' span, after the command's name. A stylesheet selects
/// an annotator's color by this token.
pub fn annotator_kind(number: usize) -> Name {
    Name::from(format!("annotator-{number}"))
}

/// The kind token `mark-continues`, carried by every cut part of a cut
/// annotation except the last: the annotation continues in a later cut part.
/// With [`MARK_CONTINUED_KIND`], it lets a stylesheet draw what opens an
/// annotation on its first cut part only and what closes it on its last cut
/// part only. An annotation that was not cut carries neither token.
///
/// A kind token has a meaning only together with the role's class, so the
/// same token on another feature's element is a different token. The kind
/// tokens of a cut part also hold the annotator's command name, which is made
/// of letters only; because of the hyphen, no command name can equal either
/// token.
pub const MARK_CONTINUES_KIND: &str = "mark-continues";

/// The kind token `mark-continued`, carried by every cut part of a cut
/// annotation except the first: the annotation began in an earlier cut part.
/// See [`MARK_CONTINUES_KIND`].
pub const MARK_CONTINUED_KIND: &str = "mark-continued";

/// The data that each element value of [`ANNOTATION`] carries: the annotator
/// whose command writes the annotation, and the annotator's number, its
/// position in the configuration counted from zero.
///
/// The element value also carries three aspects, each implemented on this
/// type: the block role ([`BlockRoleAspect`]) and the cut into cut parts
/// ([`BlockTransparent`]), described at [`AnnotationPiece`], and the content
/// apart from the text ([`ApartFromText`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationElement {
    /// The annotator.
    pub annotator: Annotator,
    /// The annotator's number.
    pub number: usize,
}

/// A comment is not part of the text around it: its remark, the argument
/// [`TEXT_ARG`], is the annotator's. The plain text of a heading's title that
/// holds a comment therefore leaves the remark out, and so does the
/// identifier made from that text. A highlight's passage is part of the
/// text, so a highlight returns no content reference.
impl ApartFromText for AnnotationElement {
    fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        match form_of(node) {
            Some(Form::Comment) => vec![ContentRef::ArgumentNamed(TEXT_ARG.to_string())],
            Some(Form::Highlight) | None => Vec::new(),
        }
    }
}

/// Returns the element value of the annotator numbered `number`.
fn annotation_element(annotator: &Annotator, number: usize) -> Arc<ElementSpec> {
    let value = Arc::new(AnnotationElement { annotator: annotator.clone(), number });
    Arc::new(
        ElementSpec::new(ANNOTATION)
            .docs(format!(
                "`\\{0}{{…}}`, `\\{0}[…]`, `\\{0} … \\end{0}`: an annotation of annotator {1}.",
                annotator.command, number
            ))
            .with::<dyn BlockRoleAspect>(Arc::clone(&value) as Arc<dyn BlockRoleAspect>)
            .with::<dyn BlockTransparent>(Arc::clone(&value) as Arc<dyn BlockTransparent>)
            .with::<dyn ApartFromText>(Arc::clone(&value) as Arc<dyn ApartFromText>)
            .with_value(value),
    )
}

/// Returns the declaration of the command of the annotator numbered `number`:
/// a macro of one argument, [`TEXT_ARG`], read by the feature's
/// [`AnnotationParser`] and laid out as inline content or as paragraphs and
/// blocks, according to its content; declared in text mode.
fn annotation_def(owner: &FeatureName, annotator: &Annotator, number: usize) -> ElementDef {
    let command = annotator.command.as_str();
    ElementDef::new(
        owner.clone(),
        [command],
        SyntaxDef::Macro {
            args: vec![ArgDef::named(TEXT_ARG, AnnotationParser::new(command))
                .layout(ArgLayout::InlineOrBlocks)
                .docs("The passage highlighted, or the remark written.")],
        },
        annotation_element(annotator, number),
    )
    .docs(format!(
        "`\\{command}{{…}}` or `\\{command} … \\end{command}` highlights a passage, \
         `\\{command}[…]` writes a remark: annotator {number}."
    ))
}

/// The `annotations` feature ([`FlmFeature`]), constructed from an
/// [`AnnotationsConfig`]: it declares one command per annotator and the
/// lowering rules of an annotation. See the [module documentation](self).
#[derive(Debug, Clone, Default)]
pub struct AnnotationsFeature {
    config: AnnotationsConfig,
}

impl AnnotationsFeature {
    /// Returns the feature constructed from `config`, which stays fixed for
    /// the life of the registry the feature is registered into.
    ///
    /// The configuration is checked when the feature registers: an invalid
    /// one ([`AnnotationsConfig::validate`]) makes the registry build fail
    /// with a [`BuildError::Feature`].
    pub fn new(config: AnnotationsConfig) -> AnnotationsFeature {
        AnnotationsFeature { config }
    }

    /// Returns the configuration this feature was constructed from.
    pub fn config(&self) -> &AnnotationsConfig {
        &self.config
    }
}

impl FlmFeature for AnnotationsFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Annotations").docs(
            "A reviewer's marks: one command per configured annotator, written `\\phf{…}` or \
             `\\phf … \\endphf` for a highlighted passage and `\\phf[…]` for a remark, with \
             `phf` the annotator's command.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        self.config
            .validate()
            .map_err(|detail| BuildError::Feature { feature: owner.clone(), detail })?;
        for (number, annotator) in self.config.annotators.iter().enumerate() {
            b.element(annotation_def(&owner, annotator, number))?;
        }
        // The element exists only where an annotator is configured, and a
        // rule names a registered element.
        if !self.config.annotators.is_empty() {
            b.lowering_rule(
                LoweringRule::new(RULE, ANNOTATION, lower::decl(), Arc::new(AnnotationRule), DefaultMark::Everywhere)
                    .docs(
                        "an annotation lowers to a `flm.pres.span` (or, for a cut part around one \
                         block, a `flm.pres.block`) of the role `flm.annotations.highlight` or \
                         `flm.annotations.comment`, with the kind tokens `<command>` and \
                         `annotator-<n>`, holding the initials in a `flm.pres.deco.margin` on a \
                         first cut part, then the passage or the remark; when annotations are hidden \
                         (`hidden`), a highlight lowers to its passage alone and a comment to \
                         nothing",
                    ),
            )?;
            b.peer_backend(
                text::BACKEND,
                "the text backend, on which the annotation's rule \
                 `flm.annotations.annotation.lower.text` is the default",
            )?;
            b.lowering_rule(
                LoweringRule::new(
                    TEXT_RULE,
                    ANNOTATION,
                    lower::decl(),
                    Arc::new(AnnotationTextRule),
                    DefaultMark::OnBackends(vec![text::BACKEND.into()]),
                )
                .docs(
                    "an annotation lowers as under the general rule, except a comment: `[`, the \
                     initials and `: ` are written as text before the remark and `]` after it, \
                     with no decoration",
                ),
            )?;
            html::register(b)?;
            latex_rendered::register(b)?;
            latex::register(b, &self.config.annotators)?;
        }
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// Returns the [`AnnotationsFeature`] under the default configuration (no
/// annotator, so no command), ready to be listed in a package entry.
///
/// To list the feature under an [`AnnotationsConfig`] of your own, use
/// `Arc::new(AnnotationsFeature::new(config))`, or pass the configuration to
/// the standard package with
/// [`StdcoreConfig::annotations`](crate::stdcore::StdcoreConfig::annotations).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(AnnotationsFeature::default())
}
