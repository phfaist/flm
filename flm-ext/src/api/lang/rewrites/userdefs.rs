//! Mirrors [`flm_core::lang::rewrites::userdefs`]: user definitions, their
//! expansion hook, and the expander a hook stages its replacement through;
//! and the templates a handler compiles itself. A template language and a
//! compiled template are durable: the token that names one is plain data,
//! valid on every engine instance and every thread for the life of the
//! durable table of the feature that made it (one table per feature and
//! registry), and stale under another registry. A handler need not keep the
//! token: a second [`compile_template_with`] with an equal specification
//! answers the same token without compiling again.

use std::fmt;
use std::sync::Arc;

use flm_core::lang::rewrites::userdefs::{
    instantiate as core_instantiate, CompiledTemplate as CoreCompiledTemplate, ExtraValues as CoreExtraValues,
    PlaceholderBindings as CorePlaceholderBindings,
    TemplateSpec as CoreTemplateSpec, UserDefinition as CoreUserDefinition,
    ValueBindings as CoreValueBindings,
};
use flm_core::lang::{Flm, GuardPolicy};
use flm_core::techy::core::node::{BuildId, NodeTree};
use flm_core::techy::error::DiagnosticInfo;

use crate::adapter::durable::{DurableTemplateFacade, DurableToken};
use crate::adapter::expand::{DefinitionBindingsFacade, LayeredBindingsFacade, ModeVariantFacade};
use crate::adapter::node::TreeFacade;
use crate::api::lang::mint::{ElementDef, Mode};
use crate::api::lang::BuiltLang;
use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::{Node, Tree};
use crate::runtime::ExtError;
use crate::sealed::{CompiledTemplateCore, ExpandCxCore, ExpanderCore, ModeVariantCore, PlaceholderBindingsCore, Token};

pub use flm_core::lang::rewrites::userdefs::{
    Definition, DefinitionKind, DefinitionModes, ExpandPolicy, PlaceholderTarget, PlaceholderVocabulary,
};

/// Mirrors [`Expand`](flm_core::lang::rewrites::userdefs::Expand): the
/// expansion hook of an element, which an extension implements and attaches
/// with [`Aspect::Expand`](crate::api::model::element::Aspect::Expand).
///
/// What differs: the node, the context and the expander are the extension
/// API's façades; errors are [`ExtError`], and an `Err` is the handler's
/// failure: the adapter recovers the use site as its literal text, with an
/// error diagnostic. The exception: once a façade method
/// ([`Expander::instantiate`], [`Expander::recover`], [`ExpandCx::stage_chars`])
/// has answered [`ExtError::Core`] of kind `expand`, the pass ends with that
/// failure, whatever the hook answers next (as `flm-core`'s pass does when a
/// hook propagates its [`ExpandError`](flm_core::lang::rewrites::userdefs::ExpandError)).
pub trait Expand: Send + Sync {
    /// Mirrors [`Expand::policy`](flm_core::lang::rewrites::userdefs::Expand::policy).
    fn policy(&self) -> ExpandPolicy {
        ExpandPolicy::PerPlan
    }

    /// Mirrors [`Expand::expand`](flm_core::lang::rewrites::userdefs::Expand::expand):
    /// decides what the use site `node` becomes. What differs: the node's
    /// tree and the three borrows share one lifetime, the call's `'c`, so that
    /// a binding can lend all three into one
    /// [`CallTable<'c>`](crate::adapter::handles::CallTable); the nodes the
    /// use site answers live for the whole call.
    ///
    /// # Errors
    ///
    /// The handler's failure; the adapter recovers the use site, unless a
    /// façade method answered [`ExtError::Core`] of kind `expand` during the
    /// call: the pass then ends, whatever the hook answers.
    fn expand<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn ExpandCx,
        xp: &'c mut dyn Expander,
    ) -> Result<Expansion, ExtError>;
}

/// Mirrors [`Expansion`](flm_core::lang::rewrites::userdefs::Expansion): what
/// a hook decided about one use site. What differs: the staged nodes are
/// [`Staged`] values, and [`Emit`](Expansion::Emit) may name only nodes
/// staged during the same call of the hook (by [`ExpandCx::stage_chars`],
/// [`Expander::instantiate`] or [`Expander::recover`]), each at most once;
/// any other answer is the hook's failure, and the adapter recovers the use
/// site (`flm-core`'s pass would end on it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    /// Leave the node as it is.
    Default,
    /// These staged nodes stand in for the use site (an empty list drops it).
    Emit(Vec<Staged>),
}

/// Mirrors techy's [`BuildId`]: a node
/// staged during one call of an expansion hook, or of a lowering rule's body
/// ([`Lower`](crate::api::core::lowering::Lower)). Only the adapter makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Staged(pub(crate) BuildId);

/// Mirrors [`Expander`](flm_core::lang::rewrites::userdefs::Expander): the
/// expand pass, as a hook sees it.
///
/// What differs: the façades in place of `flm-core`'s objects; the diagnostic
/// is the extension API's [`Diagnostic`], placed at the use site unless it
/// has a place of its own; [`diagnose`](Expander::diagnose) answers a
/// `Result`.
///
/// The trait is sealed: the adapter alone implements it.
pub trait Expander: ExpanderCore {
    /// Mirrors [`Expander::depth`](flm_core::lang::rewrites::userdefs::Expander::depth).
    fn depth(&self) -> usize;

    /// Mirrors [`Expander::recursion_limit`](flm_core::lang::rewrites::userdefs::Expander::recursion_limit).
    fn recursion_limit(&self) -> usize;

    /// Mirrors [`Expander::lang`](flm_core::lang::rewrites::userdefs::Expander::lang):
    /// the built language the expansion runs under, which a hook compiles a
    /// template it computed at the use site against
    /// ([`compile_template_with`]). What differs: a façade with no method of
    /// its own, borrowing the expander; it carries the durable table of the
    /// hook's feature.
    fn lang(&self) -> Box<dyn BuiltLang + '_>;

    /// Mirrors [`Expander::instantiate`](flm_core::lang::rewrites::userdefs::Expander::instantiate):
    /// stages `template` with `bindings` at the use site `site`. What differs:
    /// `cx` and `site` must be the ones lent to the running hook; `template`
    /// is a definition's variant's template or default value
    /// ([`ModeVariant::template`], [`ModeVariant::defaults`]) or a durable
    /// one ([`compile_template_with`]), and `bindings` the use site's
    /// ([`DefinitionBindings::new`]), values ([`ValueBindings`]), or computed
    /// values laid over other bindings ([`ExtraValues::over`]).
    ///
    /// # Errors
    ///
    /// `cx` or `site` was not lent to this call, or a façade is not one the
    /// adapter made ([`ExtError::Argument`]); the pass fails
    /// ([`ExtError::Core`] of kind `expand`): the pass then ends with that
    /// failure, whatever the hook answers.
    fn instantiate(
        &mut self,
        cx: &mut dyn ExpandCx,
        site: &dyn Node<'_>,
        template: &dyn CompiledTemplate,
        bindings: &dyn PlaceholderBindings,
    ) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Expander::recover`](flm_core::lang::rewrites::userdefs::Expander::recover):
    /// reports `diagnostic` and stages the use site's literal text. What
    /// differs: the diagnostic is data.
    ///
    /// # Errors
    ///
    /// As [`instantiate`](Expander::instantiate) (after an [`ExtError::Core`]
    /// of kind `expand`, the pass ends, whatever the hook answers); the
    /// diagnostic's data cannot cross ([`ExtError::Value`]).
    fn recover(&mut self, cx: &mut dyn ExpandCx, site: &dyn Node<'_>, diagnostic: Diagnostic) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Expander::diagnose`](flm_core::lang::rewrites::userdefs::Expander::diagnose):
    /// reports `diagnostic` and keeps the node. What differs: the diagnostic is
    /// data, and the method answers a `Result`, since the data may hold a
    /// value that cannot cross.
    ///
    /// # Errors
    ///
    /// The diagnostic's data cannot cross ([`ExtError::Value`]); nothing is
    /// reported then.
    fn diagnose(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError>;
}

/// Mirrors [`ExpandCx`](flm_core::lang::rewrites::userdefs::ExpandCx), techy's
/// restage context at a use site.
///
/// What differs: the context is opaque but for one method,
/// [`stage_chars`](ExpandCx::stage_chars), the staging of computed text (the
/// composite the expander's own recovery performs); the restage context has
/// no mirror yet.
///
/// The trait is sealed: the adapter alone implements it.
pub trait ExpandCx: ExpandCxCore {
    /// Stages one characters node holding `text`, spanned to the use site and
    /// in the use site's parsing state: the calls
    /// [`Expander::recover`](flm_core::lang::rewrites::userdefs::Expander::recover)
    /// makes to stage the site's text, with `text` in its place.
    ///
    /// # Errors
    ///
    /// The staging fails ([`ExtError::Core`] of kind `expand`): the pass then
    /// ends with that failure, whatever the hook answers.
    fn stage_chars(&mut self, text: &str) -> Result<Staged, ExtError>;

    /// No flm-core counterpart: the user definition registered with the
    /// element whose hook is running, or `None` for an element declared in the
    /// data form (an [`ElementDef`] not made by
    /// [`UserDefinition::element_def_with`]).
    ///
    /// A Rust hook owns its definition and needs nothing more; a hook written
    /// in a scripting language can hold the definition it made while
    /// registering only as an object of that call, unusable in every later
    /// call. This method hands the hook the registered definition, the one
    /// whose templates were compiled when the language was built, so that the
    /// hook can instantiate them ([`UserDefinition::variant_for_use`]). The
    /// value answered owns the definition.
    fn definition(&self) -> Option<UserDefinition>;
}

/// Mirrors [`UserDefinition`](flm_core::lang::rewrites::userdefs::UserDefinition):
/// a definition shared by its element, its hook and the late-init hook that
/// compiles its templates. What differs: a value wrapping the shared
/// definition, and the methods below.
#[derive(Clone)]
pub struct UserDefinition {
    pub(crate) definition: Arc<CoreUserDefinition>,
}

impl UserDefinition {
    /// Mirrors [`UserDefinition::new`](flm_core::lang::rewrites::userdefs::UserDefinition::new).
    pub fn new(definition: Definition) -> UserDefinition {
        UserDefinition { definition: CoreUserDefinition::new(definition) }
    }

    /// Mirrors [`UserDefinition::definition`](flm_core::lang::rewrites::userdefs::UserDefinition::definition).
    pub fn definition(&self) -> &Definition {
        self.definition.definition()
    }

    /// Mirrors [`UserDefinition::element_def`](flm_core::lang::rewrites::userdefs::UserDefinition::element_def):
    /// the declaration of the definition's element, expanded by the
    /// definition's own hook. What differs: the value holds what it was given
    /// (the definition, `owner`, `element_name`); the `flm-core` declaration is
    /// made when [`FeatureBuilder::element`](crate::api::core::registry::FeatureBuilder::element)
    /// registers it, which also has the definition's templates compiled once
    /// the language is built.
    pub fn element_def(&self, owner: &str, element_name: &str) -> ElementDef {
        ElementDef::from_helper(Arc::clone(&self.definition), owner, element_name, None)
    }

    /// Mirrors [`UserDefinition::element_def_with`](flm_core::lang::rewrites::userdefs::UserDefinition::element_def_with):
    /// the declaration of the definition's element, expanded by `hook`. What
    /// differs: as [`element_def`](UserDefinition::element_def); the hook is
    /// the extension API's, and the registration wraps it for `flm-core`
    /// together with the name of the package that registers it.
    pub fn element_def_with(&self, owner: &str, element_name: &str, hook: Arc<dyn Expand>) -> ElementDef {
        ElementDef::from_helper(Arc::clone(&self.definition), owner, element_name, Some(hook))
    }

    /// Mirrors [`UserDefinition::variant_for_use`](flm_core::lang::rewrites::userdefs::UserDefinition::variant_for_use):
    /// the definition's variant for the mode of the use site `node`. What
    /// differs: the variant is a façade that owns the definition's `Arc` and
    /// the mode, and resolves the `flm-core` variant when the adapter needs it;
    /// it borrows nothing, so it may outlive this value.
    ///
    /// # Errors
    ///
    /// The definition has no valid variant for that mode: a `BadDefinition`,
    /// as [`ExtError::Core`] whose `kind` is the condition's identifier
    /// (`flm.userdefs.bad-definition`), whose `detail` is its message and
    /// whose `data` is its data.
    pub fn variant_for_use(&self, node: &dyn Node<'_>) -> Result<Box<dyn ModeVariant>, ExtError> {
        let site = node.core_node_ref(Token::new());
        match self.definition.variant_for_use(&site) {
            Ok(variant) => {
                let mode = site.parsing_state().mode();
                Ok(Box::new(ModeVariantFacade::new(Arc::clone(&self.definition), mode, variant)))
            }
            Err(condition) => Err(ExtError::Core {
                kind: condition.identifier().to_string(),
                detail: condition.to_string(),
                data: crate::adapter::values::diagnostic_value_to_json(&condition.serializable_data()),
            }),
        }
    }
}

impl fmt::Debug for UserDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("UserDefinition").field(self.definition.definition()).finish()
    }
}

/// Mirrors [`ModeVariant`](flm_core::lang::rewrites::userdefs::ModeVariant):
/// a definition's compiled templates for one mode. What differs: a façade with
/// the methods [`template`](ModeVariant::template) and
/// [`defaults`](ModeVariant::defaults), owning the definition's `Arc` and the
/// mode.
///
/// The trait is sealed: the adapter alone implements it.
pub trait ModeVariant: ModeVariantCore {
    /// Mirrors [`ModeVariant::template`](flm_core::lang::rewrites::userdefs::ModeVariant::template).
    /// What differs: the template is a façade that shares the `flm-core`
    /// template with the variant; it borrows nothing of the variant.
    fn template(&self) -> Box<dyn CompiledTemplate>;

    /// Mirrors [`ModeVariant::defaults`](flm_core::lang::rewrites::userdefs::ModeVariant::defaults):
    /// the compiled default values of the optional arguments, with their
    /// argument indices, in increasing order of index. What differs: a list;
    /// each default value is a template façade that shares, as
    /// [`template`](ModeVariant::template)'s does, the `flm-core` value with
    /// the variant.
    fn defaults(&self) -> Vec<(usize, Box<dyn CompiledTemplate>)>;
}

/// Mirrors [`CompiledTemplate`](flm_core::lang::rewrites::userdefs::CompiledTemplate):
/// a compiled replacement text. What differs: a façade whose one method lists
/// the template's placeholders; it is passed to [`Expander::instantiate`] and
/// [`instantiate`]. The adapter's façades are a definition's template or
/// default value, which shares the `flm-core` template with the definition's
/// variant, and a durable template ([`compile_template_with`]).
///
/// The trait is sealed: the adapter alone implements it.
pub trait CompiledTemplate: CompiledTemplateCore {
    /// Mirrors [`CompiledTemplate::placeholders`](flm_core::lang::rewrites::userdefs::CompiledTemplate::placeholders):
    /// every placeholder of the template, in the tree's storage order. What
    /// differs: a list of [`PlaceholderRef`] values, which give each
    /// placeholder's substitution text in place of its node.
    fn placeholders(&self) -> Vec<PlaceholderRef>;
}

/// Mirrors [`PlaceholderRef`](flm_core::lang::rewrites::userdefs::PlaceholderRef):
/// one placeholder of a compiled template
/// ([`CompiledTemplate::placeholders`]). What differs: `substitution` holds
/// the text of the substitution argument, as
/// [`CompiledTemplate::substitution_text`](flm_core::lang::rewrites::userdefs::CompiledTemplate::substitution_text)
/// gives it, where `flm-core`'s field says whether one is written; with the
/// name of an extra placeholder, that text is the key [`ExtraValues`] holds a
/// value under. Across a binding: `{ written, target, substitution }`, the
/// target `{"argument": index}`, `"body"` or `{"extra": name}`, the
/// substitution a text or `null`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderRef {
    /// The placeholder as written between the `#` and the end of the
    /// placeholder (`1`, `body`, `title`).
    pub written: String,
    /// What it refers to.
    pub target: PlaceholderTarget,
    /// The text of the substitution argument (`#[…]{…}`): the source text
    /// between the brackets, with the whitespace around it removed; `None`
    /// when none is written.
    pub substitution: Option<String>,
}

/// Mirrors [`PlaceholderBindings`](flm_core::lang::rewrites::userdefs::PlaceholderBindings):
/// what a template's placeholders stand for at one use site. What differs:
/// opaque; the implementations are the use site's own bindings
/// ([`DefinitionBindings::new`]), the values of a template without a use
/// site ([`ValueBindings`]), and computed values laid over other bindings
/// ([`ExtraValues::over`]).
///
/// The trait is sealed: the adapter alone implements it.
pub trait PlaceholderBindings: PlaceholderBindingsCore {}

/// Mirrors [`DefinitionBindings`](flm_core::lang::rewrites::userdefs::DefinitionBindings):
/// the bindings of a use site's arguments to a definition's placeholders.
/// What differs: only its constructor, which answers the façade.
pub struct DefinitionBindings;

impl DefinitionBindings {
    /// Mirrors [`DefinitionBindings::new`](flm_core::lang::rewrites::userdefs::DefinitionBindings::new):
    /// the bindings of `site`'s arguments for `variant`. What differs: the
    /// bindings are a façade, which keeps an owned copy of the use site's node
    /// (so it lives as long as the site's tree) together with the definition's
    /// `Arc` and the mode, and builds `flm-core`'s bindings when
    /// [`Expander::instantiate`] asks.
    // The mirror of `flm-core`'s constructor, which answers the bindings; here
    // they are answered as the façade.
    #[allow(clippy::new_ret_no_self)]
    pub fn new<'t>(site: &dyn Node<'t>, variant: &dyn ModeVariant) -> Box<dyn PlaceholderBindings + 't> {
        Box::new(DefinitionBindingsFacade::new(site, variant))
    }
}

// --- templates compiled by a handler ---------------------------------------------------------

/// Mirrors [`TemplateSpec`](flm_core::lang::rewrites::userdefs::TemplateSpec):
/// what to compile, one template text for one parsing mode, against a
/// vocabulary, on behalf of a definition. What differs: the fields are owned
/// (`flm-core`'s borrow them), so that a handler writes the specification as
/// data; across a binding, `vocabulary` is written `{ arity, names?, body?,
/// extras?: [{ name, substitution }] }`, `user_definition` and `offset` may be
/// left out (`false`, `0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSpec {
    /// The template text.
    pub text: String,
    /// The parsing mode the template is compiled for.
    pub mode: Mode,
    /// What the template may refer to.
    pub vocabulary: PlaceholderVocabulary,
    /// The definition's name as an author writes it (`\foo`), or what the
    /// template is (`the heading title of myhead`), for diagnostics.
    pub definition: String,
    /// The description of the sources created for each use.
    pub label: String,
    /// What the compiled source is called, without the mode.
    pub origin_label: String,
    /// Whether the text is a user definition's own (its compiled source's
    /// origin is then a definition's rather than synthetic).
    pub user_definition: bool,
    /// The definition's argument number offset: `#1` is the argument at this
    /// index.
    pub offset: usize,
}

impl TemplateSpec {
    /// The `flm-core` specification, borrowing this one.
    pub(crate) fn as_core(&self) -> CoreTemplateSpec<'_> {
        CoreTemplateSpec {
            text: &self.text,
            mode: self.mode,
            vocabulary: &self.vocabulary,
            offset: self.offset,
            definition: &self.definition,
            label: &self.label,
            origin_label: &self.origin_label,
            user_definition: self.user_definition,
        }
    }
}

/// Mirrors [`TemplateLanguage`](flm_core::lang::rewrites::userdefs::TemplateLanguage):
/// the language template texts are parsed in, built from a registry's
/// language. What differs: a durable handle (its token, [`token`](TemplateLanguage::token),
/// is plain data a handler may keep), naming the one template language of
/// the running handler's feature under the registry in use: the adapter
/// builds it at the first [`new`](TemplateLanguage::new) and answers it again
/// at every later one, from any handler of the feature on any thread. The
/// token is stale under another registry or in another feature's handler
/// ([`ExtError::StaleDurable`] where it is resolved).
#[derive(Clone)]
pub struct TemplateLanguage {
    token: DurableToken,
}

impl TemplateLanguage {
    /// Mirrors [`TemplateLanguage::new`](flm_core::lang::rewrites::userdefs::TemplateLanguage::new):
    /// the template language of `lang`. What differs: built once per feature
    /// per registry and kept by the adapter (the durable table `lang` carries),
    /// answered again at every later call.
    ///
    /// # Errors
    ///
    /// The language does not build: [`ExtError::Core`] of kind
    /// `template_language`, its data the list of the build errors' texts.
    pub fn new(lang: &dyn BuiltLang) -> Result<TemplateLanguage, ExtError> {
        let (token, _) = lang.durable_table(Token::new()).language(lang.core_lang(Token::new()))?;
        Ok(TemplateLanguage { token })
    }

    /// No `flm-core` counterpart: the durable token that names the language.
    pub fn token(&self) -> DurableToken {
        self.token
    }

    /// The language the token `token` names, as a value (no check is made
    /// here: [`compile_template_with`] resolves it against the table of the
    /// call).
    pub(crate) fn from_token(token: DurableToken) -> TemplateLanguage {
        TemplateLanguage { token }
    }
}

impl std::fmt::Debug for TemplateLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TemplateLanguage").field(&self.token).finish()
    }
}

/// Mirrors [`compile_template_with`](flm_core::lang::rewrites::userdefs::compile_template_with):
/// compiles `spec` against `lang` with the template language `language`.
/// What differs: the template is a durable handle (its façade's
/// [`token`](DurableTemplateFacade::token) is plain data a handler may keep,
/// resolved again in a later call by the method `durable_template` of the
/// façade that lent the call: the builder, a stage's context, the expander,
/// the lowerer, a late-initialization hook's language),
/// kept by the adapter in the durable table of the running handler's feature
/// and deduplicated by the specification: a second call with an equal
/// specification, from any handler of the feature on any thread, answers the
/// same template without compiling again. `language` must be the feature's
/// own ([`TemplateLanguage::new`] of a language of the same call's feature).
///
/// # Errors
///
/// `language` is not the template language of the running handler's feature
/// under the registry in use ([`ExtError::StaleDurable`]); the language does
/// not build (as [`TemplateLanguage::new`]); the template does not compile:
/// [`ExtError::Core`] of kind `compile_template`, its detail the
/// compilation's diagnostics' texts joined by `; ` and its data the list of
/// those texts.
pub fn compile_template_with(
    spec: &TemplateSpec,
    lang: &dyn BuiltLang,
    language: &TemplateLanguage,
) -> Result<DurableTemplateFacade, ExtError> {
    let table = lang.durable_table(Token::new());
    table.template_language(language.token)?;
    let (token, _) = table.template(&spec.as_core(), lang.core_lang(Token::new()))?;
    table.compiled_template(token)
}

/// No `flm-core` counterpart: the façades a Rust handler resolves a kept
/// durable token through (the builder, a stage's context, the expander, the
/// lowerer, a late-initialization hook's language), with the method
/// `durable_template`; the trait is sealed (only the adapter implements it).
pub use crate::sealed::DurableOwner;

/// Mirrors [`ValueBindings`](flm_core::lang::rewrites::userdefs::ValueBindings):
/// placeholder bindings that give each extra placeholder (`#{name}`) of a
/// template the content of a finished tree, by name. What differs: a value
/// built with [`new`](ValueBindings::new) and [`with`](ValueBindings::with),
/// which takes the tree as its façade (a number segment's content, a tree
/// [`instantiate`] answered).
#[derive(Debug, Clone, Default)]
pub struct ValueBindings {
    core: CoreValueBindings,
}

impl ValueBindings {
    /// Mirrors [`ValueBindings::new`](flm_core::lang::rewrites::userdefs::ValueBindings::new):
    /// bindings that hold no value.
    pub fn new() -> ValueBindings {
        ValueBindings::default()
    }

    /// Mirrors [`ValueBindings::with`](flm_core::lang::rewrites::userdefs::ValueBindings::with):
    /// gives the extra placeholder `name` the tree `tree`. What differs: the
    /// tree is its façade, and the bindings share the tree it holds.
    pub fn with(self, name: &str, tree: &dyn Tree) -> ValueBindings {
        ValueBindings { core: self.core.with(name, tree.shared(Token::new())) }
    }
}

impl PlaceholderBindingsCore for ValueBindings {
    fn with_core(&self, _: Token, f: &mut dyn FnMut(&dyn CorePlaceholderBindings)) {
        f(&self.core);
    }

    fn clone_box<'a>(&self, _: Token) -> Box<dyn PlaceholderBindings + 'a>
    where
        Self: 'a,
    {
        Box::new(self.clone())
    }
}

impl PlaceholderBindings for ValueBindings {}

/// Mirrors [`ExtraValues`](flm_core::lang::rewrites::userdefs::ExtraValues):
/// the values an expansion hook computed for the extra placeholders of a
/// template, each held under the placeholder's name and the text of its
/// substitution argument ([`PlaceholderRef::substitution`]); a pair given
/// twice keeps the later value. What differs: a compiled template is given
/// by its façade (a definition's template or default value,
/// [`ModeVariant::template`], [`ModeVariant::defaults`], or a durable
/// template, [`compile_template_with`]) and held as the `flm-core` template
/// the façade shares; a tree is given by its façade and held as the
/// `flm-core` tree the façade shares. Across a binding: a list of `{ name,
/// substitution, value }` in the order given, the substitution a text or
/// `null`, the value `{"template": <handle or durable token>}`, `{"tree":
/// <tree>}` or `"no-value"`.
#[derive(Default)]
pub struct ExtraValues {
    /// The values, in the order given: the name, the substitution text, the
    /// value.
    entries: Vec<(String, Option<String>, ExtraValue)>,
}

/// One value of [`ExtraValues`].
enum ExtraValue {
    /// A compiled template, instantiated in place.
    Template(Arc<CoreCompiledTemplate>),
    /// A tree, whose root's children are substituted.
    Tree(Arc<NodeTree<Flm>>),
    /// No value.
    NoValue,
}

impl ExtraValues {
    /// Mirrors [`ExtraValues::new`](flm_core::lang::rewrites::userdefs::ExtraValues::new):
    /// values that hold nothing.
    pub fn new() -> ExtraValues {
        ExtraValues::default()
    }

    /// Mirrors [`ExtraValues::template`](flm_core::lang::rewrites::userdefs::ExtraValues::template):
    /// gives the extra placeholder `name`, written with the substitution
    /// argument whose text is `substitution` (`None` for a placeholder
    /// written without one), the compiled template `template`, instantiated
    /// in place. What differs: `template` is a façade, a definition's
    /// template or default value or a durable template; the values hold the
    /// `flm-core` template it shares.
    pub fn template(self, name: &str, substitution: Option<&str>, template: &dyn CompiledTemplate) -> ExtraValues {
        self.push(name, substitution, ExtraValue::Template(template.shared(Token::new())))
    }

    /// Mirrors [`ExtraValues::tree`](flm_core::lang::rewrites::userdefs::ExtraValues::tree):
    /// gives the extra placeholder `name`, written with the substitution
    /// argument whose text is `substitution`, the tree `tree`, whose root's
    /// children are substituted. What differs: the tree is its façade; the
    /// values hold the `flm-core` tree it shares.
    pub fn tree(self, name: &str, substitution: Option<&str>, tree: &dyn Tree) -> ExtraValues {
        self.push(name, substitution, ExtraValue::Tree(tree.shared(Token::new())))
    }

    /// Mirrors [`ExtraValues::no_value`](flm_core::lang::rewrites::userdefs::ExtraValues::no_value):
    /// gives the extra placeholder `name`, written with the substitution
    /// argument whose text is `substitution`, no value.
    pub fn no_value(self, name: &str, substitution: Option<&str>) -> ExtraValues {
        self.push(name, substitution, ExtraValue::NoValue)
    }

    /// Mirrors [`ExtraValues::over`](flm_core::lang::rewrites::userdefs::ExtraValues::over):
    /// the bindings that answer an extra placeholder from these values, as
    /// [`Unresolvable`](flm_core::lang::rewrites::userdefs::PlaceholderValue::Unresolvable)
    /// when they hold none for its name and substitution text, and every
    /// other placeholder from `base`. What differs: the bindings are a façade
    /// that owns a copy of `base` and `flm-core`'s values, so that it lends
    /// them borrowed for the whole instantiation, as `flm-core`'s bindings do;
    /// it borrows nothing of these values.
    pub fn over<'b>(&self, base: &(dyn PlaceholderBindings + 'b)) -> Box<dyn PlaceholderBindings + 'b> {
        let mut values = CoreExtraValues::new();
        for (name, substitution, value) in &self.entries {
            let substitution = substitution.as_deref();
            values = match value {
                ExtraValue::Template(template) => values.template(name, substitution, Arc::clone(template)),
                ExtraValue::Tree(tree) => values.tree(name, substitution, Arc::clone(tree)),
                ExtraValue::NoValue => values.no_value(name, substitution),
            };
        }
        Box::new(LayeredBindingsFacade::new(values, base.clone_box(Token::new())))
    }

    /// Appends `value` under `name` and `substitution`.
    fn push(mut self, name: &str, substitution: Option<&str>, value: ExtraValue) -> ExtraValues {
        self.entries.push((name.to_string(), substitution.map(str::to_string), value));
        self
    }
}

impl fmt::Debug for ExtraValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries = self.entries.iter().map(|(name, substitution, value)| {
            let value = match value {
                ExtraValue::Template(template) => format!("template {:?}", template.text()),
                ExtraValue::Tree(_) => "tree".to_string(),
                ExtraValue::NoValue => "no value".to_string(),
            };
            (name, substitution, value)
        });
        f.debug_list().entries(entries).finish()
    }
}

/// Mirrors [`instantiate`](fn@flm_core::lang::rewrites::userdefs::instantiate):
/// instantiates `template` under `bindings` as a new tree, attributed to the
/// span of `site`. What differs: the template is a façade (a durable one,
/// [`compile_template_with`], or a definition's variant's template or default
/// value, [`ModeVariant::template`], [`ModeVariant::defaults`]), the bindings
/// a façade ([`ValueBindings`], [`DefinitionBindings::new`] or
/// [`ExtraValues::over`]); the guard policy is
/// [`GuardPolicy::StackBudget`]; the new tree is a façade, which a rule
/// splices ([`Lowerer::splice`](crate::api::core::lowering::Lowerer::splice)).
///
/// As `flm-core`'s, the call must run on a [worker
/// thread](flm_core::lang::guard#the-worker-thread-requirement), since the
/// instantiation descends the template recursively; it calls `flm-core`'s
/// function in place and does not move to one. The calls that are lent a
/// node to instantiate at (a lowering rule's body, an expansion hook, a
/// stage) run on the library's worker threads already, so a handler needs
/// nothing more; a caller of its own outside them wraps the call in
/// [`on_worker_thread`](flm_core::lang::guard::on_worker_thread).
///
/// # Errors
///
/// The instantiation fails (a placeholder the bindings cannot give a value,
/// the content nesting too deeply): [`ExtError::Core`] of kind `expand`.
pub fn instantiate(
    site: &dyn Node<'_>,
    template: &dyn CompiledTemplate,
    bindings: &dyn PlaceholderBindings,
) -> Result<Box<dyn Tree>, ExtError> {
    let span = site.core_node_ref(Token::new()).span().clone();
    let mut answer = None;
    template.with_core(Token::new(), &mut |template| {
        bindings.with_core(Token::new(), &mut |bindings| {
            answer = Some(core_instantiate(&span, template, bindings, GuardPolicy::StackBudget));
        });
    });
    match answer {
        // `None` only for the bindings of a definition with no compiled
        // variant for its mode, which `variant_for_use` never answers.
        None => Err(ExtError::Argument {
            method: String::new(),
            detail: "the bindings must come from a definition compiled for the mode of the use".to_string(),
        }),
        Some(Err(error)) => Err(ExtError::Core { kind: "expand".to_string(), detail: error.to_string(), data: serde_json::Value::Null }),
        Some(Ok(tree)) => Ok(Box::new(TreeFacade::new(Arc::new(tree)))),
    }
}

fn _object_safe(
    _: &dyn Expand,
    _: &dyn Expander,
    _: &dyn ExpandCx,
    _: &dyn ModeVariant,
    _: &dyn CompiledTemplate,
    _: &dyn PlaceholderBindings,
) {
}
