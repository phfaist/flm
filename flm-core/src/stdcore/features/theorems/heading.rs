//! The heading and the closing mark of a theorem: the compilation of every
//! class's heading template, every class's closing mark and every kind's word
//! when the registry is built; the instantiation of a heading at a theorem;
//! and the three synthesized elements that an instantiated heading holds,
//! with their general lowering rules.
//!
//! # What is compiled
//!
//! A build hook ([`LateInit`]) compiles, once the language is built:
//!
//! - every class's [theorem heading
//!   template](super::TheoremClass::heading), with the two placeholders
//!   `#{name}` and `#{title}`;
//! - every class's [closing mark](super::TheoremClass::closing_mark), as FLM
//!   markup;
//! - every kind's base singular word, as FLM markup: the word a heading shows
//!   alone where the theorem has no number;
//! - every kind's words as a reference to it shows them, for the recomposed
//!   LaTeX ([`latex`](super::latex)).
//!
//! The registry build fails, with a message that identifies the class or the
//! kind, on a template that does not compile, uses a placeholder other than
//! these two, writes a placeholder with keys in brackets (`#[…]{title}`), or
//! uses `#{title}` more than once; and on a mark or a word that does not
//! parse. `#{title}` written as the test of a conditional
//! (`\IfValueT{#{title}}{…}`) shows nothing and is not counted.
//!
//! # The heading at a theorem
//!
//! The theorem's general rule ([`rules`](super::rules)) instantiates the
//! class's template at each theorem. Each `#{name}` is replaced by one node of
//! the element [`NAME_ELEMENT`], which holds the kind's word and the theorem's
//! number (`Lemma~4`, the number as the numbering feature writes it in the
//! kind's counter style), or the word alone where the document did not number
//! the theorem; its rule writes a span of role [`NAME_ROLE`] around it.
//! `#{title}` is replaced by one of two nodes:
//!
//! - for a title, a node of the element [`TITLE_POINTER`], whose payload is a
//!   [`ContentPointer`] to the theorem's title argument. Its rule
//!   lowers the title where the heading shows it, through
//!   [`lower_content_pointer`], so the title is shown as the document's own
//!   content and never copied;
//! - for a back-reference that a proof shows, a node of the element
//!   [`RELATION`], whose payload is a content pointer to the same title
//!   argument. Its rule writes what a reference to the statement writes (the
//!   statement's word and number, linked to it) through the references
//!   feature's
//!   [`lower_reference`](crate::stdcore::features::refs::lower_reference),
//!   from the resolution of the theorem's reference site.
//!
//! Either rule writes a span of role [`TITLE_ROLE`] around what it lowers. A
//! theorem with no title, or whose back-reference is not shown, binds nothing
//! to `#{title}`. The three elements are synthesized: no source text
//! produces them, so an author cannot write them. The roles are names that a
//! stylesheet or a host's lowering rule selects on (see
//! [roles](crate::stdcore::backends::pres::payload)).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use techy::core::node::{BuildId, NodeRef, NodeTree};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    NodeRegion,
};
use crate::core::registry::{LateInit, LateInitError};
use crate::lang::copy::{SynthesizedPiece, ValueContent, ValuePiece, copy_pieces_into_value};
use crate::lang::guard::GuardPolicy;
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::rewrites::userdefs::template::{
    CONDITIONAL_ELEMENT, CompiledTemplate, PlaceholderTarget, PlaceholderVocabulary,
    TemplateLanguage,
};
use crate::lang::{BuiltLang, Flm, FlmCallableType, FlmDiagnostics};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::{
    ContentPointer, lower_content_pointer, lower_region_content,
};
use crate::stdcore::backends::pres::{self, SpanPayload};
use crate::stdcore::features::numbering::{
    Binding, Bindings, COUNTERS, NumberSegment, StyleError, compile_text, instantiate_fixed,
    instantiate_template,
};
use crate::stdcore::features::refs::{self, REF_RESOLUTIONS, RefTargetText};

use super::config::TheoremsConfig;
use super::diagnostics::PointerRefused;
use super::elements::{TITLE_ARG, TheoremElement, TitleReading, relation_site, title_reading};

/// The placeholder of a theorem heading template for the kind's word and
/// the theorem's number, `name`, written `#{name}`.
pub const NAME_PLACEHOLDER: &str = "name";

/// The placeholder of a theorem heading template for the theorem's title, or
/// the statement a proof refers to, `title`, written `#{title}`.
pub const TITLE_PLACEHOLDER: &str = "title";

/// The element of the node that replaces `#{name}` in an instantiated
/// heading, `flm.theorems.name`: synthesized, its body the kind's word and
/// the theorem's number. It is an element, rather than content inserted into
/// the heading, so that its rule can stage (build into the lowered tree; see
/// [`Lowerer`]) the span of role [`NAME_ROLE`] around the name: a
/// presentation span can only be built at lowering.
pub const NAME_ELEMENT: &str = "flm.theorems.name";

/// The element of the node that replaces `#{title}` where the theorem has a
/// title, `flm.theorems.title-pointer`: synthesized, its payload a
/// [`ContentPointer`] to the theorem's title argument ([`TitlePointerRule`]).
pub const TITLE_POINTER: &str = "flm.theorems.title-pointer";

/// The element of the node that replaces `#{title}` where a proof shows its
/// back-reference, `flm.theorems.relation`: synthesized, its payload a
/// [`ContentPointer`] to the proof's title argument, in which the
/// back-reference is written ([`RelationRule`]).
pub const RELATION: &str = "flm.theorems.relation";

/// The role of the span around the kind's word and the theorem's number in a
/// heading, `flm.theorems.name` (written by [`NameRule`]).
pub const NAME_ROLE: &str = "flm.theorems.name";

/// The role of the span around the title, or the back-reference, in a
/// heading, `flm.theorems.title` (written by [`TitlePointerRule`] and
/// [`RelationRule`]).
pub const TITLE_ROLE: &str = "flm.theorems.title";

/// The name of the general lowering rule of [`NAME_ELEMENT`],
/// `flm.theorems.name.lower.default` ([`NameRule`]).
pub const NAME_RULE: &str = "flm.theorems.name.lower.default";

/// The name of the general lowering rule of [`TITLE_POINTER`],
/// `flm.theorems.title-pointer.lower.default` ([`TitlePointerRule`]).
pub const TITLE_POINTER_RULE: &str = "flm.theorems.title-pointer.lower.default";

/// The name of the general lowering rule of [`RELATION`],
/// `flm.theorems.relation.lower.default` ([`RelationRule`]).
pub const RELATION_RULE: &str = "flm.theorems.relation.lower.default";

/// The name of the body slot of a synthesized node.
const BODY_SLOT: &str = "body";

/// A synthesized, inline element named `name`.
fn synthesized_element(name: &str, docs: &str) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(name)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

/// The elements of the three synthesized nodes, in the order [`NAME_ELEMENT`],
/// [`TITLE_POINTER`], [`RELATION`].
pub(super) fn synthesized_elements() -> [Arc<ElementSpec>; 3] {
    [
        synthesized_element(
            NAME_ELEMENT,
            "The kind's word and the theorem's number in a theorem's heading; synthesized, \
             never written in a document.",
        ),
        synthesized_element(
            TITLE_POINTER,
            "The title of a theorem, shown where its heading template places `#{title}`; \
             synthesized, never written in a document.",
        ),
        synthesized_element(
            RELATION,
            "The statement a proof refers to, shown where its heading template places \
             `#{title}`; synthesized, never written in a document.",
        ),
    ]
}

// --- what is compiled ------------------------------------------------------------------------

/// What the late-initialization hook compiles.
struct Compiled {
    /// Each class's heading template, by class name.
    headings: BTreeMap<Name, CompiledTemplate>,
    /// Each class's closing mark, by class name, where it has one.
    marks: BTreeMap<Name, Arc<NodeTree<Flm>>>,
    /// Each kind's base singular word, by kind name.
    words: BTreeMap<Name, Arc<NodeTree<Flm>>>,
    /// Each kind's words as a reference to it shows them, by kind name:
    /// [`ReferenceWords`].
    reference_words: BTreeMap<Name, ReferenceWords>,
}

/// The words that a reference to a theorem of one kind shows, as the kind's
/// `zref-clever` reference type declares them for the recomposed LaTeX: each
/// key of the type with its word, compiled as FLM markup, as
/// [`refs::reference_type_words`] maps the kind's words to the keys.
pub(super) type ReferenceWords = Vec<(&'static str, Arc<NodeTree<Flm>>)>;

/// What the feature shares between its registration, its late-initialization
/// hook and the theorem's rule: the configuration, the specifications of the
/// three synthesized nodes, and what the hook compiled once it has run.
pub(super) struct Headings {
    config: TheoremsConfig,
    name: Arc<FlmSynthesizedSpec>,
    pointer: Arc<FlmSynthesizedSpec>,
    relation: Arc<FlmSynthesizedSpec>,
    compiled: OnceLock<Compiled>,
}

impl fmt::Debug for Headings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Headings").field("config", &self.config).finish_non_exhaustive()
    }
}

impl Headings {
    /// The headings of `config`, their synthesized nodes carrying `elements`
    /// ([`synthesized_elements`]).
    pub(super) fn new(config: TheoremsConfig, elements: &[Arc<ElementSpec>; 3]) -> Headings {
        let [name, pointer, relation] = elements;
        Headings {
            config,
            name: Arc::new(FlmSynthesizedSpec::new(Arc::clone(name))),
            pointer: Arc::new(FlmSynthesizedSpec::new(Arc::clone(pointer))),
            relation: Arc::new(FlmSynthesizedSpec::new(Arc::clone(relation))),
            compiled: OnceLock::new(),
        }
    }

    /// What the hook compiled; an error where it has not run.
    fn compiled(&self) -> Result<&Compiled, LowerError> {
        self.compiled
            .get()
            .ok_or_else(|| LowerError::other("the headings of the theorems feature were not compiled"))
    }

    /// The closing mark of the class `class`, where it has one.
    pub(super) fn mark(&self, class: &Name) -> Result<Option<Arc<NodeTree<Flm>>>, LowerError> {
        Ok(self.compiled()?.marks.get(class).cloned())
    }

    /// The base singular word of the kind `kind`, compiled as FLM content:
    /// the word a heading shows alone where the theorem has no number, and
    /// the display name the recomposed LaTeX gives the kind's environment in
    /// its declaration (`Lemma` in `\newtheorem{lemma}{Lemma}`).
    pub(super) fn word(&self, kind: &Name) -> Result<Arc<NodeTree<Flm>>, LowerError> {
        self.compiled()?.words.get(kind).cloned().ok_or_else(|| {
            LowerError::other(format!("the word of the theorem kind `{kind}` was not compiled"))
        })
    }

    /// The words that a reference to a theorem of the kind `kind` shows.
    pub(super) fn reference_words(&self, kind: &Name) -> Result<ReferenceWords, LowerError> {
        self.compiled()?.reference_words.get(kind).cloned().ok_or_else(|| {
            LowerError::other(format!("the words of the theorem kind `{kind}` were not compiled"))
        })
    }

    /// Whether any class of the configuration has a closing mark.
    pub(super) fn any_mark(&self) -> bool {
        self.config.classes.values().any(|class| class.closing_mark.is_some())
    }
}

/// What `error` reports, as one sentence: the diagnostics of a text that did
/// not compile, one after the other.
fn describe(error: &StyleError) -> String {
    match error {
        StyleError::Template { diagnostics, .. } | StyleError::Parse { diagnostics, .. } => {
            diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        }
        other => other.to_string(),
    }
}

/// Whether the placeholder node `node` is the test of a conditional
/// (`\IfValueT{#{title}}{…}`): the first argument of the nearest callable
/// enclosing it, which is a conditional of the template language.
fn is_conditional_test(node: &NodeRef<'_, Flm>) -> bool {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if ancestor.callable_type().is_some() {
            if ancestor.ext().element.name.as_str() != CONDITIONAL_ELEMENT {
                return false;
            }
            return ancestor.argument_content_nodes(0).is_some_and(|test| {
                test.iter().any(|top| {
                    top.id() == node.id() || top.descendants().any(|inner| inner.id() == node.id())
                })
            });
        }
        current = ancestor.parent();
    }
    false
}

/// How many times the compiled `template` shows `#{title}`: its occurrences
/// other than the test of a conditional.
fn titles_shown(template: &CompiledTemplate) -> usize {
    template
        .tree()
        .root()
        .descendants()
        .filter(|node| node.callable_type() == Some(FlmCallableType::Placeholder))
        .filter(|node| {
            template.placeholder(node.id()).is_some_and(|reference| {
                reference.target == PlaceholderTarget::Extra(TITLE_PLACEHOLDER.to_string())
            })
        })
        .filter(|node| !is_conditional_test(node))
        .count()
}

/// Compiles `text`, the heading template of the class `class`.
///
/// # Errors
///
/// A sentence that identifies the class and says what is wrong with the
/// template.
fn compile_heading(
    class: &Name,
    text: &str,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledTemplate, String> {
    let refused = |detail: String| {
        format!("the heading template `{text}` of the theorem class `{class}`: {detail}")
    };
    let vocabulary = PlaceholderVocabulary::new(0)
        .extra(NAME_PLACEHOLDER, false)
        .extra(TITLE_PLACEHOLDER, false);
    let what = format!("the heading of the theorem class `{class}`");
    let template = compile_text(text, &what, &vocabulary, lang, language)
        .map_err(|error| refused(describe(&error)))?;
    if titles_shown(&template) > 1 {
        return Err(refused(format!(
            "`#{{{TITLE_PLACEHOLDER}}}` may be used at most once outside a conditional's test, \
             and the template uses it more than once"
        )));
    }
    Ok(template)
}

/// Compiles `text` as fixed content (a closing mark, a word), which a
/// message calls `what`.
///
/// # Errors
///
/// A sentence that gives `what` and says what is wrong with the text.
fn compile_fixed(
    what: &str,
    text: &str,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<Arc<NodeTree<Flm>>, String> {
    let refused = |detail: String| format!("{what} `{text}`: {detail}");
    let template = compile_text(text, what, &PlaceholderVocabulary::new(0), lang, language)
        .map_err(|error| refused(describe(&error)))?;
    instantiate_fixed(&template).map_err(refused)
}

/// The feature's build hook ([`LateInit`]): it compiles every class's
/// heading template and closing mark and every kind's words once the
/// language is built. A text that does not compile fails the registry build:
/// it is configuration, and a mistake in it is reported once rather than at
/// every theorem.
pub(super) struct CompileHeadings {
    pub(super) headings: Arc<Headings>,
}

impl LateInit for CompileHeadings {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "the template language of the theorems feature could not be built: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let config = &self.headings.config;
        let mut compiled =
            Compiled {
                headings: BTreeMap::new(),
                marks: BTreeMap::new(),
                words: BTreeMap::new(),
                reference_words: BTreeMap::new(),
            };
        for (name, class) in &config.classes {
            let heading =
                compile_heading(name, &class.heading, lang, &language).map_err(LateInitError::new)?;
            compiled.headings.insert(name.clone(), heading);
            if let Some(mark) = &class.closing_mark {
                let what = format!("the closing mark of the theorem class `{name}`");
                let mark = compile_fixed(&what, mark, lang, &language).map_err(LateInitError::new)?;
                compiled.marks.insert(name.clone(), mark);
            }
        }
        for (name, kind) in &config.kinds {
            let what = format!("the word of the theorem kind `{name}`");
            let word =
                compile_fixed(&what, &kind.words.base.one, lang, &language).map_err(LateInitError::new)?;
            let mut reference = ReferenceWords::new();
            for (key, text) in refs::reference_type_words(&kind.words) {
                reference.push((key, compile_fixed(&what, text, lang, &language).map_err(LateInitError::new)?));
            }
            compiled.words.insert(name.clone(), word);
            compiled.reference_words.insert(name.clone(), reference);
        }
        // The hook runs once per registration of the feature, and a registry
        // holds the feature once, so the cell is empty here.
        let _ = self.headings.compiled.set(compiled);
        Ok(())
    }
}

// --- the instance ----------------------------------------------------------------------------

/// What replaces `#{title}` in a theorem's heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TitleShown {
    /// Nothing: the theorem has no title, or its back-reference is not shown.
    Nothing,
    /// The theorem's title, through a [`TITLE_POINTER`].
    Title,
    /// The statement a proof refers to, through a [`RELATION`].
    Relation,
}

/// The heading of the theorem `node`, the node being lowered, of the kind
/// `value`: its class's template with `#{name}` replaced by a
/// [`NAME_ELEMENT`] node holding `number` (the number the numbering feature
/// wrote for the theorem, word included), or the kind's word where `number`
/// is `None`, and `#{title}` replaced as `title` says. The result is a
/// synthesized value, ready to be spliced where the heading is placed
/// ([`Lowerer::splice`]).
///
/// # Errors
///
/// [`LowerError`] when the templates were not compiled — the hook did not
/// run — or when the instance could not be built.
pub(super) fn heading_instance(
    headings: &Headings,
    node: &NodeRef<'_, Flm>,
    lw: &dyn Lowerer,
    value: &TheoremElement,
    number: Option<&[NumberSegment]>,
    title: TitleShown,
) -> Result<Arc<NodeTree<Flm>>, LowerError> {
    let compiled = headings.compiled()?;
    let template = compiled.headings.get(&value.class_name).ok_or_else(|| {
        LowerError::other(format!("the heading of the theorem class `{}` was not compiled", value.class_name))
    })?;
    let body: Vec<ValuePiece<'_, '_>> = match number {
        Some(segments) => segments
            .iter()
            .flat_map(|segment| segment.content().root().children().iter().map(ValuePiece::Node))
            .collect(),
        None => {
            let word = compiled.words.get(&value.name).ok_or_else(|| {
                LowerError::other(format!("the word of the theorem kind `{}` was not compiled", value.name))
            })?;
            word.root().children().iter().map(ValuePiece::Node).collect()
        }
    };
    let piece = |spec: &Arc<FlmSynthesizedSpec>, body, payload| SynthesizedPiece {
        spec: Arc::clone(spec),
        slot: BODY_SLOT,
        span: node.span().clone(),
        state: Arc::clone(node.parsing_state()),
        body,
        payload,
    };
    let value_of = |piece: &SynthesizedPiece<'_, '_>| {
        copy_pieces_into_value(&[ValuePiece::Synthesized(piece)], node, ValueContent::Own, GuardPolicy::StackBudget)
            .map_err(|error| LowerError::other(error.to_string()))
    };
    let name = value_of(&piece(&headings.name, body, None))?;
    let mut entries =
        vec![Binding { name: NAME_PLACEHOLDER.to_string(), keys: String::new(), content: name }];
    let spec = match title {
        TitleShown::Nothing => None,
        TitleShown::Title => Some(&headings.pointer),
        TitleShown::Relation => Some(&headings.relation),
    };
    // No title binds nothing: the name is declared absent, so that the
    // template's `\IfValueT{#{title}}` takes its false branch. See decision
    // rationale at [§flm-dd-dr:std-theorem-heading].
    let bindings = match spec {
        Some(spec) => {
            let region = NodeRegion::argument_named(node, TITLE_ARG)
                .ok_or_else(|| LowerError::structure("a theorem without its title argument"))?;
            let pointer: Arc<dyn crate::lang::PresPayload> =
                Arc::new(ContentPointer { target: lw.addr(), region });
            let content = value_of(&piece(spec, Vec::new(), Some(pointer)))?;
            entries.push(Binding { name: TITLE_PLACEHOLDER.to_string(), keys: String::new(), content });
            Bindings::new(entries)
        }
        None => Bindings::new(entries).absent(TITLE_PLACEHOLDER),
    };
    instantiate_template(node.span(), template, &bindings)
        .map_err(|error| LowerError::other(error.to_string()))
}

// --- the rules of the three synthesized elements ----------------------------------------------

/// Stages `body` inside a span of role `role`; nothing for an empty body.
fn span(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    role: &str,
    body: Vec<BuildId>,
) -> Result<Vec<BuildId>, LowerError> {
    if body.is_empty() {
        return Ok(Vec::new());
    }
    let payload = SpanPayload { role: Some(Name::from(role)), ..SpanPayload::default() };
    Ok(vec![lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?])
}

/// The lowering rule [`NAME_RULE`], the default on every backend: a
/// `flm.pres.span` of role [`NAME_ROLE`] around the node's body (the kind's
/// word and the theorem's number), lowered through the rules. An empty body
/// writes nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NameRule;

impl Lower for NameRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let body = lw.lower_body(cx, &node)?;
        Ok(Lowering::Emit(span(cx, lw, NAME_ROLE, body)?))
    }
}

/// The check both pointers of a heading make: the node the pointer refers to
/// is a theorem, and the region is its title argument; anything else is
/// reported as [`PointerRefused`] at the pointer `span`.
fn title_check<'s>(
    span: &'s crate::lang::FlmSpan,
) -> impl FnMut(&mut dyn Lowerer, &NodeRef<'_, Flm>, NodeRegion) -> Result<bool, LowerError> + 's {
    move |lw: &mut dyn Lowerer, named: &NodeRef<'_, Flm>, region: NodeRegion| {
        if element_value::<TheoremElement>(named).is_some()
            && NodeRegion::argument_named(named, TITLE_ARG) == Some(region)
        {
            return Ok(true);
        }
        let element = named.ext().element.name.to_string();
        lw.error(PointerRefused { element, region: region.to_string() }, span.clone());
        Ok(false)
    }
}

/// The lowering rule [`TITLE_POINTER_RULE`], the default on every backend:
/// the title the pointer refers to, lowered where the pointer is placed, as
/// it is written in the theorem, through [`lower_content_pointer`], in a
/// `flm.pres.span` of role [`TITLE_ROLE`].
///
/// A pointer to anything but a theorem's title argument is reported as
/// [`PointerRefused`] and writes nothing; a pointer to a node the lowering
/// cannot reach is reported as the lowering stage's
/// `flm.lowering.region-not-found` and writes nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct TitlePointerRule;

impl Lower for TitlePointerRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let at = node.span().clone();
        let mut check = title_check(&at);
        let staged = lower_content_pointer(cx, lw, &node, &mut check, &mut lower_region_content)?;
        Ok(Lowering::Emit(span(cx, lw, TITLE_ROLE, staged)?))
    }
}

/// The lowering rule [`RELATION_RULE`], the default on every backend: what a
/// reference to the statement a proof refers to writes (the statement's
/// word and number, linked to it), in a `flm.pres.span` of role
/// [`TITLE_ROLE`]. The rule reads the proof whose title the node's pointer
/// refers to, and writes the reference through the references feature's
/// [`lower_reference`](refs::lower_reference); the target and its display
/// options are those of the reference site the proof declares.
///
/// A back-reference that nothing resolves shows the label's text as an
/// unresolved reference shows it; the references feature has reported it.
/// Where the registry holds no references feature there is no resolution to
/// read, and the label's text is shown, linked to nothing. The pointer is
/// checked as [`TitlePointerRule`] checks it.
#[derive(Debug, Default, Clone, Copy)]
pub struct RelationRule;

impl Lower for RelationRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let at = node.span().clone();
        let mut check = title_check(&at);
        // The region is lowered with the proof as the node being lowered, so
        // the site read is the proof's own. See decision rationale at
        // [§flm-dd-dr:std-theorem-heading].
        let mut shape = |cx: &mut LowerCx<'_>,
                         lw: &mut dyn Lowerer,
                         proof: &NodeRef<'_, Flm>,
                         _content: &[NodeRef<'_, Flm>]| {
            // The target and its display are the site's, as the proof
            // declares it; the text is the label as written.
            let (Some(TitleReading::Relation { text, .. }), Some(site)) =
                (title_reading(proof), relation_site(proof))
            else {
                return Ok(Vec::new());
            };
            if lw.get(REF_RESOLUTIONS)?.is_none() {
                return Ok(vec![lw.text(cx, &text)?]);
            }
            let target = RefTargetText { text, label: Some(site.target) };
            let items = refs::lower_reference(cx, lw, proof, &[target], &site.display)?;
            Ok(items.into_iter().flatten().collect())
        };
        let staged = lower_content_pointer(cx, lw, &node, &mut check, &mut shape)?;
        Ok(Lowering::Emit(span(cx, lw, TITLE_ROLE, staged)?))
    }
}

/// The general lowering rules of the three synthesized elements, each the
/// default on every backend.
pub(super) fn rules() -> [LoweringRule; 3] {
    [
        LoweringRule::new(
            NAME_RULE,
            NAME_ELEMENT,
            LoweringDecl::new().produces(pres::SPAN),
            Arc::new(NameRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "the kind's word and number in a theorem's heading lower to `flm.pres.span` of role \
             `flm.theorems.name` around them",
        ),
        LoweringRule::new(
            TITLE_POINTER_RULE,
            TITLE_POINTER,
            LoweringDecl::new().produces(pres::SPAN),
            Arc::new(TitlePointerRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "a title pointer lowers to `flm.pres.span` of role `flm.theorems.title` around the \
             title of the theorem it refers to, lowered where the pointer is placed",
        ),
        LoweringRule::new(
            RELATION_RULE,
            RELATION,
            LoweringDecl::new()
                .reads_opt(REF_RESOLUTIONS)
                .reads_opt(COUNTERS)
                .produces(pres::SPAN)
                .produces(pres::LINK),
            Arc::new(RelationRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "a back-reference in a proof's heading lowers to `flm.pres.span` of role \
             `flm.theorems.title` around what a reference to the proved statement writes",
        ),
    ]
}
