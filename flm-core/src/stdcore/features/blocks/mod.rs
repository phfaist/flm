//! The `blocks` feature lays out every document tree in paragraphs and blocks
//! and resolves its text, in a rewrite stage that a plan includes when its
//! capability is demanded or required, and it checks the tree contract.
//!
//! # What the feature provides to an author
//!
//! An author separates paragraphs with a blank line, and a block-level
//! construct (a list, a display-math environment, a float) is placed between
//! paragraphs, never inside one. The typographic spellings of TeX are shown as
//! the characters they stand for: `--` as an en dash, ``` `` ``` and `''` as
//! curly quotes, `...` as an ellipsis, each family switched on or off by the
//! feature's configuration ([`BlocksConfig`]). The language guide describes
//! these rules for authors: [paragraphs, blocks and
//! typography](crate::guide::language_syntax#paragraphs-blocks-and-typography).
//!
//! The work is done by the block-structure pass
//! ([`crate::lang::rewrites::blocks`]), a plain function over a tree: it lays
//! out each content region (a run of sibling nodes laid out together) as
//! blocks, as inline content or unchanged, from the block role of each
//! element and the layout each element declares for its arguments and body,
//! and it resolves the text once (merged text nodes, collapsed whitespace, the
//! typographic substitutions). This feature makes that pass a part of the
//! pipeline.
//!
//! # What the feature provides to the pipeline
//!
//! A *capability* ([`CapabilityDef`]) is a registered dotted name for a state
//! of processing that a stage brings about; a stage declares the capabilities
//! it provides and requires, and a caller demands the capabilities it wants
//! ([stages, capabilities and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans)). The
//! feature registers:
//!
//! - [`BLOCKS`] (`flm.blocks`), the parent capability: a decision about block
//!   structure has been made. Two capabilities *refine* it
//!   ([refinement](crate::guide::pipeline#refinement)): each is one way of
//!   satisfying it, and a plan that demands both is refused.
//! - [`STRUCTURE`] (`flm.blocks.structure`), the rewrite stage
//!   ([`RewriteStage`]: a stage that turns one tree into another before the
//!   trees are frozen) that runs the block-structure pass over each tree, with
//!   the region layout of the tree's top level
//!   ([`FragmentLayout`](crate::model::doc::FragmentLayout)) and the feature's
//!   configuration. It publishes
//!   the trees it produces in the trees table [`STRUCTURED`]
//!   (`flm.blocks.structured`; a [`TreesTable`], the table in which a rewrite
//!   stage publishes the trees it produces), which is also the capability it
//!   provides, the refinement of `flm.blocks` marked as the default: a plan
//!   that demands `flm.blocks` alone gets structured trees.
//!   A lowering rule that reads paragraphs requires `flm.blocks.structured`
//!   (the rule for paragraphs, for example), which puts the stage into a
//!   rendering plan.
//! - [`RAW`] (`flm.blocks.raw`), a marker stage that does nothing and provides
//!   the capability of the same name, the other refinement of `flm.blocks`:
//!   the trees stay as parsed, without paragraphs and with the text unresolved.
//!   The lowering rules of the `latex-recomposed` backend require it, because
//!   that backend writes the document back as LaTeX source.
//! - [`CONTRACT_CHECK`] (`flm.blocks.check-contract`, in the module [`check`]),
//!   the check stage that runs the [tree-contract
//!   checker](crate::lang::rewrites::blocks::check) over every [frozen
//!   tree](crate::guide::concepts_overview#frozen-trees-and-computed-data) and
//!   provides [`CONTRACT_CHECKED`] (`flm.blocks.contract-checked`), a member of
//!   the collection [`flm.validated`](crate::stdcore::VALIDATED) that every
//!   standard workflow demands.
//! - The settings declaration of [`BlocksConfig`], whose fields are the
//!   feature's settings keys ([`crate::core::settings`]).
//!
//! # Configuration
//!
//! [`BlocksConfig`] says which families of typographic substitution apply. It
//! is the feature's configuration, the plain Rust value the feature is
//! constructed from ([`Blocks::new`]); it is fixed when the registry is built
//! and cannot vary per document or per render ([settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! The feature also gives the four flags of [`BlocksConfig`] as an input of
//! the [registry digest](crate::model::digest)
//! ([`FeatureBuilder::digest_input`]), a number computed from everything about
//! a registry that can change what processing produces: the trees a run
//! freezes depend on these flags, so two registries that differ in them must
//! not share cached trees.
//!
//! # Interaction with user definitions
//!
//! A use site of a [user
//! definition](crate::lang::rewrites::userdefs::Definition) (a place where a
//! document uses a macro, environment or special character sequence that is
//! defined by substitution; see [`crate::lang::rewrites::userdefs`]) that is
//! not yet expanded can hide a paragraph break that the definition's template
//! holds, so the structure stage must not run before the plan has decided
//! what happens to user definitions. Whenever the registry declares the
//! capability [`USERDEFS`] (`flm.userdefs`, from the
//! [`userdefs`](crate::stdcore::features::userdefs) feature), the structure
//! stage therefore requires it and is ordered after it. Either alternative of
//! that capability satisfies the requirement: the use sites expanded in the
//! tree (`flm.userdefs.expanded`), or kept in the tree for the lowering rules
//! (`flm.userdefs.kept`). Under the same condition, the stage is also ordered
//! after [`ALWAYS_EXPANDED`] (`flm.userdefs.always-expanded`), so that the
//! rewrite stage `flm.userdefs.expand-always` runs before the structure stage
//! whenever both are in the plan. An ordering never adds a stage to a
//! plan: a stage runs because something requires a capability it provides,
//! never because another stage is ordered after it. With no `userdefs` feature
//! registered there are no user definitions, and the structure stage has no
//! requirement.
//!
//! # When the pass fails
//!
//! When the block-structure pass fails over a tree, the stage returns a
//! [`RewriteError`] carrying one Error diagnostic, and the tree is absent from
//! every later table and from the frozen trees. Content nested more deeply than
//! the [recursion guard](crate::lang::guard) allows
//! (`flm.blocks.nesting-too-deep`) gives [`RewriteError::NestingTooDeep`];
//! when the failure happens inside a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), value catch-up
//! reports it again under a condition of its own
//! ([`StageCx::resolve_value`]).
//! Every other failure gives [`RewriteError::Failed`].
//!
//! # Items
//!
//! - [`Blocks`], the feature, constructed with [`Blocks::new`] from a
//!   [`BlocksConfig`] (re-exported here from the block-structure pass).
//! - [`FEATURE`], the feature's name; [`BLOCKS`], [`STRUCTURE`],
//!   [`STRUCTURED`] and [`RAW`], the names of its capabilities, stages and
//!   trees table.
//! - [`check`]: the contract check stage, with [`CONTRACT_CHECK`] and
//!   [`CONTRACT_CHECKED`].

pub mod check;

use std::sync::Arc;

use techy::core::node::NodeTree;

use crate::model::addr::TreeId;
use crate::core::settings::SettingsDecl;
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::run::{RewriteCx, StageCx};
use crate::core::stage::{
    Effect, Needs, RewriteDecl, RewriteError, RewriteStage, Scope, Stage, StageDecl, TreesTable,
};
use crate::core::data::store::StageOut;
use crate::core::data::table::TableName;
use crate::lang::rewrites::blocks::{StructureError, structure_with_guard};
use crate::lang::{Flm, FlmDiagnostics};
use crate::stdcore::VALIDATED;
use crate::stdcore::features::userdefs::{ALWAYS_EXPANDED, USERDEFS};

use self::check::ContractCheck;

pub use self::check::{CONTRACT_CHECK, CONTRACT_CHECKED};
pub use crate::lang::rewrites::blocks::BlocksConfig;

/// The feature's registered name, `blocks`. It is also the middle segment of
/// the names of the feature's stages and diagnostics (`flm.blocks.structure`,
/// `flm.blocks.nesting-too-deep`).
pub const FEATURE: &str = "blocks";

/// The parent capability `flm.blocks`: a decision about block structure has
/// been made. Two capabilities refine it, [`STRUCTURED`] (the default) and
/// [`RAW`]; a plan that demands both is refused. See the module documentation.
pub const BLOCKS: &str = "flm.blocks";

/// The name of the rewrite stage that runs the block-structure pass over each
/// tree, `flm.blocks.structure`. The stage provides [`STRUCTURED`].
pub const STRUCTURE: &str = "flm.blocks.structure";

/// The trees table `flm.blocks.structured`, in which the [`STRUCTURE`] stage
/// publishes the trees it produces. A rewrite stage's trees table has the name
/// of the capability the stage provides, so `flm.blocks.structured` is also
/// that capability: the refinement of [`BLOCKS`] marked as the default, so
/// that a plan that demands `flm.blocks` alone gets structured trees.
///
/// A stage that declares an optional read of this table learns whether the
/// plan structured the trees: the table is written exactly where the
/// structure stage ran.
pub const STRUCTURED: TableName<TreesTable> = TableName::new("flm.blocks.structured");

/// The name of the marker stage `flm.blocks.raw` and of the capability it
/// provides: the refinement of [`BLOCKS`] under which the trees stay as
/// parsed, without paragraphs and with the text unresolved. The marker stage
/// does nothing; it exists so that a plan can choose this alternative, which
/// excludes the [`STRUCTURE`] stage. The lowering rules of the
/// `latex-recomposed` backend require it.
pub const RAW: &str = "flm.blocks.raw";

/// The declaration of the feature's settings: its configuration,
/// [`BlocksConfig`], whose fields are the settings keys
/// ([`crate::core::settings`]).
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE).config::<BlocksConfig>(
        "Which families of typographic substitution apply when the text of a document is \
         resolved (curly quotes, dashes, ellipses). Fixed when the registry is built: it \
         cannot vary per document or per render.",
    )
}

/// The `blocks` feature ([`FlmFeature`]): it registers the block-structure
/// rewrite stage, the raw marker stage, the contract check stage, their
/// capabilities and the feature's settings. See the module documentation.
///
/// Construct it with [`Blocks::new`] from a [`BlocksConfig`], or with
/// [`Blocks::default`] for every typographic substitution on. The standard
/// package registers it from the `blocks` field of
/// [`StdcoreConfig`](crate::stdcore::StdcoreConfig).
#[derive(Debug, Clone)]
pub struct Blocks {
    config: BlocksConfig,
}

impl Blocks {
    /// Returns the feature with the configuration `config`, which stays fixed
    /// for the life of every registry the feature is built into.
    pub fn new(config: BlocksConfig) -> Blocks {
        Blocks { config }
    }

    /// Returns the configuration the feature was constructed with.
    pub fn config(&self) -> &BlocksConfig {
        &self.config
    }
}

impl Default for Blocks {
    /// Returns the feature with the default configuration
    /// ([`BlocksConfig::default`]): every family of typographic substitution
    /// on.
    fn default() -> Blocks {
        Blocks::new(BlocksConfig::default())
    }
}

impl FlmFeature for Blocks {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Block structure").docs(
            "Lays out every document tree in paragraphs and blocks, from the block role of \
             each element and the layout it declares for its arguments and body, and resolves \
             the text once: the `flm.blocks.structure` rewrite stage provides \
             `flm.blocks.structured`; the `flm.blocks.raw` marker stage keeps the trees as \
             parsed; the `flm.blocks.check-contract` stage checks the tree contract.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.capability(CapabilityDef::new(BLOCKS).docs(
            "A decision about block structure has been made: the trees are laid out in \
             paragraphs and blocks (`flm.blocks.structured`, the default) or kept as parsed \
             (`flm.blocks.raw`).",
        ))?;
        b.capability(CapabilityDef::new(RAW).refines(BLOCKS).docs(
            "The trees are kept as parsed: no paragraphs, and the text unresolved. The \
             alternative to `flm.blocks.structured`; a plan that demands both is refused.",
        ))?;
        b.rewrite_stage(Arc::new(StructureStage { config: self.config }))?;
        b.stage(Arc::new(RawMarker))?;
        b.capability(CapabilityDef::new(CONTRACT_CHECKED).docs(
            "Every frozen tree satisfies the parts of the tree contract that the plan's \
             trees are subject to, or its violations have been reported as \
             `flm.blocks.contract-violated`. A member of `flm.validated`.",
        ))?;
        b.stage(Arc::new(ContractCheck))?;
        b.collection_member(VALIDATED, CONTRACT_CHECKED)?;
        b.digest_input(&digest_bytes(&self.config))?;
        Ok(())
    }
}

/// Returns the configuration as the bytes the feature gives to the registry
/// digest: each flag as one byte, in the order the fields are declared.
///
/// The bytes are written out field by field rather than taken from the type's
/// `Debug` output, because that output would change with a field rename and
/// would change every registry digest with it.
fn digest_bytes(config: &BlocksConfig) -> [u8; 4] {
    [
        u8::from(config.auto_unicode_quotes),
        u8::from(config.ligature_unicode_quotes),
        u8::from(config.ligature_unicode_dashes),
        u8::from(config.ligature_unicode_ellipses),
    ]
}

/// The rewrite stage `flm.blocks.structure`: it runs the block-structure pass
/// over each tree, with the region layout of the tree's top level
/// ([`FragmentLayout`](crate::model::doc::FragmentLayout)), the feature's
/// configuration and the recursion guard's policy
/// ([`GuardPolicy`](crate::lang::guard::GuardPolicy)) that the registry's
/// language was built with.
struct StructureStage {
    config: BlocksConfig,
}

impl RewriteStage for StructureStage {
    fn decl(&self) -> RewriteDecl {
        RewriteDecl::new(STRUCTURE, STRUCTURED.name()).refines(BLOCKS).default().docs(
            "Lays out each tree in paragraphs and blocks, from the block role of each element \
             and the layout it declares for its arguments and body, and resolves the text once. \
             Publishes the trees as `flm.blocks.structured`, the default refinement of \
             `flm.blocks`.",
        )
    }

    /// Requires [`USERDEFS`] and is ordered after it, whenever the registry
    /// declares that capability; is ordered after [`ALWAYS_EXPANDED`] (an
    /// ordering only), whenever the registry declares it. See the module
    /// documentation, section "Interaction with user definitions".
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        // Why a requirement and not only an ordering edge: see design rationale
        // at [§flm-dd-dr:std-blocks-requires-userdefs].
        let mut needs = Needs::new();
        if cx.registry().cap_id(USERDEFS).is_some() {
            needs = needs.requires(USERDEFS).after(USERDEFS);
        }
        if cx.registry().cap_id(ALWAYS_EXPANDED.name()).is_some() {
            needs = needs.after(ALWAYS_EXPANDED.name());
        }
        needs
    }

    fn rewrite(
        &self,
        _tree_id: TreeId,
        tree: &Arc<NodeTree<Flm>>,
        cx: &RewriteCx<'_>,
        diags: &mut FlmDiagnostics,
    ) -> Result<Arc<NodeTree<Flm>>, RewriteError> {
        let guard = cx.registry().lang().guard();
        structure_with_guard(tree, cx.layout(), &self.config, guard, diags).map_err(|error| {
            let diagnostic = error.diagnostic(tree);
            match error {
                StructureError::NestingTooDeep { .. } => RewriteError::NestingTooDeep(diagnostic),
                _ => RewriteError::Failed(diagnostic),
            }
        })
    }
}

/// The marker stage `flm.blocks.raw` ([`RAW`]): it does nothing and provides
/// the capability of the same name, the refinement of `flm.blocks` under which
/// the trees stay as parsed.
struct RawMarker;

impl Stage for RawMarker {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RAW, Effect::Compute, Scope::PerTree).provides(RAW).docs(
            "Does nothing and provides `flm.blocks.raw`, so that a plan that requires the \
             trees as parsed excludes the `flm.blocks.structure` rewrite stage.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_bytes_follow_the_flags() {
        assert_eq!(digest_bytes(&BlocksConfig::default()), [1, 1, 1, 1]);
        assert_eq!(digest_bytes(&BlocksConfig::no_typography()), [0, 0, 0, 0]);
        let config = BlocksConfig { ligature_unicode_dashes: false, ..BlocksConfig::default() };
        assert_eq!(digest_bytes(&config), [1, 1, 0, 1]);
    }

    #[test]
    fn the_structure_stage_declares_the_default_refinement_of_blocks() {
        let decl = StructureStage { config: BlocksConfig::default() }.decl();
        assert_eq!(decl.name, STRUCTURE);
        assert_eq!(decl.publishes, STRUCTURED.name());
        assert_eq!(decl.refines.as_deref().map(|n| n.as_str()), Some(BLOCKS));
        assert!(decl.default);
    }

    #[test]
    fn the_raw_marker_provides_its_own_name_and_writes_nothing() {
        let decl = RawMarker.decl();
        assert_eq!(decl.name, RAW);
        assert_eq!(decl.provides, [crate::model::names::CapName::from(RAW)]);
        assert!(decl.writes.is_empty());
    }
}
