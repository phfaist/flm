//! `ExtExpand`, `flm-core`'s expansion hook over the extension API's
//! [`Expand`], and the façades it lends to the hook: the expander
//! ([`ExpanderFacade`]), the context ([`ExpandCxFacade`]), and the definition
//! façades a hook instantiates its definition's template through
//! (`ModeVariantFacade`, `CompiledTemplateFacade`,
//! `DefinitionBindingsFacade`, crate-private).
//!
//! # One call
//!
//! [`ExtExpand::expand`] lends the hook three façades on its own stack: the use
//! site ([`NodeFacade::lent`]), the context and the expander, each over
//! `flm-core`'s own objects for this call. When the hook instantiates a
//! template whose content uses another scripted macro, `flm-core` calls that
//! macro's hook while the outer `instantiate` runs; the inner call is a new
//! [`ExtExpand::expand`] with façades of its own, so the calls nest as the
//! pass's own calls do.
//!
//! # What ends the pass
//!
//! A failure of `flm-core`'s pass met through a façade (a staging failure, a
//! failure inside an instantiated template) reaches the hook as
//! [`ExtError::Core`] of kind `expand`, and the façade keeps the original
//! error: whatever the hook answers then, the pass ends with that error, as
//! it does when a compiled hook propagates it. A hook's own failure (`Err`, or
//! an answer naming nodes this call did not stage) recovers the use site as
//! its literal text with the Error `flm.ext.handler-failed`.
//!
//! # The definition façades
//!
//! A variant, a template and a use site's bindings each own the definition's
//! `Arc` and the mode (plan A, A19); the bindings also own a copy of the use
//! site's node (A9, A16). Each resolves the `flm-core` value inside its sealed
//! accessor, when [`Expander::instantiate`] asks: the variant through the
//! definition's compiled form, the template from the variant, the bindings
//! built over the use site and the variant.

use std::sync::Arc;

use flm_core::lang::rewrites::userdefs::{
    CompiledTemplate as CoreCompiledTemplate, DefinitionBindings as CoreDefinitionBindings, Expand as CoreExpand,
    ExpandCx as CoreExpandCx, ExpandError, ExpandPolicy, Expander as CoreExpander, Expansion as CoreExpansion,
    ModeVariant as CoreModeVariant, PlaceholderBindings as CorePlaceholderBindings,
    UserDefinition as CoreUserDefinition,
};
use flm_core::lang::{Flm, FlmDiagnostic};
use flm_core::model::element::element_of;
use flm_core::techy::core::node::{BuildId, NodeKind, NodeRef};
use flm_core::techy::core::Lang;
use flm_core::techy::source::TextContent;

use crate::adapter::diag::ExtCondition;
use crate::adapter::names::Owner;
use crate::adapter::node::NodeFacade;
use crate::adapter::values::diagnostic_to_core;
use crate::api::lang::mint::Mode;
use crate::api::lang::rewrites::userdefs::{
    CompiledTemplate, Expand, ExpandCx, Expander, Expansion, ModeVariant, PlaceholderBindings, Staged,
    UserDefinition,
};
use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::adapter::builder::BuiltLangFacade;
use crate::adapter::durable::DurableTable;
use crate::api::lang::BuiltLang;
use crate::sealed::{
    CompiledTemplateCore, DurableOwner, ExpandCxCore, ExpanderCore, ModeVariantCore, PlaceholderBindingsCore, Sealed, Token,
};

/// `flm-core`'s expansion hook over an extension's [`Expand`]: what an
/// element's `Expand` aspect holds once the element is registered, with the
/// name of the package that registered it (the diagnostics the hook reports
/// must begin with it).
pub struct ExtExpand {
    /// The extension's hook.
    pub(crate) hook: Arc<dyn Expand>,
    /// The extension and the package that registered the hook.
    pub(crate) owner: Owner,
    /// The user definition the element was declared from, when a helper made
    /// the declaration ([`UserDefinition::element_def_with`]); answered by
    /// [`ExpandCx::definition`] (plan A, A37).
    pub(crate) definition: Option<UserDefinition>,
}

impl CoreExpand for ExtExpand {
    fn policy(&self) -> ExpandPolicy {
        self.hook.policy()
    }

    /// Lends the hook the use site, the context and the expander, and maps
    /// its answer: `Default` and `Emit` as they are; an `Err`, or an `Emit`
    /// naming a node this call did not stage, recovers the use site with the
    /// Error `flm.ext.handler-failed`.
    ///
    /// # Errors
    ///
    /// A failure of the pass met through a façade during the call, or by the
    /// recovery itself ([`ExpandError`]); it ends the pass.
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut CoreExpandCx<'_>,
        xp: &mut dyn CoreExpander,
    ) -> Result<CoreExpansion, ExpandError> {
        let site = NodeFacade::lent(node);
        let lent = Lent { cx: address_of(cx), site: node };
        let mut cx_facade =
            ExpandCxFacade { cx, site: node, definition: self.definition.as_ref(), staged: Vec::new(), failure: None };
        let mut xp_facade =
            ExpanderFacade { xp, lent, owner: &self.owner, staged: Vec::new(), failure: None };
        let answer = self.hook.expand(&site, &mut cx_facade, &mut xp_facade);
        if let Some(failure) = cx_facade.failure.take().or_else(|| xp_facade.failure.take()) {
            return Err(failure);
        }
        let answer = answer.and_then(|expansion| match expansion {
            Expansion::Default => Ok(CoreExpansion::Default),
            Expansion::Emit(staged) => {
                let mut minted = cx_facade.staged;
                minted.extend(xp_facade.staged);
                emitted(&staged, &minted).map(CoreExpansion::Emit)
            }
        });
        match answer {
            Ok(expansion) => Ok(expansion),
            Err(error) => {
                let handler = format!("the expansion hook of `{}`", element_of(&node).name);
                let diagnostic =
                    FlmDiagnostic::error(ExtCondition::handler_failed(&error, &handler), node.span().clone());
                Ok(CoreExpansion::Emit(xp_facade.xp.recover(cx_facade.cx, node, diagnostic)?))
            }
        }
    }
}

/// The ids of `staged`, each of which this call staged, at most once; the
/// hook's failure otherwise (a node staged by another call, or named twice,
/// would break the tree being built).
fn emitted(staged: &[Staged], minted: &[BuildId]) -> Result<Vec<BuildId>, ExtError> {
    let mut ids: Vec<BuildId> = Vec::with_capacity(staged.len());
    for Staged(id) in staged {
        if !minted.contains(id) {
            return Err(ExtError::Argument {
                method: "expand".to_string(),
                detail: "each node the hook returns must have been created during this same call"
                    .to_string(),
            });
        }
        if ids.contains(id) {
            return Err(ExtError::Argument {
                method: "expand".to_string(),
                detail: "a node may appear only once in what the hook returns".to_string(),
            });
        }
        ids.push(*id);
    }
    Ok(ids)
}

/// The address of a context, which identifies the context lent to a call.
fn address_of(cx: &CoreExpandCx<'_>) -> *const () {
    (cx as *const CoreExpandCx<'_>).cast::<()>()
}

/// Whether `a` and `b` are the same node of the same tree.
fn same_node(a: NodeRef<'_, Flm>, b: NodeRef<'_, Flm>) -> bool {
    std::ptr::eq(a.tree(), b.tree()) && a.id() == b.id()
}

/// A failure of the pass, as the hook sees it.
fn core_error(error: &ExpandError) -> ExtError {
    ExtError::Core { kind: "expand".to_string(), detail: error.to_string(), data: serde_json::Value::Null }
}

/// What was lent to one call: the context (by its address) and the use site.
#[derive(Clone, Copy)]
struct Lent<'n> {
    cx: *const (),
    site: NodeRef<'n, Flm>,
}

impl Lent<'_> {
    /// Checks that `site` is the use site lent to the call.
    fn check_site(&self, method: &str, site: &dyn Node<'_>) -> Result<(), ExtError> {
        if same_node(site.core_node_ref(Token::new()), self.site) {
            Ok(())
        } else {
            Err(ExtError::Argument {
                method: method.to_string(),
                detail: "`site` must be the node this hook call was given".to_string(),
            })
        }
    }

    /// The error of a context that is not the one lent to the call.
    fn other_cx(method: &str) -> ExtError {
        ExtError::Argument {
            method: method.to_string(),
            detail: "`cx` must be the context this hook call was given".to_string(),
        }
    }
}

// --- the context -----------------------------------------------------------------------------

/// The context lent to one call of an expansion hook: `flm-core`'s context of
/// the pass at the use site `site`, the one implementation of [`ExpandCx`].
pub struct ExpandCxFacade<'a, 'x, 'n> {
    cx: &'a mut CoreExpandCx<'x>,
    site: NodeRef<'n, Flm>,
    /// The definition the element was declared from, if a helper declared it.
    definition: Option<&'a UserDefinition>,
    /// The nodes [`stage_chars`](ExpandCx::stage_chars) staged.
    staged: Vec<BuildId>,
    /// The first failure of the pass met while staging.
    failure: Option<ExpandError>,
}

impl ExpandCxFacade<'_, '_, '_> {
    /// Stages one characters node holding `text` at the use site: the calls of
    /// `flm-core`'s recovery (`Expander::recover` of the expand pass), with
    /// owned text in place of the site's own.
    fn stage(&mut self, text: &str) -> Result<BuildId, ExpandError> {
        let span = self.site.span().clone();
        let kind = NodeKind::<Flm>::chars(TextContent::Owned(text.into()));
        let state = Arc::clone(self.site.parsing_state());
        let builder = self.cx.builder();
        let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))?;
        Ok(builder.add(kind, span, state, Vec::new(), ext, ())?)
    }
}

impl ExpandCxCore for ExpandCxFacade<'_, '_, '_> {
    fn with_core(&mut self, _: Token, f: &mut dyn FnMut(&mut CoreExpandCx<'_>)) {
        f(self.cx);
    }
}

impl ExpandCx for ExpandCxFacade<'_, '_, '_> {
    fn stage_chars(&mut self, text: &str) -> Result<Staged, ExtError> {
        match self.stage(text) {
            Ok(id) => {
                self.staged.push(id);
                Ok(Staged(id))
            }
            Err(error) => {
                let reported = core_error(&error);
                self.failure.get_or_insert(error);
                Err(reported)
            }
        }
    }

    fn definition(&self) -> Option<UserDefinition> {
        self.definition.cloned()
    }
}

// --- the expander ----------------------------------------------------------------------------

/// The expander lent to one call of an expansion hook: `flm-core`'s expander,
/// what was lent to the call, and the extension's stem and the package, whose
/// stem (and a dot) the hook's diagnostics must begin with; the one
/// implementation of [`Expander`].
pub struct ExpanderFacade<'a, 'n> {
    xp: &'a mut dyn CoreExpander,
    lent: Lent<'n>,
    owner: &'a Owner,
    /// The nodes `instantiate` and `recover` staged.
    staged: Vec<BuildId>,
    /// The first failure of the pass met through this façade.
    failure: Option<ExpandError>,
}

impl ExpanderFacade<'_, '_> {
    /// Records the answer of a call on `flm-core`'s expander: the staged
    /// nodes, or the pass's failure.
    fn answered(&mut self, answer: Result<Vec<BuildId>, ExpandError>) -> Result<Vec<Staged>, ExtError> {
        match answer {
            Ok(ids) => {
                self.staged.extend(ids.iter().copied());
                Ok(ids.into_iter().map(Staged).collect())
            }
            Err(error) => {
                let reported = core_error(&error);
                self.failure.get_or_insert(error);
                Err(reported)
            }
        }
    }

    /// The hook's diagnostic as `flm-core`'s, at the use site unless it has a
    /// place of its own, after the name rule.
    fn diagnostic(&self, diagnostic: Diagnostic) -> Result<FlmDiagnostic, ExtError> {
        self.owner.check("diagnostic", &diagnostic.id)?;
        diagnostic_to_core(diagnostic, self.lent.site.span().clone())
    }
}

impl Sealed for ExpanderFacade<'_, '_> {}

impl DurableOwner for ExpanderFacade<'_, '_> {
    fn durable_table(&self, _: Token) -> Arc<DurableTable> {
        Arc::clone(&self.owner.durable)
    }
}

impl ExpanderCore for ExpanderFacade<'_, '_> {
    fn with_core(&mut self, _: Token, f: &mut dyn FnMut(&mut dyn CoreExpander)) {
        f(self.xp);
    }
}

impl Expander for ExpanderFacade<'_, '_> {
    fn depth(&self) -> usize {
        self.xp.depth()
    }

    fn recursion_limit(&self) -> usize {
        self.xp.recursion_limit()
    }

    fn lang(&self) -> Box<dyn BuiltLang + '_> {
        Box::new(BuiltLangFacade { lang: self.xp.lang(), durable: Arc::clone(&self.owner.durable) })
    }

    fn instantiate(
        &mut self,
        cx: &mut dyn ExpandCx,
        site: &dyn Node<'_>,
        template: &dyn CompiledTemplate,
        bindings: &dyn PlaceholderBindings,
    ) -> Result<Vec<Staged>, ExtError> {
        const METHOD: &str = "instantiate";
        self.lent.check_site(METHOD, site)?;
        let site = self.lent.site;
        let lent_cx = self.lent.cx;
        let xp = &mut *self.xp;
        let mut same_cx = false;
        let mut answer = None;
        cx.with_core(Token::new(), &mut |core_cx| {
            if address_of(core_cx) != lent_cx {
                return;
            }
            same_cx = true;
            template.with_core(Token::new(), &mut |template| {
                bindings.with_core(Token::new(), &mut |bindings| {
                    answer = Some(xp.instantiate(core_cx, site, template, bindings));
                });
            });
        });
        if !same_cx {
            return Err(Lent::other_cx(METHOD));
        }
        // `None` only for a façade over a definition with no compiled variant
        // for its mode, which `variant_for_use` never answers (plan A, A23).
        match answer {
            Some(answer) => self.answered(answer),
            None => Err(ExtError::Argument {
                method: METHOD.to_string(),
                detail: "the template and the bindings must come from a compiled definition".to_string(),
            }),
        }
    }

    fn recover(&mut self, cx: &mut dyn ExpandCx, site: &dyn Node<'_>, diagnostic: Diagnostic) -> Result<Vec<Staged>, ExtError> {
        const METHOD: &str = "recover";
        self.lent.check_site(METHOD, site)?;
        let mut diagnostic = Some(self.diagnostic(diagnostic)?);
        let site = self.lent.site;
        let lent_cx = self.lent.cx;
        let xp = &mut *self.xp;
        let mut answer = None;
        cx.with_core(Token::new(), &mut |core_cx| {
            if address_of(core_cx) != lent_cx {
                return;
            }
            if let Some(diagnostic) = diagnostic.take() {
                answer = Some(xp.recover(core_cx, site, diagnostic));
            }
        });
        match answer {
            Some(answer) => self.answered(answer),
            None => Err(Lent::other_cx(METHOD)),
        }
    }

    fn diagnose(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError> {
        let diagnostic = self.diagnostic(diagnostic)?;
        self.xp.diagnose(diagnostic);
        Ok(())
    }
}

// --- the definition façades ------------------------------------------------------------------

/// The variant of `definition` for `mode`, when the definition is compiled and
/// has one.
fn variant_of(definition: &CoreUserDefinition, mode: Mode) -> Option<&CoreModeVariant> {
    definition.compiled().and_then(|compiled| compiled.variant(mode))
}

/// A definition's variant for one mode, the one implementation of
/// [`ModeVariant`]: it owns the definition's `Arc` and the mode (plan A, A19).
pub(crate) struct ModeVariantFacade {
    definition: Arc<CoreUserDefinition>,
    mode: Mode,
}

impl ModeVariantFacade {
    /// The variant of `definition` for `mode`.
    pub(crate) fn new(definition: Arc<CoreUserDefinition>, mode: Mode) -> ModeVariantFacade {
        ModeVariantFacade { definition, mode }
    }
}

impl ModeVariantCore for ModeVariantFacade {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&CoreModeVariant)) {
        if let Some(variant) = variant_of(&self.definition, self.mode) {
            f(variant);
        }
    }

    fn definition_and_mode(&self, _: Token) -> (Arc<CoreUserDefinition>, Mode) {
        (Arc::clone(&self.definition), self.mode)
    }
}

impl ModeVariant for ModeVariantFacade {
    fn template(&self) -> Box<dyn CompiledTemplate> {
        Box::new(CompiledTemplateFacade { definition: Arc::clone(&self.definition), mode: self.mode })
    }
}

/// The template of a definition's variant for one mode, the one
/// implementation of [`CompiledTemplate`]: it owns the definition's `Arc` and
/// the mode (plan A, A19).
pub(crate) struct CompiledTemplateFacade {
    definition: Arc<CoreUserDefinition>,
    mode: Mode,
}

impl CompiledTemplateCore for CompiledTemplateFacade {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&CoreCompiledTemplate)) {
        if let Some(variant) = variant_of(&self.definition, self.mode) {
            f(variant.template());
        }
    }
}

impl CompiledTemplate for CompiledTemplateFacade {}

/// The bindings of a use site for a definition's variant, the one
/// implementation of [`PlaceholderBindings`]: it owns a copy of the use site's
/// node, the definition's `Arc` and the mode, and builds `flm-core`'s
/// [`DefinitionBindings`](CoreDefinitionBindings) when asked (plan A, A9).
pub(crate) struct DefinitionBindingsFacade<'t> {
    site: Box<dyn Node<'t> + 't>,
    definition: Arc<CoreUserDefinition>,
    mode: Mode,
}

impl<'t> DefinitionBindingsFacade<'t> {
    /// The bindings of `site` for the variant `variant`.
    pub(crate) fn new(site: &dyn Node<'t>, variant: &dyn ModeVariant) -> DefinitionBindingsFacade<'t> {
        let (definition, mode) = variant.definition_and_mode(Token::new());
        DefinitionBindingsFacade { site: site.clone_box(Token::new()), definition, mode }
    }
}

impl PlaceholderBindingsCore for DefinitionBindingsFacade<'_> {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&dyn CorePlaceholderBindings)) {
        if let Some(variant) = variant_of(&self.definition, self.mode) {
            let bindings = CoreDefinitionBindings::new(self.site.core_node_ref(Token::new()), variant);
            f(&bindings);
        }
    }
}

impl PlaceholderBindings for DefinitionBindingsFacade<'_> {}
