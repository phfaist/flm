//! The handler traits of `flm:core/stage`: [`Stage`] and [`RewriteStage`].

use crate::api::core::stage::{Needs, RewriteDecl, StageDecl};
use crate::guest::core::data::store::StageOut;
use crate::guest::core::run::{RewriteCx, StageCx};
use crate::guest::techy::node::Node;
use crate::guest::techy::transform::{Restage, Restager};
use crate::runtime::ExtError;

/// Mirrors [`Stage`](crate::api::core::stage::Stage): a stage of an
/// extension.
pub trait Stage: Send + Sync {
    /// Mirrors [`Stage::decl`](crate::api::core::stage::Stage::decl): what the
    /// stage is.
    fn decl(&self) -> StageDecl;

    /// Mirrors [`Stage::needs`](crate::api::core::stage::Stage::needs): what
    /// it needs before it runs.
    fn needs(&self) -> Needs;

    /// Mirrors [`Stage::run`](crate::api::core::stage::Stage::run): runs the
    /// stage.
    ///
    /// # Errors
    ///
    /// What the stage answers.
    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError>;
}

/// Mirrors [`RewriteStage`](crate::api::core::stage::RewriteStage): a
/// rewrite stage of an extension.
pub trait RewriteStage: Send + Sync {
    /// Mirrors [`RewriteStage::decl`](crate::api::core::stage::RewriteStage::decl):
    /// what the rewrite stage is.
    fn decl(&self) -> RewriteDecl;

    /// Mirrors [`RewriteStage::needs`](crate::api::core::stage::RewriteStage::needs):
    /// what it needs before it runs.
    fn needs(&self) -> Needs;

    /// Mirrors [`RewriteStage::restage`](crate::api::core::stage::RewriteStage::restage):
    /// the rewrite of `node`.
    ///
    /// # Errors
    ///
    /// What the handler answers.
    fn restage(&self, node: &Node, cx: &RewriteCx, rs: &mut Restager) -> Result<Restage, ExtError>;
}
