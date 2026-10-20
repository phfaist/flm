//! The `input` feature: `\input{file}`, which brings the content of another file into
//! the document where the command is written.
//!
//! # What an author writes, and what it produces
//!
//! An author writes `\input{chapter.flm}` to include the file `chapter.flm`. The
//! argument is the reference to the file, as plain characters; the embedding
//! program's input resolver ([`InputResolver`](crate::lang::InputResolver)) finds and
//! reads the file it refers to (the `flm` program reads it relative to the folder of
//! the file that holds the command). The guide to the language describes the command
//! for authors ([including files](crate::guide::language_syntax#including-files)).
//!
//! An included file is one of two kinds, as its front matter declares (see [content
//! inputs and heading
//! inputs](crate::lang::input#content-inputs-and-heading-inputs)):
//!
//! - A **content input** continues the text around the command: its content is parsed
//!   where the command is written, and every output shows it as if it had been written
//!   there. A paragraph begun before the command continues into the included content.
//! - A **heading input** opens with a heading and becomes a tree of its own in the
//!   document ([`crate::model::assemble`]). The command stays in the tree as a block,
//!   and every output writes the included tree where the command is.
//!
//! When the file could not be brought in (its reference is not plain characters, no
//! file is found, the file includes itself, …), the parse reports the problem at the
//! command, and every output shows a mark where the content is missing (the dropped
//! mark `flm.pres.dropped`,
//! [`lowering::error`](crate::stdcore::backends::lowering::error), which gives the
//! reference).
//!
//! # How a parse handles the command
//!
//! The feature declares the command as an inclusion macro of the language
//! ([`SyntaxDef::Inclusion`]), whose one argument is the reference. What happens while
//! the document is parsed (the embedding program asked for the file, the content of a
//! content input parsed where the command is written) is done by the language layer
//! ([`crate::lang::input`]).
//!
//! # The splicing rewrite
//!
//! A rewrite stage runs before the document's trees are frozen and turns one tree into
//! another ([`RewriteStage`]). The feature's rewrite stage [`SPLICE`]
//! (`flm.input.splice`) replaces every command whose file was brought in by the file's
//! content, in place of the command among its sibling nodes
//! ([`crate::lang::rewrites::input`]), and publishes the trees as [`SPLICED`]
//! (`flm.input.spliced`). It runs before the rewrites of the user definitions and of
//! the block structure, so that they, and every later stage, see one text: a paragraph
//! begun before the command continues into the included content, a list written there
//! nests in the list around the command, and a user definition used there is
//! expanded. The capability `flm.input.spliced` is a member of the collection
//! [`flm.validated`](crate::stdcore::VALIDATED), which every standard workflow
//! demands, so the rewrite runs in every plan of the standard package.
//!
//! # The lowering rule
//!
//! Only two kinds of `\input` command remain in the trees after the splicing rewrite,
//! and the rule [`RULE`] (`flm.input.lower.default`, [`InputRule`]), the default on
//! every backend, lowers both:
//!
//! - **The command of a heading input.** The command places the included file's tree.
//!   For each such command in the rewritten trees, the run records the tree the command
//!   places, and the document order enters that tree at the command
//!   ([`crate::core::run::order`]). The rule asks which tree
//!   the command places ([`Lowerer::placement`]) and lowers the command to a
//!   [`flm.pres.subtree`](crate::stdcore::backends::pres::SUBTREE) node that gives that
//!   tree, which every output writes where the command is.
//! - **A command whose file was not brought in.** The parse reported the problem, and
//!   the rule lowers the command to a
//!   [`flm.pres.dropped`](crate::stdcore::backends::pres::DROPPED) mark that gives the
//!   reference (`\input{chapter.flm}`), on every output, so that a reader sees where
//!   content is missing.
//!
//! A command of a heading input that a rewrite copied (a user definition whose
//! template writes twice the argument that holds the command) places its file at its
//! first copy only, because a file is placed at one place of the document. The run
//! reports each other copy (`flm.run.placed-twice`), and the rule lowers it to the same
//! dropped mark.
//!
//! # Items
//!
//! - The feature: [`Input`], [`feature`], [`FEATURE`].
//! - The element: [`ELEMENT`] (`flm.input`), and the argument role
//!   [`PATH_ARG_ROLE`] (`flm.input.path`), which states that an argument holds
//!   a reference to another file whose content is read in
//!   ([`ArgDef::role`](crate::lang::mint::ArgDef::role)).
//! - The rewrite stage: [`SPLICE`], [`SPLICED`].
//! - The lowering rule: [`RULE`], [`InputRule`], [`rule`].

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    chars_at_lowering,
};
use crate::core::plan::PlanCx;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::run::RewriteCx;
use crate::core::stage::{Needs, RewriteDecl, RewriteError, RewriteStage, TreesTable};
use crate::lang::input::{is_placing, REFERENCE_ARGUMENT};
use crate::stdcore::backends::pres::SubtreePayload;
use crate::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use crate::lang::rewrites::input::{SpliceFailure, splice};
use crate::lang::{Flm, FlmDiagnostics};
use crate::model::addr::TreeId;
use crate::model::aspects::{ApartFromText, BlockRole, BlockRoleAspect, ContentRef};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, FeatureName, Name};
use crate::core::data::table::TableName;
use crate::stdcore::VALIDATED;
use crate::stdcore::backends::pres::{self, DroppedPayload};
use crate::stdcore::features::blocks::STRUCTURED;
use crate::stdcore::features::userdefs::{ALWAYS_EXPANDED, EXPANDED};

/// The feature's registered name, `input`.
pub const FEATURE: &str = "input";

/// The name of the element of the command `\input`, `flm.input`.
pub const ELEMENT: &str = "flm.input";

/// The name of the splicing rewrite stage, `flm.input.splice`, which replaces
/// every command whose file was brought in by the file's content.
pub const SPLICE: &str = "flm.input.splice";

/// The table of trees that the splicing rewrite publishes, which is also the
/// capability it provides, `flm.input.spliced`: the content of every included
/// file is in place of its command, among the command's sibling nodes.
pub const SPLICED: TableName<TreesTable> = TableName::new("flm.input.spliced");

/// The name of the lowering rule of the command of a heading input and of a
/// command whose file was not brought in, `flm.input.lower.default`
/// ([`InputRule`]).
pub const RULE: &str = "flm.input.lower.default";

/// The argument role of a reference to another file whose content is read
/// in, `flm.input.path`: the role of the reference argument
/// ([`REFERENCE_ARGUMENT`]) of `\input`.
pub const PATH_ARG_ROLE: &str = "flm.input.path";

/// The [`ApartFromText`] aspect of the command: the reference is a file name, not
/// part of the words of the text around it.
#[derive(Debug, Clone, Copy)]
struct InputText;

impl ApartFromText for InputText {
    fn apart_from_text(&self, _node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
        vec![ContentRef::ArgumentNamed(REFERENCE_ARGUMENT.to_string())]
    }
}

/// The block role of the command: a command that places a heading input's tree
/// is a block; any other is inline content (the command of a content input is
/// replaced by its content before the block-structure pass runs).
#[derive(Debug, Clone, Copy)]
struct InputBlockRole;

impl BlockRoleAspect for InputBlockRole {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        if is_placing(node) { BlockRole::Block } else { BlockRole::DEFAULT }
    }
}

/// Returns the element value of the command: the element [`ELEMENT`] with its
/// block role ([`InputBlockRole`]) and its [`ApartFromText`] aspect
/// ([`InputText`]).
fn element() -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(ELEMENT)
            .docs(
                "`\\input{file}`: the content of another file, brought in where the command \
                 is written.",
            )
            .with::<dyn BlockRoleAspect>(Arc::new(InputBlockRole))
            .with::<dyn ApartFromText>(Arc::new(InputText)),
    )
}

/// The splicing rewrite stage [`SPLICE`]. See the [module
/// documentation](self#the-splicing-rewrite).
struct SpliceStage;

impl RewriteStage for SpliceStage {
    fn decl(&self) -> RewriteDecl {
        RewriteDecl::new(SPLICE, SPLICED.name()).docs(
            "Replaces every `\\input` whose file was brought in by the file's content, in place \
             of the command among its sibling nodes. Publishes the trees as \
             `flm.input.spliced`; runs before the expansion of user definitions and before \
             block structure.",
        )
    }

    /// Ordering only: the stage runs before the rewrites of the user definitions
    /// and of the block structure, for those of them the registry declares.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        // See decision rationale at [§flm-dd-dr:std-input-splice-scheduled].
        let mut needs = Needs::new();
        for later in [ALWAYS_EXPANDED.name(), EXPANDED.name(), STRUCTURED.name()] {
            if cx.registry().cap_id(later).is_some() {
                needs = needs.before(later);
            }
        }
        needs
    }

    fn rewrite(
        &self,
        _tree_id: TreeId,
        tree: &Arc<NodeTree<Flm>>,
        cx: &RewriteCx<'_>,
        _diags: &mut FlmDiagnostics,
    ) -> Result<Arc<NodeTree<Flm>>, RewriteError> {
        splice(tree, cx.registry().lang().guard()).map_err(|failure| match failure {
            SpliceFailure::NestingTooDeep(diagnostic) => RewriteError::NestingTooDeep(diagnostic),
            SpliceFailure::Failed(diagnostic) | SpliceFailure::WorkerThread(diagnostic) => {
                RewriteError::Failed(diagnostic)
            }
        })
    }
}

/// The implementation of the lowering rule [`RULE`]: it lowers the command of a
/// heading input to a `flm.pres.subtree` node that gives the tree the command
/// places, and a command whose file was not brought in to a `flm.pres.dropped`
/// mark that gives the reference. The command of a content input never reaches
/// the rule, because the splicing rewrite replaced it. See the [module
/// documentation](self#the-lowering-rule).
#[derive(Debug, Default, Clone, Copy)]
pub struct InputRule;

impl Lower for InputRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        if is_placing(&node) {
            if let Some(tree) = lw.placement() {
                let subtree = lw.pres_with(cx, &ElementName::from(pres::SUBTREE), SubtreePayload { tree }, Vec::new())?;
                return Ok(Lowering::Emit(vec![subtree]));
            }
        }
        let reference = match node.argument_content_nodes_named(REFERENCE_ARGUMENT) {
            Ok(Some(content)) => chars_at_lowering(content.iter())?.unwrap_or_default(),
            _ => String::new(),
        };
        // See decision rationale at [§flm-dd-dr:std-input-dropped-mark].
        let mark = DroppedPayload {
            condition: Name::from(ELEMENT),
            detail: format!("\\input{{{reference}}}"),
        };
        let dropped = lw.pres_with(cx, &ElementName::from(pres::DROPPED), mark, Vec::new())?;
        Ok(Lowering::Emit(vec![dropped]))
    }
}

/// Returns the lowering rule [`RULE`], the default of the element on every
/// backend.
pub fn rule() -> LoweringRule {
    LoweringRule::new(
        RULE,
        ELEMENT,
        LoweringDecl::new().produces(pres::SUBTREE).produces(pres::DROPPED),
        Arc::new(InputRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "an `\\input` that places a heading input's tree lowers to a `flm.pres.subtree` node \
         that gives the tree; one whose file was not brought in, to a `flm.pres.dropped` mark \
         that gives the reference",
    )
}

/// The `input` feature: it registers the command `\input`, the splicing
/// rewrite stage [`SPLICE`] and its membership in the collection
/// `flm.validated`, and the lowering rule [`RULE`]. See the [module
/// documentation](self).
#[derive(Debug, Default, Clone, Copy)]
pub struct Input;

impl FlmFeature for Input {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Included files").docs(
            "`\\input{file}`: the content of another file, parsed where the command is written \
             and put in place of the command by the rewrite `flm.input.splice`, which \
             publishes `flm.input.spliced`.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        b.argument_role(
            PATH_ARG_ROLE,
            "A reference to another file, usually its path, whose content is read in where \
             the command is written.",
        )?;
        b.element(
            ElementDef::new(
                owner,
                ["input"],
                SyntaxDef::Inclusion {
                    args: vec![ArgDef::named(REFERENCE_ARGUMENT, ArgStdKind::CharsGroup)
                        .docs(
                            "The reference to the file, as plain characters; the embedding \
                             program's input resolver finds the file it refers to.",
                        )
                        .role(PATH_ARG_ROLE)],
                },
                element(),
            )
            .docs("`\\input{file}`: the content of another file, where the command is written."),
        )?;
        b.rewrite_stage(Arc::new(SpliceStage))?;
        b.collection_member(VALIDATED, SPLICED.name())?;
        b.lowering_rule(rule())?;
        Ok(())
    }
}

/// Returns the [`Input`] feature, ready to be listed in a package entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Input)
}
