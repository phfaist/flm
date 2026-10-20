//! Handles: what a handler in a scripting language holds in place of a Rust
//! reference.
//!
//! A handler runs inside an `flm-core` call that lends it objects with
//! lifetimes: the use site and the expander during an expansion, the context
//! and the output during a stage's run, the builder during `register`. Rust
//! code against the extension API receives them as `&dyn Node`, `&mut dyn
//! Expander` and so on. A script object cannot carry a lifetime, so a binding
//! lends each object as a [`Handle`]: an index into a [`CallTable`] that lives
//! for one call.
//!
//! A handle is valid in the call that lent it, while that call is the
//! innermost running one: a binding keeps a stack of tables and hands the
//! innermost one to [`dispatch::call`](crate::adapter::dispatch::call), so a
//! handle of a finished or of an outer call is
//! [`ExtError::StaleHandle`].
//!
//! The free functions of the extension API are dispatched on *module handles*
//! ([`module_handle`]), which are valid in every call.

use std::sync::Arc;

use crate::adapter::durable::DurableTable;
use crate::api;
use crate::runtime::ExtError;
use crate::sealed::Token;

/// A handle: which table lent it (`call`), what it is (`kind`), where
/// (`index`). Its serde form is the map `{"kind", "call", "index"}`; across a
/// JSON binding a handle is written `{"@handle": {"kind", "call", "index"}}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Handle {
    /// What the handle stands for.
    pub kind: HandleKind,
    /// The number of the call whose table lent it; 0 for a module handle.
    pub call: u64,
    /// Its position in that table, or in [`MODULE_PATHS`] for a module handle.
    pub index: u32,
}

/// Every kind of object a handle may stand for: one variant per façade trait
/// of the extension API, the objects a binding lends *into* the table (a
/// [`Stage`](api::core::stage::Stage) or an
/// [`Expand`](api::lang::rewrites::userdefs::Expand) or a
/// [`Lower`](api::core::lowering::Lower) of its own, a
/// [`LoweringRule`](api::core::lowering::LoweringRule) or an
/// [`ElementDef`](api::lang::mint::ElementDef) a constructor or a helper
/// produced), and the module handles the free functions dispatch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleKind {
    /// A [`FeatureBuilder`](api::core::registry::FeatureBuilder).
    FeatureBuilder,
    /// A [`StageCx`](api::core::run::StageCx).
    StageCx,
    /// A [`StageOut`](api::core::data::store::StageOut).
    StageOut,
    /// A [`TablePiece`](api::core::data::store::TablePiece).
    TablePiece,
    /// An [`ExtTableRead`](api::core::data::table::ExtTableRead).
    ExtTableRead,
    /// A [`TreesTable`](api::core::stage::TreesTable).
    TreesTable,
    /// A [`Node`](api::techy::node::Node).
    Node,
    /// A [`KeyVals`](api::techy::extract::KeyVals).
    KeyVals,
    /// A [`KeyValEntry`](api::techy::extract::KeyValEntry).
    KeyValEntry,
    /// An [`ExpandCx`](api::lang::rewrites::userdefs::ExpandCx).
    ExpandCx,
    /// An [`Expander`](api::lang::rewrites::userdefs::Expander).
    Expander,
    /// A [`UserDefinition`](api::lang::rewrites::userdefs::UserDefinition).
    UserDefinition,
    /// A [`ModeVariant`](api::lang::rewrites::userdefs::ModeVariant).
    ModeVariant,
    /// A [`CompiledTemplate`](api::lang::rewrites::userdefs::CompiledTemplate).
    CompiledTemplate,
    /// A [`PlaceholderBindings`](api::lang::rewrites::userdefs::PlaceholderBindings).
    PlaceholderBindings,
    /// A [`Staged`](api::lang::rewrites::userdefs::Staged) node.
    Staged,
    /// A [`Stage`](api::core::stage::Stage) the binding lent in.
    Stage,
    /// An [`Expand`](api::lang::rewrites::userdefs::Expand) the binding lent in.
    Expand,
    /// A [`LoweringRule`](api::core::lowering::LoweringRule) a constructor
    /// or a helper made.
    LoweringRule,
    /// A [`LowerCx`](api::core::lowering::LowerCx).
    LowerCx,
    /// A [`Lowerer`](api::core::lowering::Lowerer).
    Lowerer,
    /// A [`Lower`](api::core::lowering::Lower) the binding lent in.
    Lower,
    /// An [`ElementDef`](api::lang::mint::ElementDef) a helper made.
    ElementDef,
    /// A [`RewriteStage`](api::core::stage::RewriteStage) the binding lent in.
    RewriteStage,
    /// A [`RewriteCx`](api::core::run::RewriteCx).
    RewriteCx,
    /// A [`Restager`](api::techy::transform::Restager).
    Restager,
    /// A [`RestagedArgument`](api::techy::transform::RestagedArgument) bundle.
    RestagedArgument,
    /// A [`RestagedSlot`](api::techy::transform::RestagedSlot) bundle.
    RestagedSlot,
    /// A [`LoweredTrees`](api::stdcore::backends::lowering::LoweredTrees)
    /// table, read.
    LoweredTrees,
    /// A [`LibraryStage`](api::core::registry::LibraryStage) a constructor
    /// made ([`LoweringStage::new`](api::stdcore::backends::lowering::LoweringStage::new)).
    LibraryStage,
    /// A [`HostService`](api::host::HostService).
    HostService,
    /// A [`Files`](api::host::Files) service.
    Files,
    /// A [`CitationProvider`](api::stdcore::features::citations::CitationProvider)
    /// the binding lent in.
    CitationProvider,
    /// A [`CitationEntry`](api::stdcore::features::citations::CitationEntry).
    CitationEntry,
    /// A [`ProviderCx`](api::stdcore::features::citations::ProviderCx).
    ProviderCx,
    /// A [`TemplateModule`](api::stdcore::features::compose::TemplateModule)
    /// the binding lent in.
    TemplateModule,
    /// A [`TemplateCx`](api::stdcore::features::compose::TemplateCx).
    TemplateCx,
    /// The module `api::<path>`, on which its free functions are dispatched
    /// (one of [`MODULE_PATHS`]). Module handles are not lent:
    /// `Handle { kind: Module, call: 0, index: <position in MODULE_PATHS> }` is
    /// valid in every call.
    Module,
    /// A [`Referenceable`](api::model::aspects::Referenceable) the binding
    /// lent in.
    Referenceable,
    /// A [`Sectioning`](api::model::aspects::Sectioning) the binding lent in.
    Sectioning,
    /// An [`ArgDef`](api::lang::mint::ArgDef) a helper made
    /// ([`label_fields_arg`](api::stdcore::features::refs::label_fields_arg)).
    ArgDef,
    /// A [`BuiltLang`](api::lang::BuiltLang).
    BuiltLang,
    /// An [`FlmDiagnostics`](api::lang::FlmDiagnostics).
    FlmDiagnostics,
    /// A [`LateInit`](api::core::registry::LateInit) the binding lent in.
    LateInit,
    /// A [`Finalizer`](api::core::registry::Finalizer) the binding lent in.
    Finalizer,
    /// A [`Counters`](api::stdcore::features::numbering::Counters) table.
    Counters,
    /// An [`Anchors`](api::stdcore::features::refs::Anchors) table.
    Anchors,
    /// A [`TemplateLanguage`](api::lang::rewrites::userdefs::TemplateLanguage),
    /// resolved for the call from its durable token.
    TemplateLanguage,
    /// A [`ValueBindings`](api::lang::rewrites::userdefs::ValueBindings).
    ValueBindings,
}

impl HandleKind {
    /// The kind's name, as its serde form writes it (`feature_builder`,
    /// `key_val_entry`, …).
    pub fn name(self) -> &'static str {
        match self {
            HandleKind::FeatureBuilder => "feature_builder",
            HandleKind::StageCx => "stage_cx",
            HandleKind::StageOut => "stage_out",
            HandleKind::TablePiece => "table_piece",
            HandleKind::ExtTableRead => "ext_table_read",
            HandleKind::TreesTable => "trees_table",
            HandleKind::Node => "node",
            HandleKind::KeyVals => "key_vals",
            HandleKind::KeyValEntry => "key_val_entry",
            HandleKind::ExpandCx => "expand_cx",
            HandleKind::Expander => "expander",
            HandleKind::UserDefinition => "user_definition",
            HandleKind::ModeVariant => "mode_variant",
            HandleKind::CompiledTemplate => "compiled_template",
            HandleKind::PlaceholderBindings => "placeholder_bindings",
            HandleKind::Staged => "staged",
            HandleKind::Stage => "stage",
            HandleKind::Expand => "expand",
            HandleKind::LoweringRule => "lowering_rule",
            HandleKind::LowerCx => "lower_cx",
            HandleKind::Lowerer => "lowerer",
            HandleKind::Lower => "lower",
            HandleKind::ElementDef => "element_def",
            HandleKind::RewriteStage => "rewrite_stage",
            HandleKind::RewriteCx => "rewrite_cx",
            HandleKind::Restager => "restager",
            HandleKind::RestagedArgument => "restaged_argument",
            HandleKind::RestagedSlot => "restaged_slot",
            HandleKind::LoweredTrees => "lowered_trees",
            HandleKind::LibraryStage => "library_stage",
            HandleKind::HostService => "host_service",
            HandleKind::Files => "files",
            HandleKind::CitationProvider => "citation_provider",
            HandleKind::CitationEntry => "citation_entry",
            HandleKind::ProviderCx => "provider_cx",
            HandleKind::TemplateModule => "template_module",
            HandleKind::TemplateCx => "template_cx",
            HandleKind::Module => "module",
            HandleKind::Referenceable => "referenceable",
            HandleKind::Sectioning => "sectioning",
            HandleKind::ArgDef => "arg_def",
            HandleKind::BuiltLang => "built_lang",
            HandleKind::FlmDiagnostics => "flm_diagnostics",
            HandleKind::LateInit => "late_init",
            HandleKind::Finalizer => "finalizer",
            HandleKind::Counters => "counters",
            HandleKind::Anchors => "anchors",
            HandleKind::TemplateLanguage => "template_language",
            HandleKind::ValueBindings => "value_bindings",
        }
    }
}

/// The list is generated from the sources under `src/api/`.
pub use crate::adapter::generated::MODULE_PATHS;

/// The handle of the module at `path` (one of [`MODULE_PATHS`]), or `None` for
/// another path.
pub fn module_handle(path: &str) -> Option<Handle> {
    let index = MODULE_PATHS.iter().position(|candidate| *candidate == path)?;
    Some(Handle { kind: HandleKind::Module, call: 0, index: u32::try_from(index).ok()? })
}

/// The table of one call. It is created on the stack of the binding's
/// [`ExtRuntime::register`](crate::ExtRuntime::register),
/// [`Stage::run`](api::core::stage::Stage::run),
/// [`Expand::expand`](api::lang::rewrites::userdefs::Expand::expand) or
/// [`Lower::lower`](api::core::lowering::Lower::lower) or
/// [`RewriteStage::restage`](api::core::stage::RewriteStage::restage) implementation (and of
/// a citation provider's or a template module's calls), filled with the objects that call lends, extended by the
/// dispatcher when a method answers a new object, and dropped when the handler
/// returns, which ends every handle of the call. `'c` is the call's lifetime:
/// nothing in the table outlives the Rust frame that owns the table.
///
/// The table also holds the durable table of the running handler's feature
/// ([`durable`](CallTable::durable)), taken from the first façade lent that
/// carries one (the builder, a stage's context, the expander, the lowerer, a
/// late-initialization hook's language): a durable token passed to the
/// dispatcher is resolved against it.
pub struct CallTable<'c> {
    call: u64,
    entries: Vec<Entry<'c>>,
    durable: Option<Arc<DurableTable>>,
}

/// One lent object. Objects lent by the adapter are references; objects the
/// dispatcher creates while answering a method (a child node, a key-value list,
/// a template) or that the binding lends in (a stage object) are owned, or
/// record how to ask for the object again (a table piece).
///
/// The ownership principle (plan A, A15): every object lent into the table is
/// owned, or borrows only something that lives for the whole call outside the
/// table (the objects the adapter lent at the start). An object answered by a
/// method owns what it points at, never a borrow of the entry it was asked on,
/// since such a borrow would borrow the table itself.
pub enum Entry<'c> {
    /// The builder lent to `register`.
    Builder(&'c mut dyn api::core::registry::FeatureBuilder),
    /// The context lent to a stage's `run`.
    StageCx(&'c dyn api::core::run::StageCx),
    /// The output lent to a stage's `run`.
    StageOut(&'c mut dyn api::core::data::store::StageOut),
    /// A piece of a table the stage writes, recorded as how to ask for it
    /// again: [`StageOut::table`](api::core::data::store::StageOut::table)
    /// answers a borrow of the output, which cannot be lent into the table
    /// that holds the output, so the dispatcher resolves
    /// `out.table(&name)?` anew on every call on the piece (plan A, A17).
    TablePiece {
        /// The handle of the [`StageOut`](api::core::data::store::StageOut)
        /// entry the piece belongs to.
        out: Handle,
        /// The name of the table, as passed to `StageOut::table`.
        name: String,
    },
    /// A table of an extension's own, read.
    ExtTableRead(Box<dyn api::core::data::table::ExtTableRead + 'c>),
    /// A table of a rewrite's trees, read.
    TreesTable(Box<dyn api::core::stage::TreesTable<'c> + 'c>),
    /// A node.
    Node(Box<dyn api::techy::node::Node<'c> + 'c>),
    /// A key-value list.
    KeyVals(Box<dyn api::techy::extract::KeyVals<'c> + 'c>),
    /// An entry of a key-value list.
    KeyValEntry(Box<dyn api::techy::extract::KeyValEntry<'c> + 'c>),
    /// The context lent to an expansion hook.
    ExpandCx(&'c mut dyn api::lang::rewrites::userdefs::ExpandCx),
    /// The expander lent to an expansion hook.
    Expander(&'c mut dyn api::lang::rewrites::userdefs::Expander),
    /// A user definition.
    UserDefinition(api::lang::rewrites::userdefs::UserDefinition),
    /// A definition's variant for one mode.
    ModeVariant(Box<dyn api::lang::rewrites::userdefs::ModeVariant + 'c>),
    /// A compiled template.
    CompiledTemplate(Box<dyn api::lang::rewrites::userdefs::CompiledTemplate + 'c>),
    /// The bindings of a use site.
    PlaceholderBindings(Box<dyn api::lang::rewrites::userdefs::PlaceholderBindings + 'c>),
    /// A staged node.
    Staged(api::lang::rewrites::userdefs::Staged),
    /// A stage object the binding lent in.
    Stage(Arc<dyn api::core::stage::Stage>),
    /// An expansion hook the binding lent in.
    Expand(Arc<dyn api::lang::rewrites::userdefs::Expand>),
    /// A lowering rule a constructor or a helper made.
    LoweringRule(api::core::lowering::LoweringRule),
    /// The context lent to a rule's body.
    LowerCx(&'c mut dyn api::core::lowering::LowerCx),
    /// The lowerer lent to a rule's body.
    Lowerer(&'c mut dyn api::core::lowering::Lowerer),
    /// A rule's body the binding lent in.
    Lower(Arc<dyn api::core::lowering::Lower>),
    /// An element declaration a helper made.
    ElementDef(api::lang::mint::ElementDef),
    /// A rewrite stage's handler the binding lent in.
    RewriteStage(Arc<dyn api::core::stage::RewriteStage>),
    /// The context lent to a rewrite stage's handler.
    RewriteCx(&'c dyn api::core::run::RewriteCx),
    /// The restager lent to a rewrite stage's handler.
    Restager(&'c mut dyn api::techy::transform::Restager),
    /// A restaged argument a restager's method answered.
    RestagedArgument(Box<dyn api::techy::transform::RestagedArgument>),
    /// A restaged slot a restager's method answered.
    RestagedSlot(Box<dyn api::techy::transform::RestagedSlot>),
    /// A table of a backend's lowered trees, read.
    LoweredTrees(Box<dyn api::stdcore::backends::lowering::LoweredTrees<'c> + 'c>),
    /// A stage of the library a constructor made.
    LibraryStage(api::core::registry::LibraryStage),
    /// A host service the package declared, lent by
    /// [`StageCx::host`](api::core::run::StageCx::host).
    HostService(Box<dyn api::host::HostService>),
    /// The `files` host service, lent by
    /// [`StageCx::host`](api::core::run::StageCx::host).
    Files(Box<dyn api::host::Files>),
    /// A citation provider the binding lent in.
    CitationProvider(Arc<dyn api::stdcore::features::citations::CitationProvider>),
    /// A citation entry a provider's context made.
    CitationEntry(Box<dyn api::stdcore::features::citations::CitationEntry>),
    /// The context lent to a citation provider's call.
    ProviderCx(&'c dyn api::stdcore::features::citations::ProviderCx),
    /// A template module the binding lent in.
    TemplateModule(Arc<dyn api::stdcore::features::compose::TemplateModule>),
    /// The context lent to a template module's `files` or `render`.
    TemplateCx(&'c dyn api::stdcore::features::compose::TemplateCx),
    /// A reference-points aspect the binding lent in.
    Referenceable(Arc<dyn api::model::aspects::Referenceable>),
    /// A sectioning aspect the binding lent in.
    Sectioning(Arc<dyn api::model::aspects::Sectioning>),
    /// An argument a helper made.
    ArgDef(api::lang::mint::ArgDef),
    /// The built language: lent to a late-initialization hook, or answered
    /// by a lowerer's or an expander's `lang()`.
    BuiltLang(LangEntry<'c>),
    /// The diagnostics lent to a late-initialization hook.
    FlmDiagnostics(&'c mut dyn api::lang::FlmDiagnostics),
    /// A late-initialization hook the binding lent in.
    LateInit(Arc<dyn api::core::registry::LateInit>),
    /// A finalizer the binding lent in.
    Finalizer(Arc<dyn api::core::registry::Finalizer>),
    /// The counters table a stage's context or a rule's lowerer answered.
    Counters(TableEntry<Box<dyn api::stdcore::features::numbering::Counters + 'c>>),
    /// The anchors table a stage's context or a rule's lowerer answered.
    Anchors(TableEntry<Box<dyn api::stdcore::features::refs::Anchors + 'c>>),
    /// A template language, resolved from its durable token.
    TemplateLanguage(api::lang::rewrites::userdefs::TemplateLanguage),
    /// Value bindings.
    ValueBindings(api::lang::rewrites::userdefs::ValueBindings),
}

/// The built language in the call's table: lent, as a late-initialization
/// hook is given it, or recorded as the lowerer or the expander that answered
/// it: the façade `lang()` answers borrows the lowerer or the expander, which
/// cannot be lent beside it, so the dispatcher asks for it again on every use
/// (as a table a rule's lowerer answered, [`TableEntry::Recorded`]).
pub enum LangEntry<'c> {
    /// The façade itself.
    Lent(&'c dyn api::lang::BuiltLang),
    /// The handle of the [`Lowerer`](api::core::lowering::Lowerer) or the
    /// [`Expander`](api::lang::rewrites::userdefs::Expander) entry whose
    /// `lang()` answers it.
    Of(Handle),
}

/// A table façade in the call's table: lent, as a stage's context answers
/// it, or recorded as how to ask for it again, as a rule's lowerer answers
/// it: the façade borrows the lowerer, which cannot be lent beside it, so
/// the dispatcher resolves `lowerer.get(&name)` anew on every call on it.
pub enum TableEntry<T> {
    /// The façade itself.
    Lent(T),
    /// How to ask for the façade again.
    Recorded {
        /// The handle of the [`Lowerer`](api::core::lowering::Lowerer) entry
        /// that answered the table.
        lowerer: Handle,
        /// The table's name, as passed to `get`.
        name: String,
    },
}

impl Entry<'_> {
    /// The kind of handle that stands for this entry.
    pub fn kind(&self) -> HandleKind {
        match self {
            Entry::Builder(_) => HandleKind::FeatureBuilder,
            Entry::StageCx(_) => HandleKind::StageCx,
            Entry::StageOut(_) => HandleKind::StageOut,
            Entry::TablePiece { .. } => HandleKind::TablePiece,
            Entry::ExtTableRead(_) => HandleKind::ExtTableRead,
            Entry::TreesTable(_) => HandleKind::TreesTable,
            Entry::Node(_) => HandleKind::Node,
            Entry::KeyVals(_) => HandleKind::KeyVals,
            Entry::KeyValEntry(_) => HandleKind::KeyValEntry,
            Entry::ExpandCx(_) => HandleKind::ExpandCx,
            Entry::Expander(_) => HandleKind::Expander,
            Entry::UserDefinition(_) => HandleKind::UserDefinition,
            Entry::ModeVariant(_) => HandleKind::ModeVariant,
            Entry::CompiledTemplate(_) => HandleKind::CompiledTemplate,
            Entry::PlaceholderBindings(_) => HandleKind::PlaceholderBindings,
            Entry::Staged(_) => HandleKind::Staged,
            Entry::Stage(_) => HandleKind::Stage,
            Entry::Expand(_) => HandleKind::Expand,
            Entry::LoweringRule(_) => HandleKind::LoweringRule,
            Entry::LowerCx(_) => HandleKind::LowerCx,
            Entry::Lowerer(_) => HandleKind::Lowerer,
            Entry::Lower(_) => HandleKind::Lower,
            Entry::ElementDef(_) => HandleKind::ElementDef,
            Entry::RewriteStage(_) => HandleKind::RewriteStage,
            Entry::RewriteCx(_) => HandleKind::RewriteCx,
            Entry::Restager(_) => HandleKind::Restager,
            Entry::RestagedArgument(_) => HandleKind::RestagedArgument,
            Entry::RestagedSlot(_) => HandleKind::RestagedSlot,
            Entry::LoweredTrees(_) => HandleKind::LoweredTrees,
            Entry::LibraryStage(_) => HandleKind::LibraryStage,
            Entry::HostService(_) => HandleKind::HostService,
            Entry::Files(_) => HandleKind::Files,
            Entry::CitationProvider(_) => HandleKind::CitationProvider,
            Entry::CitationEntry(_) => HandleKind::CitationEntry,
            Entry::ProviderCx(_) => HandleKind::ProviderCx,
            Entry::TemplateModule(_) => HandleKind::TemplateModule,
            Entry::TemplateCx(_) => HandleKind::TemplateCx,
            Entry::Referenceable(_) => HandleKind::Referenceable,
            Entry::Sectioning(_) => HandleKind::Sectioning,
            Entry::ArgDef(_) => HandleKind::ArgDef,
            Entry::BuiltLang(_) => HandleKind::BuiltLang,
            Entry::FlmDiagnostics(_) => HandleKind::FlmDiagnostics,
            Entry::LateInit(_) => HandleKind::LateInit,
            Entry::Finalizer(_) => HandleKind::Finalizer,
            Entry::Counters(_) => HandleKind::Counters,
            Entry::Anchors(_) => HandleKind::Anchors,
            Entry::TemplateLanguage(_) => HandleKind::TemplateLanguage,
            Entry::ValueBindings(_) => HandleKind::ValueBindings,
        }
    }

    /// The durable table the entry lends, for the façades that carry one.
    fn durable(&self) -> Option<Arc<DurableTable>> {
        let token = Token::new();
        match self {
            Entry::Builder(b) => Some(b.durable_table(token)),
            Entry::StageCx(cx) => Some(cx.durable_table(token)),
            Entry::Expander(xp) => Some(xp.durable_table(token)),
            Entry::Lowerer(lw) => Some(lw.durable_table(token)),
            Entry::BuiltLang(LangEntry::Lent(lang)) => Some(lang.durable_table(token)),
            _ => None,
        }
    }
}

impl<'c> CallTable<'c> {
    /// A fresh table for the call numbered `call`. The binding numbers its
    /// calls from a counter of its own, starting at 1 (0 is the module
    /// handles'); per thread suffices, since a handle never crosses threads.
    pub fn new(call: u64) -> CallTable<'c> {
        CallTable { call, entries: Vec::new(), durable: None }
    }

    /// Lends `entry`; answers its handle. Used by the binding for the objects
    /// the adapter passed to it, and by the dispatcher for the objects it
    /// creates.
    pub fn lend(&mut self, entry: Entry<'c>) -> Handle {
        let kind = entry.kind();
        if self.durable.is_none() {
            self.durable = entry.durable();
        }
        // The index saturates at `u32::MAX`: past 2^32 - 1 entries, every new
        // handle would stand for the entry at that index. One call never lends
        // that many objects (the entries alone would fill tens of gigabytes),
        // so the bound is stated here rather than refused.
        let index = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        self.entries.push(entry);
        Handle { kind, call: self.call, index }
    }

    /// The entry of `handle`. The dispatcher then checks the entry's kind.
    ///
    /// # Errors
    ///
    /// The handle belongs to another call ([`ExtError::StaleHandle`]), or this
    /// call lent nothing at its index ([`ExtError::Argument`]).
    pub fn entry(&mut self, handle: Handle) -> Result<&mut Entry<'c>, ExtError> {
        if handle.call != self.call {
            return Err(ExtError::StaleHandle { handle });
        }
        let never_lent = || ExtError::Argument {
            method: String::new(),
            detail: format!(
                "the running call lent no `{}` object at index {}",
                handle.kind.name(),
                handle.index
            ),
        };
        let index = usize::try_from(handle.index).map_err(|_| never_lent())?;
        self.entries.get_mut(index).ok_or_else(never_lent)
    }

    /// The number of the call this table belongs to.
    pub fn call(&self) -> u64 {
        self.call
    }

    /// The durable table of the running handler's feature, taken from the
    /// first façade lent that carries one; `None` before any such façade is
    /// lent.
    pub fn durable(&self) -> Option<&Arc<DurableTable>> {
        self.durable.as_ref()
    }

    /// The entries of `handles`, all at once (plan A, A20): what a method
    /// taking several lent objects needs (`instantiate` takes the expander,
    /// the context, the use site, the template and the bindings). Each handle
    /// is checked as [`entry`](CallTable::entry) checks it; the dispatcher
    /// then checks the kinds.
    ///
    /// # Errors
    ///
    /// As [`entry`](CallTable::entry); two of the handles stand for one
    /// object ([`ExtError::Argument`] naming `method`).
    pub(crate) fn entries_mut<const N: usize>(
        &mut self,
        method: &str,
        handles: [Handle; N],
    ) -> Result<[&mut Entry<'c>; N], ExtError> {
        let mut indices = [0usize; N];
        for (slot, handle) in indices.iter_mut().zip(handles) {
            // Checks the call and the index, and answers the position.
            self.entry(handle)?;
            *slot = usize::try_from(handle.index).unwrap_or(usize::MAX);
        }
        self.entries.get_disjoint_mut(indices).map_err(|_| ExtError::Argument {
            method: method.to_string(),
            detail: "each object must be passed once".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::lang::rewrites::userdefs::{Definition, UserDefinition};

    fn definition() -> Entry<'static> {
        Entry::UserDefinition(UserDefinition::new(Definition::macro_("x", Vec::new())))
    }

    #[test]
    fn a_lent_entry_is_found_by_its_handle() {
        let mut table = CallTable::new(7);
        let handle = table.lend(definition());
        assert_eq!(handle, Handle { kind: HandleKind::UserDefinition, call: 7, index: 0 });
        assert_eq!(table.entry(handle).map(|entry| entry.kind()), Ok(HandleKind::UserDefinition));
    }

    #[test]
    fn a_handle_of_another_call_is_stale() {
        let mut table = CallTable::new(7);
        let handle = table.lend(definition());
        let mut later = CallTable::new(8);
        let _ = later.lend(definition());
        assert_eq!(later.entry(handle).map(|entry| entry.kind()), Err(ExtError::StaleHandle { handle }));
    }

    #[test]
    fn an_index_the_call_never_lent_is_a_bad_argument() {
        let mut table = CallTable::new(7);
        let handle = table.lend(definition());
        let beyond = Handle { index: 5, ..handle };
        let error = table.entry(beyond).map(|entry| entry.kind()).expect_err("nothing at index 5");
        assert_eq!(error.kind(), "argument");
        assert_eq!(error.to_string(), "the running call lent no `user_definition` object at index 5");
    }

    #[test]
    fn module_handles_are_numbered_by_their_position() {
        assert_eq!(module_handle("core/artifact"), Some(Handle { kind: HandleKind::Module, call: 0, index: 0 }));
        assert_eq!(module_handle("core/lowering").map(|h| h.index), Some(1));
        assert_eq!(module_handle("lang/mint").map(|h| h.index), Some(2));
        assert_eq!(module_handle("stdcore/features/math").map(|h| h.index), Some(15));
        assert_eq!(module_handle("lang/unknown"), None);
    }

    #[test]
    fn a_kind_s_name_is_its_serde_form() {
        // Every variant. The `match` below stops compiling when a variant is
        // added, a reminder to list the new one here as well.
        let every = [
            HandleKind::FeatureBuilder,
            HandleKind::StageCx,
            HandleKind::StageOut,
            HandleKind::TablePiece,
            HandleKind::ExtTableRead,
            HandleKind::TreesTable,
            HandleKind::Node,
            HandleKind::KeyVals,
            HandleKind::KeyValEntry,
            HandleKind::ExpandCx,
            HandleKind::Expander,
            HandleKind::UserDefinition,
            HandleKind::ModeVariant,
            HandleKind::CompiledTemplate,
            HandleKind::PlaceholderBindings,
            HandleKind::Staged,
            HandleKind::Stage,
            HandleKind::Expand,
            HandleKind::LoweringRule,
            HandleKind::LowerCx,
            HandleKind::Lowerer,
            HandleKind::Lower,
            HandleKind::ElementDef,
            HandleKind::RewriteStage,
            HandleKind::RewriteCx,
            HandleKind::Restager,
            HandleKind::RestagedArgument,
            HandleKind::RestagedSlot,
            HandleKind::LoweredTrees,
            HandleKind::LibraryStage,
            HandleKind::HostService,
            HandleKind::Files,
            HandleKind::CitationProvider,
            HandleKind::CitationEntry,
            HandleKind::ProviderCx,
            HandleKind::TemplateModule,
            HandleKind::TemplateCx,
            HandleKind::Module,
            HandleKind::Referenceable,
            HandleKind::Sectioning,
            HandleKind::ArgDef,
            HandleKind::BuiltLang,
            HandleKind::FlmDiagnostics,
            HandleKind::LateInit,
            HandleKind::Finalizer,
            HandleKind::Counters,
            HandleKind::Anchors,
            HandleKind::TemplateLanguage,
            HandleKind::ValueBindings,
        ];
        for kind in every {
            match kind {
                HandleKind::FeatureBuilder
                | HandleKind::StageCx
                | HandleKind::StageOut
                | HandleKind::TablePiece
                | HandleKind::ExtTableRead
                | HandleKind::TreesTable
                | HandleKind::Node
                | HandleKind::KeyVals
                | HandleKind::KeyValEntry
                | HandleKind::ExpandCx
                | HandleKind::Expander
                | HandleKind::UserDefinition
                | HandleKind::ModeVariant
                | HandleKind::CompiledTemplate
                | HandleKind::PlaceholderBindings
                | HandleKind::Staged
                | HandleKind::Stage
                | HandleKind::Expand
                | HandleKind::LoweringRule
                | HandleKind::LowerCx
                | HandleKind::Lowerer
                | HandleKind::Lower
                | HandleKind::ElementDef
                | HandleKind::RewriteStage
                | HandleKind::RewriteCx
                | HandleKind::Restager
                | HandleKind::RestagedArgument
                | HandleKind::RestagedSlot
                | HandleKind::LoweredTrees
                | HandleKind::LibraryStage
                | HandleKind::HostService
                | HandleKind::Files
                | HandleKind::CitationProvider
                | HandleKind::CitationEntry
                | HandleKind::ProviderCx
                | HandleKind::TemplateModule
                | HandleKind::TemplateCx
                | HandleKind::Module
                | HandleKind::Referenceable
                | HandleKind::Sectioning
                | HandleKind::ArgDef
                | HandleKind::BuiltLang
                | HandleKind::FlmDiagnostics
                | HandleKind::LateInit
                | HandleKind::Finalizer
                | HandleKind::Counters
                | HandleKind::Anchors
                | HandleKind::TemplateLanguage
                | HandleKind::ValueBindings => {}
            }
            let json = serde_json::to_value(kind).ok();
            assert_eq!(json, Some(serde_json::Value::String(kind.name().to_string())), "{kind:?}");
            assert_eq!(serde_json::from_value::<HandleKind>(serde_json::Value::String(kind.name().to_string())).ok(), Some(kind));
        }
        let mut names: Vec<&str> = every.iter().map(|kind| kind.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), every.len());
    }
}
