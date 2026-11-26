//! A collection loaded by a host program ([`Collection`]): one WebAssembly
//! module that holds several extensions whole, compiled once, and the files
//! of each, read from its custom section `flmextensions`
//! ([`crate::collection`]).

use std::fmt;
use std::sync::Arc;

use wasmi::Module;

use super::engine::{check_abi, one_line, AbiProblem};
use super::instance::WasmModule;
use crate::collection::{read_section, SectionError, SectionErrorKind, SECTION, VERSION};

/// A collection that a host program loaded: the module, compiled once and
/// checked against the guest's ABI, and the extensions it holds, each with
/// its files. The program makes one with [`Collection::read`] and offers its
/// extensions: its source answers a name from the rows
/// ([`Collection::extensions`]), and the engine `wasm` runs the rows that
/// have code on the collection's module
/// ([`WasmEngine::with_collection`](super::WasmEngine::with_collection)).
///
/// The identity of a row is `LABEL:FOLDER/STEM` (`shipped:rust/phfpack`), the
/// label being the one the program gave. Every extension of a collection
/// runs, on each thread and for each host, in one instance of the module
/// ([the module page](super#collections)).
///
/// Cheap to clone (an `Arc` inside); every clone is the same collection, and
/// shares its instances.
#[derive(Clone)]
pub struct Collection(Arc<CollectionInner>);

/// What a [`Collection`] holds.
struct CollectionInner {
    /// The label the program gave.
    label: String,
    /// The bytes of the collection's file, which enter the digest.
    bytes: Arc<[u8]>,
    /// The module, compiled once, from which its instances are created.
    module: Arc<WasmModule>,
    /// The extensions it holds, ordered by folder, then stem.
    extensions: Vec<CollectionExtension>,
}

/// One extension of a [`Collection`]: the folder that held it among the
/// folders of extensions (`data`, `js`, `rust`), its stem, its identity, and
/// its files, the manifest among them.
#[derive(Debug, Clone)]
pub struct CollectionExtension {
    /// The folder: `data`, `js` or `rust`.
    folder: String,
    /// The stem.
    stem: String,
    /// The identity, `LABEL:FOLDER/STEM`.
    identity: String,
    /// The files, in path order.
    files: Vec<CollectionFile>,
}

impl CollectionExtension {
    /// The folder that held the extension: `data`, `js` or `rust`.
    pub fn folder(&self) -> &str {
        &self.folder
    }

    /// The extension's stem: the name it is found by.
    pub fn stem(&self) -> &str {
        &self.stem
    }

    /// The extension's identity, `LABEL:FOLDER/STEM`.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The extension's files, the manifest among them, in path order.
    pub fn files(&self) -> &[CollectionFile] {
        &self.files
    }
}

/// One file of an extension of a [`Collection`]: its path relative to the
/// extension's folder, with `/` between the names, and its bytes, shared.
#[derive(Debug, Clone)]
pub struct CollectionFile {
    /// The path.
    path: String,
    /// The bytes.
    bytes: Arc<[u8]>,
}

impl CollectionFile {
    /// The file's path relative to the extension's folder.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The file's bytes, as they were in the extension's folder.
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}

impl Collection {
    /// Reads the collection whose file holds `bytes`, under the label `label`:
    /// compiles the module, checks its imports and exports against the
    /// guest's ABI, and reads its section `flmextensions`. No entry of the
    /// module is called.
    ///
    /// # Errors
    ///
    /// [`CollectionError`]: the bytes are not a WebAssembly module, the
    /// module does not follow the guest's ABI, it holds no section
    /// `flmextensions`, or the reader refuses the section
    /// ([`read_section`]).
    pub fn read(label: &str, bytes: Vec<u8>) -> Result<Collection, CollectionError> {
        let error = |kind| CollectionError { label: label.to_string(), kind };
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = wasmi::Engine::new(&config);
        let module =
            Module::new(&engine, &bytes).map_err(|wasmi| error(CollectionErrorKind::Module(one_line(&wasmi.to_string()))))?;
        let problems = check_abi(&module);
        if !problems.is_empty() {
            return Err(error(CollectionErrorKind::Abi(problems)));
        }
        let mut section = Vec::new();
        let mut found = false;
        for custom in module.custom_sections().filter(|custom| custom.name() == SECTION) {
            section.extend_from_slice(custom.data());
            found = true;
        }
        if !found {
            return Err(error(CollectionErrorKind::NoSection));
        }
        let read = read_section(&section).map_err(|refused| error(CollectionErrorKind::Section(refused)))?;
        let extensions = read
            .into_iter()
            .map(|extension| CollectionExtension {
                folder: extension.folder.to_string(),
                stem: extension.stem.to_string(),
                identity: format!("{label}:{}/{}", extension.folder, extension.stem),
                files: extension
                    .files
                    .into_iter()
                    .map(|file| CollectionFile { path: file.path.to_string(), bytes: Arc::from(file.bytes) })
                    .collect(),
            })
            .collect();
        Ok(Collection(Arc::new(CollectionInner {
            label: label.to_string(),
            bytes: Arc::from(bytes),
            module: Arc::new(WasmModule::new(engine, module)),
            extensions,
        })))
    }

    /// The label the program gave.
    pub fn label(&self) -> &str {
        &self.0.label
    }

    /// The extensions the collection holds, ordered by folder, then stem.
    pub fn extensions(&self) -> &[CollectionExtension] {
        &self.0.extensions
    }

    /// Whether the collection holds the extension of identity `identity`.
    pub(crate) fn holds(&self, identity: &str) -> bool {
        self.0.extensions.iter().any(|extension| extension.identity == identity)
    }

    /// The bytes of the collection's file.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0.bytes
    }

    /// The module the instances of the collection are created from.
    pub(crate) fn module(&self) -> &Arc<WasmModule> {
        &self.0.module
    }
}

impl fmt::Debug for Collection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stems: Vec<&str> = self.0.extensions.iter().map(|extension| extension.identity.as_str()).collect();
        f.debug_struct("Collection").field("label", &self.0.label).field("extensions", &stems).finish_non_exhaustive()
    }
}

/// Why [`Collection::read`] refused a file: the label it was given, and what
/// is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionError {
    /// The label the program gave.
    pub label: String,
    /// What is wrong.
    pub kind: CollectionErrorKind,
}

/// What is wrong with the file of a collection ([`CollectionError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CollectionErrorKind {
    /// The bytes are not a valid WebAssembly module: wasmi's reason.
    Module(String),
    /// The module does not follow the guest's ABI: each problem.
    Abi(Vec<AbiProblem>),
    /// The module holds no section `flmextensions`.
    NoSection,
    /// The reader refused the section.
    Section(SectionError),
}

impl fmt::Display for CollectionError {
    /// "Invalid WebAssembly module of the collection ‘L’: …", "Missing export
    /// ‘memory’ in the module of the collection ‘L’" and the other problems
    /// against the ABI (several separated by semicolons), "Missing section
    /// ‘flmextensions’ in the module of the collection ‘L’", "Truncated record
    /// at byte N of the section ‘flmextensions’ of the collection ‘L’ (…)", and
    /// the other refusals of the section.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = &self.label;
        let at = |offset: usize| format!("at byte {offset} of the section ‘{SECTION}’ of the collection ‘{label}’");
        match &self.kind {
            CollectionErrorKind::Module(detail) => {
                write!(f, "Invalid WebAssembly module of the collection ‘{label}’: {detail}")
            }
            CollectionErrorKind::Abi(problems) => {
                let problems: Vec<String> = problems.iter().map(|problem| abi_problem(label, problem)).collect();
                f.write_str(&problems.join("; "))
            }
            CollectionErrorKind::NoSection => {
                write!(f, "Missing section ‘{SECTION}’ in the module of the collection ‘{label}’")
            }
            CollectionErrorKind::Section(SectionError { offset, kind }) => match kind {
                SectionErrorKind::Truncated { needed, remaining } => {
                    write!(f, "Truncated record {} ({needed} bytes needed, {remaining} remain)", at(*offset))
                }
                SectionErrorKind::Version(version) => write!(
                    f,
                    "Unsupported format version {version} of the record {} (supported: {VERSION})",
                    at(*offset)
                ),
                SectionErrorKind::Length(length) => write!(
                    f,
                    "Invalid length {length} of the record {} (shorter than its header and texts)",
                    at(*offset)
                ),
                SectionErrorKind::NotText(field) => {
                    write!(f, "Invalid UTF-8 in the {field} of the record {}", at(*offset))
                }
                SectionErrorKind::DuplicateFile { folder, stem, path } => write!(
                    f,
                    "Duplicate file ‘{path}’ of the extension ‘{folder}/{stem}’ in the section ‘{SECTION}’ of the \
                     collection ‘{label}’"
                ),
            },
        }
    }
}

/// The message of `problem` of the module of the collection `label` against
/// the ABI: "Unsupported import ‘env.f’ in the module of the collection ‘L’
/// (the imports offered: ‘flm.call’, ‘flm.take’)", "Missing export ‘memory’ in
/// the module of the collection ‘L’", "Invalid type of the export ‘flm_entry’
/// in the module of the collection ‘L’ (expected: ‘…’)".
fn abi_problem(label: &str, problem: &AbiProblem) -> String {
    let module = format!("the module of the collection ‘{label}’");
    match problem {
        AbiProblem::Import { module: from, name } => {
            format!("Unsupported import ‘{from}.{name}’ in {module} (the imports offered: ‘flm.call’, ‘flm.take’)")
        }
        AbiProblem::ImportType { name, expected } => {
            format!("Invalid type of the import ‘flm.{name}’ in {module} (expected: ‘{expected}’)")
        }
        AbiProblem::MissingExport(name) => format!("Missing export ‘{name}’ in {module}"),
        AbiProblem::ExportType { name, expected } => {
            format!("Invalid type of the export ‘{name}’ in {module} (expected: ‘{expected}’)")
        }
    }
}

impl std::error::Error for CollectionError {}
