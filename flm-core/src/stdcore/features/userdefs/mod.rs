//! The `userdefs` feature: it registers every user definition of its
//! configuration as an element, the rewrite stages that expand their use
//! sites, and the lowering rules that write the use sites a plan keeps.
//!
//! # What the feature provides to an author
//!
//! A user definition ([`Definition`]) is a macro, environment or special
//! character sequence that a document's author or the host defines by
//! substitution: a name, an argument syntax, a template and the parsing modes
//! it is defined for. Its template is FLM text with placeholders (`#1`,
//! `#{name}`, `#{body}`) in the [template
//! language](crate::lang::rewrites::userdefs::template). A *use site* of a
//! definition is a place in a document where the document uses it, such as
//! `\hi{World}` for a macro `\hi`; the use site is replaced by the definition's
//! template, with the arguments written there in place of the placeholders. A
//! document's body cannot define a macro: every user definition comes from this
//! feature's configuration ([`UserdefsConfig`]), which an author writes as
//! settings (the keys `macros`, `environments` and `specials`,
//! [`UserdefsData`]), for example in a document's front matter when the `flm`
//! program reads it. In settings, a definition gives its arguments under the
//! key `args` ([`DefinitionData::args`]), each argument's syntax in one of two
//! forms. The structured syntax is a map whose key `syntax` holds the name of
//! a standard kind of argument and whose other keys hold the kind's
//! parameters, beside the argument's own keys: `{syntax: delimited-group,
//! delimiters: ["(", ")"], name: size}` declares a group in parentheses named
//! `size`. An argument code is a shorthand for a structured syntax, written
//! under the key `code` of the map or alone: `{code: "r()", name: size}`
//! declares the same argument, and `"{"` a mandatory group. The language
//! guide describes definitions for authors:
//! [author definitions](crate::guide::language_syntax#author-definitions) and
//! [when a definition is
//! expanded](crate::guide::language_syntax#when-a-definition-is-expanded).
//!
//! A definition's *expansion policy* ([`ExpandPolicy`]) says when its use
//! sites are expanded: `always`, in every plan, or `per-plan` (the default),
//! as each plan decides. A plan for any backend other than
//! `latex-recomposed` expands the use sites of every definition, unless its
//! plan request selects another lowering rule by name. A plan for the
//! `latex-recomposed` backend keeps the use sites of a `per-plan` definition in
//! the tree and writes the definition once as a LaTeX definition in the [style
//! file](crate::stdcore::backends::latex::style) and each use site as a call of
//! it; when the definition cannot be written as one LaTeX definition, or its
//! template needs the characters of an argument, it expands each use site at
//! lowering instead ([user definitions in the recomposed
//! LaTeX](crate::guide::rendering#user-definitions-in-the-recomposed-latex)).
//! The module [`kept`] holds the rules that do this.
//!
//! # Definitions are elements
//!
//! Every definition of the configuration becomes an element declaration like
//! any other ([`UserDefinition::element_def`]): an element named
//! `flm.userdefs.<kind>.<name>` ([`element_name`]) that carries the definition
//! as its value (a [`UserDefinition`]), the
//! [`Expand`](crate::lang::rewrites::userdefs::Expand) aspect (an
//! [aspect](crate::guide::concepts_overview#elements-and-aspects) is behavior
//! attached to an element; this one is what the expand pass calls to expand a
//! use site) and the block role the definition declares. An extension written
//! in Rust registers the same things for an expandable element of its own
//! ([`element_name_under`],
//! [`expand_in_tree_rule_for`]); nothing about user definitions is special to
//! the registry.
//!
//! - **The kind is part of the element name**, because an element is found by
//!   its name: a macro and an environment may have the same written name, and
//!   they must be two elements with two names.
//! - **A definition is available in the parsing modes it is defined for, and
//!   nowhere else** ([`DefinitionModes`]): its element is declared available in
//!   those modes alone, so a use of it in another mode is an unknown command,
//!   like any name the registry does not define (reported in running text; kept
//!   as written without a report inside mathematics, where the `math` feature
//!   accepts any unknown command). A definition for both modes
//!   applies in each mode where its template parses; a use in a mode where the
//!   template does not parse is an Error at that use.
//! - **One name may be declared twice**, once for text mode and once for
//!   mathematics mode, with a different template each time: the two never apply
//!   in the same place, and each becomes an element of its own (the element of
//!   a definition for mathematics mode alone has an extra `math` segment in its
//!   name, [`element_name`]). Two definitions of one kind and one name whose
//!   modes overlap make the registry build fail, since only one of them could
//!   apply where they overlap.
//! - **A definition that replaces another feature's command** (marked with
//!   [`Definition::overriding`]) also registers an override declaration
//!   ([`FeatureBuilder::override_callable`]). Without it, two features that
//!   define one command in overlapping modes make the registry build fail with
//!   an error that gives both, unless the feature order places the feature
//!   registered last explicitly after the other (the chain
//!   `math-mathjax < userdefs` lets a user definition replace a command of
//!   `math-mathjax`; see [`crate::core::registry::order`]).
//!
//! # Capabilities and stages
//!
//! A *capability* ([`CapabilityDef`]) is a registered dotted name for a state
//! of processing that a stage brings about; a stage declares the capabilities
//! it provides and requires, and a caller demands the capabilities it wants
//! ([stages, capabilities and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans)). The
//! feature registers:
//!
//! - [`USERDEFS`] (`flm.userdefs`), the parent capability: a decision about the
//!   `per-plan` definitions has been made. Two capabilities *refine* it
//!   ([refinement](crate::guide::pipeline#refinement)): each is one way of
//!   satisfying it, and a plan that demands both is refused.
//! - [`EXPAND`] (`flm.userdefs.expand`), the rewrite stage
//!   ([`RewriteStage`](crate::core::stage::RewriteStage): a stage that turns
//!   one tree into another before the trees are frozen) that expands the use
//!   sites of the `per-plan` definitions. It publishes its trees in the trees
//!   table [`EXPANDED`] (`flm.userdefs.expanded`; a [`TreesTable`], the table
//!   in which a rewrite stage publishes the trees it produces), which is also
//!   the
//!   capability it provides, the refinement of `flm.userdefs` marked as the
//!   default: a plan that demands `flm.userdefs` alone expands the use sites.
//! - [`KEEP`] (`flm.userdefs.keep`), a marker stage that does nothing and
//!   provides [`KEPT`] (`flm.userdefs.kept`), the other refinement: the use
//!   sites stay in the tree for the lowering rules.
//! - [`EXPAND_ALWAYS`] (`flm.userdefs.expand-always`), the rewrite stage that
//!   expands the use sites of the `always` definitions. It publishes
//!   [`ALWAYS_EXPANDED`] (`flm.userdefs.always-expanded`), a capability that
//!   refines nothing, so it goes with either alternative; `flm.userdefs.expand`
//!   is ordered after it whenever both are in a plan.
//!
//! Both rewrite stages run the expand pass
//! ([`expand`](crate::lang::rewrites::userdefs::expand)) under the expansion
//! depth limit of the configuration; the module `stages` holds them.
//!
//! # Lowering rules
//!
//! Every definition registers its `expand-in-tree` lowering rule
//! ([`expand_in_tree_rule`], module [`lowering`]), the default on every
//! backend. The rule requires the trees table of the rewrite stage that expands
//! the definition (`flm.userdefs.expanded` for a `per-plan` definition,
//! `flm.userdefs.always-expanded` for an `always` one) and never lowers a use
//! site, since the stage has replaced every one before lowering. It has two
//! purposes: the definition's element has a rule on every backend, so that a
//! plan finds no element without a rule; and selecting the rule puts the
//! expansion into the plan. Every plan that lowers a registry holding an
//! `always` definition therefore holds `flm.userdefs.expand-always`.
//!
//! A `per-plan` definition registers two more rules, for its use sites kept in
//! the tree (module [`kept`]): `emit-call` ([`emit_call_rule`]), the default on
//! the `latex-recomposed` backend alone, which writes a kept use site as a call
//! of the definition and the definition itself into the style file, or expands
//! the use site instead when the definition is not recomposable or its template
//! needs the characters of an argument; and
//! `instantiate-at-lowering` ([`instantiate_at_lowering_rule`]), never a
//! default, which a plan request selects by name to have each kept use site
//! expanded at lowering on any backend. Both require `flm.userdefs.kept`, so
//! selecting either puts the `flm.userdefs.keep` marker into the plan. The rule
//! selection order ([`SelectedRules`](crate::core::plan::SelectedRules)) puts a
//! rule marked default on one backend before a rule marked default everywhere,
//! so the three rules of one definition never compete: `latex-recomposed` gets
//! `emit-call` and every other backend gets `expand-in-tree`. A plan that
//! demands `flm.userdefs.kept` on a backend where a `per-plan` definition's
//! selected rule is `expand-in-tree` is refused when the plan is built, since
//! that rule requires the other alternative.
//!
//! The function [`kept::register`] also registers, once, a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)) of the `latex-recomposed`
//! backend and of each LaTeX construct element that the `emit-call` rules
//! stage (build into the lowered tree), so that a registry without the LaTeX
//! features still builds; the `emit-call` rules are then set aside (excluded
//! from rule selection).
//!
//! # Templates are compiled when the registry is built
//!
//! A template is parsed against the finished language, which exists only once
//! every feature has registered. The feature therefore registers a
//! late-initialization build hook ([`LateInit`](crate::core::registry::LateInit))
//! that compiles the templates of every definition over one template language
//! ([`TemplateLanguage`](crate::lang::rewrites::userdefs::TemplateLanguage))
//! built for all of them. Every diagnostic of a compilation (the warnings of a
//! definition that compiled, the errors of one that was refused) becomes a
//! build diagnostic of the registry
//! ([`FlmRegistry::build_diagnostics`](crate::core::registry::FlmRegistry::build_diagnostics)),
//! so a mistake in a template is reported once, when the registry is built. A
//! refused definition stays registered without compiled templates, and each of
//! its use sites is replaced by its literal source text and reported with
//! `flm.userdefs.bad-definition`. None of this makes the registry build fail.
//!
//! The same hook then records, on every definition, whether it is
//! *recomposable* (whether a backend can write it as one definition of its own
//! output language; [`NotRecomposable`] lists the reasons it may not be) and
//! the arguments one call of it writes ([`kept::record_recomposability`]). The
//! hook reports nothing about recomposability: a refused definition was
//! reported when it was refused, and none of the other reasons that
//! [`NotRecomposable`] lists is a mistake in the document.
//!
//! # Configuration
//!
//! [`UserdefsConfig`] holds the definitions and the expansion depth limit. It is
//! the feature's configuration, the plain Rust value the feature is constructed
//! from ([`Userdefs::new`]); it is fixed when the registry is built and cannot
//! vary per document or per render ([settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
//! because the trees a run freezes depend on it. The feature gives its
//! configuration's canonical bytes ([`UserdefsConfig::digest_bytes`]) as an
//! input of the [registry digest](crate::model::digest), a number computed
//! from everything about a registry that can change what processing produces,
//! so that two registries that differ in a template do not share cached trees.
//! In settings written as data, the configuration is read as a
//! [`UserdefsData`].
//!
//! The feature creates a new [`UserDefinition`] for every definition each time
//! it registers, so one feature value may be registered into several
//! registries, each of which compiles its own templates against its own
//! language.
//!
//! # Interactions with other features
//!
//! - The [`blocks`](crate::stdcore::features::blocks) feature's structure stage
//!   requires `flm.userdefs` and is ordered after `flm.userdefs.always-expanded`,
//!   because a use site that is not yet expanded can hide a paragraph break;
//!   its contract check reports a use site that remains in a tree the plan
//!   expanded.
//! - The [`math-passthrough`](crate::math_passthrough) feature's rules for the
//!   `latex-recomposed` backend write mathematics from its nodes rather than
//!   through the lowering rules, and write a use site of a user definition
//!   inside mathematics through this feature's mathematics writer
//!   ([`kept::write_math_run`]).
//! - The LaTeX family ([`crate::stdcore::backends::latex`], the parts every
//!   LaTeX backend shares) provides the rule that writes the placeholders of a
//!   template written into the style file
//!   ([`placeholder_rule`](crate::stdcore::backends::latex::placeholder_rule)).
//!
//! # Items
//!
//! - [`Userdefs`], the feature, constructed with [`Userdefs::new`] from a
//!   [`UserdefsConfig`]; the configuration's data forms [`UserdefsData`],
//!   [`NamedDefinition`], [`DefinitionData`], [`DefinitionContent`] and
//!   [`ModeTemplates`].
//! - [`element_name`] and [`element_name_under`], the name of a definition's
//!   element.
//! - [`FEATURE`], [`USERDEFS`], [`EXPAND`], [`EXPANDED`], [`KEEP`], [`KEPT`],
//!   [`EXPAND_ALWAYS`] and [`ALWAYS_EXPANDED`], the names of the feature, its
//!   capabilities, stages and trees tables.
//! - [`lowering`]: the `expand-in-tree` rule ([`ExpandInTree`],
//!   [`expand_in_tree_rule`], [`expand_in_tree_rule_for`], [`rule_name`]).
//! - [`kept`]: the rules for kept use sites ([`EmitCall`],
//!   [`InstantiateAtLowering`] and their constructors and names), the
//!   recomposability computation, the condition
//!   [`KeptTemplateDeclaresPoint`] and the mathematics writer.
//! - Re-exported from [`crate::lang::rewrites::userdefs`]: [`Definition`],
//!   [`DefinitionKind`], [`DefinitionModes`], [`ExpandPolicy`] and
//!   [`NotRecomposable`].

// Naming, the two-round build and the digest: see design rationale at
// [§flm-dd-dr:name-userdef-element], [§flm-dd-dr:feat-late-init] and
// [§flm-dd-dr:feat-digest]. The guard against two definitions of one name sits
// where the names are minted, because the registry's own roster validation
// permits two element values under one name and reports the ambiguity only
// when the name is resolved: [§flm-dd-dr:feat-name-guard],
// [§flm-dd-dr:std-userdefs-element].

mod config;
pub mod kept;
pub mod lowering;
mod stages;

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::model::names::{ElementName, Name};
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::stage::TreesTable;
use crate::core::data::table::TableName;
use crate::lang::rewrites::userdefs::UserDefinition;

use self::stages::{CompileTemplates, ExpandStage, Expands, KeepMarker};

pub use self::config::{
    DefinitionContent, DefinitionData, ModeTemplates, NamedDefinition, UserdefsConfig, UserdefsData,
};
pub use self::kept::{
    EMIT_CALL, EmitCall, INSTANTIATE_AT_LOWERING, InstantiateAtLowering, KeptTemplateDeclaresPoint,
    emit_call_rule, emit_call_rule_name, instantiate_at_lowering_rule, instantiate_rule_name,
};
pub use self::lowering::{
    EXPAND_IN_TREE, ExpandInTree, expand_in_tree_rule, expand_in_tree_rule_for, rule_name,
};
pub use crate::lang::rewrites::userdefs::{
    Definition, DefinitionKind, DefinitionModes, ExpandPolicy, NotRecomposable,
};

/// The feature's registered name, `userdefs`. It is also the middle segment of
/// the names of the feature's stages and diagnostics (`flm.userdefs.expand`,
/// `flm.userdefs.recursion-limit`).
pub const FEATURE: &str = "userdefs";

/// The declaration of the feature's settings ([`crate::core::settings`]): the
/// fields of [`UserdefsData`] are the settings keys, and the data read is
/// converted to a [`UserdefsConfig`]. Each map of definitions is merged entry by
/// entry across sources of settings, and the template texts and default values
/// are marked as FLM markup.
fn settings() -> SettingsDecl {
    let mut decl = SettingsDecl::new(FEATURE).config_from::<UserdefsData, UserdefsConfig>(
        "The user definitions (macros, environments and special character sequences \
        defined by substitution), one map per kind, and the expansion depth limit. Fixed \
        when the registry is built: it cannot vary per document or per render.",
    );
    // The feature defines nothing by default (`UserdefsConfig::default`), so
    // each kind's default entries are the empty map.
    for kind in ["macros", "environments", "specials"] {
        decl = decl
            .merge_entries(kind, || Ok(serde_json::Value::Object(serde_json::Map::new())))
            .markup(format!("{kind}.*.content"))
            .markup(format!("{kind}.*.defaults"));
    }
    decl
}

/// The parent capability `flm.userdefs`: a decision about the use sites of the
/// `per-plan` definitions has been made. Two capabilities refine it,
/// [`EXPANDED`] (the default) and [`KEPT`]; a plan that demands both is refused.
/// See the module documentation.
pub const USERDEFS: &str = "flm.userdefs";

/// The name of the rewrite stage that expands the use sites of the `always`
/// definitions, `flm.userdefs.expand-always`. It publishes its trees in
/// [`ALWAYS_EXPANDED`].
pub const EXPAND_ALWAYS: &str = "flm.userdefs.expand-always";

/// The trees table `flm.userdefs.always-expanded`, in which the
/// [`EXPAND_ALWAYS`] stage publishes its trees, and the capability of the same
/// name that the stage provides. It refines nothing, so it goes with either
/// alternative of [`USERDEFS`]. The `expand-in-tree` rule of every `always`
/// definition requires it.
pub const ALWAYS_EXPANDED: TableName<TreesTable> =
    TableName::new("flm.userdefs.always-expanded");

/// The name of the rewrite stage that expands the use sites of the `per-plan`
/// definitions, `flm.userdefs.expand`. It publishes its trees in [`EXPANDED`]
/// and is ordered after [`ALWAYS_EXPANDED`] whenever both are in a plan.
pub const EXPAND: &str = "flm.userdefs.expand";

/// The trees table `flm.userdefs.expanded`, in which the [`EXPAND`] stage
/// publishes its trees, and the capability of the same name that the stage
/// provides: the refinement of [`USERDEFS`] marked as the default. The
/// `expand-in-tree` rule of every `per-plan` definition requires it.
///
/// A stage that declares an optional read of this table learns whether the
/// plan expanded the use sites of the `per-plan` definitions: the table is
/// written exactly where the stage ran.
pub const EXPANDED: TableName<TreesTable> = TableName::new("flm.userdefs.expanded");

/// The name of the marker stage `flm.userdefs.keep`: it does nothing and
/// provides [`KEPT`], so that a plan can choose to keep the use sites of the
/// `per-plan` definitions in the tree.
pub const KEEP: &str = "flm.userdefs.keep";

/// The capability `flm.userdefs.kept` that the [`KEEP`] stage provides: the
/// refinement of [`USERDEFS`] under which the use sites of the `per-plan`
/// definitions stay in the tree for the lowering rules. The `emit-call` and
/// `instantiate-at-lowering` rules require it ([`kept`]).
pub const KEPT: &str = "flm.userdefs.kept";

/// Returns the name of the element of `definition`:
/// `flm.userdefs.<kind>.<name>`, where the kind is `macro`, `environment` or
/// `specials`, and `flm.userdefs.<kind>.math.<name>` for a definition that
/// applies in mathematics mode alone.
///
/// The extra `math` segment makes a definition for text mode alone and a
/// definition for mathematics mode alone of one name two elements with two
/// names, as they must be, since an element is found by its name. Every
/// definition that applies in text mode (alone or in both modes, the common
/// case) has the name without the segment.
///
/// The characters of a special character sequence are written into the name
/// as they are, dots included. The module documentation says why the kind is
/// part of the name. [`element_name_under`] gives the same name under another
/// prefix.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, element_name};
///
/// let definition = Definition::macro_("foo", Vec::new());
/// assert_eq!(element_name(&definition), "flm.userdefs.macro.foo");
///
/// let in_math = Definition::macro_("foo", Vec::new()).math("\\mathbf{f}");
/// assert_eq!(element_name(&in_math), "flm.userdefs.macro.math.foo");
/// ```
pub fn element_name(definition: &Definition) -> ElementName {
    element_name_under("flm.userdefs", definition)
}

/// Returns the name of the element of `definition` under the prefix `prefix`:
/// `<prefix>.<kind>.<name>`, and `<prefix>.<kind>.math.<name>` for a
/// definition that applies in mathematics mode alone. [`element_name`] is this
/// function with the prefix `flm.userdefs`.
///
/// A feature of another crate that registers definitions as elements of its
/// own calls this function with its own prefix, so that every definition
/// element is named by one rule (the kind, then the `math` segment for a
/// definition of mathematics mode alone). Such a feature registers each
/// element's `expand-in-tree` rule with [`expand_in_tree_rule_for`].
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, element_name_under};
///
/// let in_math = Definition::macro_("ket", Vec::new()).math("\\lvert x\\rangle");
/// assert_eq!(element_name_under("mypack.notation", &in_math), "mypack.notation.macro.math.ket");
/// ```
pub fn element_name_under(prefix: &str, definition: &Definition) -> ElementName {
    let kind = definition.kind.token();
    let restricted = match definition.modes {
        DefinitionModes::Math => "math.",
        _ => "",
    };
    ElementName::from(format!("{prefix}.{kind}.{restricted}{}", definition.name))
}

/// The `userdefs` feature ([`FlmFeature`]): it registers one element per user
/// definition with its lowering rules (one for an `always` definition, three
/// for a `per-plan` one), the capabilities, the two expansion rewrite stages,
/// the keep marker stage, the peer declarations of the LaTeX constructs, the
/// late-initialization build hook that compiles the templates, and the
/// feature's settings. See the module documentation.
///
/// Construct it with [`Userdefs::new`] from a [`UserdefsConfig`]; the default
/// value defines nothing. The standard package registers it from the
/// `userdefs` field of [`StdcoreConfig`](crate::stdcore::StdcoreConfig).
///
/// Registering the feature fails ([`BuildError::Feature`]) when two
/// definitions of one kind and one name apply in a common parsing mode, or when
/// two definitions would have the same element name.
#[derive(Debug, Clone, Default)]
pub struct Userdefs {
    config: UserdefsConfig,
}

impl Userdefs {
    /// Returns the feature with the configuration `config` (the definitions and
    /// the expansion depth limit), which stays fixed for the life of every
    /// registry the feature is built into.
    pub fn new(config: UserdefsConfig) -> Userdefs {
        Userdefs { config }
    }

    /// Returns the configuration the feature was constructed with.
    pub fn config(&self) -> &UserdefsConfig {
        &self.config
    }
}

impl FlmFeature for Userdefs {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "User definitions").docs(
            "Macros, environments and special character sequences defined by substitution. \
             Each definition is an element carrying the `Expand` aspect. The rewrite stages \
             `flm.userdefs.expand-always` and `flm.userdefs.expand` expand the use sites of \
             the `always` and of the `per-plan` definitions; the `flm.userdefs.keep` marker \
             stage keeps the latter in the tree instead. Every definition's `expand-in-tree` \
             rule gives its element a rule on every backend; a `per-plan` definition's \
             `emit-call` and `instantiate-at-lowering` rules lower a use site kept in the \
             tree. The templates are compiled when the registry is built, which also records \
             whether each definition can be written as one LaTeX definition.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        b.capability(CapabilityDef::new(USERDEFS).docs(
            "A decision about the `per-plan` user definitions has been made: their use sites \
             are expanded in the tree (`flm.userdefs.expanded`, the default) or kept in the \
             tree for the lowering rules (`flm.userdefs.kept`).",
        ))?;
        b.capability(CapabilityDef::new(KEPT).refines(USERDEFS).docs(
            "The use sites of the `per-plan` user definitions are kept in the tree, for a \
             lowering rule to expand or to write as a call. The alternative to \
             `flm.userdefs.expanded`; a plan that demands both is refused.",
        ))?;
        let mut values = Vec::with_capacity(self.config.definitions.len());
        let mut names: BTreeSet<ElementName> = BTreeSet::new();
        let mut declared: Vec<(&Name, DefinitionKind, DefinitionModes)> = Vec::new();
        for definition in &self.config.definitions {
            let name = element_name(definition);
            // Two definitions of one name and kind are two definitions when the
            // parsing modes they apply in do not overlap, and a mistake when
            // they do: in an overlapping mode only one of the two could be in
            // force.
            let overlapping = declared.iter().any(|(earlier, kind, modes)| {
                *earlier == &definition.name
                    && *kind == definition.kind
                    && modes.overlaps(definition.modes)
            });
            if overlapping {
                return Err(BuildError::Feature {
                    feature: owner.clone(),
                    detail: format!(
                        "two definitions of `{}` must not apply in the same parsing mode",
                        definition.name
                    ),
                });
            }
            declared.push((&definition.name, definition.kind, definition.modes));
            // Their elements must differ too: everything that resolves an
            // element by its name trips over two element values under one name.
            if !names.insert(name.clone()) {
                return Err(BuildError::Feature {
                    feature: owner.clone(),
                    detail: format!(
                        "the definition of `{}` declares the element `{name}`, which an earlier \
                         definition declares too; two definitions must not declare the same \
                         element",
                        definition.name
                    ),
                });
            }
            let value = UserDefinition::new(definition.clone());
            value.record_recursion_limit(self.config.recursion_limit);
            // The builder stamps the feature on the declaration and on the
            // override itself; the owner handed over here only fills the
            // declarations' own fields.
            b.element(value.element_def(owner.clone(), name))?;
            if let Some(over) = value.override_def(owner.clone()) {
                b.override_callable(over.kind, over.name)?;
            }
            b.lowering_rule(expand_in_tree_rule(definition))?;
            if definition.expand == ExpandPolicy::PerPlan {
                b.lowering_rule(kept::instantiate_at_lowering_rule(definition))?;
                b.lowering_rule(kept::emit_call_rule(definition))?;
            }
            values.push(value);
        }
        kept::register(b)?;
        let recursion_limit = self.config.recursion_limit;
        b.rewrite_stage(Arc::new(ExpandStage { expands: Expands::Always, recursion_limit }))?;
        b.rewrite_stage(Arc::new(ExpandStage { expands: Expands::PerPlan, recursion_limit }))?;
        b.stage(Arc::new(KeepMarker))?;
        b.digest_input(&self.config.digest_bytes())?;
        b.late_init(Arc::new(CompileTemplates { definitions: values }))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::stage::{RewriteStage, Stage};

    #[test]
    fn an_element_is_named_by_its_kind_and_name() {
        assert_eq!(
            element_name(&Definition::macro_("foo", Vec::new())),
            "flm.userdefs.macro.foo"
        );
        assert_eq!(
            element_name(&Definition::environment("foo", Vec::new())),
            "flm.userdefs.environment.foo"
        );
        assert_eq!(
            element_name(&Definition::specials("~", Vec::new())),
            "flm.userdefs.specials.~"
        );
    }

    #[test]
    fn the_per_plan_stage_declares_the_default_refinement_of_userdefs() {
        let decl = ExpandStage { expands: Expands::PerPlan, recursion_limit: 1 }.decl();
        assert_eq!(decl.name, EXPAND);
        assert_eq!(decl.publishes, EXPANDED.name());
        assert_eq!(decl.refines.as_deref().map(|n| n.as_str()), Some(USERDEFS));
        assert!(decl.default);
    }

    #[test]
    fn the_always_stage_refines_nothing() {
        let decl = ExpandStage { expands: Expands::Always, recursion_limit: 1 }.decl();
        assert_eq!(decl.name, EXPAND_ALWAYS);
        assert_eq!(decl.publishes, ALWAYS_EXPANDED.name());
        assert!(decl.refines.is_none());
        assert!(!decl.default);
    }

    #[test]
    fn the_keep_marker_provides_kept_and_writes_nothing() {
        let decl = KeepMarker.decl();
        assert_eq!(decl.name, KEEP);
        assert_eq!(decl.provides, [crate::model::names::CapName::from(KEPT)]);
        assert!(decl.writes.is_empty());
    }
}
