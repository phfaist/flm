//! Mirrors [`flm_core::core::lowering`]: lowering rules, what they declare,
//! the interface a rule body works through, and the text readers of lowering
//! rules. A rule body reads a table it declared with `get`, answered as a
//! [`TableValue`], which a script receives as `{ ext }`, `{ trees }`,
//! `{ projection }`, `{ counters }` (the numbering feature's table, a
//! `Counters` façade) or `{ anchors }` (the references feature's anchors, an
//! `Anchors` façade).
//!
//! An extension declares a rule with [`LoweringRule::new`] and registers it
//! with [`FeatureBuilder::lowering_rule`](crate::api::core::registry::FeatureBuilder::lowering_rule).
//! The rule's body implements [`Lower`]; when the lowering stage reaches a node
//! of the rule's element, the adapter lends the body the node, a [`LowerCx`]
//! and a [`Lowerer`] for that one call, and the body builds the node's
//! presentation through the [`Lowerer`]'s methods, each answering the staged
//! nodes as [`Staged`] values.
//!
//! [`pres_tree`] has no counterpart in `flm-core`: it stages a whole tree of
//! presentation nodes, written as data, in one call.

use std::fmt;
use std::sync::Arc;

use flm_core::core::lowering::{LowerError, LoweringRule as CoreLoweringRule};
use flm_core::model::addr::NodeAddr;
use serde_json::Value;

use crate::api::core::data::table::TableValue;
use crate::api::lang::BuiltLang;
use crate::api::lang::text::NotCharacters;
use crate::api::techy::error::Diagnostic;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::sealed::{LowerCxCore, LowererCore, Sealed, Token};

pub use crate::api::lang::rewrites::userdefs::Staged;
pub use flm_core::core::lowering::{DefaultMark, NodeRegion};

/// A `LowerError` as the extension API reports it.
fn lower(error: &LowerError) -> ExtError {
    ExtError::Core { kind: "lower".to_string(), detail: error.to_string(), data: serde_json::Value::Null }
}

/// Mirrors [`chars_at_lowering`](flm_core::core::lowering::chars_at_lowering):
/// the characters of `nodes`, or the node that is not characters. What
/// differs: the nodes are the extension API's; a `LowerError` is
/// [`ExtError::Core`].
///
/// # Errors
///
/// The nodes hold a placeholder ([`ExtError::Core`]).
pub fn chars_at_lowering<'t>(nodes: &[Box<dyn Node<'t> + 't>]) -> Result<Result<String, NotCharacters>, ExtError> {
    flm_core::core::lowering::chars_at_lowering(nodes.iter().map(|node| node.core_node_ref(Token::new())))
        .map_err(|error| lower(&error))
}

/// Mirrors [`spelling_at_lowering`](flm_core::core::lowering::spelling_at_lowering):
/// whether a rule may compute on the spelling of `nodes` (the content as
/// written, markup included): no placeholder of a definition's template stands
/// in it. What differs: the nodes
/// are the extension API's; a `LowerError` is [`ExtError::Core`].
///
/// # Errors
///
/// The nodes hold a placeholder ([`ExtError::Core`]).
pub fn spelling_at_lowering<'t>(nodes: &[Box<dyn Node<'t> + 't>]) -> Result<(), ExtError> {
    flm_core::core::lowering::spelling_at_lowering(nodes.iter().map(|node| node.core_node_ref(Token::new())))
        .map_err(|error| lower(&error))
}

// --- the rule ----------------------------------------------------------------------------------

/// Mirrors [`Lower`](flm_core::core::lowering::Lower): the body of a lowering
/// rule, which an extension implements and passes to [`LoweringRule::new`].
///
/// What differs: the node, the context and the lowerer are the extension
/// API's façades, lent for one call; errors are [`ExtError`]. An `Err` (or an
/// [`Emit`](Lowering::Emit) naming a node the call did not stage, or naming
/// one twice) is the body's failure: the adapter reports the Error
/// `flm.ext.handler-failed` at the node, whose message names the rule, and
/// the node is lowered as `flm-core` lowers the node of a rule that refused
/// it ([`LowerError::Refused`]): a childless `flm.pres.unsupported` stand-in
/// takes its place, and the rest of the tree lowers normally. The exception,
/// as for `flm-core`'s rules, which must return every error the lowerer gives
/// them: once a [`Lowerer`] method that stages (one that takes the
/// [`LowerCx`]) has answered [`ExtError::Core`] of kind `lower`, the call ends
/// with that `flm-core` error, whatever the body answers next.
pub trait Lower: Send + Sync {
    /// Mirrors [`Lower::lower`](flm_core::core::lowering::Lower::lower): lowers
    /// `node`, a node of the rule's element, and answers what replaces it.
    /// What differs: the node's tree and the three borrows share one
    /// lifetime, the call's `'c`, so that a binding can lend all three into
    /// one [`CallTable<'c>`](crate::adapter::handles::CallTable), as for
    /// [`Expand::expand`](crate::api::lang::rewrites::userdefs::Expand::expand).
    ///
    /// # Errors
    ///
    /// The body's failure; see the trait's documentation for what follows.
    fn lower<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn LowerCx,
        lw: &'c mut dyn Lowerer,
    ) -> Result<Lowering, ExtError>;
}

/// Mirrors [`Lowering`](flm_core::core::lowering::Lowering): what a rule's
/// body answers for one node. What differs: the nodes are [`Staged`] values,
/// each of which the same call must have staged, and each named once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lowering {
    /// Copy the node into the presentation tree unchanged, and lower its
    /// children.
    Default,
    /// The nodes the body staged to replace the node, in order (an empty list
    /// removes the node).
    Emit(Vec<Staged>),
}

/// Mirrors [`LoweringDecl`](flm_core::core::lowering::LoweringDecl): what a
/// lowering rule declares about itself. What differs: names are strings; a
/// table is named by its name alone, and the adapter gives it the value type
/// the registry declares for that name once every feature has registered (a
/// table no feature declares is read as an extension's own); an option type
/// is named by its name, as a feature's `settings.options` declares it
/// ([`LoweringDecl::option_named`](flm_core::core::lowering::LoweringDecl::option_named)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoweringDecl {
    /// The capabilities the rule requires.
    pub requires: Vec<String>,
    /// The tables the rule reads, which must be written.
    pub reads: Vec<String>,
    /// The tables the rule reads if the run holds them.
    pub reads_opt: Vec<String>,
    /// The option types the rule reads, by name.
    pub options: Vec<String>,
    /// Every presentation element the rule may build.
    pub produces: Vec<String>,
    /// The requirements the rule records whenever it is selected.
    pub selection_requires: Vec<String>,
}

impl LoweringDecl {
    /// Mirrors [`LoweringDecl::new`](flm_core::core::lowering::LoweringDecl::new):
    /// a declaration that needs nothing, produces nothing and records no
    /// requirement.
    pub fn new() -> LoweringDecl {
        LoweringDecl::default()
    }

    /// Mirrors [`LoweringDecl::requires`](flm_core::core::lowering::LoweringDecl::requires).
    pub fn requires(mut self, capability: impl Into<String>) -> LoweringDecl {
        self.requires.push(capability.into());
        self
    }

    /// Mirrors [`LoweringDecl::reads`](flm_core::core::lowering::LoweringDecl::reads).
    /// What differs: the table by its name.
    pub fn reads(mut self, table: impl Into<String>) -> LoweringDecl {
        self.reads.push(table.into());
        self
    }

    /// Mirrors [`LoweringDecl::reads_opt`](flm_core::core::lowering::LoweringDecl::reads_opt).
    /// What differs: the table by its name.
    pub fn reads_opt(mut self, table: impl Into<String>) -> LoweringDecl {
        self.reads_opt.push(table.into());
        self
    }

    /// Mirrors [`LoweringDecl::option`](flm_core::core::lowering::LoweringDecl::option).
    /// What differs: the option type is named by its name
    /// ([`LoweringDecl::option_named`](flm_core::core::lowering::LoweringDecl::option_named)),
    /// which names an option type declared from data; the two standard option
    /// types the `math` library's LaTeX parts read,
    /// `MathRecomposeOptions` and `LatexRecomposedRenderOptions`
    /// ([`api::stdcore::features::math`](crate::api::stdcore::features::math)),
    /// are named by those names and declared as their typed reads, so that a
    /// rule that calls the parts declares them here. Their values are read by
    /// the parts, not by [`Lowerer::option`].
    pub fn option(mut self, name: impl Into<String>) -> LoweringDecl {
        self.options.push(name.into());
        self
    }

    /// Mirrors [`LoweringDecl::produces`](flm_core::core::lowering::LoweringDecl::produces).
    pub fn produces(mut self, element: impl Into<String>) -> LoweringDecl {
        self.produces.push(element.into());
        self
    }

    /// Mirrors [`LoweringDecl::selection_requires`](flm_core::core::lowering::LoweringDecl::selection_requires).
    pub fn selection_requires(mut self, id: impl Into<String>) -> LoweringDecl {
        self.selection_requires.push(id.into());
        self
    }
}

/// Mirrors [`LoweringRule`](flm_core::core::lowering::LoweringRule): a
/// lowering rule, ready to be registered.
///
/// What differs: an opaque value, read through the methods below, which holds
/// either a rule an extension built with [`LoweringRule::new`] or one a helper
/// made ([`expand_in_tree_rule_for`](crate::api::stdcore::features::userdefs::expand_in_tree_rule_for));
/// [`FeatureBuilder::lowering_rule`](crate::api::core::registry::FeatureBuilder::lowering_rule)
/// takes both.
#[derive(Clone)]
pub struct LoweringRule {
    pub(crate) form: RuleForm,
}

/// What a [`LoweringRule`] holds.
#[derive(Clone)]
pub(crate) enum RuleForm {
    /// A rule a helper made, as `flm-core` built it.
    Core(CoreLoweringRule),
    /// A rule an extension built, whose body is the extension's.
    Ext(ExtRule),
}

/// The parts of a rule an extension built.
#[derive(Clone)]
pub(crate) struct ExtRule {
    pub(crate) name: String,
    pub(crate) element: String,
    pub(crate) decl: LoweringDecl,
    pub(crate) body: Arc<dyn Lower>,
    pub(crate) mark: DefaultMark,
    pub(crate) docs: String,
}

impl LoweringRule {
    /// Mirrors [`LoweringRule::new`](flm_core::core::lowering::LoweringRule::new):
    /// the rule `name`, which lowers `element` through `body`, with the
    /// declaration `decl`, the default mark `mark` and no description. What
    /// differs: the body is the extension API's [`Lower`]; the adapter wraps it
    /// for `flm-core` when the rule is registered.
    pub fn new(
        name: impl Into<String>,
        element: impl Into<String>,
        decl: LoweringDecl,
        body: Arc<dyn Lower>,
        mark: DefaultMark,
    ) -> LoweringRule {
        LoweringRule {
            form: RuleForm::Ext(ExtRule {
                name: name.into(),
                element: element.into(),
                decl,
                body,
                mark,
                docs: String::new(),
            }),
        }
    }

    /// Mirrors [`LoweringRule::docs`](flm_core::core::lowering::LoweringRule::docs):
    /// sets the rule's one-line description, and returns the rule.
    pub fn docs(self, docs: impl Into<String>) -> LoweringRule {
        let form = match self.form {
            RuleForm::Core(rule) => RuleForm::Core(rule.docs(docs)),
            RuleForm::Ext(rule) => RuleForm::Ext(ExtRule { docs: docs.into(), ..rule }),
        };
        LoweringRule { form }
    }

    /// The rule's name: `flm-core`'s field `name`.
    pub fn name(&self) -> &str {
        match &self.form {
            RuleForm::Core(rule) => rule.name.as_str(),
            RuleForm::Ext(rule) => &rule.name,
        }
    }

    /// The element the rule lowers: `flm-core`'s field `element`.
    pub fn element(&self) -> &str {
        match &self.form {
            RuleForm::Core(rule) => rule.element.as_str(),
            RuleForm::Ext(rule) => &rule.element,
        }
    }

    /// The rule's default mark: `flm-core`'s field `default`.
    pub fn mark(&self) -> &DefaultMark {
        match &self.form {
            RuleForm::Core(rule) => &rule.default,
            RuleForm::Ext(rule) => &rule.mark,
        }
    }

    /// The rule's one-line description: `flm-core`'s field `docs`.
    pub fn description(&self) -> &str {
        match &self.form {
            RuleForm::Core(rule) => &rule.docs,
            RuleForm::Ext(rule) => &rule.docs,
        }
    }

    /// The rule a helper made, as `flm-core` built it.
    pub(crate) fn from_core(rule: CoreLoweringRule) -> LoweringRule {
        LoweringRule { form: RuleForm::Core(rule) }
    }
}

impl fmt::Debug for LoweringRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.form {
            RuleForm::Core(rule) => rule.fmt(f),
            RuleForm::Ext(rule) => f
                .debug_struct("LoweringRule")
                .field("name", &rule.name)
                .field("element", &rule.element)
                .field("decl", &rule.decl)
                .field("default", &rule.mark)
                .finish(),
        }
    }
}

// --- the lowering façade -----------------------------------------------------------------------

/// Mirrors [`LowerCx`](flm_core::core::lowering::LowerCx), the presentation
/// tree under construction: what every [`Lowerer`] method that builds
/// something takes.
///
/// What differs: opaque; it is passed to the [`Lowerer`]'s methods and nothing
/// else, and only the context lent to the running call is accepted, as the
/// expand façade's [`ExpandCx`](crate::api::lang::rewrites::userdefs::ExpandCx)
/// is.
///
/// The trait is sealed: the adapter alone implements it.
pub trait LowerCx: LowerCxCore {}

/// Mirrors [`Lowerer`](flm_core::core::lowering::Lowerer): the interface a
/// rule's body works through, lent for one call.
///
/// What differs: names are strings, tree ids their display text (`tree#0`,
/// `value#1`); a staged node is a [`Staged`] value, and a method that stages
/// accepts only the [`LowerCx`] lent to the running call and, as content,
/// only nodes the same call staged, each placed once ([`ExtError::Argument`]
/// otherwise); a diagnostic is the extension API's [`Diagnostic`]; errors are
/// [`ExtError`], and a `flm-core` error is [`ExtError::Core`] of kind `lower`
/// (see [`Lower`] for what then follows); [`lang`](Lowerer::lang) answers
/// the built language as a façade lent for the call. Left out: `layout()` (the region layout serves a helper of the
/// block structure that has no mirror); the type-erased `get_erased` and
/// `option_erased`, whose by-name forms [`get`](Lowerer::get) and
/// [`option`](Lowerer::option) replace them; the typed helpers `pres_with`,
/// `pres_slots_with`, `error`, `warning` and `note` of `dyn Lowerer`, which
/// [`pres`](Lowerer::pres), [`pres_slots`](Lowerer::pres_slots) and
/// [`diagnose`](Lowerer::diagnose) cover.
///
/// The trait is sealed: the adapter alone implements it.
pub trait Lowerer: Sealed + LowererCore {
    /// Mirrors [`Lowerer::rule`](flm_core::core::lowering::Lowerer::rule).
    fn rule(&self) -> String;

    /// Mirrors [`Lowerer::lang`](flm_core::core::lowering::Lowerer::lang):
    /// the language the registry was built with. What differs: a façade with
    /// no method of its own, borrowing the lowerer, which the functions over
    /// the built language take
    /// ([`TemplateLanguage::new`](crate::api::lang::rewrites::userdefs::TemplateLanguage::new),
    /// [`compile_template_with`](crate::api::lang::rewrites::userdefs::compile_template_with));
    /// it carries the durable table of the rule's feature.
    fn lang(&self) -> Box<dyn BuiltLang + '_>;

    /// Mirrors [`Lowerer::backend`](flm_core::core::lowering::Lowerer::backend).
    fn backend(&self) -> String;

    /// Mirrors [`Lowerer::comment_start`](flm_core::core::lowering::Lowerer::comment_start).
    fn comment_start(&self) -> Option<String>;

    /// Mirrors [`Lowerer::tree_id`](flm_core::core::lowering::Lowerer::tree_id):
    /// the tree being lowered. What differs: the id's display text.
    fn tree_id(&self) -> String;

    /// Mirrors [`Lowerer::addr`](flm_core::core::lowering::Lowerer::addr): the
    /// address of the node being lowered. What differs: nothing here; across a
    /// binding the address is data (`{"tree", "node"}`) that nothing takes
    /// back, as a walk's addresses are.
    fn addr(&self) -> NodeAddr;

    /// Mirrors [`Lowerer::addr_of`](flm_core::core::lowering::Lowerer::addr_of).
    ///
    /// # Errors
    ///
    /// `node` belongs to another tree ([`ExtError::Core`] of kind `lower`).
    fn addr_of(&self, node: &dyn Node<'_>) -> Result<NodeAddr, ExtError>;

    /// Mirrors [`Lowerer::placement`](flm_core::core::lowering::Lowerer::placement).
    /// What differs: the tree id's display text.
    fn placement(&self) -> Option<String>;

    /// Mirrors `get`, defined on `dyn Lowerer` over
    /// [`Lowerer::get_erased`](flm_core::core::lowering::Lowerer::get_erased):
    /// the table named `name`, which the rule declared it reads. What differs:
    /// the table by its name, answered as a [`TableValue`] by its type, as
    /// [`StageCx::get`](crate::api::core::run::StageCx::get) answers it; in a
    /// standard table's projection, a node of the tree being lowered renders
    /// as `elem@line:col` and any other node as `?@<tree>/<node index>`.
    ///
    /// # Errors
    ///
    /// The rule did not declare the read ([`ExtError::Core`] of kind
    /// `access`); a projection holds a value that cannot cross
    /// ([`ExtError::Value`]).
    fn get(&self, name: &str) -> Result<Option<TableValue<'_>>, ExtError>;

    /// Mirrors [`Lowerer::producer_of`](flm_core::core::lowering::Lowerer::producer_of).
    /// What differs: names are strings.
    fn producer_of(&self, table: &str) -> Option<String>;

    /// Mirrors `option_named`, defined on `dyn Lowerer`: the value of the
    /// option type `name` the rule declared it reads
    /// ([`LoweringDecl::option`]). What differs: the value comes as data.
    ///
    /// # Errors
    ///
    /// The rule did not declare the read ([`ExtError::Core`] of kind
    /// `option`).
    fn option(&self, name: &str) -> Result<Value, ExtError>;

    /// Mirrors [`Lowerer::lower_children`](flm_core::core::lowering::Lowerer::lower_children).
    ///
    /// # Errors
    ///
    /// `cx` was not lent to this call ([`ExtError::Argument`]); `flm-core`
    /// refuses ([`ExtError::Core`] of kind `lower`).
    fn lower_children(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::lower_body`](flm_core::core::lowering::Lowerer::lower_body).
    ///
    /// # Errors
    ///
    /// As [`lower_children`](Lowerer::lower_children).
    fn lower_body(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::lower_arg`](flm_core::core::lowering::Lowerer::lower_arg).
    ///
    /// # Errors
    ///
    /// As [`lower_children`](Lowerer::lower_children).
    fn lower_arg(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>, index: usize) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::lower_arg_named`](flm_core::core::lowering::Lowerer::lower_arg_named).
    ///
    /// # Errors
    ///
    /// As [`lower_children`](Lowerer::lower_children).
    fn lower_arg_named(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>, name: &str) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::lower_nodes`](flm_core::core::lowering::Lowerer::lower_nodes).
    /// What differs: the run of siblings is a list of nodes, which must follow
    /// one another in one tree.
    ///
    /// # Errors
    ///
    /// The nodes are not such a run, or `cx` was not lent to this call
    /// ([`ExtError::Argument`]); `flm-core` refuses ([`ExtError::Core`] of
    /// kind `lower`).
    fn lower_nodes<'t>(&mut self, cx: &mut dyn LowerCx, nodes: &[Box<dyn Node<'t> + 't>]) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::default`](flm_core::core::lowering::Lowerer::default).
    ///
    /// # Errors
    ///
    /// As [`lower_children`](Lowerer::lower_children).
    fn default(&mut self, cx: &mut dyn LowerCx, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::pres`](flm_core::core::lowering::Lowerer::pres):
    /// stages a node of the presentation element `element` carrying the
    /// payload `payload` and holding `children`. What differs: the payload is
    /// data, the projection of the element's payload type, read back through
    /// that type's `from_projection`; the elements whose payload can be read
    /// so are those of the standard presentation vocabulary
    /// ([`pres::names::ALL`](flm_core::stdcore::backends::pres::names::ALL))
    /// and the HTML construct `flm.pres.html.tag` with children (its markup
    /// body is not offered).
    ///
    /// # Errors
    ///
    /// `element` is not one of those elements, the payload is not of its
    /// shape, a child was not staged by this call or is placed twice, or `cx`
    /// was not lent to this call ([`ExtError::Argument`]); `flm-core` refuses
    /// ([`ExtError::Core`] of kind `lower`).
    fn pres(&mut self, cx: &mut dyn LowerCx, element: &str, payload: &Value, children: Vec<Staged>) -> Result<Staged, ExtError>;

    /// Mirrors [`Lowerer::pres_slots`](flm_core::core::lowering::Lowerer::pres_slots).
    /// What differs: as [`pres`](Lowerer::pres).
    ///
    /// # Errors
    ///
    /// As [`pres`](Lowerer::pres).
    fn pres_slots(
        &mut self,
        cx: &mut dyn LowerCx,
        element: &str,
        payload: &Value,
        slots: Vec<(String, Vec<Staged>)>,
    ) -> Result<Staged, ExtError>;

    /// Mirrors [`Lowerer::text`](flm_core::core::lowering::Lowerer::text).
    ///
    /// # Errors
    ///
    /// As [`lower_children`](Lowerer::lower_children).
    fn text(&mut self, cx: &mut dyn LowerCx, text: &str) -> Result<Staged, ExtError>;

    /// Mirrors [`Lowerer::splice`](flm_core::core::lowering::Lowerer::splice).
    /// What differs: the synthesized value is the root node of its tree (a
    /// tree a table answered); the adapter splices a copy of that tree.
    ///
    /// # Errors
    ///
    /// `value` is not the root of its tree, or `cx` was not lent to this call
    /// ([`ExtError::Argument`]); `flm-core` refuses ([`ExtError::Core`] of
    /// kind `lower`).
    fn splice(&mut self, cx: &mut dyn LowerCx, value: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::lower_region`](flm_core::core::lowering::Lowerer::lower_region).
    /// What differs: no shape function; the region's content is lowered as
    /// one run of siblings, as `flm-core`'s
    /// [`lower_region_content`](flm_core::stdcore::backends::lowering::lower_region_content)
    /// does. A binding does not offer this method, since an address crosses
    /// as data that nothing takes back.
    ///
    /// # Errors
    ///
    /// `cx` was not lent to this call ([`ExtError::Argument`]); `flm-core`
    /// refuses ([`ExtError::Core`] of kind `lower`).
    fn lower_region(&mut self, cx: &mut dyn LowerCx, at: NodeAddr, region: NodeRegion) -> Result<Vec<Staged>, ExtError>;

    /// Mirrors [`Lowerer::require`](flm_core::core::lowering::Lowerer::require).
    /// What differs: the identifier is a string.
    fn require(&mut self, id: &str);

    /// Mirrors [`Lowerer::diagnose`](flm_core::core::lowering::Lowerer::diagnose):
    /// reports `diagnostic`, at the node being lowered unless it has a place
    /// of its own. What differs: the diagnostic is data, and the method
    /// answers a `Result`, since it checks the diagnostic's identifier.
    ///
    /// # Errors
    ///
    /// The diagnostic's identifier does not begin with the extension's stem
    /// and a dot ([`ExtError::NameOutsideExtension`]), or its data cannot
    /// cross ([`ExtError::Value`]); nothing is reported then.
    fn diagnose(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError>;
}

// --- a tree staged in one call -----------------------------------------------------------------

/// A tree of presentation nodes written as data, for [`pres_tree`]: no
/// `flm-core` counterpart.
///
/// Across a binding: `{"element", "payload", "children": [...]}` for a node,
/// `{"build": <staged node>}` for a node the body already staged,
/// `{"text": "…"}` for characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresTree {
    /// A presentation node: its element, its payload (as
    /// [`Lowerer::pres`] takes it) and its children.
    Node {
        /// The presentation element.
        element: String,
        /// The payload, as data.
        payload: Value,
        /// The children, in order.
        children: Vec<PresTree>,
    },
    /// A node the body staged before (lowered content), placed here.
    Build(Staged),
    /// Characters, staged as [`Lowerer::text`] stages them.
    Text(String),
}

/// No `flm-core` counterpart: stages the tree `tree` in one call, through
/// [`Lowerer::pres`] and [`Lowerer::text`], children first, and answers the
/// staged root. A [`PresTree::Build`] is answered as it is.
///
/// A binding calls it once for a tree of tens or hundreds of nodes where one
/// call per node would make the crossings the cost of lowering.
///
/// # Errors
///
/// The first error of [`Lowerer::pres`] or [`Lowerer::text`]; what was staged
/// before it stays staged and unplaced.
pub fn pres_tree(lw: &mut dyn Lowerer, cx: &mut dyn LowerCx, tree: &PresTree) -> Result<Staged, ExtError> {
    match tree {
        PresTree::Build(staged) => Ok(*staged),
        PresTree::Text(text) => lw.text(cx, text),
        PresTree::Node { element, payload, children } => {
            let mut staged = Vec::with_capacity(children.len());
            for child in children {
                staged.push(pres_tree(lw, cx, child)?);
            }
            lw.pres(cx, element, payload, staged)
        }
    }
}

fn _object_safe(_: &dyn Lower, _: &dyn LowerCx, _: &dyn Lowerer) {}
