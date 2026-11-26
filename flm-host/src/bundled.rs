//! The extensions that the program offers, compiled into it, found by name with nothing
//! installed: the tables that a program gives in its request ([`Bundled`],
//! [`Request::bundled`](crate::session::Request::bundled)), and the source that answers a
//! stem from them ([`BundledSource`]).
//!
//! A program describes the extensions it carries with two tables, whose types the extension
//! layer defines: the *table of files*, one row per extension, its folder (`data`, `js` or
//! `rust`), its stem and its files, the manifest among them
//! ([`BundledExtension`]); and, under the cargo feature `native`, the *table of code*, one
//! row per extension written in Rust and linked into the program, its stem and its code
//! (`flm_ext::guest::NamedExtension`). The crate `flm-extensions` gives
//! the tables of the extensions shipped with FLM. Both tables are empty by default: a
//! request offers no extension until its program gives the tables.
//!
//! [`BundledSource`] answers a locator that is the stem of a row of the table of files with
//! that extension, every row through one path: its identity `bundled:FOLDER/STEM`
//! (`bundled:js/math-mathjax`), its manifest (the row's file `flmextension.yaml`, or
//! `STEM.yaml` for an extension written as one file) parsed as the filesystem source parses
//! one ([`ParsedManifests::parse`], so that a problem with a value the manifest writes is
//! placed at its line, the resolver of its places given to the loader), and the row's files
//! ([`StaticFiles`]), as the compose templates of the package `std` are read. When the
//! table of code holds the stem, the manifest is given with `extension.engine` set to
//! `native` and `extension.wasm` (the module of the extension's WebAssembly form) removed:
//! the loader then gives the extension to the engine `native`, which the host creates over
//! the request's table of code ([`loader`](crate::packages::loader)), and the extension
//! reads the files of its row, a compose template among them.
//!
//! The source also answers the extensions of the collections that the program loaded
//! (cargo feature `wasm`; [`Request::collections`](crate::session::Request::collections),
//! `flm_ext::wasm::Collection`), each a module that holds several extensions whole, before
//! the rows of its own table of files: a row of a collection is answered by the same path,
//! with the identity `LABEL:FOLDER/STEM`, the label being the one the program gave the
//! collection, its manifest given as written, and its files held as shared bytes. The engine
//! `wasm`, which the host gives the collections, runs a row that has code on the
//! collection's module. The host also reads the manifest of every row of each collection
//! at each preparation, by the same rule, and gives it to the engine, which sums the memory
//! limits of the rows that have code for the limit of the collection's instances; a row
//! whose manifest does not read counts for nothing there.
//!
//! The host puts the source last on the search path ([`loader`](crate::packages::loader)),
//! so that a copy of the extension a user installs on the search path (for an extension
//! written in Rust, its WebAssembly form) wins over the bundled one, and the collections
//! win over the program's own tables, in the order given. A bundled file never changes: no
//! read of one is recorded among the files a build read.
//! [`Bundled::stems`] lists the extensions offered, which the JSON Schemas suggest for an
//! entry of `use` ([`crate::packages::suggestions`]).

use std::sync::Arc;

use flm_ext::Locator;
use flm_ext::source::{ExtensionSource, FoundExtension, SourceError};
#[cfg(feature = "native")]
use serde_json::{Map, Value};

use crate::extensions::{MANIFEST_FILE, ParsedManifests, StaticFiles};

/// The types of the rows of a table of files, which the extension layer defines.
pub use flm_ext::source::{BundledExtension, BundledFile};

/// The extensions that a program offers, compiled into it: its table of files and, under
/// the cargo feature `native`, its table of code. See the [module documentation](self).
///
/// The default value holds two empty tables, and offers no extension. A program makes the
/// value with [`Bundled::new`], from its table of files, and gives its table of code with
/// `Bundled::with_code` (cargo feature `native`). The type is `#[non_exhaustive]` because
/// the table of code exists only under a cargo feature: a program that makes the value with
/// these constructors compiles whichever features its build turns on.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct Bundled {
    /// The table of files: one row per extension offered, in the order in which
    /// [`stems`](Bundled::stems) lists them.
    pub files: &'static [BundledExtension],
    /// The table of code (cargo feature `native`): one row per extension of the table of
    /// files that is written in Rust and linked into the program, which the engine `native`
    /// runs.
    #[cfg(feature = "native")]
    pub code: &'static [flm_ext::guest::NamedExtension],
}

impl Bundled {
    /// Returns the tables of the extensions that `files` describes, with an empty table of
    /// code.
    pub const fn new(files: &'static [BundledExtension]) -> Bundled {
        Bundled {
            files,
            #[cfg(feature = "native")]
            code: &[],
        }
    }

    /// Returns these tables with `code` as the table of code (cargo feature `native`).
    #[cfg(feature = "native")]
    pub const fn with_code(self, code: &'static [flm_ext::guest::NamedExtension]) -> Bundled {
        Bundled { code, ..self }
    }

    /// Returns the stems of the extensions offered, each the locator that finds it, in the
    /// order of the table of files.
    pub fn stems(&self) -> Vec<&'static str> {
        self.files.iter().map(|extension| extension.stem).collect()
    }

    /// The row of the table of files whose stem is `stem`.
    fn find(&self, stem: &str) -> Option<&'static BundledExtension> {
        self.files.iter().find(|extension| extension.stem == stem)
    }

    /// Whether the table of code holds the stem `stem`.
    #[cfg(feature = "native")]
    fn has_code(&self, stem: &str) -> bool {
        self.code.iter().any(|extension| extension.stem == stem)
    }
}

/// The source of the extensions that the program offers. See the
/// [module documentation](self).
#[derive(Debug)]
pub struct BundledSource {
    /// Where the manifests parsed are kept, shared with the filesystem source.
    manifests: Arc<ParsedManifests>,
    /// The tables of the extensions offered.
    bundled: Bundled,
    /// The collections the program loaded (cargo feature `wasm`), in the order given.
    #[cfg(feature = "wasm")]
    collections: Vec<flm_ext::wasm::Collection>,
}

impl BundledSource {
    /// Returns the source of the extensions of `bundled`, keeping the manifests it parses in
    /// `manifests` (the filesystem source's store,
    /// [`ExtensionFinder::parsed_manifests`](crate::extensions::ExtensionFinder::parsed_manifests),
    /// through which the host places a problem with a manifest's value).
    pub fn new(manifests: Arc<ParsedManifests>, bundled: Bundled) -> BundledSource {
        BundledSource {
            manifests,
            bundled,
            #[cfg(feature = "wasm")]
            collections: Vec::new(),
        }
    }

    /// Returns this source with the extensions of `collections` (cargo feature `wasm`),
    /// answered in the order given, before the rows of the tables.
    #[cfg(feature = "wasm")]
    pub fn with_collections(self, collections: Vec<flm_ext::wasm::Collection>) -> BundledSource {
        BundledSource { collections, ..self }
    }

    /// The extension whose row has the identity `identity` and the stem `stem`, the files
    /// `files`, found for the locator `written`; its manifest the file `flmextension.yaml`
    /// of the row or, for an extension written as one file, `STEM.yaml`, which then holds no
    /// file; the manifest given to the engine `native` when `native`. `None` when the row
    /// holds neither file.
    ///
    /// # Errors
    ///
    /// A manifest that is not text encoded as UTF-8 or not valid YAML.
    fn answer(
        &self,
        written: &str,
        identity: String,
        stem: &str,
        files: StaticFiles,
        native: bool,
    ) -> Result<Option<FoundExtension>, SourceError> {
        let Some((path, in_folder)) = manifest_file(stem, |path| files.file(path).is_some()) else { return Ok(None) };
        let text = manifest_text(written, &identity, &path, files.file(&path).unwrap_or_default())?;
        let (manifest, place_of) = self.manifests.parse(written, stem, &identity, text)?;
        #[cfg(feature = "native")]
        let manifest = if native { for_the_engine_native(manifest) } else { manifest };
        #[cfg(not(feature = "native"))]
        debug_assert!(!native, "without the cargo feature `native`, no row is given to the engine `native`");
        // An extension written as one file has no files, as on the disk.
        let files = if in_folder { files } else { StaticFiles::new([], &identity, stem) };
        Ok(Some(FoundExtension { manifest, identity, files: Box::new(files), place_of: Some(place_of) }))
    }
}

impl ExtensionSource for BundledSource {
    /// The extension of the row of the table of files whose stem is the locator as written,
    /// given to the engine `native` when the table of code holds the stem; `Ok(None)` for any
    /// other locator (a nested one, `.`).
    ///
    /// # Errors
    ///
    /// A manifest that is not text encoded as UTF-8 or not valid YAML.
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError> {
        let Locator::Named(written) = locator else { return Ok(None) };
        #[cfg(feature = "wasm")]
        for collection in &self.collections {
            let Some(row) = collection.extensions().iter().find(|row| row.stem() == written) else { continue };
            let held = row.files().iter().map(|file| (file.path().to_string(), Arc::clone(file.bytes())));
            let files = StaticFiles::shared(held, row.identity(), row.stem());
            return self.answer(written, row.identity().to_string(), row.stem(), files, false);
        }
        let Some(extension) = self.bundled.find(written) else { return Ok(None) };
        let identity = format!("bundled:{}/{}", extension.folder, extension.stem);
        let files = StaticFiles::new(extension.files.iter().map(|file| (file.path, file.bytes)), &identity, extension.stem);
        #[cfg(feature = "native")]
        let native = self.bundled.has_code(extension.stem);
        #[cfg(not(feature = "native"))]
        let native = false;
        self.answer(written, identity, extension.stem, files, native)
    }
}

/// The manifest file of an extension of stem `stem`, `holds` telling which files it holds:
/// `flmextension.yaml` when it holds one (`true`: the extension is a folder), else
/// `STEM.yaml` (`false`: the extension is written as one file); `None` when it holds
/// neither.
fn manifest_file(stem: &str, holds: impl Fn(&str) -> bool) -> Option<(String, bool)> {
    let one_file = format!("{stem}.yaml");
    if holds(MANIFEST_FILE) {
        Some((MANIFEST_FILE.to_string(), true))
    } else if holds(&one_file) {
        Some((one_file, false))
    } else {
        None
    }
}

/// The manifest of the row `row` of a collection, as [`BundledSource`] reads it when a
/// document uses the row, given to the engine `wasm` for the memory limit of the
/// collection's instances ([`loader`](crate::packages::loader)); `None` when the row holds
/// no manifest, or its manifest is not text encoded as UTF-8, not valid YAML or not a valid
/// manifest. The manifest is not kept among the manifests a preparation parsed: a row that
/// no entry of `use` names is not found for any locator.
#[cfg(feature = "wasm")]
pub(crate) fn collection_manifest(row: &flm_ext::wasm::CollectionExtension) -> Option<flm_ext::Manifest> {
    let file = |path: &str| row.files().iter().find(|file| file.path() == path);
    let (path, _) = manifest_file(row.stem(), |path| file(path).is_some())?;
    let text = std::str::from_utf8(file(&path)?.bytes()).ok()?;
    let value = crate::seed::read_front_matter(text, 1).ok()?.to_json();
    flm_ext::Manifest::read(row.stem(), &value).ok()
}

/// Returns `manifest`, the manifest of an extension's WebAssembly form, with its engine set
/// to `native` and its module (`extension.wasm`) removed. A block `extension` that is not a
/// map is left as written, for the loader to report.
#[cfg(feature = "native")]
fn for_the_engine_native(mut manifest: Value) -> Value {
    let block = manifest.as_object_mut().map(|root| root.entry("extension").or_insert_with(|| Value::Object(Map::new())));
    if let Some(Value::Object(block)) = block {
        block.insert("engine".to_string(), Value::from(flm_ext::native::NATIVE));
        block.remove("wasm");
    }
    manifest
}

/// The text of the manifest `bytes`, the file `path` of the bundled extension `identity`,
/// found for the locator `written`.
///
/// # Errors
///
/// The file is not text encoded as UTF-8.
fn manifest_text<'b>(written: &str, identity: &str, path: &str, bytes: &'b [u8]) -> Result<&'b str, SourceError> {
    std::str::from_utf8(bytes).map_err(|_| SourceError {
        locator: written.to_string(),
        path: Some(identity.to_string()),
        place: None,
        detail: format!("the file `{identity}/{path}` must be text encoded as UTF-8"),
    })
}
