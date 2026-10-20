//! The document's settings for the live preview's *Settings* tool: the schema of a front
//! matter, and the document's file, read and written.
//!
//! The *Settings* tool, in the toolbar of the preview's page, shows the document's features
//! and lets the author change the settings that the document's [front matter](crate::seed)
//! gives them under `flm.features`, then save. The preview's server answers the tool's two
//! requests (read and save, at the address [`super::server::SETTINGS`]) through a
//! [`DocumentSettings`], which holds:
//!
//! - the schema from which the tool builds its forms ([`crate::seed::front_matter_schema`]),
//!   which covers the packages the document uses: the server starts with the schema of
//!   the compiled packages and replaces it, after each build that prepared the document
//!   again, with the schema of that preparation ([`DocumentSettings::set_schema`]);
//! - the document's file, which is read as it is at the moment of each request and, at a
//!   save, written with its front matter's `flm.features` edited in place
//!   ([`super::front_matter::edit_features`]): the values that changed are rewritten, and
//!   the author's comments and layout stay.
//!
//! A save changes the file on the disk; the watch loop sees that change and builds the
//! document again, as after any other save of the file.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value};

use crate::files::Files;
use crate::output::OutputError;
use super::front_matter::edit_features;
use crate::seed::{SeedError, written_features};

/// The function that writes a file's bytes: [`crate::output::write_file`] for the disk.
type Writer = Box<dyn Fn(&Path, &[u8]) -> Result<(), OutputError> + Send + Sync>;

/// What the preview's server needs to read and save the document's settings for the
/// *Settings* tool: the schema of a front matter, and the document's file with the
/// functions that read and write it. See the [module documentation](self).
///
/// Create it with [`DocumentSettings::on_disk`] for a file on the disk, or with
/// [`DocumentSettings::new`] to read and write through functions of the caller's.
pub struct DocumentSettings {
    schema: Mutex<Arc<Value>>,
    path: PathBuf,
    files: Arc<dyn Files>,
    write: Writer,
}

impl fmt::Debug for DocumentSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentSettings").field("path", &self.path).finish_non_exhaustive()
    }
}

/// The error of [`DocumentSettings::features`] and [`DocumentSettings::save`]: why the
/// settings of the document's features could not be read or saved. Its text (`Display`) is
/// the message the server sends to the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// The document's file cannot be read, or cannot be written; the server answers with
    /// the status `500`.
    File(String),
    /// The file's front matter cannot be read (it is not YAML, or does not have the shape
    /// of a front matter), or the settings cannot be written into it exactly as it is laid
    /// out ([`super::front_matter::edit_features`]). Nothing was written. The server
    /// answers with the status `409`. The text begins with the file's path and, when known,
    /// the place in the file.
    FrontMatter(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::File(detail) | SettingsError::FrontMatter(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for SettingsError {}

impl DocumentSettings {
    /// Creates the settings of the document whose file is `path`, read through `files` and
    /// written through `write`, with `schema` the schema from which the tool builds its
    /// forms until [`DocumentSettings::set_schema`] replaces it (the schema of a front
    /// matter, [`crate::seed::front_matter_schema`]).
    pub fn new(
        schema: Value,
        path: impl Into<PathBuf>,
        files: Arc<dyn Files>,
        write: impl Fn(&Path, &[u8]) -> Result<(), OutputError> + Send + Sync + 'static,
    ) -> DocumentSettings {
        DocumentSettings { schema: Mutex::new(Arc::new(schema)), path: path.into(), files, write: Box::new(write) }
    }

    /// Creates the settings of the document whose file on the disk is `path`, read through
    /// [`crate::files::DiskFiles`] and written through [`crate::output::write_file`].
    pub fn on_disk(schema: Value, path: impl Into<PathBuf>) -> DocumentSettings {
        DocumentSettings::new(schema, path, Arc::new(crate::files::DiskFiles), crate::output::write_file)
    }

    /// Returns the schema from which the tool builds its forms: the one given last to
    /// [`DocumentSettings::set_schema`], or the one the settings were created with.
    pub fn schema(&self) -> Arc<Value> {
        Arc::clone(&self.schema.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Replaces the schema from which the tool builds its forms with `schema`; the requests
    /// that follow answer it.
    pub fn set_schema(&self, schema: Arc<Value>) {
        *self.schema.lock().unwrap_or_else(PoisonError::into_inner) = schema;
    }

    /// Returns the path of the document's file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the file now and returns what its front matter holds under `flm.features`, as
    /// data ([`written_features`]): `null` when it holds nothing there.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the file cannot be read, or when its front matter
    /// cannot be read.
    pub fn features(&self) -> Result<Value, SettingsError> {
        let text = self.text()?;
        written_features(&text).map_err(|error| self.front_matter_error(&error))
    }

    /// Writes the file with its front matter's `flm.features` holding `features`, edited in
    /// place from the file's text as it is now ([`edit_features`]): only the values that
    /// differ from what the front matter holds are rewritten, and every other byte stays.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the file cannot be read or written, when its front
    /// matter cannot be read, and when the settings cannot be written into it exactly as it
    /// is laid out; nothing is written then.
    pub fn save(&self, features: &Map<String, Value>) -> Result<(), SettingsError> {
        let text = self.text()?;
        let written = edit_features(&text, features).map_err(|error| self.front_matter_error(&error))?;
        (self.write)(&self.path, written.as_bytes()).map_err(|error| SettingsError::File(error.to_string()))
    }

    /// Reads the file's text as it is now.
    fn text(&self) -> Result<String, SettingsError> {
        let bytes = self.files.read(&self.path).map_err(|error| SettingsError::File(error.to_string()))?;
        String::from_utf8(bytes)
            .map_err(|_| SettingsError::File(format!("the file `{}` must be text encoded as UTF-8", self.path.display())))
    }

    /// The error `error` of the file's front matter, prefixed with the file and the place.
    fn front_matter_error(&self, error: &SeedError) -> SettingsError {
        let file = self.path.display();
        SettingsError::FrontMatter(match error.place {
            Some(place) => format!("{file}:{place}: {error}"),
            None => format!("{file}: {error}"),
        })
    }
}
