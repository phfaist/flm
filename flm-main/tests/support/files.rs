//! Manifests of extensions and documents written in memory for the tests of `flm-main`,
//! each checked against the schema of its kind where the test writes it: a manifest against
//! the schema of an extension's manifest, a document's front matter against the schema of a
//! front matter, the two schemas `flm schema` prints for the program's own packages. So
//! every settings file a test uses is spelled once, in the test, and the schemas are checked
//! against all of them. Shared by the tests of `flm-main`, each including this file as a
//! module; a test includes the JSON Schema validator of `flm-core`'s tests through it.
//!
//! A file is written through the form that says what it is:
//!
//! - [`Written::extension`] and [`Written::document`]: a file written to be valid, which the
//!   schema accepts. A name it gives may still name nothing — a package, a workflow, an
//!   output — since a schema cannot know which exist.
//! - [`Written::refused_extension`] and [`Written::refused_document`]: a file written with a
//!   fault the schema sees, which the schema refuses.
//! - [`Written::faulty`]: a file written with a fault the schema does not see — text that
//!   is not YAML, or a value the schema admits and the reader refuses — and not checked.
//!
//! Every other file (a template, a file of an extension's folder that is not its manifest) is
//! written with [`MemoryFiles::with`].

#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use flm_main::files::MemoryFiles;
use flm_main::output::Naming;
use flm_main::packages::{carried, extension_schema};
use flm_main::seed::{front_matter_schema, read_front_matter, split};
use flm_main::stdpkg::{self, StdParts};
use serde_json::Value;

#[path = "../../../flm-core/tests/support/json_schema.rs"]
pub mod validator;

/// The compiled packages of the program: `stdcore`, `stdmain` and `std`.
pub fn factories() -> Vec<flm_core::core::registry::PackageFactory> {
    let stdmain = stdpkg::stdmain_factory(StdParts::new(Arc::new(Naming::Folder))).unwrap();
    carried(stdmain, &[]).unwrap()
}

/// The schema of an extension's manifest, and that of a front matter, for the program's own
/// packages; assembled once for all the tests of one test file.
fn schemas() -> &'static (Value, Value) {
    static SCHEMAS: OnceLock<(Value, Value)> = OnceLock::new();
    SCHEMAS.get_or_init(|| {
        let factories = factories();
        let factories: Vec<_> = factories.iter().collect();
        (extension_schema(&factories, &[]), front_matter_schema(&factories, &[]))
    })
}

/// The schema of an extension's manifest.
pub fn extension() -> Value {
    schemas().0.clone()
}

/// The schema of a front matter.
pub fn front_matter() -> Value {
    schemas().1.clone()
}

/// The YAML `text` as data.
pub fn data(text: &str) -> Value {
    read_front_matter(text, 1).expect("YAML").to_json()
}

/// Whether `schema` accepts `data`.
pub fn accepts(schema: &Value, data: &Value) -> bool {
    validator::valid(schema, schema, data)
}

/// Whether the schema of a manifest accepts the manifest `text`. An empty file is an
/// extension of the engine `data` that writes nothing.
pub fn extension_accepted(text: &str) -> bool {
    let data = match data(text) {
        Value::Null => Value::Object(Default::default()),
        data => data,
    };
    accepts(&schemas().0, &data)
}

/// Whether the schema of a front matter accepts the front matter of the document `text`;
/// `true` for a document without one.
pub fn document_accepted(text: &str) -> bool {
    let split = split(text).expect("a document");
    split.front_matter.is_none_or(|front_matter| accepts(&schemas().1, &data(front_matter)))
}

/// The forms in which a test writes a manifest or a document (see the module
/// documentation).
pub trait Written {
    /// Adds the manifest `path` holding `text`, written to be valid.
    ///
    /// # Panics
    ///
    /// When the schema of a manifest refuses `text`.
    fn extension(self, path: &str, text: &str) -> MemoryFiles;

    /// Adds the manifest `path` holding `text`, written with a fault the schema sees.
    ///
    /// # Panics
    ///
    /// When the schema of a manifest accepts `text`.
    fn refused_extension(self, path: &str, text: &str) -> MemoryFiles;

    /// Adds the document `path` holding `text`, written to be valid.
    ///
    /// # Panics
    ///
    /// When the schema of a front matter refuses the document's front matter.
    fn document(self, path: &str, text: &str) -> MemoryFiles;

    /// Adds the document `path` holding `text`, its front matter written with a fault the
    /// schema sees.
    ///
    /// # Panics
    ///
    /// When the schema of a front matter accepts the document's front matter.
    fn refused_document(self, path: &str, text: &str) -> MemoryFiles;

    /// Adds the file `path` holding `text`, written with a fault the schema does not see.
    fn faulty(self, path: &str, text: &str) -> MemoryFiles;
}

impl Written for MemoryFiles {
    fn extension(self, path: &str, text: &str) -> MemoryFiles {
        assert!(extension_accepted(text), "the manifest schema refuses `{path}`:\n{text}");
        self.with(path, text)
    }

    fn refused_extension(self, path: &str, text: &str) -> MemoryFiles {
        assert!(!extension_accepted(text), "the manifest schema accepts `{path}`:\n{text}");
        self.with(path, text)
    }

    fn document(self, path: &str, text: &str) -> MemoryFiles {
        assert!(document_accepted(text), "the front matter schema refuses `{path}`:\n{text}");
        self.with(path, text)
    }

    fn refused_document(self, path: &str, text: &str) -> MemoryFiles {
        assert!(!document_accepted(text), "the front matter schema accepts `{path}`:\n{text}");
        self.with(path, text)
    }

    fn faulty(self, path: &str, text: &str) -> MemoryFiles {
        self.with(path, text)
    }
}
