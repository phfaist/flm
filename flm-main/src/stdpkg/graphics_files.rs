//! The feature `graphics-files` of the package `stdmain` (cargo feature `graphics`): it
//! contributes the host's services for image files ([`crate::graphics`]) to the library's
//! `graphics` feature ([`flm_core::stdcore::features::graphics`]).
//!
//! The library's `graphics` feature handles `\includegraphics` and publishes each image
//! on each output, but opens no file itself: it declares three service slots for the work
//! that needs files or programs of the machine. A service slot is the declaration, by the
//! feature that owns an interaction, of a dotted name, the type of the values the slot
//! takes and its arity ([`flm_core::core::services`]). The feature `graphics-files`
//! contributes:
//!
//! - the host's graphics resolver, [`FileResolver`], to the service slot
//!   `flm.graphics.resolver`;
//! - the host's inspector, [`FileInspector`], to the service slot `flm.graphics.inspector`;
//! - two converters, to the service slot `flm.graphics.converters`, in this order:
//!   [`RasterConverter`], which converts in the program's own code, and
//!   [`MagickConverter`], which runs ImageMagick's program `magick` and so converts every
//!   pair of formats that the first one does not offer.
//!
//! The three service slots belong to the `graphics` feature, so these are peer
//! contributions ([`FeatureBuilder::peer_slot`]): where the `graphics` feature is switched
//! off, the registry build drops them and records one note
//! ([`UnresolvedPeer`](flm_core::core::registry::UnresolvedPeer)) per service slot among
//! its build diagnostics.
//!
//! # Settings
//!
//! The feature's configuration is [`GraphicsFilesConfig`]: `search_path`, the folders in
//! which an image file is looked for. A feature's configuration is the plain Rust value
//! the feature is constructed from; it is fixed when the registry is built and cannot vary
//! per document or per render ([`flm_core::core::settings`]).
//!
//! How each output publishes its images and which conversions it applies are settings of
//! the library's `graphics` feature (`strategy` and `convert`,
//! [`GraphicsRenderOptions`](flm_core::stdcore::features::graphics::GraphicsRenderOptions)),
//! for which the `flm` program sets defaults of its own ([`super::defaults`]). The program
//! that the converter `magick` runs is not a setting: the program embedding the host gives
//! it ([`GraphicsFilesFeature::new`]).

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::settings::SettingsDecl;
use flm_core::stdcore::features::graphics::{
    CONVERTERS_SLOT, GraphicsConverter, GraphicsInspector, GraphicsResolver, INSPECTOR_SLOT, RESOLVER_SLOT,
};

use crate::files::ServiceFiles;
use crate::graphics::{FileInspector, FileResolver, MagickConverter, RasterConverter};
use crate::output::OutputFolder;

/// The name under which the feature is registered, `graphics-files`.
pub const FEATURE: &str = "graphics-files";

/// The configuration of the feature `graphics-files`: where image files are looked for.
/// It is fixed when the registry is built and cannot vary per document or per render that
/// one registry serves. The `flm` program builds the registry of each document from that
/// document's settings, so a document writes it under the feature's name, for instance
/// `flm: {features: {graphics-files: {search_path: [figures, .]}}}` in the front matter.
/// The program that the converter `magick` runs is not part of it: it is a fact of the
/// machine, which the program embedding the host gives ([`GraphicsFilesFeature::new`]),
/// so that no document can choose a program that the host starts.
///
/// # Examples
///
/// ```
/// use flm_main::stdpkg::graphics_files::GraphicsFilesConfig;
///
/// let config = GraphicsFilesConfig::default();
/// assert_eq!(config.search_path, ["."]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct GraphicsFilesConfig {
    /// The folders in which the file of an image is looked for, in order: the first
    /// folder that holds a file at the image's path gives the file. Each folder is
    /// relative to the folder of the file in which the image is written, or absolute. The
    /// default is that folder alone, `.`.
    pub search_path: Vec<String>,
}

impl Default for GraphicsFilesConfig {
    fn default() -> GraphicsFilesConfig {
        GraphicsFilesConfig { search_path: vec![String::from(".")] }
    }
}

/// Returns the declaration of the feature's settings: the fields of
/// [`GraphicsFilesConfig`] are its keys.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE).config::<GraphicsFilesConfig>(
        "The folders in which image files are looked for, in order (`search_path`), each \
         relative to the folder of the file in which the image is written, or absolute. \
         Fixed when the registry is built: it cannot vary per document or per render.",
    )
}

/// The feature `graphics-files`, which contributes the host's services for image files to
/// the library's `graphics` feature: see the [module documentation](self).
///
/// [`stdmain_factory`](super::stdmain_factory) makes it from the settings and from the parts of the
/// command ([`StdParts`](super::StdParts)).
#[derive(Debug, Clone)]
pub struct GraphicsFilesFeature {
    config: GraphicsFilesConfig,
    files: ServiceFiles,
    output: OutputFolder,
    magick_program: OsString,
}

impl GraphicsFilesFeature {
    /// Returns the feature with the configuration `config`. Its services read files
    /// through `files`; the graphics resolver computes the address of an image file from
    /// the output's folder `output`; the converter `magick` runs `magick_program`.
    ///
    /// `magick_program` is a name with no folder, which the operating system looks for in
    /// its folders of programs when the program is started, or a path
    /// ([`MagickConverter::new`]). Like the output's folder, it depends on the machine the
    /// program runs on, and the program embedding the host gives it
    /// ([`StdParts::magick_program`](super::StdParts::magick_program)); it is never a
    /// setting, so that no document can choose a program that the host starts.
    pub fn new(
        config: GraphicsFilesConfig,
        files: ServiceFiles,
        output: OutputFolder,
        magick_program: impl Into<OsString>,
    ) -> GraphicsFilesFeature {
        GraphicsFilesFeature { config, files, output, magick_program: magick_program.into() }
    }
}

impl FlmFeature for GraphicsFilesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Image files").docs(
            "the host's services for image files: finding the file of an image in the folders \
             of a search path and reading it; reading the image's size from its content; \
             converting a raster image into a PNG image in the program itself; and converting \
             between any two formats by running ImageMagick's program `magick`",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let search_path = self.config.search_path.iter().map(PathBuf::from).collect();
        let resolver = FileResolver::new(self.files.clone(), search_path, self.output.clone());
        b.peer_slot::<dyn GraphicsResolver>(RESOLVER_SLOT, Arc::new(resolver))?;
        b.peer_slot::<dyn GraphicsInspector>(INSPECTOR_SLOT, Arc::new(FileInspector))?;
        b.peer_slot::<dyn GraphicsConverter>(CONVERTERS_SLOT, Arc::new(RasterConverter))?;
        let magick = MagickConverter::new(self.magick_program.clone());
        b.peer_slot::<dyn GraphicsConverter>(CONVERTERS_SLOT, Arc::new(magick))
    }
}
