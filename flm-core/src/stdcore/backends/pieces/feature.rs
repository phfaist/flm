//! The `pieces` feature: the service slot into which features declare their
//! pieces, the render option that chooses which pieces a render places, and
//! the function that applies it.
//!
//! The `pieces` feature ([`PiecesFeature`], an [`FlmFeature`]) registers the
//! service slot [`SLOT`] (`flm.pieces`), to which a feature contributes the
//! [`PieceDecl`] of each piece it offers, and the option type
//! [`PiecesRenderOptions`], set per render request. A **piece** is a
//! document-level synthesized value that a backend places before or after the
//! document's content ([`Piece`](crate::core::piece::Piece)). The feature registers no stage, no
//! table and no element: each backend that places pieces creates a
//! [`PiecesStage`](super::PiecesStage) itself and declares its own
//! lowered-pieces table, as it does for its lowering stage.
//!
//! A service slot is the declaration, by the feature that owns an interaction,
//! of a dotted name, the type of the values the slot takes and its arity; any
//! feature, and the embedding program through the registry builder, may
//! contribute a value to it ([`crate::core::services`]). The piece
//! declarations are contributions to a service slot rather than a kind of
//! registry entity because a piece is only data that another feature's stage
//! reads (a name, a table, a place and a default), and nothing the registry
//! itself has to check or schedule. A feature offering a piece contributes to
//! the slot as a peer contribution ([`FeatureBuilder::peer_slot`]), so that
//! the feature also builds in a registry without the `pieces` feature.
//!
//! A registry holding a backend whose stages read pieces must hold this
//! feature too: the backend's pieces stage reads [`PiecesRenderOptions`], and
//! a plan of that backend fails, with an error that identifies the option
//! type, in a registry that does not declare it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::piece::PieceDecl;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry};
use crate::core::services::Arity;
use crate::model::names::{BackendName, Name};
use crate::model::options::Bind;

/// The feature's registered name, `pieces`; also the middle segment of the
/// identifiers of the conditions the pieces stages report, `flm.pieces.*`.
pub const FEATURE: &str = "pieces";

/// The declaration of the feature's settings: the fields of
/// [`PiecesRenderOptions`] are its settings keys. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .options::<PiecesRenderOptions>(
            Bind::Render,
            "the pieces placed although they are declared off by default (`include`), and the \
            pieces declared on by default that are left out (`exclude`). Set per render \
            request.",
        )
}

/// The name of the service slot `flm.pieces`: every contribution is a
/// [`PieceDecl`], kept in contribution order, which is the order in which a
/// backend places two pieces at the same place.
///
/// A feature offering a piece contributes its declaration with
/// [`FeatureBuilder::peer_slot`], so that the feature builds in a registry
/// without the `pieces` feature; the declaration is then dropped with a
/// Note.
pub const SLOT: &str = "flm.pieces";

/// The render option that chooses which declared pieces a render places,
/// beyond the defaults, and which of them each backend may place.
///
/// A render places every piece declared on by default
/// ([`PieceDecl::default_on`](crate::core::piece::PieceDecl#structfield.default_on))
/// that `exclude` does not list, and every piece `include` lists; a piece in
/// both sets is therefore placed. On a backend that has an entry in
/// [`backends`](Self::backends), the render places only those of them the
/// entry lists. On a backend without an entry, it places those of them the
/// backend places by default: all of them, or none for a backend whose
/// pieces stage places only the pieces listed for it
/// ([`PiecesStage::placing_only_named`](super::PiecesStage::placing_only_named)).
/// [`selected_pieces`] is the only function that applies these rules.
///
/// The option is declared with [`Bind::Render`]: it is part of the render
/// request and of the plan key, since the pieces a render places decide
/// which builder stages the plan schedules. A piece left out schedules
/// nothing. The two sets apply to every backend of the render alike. A piece
/// name in them that no feature declares is reported
/// (`flm.pieces.unknown-piece`) and ignored.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
///
/// let options = PiecesRenderOptions::default().exclude("mypkg.notes");
/// assert!(options.exclude.iter().any(|name| name.as_str() == "mypkg.notes"));
/// assert!(options.include.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PiecesRenderOptions {
    /// Pieces placed although they are declared off by default.
    pub include: BTreeSet<Name>,
    /// Pieces declared on by default that the render request leaves out.
    pub exclude: BTreeSet<Name>,
    /// For each backend with an entry here, the pieces it may place: of the
    /// pieces the two sets above select, those this entry lists. It is not a
    /// setting written as data: a host that places the output with a template
    /// of its own sets it from what that template accepts, so that a piece no
    /// template places is never built.
    #[serde(skip)]
    #[schemars(skip)]
    pub backends: BTreeMap<BackendName, BTreeSet<Name>>,
}

impl PiecesRenderOptions {
    /// Builder: places the piece `name` although it is declared off by
    /// default.
    pub fn include(mut self, name: impl Into<Name>) -> PiecesRenderOptions {
        self.include.insert(name.into());
        self
    }

    /// Builder: leaves out the piece `name`, declared on by default.
    pub fn exclude(mut self, name: impl Into<Name>) -> PiecesRenderOptions {
        self.exclude.insert(name.into());
        self
    }
}

/// Every piece the registry declares, in slot order.
pub fn declared_pieces(registry: &FlmRegistry) -> impl Iterator<Item = &PieceDecl> + '_ {
    registry.services().services_named::<PieceDecl>(SLOT)
}

/// The pieces a render places on `backend`, in slot order: every declared
/// piece that is on by default and not excluded, and every included one; of
/// those, the ones [`backends`](PiecesRenderOptions::backends) lists for
/// `backend` when it has an entry for it, otherwise all of them when
/// `only_named` is false and none when it is true.
///
/// This is the only function that decides which pieces a render places. A
/// backend's pieces stage calls it once when the plan is built, so that the
/// plan schedules the builders of the pieces it returns and no others, and
/// again when the stage runs, each time with the stage's own `only_named`
/// ([`PiecesStage::placing_only_named`](super::PiecesStage::placing_only_named)).
/// A registry without the `pieces` feature declares no piece, and the result
/// is empty.
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

/// The piece names in the two sets of `options` that no declared piece
/// carries, in name order: what a pieces stage reports as
/// `flm.pieces.unknown-piece`.
pub fn unknown_pieces(registry: &FlmRegistry, options: &PiecesRenderOptions) -> Vec<Name> {
    let declared: BTreeSet<&Name> = declared_pieces(registry).map(|decl| &decl.name).collect();
    let named: BTreeSet<&Name> = options.include.iter().chain(&options.exclude).collect();
    named.into_iter().filter(|name| !declared.contains(name)).cloned().collect()
}

/// The `pieces` feature (see the module documentation).
///
/// Use [`feature()`] to obtain it as a value a package entry can list.
#[derive(Debug, Default)]
pub struct PiecesFeature;

impl FlmFeature for PiecesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Pieces").docs(
            "the service slot into which features declare the document-level values a backend \
             places before or after the content, and the render option that chooses which of \
             them a render request places",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.service_slot::<PieceDecl>(
            SLOT,
            Arity::OrderedMany,
            "the pieces each feature offers (document-level values a backend places before or \
             after the content), in the order a backend places them",
        )?;
        Ok(())
    }
}

/// The `pieces` feature as a shared value, ready to be listed in a package
/// entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pieces;
///
/// assert_eq!(pieces::feature().info().name.as_str(), pieces::FEATURE);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(PiecesFeature)
}
