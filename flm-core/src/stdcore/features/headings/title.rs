//! The heading title template: what is written before the title of a numbered
//! heading, such as `1.2. ` in `1.2. Notation`. Each heading kind has one, its
//! [`title_number_format`](super::HeadingDecl::title_number_format); it is written in
//! the template language, compiled once when the registry is built, and instantiated
//! at every numbered heading.
//!
//! # The template
//!
//! The template states how a heading's number joins its title: the number and the
//! text around it, `#{number}. ` by default, which writes `1. ` before
//! `Introduction`. It may hold a word (`Chapter~#{number}: `) or show the number in
//! another form (`#[bare]{number} `). It is written in the template language, FLM text
//! with placeholders (see [the template language shared by
//! features](crate::guide::concepts_overview#the-template-language-shared-by-features)),
//! and its only placeholder name is [`NUMBER_PLACEHOLDER`], `number`:
//!
//! - `#{number}` is the heading's number as the kind's counter style writes it,
//!   without the word ([`WORD`](super::numbers::WORD), `§`) and the delimiters a
//!   reference writes: `1.2` for a subsection, the numbers of its parent counters
//!   joined as the counter style joins them.
//! - `#[bare]{number}` is the kind's own number without the numbers of the counters
//!   it is numbered within: `2` where `#{number}` writes `1.2`.
//! - `#[Roman]{number}` is the kind's own number in another number format (any number
//!   format name, [`NumberFormat`](crate::stdcore::features::numbering::NumberFormat)),
//!   with the numbers of the parent counters written as usual: `1.II`. The two keys can be combined: `#[Roman,bare]{number}`.
//!
//! Everything else in the template is FLM content, and markup may enclose the
//! placeholder. A template without any placeholder is allowed: the heading then shows
//! its text and nothing of its number, and still counts.
//!
//! The template carries structure, not styling: how the number looks is decided by a
//! program's lowering rule or by a stylesheet, through the role [`NUMBER_ROLE`] the
//! number carries.
//!
//! # How the template is compiled and written
//!
//! A late-initialization build hook
//! ([`LateInit`]) compiles every kind's template once
//! the language is built. A template that does not parse, that uses a placeholder
//! other than `number`, or that gives `number` a key it does not take, fails the
//! registry build with an error that gives the heading kind.
//!
//! At each heading the document numbered, the lowering rule of the heading
//! ([`HeadingRule`](super::HeadingRule)) instantiates the kind's template. Each
//! `#{number}` is replaced by a node of the element [`NUMBER_ELEMENT`] holding the
//! number, and that node lowers to a
//! [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) of role [`NUMBER_ROLE`]
//! around the number alone ([`NumberRule`]); the rest of the template is ordinary
//! heading content before the title. The element is *synthesized*: it has no source
//! form, so no document can write it.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use techy::core::node::{
    BodySlotExt, BuildId, ChildRegion, ContentNodes, NodeKind, NodeRef, NodeTree, ParsedSlot,
    ParsedSlots, SlotRole,
};
use techy::core::Lang;
use techy::latexlike::BodyMarker;
use techy::source::SourceSpan;
use techy::transform::{Restage, RestageContext, RestageVisitor};

use crate::core::lowering::{Lower, LowerCx, LowerError, Lowerer, Lowering};
use crate::core::registry::{LateInit, LateInitError};
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::rewrites::userdefs::template::{
    CompiledTemplate, PlaceholderVocabulary, TemplateLanguage,
};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::pres::{self, SpanPayload};
use crate::stdcore::features::numbering::{
    compile_text, instantiate_template, occurrences_of, parse_keys, AncestorKeys, Binding,
    Bindings, NumberSegment, StyleError,
};

use super::numbers::NUMBER_ROLE;

/// The only placeholder name of a heading title template, `number`: `#{number}`
/// is the heading's number.
pub const NUMBER_PLACEHOLDER: &str = "number";

/// The heading title template every heading kind has by default, `#{number}. `:
/// the number, a period and a space before the title, as in `1. Introduction`.
pub const DEFAULT_TITLE_NUMBER_FORMAT: &str = "#{number}. ";

/// The element of the node that holds a heading's number inside the
/// instantiated heading title template, `flm.heading.number`.
///
/// The element is synthesized (it has no source form, so no document can write
/// it). Its lowering rule, [`NUMBER_RULE`], lowers it to a span of role
/// [`NUMBER_ROLE`] around the number.
pub const NUMBER_ELEMENT: &str = "flm.heading.number";

/// The name of the default lowering rule of [`NUMBER_ELEMENT`],
/// `flm.heading.number.lower.default` ([`NumberRule`]). On the `latex-recomposed`
/// backend the element has its own rule
/// ([`NUMBER_LATEX_RULE`](super::latex::NUMBER_LATEX_RULE)).
pub const NUMBER_RULE: &str = "flm.heading.number.lower.default";

/// The name of the body slot the number node holds the number in.
const BODY_SLOT: &str = "body";

/// One `#{number}` of a compiled template: the text of its bracket as written
/// (`written`), which matches the placeholder to its binding when the template is
/// instantiated, and the keys read from that text (`keys`).
struct Occurrence {
    written: String,
    keys: AncestorKeys,
}

/// One kind's heading title template, compiled.
struct CompiledTitle {
    template: CompiledTemplate,
    /// Every distinct bracket text the template gives `#{number}`, in the
    /// order first written.
    occurrences: Vec<Occurrence>,
}

/// What the headings feature shares between its registration, its
/// late-initialization build hook and its lowering rule: each kind's heading
/// title template as declared, the element of the number node, and the compiled
/// templates once the hook has run.
pub(super) struct Titles {
    declared: Vec<(Name, String)>,
    spec: Arc<FlmSynthesizedSpec>,
    compiled: OnceLock<BTreeMap<Name, CompiledTitle>>,
}

impl fmt::Debug for Titles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Titles").field("declared", &self.declared).finish_non_exhaustive()
    }
}

impl Titles {
    /// Returns the shared state for `declared` (each kind's name and heading
    /// title template), with the number node carrying `element`.
    pub(super) fn new(declared: Vec<(Name, String)>, element: Arc<ElementSpec>) -> Titles {
        Titles {
            declared,
            spec: Arc::new(FlmSynthesizedSpec::new(element)),
            compiled: OnceLock::new(),
        }
    }
}

/// Returns the element value of [`NUMBER_ELEMENT`].
pub(super) fn number_element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(NUMBER_ELEMENT)
            .docs(
                "The number of a numbered heading, where the kind's heading title template \
                 writes `#{number}`. No document can write it.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

// --- compiling -------------------------------------------------------------------------------

/// Compiles `text`, the heading title template of the kind `kind`.
///
/// # Errors
///
/// A sentence that gives the kind and what is wrong: a text that is not valid
/// FLM, a placeholder other than `number`, or a key `#[…]{number}` does not take.
fn compile_title(
    kind: &Name,
    text: &str,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledTitle, String> {
    let refused = |detail: String| {
        format!("the heading title template `{text}` of the heading kind `{kind}` is not valid: {detail}")
    };
    let vocabulary = PlaceholderVocabulary::new(0).extra(NUMBER_PLACEHOLDER, true);
    let what = format!("the heading kind `{kind}`");
    let template = compile_text(text, &what, &vocabulary, lang, language)
        .map_err(|error| refused(describe(&error)))?;
    let mut occurrences: Vec<Occurrence> = Vec::new();
    for (_, written) in occurrences_of(&template) {
        let keys = parse_keys(&written).map_err(|key| {
            refused(format!(
                "`#[…]{{{NUMBER_PLACEHOLDER}}}` takes the key `bare` and number format names, \
                 not `{key}`"
            ))
        })?;
        if !occurrences.iter().any(|occurrence| occurrence.written == written) {
            occurrences.push(Occurrence { written, keys });
        }
    }
    Ok(CompiledTitle { template, occurrences })
}

/// Returns what `error` reports, as a sentence: the diagnostics of a template
/// that did not compile, one after the other.
fn describe(error: &StyleError) -> String {
    match error {
        StyleError::Template { diagnostics, .. } | StyleError::Parse { diagnostics, .. } => {
            diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        }
        other => other.to_string(),
    }
}

/// The late-initialization build hook of the headings feature: it compiles every
/// kind's heading title template once the language is built.
///
/// A template that does not compile fails the registry build with an error that
/// gives the kind: the template is configuration, so a mistake in it is reported
/// once rather than at every numbered heading. Every kind's template is compiled,
/// whether the kind is numbered or not.
pub(super) struct CompileTitles {
    pub(super) titles: Arc<Titles>,
}

impl LateInit for CompileTitles {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "the template language of the headings feature: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let mut compiled = BTreeMap::new();
        for (kind, text) in &self.titles.declared {
            let title = compile_title(kind, text, lang, &language).map_err(LateInitError::new)?;
            compiled.insert(kind.clone(), title);
        }
        // The hook runs once per registration of the feature, and a registry
        // holds the feature once, so the cell is empty here.
        let _ = self.titles.compiled.set(compiled);
        Ok(())
    }
}

// --- writing ---------------------------------------------------------------------------------

/// Returns what is written before the title of a heading of kind `kind`: the
/// kind's heading title template with each `#{number}` replaced by what `number`
/// writes for the keys of that placeholder, as a synthesized value ready to be
/// spliced ([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)). `None`
/// when `number` has no number for the heading.
///
/// # Errors
///
/// [`LowerError`] when the template was not compiled (the build hook did not
/// run), when the number could not be written, or when the content could not be
/// built.
pub(super) fn title_number(
    titles: &Titles,
    kind: &Name,
    mut number: impl FnMut(AncestorKeys) -> Option<Result<Vec<NumberSegment>, String>>,
) -> Result<Option<Arc<NodeTree<Flm>>>, LowerError> {
    let title = titles.compiled.get().and_then(|compiled| compiled.get(kind)).ok_or_else(|| {
        LowerError::other(format!(
            "the heading title template of the heading kind `{kind}` was not compiled"
        ))
    })?;
    let mut entries = Vec::with_capacity(title.occurrences.len());
    for occurrence in &title.occurrences {
        let Some(segments) = number(occurrence.keys) else { return Ok(None) };
        let segments = segments.map_err(LowerError::other)?;
        entries.push(Binding {
            name: NUMBER_PLACEHOLDER.to_string(),
            keys: occurrence.written.clone(),
            content: number_node(&titles.spec, title.template.tree(), &segments)?,
        });
    }
    let site = title.template.tree().root().span().clone();
    instantiate_template(&site, &title.template, &Bindings::new(entries))
        .map(Some)
        .map_err(|error| LowerError::other(error.to_string()))
}

/// Returns a value tree holding one node of [`NUMBER_ELEMENT`] whose body is the
/// content of `segments`, with a source span at the start of `anchor`, the
/// compiled template the node is substituted into.
fn number_node(
    spec: &Arc<FlmSynthesizedSpec>,
    anchor: &Arc<NodeTree<Flm>>,
    segments: &[NumberSegment],
) -> Result<Arc<NodeTree<Flm>>, LowerError> {
    let mut visitor = NumberNode { spec, segments };
    guard::restager(&mut visitor, GuardPolicy::StackBudget)
        .restage(anchor)
        .map(Arc::new)
        .map_err(|error| LowerError::other(error.to_string()))
}

/// The visitor [`number_node`] runs: called once, at the anchor tree's root,
/// it emits a list node holding the number node over a copy of every
/// segment's nodes.
struct NumberNode<'a> {
    spec: &'a Arc<FlmSynthesizedSpec>,
    segments: &'a [NumberSegment],
}

/// A visitor that carries every node over unchanged.
struct Verbatim;

impl RestageVisitor<Flm, (), ()> for Verbatim {
    type Error = String;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, String> {
        Ok(Restage::Descend(()))
    }
}

impl RestageVisitor<Flm, (), ()> for NumberNode<'_> {
    type Error = String;

    fn restage(
        &mut self,
        root: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, String> {
        let mut children: Vec<BuildId> = Vec::new();
        for segment in self.segments {
            for node in segment.content().root().children().iter() {
                children.extend(
                    cx.restage_subtree(node, &mut Verbatim).map_err(|error| error.to_string())?,
                );
            }
        }
        let span = SourceSpan::new(root.span().source(), root.span().start()..root.span().start());
        let state = Arc::clone(root.parsing_state());
        let count = children.len() as u32;
        let mut data = self.spec.callable_data();
        data.slots = ParsedSlots::new(vec![ParsedSlot::new(
            ChildRegion::new(0..count, ContentNodes::InRegion(0..count)),
            BODY_SLOT,
            SlotRole::Content,
            BodyMarker::make_body(),
        )]);
        let kind = NodeKind::callable(data);
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&children))
            .map_err(|error| error.to_string())?;
        let number = cx
            .builder()
            .add(kind, span.clone(), Arc::clone(&state), children, ext, ())
            .map_err(|error| error.to_string())?;
        let kind = NodeKind::list();
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&[number]))
            .map_err(|error| error.to_string())?;
        let list = cx
            .builder()
            .add(kind, span, state, vec![number], ext, ())
            .map_err(|error| error.to_string())?;
        Ok(Restage::Emit(vec![list]))
    }
}

// --- lowering the number node ------------------------------------------------------------------

/// The implementation of the lowering rule [`NUMBER_RULE`]: a node of
/// [`NUMBER_ELEMENT`] lowers to a
/// [`flm.pres.span`](crate::stdcore::backends::pres::SPAN) of role
/// [`NUMBER_ROLE`] around its body, the number, lowered through the lowering
/// rules.
#[derive(Debug, Default, Clone, Copy)]
pub struct NumberRule;

impl Lower for NumberRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let body = lw.lower_body(cx, &node)?;
        let payload = SpanPayload {
            style: None,
            role: Some(Name::from(NUMBER_ROLE)),
            kinds: Vec::new(),
            anchor: None,
            doc_role: None,
        };
        let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?;
        Ok(Lowering::Emit(vec![span]))
    }
}
