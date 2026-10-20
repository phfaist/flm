//! Mirrors [`flm_core::core::stage`]: the stage trait, its declaration, its
//! needs, the rewrite stage trait and its declaration, and the table of a
//! rewrite's trees.

use crate::api::core::data::store::StageOut;
use crate::api::core::run::{RewriteCx, StageCx};
use crate::api::techy::node::Node;
use crate::api::techy::transform::{Restage, Restager};
use crate::runtime::ExtError;
use crate::sealed::Sealed;

pub use flm_core::core::stage::{Effect, Scope};
// The severity of a diagnostic, whose own module is `techy/error`: the
// prelude module `flm:core/stage` exports it beside the stage's enums.
pub use crate::api::techy::error::Severity;

/// Mirrors [`Stage`](flm_core::core::stage::Stage): one unit of work of a run,
/// which an extension implements and registers with
/// [`FeatureBuilder::stage`](crate::api::core::registry::FeatureBuilder::stage).
///
/// What differs: [`needs`](Stage::needs) takes no plan context and, like
/// [`decl`](Stage::decl), is read once, when the stage is registered;
/// [`run`](Stage::run) returns a `Result`, and an `Err` is the handler's
/// failure, which the adapter reports as an error diagnostic at the stage.
pub trait Stage: Send + Sync {
    /// Mirrors [`Stage::decl`](flm_core::core::stage::Stage::decl): the stage's
    /// name, effect, scope, and what it provides and writes.
    fn decl(&self) -> StageDecl;

    /// Mirrors [`Stage::needs`](flm_core::core::stage::Stage::needs): what the
    /// stage requires and reads. What differs: no plan context; read once at
    /// registration.
    fn needs(&self) -> Needs;

    /// Mirrors [`Stage::run`](flm_core::core::stage::Stage::run): the stage's
    /// work, reading through `cx` and writing through `out`. What differs: both
    /// borrows share one lifetime, the call's `'c`, so that a binding can lend
    /// them into one [`CallTable<'c>`](crate::adapter::handles::CallTable)
    /// (plan A, A18).
    ///
    /// # Errors
    ///
    /// The handler's failure. What the stage wrote before failing stays
    /// written.
    fn run<'c>(&self, cx: &'c dyn StageCx, out: &'c mut dyn StageOut) -> Result<(), ExtError>;
}

/// Mirrors [`StageDecl`](flm_core::core::stage::StageDecl): a stage's
/// declaration. What differs: table and capability names are strings; every
/// table in [`writes`](StageDecl::writes) is a table of the extension's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDecl {
    /// The stage's name.
    pub name: String,
    /// What the stage's effects are.
    pub effect: Effect,
    /// Whether the stage runs once per tree or once for the document.
    pub scope: Scope,
    /// The capabilities the stage provides.
    pub provides: Vec<String>,
    /// The tables the stage writes.
    pub writes: Vec<String>,
    /// Documentation prose about the stage.
    pub docs: String,
}

impl StageDecl {
    /// Mirrors [`StageDecl::new`](flm_core::core::stage::StageDecl::new).
    pub fn new(name: impl Into<String>, effect: Effect, scope: Scope) -> StageDecl {
        StageDecl {
            name: name.into(),
            effect,
            scope,
            provides: Vec::new(),
            writes: Vec::new(),
            docs: String::new(),
        }
    }

    /// Mirrors [`StageDecl::provides`](flm_core::core::stage::StageDecl::provides).
    pub fn provides(mut self, capability: impl Into<String>) -> StageDecl {
        self.provides.push(capability.into());
        self
    }

    /// Mirrors [`StageDecl::writes`](flm_core::core::stage::StageDecl::writes).
    /// What differs: the table by its name.
    pub fn writes(mut self, table: impl Into<String>) -> StageDecl {
        self.writes.push(table.into());
        self
    }

    /// Mirrors [`StageDecl::docs`](flm_core::core::stage::StageDecl::docs).
    pub fn docs(mut self, docs: impl Into<String>) -> StageDecl {
        self.docs = docs.into();
        self
    }
}

/// Mirrors [`Needs`](flm_core::core::stage::Needs): what a stage requires and
/// reads. What differs: names are strings; an option type is named by its
/// name, as a feature's settings declare it from data
/// ([`Needs::option_named`](flm_core::core::stage::Needs::option_named)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Needs {
    /// The capabilities the stage requires.
    pub requires: Vec<String>,
    /// The tables the stage reads, which must be written.
    pub reads: Vec<String>,
    /// The tables the stage reads if they are written.
    pub reads_opt: Vec<String>,
    /// The capabilities the stage runs after, if they are provided.
    pub after: Vec<String>,
    /// The capabilities the stage runs before, if they are provided.
    pub before: Vec<String>,
    /// The option types the stage reads, by name.
    pub options: Vec<String>,
}

impl Needs {
    /// Mirrors [`Needs::new`](flm_core::core::stage::Needs::new): needs nothing.
    pub fn new() -> Needs {
        Needs::default()
    }

    /// Mirrors [`Needs::requires`](flm_core::core::stage::Needs::requires).
    pub fn requires(mut self, capability: impl Into<String>) -> Needs {
        self.requires.push(capability.into());
        self
    }

    /// Mirrors [`Needs::reads`](flm_core::core::stage::Needs::reads). What
    /// differs: the table by its name.
    pub fn reads(mut self, table: impl Into<String>) -> Needs {
        self.reads.push(table.into());
        self
    }

    /// Mirrors [`Needs::reads_opt`](flm_core::core::stage::Needs::reads_opt).
    /// What differs: the table by its name.
    pub fn reads_opt(mut self, table: impl Into<String>) -> Needs {
        self.reads_opt.push(table.into());
        self
    }

    /// Mirrors [`Needs::after`](flm_core::core::stage::Needs::after).
    pub fn after(mut self, capability: impl Into<String>) -> Needs {
        self.after.push(capability.into());
        self
    }

    /// Mirrors [`Needs::before`](flm_core::core::stage::Needs::before).
    pub fn before(mut self, capability: impl Into<String>) -> Needs {
        self.before.push(capability.into());
        self
    }

    /// Mirrors [`Needs::option`](flm_core::core::stage::Needs::option). What
    /// differs: the option type is named by its name, as a feature's
    /// `settings.options` declares it
    /// ([`Needs::option_named`](flm_core::core::stage::Needs::option_named)).
    pub fn option(mut self, name: impl Into<String>) -> Needs {
        self.options.push(name.into());
        self
    }
}

/// Mirrors [`RewriteStage`](flm_core::core::stage::RewriteStage): a stage that
/// runs before the document's trees are frozen and turns one tree into
/// another, which an extension implements; it publishes its trees in a
/// [`TreesTable`] named after the capability it provides.
///
/// What differs: the handler is techy's per-node visitor
/// ([`RestageVisitor`](flm_core::techy::transform::RestageVisitor)) in place
/// of `flm-core`'s `rewrite(tree_id, tree, cx, diags)`: the adapter drives
/// the tree, calling [`restage`](RewriteStage::restage) once for every node
/// it reaches, from the root downward, and the handler decides, node by
/// node, whether the node is kept ([`Restage::Descend`]) or replaced by
/// nodes it staged ([`Restage::Emit`]); the finished tree is the driver's.
/// [`needs`](RewriteStage::needs) takes no plan context and, like
/// [`decl`](RewriteStage::decl), is read once; the diagnostics go through
/// [`Restager::diagnose`].
///
/// What a failure leaves behind:
///
/// - The handler fails at a node (an `Err`, an [`Emit`](Restage::Emit)
///   naming a node this call did not stage or naming one twice, a stale
///   handle): the adapter reports the Error `flm.ext.handler-failed` at the
///   node, whose message names the stage, and the node is kept as a node the
///   handler did not touch, copied over its restaged children. The rest of
///   the tree is rewritten normally; the nodes the handler staged before
///   failing are dropped.
/// - A [`Restager`] method fails with an error that leaves the rewrite unable
///   to go on (a refusal of the output builder, a dropped content parent,
///   the descent guard's refusal): the call ends with that error, whatever
///   the handler answers next, and the tree's rewrite is abandoned. The tree
///   is then absent from every later table and from the frozen trees, as for
///   a `flm-core` rewrite stage that produces no tree, and an Error is
///   reported for it.
pub trait RewriteStage: Send + Sync {
    /// Mirrors [`RewriteStage::decl`](flm_core::core::stage::RewriteStage::decl):
    /// the stage's name, the capability it publishes, and what that
    /// capability refines.
    fn decl(&self) -> RewriteDecl;

    /// Mirrors [`RewriteStage::needs`](flm_core::core::stage::RewriteStage::needs):
    /// what the stage requires, the stages it runs after or before, and the
    /// option types it reads. What differs: no plan context; read once at
    /// registration. A table read, plain or optional, is refused when the
    /// plan is built, as for `flm-core`'s rewrite stages.
    fn needs(&self) -> Needs;

    /// Mirrors techy's
    /// [`RestageVisitor::restage`](flm_core::techy::transform::RestageVisitor::restage)
    /// over the tree `flm-core`'s
    /// [`RewriteStage::rewrite`](flm_core::core::stage::RewriteStage::rewrite)
    /// rewrites: decides what becomes of `node` in the output tree. What
    /// differs: the node, the context and the restage façade are the
    /// extension API's, lent for one call; the three borrows share one
    /// lifetime, the call's `'c`, so that a binding can lend all three into
    /// one [`CallTable<'c>`](crate::adapter::handles::CallTable), as for
    /// [`Lower::lower`](crate::api::core::lowering::Lower::lower).
    ///
    /// # Errors
    ///
    /// The handler's failure; see the trait's documentation for what follows.
    fn restage<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c dyn RewriteCx,
        rs: &'c mut dyn Restager,
    ) -> Result<Restage, ExtError>;
}

/// Mirrors [`RewriteDecl`](flm_core::core::stage::RewriteDecl): what a
/// rewrite stage declares about itself. What differs: the stage's and the
/// capabilities' names are strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteDecl {
    /// The rewrite stage's name.
    pub name: String,
    /// The capability the rewrite stage provides, which is also the name of
    /// the [`TreesTable`] in which its output is published.
    pub publishes: String,
    /// The parent capability that [`publishes`](RewriteDecl::publishes)
    /// refines, if any.
    pub refines: Option<String>,
    /// Whether [`publishes`](RewriteDecl::publishes) is the refinement the
    /// plan chooses among its parent's refinements when nothing else decides.
    pub default: bool,
    /// A one-line description of the rewrite stage, which is also the
    /// description of the trees table it publishes.
    pub docs: String,
}

impl RewriteDecl {
    /// Mirrors [`RewriteDecl::new`](flm_core::core::stage::RewriteDecl::new):
    /// the declaration of the rewrite stage `name` publishing the capability
    /// `publishes`, refining no capability and not marked as the default.
    pub fn new(name: impl Into<String>, publishes: impl Into<String>) -> RewriteDecl {
        RewriteDecl {
            name: name.into(),
            publishes: publishes.into(),
            refines: None,
            default: false,
            docs: String::new(),
        }
    }

    /// Mirrors [`RewriteDecl::refines`](flm_core::core::stage::RewriteDecl::refines).
    pub fn refines(mut self, parent: impl Into<String>) -> RewriteDecl {
        self.refines = Some(parent.into());
        self
    }

    /// Mirrors [`RewriteDecl::default`](flm_core::core::stage::RewriteDecl::default).
    pub fn default(mut self) -> RewriteDecl {
        self.default = true;
        self
    }

    /// Mirrors [`RewriteDecl::docs`](flm_core::core::stage::RewriteDecl::docs).
    pub fn docs(mut self, docs: impl Into<String>) -> RewriteDecl {
        self.docs = docs.into();
        self
    }
}

/// Mirrors [`TreesTable`](flm_core::core::stage::TreesTable): a rewrite's
/// output tree for each tree it ran on. What differs: a façade over the table,
/// reading its public field `trees`; tree ids are their display text and each
/// tree is its root node. `'t` is the lifetime of the trees, as for
/// [`Node`].
///
/// The trait is sealed: the adapter alone implements it.
pub trait TreesTable<'t>: Sealed {
    /// The table's trees, by id, in id order.
    fn trees(&self) -> Vec<(String, Box<dyn Node<'t> + 't>)>;

    /// The root of the tree of id `id`, if the table holds it.
    fn tree(&self, id: &str) -> Option<Box<dyn Node<'t> + 't>>;
}

fn _object_safe(_: &dyn Stage, _: &dyn RewriteStage, _: &dyn TreesTable<'_>) {}
