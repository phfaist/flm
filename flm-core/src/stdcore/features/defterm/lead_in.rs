//! The lead-in of a term definition: the configured lead-in template
//! ([`DeftermConfig::lead_in`](super::DeftermConfig::lead_in)), compiled once
//! when the registry is built and written at every term definition as a
//! run-in decoration of its first paragraph; and the term pointer, the
//! synthesized node that shows the term inside the lead-in.
//!
//! # The template and the term pointer
//!
//! A build hook ([`LateInit`]) compiles the template once the language is
//! built. The registry build fails, with a message that quotes the template,
//! on a template that does not parse, that uses a placeholder other than
//! `#{term}`, that writes it with keys in brackets (`#[…]{term}`), that uses
//! it more than once, or that is blank.
//!
//! At a term definition, the term definition's rule instantiates the
//! template. Its `#{term}` is replaced by one node of the element
//! [`TERM_POINTER`], whose payload is a [`ContentPointer`] to the term
//! definition's node and the region of its term argument; the rest of
//! the template is ordinary content. The instance is lowered where the
//! lead-in is placed, and the pointer's rule ([`PointerRule`]) lowers the
//! term there, through [`lower_content_pointer`]: the term is shown where it
//! was written, as the document's own content, and never copied. The term is
//! lowered there and nowhere else, because the term definition's rule lowers
//! its term argument through the lead-in alone; a template therefore may use
//! `#{term}` at most once.
//!
//! The pointer is checked where it is lowered: a pointer to anything but a
//! term definition's term argument is reported as [`PointerRefused`].
//!
//! The element is synthesized: no source text produces it, so an author
//! cannot write it.

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
    CompiledTemplate, PlaceholderVocabulary, TemplateLanguage,
};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics};
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_value};
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::{
    ContentPointer, lower_content_pointer, lower_region_content,
};
use crate::stdcore::backends::pres::{self, DecorationPayload, SpanPayload};
use crate::stdcore::features::numbering::{
    Binding, Bindings, StyleError, compile_text, instantiate_template, occurrences_of,
};

use super::diagnostics::PointerRefused;
use super::elements::{DeftermElement, TERM_ARG};

/// The only placeholder name of the lead-in template, `term`: `#{term}` is
/// the term being defined, as the term definition writes it.
pub const TERM_PLACEHOLDER: &str = "term";

/// The element of the node that replaces `#{term}` in an instantiated
/// lead-in, `flm.defterm.term-pointer`: synthesized, never written in a
/// document; its payload is a [`ContentPointer`] to the term ([`PointerRule`]).
pub const TERM_POINTER: &str = "flm.defterm.term-pointer";

/// The name of the general lowering rule of [`TERM_POINTER`],
/// `flm.defterm.term-pointer.lower.default` ([`PointerRule`]), the default on
/// every backend.
pub const POINTER_RULE: &str = "flm.defterm.term-pointer.lower.default";

/// The role of the span that holds a term definition's lead-in inside its
/// run-in decoration, `flm.defterm.leadin`.
pub const LEAD_IN_ROLE: &str = "flm.defterm.leadin";

/// The name of the body slot of a pointer node, which holds nothing.
const BODY_SLOT: &str = "body";

/// The element value of [`TERM_POINTER`]: inline content, with no aspect but
/// its block role.
pub(super) fn pointer_element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(TERM_POINTER)
            .docs(
                "The term of a term definition, shown where a lead-in template places \
                 `#{term}`; synthesized, never written in a document.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT))),
    )
}

// --- the template ----------------------------------------------------------------------------

/// What the feature shares between its registration, its build hook and the
/// term definition's rules: the lead-in as configured, the element of the
/// pointer node, and the compiled template once the hook has run.
pub(super) struct LeadIn {
    declared: Option<String>,
    spec: Arc<FlmSynthesizedSpec>,
    compiled: OnceLock<CompiledTemplate>,
}

impl fmt::Debug for LeadIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeadIn").field("declared", &self.declared).finish_non_exhaustive()
    }
}

impl LeadIn {
    /// The lead-in `declared` — the template, or `None` for no lead-in —
    /// with the pointer node carrying `element`.
    pub(super) fn new(declared: Option<String>, element: Arc<ElementSpec>) -> LeadIn {
        LeadIn {
            declared,
            spec: Arc::new(FlmSynthesizedSpec::new(element)),
            compiled: OnceLock::new(),
        }
    }

    /// Whether a lead-in is configured at all.
    pub(super) fn is_set(&self) -> bool {
        self.declared.is_some()
    }
}

/// Compiles `text`, the configured lead-in.
///
/// # Errors
///
/// A sentence that quotes the template and says what is wrong with it.
fn compile_lead_in(
    text: &str,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledTemplate, String> {
    let refused = |detail: String| {
        format!("the option `lead_in` of the feature `defterm` is set to `{text}`, which {detail}")
    };
    if text.trim().is_empty() {
        return Err(refused(String::from(
            "is blank; leave `lead_in` unset to give term definitions no lead-in",
        )));
    }
    let vocabulary = PlaceholderVocabulary::new(0).extra(TERM_PLACEHOLDER, false);
    let template = compile_text(text, "the defterm feature's lead-in", &vocabulary, lang, language)
        .map_err(|error| refused(format!("is not valid FLM text: {}", describe(&error))))?;
    if occurrences_of(&template).len() > 1 {
        return Err(refused(format!(
            "uses `#{{{TERM_PLACEHOLDER}}}` more than once, and it may use it at most once"
        )));
    }
    Ok(template)
}

/// What `error` reports, as one sentence: the diagnostics of a template that
/// did not compile, one after the other.
fn describe(error: &StyleError) -> String {
    match error {
        StyleError::Template { diagnostics, .. } | StyleError::Parse { diagnostics, .. } => {
            diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        }
        other => other.to_string(),
    }
}

/// The feature's build hook ([`LateInit`]): it compiles the configured
/// lead-in once the language is built. A template that does not compile
/// fails the registry build: it is configuration, and a mistake in it is
/// reported once rather than at every term definition. Nothing is compiled
/// where no lead-in is configured.
pub(super) struct CompileLeadIn {
    pub(super) lead_in: Arc<LeadIn>,
}

impl LateInit for CompileLeadIn {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let Some(text) = &self.lead_in.declared else { return Ok(()) };
        let language = TemplateLanguage::new(lang).map_err(|errors| {
            LateInitError::new(format!(
                "the template language of the defterm feature: {}",
                errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ))
        })?;
        let template = compile_lead_in(text, lang, &language).map_err(LateInitError::new)?;
        // The hook runs once per registration of the feature, and a registry
        // holds the feature once, so the cell is empty here.
        let _ = self.lead_in.compiled.set(template);
        Ok(())
    }
}

// --- writing ---------------------------------------------------------------------------------

/// The lead-in's instance at the term definition `node`, the node being
/// lowered: the compiled template with its `#{term}` replaced by a term
/// pointer to the term definition's term argument, as a synthesized value
/// ready to be spliced ([`Lowerer::splice`]); `None` where no lead-in is
/// configured.
///
/// Every backend instantiates the lead-in through this function:
/// [`stage_lead_in`] lowers the instance where a term definition's first
/// paragraph opens, and the recomposed LaTeX lowers it as the begin code of
/// the environment's LaTeX definition, where the pointer's rule on that
/// backend writes the environment's parameter ([`latex`](super::latex)).
///
/// # Errors
///
/// [`LowerError`] when the template was not compiled — the hook did not run —
/// or when the instance could not be built.
pub(super) fn lead_in_instance(
    lead_in: &LeadIn,
    node: &NodeRef<'_, Flm>,
    lw: &dyn Lowerer,
) -> Result<Option<Arc<NodeTree<Flm>>>, LowerError> {
    if !lead_in.is_set() {
        return Ok(None);
    }
    let template = lead_in
        .compiled
        .get()
        .ok_or_else(|| LowerError::other("the lead-in of the defterm feature was not compiled"))?;
    let region = NodeRegion::argument_named(node, TERM_ARG)
        .ok_or_else(|| LowerError::structure("a definition without its term argument"))?;
    let pointer = SynthesizedPiece {
        spec: Arc::clone(&lead_in.spec),
        slot: BODY_SLOT,
        span: node.span().clone(),
        state: Arc::clone(node.parsing_state()),
        body: Vec::new(),
        payload: Some(Arc::new(ContentPointer { target: lw.addr(), region })),
    };
    let term = copy_pieces_into_value(&[ValuePiece::Synthesized(&pointer)], node, ValueContent::Own, GuardPolicy::StackBudget)
        .map_err(|error| LowerError::other(error.to_string()))?;
    let bindings = Bindings::new(vec![Binding {
        name: TERM_PLACEHOLDER.to_string(),
        keys: String::new(),
        content: term,
    }]);
    instantiate_template(node.span(), template, &bindings)
        .map(Some)
        .map_err(|error| LowerError::other(error.to_string()))
}

/// The lead-in of the term definition `node`, the node being lowered: its
/// instance ([`lead_in_instance`]) lowered where the lead-in is placed, in a
/// span of role [`LEAD_IN_ROLE`] inside a
/// [`flm.pres.deco.runin`](crate::stdcore::backends::pres::DECO_RUNIN)
/// decoration; nothing where no lead-in is configured.
///
/// The calling rule places the decoration at the start of the term
/// definition's first paragraph, and must declare that it produces
/// `flm.pres.span` and `flm.pres.deco.runin`.
///
/// # Errors
///
/// [`LowerError`] when the instance could not be built, and from its
/// lowering.
pub(super) fn stage_lead_in(
    lead_in: &LeadIn,
    node: &NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(instance) = lead_in_instance(lead_in, node, lw)? else {
        return Ok(Vec::new());
    };
    let content = lw.splice(cx, &instance)?;
    let payload = SpanPayload { role: Some(Name::from(LEAD_IN_ROLE)), ..SpanPayload::default() };
    let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, content)?;
    let deco = lw.pres_with(cx, &ElementName::from(pres::DECO_RUNIN), DecorationPayload, vec![span])?;
    Ok(vec![deco])
}

// --- the pointer's rule ----------------------------------------------------------------------

/// The general rule of the element [`TERM_POINTER`], [`POINTER_RULE`]: the
/// default on every backend.
pub(super) fn pointer_rule() -> LoweringRule {
    LoweringRule::new(
        POINTER_RULE,
        TERM_POINTER,
        LoweringDecl::new(),
        Arc::new(PointerRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "a term pointer lowers to the term of the term definition it refers to, lowered where \
         the pointer is placed",
    )
}

/// The lowering rule [`POINTER_RULE`], the default on every backend: the
/// term the pointer refers to ([`ContentPointer`]), lowered where the
/// pointer is placed, as it is written in the term definition, through
/// [`lower_content_pointer`].
///
/// A pointer to anything but a term definition's term argument is reported
/// as [`PointerRefused`] and writes nothing. A pointer to a node the lowering
/// cannot reach, or to a region the node does not have, is reported at the
/// pointer as the lowering stage's `flm.lowering.region-not-found` and writes
/// nothing; the lead-in around it and the term definition are still
/// written.
#[derive(Debug, Default, Clone, Copy)]
pub struct PointerRule;

impl Lower for PointerRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let span = node.span().clone();
        let mut check = |lw: &mut dyn Lowerer, named: &NodeRef<'_, Flm>, region: NodeRegion| {
            if element_value::<DeftermElement>(named).is_some()
                && NodeRegion::argument_named(named, TERM_ARG) == Some(region)
            {
                return Ok(true);
            }
            let element = named.ext().element.name.to_string();
            lw.error(PointerRefused { element, region: region.to_string() }, span.clone());
            Ok(false)
        };
        let staged = lower_content_pointer(cx, lw, &node, &mut check, &mut lower_region_content)?;
        Ok(Lowering::Emit(staged))
    }
}
