//! This module defines the `pieces` feature ([`PiecesFeature`]), along with
//! the functions that select the pieces placed during a render.
//!
//! A *piece* is a part of the output that a backend places before or after the
//! document's content, such as the document's title or the bibliography
//! ([`Piece`](crate::core::piece::Piece)). The `pieces` feature registers two
//! things:
//!
//! - the service slot [`SLOT`] (`flm.pieces`), to which each feature that
//!   offers a piece contributes the declaration of the piece ([`PieceDecl`]);
//! - the settings `include` and `exclude` ([`PiecesRenderOptions`]), with which
//!   the pieces to be placed during a render are selected.
//!
//! The feature registers no stage and no table. Each backend that places
//! pieces registers its own pieces stage ([`PiecesStage`](super::PiecesStage))
//! and the table that this stage writes.
//!
//! The declarations of the pieces are values contributed to a service slot
//! ([`crate::core::services`]), and not a kind of definition in the registry.
//! A declaration is data that the pieces stages read: a name, a table, a place
//! and a default. The registry has nothing to check or to schedule for it.
//!
//! A registry that contains a backend with a pieces stage must also contain the
//! `pieces` feature. Otherwise, building a plan for this backend fails with an
//! error that identifies the option type [`PiecesRenderOptions`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::piece::PieceDecl;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry};
use crate::core::services::Arity;
use crate::model::names::{BackendName, Name};
use crate::model::options::Bind;

/// The name of the `pieces` feature, `pieces`.
///
/// The identifiers of the conditions that the pieces stages report begin with
/// `flm.pieces.` ([`super::error`]).
pub const FEATURE: &str = "pieces";

/// Returns the settings declaration of the `pieces` feature. The fields of
/// [`PiecesRenderOptions`] are the settings of the feature, set on the render
/// request.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .options::<PiecesRenderOptions>(
            Bind::Render,
            "The pieces placed during a render in addition to those placed by default \
            (`include`), and the pieces placed by default that are left out of a render \
            (`exclude`). Set per render request.",
        )
}

/// The name of the service slot `flm.pieces`, to which features contribute the
/// declarations of their pieces.
///
/// Each value of this service slot is a [`PieceDecl`]. The service slot keeps
/// the declarations in the order of their contributions. A backend places two
/// pieces with the same place in this order.
///
/// A feature that offers a piece contributes its declaration with
/// [`FeatureBuilder::peer_slot`], so that the feature can also be used in a
/// registry without the `pieces` feature. In such a registry, the declaration
/// is dropped, and the registry build reports a note.
pub const SLOT: &str = "flm.pieces";

/// The render option with which the pieces placed during a render are
/// selected.
///
/// A piece is placed during a render if it is listed in `include`, or if it is
/// placed by default
/// ([`PieceDecl::default_on`](crate::core::piece::PieceDecl#structfield.default_on))
/// and is not listed in `exclude`. A piece that is listed in both sets is
/// therefore placed. The two sets apply to every backend of the render.
///
/// The field [`backends`](Self::backends) restricts this selection for each
/// backend separately:
///
/// - On a backend that has an entry in `backends`, only the selected pieces
///   listed in this entry are placed.
/// - On a backend without an entry, all selected pieces are placed, unless the
///   pieces stage of the backend places only the pieces listed for it
///   ([`PiecesStage::placing_only_named`](super::PiecesStage::placing_only_named)).
///   In that case, no piece is placed on this backend.
///
/// The function [`selected_pieces`] applies these rules.
///
/// The option is set on the render request ([`Bind::Render`]). A piece that is
/// not placed during a render is not built: the stage that builds the content
/// of the piece is not scheduled in the plan. A name in `include` or `exclude`
/// that is not the name of a piece defined in any feature is ignored, and
/// reported as `flm.pieces.unknown-piece`
/// ([`UnknownPiece`](super::UnknownPiece)).
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::backends::pieces::PiecesRenderOptions;
///
/// let options = PiecesRenderOptions::default().exclude("mypkg.notes");
/// assert!(options.exclude.iter().any(|name| name.as_str() == "mypkg.notes"));
/// assert!(options.include.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PiecesRenderOptions {
    /// Pieces to be placed during a render although they are not placed by
    /// default, such as `flm.toc`.
    pub include: BTreeSet<Name>,
    /// Pieces placed by default that are left out of a render.
    pub exclude: BTreeSet<Name>,
    /// For each backend that has an entry, the only pieces that the backend
    /// may place.
    ///
    /// A host sets this field from the pieces that its compose template
    /// accepts
    /// ([`select_pieces`](crate::stdlib::features::compose::select_pieces)),
    /// so that a piece for which the template has no place is never built. This
    /// field cannot be set as a setting.
    #[serde(skip)]
    #[schemars(skip)]
    pub backends: BTreeMap<BackendName, BTreeSet<Name>>,
}

impl PiecesRenderOptions {
    /// Adds the piece `name` to the set
    /// [`include`](PiecesRenderOptions#structfield.include), and returns the
    /// options.
    pub fn include(mut self, name: impl Into<Name>) -> PiecesRenderOptions {
        self.include.insert(name.into());
        self
    }

    /// Adds the piece `name` to the set
    /// [`exclude`](PiecesRenderOptions#structfield.exclude), and returns the
    /// options.
    pub fn exclude(mut self, name: impl Into<Name>) -> PiecesRenderOptions {
        self.exclude.insert(name.into());
        self
    }
}

/// Returns the declarations of all pieces of `registry`, in the order of the
/// service slot [`SLOT`].
///
/// In a registry without the `pieces` feature, no piece is declared, and the
/// result is empty.
pub fn declared_pieces(registry: &FlmRegistry) -> impl Iterator<Item = &PieceDecl> + '_ {
    registry.services().services_named::<PieceDecl>(SLOT)
}

/// Returns the declarations of the pieces placed on `backend` during a render
/// with `options`, in the order of the service slot [`SLOT`].
///
/// The selection follows the rules described at [`PiecesRenderOptions`]. The
/// flag `only_named` applies when `backend` has no entry in
/// [`PiecesRenderOptions::backends`]. In that case, the function returns all
/// pieces that `include` and `exclude` select if `only_named` is `false`, and
/// no piece if `only_named` is `true`.
///
/// The pieces stage of a backend calls this function with its own flag
/// ([`PiecesStage::places_only_named`](super::PiecesStage::places_only_named)),
/// first when the plan is built, then when the stage runs. When the plan is
/// built, the stages that build the returned pieces are scheduled. When the
/// stage runs, it lowers the returned pieces.
///
/// In a registry without the `pieces` feature, no piece is declared, and the
/// result is empty.
pub fn selected_pieces(
    registry: &FlmRegistry,
    options: &PiecesRenderOptions,
    backend: &BackendName,
    only_named: bool,
) -> Vec<PieceDecl> {
    let named = options.backends.get(backend);
    if named.is_none() && only_named {
        return Vec::new();
    }
    declared_pieces(registry)
        .filter(|decl| {
            (decl.default_on && !options.exclude.contains(&decl.name))
                || options.include.contains(&decl.name)
        })
        .filter(|decl| named.is_none_or(|names| names.contains(&decl.name)))
        .cloned()
        .collect()
}

/// Returns the names in `include` and `exclude` of `options` that are not the
/// names of pieces declared in `registry`, sorted and without duplicates.
///
/// The pieces stage of each backend reports each of these names as
/// `flm.pieces.unknown-piece` ([`UnknownPiece`](super::UnknownPiece)).
pub fn unknown_pieces(registry: &FlmRegistry, options: &PiecesRenderOptions) -> Vec<Name> {
    let declared: BTreeSet<&Name> = declared_pieces(registry).map(|decl| &decl.name).collect();
    let named: BTreeSet<&Name> = options.include.iter().chain(&options.exclude).collect();
    named.into_iter().filter(|name| !declared.contains(name)).cloned().collect()
}

/// The `pieces` feature, which registers the service slot [`SLOT`] and the
/// settings [`PiecesRenderOptions`] (see the [module
/// documentation](mod@super::feature)).
///
/// Use [`feature()`] to obtain the feature as a shared value.
#[derive(Debug, Default)]
pub struct PiecesFeature;

impl FlmFeature for PiecesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Pieces").docs(
            "The `pieces` feature provides the service slot `flm.pieces`, to which features \
             contribute the declarations of their pieces. A piece is a part of the output that a \
             backend places before or after the document's content, such as the title or the \
             bibliography. The pieces to be rendered are selected with the `include` and \
             `exclude` settings of this feature.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.service_slot::<PieceDecl>(
            SLOT,
            Arity::OrderedMany,
            "The declarations of the pieces that the features offer, the parts that a backend \
             places before or after the document's content. A backend places two pieces with \
             the same place in the order of their declarations.",
        )?;
        Ok(())
    }
}

/// Returns the `pieces` feature as a shared value, for instance to list it in
/// a package entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::backends::pieces;
///
/// assert_eq!(pieces::feature().info().name.as_str(), pieces::FEATURE);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(PiecesFeature)
}
