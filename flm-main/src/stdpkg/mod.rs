//! The host's own packages, `stdmain` and `std`: the features that the `flm` program adds
//! on top of the library's standard package, and the program's defaults for the library's
//! features.
//!
//! A package is a named bundle of features together with the packages it builds on
//! ([`PackageEntry`]); a feature
//! ([`FlmFeature`](flm_core::core::registry::FlmFeature)) is a Rust value that contributes a
//! related bundle of things to a registry (elements, stages, tables, lowering rules,
//! backends, services, settings). The library's standard package, `stdcore`
//! ([`flm_core::stdcore`]), defines the standard commands and environments of FLM and the
//! four output formats `text`, `html`, `latex-rendered` and `latex-recomposed`. The host
//! adds two packages:
//!
//! - `stdmain` ([`STDMAIN_PACKAGE`], [`stdmain_factory`]) holds the features of this
//!   module: what depends on the program around the library, namely assembling an output
//!   through a compose template, writing the output's files, the page of the live preview,
//!   and, under cargo features, reading image files and fetching citation entries. It uses
//!   `stdcore`: writing an output needs the library's outputs.
//! - `std` ([`PACKAGE`], [`factory`]) has no features of its own. It uses `stdcore` and
//!   `stdmain`, so that a document that uses `std` has both.
//!
//! A document uses the packages of the *initial list* before those its front matter lists
//! under `use`; a request's initial list is `std` unless the host gives another
//! ([`Request::default_initial_use`](crate::session::Request::default_initial_use);
//! [`crate::packages`]). Neither package processes mathematics: the package that does, the
//! math processor, is chosen apart from the initial list, by the host
//! ([`Request::default_math`](crate::session::Request::default_math), for example the
//! library's `math-passthrough`) or by the document's front matter (`math`).
//!
//! The host builds `stdmain` for one command with [`stdmain_factory`], from the parts that
//! the command gives ([`StdParts`]): how the artifacts of an output are named, where the
//! output's files go, how the package's services read files, the folder the output is
//! written in, and, for two of the features, values that depend on the machine the program
//! runs on: the program that the converter `magick` runs, and what the citations provider
//! needs (its access to the network, the folder of its cache file, the listener told of a
//! fetch's progress).
//! [`crate::session::prepare`] does this for every command of the `flm` program that
//! processes a document.
//!
//! # The features of `stdmain`
//!
//! | Feature | What it registers | Module |
//! |---|---|---|
//! | `compose` | the library's feature ([`flm_core::stdcore::features::compose`], which the library's package `stdcore` does not register): the service slots of the template engines and of the compose templates, the template engine `content-only`, and one stage per output made of output regions, which assembles the output's entry artifact through the chosen compose template | [`flm_core::stdcore::features::compose`] |
//! | `compose-standard` | the template engine `minijinja` and `stdmain`'s own compose templates, contributed to the service slots of `compose` | [`compose_standard`] |
//! | `write` | the stage `flm.write.files`, which hands the files of every output the plan demands to the sink (the destination the host gives: the disk, memory, the standard output), and the host's naming scheme for the output's files | [`write`](mod@write) |
//! | `preview` | the script and the stylesheet of the live preview's page | [`preview`](mod@preview) |
//! | `graphics-files` (cargo feature `graphics`) | the host's services for image files, contributed to the library's `graphics` feature | [`graphics_files`] |
//! | `citations-standard` (cargo feature `citations`) | the host's provider of citation entries, contributed to the library's `citations` feature, and the two exports `bibtex` and `csl-json` | [`citations_standard`] |
//!
//! Each of these features can be switched off in the settings, as any feature can: a
//! package of an extension ([`crate::packages`]) or the front matter writes `false` for it
//! (`flm: {features: {preview: false}}`; see [`crate::settings`]), and the factory then
//! leaves it out of the package. A command that writes an output refuses to start when
//! `write` is switched off, since no stage would then write the output.
//!
//! # The program's defaults
//!
//! Where the defaults of the `flm` program differ from the library's, [`defaults`] gives
//! the program's values, so that `flm` writes what the earlier Python implementation of
//! the program wrote. The host enters them as the lowest-ranked source of settings: above
//! each feature's own defaults, and below every value that a package of an extension,
//! the front matter, the workflow or the command line writes ([`crate::settings`]). A
//! program that embeds the library without this host keeps the library's defaults. The
//! program's defaults are:
//!
//! - footnotes are marked a, b, c, and a reference to a footnote writes its letter (the
//!   library: 1, 2, 3); the heading above the list of footnotes, `Footnotes`, is at level
//!   1 (the library: level 2);
//! - a figure is captioned `Fig.` and numbered I, II, III (the library: `Figure`,
//!   numbered 1, 2, 3);
//! - a table is captioned `Tab.` and numbered I, II, III (the library: `Table`, numbered
//!   1, 2, 3);
//! - the heading above the list of cited works, `References`, is at level 1, as the
//!   heading of the footnotes is (the library: level 2;
//!   [`CitationsConfig::heading_level`](flm_core::stdcore::features::citations::CitationsConfig::heading_level));
//! - under the cargo feature `graphics`, which provides the services that read image
//!   files: the `html` output writes the content of each image into the page itself (the
//!   strategy `data-uri`; the library: `reference`) and converts a PDF drawing into a PNG
//!   image at 192 dots per inch; the `latex-recomposed` and `latex-rendered` outputs,
//!   under the strategy `copy`, convert an SVG drawing into a PDF file and a GIF image
//!   into a PNG image (the library: no conversion rule). These two settings of the `graphics` feature,
//!   `strategy` and `convert`, are maps by output, combined entry by entry, so a source
//!   that writes the entry of one output keeps the program's entries for the others.
//!
//! The xnote kinds and the float types are collections of named things, which are
//! combined entry by entry. An xnote is a piece of text set apart from the running text,
//! marked where it belongs and shown elsewhere or in place (a footnote, an endnote, a side
//! note); the configuration of the library's `xnotes` feature declares each kind of xnote
//! ([`flm_core::stdcore::features::xnotes`]). In such a collection, an entry written
//! replaces the library's entry of the same name whole ([`crate::settings`]). So the
//! program writes each changed entry in full: the library's own entry
//! ([`XnoteKind::footnote`], [`FloatType::figure`], [`FloatType::table`]) with the fields
//! above changed, converted to data by the library's function for collections of named
//! things ([`named::to_data`]). The library's other entries stay as they are. The heading
//! level of the list of cited works is a number, written as it is.
//!
//! # Map of the items
//!
//! - [`STDMAIN_PACKAGE`], [`stdmain_factory`], [`StdParts`]: the package `stdmain` and how
//!   the host builds it; [`PACKAGE`], [`factory`]: the package `std`.
//! - [`defaults`]: the program's defaults.
//! - [`compose_standard`], [`write`](mod@write), [`preview`](mod@preview),
//!   [`graphics_files`], [`citations_standard`]: the features of this crate, one module
//!   each; [`WriteFeature`] and [`PreviewFeature`] are re-exported here. The feature
//!   `compose` is the library's ([`ComposeFeature`]).

#[cfg(feature = "citations")]
pub mod citations_standard;
pub mod compose_standard;
#[cfg(feature = "graphics")]
pub mod graphics_files;
pub mod preview;
pub mod write;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactNamer;
use flm_core::core::registry::{PackageEntry, PackageError, PackageFactory};
use flm_core::core::settings::PackageSettings;

use flm_core::core::settings::named;
use flm_core::stdcore::features::floats::FloatType;
use flm_core::stdcore::features::compose::{self, ComposeFeature};
use flm_core::stdcore::features::numbering::NumberTemplate;
use flm_core::stdcore::features::xnotes::XnoteKind;
use serde_json::Value;

use crate::files::{MemoryFiles, ServiceFiles};
use crate::output::{OutputFolder, Sink};

pub use preview::PreviewFeature;
pub use write::WriteFeature;

/// The name of the host's package `std`, which has no features of its own and uses the
/// packages `stdcore` and `stdmain` ([`factory`]).
pub const PACKAGE: &str = "std";

/// The name of the host's package `stdmain`, which holds the features of this module and
/// uses the package `stdcore` ([`stdmain_factory`]).
pub const STDMAIN_PACKAGE: &str = "stdmain";

/// Returns the program's defaults (see the [module
/// documentation](self#the-programs-defaults)), each as a feature's name, one of the
/// feature's settings keys, and the value written as data.
///
/// The host enters each as a setting of the lowest-ranked source, the program's defaults
/// ([`Source::Defaults`](crate::settings::Source::Defaults)).
///
/// # Errors
///
/// Returns the text of [`named::to_data`] when it refuses one of the library's own
/// entries. That would be a fault in the library's code; the tests of this crate check
/// that it does not happen.
pub fn defaults() -> Result<Vec<(&'static str, &'static str, Value)>, String> {
    let roman = || NumberTemplate::formats("#{Roman}");
    let footnote = XnoteKind::footnote()
        .number_format(NumberTemplate::formats("#{alph}"))
        .mark(NumberTemplate::formats("#{alph}"))
        .title_level(1);
    let figure = FloatType::figure().caption_name("Fig.").number_format(roman());
    let table = FloatType::table().caption_name("Tab.").number_format(roman());
    #[allow(unused_mut, reason = "the rows of image files are added under the cargo feature `graphics` alone")]
    let mut rows = vec![
        ("xnotes", "kinds", named::to_data(&[footnote], "name")?),
        ("floats", "types", named::to_data(&[figure, table], "name")?),
        ("citations", "heading_level", Value::from(1)),
    ];
    // See decision rationale at [§flm-dd-dr:host-graphics-defaults].
    #[cfg(feature = "graphics")]
    rows.extend(graphics_defaults());
    Ok(rows)
}

/// Returns the program's defaults for the library's `graphics` feature, which take effect
/// with the host's services for image files: the `html` output writes the content of each
/// image into the page and converts a PDF drawing into a PNG image at 192 dots per inch;
/// the `latex-recomposed` and `latex-rendered` outputs, under the strategy `copy`, convert
/// an SVG drawing into a PDF file and a GIF image into a PNG image. Both keys, `strategy`
/// and `convert`, are maps by output, combined entry by entry.
#[cfg(feature = "graphics")]
fn graphics_defaults() -> [(&'static str, &'static str, Value); 2] {
    use flm_core::stdcore::backends::{html, latex_recomposed, latex_rendered};
    use flm_core::stdcore::features::graphics::{FEATURE, Strategy};
    [
        (FEATURE, "strategy", serde_json::json!({ html::BACKEND: Strategy::DataUri.name() })),
        (
            FEATURE,
            "convert",
            serde_json::json!({
                html::BACKEND: [{ "from": "pdf", "to": "png", "options": { "dpi": 192 } }],
                latex_recomposed::BACKEND: [{ "from": "svg", "to": "pdf" }, { "from": "gif", "to": "png" }],
                latex_rendered::BACKEND: [{ "from": "svg", "to": "pdf" }, { "from": "gif", "to": "png" }],
            }),
        ),
    ]
}

/// What the host gives the package `stdmain` for one command: how the artifacts of an output
/// are named, where the output's files go, how the services read files, the folder the
/// output is written in, and what the features `graphics-files` and `citations-standard`
/// need of the machine the program runs on (a program to run, the network, a folder).
///
/// Build one with [`StdParts::new`] and set the fields that the command needs;
/// [`stdmain_factory`] builds the package from it.
#[derive(Clone)]
pub struct StdParts {
    /// The naming scheme of the output's files ([`ArtifactNamer`]), which the feature
    /// `write` contributes to the library's service slot `flm.artifacts.namer`. Every
    /// command gives one, whether or not it writes an output: a stage of the library may
    /// ask for the path of an artifact either way (the `graphics` feature does, for an image
    /// it copies beside the output).
    pub namer: Arc<dyn ArtifactNamer>,
    /// Where the files of an output go ([`Sink`]), for a command that writes an output;
    /// `None` for a command that writes nothing (`flm check`).
    pub sink: Option<Arc<dyn Sink>>,
    /// How the services of the package's features read files ([`ServiceFiles`]): every
    /// file they read is listed among the files that the build under way read.
    pub files: ServiceFiles,
    /// The folder the output is written in. The host's graphics resolver computes from it
    /// the address at which an output reaches an image file in its own folder (the
    /// strategy `reference`).
    pub output_folder: OutputFolder,
    /// The program that the converter `magick` of the feature `graphics-files` runs
    /// ([`graphics_files::GraphicsFilesFeature::new`]): a name that the operating system
    /// looks for in its folders of programs, or a path, given to the operating system as
    /// written. It depends on the machine the program runs on, so the program embedding
    /// the host gives it, as it gives the output's folder. It is never a setting, so that
    /// no document can choose a program that the host starts.
    // A converter added later that runs a program takes its program the same way, from a
    // field of its own here.
    // See decision rationale at [§flm-dd-dr:host-magick-program]
    #[cfg(feature = "graphics")]
    pub magick_program: std::ffi::OsString,
    /// What the feature `citations-standard` is given of the machine (cargo feature
    /// `citations`): the access to the network, the folder of the cache file, and the
    /// listener told of the progress of a fetch
    /// ([`CitationsHost`](crate::citations::CitationsHost)). Like the program that the
    /// converter `magick` runs, these depend on the machine the program runs on: the
    /// program embedding the host gives them, and they are never settings.
    #[cfg(feature = "citations")]
    pub citations: crate::citations::CitationsHost,
}

impl StdParts {
    /// Returns the parts of a command whose naming scheme is `namer` and which writes
    /// nothing: no sink, services that read from an empty set of files held in memory
    /// ([`MemoryFiles`]), the current folder as the output's folder, the program
    /// [`MagickConverter::DEFAULT_PROGRAM`](crate::graphics::MagickConverter::DEFAULT_PROGRAM)
    /// for the converter `magick` (cargo feature `graphics`), and a citations host with no
    /// network, no cache file and no listener (cargo feature `citations`).
    ///
    /// `flm schema` and a test that runs no build use these parts as they are; a command
    /// that builds a document sets the fields it needs.
    pub fn new(namer: Arc<dyn ArtifactNamer>) -> StdParts {
        StdParts {
            namer,
            sink: None,
            files: ServiceFiles::new(Arc::new(MemoryFiles::new())),
            output_folder: OutputFolder::default(),
            #[cfg(feature = "graphics")]
            magick_program: crate::graphics::MagickConverter::DEFAULT_PROGRAM.into(),
            #[cfg(feature = "citations")]
            citations: crate::citations::CitationsHost::default(),
        }
    }
}

/// Returns the factory of the package `std`, which has no features of its own and uses the
/// library's standard package `stdcore` ([`flm_core::stdcore`]) and the host's package
/// `stdmain` ([`stdmain_factory`]), in this order.
///
/// # Errors
///
/// Returns the [`PackageError`] of [`PackageFactory::new`], which does not fail for a
/// package with no features.
pub fn factory() -> Result<PackageFactory, PackageError> {
    let factory = PackageFactory::new(PACKAGE, |_: &PackageSettings| Ok(PackageEntry::new(PACKAGE)))?;
    Ok(factory.uses(flm_core::stdcore::PACKAGE).uses(STDMAIN_PACKAGE))
}

/// Returns the factory of the package `stdmain`, which uses the library's standard package
/// `stdcore` ([`flm_core::stdcore`]).
///
/// From the settings of a document, the factory builds a package entry holding the
/// features `compose` (the library's, [`ComposeFeature`]), `compose-standard` (with
/// `stdmain`'s own compose templates), `write` and `preview`; under
/// the cargo feature `graphics`, the feature `graphics-files`; and under the cargo
/// feature `citations`, the feature `citations-standard`. A feature that the settings
/// switch off is left out. The features `write`, `graphics-files` and
/// `citations-standard` are made from `parts`; `graphics-files` and
/// `citations-standard` also take their configuration from the settings, or the default
/// configuration where the settings write none.
///
/// # Errors
///
/// Returns a [`PackageError`] when `stdmain`'s own compose templates cannot be read
/// ([`compose_standard::templates::std_templates`]), or when [`PackageFactory::new`] fails because
/// the build of the package fails for empty settings. Neither happens with the files
/// this crate ships, as its tests check.
pub fn stdmain_factory(parts: StdParts) -> Result<PackageFactory, PackageError> {
    let templates = compose_standard::templates::std_templates().map_err(PackageError::new)?;
    let factory = PackageFactory::new(STDMAIN_PACKAGE, move |settings: &PackageSettings| {
        let mut entry = PackageEntry::new(STDMAIN_PACKAGE);
        if !settings.is_off(compose::FEATURE) {
            entry = entry.feature(Arc::new(ComposeFeature::new()));
        }
        if !settings.is_off(compose_standard::FEATURE) {
            entry = entry.feature(Arc::new(compose_standard::ComposeStandardFeature::new(templates.clone())));
        }
        if !settings.is_off(write::FEATURE) {
            let feature = WriteFeature::new(Arc::clone(&parts.namer), parts.sink.clone());
            entry = entry.feature(Arc::new(feature));
        }
        if !settings.is_off(preview::FEATURE) {
            entry = entry.feature(Arc::new(PreviewFeature));
        }
        #[cfg(feature = "graphics")]
        if !settings.is_off(graphics_files::FEATURE) {
            let config = settings.config::<graphics_files::GraphicsFilesConfig>(graphics_files::FEATURE);
            let feature = graphics_files::GraphicsFilesFeature::new(
                config.cloned().unwrap_or_default(),
                parts.files.clone(),
                parts.output_folder.clone(),
                parts.magick_program.clone(),
            );
            entry = entry.feature(Arc::new(feature));
        }
        #[cfg(feature = "citations")]
        if !settings.is_off(citations_standard::FEATURE) {
            let config = settings.config::<crate::citations::ProviderConfig>(citations_standard::FEATURE);
            let feature = citations_standard::CitationsStandardFeature::new(
                config.cloned().unwrap_or_default(),
                parts.files.clone(),
                parts.citations.clone(),
            );
            entry = entry.feature(Arc::new(feature));
        }
        Ok(entry)
    })?;
    Ok(factory.uses(flm_core::stdcore::PACKAGE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Origin, Settings, Source};
    use flm_core::stdcore::features::citations::CitationsConfig;
    use flm_core::stdcore::features::floats::FloatsConfig;
    use flm_core::stdcore::features::xnotes::XnotesConfig;

    #[test]
    fn std_has_no_features_and_uses_stdcore_then_stdmain() {
        let std = factory().unwrap();
        assert_eq!(std.name(), PACKAGE);
        assert!(std.features().is_empty());
        let used: Vec<&str> = std.used().iter().map(|name| name.as_str()).collect();
        assert_eq!(used, [flm_core::stdcore::PACKAGE, STDMAIN_PACKAGE]);
        let stdmain = stdmain_factory(StdParts::new(Arc::new(crate::output::Naming::Folder))).unwrap();
        let used: Vec<&str> = stdmain.used().iter().map(|name| name.as_str()).collect();
        assert_eq!(used, [flm_core::stdcore::PACKAGE]);
        let features: Vec<String> = stdmain.features().iter().map(|feature| feature.name.to_string()).collect();
        #[allow(unused_mut, reason = "the features of the cargo features `graphics` and `citations` are added under them")]
        let mut expected = vec![compose::FEATURE, compose_standard::FEATURE, write::FEATURE, preview::FEATURE];
        #[cfg(feature = "graphics")]
        expected.push(graphics_files::FEATURE);
        #[cfg(feature = "citations")]
        expected.push(citations_standard::FEATURE);
        assert_eq!(features, expected);
    }

    #[test]
    fn each_default_is_the_library_s_value_with_the_program_s_differences() {
        let mut settings = Settings::new();
        for (feature, key, value) in defaults().unwrap() {
            settings.set(feature, key, value, Origin { source: Source::Defaults, place: None });
        }
        let factory = flm_core::stdcore::factory().unwrap();
        let read = settings.read(&[&factory]).unwrap();
        let package = &read.packages[factory.name()];
        let footnote = XnoteKind::footnote();
        let expected = XnoteKind {
            number_format: NumberTemplate::formats("#{alph}"),
            mark: NumberTemplate::formats("#{alph}"),
            title_level: 1,
            ..footnote
        };
        assert_eq!(package.config::<XnotesConfig>("xnotes").unwrap().kinds, [expected]);
        let roman = NumberTemplate::formats("#{Roman}");
        let figure = FloatType { caption_name: "Fig.".into(), number_format: roman.clone(), ..FloatType::figure() };
        let table = FloatType { caption_name: "Tab.".into(), number_format: roman, ..FloatType::table() };
        assert_eq!(package.config::<FloatsConfig>("floats").unwrap().types, [figure, table]);
        let citations = CitationsConfig { heading_level: 1, ..CitationsConfig::new() };
        assert_eq!(*package.config::<CitationsConfig>("citations").unwrap(), citations);
    }
}
