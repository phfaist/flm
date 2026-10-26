//! The emitter of the `latex-recomposed` backend: it walks one lowered tree
//! and writes the tree's recomposed LaTeX through the typed sink
//! ([`LatexSink`]) of the LaTeX family (the
//! [`latex`](crate::stdcore::backends::latex) module shared by the LaTeX
//! backends), recording what the style file must hold because of the tree.
//!
//! An emitter is a backend's emit stage: it walks the lowered trees and writes
//! the output's bytes into a table (see [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! [`emit_tree`] is this backend's entry point; the emit stage
//! ([`LatexRecomposedEmitStage`](super::stages::LatexRecomposedEmitStage))
//! calls it once per document tree, and the regions stage
//! ([`LatexRecomposedRegionsStage`](super::stages::LatexRecomposedRegionsStage))
//! once per placed [piece](crate::core::piece) (a document-level value such
//! as the title, placed beside the content). It returns an [`Emitted`] value: the tree's
//! [`LatexFragment`] (the LaTeX text, the characters to declare, the
//! requirement identifiers, the recomposed user definitions and the trees
//! placed in it) and the diagnostics.
//!
//! The emitter writes bytes as it walks, keeps the author's whitespace, and
//! uses no layout engine. Every byte reaches the output through one method of
//! the [`LatexSink`], chosen by what the byte is: document text through
//! `text`, which escapes it; byte-exact LaTeX through `raw`, which accepts
//! only a value made from a raw construct node or from the source of a node;
//! verbatim content through `escverb` and `verbatim_block`; comments through
//! `comment`; and the emitter's own syntax through the methods that write
//! exactly that syntax. Nothing reads the output back. Because of this,
//! escaping text twice, or escaping inside mathematics or verbatim content,
//! cannot be expressed in this module.
//!
//! **Comments end their own line.** The sink's comment writer writes the
//! newline that ends a comment, so a comment can never run on into what the
//! emitter writes next. The whitespace that followed the comment in the source
//! is written after it, minus the one newline already written. Where no
//! whitespace follows the comment in the source (a paragraph break took it,
//! or the comment ended the input), the emitter leaves out the leading
//! newline of the node that follows instead, so the comment line never adds a
//! blank line. The emitter decides this from the neighboring nodes, never
//! from the output.
//!
//! # What each node writes
//!
//! | lowered node | written |
//! |---|---|
//! | `flm.text` | the text, escaped ([`escape`](crate::stdcore::backends::latex::escape)) |
//! | `flm.group` | `{`, the children, `}` (a group written with brackets keeps them) |
//! | `flm.opaque-wrapper` | its children (the group has no delimiters) |
//! | `flm.comment` | the comment prefix the document was parsed with, the comment's text, the newline that ends the comment's line, and the whitespace that followed it in the source minus that newline; nothing at all, whitespace included, under [`drop_comments`](super::LatexRecomposedRenderOptions::drop_comments) |
//! | `flm.container` | its children |
//! | `flm.pres.latex.macro` | `\name`, then each argument in its shape (below) |
//! | `flm.pres.latex.environment` | `\begin{name}`, the arguments, the body, `\end{name}`; a verbatim body byte for byte on lines of its own, or the escaped block (below) where the environment is inside a token group |
//! | `flm.pres.latex.raw` | the payload's text, byte for byte; text holding a `%` that would start a comment is refused and reported, since only the comment writer writes comments |
//! | `flm.pres.latex.definition` | nothing at its place: the definition is recorded for the style file (below) |
//! | `flm.pres.math-passthrough` | `\(`…`\)`, `\[`…`\]` or `\begin{env}`…`\end{env}` around the source, byte for byte |
//! | `flm.pres.verbatim` | inline: `\EscVerb[formatcom=\flmFmtVRB{<kind>}]{…}` in the `\EscVerb` encoding; block: the `flmVerbatimCode` or `flmVerbatimText` environment, or the escaped block (below) |
//! | `flm.pres.unknown` | a macro as `\name`; an environment as `\begin{name}`, the body, `\end{name}` |
//! | `flm.pres.unsupported` | the comment `% flm: unsupported element <name>` on a line of its own, through the comment writer, which ends that line |
//! | `flm.pres.dropped` | the comment `% flm: dropped <detail>` on a line of its own, written like the `flm.pres.unsupported` comment |
//! | `flm.pres.subtree` | nothing at its place: the position is recorded ([`LatexFragment::placed`]), and the regions stage writes the placed tree's LaTeX there |
//! | `flm.pres.span` | its children; the role, kinds, style and anchor are not written (the structural rule of `flm.math` stages one where no feature processes mathematics) |
//!
//! **The syntax of a call is normalized; its content is kept.** A construct
//! element is written from its payload: a mandatory argument is always
//! braced, no space is written between a command name and its arguments, and
//! `\begin{name}` is followed directly by the arguments and the body. The
//! bytes inside every argument are the document's. An optional argument is
//! written `[…]` when every node in it is a text node or a raw node holding no
//! `]`, or a brace group with any content, and `[{…}]` as soon as it holds
//! anything else: a `]` in text, or a construct whose bytes are not inspected
//! (a macro node, an environment, mathematics), so that a `]` cannot end the
//! argument early. The decision reads the argument's nodes, never the output.
//! A `\ref`'s options, staged (built into the lowered tree; see
//! [`Lowerer`](crate::core::lowering::Lowerer)) as text nodes and raw nodes,
//! take the first
//! form (`\flmRef[S,cap={Figure}]{ref1}`) until a value holds a command
//! (`\flmRef[{cap={\emph{z}}}]{ref1}`); LaTeX reads an optional argument that
//! is one brace group as that group's content. A macro without any argument
//! whose name ends in a letter is followed by one space when the next node
//! begins with a letter: LaTeX skips the space after a command name made of
//! letters, so the meaning is unchanged, whereas a letter written directly
//! after the name would become part of it. The same holds for a macro that no
//! feature declares.
//!
//! **A definition node** ([`LatexDefinitionPayload`]) writes nothing at its
//! place. Its LaTeX, `\providecommand\name[n]{…}` for a macro or
//! `\ProvideDocumentEnvironment{name}{+m …}{…}{…}` for an environment, with
//! the definition's body written through a sink of its own under the same
//! escaping, is recorded once per name in [`LatexFragment::definitions`] for
//! the style file. These definition commands are guarded: a definition of the same name
//! that the user's document class or preamble made first takes precedence. A
//! definition that the render option `bare_names` puts under the document's
//! own name is written unconditionally instead (`\long\def`,
//! `\DeclareDocumentEnvironment`), because such a name is listed precisely so
//! that the document's definition replaces LaTeX's own, which a guard would
//! prevent. A definition that a feature writes from its configuration
//! ([`LatexDefinitionPayload::feature`]) is always guarded, and is recorded
//! with the feature's name, which the style file writes in its comment line.
//! A macro that the render option `robust_names` lists is written
//! `\ProvideDocumentCommand\name{+m …}` (`\DeclareDocumentCommand` under the
//! document's own name), which makes it protected from expansion rather than
//! expandable; an environment is protected however it is written, so the
//! option changes nothing for one.
//!
//! **A verbatim block is written in one of two forms, by one rule.** LaTeX
//! gives a character its meaning when it reads the character from the file,
//! once. A block written out in full (`\begin{flmVerbatimCode}[<lang>]`, the
//! content on lines of its own, `\end{flmVerbatimCode}`) relies on that, so it
//! works only where LaTeX reads it *as lines of the file*. Where the block
//! reaches LaTeX *inside a group that LaTeX reads as tokens* (an argument of a
//! call, the body that an environment declared with `+b` passes on as an
//! argument, or a recomposed definition's own body in the style file), it is
//! written instead as the escaped call
//! `\flmVerbatimBlock[<lang>]{<environment>}{<lines>}`
//! ([`VERBATIM_BLOCK_COMMAND`](preamble::VERBATIM_BLOCK_COMMAND)), which
//! rebuilds the block where it is: one brace group per line of the block and,
//! inside each line, one brace group per tab-separated segment, every segment
//! in the encoding that `\EscVerb` reads. The emitter chooses the form from
//! the constructs that enclose the block. A block in plain document content
//! is therefore written out in full:
//! the recomposed LaTeX is meant to be read and edited by a person, and a code
//! listing is written the way a person would write it.
//!
//! **A verbatim node without a construct around it** (a `flm.pres.verbatim`
//! node that reaches the emitter outside an `\EscVerb` argument or a verbatim
//! environment's body, which happens for a verbatim group that no feature's
//! rule handled) is written the way the `verbatim` feature's rules write
//! theirs, with the `\EscVerb` option from the same
//! [`escverb_options`](preamble::escverb_options), so that the output
//! compiles either way. The emitter then adds the `fvextra` package and the
//! LaTeX family's verbatim definition piece to the tree's requirements
//! ([`LatexFragment::requires`]).
//!
//! # Conditions the emitter reports
//!
//! A condition is the typed body of a diagnostic: a value that says what
//! happened and holds the facts describing it, identified by a name such as
//! `flm.latex-recomposed.unknown-character` (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). The emitter
//! reports four:
//!
//! - [`UnknownCharacter`] (`flm.latex-recomposed.unknown-character`), a
//!   Warning: a character of document text for which no LaTeX spelling is
//!   known. Under the `Declare` Unicode mode the character is written as it
//!   is; under `Replace` it is replaced by a spelling that shows its code
//!   point. Reported once per character per tree.
//! - [`NestingTooDeep`] (`flm.latex-recomposed.nesting-too-deep`), an Error:
//!   the tree nests deeper than [`DEPTH_LIMIT`]; nothing is written for the
//!   tree.
//! - [`NoWorkerThread`] (`flm.latex-recomposed.worker-thread-unavailable`), an
//!   Error: the walk recurses, so it runs on a worker thread, and none could
//!   be created.
//! - [`StageFailed`] (`flm.latex-recomposed.stage-failed`), an Error: the
//!   emitter found a node that breaks an invariant of the lowering rules (an
//!   element that the backend neither writes natively nor expects, a
//!   construct node without its payload, a packed call with an argument that
//!   is not a group), or raw text that a lowering rule assembled holds a `%`.

use std::collections::BTreeSet;
use std::sync::Arc;

use techy::core::node::{NodeKind, NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::lang::guard;
use crate::lang::structural;
use crate::lang::{nowhere, Flm, FlmDiagnostics, FlmGroupType, FlmOrigin};
use crate::model::addr::TreeId;
use crate::model::element::element_of;
use crate::model::names::Name;
use crate::stdcore::backends::latex::construct::{self, BEGIN_SLOT, END_SLOT, arg_slots, slot_named};
use crate::stdcore::backends::latex::{
    Declared, EscapeConfig, LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload,
    LatexEnvironmentPayload, LatexMacroPayload, LatexRaw, LatexRawPayload, LatexSink,
    RecomposedDefinition, packages, preamble,
};
use crate::stdcore::backends::pres::{
    self, BODY_SLOT, MathDisplay, MathPassthroughPayload, UnknownKind, UnknownPayload,
    DroppedPayload, SubtreePayload, UnsupportedPayload, VerbatimDisplay, VerbatimKind, VerbatimPayload, is_presentation,
    payload_of, recompose_source,
};

use super::feature::LatexRecomposedRenderOptions;
use super::stages::StageFailed;

// --- the per-tree record ------------------------------------------------------------------

/// The recomposed LaTeX of one tree, and what the style file must hold because
/// of that tree: the result of [`emit_tree`], and the per-tree value of the
/// table `flm.output.latex-recomposed`
/// ([`LatexFragments`](super::stages::LatexFragments)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LatexFragment {
    /// The LaTeX written for the tree.
    pub text: String,
    /// The characters written as UTF-8 that the style file must declare,
    /// with what their spellings need in the preamble (see
    /// [`Declared`]).
    pub declared: Declared,
    /// The requirement identifiers that the tree needs: the identifiers of
    /// LaTeX packages and definition pieces. [`emit_tree`] fills in only what
    /// the emitter needs on its own, the `fvextra` package and the verbatim
    /// definition piece for a verbatim node without a construct around it
    /// (see the module documentation); the emit stage then adds what the
    /// tree's lowering rules required. A user's per-character spelling
    /// ([`unicode_overrides`](super::LatexRecomposedRenderOptions::unicode_overrides))
    /// adds nothing here.
    pub requires: BTreeSet<Name>,
    /// The user definitions recomposed from the tree's definition nodes, each
    /// with the name it defines (`\hello`, or `note` for an environment) and
    /// its LaTeX text, once per name, in order of first occurrence.
    pub definitions: Vec<RecomposedDefinition>,
    /// The trees placed in this tree (`flm.pres.subtree` nodes), each with the
    /// position in [`text`](Self::text), in bytes, at which its LaTeX is to be
    /// written, in the order they occur. The regions stage writes each placed
    /// tree's LaTeX at its position.
    pub placed: Vec<(usize, TreeId)>,
}

/// What one call of [`emit_tree`] produced: the tree's [`LatexFragment`] and
/// the diagnostics raised.
///
/// A failure is not returned as an error: the fragment is `None`, and an Error
/// diagnostic among [`diags`](Self::diags) says why.
#[derive(Debug)]
pub struct Emitted {
    /// The tree's recomposed LaTeX and record; `None` when the emission could
    /// not run on a worker thread or the tree nests deeper than
    /// [`DEPTH_LIMIT`].
    pub fragment: Option<LatexFragment>,
    /// The diagnostics, in the order they were raised.
    pub diags: FlmDiagnostics,
}

// --- conditions ---------------------------------------------------------------------------

/// Condition `flm.latex-recomposed.unknown-character`, a Warning: a character
/// of document text has no LaTeX spelling, neither in the Unicode-to-LaTeX
/// table of [`untechxt`] (the table that gives each
/// character a LaTeX spelling and states what that spelling needs) nor among
/// the user's per-character spellings
/// ([`unicode_overrides`](super::LatexRecomposedRenderOptions::unicode_overrides)).
///
/// Reported once per character per tree, at the tree's root. Recovery: under
/// the `Declare` Unicode mode the character is written as it is, without a
/// declaration, and the recomposed LaTeX compiles only where the user's LaTeX
/// installation can print it; under `Replace` it is replaced by a spelling
/// that shows its code point
/// ([`escape`](crate::stdcore::backends::latex::escape)).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.unknown-character",
    message = "the character `{ch}` (U+{code}) has no known LaTeX spelling: neither the \
               built-in table of LaTeX spellings nor the render option `unicode_overrides` gives \
               one; give it a spelling in `unicode_overrides`, or leave the character out of \
               the document"
)]
pub struct UnknownCharacter {
    /// The character that has no spelling.
    pub ch: char,
    /// Its code point, as four or more uppercase hexadecimal digits.
    pub code: String,
}

/// Condition `flm.latex-recomposed.worker-thread-unavailable`, an Error: the
/// emitter could not run on a worker thread, because the calling thread is not
/// one and the operating system refused to create one.
///
/// A worker thread is a thread created with at least
/// [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack
/// ([`on_worker_thread`](crate::lang::guard::on_worker_thread)); the emitter
/// needs one because its walk recurses as deeply as the tree nests, and the
/// depth limit ([`DEPTH_LIMIT`]) is computed from the recursion guard's stack
/// budget ([`STACK_BUDGET`](crate::lang::guard::STACK_BUDGET)), which fits
/// within that stack. See [the
/// recursion guard and worker
/// threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads).
/// Recovery: the tree has no recomposed LaTeX ([`Emitted::fragment`] is
/// `None`).
// The refusal is the operating system's and cannot be provoked without
// injecting a failure, so no test covers this arm. See the design rationale
// at [§flm-dd-dr:lang-worker-stack].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.worker-thread-unavailable",
    message = "writing the document as LaTeX needs a thread with a large stack, and the \
               operating system refused to create one: {detail}"
)]
pub struct NoWorkerThread {
    /// The operating system's error message.
    pub detail: String,
}

/// The emitter's per-level stack estimate: an upper bound on the stack that
/// one level of nesting of the walk uses. One level is two frames of the
/// recursion, well under 4 KiB in a debug build; the estimate is three times
/// that, so that the limit holds in every build profile.
const LEVEL_STACK_ESTIMATE: usize = 12 * 1024;

/// The nesting depth beyond which the emitter refuses a tree and reports
/// [`NestingTooDeep`]: the recursion guard's stack budget
/// ([`STACK_BUDGET`](crate::lang::guard::STACK_BUDGET)) divided by the
/// estimated stack use of one level, which gives 4096 levels.
///
/// A tree that came from a parsed document never comes near it, since the
/// parse refuses much shallower nesting; a lowered tree can, when its
/// lowering rules stage (build into the lowered tree) levels of their own.
pub const DEPTH_LIMIT: usize = guard::STACK_BUDGET / LEVEL_STACK_ESTIMATE;

// The limit is a few thousand levels: deep enough for any parsed document,
// shallow enough that a worker thread's stack holds it in every build profile.
const _: () = assert!(DEPTH_LIMIT >= 1024 && DEPTH_LIMIT <= 65536);

/// Condition `flm.latex-recomposed.nesting-too-deep`, an Error: a lowered tree
/// nests deeper than the emitter's depth limit ([`DEPTH_LIMIT`]).
///
/// Reported once, at the tree's root. Recovery: nothing is written for that
/// tree ([`Emitted::fragment`] is `None`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-recomposed.nesting-too-deep",
    message = "the content nests too deeply to be written as LaTeX: {detail}"
)]
pub struct NestingTooDeep {
    /// The limit that was exceeded, as a number of levels.
    pub detail: String,
}

/// What a diagnostic shows in place of a position, for a condition that this
/// backend reports and that has no position in any document: the label of the
/// empty source that [`nowhere`] spans.
pub(crate) const NOWHERE_LABEL: &str = "latex-recomposed backend";

// --- the entry point ----------------------------------------------------------------------

/// Writes the recomposed LaTeX of one lowered tree under the render options
/// `options`, and returns it with the diagnostics raised.
///
/// `comment_prefix` is the text that starts a comment under the document's
/// parsing configuration
/// ([`FlmParsing::comment_start`](crate::lang::FlmParsing::comment_start)); it
/// is `None` when the document recognizes no comments, in which case the tree
/// holds no comment node and the prefix is never used.
///
/// The walk recurses as deeply as the tree nests, so it must run on a worker
/// thread ([`on_worker_thread`](crate::lang::guard::on_worker_thread)): this
/// function runs it on the calling thread when that thread is a worker thread,
/// and on a newly created worker thread otherwise.
///
/// It never panics. A tree that cannot be recomposed comes back with
/// [`Emitted::fragment`] set to `None` and an Error diagnostic
/// ([`NoWorkerThread`] or [`NestingTooDeep`]). The emit stage
/// ([`LatexRecomposedEmitStage`](super::stages::LatexRecomposedEmitStage))
/// calls it once per document tree, and the regions stage once per placed
/// piece; see the module documentation for what each node writes.
pub fn emit_tree(
    tree: &Arc<NodeTree<Flm>>,
    options: &LatexRecomposedRenderOptions,
    comment_prefix: Option<&str>,
) -> Emitted {
    match guard::on_worker_thread(|| emit_on_worker_thread(tree, options, comment_prefix)) {
        Ok(emitted) => emitted,
        Err(error) => {
            let mut diags = FlmDiagnostics::new();
            diags.push(Diagnostic::error(
                NoWorkerThread { detail: error.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
            Emitted { fragment: None, diags }
        }
    }
}

/// The escaping configuration ([`EscapeConfig`]) that the render options
/// translate to: the Unicode mode
/// ([`unicode`](LatexRecomposedRenderOptions::unicode)) and the user's
/// per-character spellings
/// ([`unicode_overrides`](LatexRecomposedRenderOptions::unicode_overrides)).
pub fn escape_config(options: &LatexRecomposedRenderOptions) -> EscapeConfig {
    let overrides =
        options.unicode_overrides.iter().map(|(ch, spelling)| (*ch, spelling.clone())).collect();
    EscapeConfig::new(options.unicode).with_overrides(overrides)
}

/// The body of [`emit_tree`], run once the calling thread is known to be a
/// worker thread.
fn emit_on_worker_thread(
    tree: &Arc<NodeTree<Flm>>,
    options: &LatexRecomposedRenderOptions,
    comment_prefix: Option<&str>,
) -> Emitted {
    let mut emitter = Emitter {
        sink: LatexSink::new(escape_config(options)),
        options,
        comment_prefix: comment_prefix.unwrap_or("%"),
        depth: 0,
        in_token_group: false,
        refused: false,
        requires: BTreeSet::new(),
        definitions: Vec::new(),
        placed: Vec::new(),
        extra_declared: Declared::new(),
        extra_unknown: Vec::new(),
        diags: FlmDiagnostics::new(),
    };
    let root = tree.root();
    let children: Vec<NodeRef<'_, Flm>> = root.children().iter().collect();
    if root.is_list() {
        emitter.run(&children);
    } else {
        emitter.run(&[root]);
    }
    let Emitter { sink, refused, requires, definitions, placed, extra_declared, extra_unknown, mut diags, .. } =
        emitter;
    if refused {
        diags.push(Diagnostic::error(
            NestingTooDeep {
                detail: format!(
                    "more than {DEPTH_LIMIT} levels"
                ),
            },
            root.span().clone(),
        ));
        return Emitted { fragment: None, diags };
    }
    let mut output = sink.finish();
    output.declared.merge(&extra_declared);
    for ch in extra_unknown {
        if !output.unknown.contains(&ch) {
            output.unknown.push(ch);
        }
    }
    for ch in output.unknown {
        diags.push(Diagnostic::warning(
            UnknownCharacter { ch, code: format!("{:04X}", u32::from(ch)) },
            root.span().clone(),
        ));
    }
    let fragment =
        LatexFragment { text: output.text, declared: output.declared, requires, definitions, placed };
    Emitted { fragment: Some(fragment), diags }
}

// --- the emitter ---------------------------------------------------------------------------

/// The state of one walk of [`emit_tree`]: the sink, the render options, and
/// what the walk collects for the tree's [`LatexFragment`].
struct Emitter<'a> {
    /// The sink that every byte of the tree's LaTeX is written through.
    sink: LatexSink,
    /// The render options of the run.
    options: &'a LatexRecomposedRenderOptions,
    /// The text that starts a document comment: `%` when the document
    /// recognizes no comments, in which case the tree holds no comment node
    /// and the value is never used.
    comment_prefix: &'a str,
    /// The number of open levels of the walk.
    depth: usize,
    /// Whether what is being written now reaches LaTeX **inside a group LaTeX
    /// reads as tokens** rather than as lines of the file: an argument of a
    /// call, the body that an environment declared with `+b` passes on as an
    /// argument, or a recomposed definition's own body in the style file.
    ///
    /// It decides how a verbatim block is written, and nothing else. LaTeX
    /// gives a character its meaning once, when it reads it from the file; a
    /// block written out in full relies on that, so inside a token group,
    /// whose characters were read before any command could mark them as
    /// verbatim, the block is written as the escaped call
    /// `\flmVerbatimBlock` instead ([`Emitter::verbatim_block_call`]).
    in_token_group: bool,
    /// Whether the depth limit ([`DEPTH_LIMIT`]) was reached.
    refused: bool,
    /// The requirements the emitter added on its own.
    requires: BTreeSet<Name>,
    /// The recomposed definitions, once per name.
    definitions: Vec<RecomposedDefinition>,
    /// The trees placed in the tree, each at its position in the text.
    placed: Vec<(usize, TreeId)>,
    /// What the definitions' own sinks declared, added to the tree's record
    /// when the walk ends.
    extra_declared: Declared,
    /// The characters that the definitions' own sinks could not spell,
    /// reported with the tree's own when the walk ends.
    extra_unknown: Vec<char>,
    /// The diagnostics raised so far.
    diags: FlmDiagnostics,
}

/// How many LaTeX parameters a recomposed definition declares: one per
/// argument, or a single one, regardless of the number of arguments, where
/// the call passes all the arguments in one group
/// ([`LatexDefinitionPayload::packed`]).
fn latex_parameters(payload: &LatexDefinitionPayload) -> usize {
    if payload.packed { 1 } else { payload.args }
}

/// The text written before a recomposed macro definition's body: the defining
/// command, the name, and the number of parameters the definition declares.
///
/// `\providecommand\<name>[<n>]` guards the name — a definition of it the
/// user's class or preamble made before the style file is loaded takes
/// precedence — and
/// makes a **long** macro, so an argument may hold a paragraph break, and a
/// plainly **expandable** one, so a call of it survives `\edef`, a PDF
/// bookmark and a case change. A definition the render option `bare_names`
/// puts under the document's own name takes `\long\def\<name>#1…#<n>`
/// instead: unconditional, because listing the name asks for exactly that, and
/// with the parameter text written out, which is how a `\def` says how many
/// arguments it takes. `\long\def` also has no opinion about the name, where
/// `\providecommand` refuses one whose first three letters are `end`.
///
/// A definition the render option `robust_names` asks for
/// ([`LatexDefinitionPayload::robust`]) takes the document command interface
/// instead — `\ProvideDocumentCommand\<name>{+m …}`, one `+m` per parameter,
/// and `\DeclareDocumentCommand` where the name is the document's own — which
/// makes a command protected from expansion: a call of it goes into a table of
/// contents or an auxiliary file as itself rather than as what it expands to,
/// `hyperref` cannot expand it into a PDF bookmark, and a case change leaves
/// the definition's own words alone. The `+` makes each argument long, as
/// `\providecommand` is of itself.
///
/// A definition whose call packs its arguments declares exactly one
/// parameter, which holds every argument ([`latex_parameters`]).
fn macro_definition_head(payload: &LatexDefinitionPayload) -> String {
    let name = &payload.name;
    let parameters = latex_parameters(payload);
    if payload.robust {
        let definer =
            if payload.is_bare() { "DeclareDocumentCommand" } else { "ProvideDocumentCommand" };
        let specifiers: Vec<&str> = std::iter::repeat_n("+m", parameters).collect();
        return format!("\\{definer}\\{name}{{{}}}", specifiers.join(" "));
    }
    if payload.is_bare() {
        let text: String = (1..=parameters).map(|at| format!("#{at}")).collect();
        return format!("\\long\\def\\{name}{text}");
    }
    match parameters {
        0 => format!("\\providecommand\\{name}"),
        count => format!("\\providecommand\\{name}[{count}]"),
    }
}

/// A recomposed environment definition's argument specification, in braces:
/// one `+m` per argument, in order, and `{}` where it takes none — or one
/// `+m` altogether where the call hands every argument over in a single group
/// ([`latex_parameters`]) — followed by `+b` where the environment takes its
/// own body as an argument ([`LatexDefinitionPayload::body_argument`]), which
/// is always the last one.
///
/// Every argument of a recomposed definition is mandatory, since a call writes
/// one brace group per argument and nothing else. The `+` makes the argument
/// long, so it may hold a paragraph break; the document-environment interface
/// has accepted it since `xparse`, on the body-grabbing `b` as on `m`.
///
/// `b` asks LaTeX to pass what is written between `\begin` and `\end` to the
/// definition as an argument instead of setting it between the begin and the
/// end code. A definition is written that way when it wants the body somewhere
/// a begin code and an end code cannot put it — inside another command's
/// argument, or in two places — and its whole definition is then the begin
/// code, the body written as this parameter wherever the definition wants it.
fn environment_argument_specification(payload: &LatexDefinitionPayload) -> String {
    let mut specifiers: Vec<&str> = std::iter::repeat_n("+m", latex_parameters(payload)).collect();
    if payload.body_argument {
        specifiers.push("+b");
    }
    format!("{{{}}}", specifiers.join(" "))
}

/// Whether `node` is a `flm.pres.unsupported` node or a `flm.pres.dropped`
/// node, each of which is written as a comment that ends its own line.
fn is_comment_line(node: &NodeRef<'_, Flm>) -> bool {
    is_presentation(node) && matches!(element_of(node).name.as_str(), pres::UNSUPPORTED | pres::DROPPED)
}

/// Whether `node` is a text node whose text ends with a newline.
fn ends_with_newline(node: &NodeRef<'_, Flm>) -> bool {
    node.chars().is_some_and(|text| text.ends_with('\n'))
}

/// Whether what is written for `node` begins with an ASCII letter, which
/// written directly after a command name would extend the name: a text node
/// or a raw node (`flm.pres.latex.raw`) whose text begins with one, or a
/// container or list whose
/// first child does.
fn begins_with_letter(node: &NodeRef<'_, Flm>) -> bool {
    let starts = |text: &str| text.starts_with(|c: char| c.is_ascii_alphabetic());
    if let Some(text) = node.chars() {
        return starts(text);
    }
    if let Some(raw) = payload_of::<LatexRawPayload>(node) {
        return starts(&raw.text);
    }
    let transparent = node.is_list()
        || (!is_presentation(node) && element_of(node).name.as_str() == structural::CONTAINER)
        || (is_presentation(node) && element_of(node).name.as_str() == pres::SPAN);
    transparent && node.children().first().is_some_and(|first| begins_with_letter(&first))
}

/// One part of a run of verbatim content, as the emitter writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum VerbatimPiece {
    /// The document's own characters: verbatim content, written as such.
    Text(String),
    /// A raw node (`flm.pres.latex.raw`) inside the verbatim content: LaTeX
    /// that the recomposition put there (the parameter that one of a
    /// recomposed definition's arguments is written as), which is written
    /// unchanged rather than as characters of the document.
    Latex(String),
}

/// The parts of a run of verbatim nodes ([`VerbatimPiece`]): a raw node as
/// the LaTeX it holds, text runs as they are, an
/// interpolated brace group's children without its delimiters (its braces
/// are the syntax of the invocation that substituted it, not content), and
/// anything else as its source spelling.
fn verbatim_pieces(nodes: &[NodeRef<'_, Flm>]) -> Vec<VerbatimPiece> {
    let mut out = Vec::new();
    collect_verbatim_pieces(nodes, &mut out);
    out
}

fn collect_verbatim_pieces(nodes: &[NodeRef<'_, Flm>], out: &mut Vec<VerbatimPiece>) {
    for node in nodes {
        if let Some(raw) = payload_of::<LatexRawPayload>(node) {
            out.push(VerbatimPiece::Latex(raw.text.clone()));
        } else if let Some(text) = node.chars() {
            out.push(VerbatimPiece::Text(String::from(text)));
        } else if node.group_type() == Some(FlmGroupType::Content) {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            collect_verbatim_pieces(&children, out);
        } else {
            out.push(VerbatimPiece::Text(recompose_source(&[*node])));
        }
    }
}

/// The characters of a run of verbatim nodes: the parts that
/// [`verbatim_pieces`] returns, joined. A block written out in full holds
/// these characters.
fn verbatim_text(nodes: &[NodeRef<'_, Flm>]) -> String {
    let mut out = String::new();
    for piece in verbatim_pieces(nodes) {
        match piece {
            VerbatimPiece::Text(text) | VerbatimPiece::Latex(text) => out.push_str(&text),
        }
    }
    out
}

/// The line and segment groups of an escaped block: one group per line of the
/// content and, inside each line, one group per tab-separated segment, each
/// holding the parts that fall in it.
///
/// A content ending with a line feed ends its last line rather than opening an
/// empty one, because a block written out in full writes no empty last line
/// either ([`Emitter::verbatim_body`]) and the two forms must set the same
/// block. Every other line feed opens a line, an empty one included, and
/// content that is empty altogether is one empty line — again what the full
/// form writes.
fn verbatim_block_groups(pieces: Vec<VerbatimPiece>) -> Vec<Vec<Vec<VerbatimPiece>>> {
    let mut lines: Vec<Vec<Vec<VerbatimPiece>>> = vec![vec![Vec::new()]];
    let push = |lines: &mut Vec<Vec<Vec<VerbatimPiece>>>, piece: VerbatimPiece| {
        lines
            .last_mut()
            .expect("a line is open")
            .last_mut()
            .expect("a segment is open")
            .push(piece);
    };
    for piece in pieces {
        match piece {
            VerbatimPiece::Latex(text) => push(&mut lines, VerbatimPiece::Latex(text)),
            VerbatimPiece::Text(text) => {
                for (at_line, line) in text.split('\n').enumerate() {
                    if at_line > 0 {
                        lines.push(vec![Vec::new()]);
                    }
                    for (at_segment, segment) in line.split('\t').enumerate() {
                        if at_segment > 0 {
                            lines.last_mut().expect("a line is open").push(Vec::new());
                        }
                        if !segment.is_empty() {
                            push(&mut lines, VerbatimPiece::Text(String::from(segment)));
                        }
                    }
                }
            }
        }
    }
    let empty_last = lines.last().is_some_and(|line| line.len() == 1 && line[0].is_empty());
    if lines.len() > 1 && empty_last {
        lines.pop();
    }
    lines
}

impl Emitter<'_> {
    /// Opens one level of the walk; `false`, with the refusal recorded,
    /// past the depth limit.
    fn descend(&mut self) -> bool {
        if self.depth >= DEPTH_LIMIT {
            self.refused = true;
            return false;
        }
        self.depth += 1;
        true
    }

    /// Closes one level of the walk.
    fn ascend(&mut self) {
        self.depth -= 1;
    }

    /// Reports a broken internal invariant at `node` as
    /// [`StageFailed`](super::StageFailed): an element that the backend
    /// neither writes natively nor expects reached the emitter. `what`
    /// describes the node and comes before the element's name in the
    /// message's detail.
    fn unexpected(&mut self, node: &NodeRef<'_, Flm>, what: &str) {
        self.diags.push(Diagnostic::error(
            StageFailed {
                stage: String::from(super::stages::EMIT_STAGE),
                detail: format!("{what} `{}` reached the LaTeX emitter, which cannot write it", element_of(node).name),
            },
            node.span().clone(),
        ));
    }

    /// A run of sibling nodes, in order.
    fn run(&mut self, nodes: &[NodeRef<'_, Flm>]) {
        for (index, node) in nodes.iter().enumerate() {
            if !self.descend() {
                return;
            }
            self.node(node, nodes, index);
            self.ascend();
        }
    }

    /// One node, with its siblings and its index among them (the
    /// `flm.pres.unsupported` and `flm.pres.dropped` nodes depend on their
    /// neighbors).
    fn node(&mut self, node: &NodeRef<'_, Flm>, siblings: &[NodeRef<'_, Flm>], index: usize) {
        let ended = self.line_already_ended(siblings, index);
        if let Some(text) = node.chars() {
            self.text(if ended { text.strip_prefix('\n').unwrap_or(text) } else { text });
            return;
        }
        if node.is_comment() {
            self.comment(node);
            return;
        }
        if !is_presentation(node) {
            match node.kind() {
                NodeKind::Group(data) if data.group_type == Some(FlmGroupType::OpaqueWrapper) => {
                    self.children(node);
                }
                NodeKind::Group(data) => {
                    let source = node.span().source();
                    let brackets =
                        data.open.resolve(source) == "[" && data.close.resolve(source) == "]";
                    if brackets {
                        self.sink.open_bracket();
                    } else {
                        self.sink.open_brace();
                    }
                    self.children(node);
                    if brackets {
                        self.sink.close_bracket();
                    } else {
                        self.sink.close_brace();
                    }
                }
                NodeKind::List => {
                    self.children(node);
                }
                _ => {
                    let name = element_of(node).name.clone();
                    if name.as_str() == structural::CONTAINER {
                        self.children(node);
                    } else {
                        self.unexpected(node, "a node of the element");
                    }
                }
            }
            return;
        }
        let next = siblings.get(index + 1);
        let name = element_of(node).name.clone();
        match name.as_str() {
            construct::MACRO => self.macro_call(node, next),
            construct::ENVIRONMENT => self.environment(node),
            construct::RAW => {
                if let Some(payload) = payload_of::<LatexRawPayload>(node) {
                    // A paragraph break is a raw piece of two newlines, and the
                    // first of them is the one that ends the line before it; a
                    // comment has already ended that line.
                    if ended && payload.text.starts_with('\n') {
                        let text = LatexRawPayload::new(&payload.text[1..]);
                        self.write_raw(node, &LatexRaw::from_raw_payload(&text));
                    } else {
                        self.write_raw(node, &LatexRaw::from_raw_payload(payload));
                    }
                }
            }
            construct::DEFINITION => self.definition(node),
            pres::MATH_PASSTHROUGH => self.math(node),
            pres::VERBATIM => self.verbatim(node),
            pres::UNKNOWN => self.unknown(node, next),
            pres::UNSUPPORTED => self.unsupported(node, siblings, index),
            pres::DROPPED => self.dropped(node, siblings, index),
            pres::SUBTREE => self.subtree(node),
            pres::SPAN => self.children(node),
            _ => self.unexpected(node, "a presentation node of the element"),
        }
    }

    /// Whether the line is already ended at this position, so that what is
    /// written next drops one leading newline of its own.
    ///
    /// Three things end their own line through the sink's one comment writer: a
    /// comment of the document, the comment written for a
    /// `flm.pres.unsupported` node, and the comment written for a
    /// `flm.pres.dropped` node. For a comment, the newline the sink wrote replaces the first
    /// newline of the whitespace that followed the comment in the source —
    /// which the comment itself writes back minus that newline when it has
    /// one. Where it has none, because a paragraph break took the whitespace,
    /// the newline to leave out is the next node's.
    ///
    /// A dropped comment (`drop_comments`) ends no line: it writes nothing.
    fn line_already_ended(&self, siblings: &[NodeRef<'_, Flm>], index: usize) -> bool {
        let Some(previous) = index.checked_sub(1).and_then(|i| siblings.get(i)) else {
            return false;
        };
        if is_comment_line(previous) {
            return true;
        }
        if self.options.drop_comments {
            return false;
        }
        let Some(data) = previous.comment() else { return false };
        !data.post_space.resolve(previous.span().source()).starts_with('\n')
    }

    /// The children of `node`, as a run.
    fn children(&mut self, node: &NodeRef<'_, Flm>) {
        let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
        self.run(&children);
    }

    /// The content of a slot, as a run; nothing for an absent slot.
    fn slot(&mut self, slot: Option<techy::core::node::NodeSlice<'_, Flm>>) {
        if let Some(slot) = slot {
            let nodes: Vec<NodeRef<'_, Flm>> = slot.iter().collect();
            self.run(&nodes);
        }
    }

    /// Writes document text, escaped by the sink. A user's per-character
    /// spelling adds nothing to the style file: the document's preamble must
    /// load what that spelling needs.
    fn text(&mut self, text: &str) {
        self.sink.text(text);
    }

    /// A comment: the prefix and the text through the sink, which ends the
    /// comment's line itself, then the whitespace that followed the comment in
    /// the source minus the one newline the sink has already written — a
    /// comment runs to the end of its line, so that whitespace begins with a
    /// newline except at the end of the input, where there is none to leave
    /// out. Nothing at all under `drop_comments`: the comment goes and its
    /// whitespace goes with it.
    fn comment(&mut self, node: &NodeRef<'_, Flm>) {
        if self.options.drop_comments {
            return;
        }
        let Some(data) = node.comment() else { return };
        let source = node.span().source();
        self.sink.comment(self.comment_prefix, data.content.resolve(source));
        let after = data.post_space.resolve(source);
        self.sink.text(after.strip_prefix('\n').unwrap_or(after));
    }

    /// The arguments of a macro or environment node, each in its shape — or,
    /// under `packed`, every argument inside one brace group, one pair of
    /// braces around each ([`packed_arguments`](Emitter::packed_arguments)).
    fn arguments(&mut self, node: &NodeRef<'_, Flm>, args: &[LatexArgShape], packed: bool) {
        // Every argument of a call is a group LaTeX reads as tokens, whatever
        // its shape and however deep the content inside it goes.
        let outer = std::mem::replace(&mut self.in_token_group, true);
        if packed {
            self.packed_arguments(node, args);
        } else {
            self.argument_slots(node, args);
        }
        self.in_token_group = outer;
    }

    /// The arguments of a macro or environment node written one after the
    /// other, each in its shape ([`Emitter::arguments`], which sets the
    /// token-group flag around this).
    fn argument_slots(&mut self, node: &NodeRef<'_, Flm>, args: &[LatexArgShape]) {
        for (shape, slot) in arg_slots(node, args) {
            match shape {
                LatexArgShape::Star => self.sink.star(),
                LatexArgShape::Optional => {
                    let nodes: Vec<NodeRef<'_, Flm>> =
                        slot.map(|slot| slot.iter().collect()).unwrap_or_default();
                    self.optional(&nodes);
                }
                LatexArgShape::Mandatory => {
                    self.sink.open_brace();
                    self.slot(slot);
                    self.sink.close_brace();
                }
                LatexArgShape::EscVerb => {
                    let nodes: Vec<NodeRef<'_, Flm>> =
                        slot.map(|slot| slot.iter().collect()).unwrap_or_default();
                    let pieces = verbatim_pieces(&nodes);
                    self.sink.open_brace();
                    self.escverb_pieces(node, pieces);
                    self.sink.close_brace();
                }
            }
        }
    }

    /// Every argument of a call inside **one** brace group, one pair of braces
    /// around each, in order: `\<name>{{first}{second}…}`.
    ///
    /// This is how a call of a definition of more than nine effective arguments is
    /// written, since a LaTeX macro body can name only nine parameters; the
    /// definition's body reads argument *n* back out of the group with the
    /// style file's accessor (`\flmArg{<n>}{#1}`). Every argument written this
    /// way is a brace group regardless of its shape, so a star or an optional
    /// argument, neither of which is a brace group, cannot be packed. Finding
    /// one breaks an invariant of the lowering rule that staged the node, and
    /// is not something a document can ask for; the same holds for verbatim
    /// content ([`LatexArgShape::EscVerb`]), which no call of a recomposed
    /// definition writes. Such a call is reported as [`StageFailed`], and its
    /// arguments are not written; the command name, and for an environment
    /// its body and `\end`, are.
    fn packed_arguments(&mut self, node: &NodeRef<'_, Flm>, args: &[LatexArgShape]) {
        if args.iter().any(|shape| *shape != LatexArgShape::Mandatory) {
            return self.unexpected(node, "a packed call with an argument that is not a group,");
        }
        self.sink.open_brace();
        for (_, slot) in arg_slots(node, args) {
            self.sink.open_brace();
            self.slot(slot);
            self.sink.close_brace();
        }
        self.sink.close_brace();
    }

    /// An optional argument: `[…]` when every node of the slot is a text
    /// node or a raw node holding no `]`, or a brace group; `[{…}]` as soon
    /// as the slot holds anything else (see the module documentation).
    fn optional(&mut self, nodes: &[NodeRef<'_, Flm>]) {
        let plain = nodes.iter().all(|node| {
            if let Some(text) = node.chars() {
                !text.contains(']')
            } else if let Some(raw) = payload_of::<LatexRawPayload>(node) {
                !raw.text.contains(']')
            } else {
                node.group_type() == Some(FlmGroupType::Content)
            }
        });
        self.sink.open_bracket();
        if !plain {
            self.sink.open_brace();
        }
        self.run(nodes);
        if !plain {
            self.sink.close_brace();
        }
        self.sink.close_bracket();
    }

    /// `\name` and, when no argument follows the name, it ends in a letter
    /// and the next node of the run begins with one, one space, so that the
    /// letter written next does not extend the name.
    fn control_word(&mut self, name: &str, has_arguments: bool, next: Option<&NodeRef<'_, Flm>>) {
        self.sink.control_word(name);
        if !has_arguments
            && name.ends_with(|c: char| c.is_ascii_alphabetic())
            && next.is_some_and(begins_with_letter)
        {
            self.sink.text(" ");
        }
    }

    /// A `flm.pres.latex.macro` node, given the node that follows it in its
    /// run.
    fn macro_call(&mut self, node: &NodeRef<'_, Flm>, next: Option<&NodeRef<'_, Flm>>) {
        let Some(payload) = payload_of::<LatexMacroPayload>(node) else {
            return self.unexpected(node, "a macro node without its payload,");
        };
        self.control_word(&payload.name, !payload.args.is_empty(), next);
        self.arguments(node, &payload.args, payload.packed);
    }

    /// A `flm.pres.latex.environment` node.
    fn environment(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<LatexEnvironmentPayload>(node) else {
            return self.unexpected(node, "an environment node without its payload,");
        };
        if payload.verbatim && self.in_token_group {
            return self.escaped_verbatim_environment(node, payload);
        }
        self.sink.begin(&payload.name);
        self.arguments(node, &payload.args, payload.packed);
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        // An environment declared with `+b` hands its body to the definition
        // as an argument, so the body is a token group; an environment LaTeX
        // sets the body of keeps whatever the body already was.
        let outer = self.in_token_group;
        self.in_token_group |= payload.body_argument;
        if payload.verbatim {
            self.verbatim_body(&body);
        } else {
            self.run(&body);
        }
        self.in_token_group = outer;
        self.sink.end(&payload.name);
    }

    /// A verbatim block written as the escaped call
    /// [`\flmVerbatimBlock`](preamble::VERBATIM_BLOCK_COMMAND), which is how a
    /// block is written wherever it reaches LaTeX inside a group LaTeX reads
    /// as tokens ([`in_token_group`](Emitter::in_token_group)):
    ///
    /// ```latex
    /// \flmVerbatimBlock[<lang>]{flmVerbatimCode}{{{first\ line}}{{second}}}
    /// ```
    ///
    /// The optional argument is the block environment's own, written always,
    /// `[<lang>]` or `[]`, exactly as the full form writes it. The first
    /// mandatory argument is the environment's name, so one command serves
    /// every block environment. The second is the content: one brace group per
    /// line and, inside each line, one brace group per tab-separated segment,
    /// every segment in the encoding `\EscVerb` reads
    /// ([`escverb`](LatexSink::escverb)) — the same encoding inline verbatim
    /// content is written in.
    ///
    /// The line and segment structure is carried by braces rather than by a
    /// line feed and a tab character, because LaTeX does not keep either
    /// character: a line feed written into an auxiliary file (a table of
    /// contents) is read back from it as an ordinary space, and a tab inside a
    /// macro argument becomes a space token before any command can inspect it.
    /// Brace groups are read back unchanged.
    fn verbatim_block_call(
        &mut self,
        node: &NodeRef<'_, Flm>,
        env: &str,
        options: impl FnOnce(&mut Self),
        pieces: Vec<VerbatimPiece>,
    ) {
        self.sink.control_word(preamble::VERBATIM_BLOCK_COMMAND);
        options(self);
        self.sink.open_brace();
        self.raw_text(node, env);
        self.sink.close_brace();
        self.sink.open_brace();
        for line in verbatim_block_groups(pieces) {
            self.sink.open_brace();
            for segment in line {
                self.sink.open_brace();
                self.escverb_pieces(node, segment);
                self.sink.close_brace();
            }
            self.sink.close_brace();
        }
        self.sink.close_brace();
    }

    /// Writes a run of verbatim parts in the encoding that `\EscVerb` reads:
    /// the document's characters escaped, and the LaTeX that a recomposed
    /// template writes at one of its placeholders written unchanged, since an
    /// escaped parameter would be the two characters `\#` and a digit rather
    /// than a parameter.
    fn escverb_pieces(&mut self, node: &NodeRef<'_, Flm>, pieces: Vec<VerbatimPiece>) {
        for piece in pieces {
            match piece {
                VerbatimPiece::Text(text) => self.sink.escverb(&text),
                VerbatimPiece::Latex(text) => self.raw_text(node, &text),
            }
        }
    }

    /// A `flm.pres.latex.environment` node whose body is verbatim, found inside
    /// a token group: the escaped call ([`Emitter::verbatim_block_call`]) over
    /// the environment's name, its own optional argument and its body's text.
    fn escaped_verbatim_environment(
        &mut self,
        node: &NodeRef<'_, Flm>,
        payload: &LatexEnvironmentPayload,
    ) {
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let pieces = verbatim_pieces(&body);
        let args = payload.args.clone();
        let name = payload.name.clone();
        self.verbatim_block_call(node, &name, |me| me.arguments(node, &args, false), pieces);
    }

    /// A verbatim environment body on lines of its own: a newline after the
    /// `\begin` line, the content byte for byte, and a newline before the
    /// `\end` line unless the content already ends with one.
    fn verbatim_body(&mut self, body: &[NodeRef<'_, Flm>]) {
        let text = verbatim_text(body);
        self.sink.newline();
        // Verbatim content, not LaTeX: every character stands for itself, `%`
        // included, so this is the sink's verbatim entry point rather than its
        // raw one.
        self.sink.verbatim_block(&text);
        if !text.ends_with('\n') {
            self.sink.newline();
        }
    }

    /// A `flm.pres.latex.definition` node: nothing at its place; the definition
    /// recorded for the style file, once per name.
    ///
    /// A definition written under a prefix of its own is **guarded**, so that
    /// a definition of that name the user's class or preamble made before the
    /// style file is loaded takes precedence: `\providecommand\<name>[<n>]{<body>}` for a
    /// macro, which makes a long expandable macro, and
    /// `\ProvideDocumentEnvironment{<name>}{+m …}{<begin>}{<end>}` for an
    /// environment, which cannot be expandable — `\begin` opens a group, runs
    /// the begin code, sets the body and runs the end code, and there is
    /// nothing expansion alone could produce.
    ///
    /// A definition the render option `bare_names` puts under the document's
    /// own name is written **unconditionally** — `\long\def` for a macro,
    /// `\DeclareDocumentEnvironment` for an environment — because that name is
    /// listed exactly so that the document's definition replaces LaTeX's own,
    /// which a guard would quietly prevent.
    ///
    /// A macro that the render option `robust_names` lists
    /// ([`LatexDefinitionPayload::robust`]) is written through the document
    /// command interface, `\ProvideDocumentCommand\<name>{+m …}` — one `+m`
    /// per parameter, `\DeclareDocumentCommand` under the document's own name —
    /// which makes it **robust** (protected from expansion), so a call goes into a
    /// table of contents or an auxiliary file as itself. An environment is
    /// robust however it is written, so the option changes nothing for one.
    ///
    /// A definition whose call **packs** its arguments
    /// ([`LatexDefinitionPayload::packed`]) declares one LaTeX parameter
    /// holding all of them, regardless of their number: `\providecommand\<name>[1]{…}`,
    /// `\ProvideDocumentEnvironment{<name>}{+m}{…}{…}`. Its body reads
    /// argument *n* out of that parameter with the style file's accessor,
    /// which the lowering rule writes and requires.
    ///
    /// An environment that takes its **body** as an argument
    /// ([`LatexDefinitionPayload::body_argument`]) declares `+b` after its
    /// arguments and carries its whole definition in the begin code, its end
    /// code empty: `\ProvideDocumentEnvironment{<name>}{+m +b}{…#2…}{}`.
    ///
    /// A feature's **declaration** ([`LatexDefinitionKind::Declaration`]) is
    /// written on one line as `\ifcsname <name>\endcsname\else<body>\fi`,
    /// always guarded: something the class, a package or the author's
    /// preamble defined under that name first is kept.
    fn definition(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<LatexDefinitionPayload>(node) else {
            return self.unexpected(node, "a definition node without its payload,");
        };
        let display_name = match payload.kind {
            LatexDefinitionKind::Macro => format!("\\{}", payload.name),
            LatexDefinitionKind::Environment | LatexDefinitionKind::Declaration => {
                payload.name.clone()
            }
        };
        // The name the document declared, for the style file's comment line;
        // the definition itself is written under the recomposed name.
        let source_display_name = match payload.kind {
            LatexDefinitionKind::Macro => format!("\\{}", payload.source_name),
            LatexDefinitionKind::Environment | LatexDefinitionKind::Declaration => {
                payload.source_name.clone()
            }
        };
        if self.definitions.iter().any(|definition| definition.name == display_name) {
            return;
        }
        // The slots are written through a sink of their own — the definition
        // is not in flow — under the same escaping; what that sink learned
        // joins this tree's record.
        let mut sub = Emitter {
            sink: LatexSink::new(self.sink.config().clone()),
            options: self.options,
            comment_prefix: self.comment_prefix,
            depth: self.depth,
            // A definition's body, begin code and end code are token lists of
            // the style file: `\providecommand\name{<body>}` and
            // `\ProvideDocumentEnvironment{name}{…}{<begin>}{<end>}` are read
            // as tokens when the style file is loaded.
            in_token_group: true,
            refused: false,
                requires: BTreeSet::new(),
            definitions: Vec::new(),
            placed: Vec::new(),
            extra_declared: Declared::new(),
            extra_unknown: Vec::new(),
            diags: FlmDiagnostics::new(),
        };
        match payload.kind {
            LatexDefinitionKind::Macro => {
                sub.raw_text(node, &macro_definition_head(payload));
                sub.sink.open_brace();
                sub.slot(slot_named(node, BODY_SLOT));
                sub.sink.close_brace();
                sub.sink.newline();
            }
            LatexDefinitionKind::Environment => {
                let definer = if payload.is_bare() {
                    "DeclareDocumentEnvironment"
                } else {
                    "ProvideDocumentEnvironment"
                };
                sub.raw_text(node, &format!("\\{definer}{{{}}}", payload.name));
                sub.raw_text(node, &environment_argument_specification(payload));
                sub.sink.open_brace();
                sub.slot(slot_named(node, BEGIN_SLOT));
                sub.sink.close_brace();
                sub.sink.open_brace();
                sub.slot(slot_named(node, END_SLOT));
                sub.sink.close_brace();
                sub.sink.newline();
            }
            LatexDefinitionKind::Declaration => {
                sub.raw_text(node, &format!("\\ifcsname {}\\endcsname\\else", payload.name));
                sub.slot(slot_named(node, BODY_SLOT));
                sub.raw_text(node, "\\fi");
                sub.sink.newline();
            }
        }
        let Emitter { sink, refused, requires, definitions, extra_declared, extra_unknown, diags, .. } =
            sub;
        self.refused |= refused;
        self.requires.extend(requires);
        for definition in definitions {
            if !self.definitions.iter().any(|seen| seen.name == definition.name) {
                self.definitions.push(definition);
            }
        }
        for diagnostic in diags.iter() {
            self.diags.push(diagnostic.clone());
        }
        let output = sink.finish();
        self.extra_declared.merge(&output.declared);
        self.extra_declared.merge(&extra_declared);
        for ch in output.unknown.into_iter().chain(extra_unknown) {
            if !self.extra_unknown.contains(&ch) {
                self.extra_unknown.push(ch);
            }
        }
        let mut recorded =
            RecomposedDefinition::new(display_name, source_display_name, output.text);
        if payload.robust {
            recorded = recorded.robust();
        }
        if payload.kind == LatexDefinitionKind::Declaration {
            recorded = recorded.declaration();
        }
        if let Some(feature) = &payload.feature {
            recorded = recorded.of_feature(feature.as_str());
        }
        self.definitions.push(recorded);
    }

    /// The emitter's own syntax written byte for byte, reported at `node`
    /// where the sink refuses it.
    fn raw_text(&mut self, node: &NodeRef<'_, Flm>, text: &str) {
        self.write_raw(node, &LatexRaw::from_raw_payload(&LatexRawPayload::new(text)));
    }

    /// Writes byte-exact LaTeX through the sink, and reports the only thing
    /// the sink refuses: raw text that a lowering rule assembled holding a `%`,
    /// which would turn everything after it into a comment. That is an
    /// internal error, never anything an author can write; nothing is written
    /// for the text, and [`StageFailed`](super::StageFailed) is reported at
    /// `node`, the node the text came from.
    fn write_raw(&mut self, node: &NodeRef<'_, Flm>, raw: &LatexRaw) {
        let Err(refused) = self.sink.raw(raw) else { return };
        self.diags.push(Diagnostic::error(
            StageFailed {
                stage: String::from(super::stages::EMIT_STAGE),
                detail: refused.to_string(),
            },
            node.span().clone(),
        ));
    }

    /// A `flm.pres.math-passthrough` node: the delimiters, and the source byte
    /// for byte.
    fn math(&mut self, node: &NodeRef<'_, Flm>) {
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let payload = payload_of::<MathPassthroughPayload>(node);
        let display = payload.map_or(MathDisplay::Inline, |payload| payload.display);
        let env = payload.and_then(|payload| payload.env.clone());
        match (display, env) {
            (_, Some(env)) => {
                self.sink.begin(&env);
                self.write_raw(node, &LatexRaw::from_source(&body));
                self.sink.end(&env);
            }
            (MathDisplay::Inline, None) => {
                self.sink.control_word("(");
                self.write_raw(node, &LatexRaw::from_source(&body));
                self.sink.control_word(")");
            }
            (MathDisplay::Display, None) => {
                self.sink.control_word("[");
                self.write_raw(node, &LatexRaw::from_source(&body));
                self.sink.control_word("]");
            }
        }
    }

    /// A bare `flm.pres.verbatim` node, written as the `verbatim` feature's
    /// rules write theirs (see the module documentation).
    fn verbatim(&mut self, node: &NodeRef<'_, Flm>) {
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let (kind, display, lang) = payload_of::<VerbatimPayload>(node)
            .map_or((VerbatimKind::Text, VerbatimDisplay::Inline, None), |payload| {
                (payload.kind, payload.display, payload.lang.clone())
            });
        self.requires.insert(Name::from(packages::FVEXTRA));
        self.requires.insert(Name::from(preamble::VERBATIM));
        match display {
            VerbatimDisplay::Inline => {
                self.sink.control_word(preamble::ESCVERB);
                self.sink.open_bracket();
                self.write_raw(
                    node,
                    &LatexRaw::from_raw_payload(&LatexRawPayload::new(preamble::escverb_options(
                        kind,
                    ))),
                );
                self.sink.close_bracket();
                self.sink.open_brace();
                let pieces = verbatim_pieces(&body);
                self.escverb_pieces(node, pieces);
                self.sink.close_brace();
            }
            VerbatimDisplay::Block => {
                let env = match kind {
                    VerbatimKind::Text => preamble::VERBATIM_TEXT_ENVIRONMENT,
                    VerbatimKind::Code | VerbatimKind::Identifier => {
                        preamble::VERBATIM_CODE_ENVIRONMENT
                    }
                };
                let lang = lang.filter(|_| env == preamble::VERBATIM_CODE_ENVIRONMENT);
                if self.in_token_group {
                    let pieces = verbatim_pieces(&body);
                    return self.verbatim_block_call(
                        node,
                        env,
                        |me| {
                            me.sink.open_bracket();
                            if let Some(lang) = lang {
                                me.sink.text(&lang);
                            }
                            me.sink.close_bracket();
                        },
                        pieces,
                    );
                }
                self.sink.begin(env);
                if let Some(lang) = lang {
                    self.sink.open_bracket();
                    self.sink.text(&lang);
                    self.sink.close_bracket();
                }
                self.verbatim_body(&body);
                self.sink.end(env);
            }
        }
    }

    /// A `flm.pres.unknown` node: a macro as its name, an environment
    /// around its lowered body.
    fn unknown(&mut self, node: &NodeRef<'_, Flm>, next: Option<&NodeRef<'_, Flm>>) {
        let Some(payload) = payload_of::<UnknownPayload>(node) else {
            return self.unexpected(node, "an unattributed-callable node without its payload,");
        };
        match payload.kind {
            UnknownKind::Macro => self.control_word(&payload.name, false, next),
            UnknownKind::Environment => {
                self.sink.begin(&payload.name);
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|body| body.iter().collect()).unwrap_or_default();
                self.run(&body);
                self.sink.end(&payload.name);
            }
        }
    }

    /// A `flm.pres.unsupported` node: `% flm: unsupported element <name>` on
    /// a line of its own — a newline before it unless the previous sibling's
    /// text ends with one. The comment ends its own line, so nothing is
    /// written after it; where the document's own text resumes with a newline,
    /// that newline is left out when the text is written (see
    /// [`Emitter::node`]), so the comment line never doubles one. The decision
    /// reads the neighboring nodes, never the output.
    fn unsupported(&mut self, node: &NodeRef<'_, Flm>, siblings: &[NodeRef<'_, Flm>], index: usize) {
        let element = payload_of::<UnsupportedPayload>(node)
            .map_or_else(|| String::from("?"), |payload| payload.element.to_string());
        self.comment_line(siblings, index, &format!(" flm: unsupported element {element}"));
    }

    /// A `flm.pres.dropped` mark: `% flm: dropped <detail>` on a line of its
    /// own, written as [`Emitter::unsupported`] writes its comment: a comment,
    /// so that a document holding a mark compiles and the mark shows in its
    /// source.
    fn dropped(&mut self, node: &NodeRef<'_, Flm>, siblings: &[NodeRef<'_, Flm>], index: usize) {
        let detail = payload_of::<DroppedPayload>(node)
            .map_or_else(|| String::from("?"), |payload| payload.detail.clone());
        self.comment_line(siblings, index, &format!(" flm: dropped {detail}"));
    }

    /// A `flm.pres.subtree`: the position at which the placed tree's LaTeX is
    /// written, recorded so that the regions stage writes the placed tree there
    /// ([`LatexFragment::placed`]).
    fn subtree(&mut self, node: &NodeRef<'_, Flm>) {
        if let Some(payload) = payload_of::<SubtreePayload>(node) {
            self.placed.push((self.sink.position(), payload.tree));
        }
    }

    /// The comment `%<text>` on a line of its own: a newline before it unless
    /// the previous sibling's text ends with one or the previous sibling is
    /// such a line itself, and the comment writer's newline after it.
    fn comment_line(&mut self, siblings: &[NodeRef<'_, Flm>], index: usize, text: &str) {
        let previous = index.checked_sub(1).and_then(|i| siblings.get(i));
        if previous.is_some_and(|node| !ends_with_newline(node) && !is_comment_line(node)) {
            self.sink.newline();
        }
        self.sink.comment("%", text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_escape_config_carries_the_mode_and_the_override_spellings() {
        use crate::stdcore::backends::latex::UnicodeMode;

        let options = LatexRecomposedRenderOptions {
            unicode: UnicodeMode::Replace,
            unicode_overrides: [('𝟙', String::from("\\ensuremath{\\mathds{1}}"))]
                .into_iter()
                .collect(),
            ..LatexRecomposedRenderOptions::default()
        };
        let config = escape_config(&options);
        assert_eq!(config.mode, UnicodeMode::Replace);
        assert_eq!(config.overrides.get(&'𝟙').map(String::as_str), Some("\\ensuremath{\\mathds{1}}"));
    }

    #[test]
    fn a_block_is_split_into_one_group_per_line_and_one_per_tab_separated_segment() {
        // What the groups hold, written as the test reads them: one string per
        // segment, one list per line.
        fn groups(text: &str) -> Vec<Vec<String>> {
            verbatim_block_groups(vec![VerbatimPiece::Text(String::from(text))])
                .into_iter()
                .map(|line| {
                    line.into_iter()
                        .map(|segment| {
                            segment
                                .into_iter()
                                .map(|piece| match piece {
                                    VerbatimPiece::Text(t) | VerbatimPiece::Latex(t) => t,
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect()
        }
        // Content that is empty altogether is one empty line, which is what a
        // block written out in full writes for it.
        assert_eq!(groups(""), vec![vec![String::new()]]);
        assert_eq!(groups("one"), vec![vec![String::from("one")]]);
        assert_eq!(
            groups("one\ntwo"),
            vec![vec![String::from("one")], vec![String::from("two")]]
        );
        // A tab opens a segment, an empty one included; a tab at either end of
        // a line leaves an empty segment there.
        assert_eq!(
            groups("a\tb\t\tc"),
            vec![vec![
                String::from("a"),
                String::from("b"),
                String::new(),
                String::from("c"),
            ]]
        );
        assert_eq!(
            groups("\ta\t"),
            vec![vec![String::new(), String::from("a"), String::new()]]
        );
        // An empty line inside the content is an empty line group.
        assert_eq!(
            groups("a\n\nb"),
            vec![
                vec![String::from("a")],
                vec![String::new()],
                vec![String::from("b")],
            ]
        );
        // Content ending in a line feed ends its last line rather than opening
        // an empty one: a block written out in full writes no empty last line
        // either, and the two forms must set the same block. This is the one
        // rule that decides it.
        assert_eq!(groups("a\n"), vec![vec![String::from("a")]]);
        assert_eq!(
            groups("a\n\n"),
            vec![vec![String::from("a")], vec![String::new()]]
        );
        // A line feed alone is one empty line, not two.
        assert_eq!(groups("\n"), vec![vec![String::new()]]);
        // A raw piece — what a recomposed template's stamped point becomes —
        // joins the segment it stands in and splits nothing, its LaTeX being
        // written through rather than read as characters.
        let mixed = verbatim_block_groups(vec![
            VerbatimPiece::Text(String::from("v = ")),
            VerbatimPiece::Latex(String::from("#2")),
            VerbatimPiece::Text(String::from(";\nend")),
        ]);
        assert_eq!(mixed.len(), 2);
        assert_eq!(mixed[0].len(), 1);
        assert_eq!(mixed[0][0].len(), 3);
        assert_eq!(mixed[1].len(), 1);
    }

    #[test]
    fn an_environment_declares_one_specifier_per_argument_and_a_body_argument_last() {
        let plain = LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEnote", 2);
        assert_eq!(environment_argument_specification(&plain), "{+m +m}");
        assert_eq!(
            environment_argument_specification(&LatexDefinitionPayload::new(
                LatexDefinitionKind::Environment,
                "flmUEbare",
                0
            )),
            "{}"
        );
        // The body is the last argument, after every `+m`, and a packed
        // environment's one `+m` holds every argument, so the body is `#2`.
        assert_eq!(environment_argument_specification(&plain.clone().body_argument()), "{+m +m +b}");
        let packed = LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEbig", 12)
            .packed();
        assert_eq!(environment_argument_specification(&packed), "{+m}");
        assert_eq!(environment_argument_specification(&packed.clone().body_argument()), "{+m +b}");
    }
}
