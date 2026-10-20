//! The bundled source of extensions: the extensions shipped with FLM, compiled into the
//! program (the crate `flm-extensions`), found by name with nothing installed.
//!
//! [`BundledSource`] answers a locator that is the stem of a bundled extension with that
//! extension. The crate `flm-extensions` holds two kinds:
//!
//! - an extension of the engine `data` or `js` (`lecturenotes`, `math-mathjax`), held as its
//!   files: the source answers its manifest parsed as the filesystem source parses one
//!   ([`ParsedManifests::parse`], so that a problem with a value the manifest writes is
//!   placed at its line, the resolver of its places given to the loader), its identity
//!   `bundled:ENGINE/STEM` (`bundled:js/math-mathjax`), and its files read from the table
//!   ([`StaticFiles`]), as the compose templates of the package `stdmain` are;
//! - an extension written in Rust and compiled into the program (`profile-pdf-latex`),
//!   held as its code and the text of its manifest: the source answers the manifest parsed
//!   in the same way, with `extension.engine` set to `native` and `extension.wasm` (the
//!   module of its WebAssembly form) removed, its identity `bundled:rust/STEM`
//!   (`bundled:rust/profile-pdf-latex`), and no files. The loader then gives the extension
//!   to the engine `native`, which the host creates over the same table
//!   ([`loader`](crate::packages::loader)).
//!
//! The host puts the source last on the search path ([`loader`](crate::packages::loader)),
//! so that a copy of the extension a user installs on the search path (for an extension
//! written in Rust, its WebAssembly form) wins over the bundled one. A bundled file never
//! changes: no read of one is recorded among the files a build read. [`stems`] lists the
//! bundled extensions, which the JSON Schemas suggest for an entry of `use`
//! ([`crate::packages::suggestions`]).
//!
//! The module is compiled under the cargo feature `bundled-extensions`.

use std::sync::Arc;

use flm_ext::Locator;
use flm_ext::native::{NATIVE, NativeExtension};
use flm_ext::source::{ExtensionSource, FoundExtension, SourceError};
use flm_extensions::BundledExtension;
use serde_json::{Map, Value};

use crate::extensions::{MANIFEST_FILE, NoFiles, ParsedManifests, StaticFiles};

/// Returns the stems of the bundled extensions, each the locator that finds it: those held
/// as files, in the order of their table (`flm_extensions::EXTENSIONS`), then those written
/// in Rust, in the order of theirs (`flm_extensions::NATIVE_EXTENSIONS`).
pub fn stems() -> Vec<&'static str> {
    flm_extensions::EXTENSIONS
        .iter()
        .map(|extension| extension.stem)
        .chain(flm_extensions::NATIVE_EXTENSIONS.iter().map(|extension| extension.stem))
        .collect()
}

/// The source of the extensions compiled into the program. See the
/// [module documentation](self).
#[derive(Debug)]
pub struct BundledSource {
    /// Where the manifests parsed are kept, shared with the filesystem source.
    manifests: Arc<ParsedManifests>,
}

impl BundledSource {
    /// Returns the source of the bundled extensions, keeping the manifests it parses in
    /// `manifests` (the filesystem source's store,
    /// [`ExtensionFinder::parsed_manifests`](crate::extensions::ExtensionFinder::parsed_manifests),
    /// through which the host places a problem with a manifest's value).
    pub fn new(manifests: Arc<ParsedManifests>) -> BundledSource {
        BundledSource { manifests }
    }
}

impl ExtensionSource for BundledSource {
    /// The bundled extension whose stem is the locator as written, looked for among the
    /// extensions held as files, then among those written in Rust; `Ok(None)` for any other
    /// locator (a nested one, `.`).
    ///
    /// # Errors
    ///
    /// A manifest that is not text encoded as UTF-8 or not valid YAML.
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError> {
        let Locator::Named(written) = locator else { return Ok(None) };
        if let Some(extension) = flm_extensions::find(written) {
            return self.held_as_files(written, extension);
        }
        match flm_extensions::find_native(written) {
            Some(extension) => self.compiled_in(written, extension).map(Some),
            None => Ok(None),
        }
    }
}

impl BundledSource {
    /// The extension `extension`, held as files, found for the locator `written`.
    fn held_as_files(
        &self,
        written: &str,
        extension: &'static BundledExtension,
    ) -> Result<Option<FoundExtension>, SourceError> {
        let identity = format!("bundled:{}/{}", extension.engine_folder, extension.stem);
        let (manifest, in_folder) = match extension.file(MANIFEST_FILE) {
            Some(file) => (file, true),
            None => match extension.file(&format!("{}.yaml", extension.stem)) {
                Some(file) => (file, false),
                None => return Ok(None),
            },
        };
        let text = std::str::from_utf8(manifest.bytes).map_err(|_| SourceError {
            locator: written.to_string(),
            path: Some(identity.clone()),
            place: None,
            detail: format!("the file `{identity}/{}` must be text encoded as UTF-8", manifest.path),
        })?;
        let (manifest, place_of) = self.manifests.parse(written, extension.stem, &identity, text)?;
        // An extension written as one file has no files, as on the disk.
        let held = if in_folder { extension.files } else { &[] };
        let files = StaticFiles::new(held.iter().map(|file| (file.path, file.bytes)), &identity, extension.stem);
        Ok(Some(FoundExtension { manifest, identity, files: Box::new(files), place_of: Some(place_of) }))
    }

    /// The extension `extension`, written in Rust and compiled into the program, found for
    /// the locator `written`: its manifest, the one of its WebAssembly form, given to the
    /// engine `native` in place of the engine `wasm`, without the module.
    fn compiled_in(&self, written: &str, extension: &'static NativeExtension) -> Result<FoundExtension, SourceError> {
        // See decision rationale at [§flm-dd-dr:ext-bundled]
        let identity = format!("bundled:rust/{}", extension.stem);
        let (mut manifest, place_of) = self.manifests.parse(written, extension.stem, &identity, extension.manifest)?;
        // A block that is not a map is left as written, for the loader to report.
        let block = manifest.as_object_mut().map(|root| root.entry("extension").or_insert_with(|| Value::Object(Map::new())));
        if let Some(Value::Object(block)) = block {
            block.insert("engine".to_string(), Value::from(NATIVE));
            block.remove("wasm");
        }
        Ok(FoundExtension { manifest, identity, files: Box::new(NoFiles), place_of: Some(place_of) })
    }
}
