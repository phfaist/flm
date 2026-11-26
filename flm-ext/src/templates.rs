//! Reading compose templates from an extension's files, for every engine: one
//! template from its description's path ([`read`]), and every template of the
//! `templates/` folder ([`read_folder`]).
//!
//! A compose template's description is a data file of the extension whose
//! keys `name` and `output` give the template's name and the output it
//! serves; [`Template::from_data`] makes the template from the description.
//! The description's path says where it is, nothing more: a package object
//! names a description by any path of the extension's files, and the
//! `templates/` folder is where [`read_folder`] finds descriptions, in two
//! layouts: `templates/OUTPUT/FILE.yaml`, the template written as a file (a
//! description alone), and `templates/OUTPUT/FOLDER/flmtemplate.yaml`
//! ([`TEMPLATE_FILE`]), the template written as the folder `FOLDER/` holding
//! the description and the files that are the template's own. The names of
//! these folders and files give neither the template's name nor its output:
//! the description identifies the template. A name beginning with `.`
//! (`.DS_Store`) is left out, under `templates/` and under each
//! `templates/OUTPUT/` alike.
//!
//! Nothing is read from a listing of the description's folder: every file
//! read here is one whose path the description gives, relative to the
//! description's folder (in the folder form, the template's
//! folder): each is read through the extension's files as a path written in
//! the description ([`ExtensionFiles`], which joins it and keeps it inside
//! the extension's folder). The description's key `main` gives the
//! template's text, read through [`ExtensionFiles::read_text`]; without
//! `main`, the text is empty.
//! The files the description lists under `artifacts` (a stylesheet, a logo
//! the template ships) are read through [`ExtensionFiles::read_bytes`]; a
//! folder it lists under `artifacts` with the key `dir` is listed through
//! [`ExtensionFiles::list`], at every depth, and each of its files read the
//! same way.
//!
//! The files read are under a root ([`crate::paths`]): `ext+NAME` for an
//! extension, `NAME` the use name that first reached it, or `std` for the
//! program's own files. Every template read states the place of its text
//! ([`Template::source`]) and of each file it lists (the field `source` of
//! [`TemplateArtifact::File`](flm_core::stdlib::features::compose::TemplateArtifact::File)):
//! the qualified path of the file under that root, such as
//! `ext+mytemplate:templates/html/page/page.html`. A template engine reads
//! the files a template includes when it is applied, by a path read at the
//! place of the text that writes it.

use flm_core::stdlib::features::compose::Template;
use serde_json::Value;

use crate::engine::{ExtensionIdentity, LoadError};
use crate::paths::{QualifiedPath, Root, setting_path};
use crate::source::{ExtensionFiles, FilesError, FilesErrorKind, held_at};

/// The folder where [`read_folder`] finds the descriptions of an extension's
/// compose templates, one subfolder per output (`templates/OUTPUT/`).
pub const TEMPLATES_FOLDER: &str = "templates";

/// The name of the description of a compose template written as a folder
/// (`templates/OUTPUT/FOLDER/flmtemplate.yaml`).
pub const TEMPLATE_FILE: &str = "flmtemplate.yaml";

/// The compose template described at `path` (a data file of the extension,
/// relative to the extension's folder, read under the root `root`): its
/// description through
/// [`read_value`](ExtensionFiles::read_value), made by
/// [`Template::from_data`], which reads the template's name and output from
/// the description. `package` is the factory name. Every other file is read
/// by a path written in the description, relative to its folder
/// ([`ExtensionFiles`]). The text is the file that the
/// description's `main` gives, read through
/// [`read_text`](ExtensionFiles::read_text); no other file of the folder is
/// looked for. Each file the description lists under `artifacts` is read
/// through [`read_bytes`](ExtensionFiles::read_bytes). Each folder it lists
/// (the key `dir`) is listed through [`list`](ExtensionFiles::list) at every
/// depth: a name under which `list` finds a folder is entered, any other is
/// read as a file; a name beginning with `.` (`.DS_Store`) is left out at
/// every depth, file or folder. The folder's files reach
/// [`Template::from_data`] by their paths relative to the folder, in lexical
/// order. The place of the text and of each listed file is its path joined
/// to the description's folder under `root` (`ROOT:PATH`, [`QualifiedPath`]).
/// A path that a setting's `default` writes, where the schema marks it as a
/// path, is read at the place of the description ([`setting_path`]): a bare
/// path is the file beside the description, under `root`.
///
/// # Errors
///
/// [`LoadError::Load`], naming the extension and the path: a path the
/// extension's files refuse, or that names no file on this system; no file
/// at `path`; a description that cannot be read (a description that
/// [`Template::from_data`] refuses: its own text, after "in the extension
/// `STEM`, `PATH`: "). A text, or a listed file, whose path is refused or names
/// no file on this system, or that cannot be read, is refused by
/// [`Template::from_data`] as a file that cannot be read, with the text of
/// the [`FilesError`] ("the path `F` leaves the extension's folder", "must be
/// relative", "leads outside the extension's folder through a symbolic link",
/// "names no file on this system: " and the reason, or the reading's own
/// text). A listed folder is refused, after "in the extension `STEM`, `PATH`:
/// ", as "the shipped folder `DIR` of the compose template `NAME` " followed
/// by "is not a folder of the extension" (nothing is at the path, or a file
/// is), "holds no file" (none, or only names beginning with `.`), "leaves the
/// extension's folder", "must be a relative path", "leads outside the
/// extension's folder through a symbolic link", "lists `ENTRY`, which cannot
/// be read" (a name the listing gives and under which nothing can be read,
/// such as a dangling symbolic link), or "cannot be read: " and the reading's
/// own text; a listed folder whose path names no file on this system, as
/// "the shipped folder ‘DIR’ of the compose template ‘NAME’ names no file on
/// this system: " and the reason; a listed folder with an entry that the
/// extension's files refuse, as "the shipped folder ‘DIR’ of the compose
/// template ‘NAME’ lists ‘ENTRY’, which leads outside the extension's folder
/// through a symbolic link" (or "which names no file on this system: " and
/// the reason).
pub fn read(
    files: &dyn ExtensionFiles,
    root: &Root,
    path: &str,
    package: &str,
    extension: &ExtensionIdentity,
) -> Result<Template, LoadError> {
    let stem = &extension.stem;
    let short = package.strip_prefix(&format!("{}#", extension.identity)).unwrap_or(package);
    let load = |detail: String| LoadError::Load { detail };
    let description = match files.read_value("", path) {
        Ok(Some(description)) => description,
        Ok(None) => {
            return Err(load(format!(
                "the template description `{path}` of the package `{short}` of the extension `{stem}` is not a file of \
                 the extension"
            )));
        }
        Err(FilesError { kind: FilesErrorKind::Unreadable(detail), .. }) => {
            return Err(load(format!(
                "the template description ‘{path}’ of the package ‘{short}’ of the extension ‘{stem}’ cannot be \
                 read: {detail}"
            )));
        }
        Err(FilesError { kind: FilesErrorKind::Link, .. }) => {
            return Err(load(format!(
                "the template description ‘{path}’ of the package ‘{short}’ of the extension ‘{stem}’ leads outside \
                 the extension's folder through a symbolic link"
            )));
        }
        Err(FilesError { kind: FilesErrorKind::NoSystemForm(reason), .. }) => {
            return Err(load(format!(
                "the template description ‘{path}’ of the package ‘{short}’ of the extension ‘{stem}’ names no file \
                 on this system: {reason}"
            )));
        }
        Err(FilesError { kind: FilesErrorKind::Absolute | FilesErrorKind::Above, .. }) => {
            return Err(load(format!(
                "the template description `{path}` of the package `{short}` of the extension `{stem}` leaves the \
                 extension's folder"
            )));
        }
    };
    // The template's name, for the readers' texts: `Template::from_data`
    // reads the description, its `name` included, before it calls a reader.
    let name = description.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
    // `Template::from_data` states its own condition, naming the template and
    // the file.
    let text = |main: &str| files.read_text(path, main).map_err(|error| error.to_string());
    let listed = |file: &str| files.read_bytes(path, file).map_err(|error| error.to_string());
    let folders = |dir: &str| {
        let refused = |reason: &str| format!("the shipped folder `{dir}` of the compose template `{name}` {reason}");
        let Some(names) = files.list(path, dir).map_err(|error| match error.kind {
            FilesErrorKind::NoSystemForm(reason) => format!(
                "the shipped folder ‘{dir}’ of the compose template ‘{name}’ names no file on this system: {reason}"
            ),
            kind => refused(&folder_refusal(kind)),
        })?
        else {
            return Err(refused("is not a folder of the extension"));
        };
        let mut found = Vec::new();
        folder_files(files, path, dir, "", names, &mut found).map_err(|error| match error {
            FolderError::Files(_, FilesErrorKind::Unreadable(detail)) => refused(&format!("cannot be read: {detail}")),
            FolderError::Files(entry, kind) => format!(
                "the shipped folder ‘{dir}’ of the compose template ‘{name}’ lists ‘{entry}’, which {}",
                folder_refusal(kind)
            ),
            FolderError::Unread(entry) => refused(&format!("lists `{entry}`, which cannot be read")),
        })?;
        found.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Some(found))
    };
    // A file read through `files` is held at the same join of the
    // description's folder and the written path.
    let place = |written: &str| {
        let held = held_at(path, written).ok()?;
        QualifiedPath::new(root.clone(), &held).ok().map(|place| place.to_string())
    };
    // A path that a setting's default writes is read at the place of the
    // description.
    let default_path = |written: &str| {
        let described = QualifiedPath::new(root.clone(), path).map_err(|error| error.to_string())?;
        setting_path(written, &described).map_err(|error| error.to_string())
    };
    Template::from_data(package, description, &text, &listed, &folders, &place, &default_path)
        .map_err(|detail| load(format!("in the extension `{stem}`, `{path}`: {detail}")))
}

/// What a message says of a shipped folder, or of an entry it lists, that the
/// extension's files refused for `kind`: "leaves the extension's folder",
/// "must be a relative path", "leads outside the extension's folder through a
/// symbolic link", "names no file on this system: " and the reason, or
/// "cannot be read: " and the reading's text.
fn folder_refusal(kind: FilesErrorKind) -> String {
    match kind {
        FilesErrorKind::Above => String::from("leaves the extension's folder"),
        FilesErrorKind::Absolute => String::from("must be a relative path"),
        FilesErrorKind::Link => String::from("leads outside the extension's folder through a symbolic link"),
        FilesErrorKind::NoSystemForm(reason) => format!("names no file on this system: {reason}"),
        FilesErrorKind::Unreadable(detail) => format!("cannot be read: {detail}"),
    }
}

/// Why the files of a shipped folder cannot be gathered ([`folder_files`]).
enum FolderError {
    /// A listing or a read of the entry at this path, relative to the shipped
    /// folder, failed.
    Files(String, FilesErrorKind),
    /// The entry at this path, relative to the shipped folder, is listed, and
    /// reading it finds nothing (a dangling symbolic link).
    Unread(String),
}

/// Adds to `found` every file under the folder `folder` (a path written in
/// the description `description`) whose entries are `names`, each by its path
/// relative to the shipped folder (`within` the path of `folder` relative to
/// it, the empty text for the shipped folder itself) and its bytes, entering
/// every subfolder and leaving out every name beginning with `.`.
fn folder_files(
    files: &dyn ExtensionFiles,
    description: &str,
    folder: &str,
    within: &str,
    names: Vec<String>,
    found: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), FolderError> {
    for name in names.into_iter().filter(|name| !name.starts_with('.')) {
        // `./` keeps a name `~` a name of the folder, not the home folder.
        let full =
            if folder.is_empty() { format!("./{name}") } else { format!("{}/{name}", folder.trim_end_matches('/')) };
        let relative = if within.is_empty() { name } else { format!("{within}/{name}") };
        let failed = |error: FilesError| FolderError::Files(relative.clone(), error.kind);
        match files.list(description, &full).map_err(failed)? {
            Some(entries) => folder_files(files, description, &full, &relative, entries, found)?,
            None => match files.read_bytes(description, &full).map_err(failed)? {
                Some(bytes) => found.push((relative, bytes)),
                None => return Err(FolderError::Unread(relative)),
            },
        }
    }
    Ok(())
}

/// Every template of the `templates/` folder, read under the root `root`:
/// each folder of `templates/`
/// (`templates/OUTPUT/`) holds templates, and each entry of that folder, in
/// the listing's order, that is a description or holds one is read with
/// [`read`]: a file whose name ends in `.yaml` (a name under which
/// [`list`](ExtensionFiles::list) finds no folder), the description
/// `FILE.yaml`, and a folder whose listing holds [`TEMPLATE_FILE`], the
/// description `FOLDER/flmtemplate.yaml`. A file of `templates/` (a
/// `README`), a folder of `templates/OUTPUT/` whose listing does not hold
/// [`TEMPLATE_FILE`] and any other file of `templates/OUTPUT/` are left out.
/// Each description gives its template's name and output; the names of the
/// folders and files give neither. A name beginning with `.` (`.DS_Store`) is
/// left out at both levels, under `templates/` and under `templates/OUTPUT/`.
/// None when there is no `templates/` folder.
///
/// # Errors
///
/// Every problem found: a folder that cannot be listed, and each problem of
/// [`read`].
pub fn read_folder(
    files: &dyn ExtensionFiles,
    root: &Root,
    package: &str,
    extension: &ExtensionIdentity,
) -> Result<Vec<Template>, Vec<LoadError>> {
    let stem = &extension.stem;
    let unlisted = |folder: &str, error: FilesError| LoadError::Load {
        detail: format!("the folder ‘{folder}’ of the extension ‘{stem}’ {}", folder_refusal(error.kind)),
    };
    let outputs = match files.list("", TEMPLATES_FOLDER) {
        Ok(Some(outputs)) => outputs,
        Ok(None) => return Ok(Vec::new()),
        Err(error) => return Err(vec![unlisted(TEMPLATES_FOLDER, error)]),
    };
    let mut templates = Vec::new();
    let mut errors = Vec::new();
    for output in outputs.iter().filter(|name| !name.starts_with('.')) {
        let folder = format!("{TEMPLATES_FOLDER}/{output}");
        let names = match files.list("", &folder) {
            Ok(Some(names)) => names,
            Ok(None) => continue,
            Err(error) => {
                errors.push(unlisted(&folder, error));
                continue;
            }
        };
        for entry in names.iter().filter(|name| !name.starts_with('.')) {
            let at = format!("{folder}/{entry}");
            let description = match files.list("", &at) {
                Ok(Some(inner)) if inner.iter().any(|name| name == TEMPLATE_FILE) => format!("{at}/{TEMPLATE_FILE}"),
                Ok(None) if entry.ends_with(".yaml") => at,
                Ok(_) => continue,
                Err(error) => {
                    errors.push(unlisted(&at, error));
                    continue;
                }
            };
            match read(files, root, &description, package, extension) {
                Ok(template) => templates.push(template),
                Err(error) => errors.push(error),
            }
        }
    }
    if errors.is_empty() { Ok(templates) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::source::MemoryFiles;
    use flm_core::stdlib::features::compose::TemplateArtifact;
    use serde_json::json;

    /// The root of the extension `ext` of the tests.
    fn root() -> Root {
        Root::Ext(String::from("ext"))
    }

    fn identity() -> ExtensionIdentity {
        ExtensionIdentity { identity: String::from("ext/flm.yaml"), stem: String::from("ext"), name: String::from("ext") }
    }

    fn description(file: &str) -> serde_json::Value {
        json!({
            "name": "fancy",
            "output": "html",
            "engine": "minijinja",
            "accepts": "all",
            "artifacts": {
                "style": { "file": "style.css", "template": true },
                "logo": { "file": file },
            },
        })
    }

    /// Why [`read`] refuses the description `description` at `path`, the
    /// extension's one file.
    fn refusal_of(path: &str, description: serde_json::Value) -> String {
        match read(&MemoryFiles::new().with_value(path, description), &root(), path, "ext/flm.yaml#p", &identity()) {
            Err(LoadError::Load { detail }) => detail,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_template_folder_gives_the_folder_of_every_path() {
        let mut fancy = description("../../../img/logo.png");
        fancy["main"] = json!("fancy.html");
        let files = MemoryFiles::new()
            .with_value("templates/html/fancy/flmtemplate.yaml", fancy)
            .with_text("templates/html/fancy/fancy.html", "<link href=\"{{ artifacts.style }}\">")
            .with_text("templates/html/fancy/style.css", "p { color: {{ config.color }}; }")
            .with_bytes("img/logo.png", &[0x89, b'P', b'N', b'G', 0xff]);
        let template = read(&files, &root(), "templates/html/fancy/flmtemplate.yaml", "ext/flm.yaml#p", &identity()).unwrap();
        assert_eq!((template.name.as_str(), template.text.as_str()), ("fancy", "<link href=\"{{ artifacts.style }}\">"));
        assert_eq!(template.source.as_deref(), Some("ext+ext:templates/html/fancy/fancy.html"));
        let artifacts: Vec<(&str, &str, &str, bool, &[u8])> = template
            .artifacts
            .iter()
            .map(|artifact| match artifact {
                TemplateArtifact::File { name, extension, media_type, apply, bytes, .. } => {
                    (name.as_str(), extension.as_str(), media_type.as_str(), *apply, bytes.as_slice())
                }
                TemplateArtifact::Folder { name, .. } => panic!("`{name}` is a folder"),
            })
            .collect();
        assert_eq!(
            artifacts,
            [
                ("style", "css", "text/css", true, &b"p { color: {{ config.color }}; }"[..]),
                ("logo", "png", "image/png", false, &[0x89, b'P', b'N', b'G', 0xff][..]),
            ]
        );
    }

    #[test]
    fn a_listed_file_leaving_the_folder_or_missing_is_refused() {
        let files = |logo: &str| {
            MemoryFiles::new()
                .with_value("templates/html/fancy.yaml", description(logo))
                .with_text("templates/html/style.css", "p {}")
        };
        let refused = |logo: &str| match read(&files(logo), &root(), "templates/html/fancy.yaml", "ext/flm.yaml#p", &identity()) {
            Err(LoadError::Load { detail }) => detail,
            other => panic!("expected a refusal, got {other:?}"),
        };
        // Each is refused: `refused` fails the test on a description that reads.
        refused("../../../logo.png");
        refused("logo.png");
    }

    #[test]
    fn a_text_leaving_the_extension_s_folder_is_refused() {
        let description =
            json!({ "name": "fancy", "output": "html", "engine": "minijinja", "main": "../../../../x.html", "accepts": "all" });
        refusal_of("templates/html/fancy/flmtemplate.yaml", description);
    }

    #[test]
    fn a_description_gives_the_name_and_the_output_wherever_it_stands() {
        let description = json!({ "name": "fancy", "output": "html", "engine": "content-only", "accepts": "all" });
        for path in ["other/fancy.yaml", "./flmtemplate.yaml", "templates/latex/odd.yaml"] {
            let files = MemoryFiles::new().with_value(path, description.clone());
            let template = read(&files, &root(), path, "ext/flm.yaml#p", &identity()).unwrap();
            assert_eq!((template.name.as_str(), template.output.as_str()), ("fancy", "html"), "{path}");
        }
    }

    #[test]
    fn the_templates_folder_s_descriptions_give_their_names_and_outputs() {
        let described =
            |name: &str, output: &str| json!({ "name": name, "output": output, "engine": "content-only", "accepts": "all" });
        let files = MemoryFiles::new()
            .with_value("templates/html/fancy/flmtemplate.yaml", described("plain", "text"))
            .with_value("templates/html/flmtemplate.yaml", described("fancy", "html"));
        let templates = read_folder(&files, &root(), "ext/flm.yaml#p", &identity()).unwrap();
        let found: Vec<(&str, &str)> =
            templates.iter().map(|template| (template.name.as_str(), template.output.as_str())).collect();
        assert_eq!(found, [("plain", "text"), ("fancy", "html")]);
    }

    /// Files whose listing of `icons`, written in `templates/html/t.yaml`,
    /// also names `gone.svg`, under which nothing can be read, as a dangling
    /// symbolic link.
    struct Dangling(MemoryFiles);

    impl ExtensionFiles for Dangling {
        fn read_text(&self, from: &str, path: &str) -> Result<Option<String>, FilesError> {
            self.0.read_text(from, path)
        }
        fn read_bytes(&self, from: &str, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
            self.0.read_bytes(from, path)
        }
        fn read_value(&self, from: &str, path: &str) -> Result<Option<serde_json::Value>, FilesError> {
            self.0.read_value(from, path)
        }
        fn list(&self, from: &str, path: &str) -> Result<Option<Vec<String>>, FilesError> {
            let mut names = self.0.list(from, path)?;
            if (from, path) == ("templates/html/t.yaml", "icons") {
                names.get_or_insert_with(Vec::new).push(String::from("gone.svg"));
            }
            Ok(names)
        }
        fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync> {
            Arc::new(Dangling(self.0.clone()))
        }
    }

    #[test]
    fn a_listed_entry_of_a_shipped_folder_that_cannot_be_read_fails_the_load() {
        let description = json!({ "name": "t", "output": "html", "engine": "content-only", "accepts": "all", "artifacts": { "icons": { "dir": "icons" } } });
        let files = Dangling(
            MemoryFiles::new()
                .with_value("templates/html/t.yaml", description)
                .with_bytes("templates/html/icons/check.svg", b"<svg/>"),
        );
        match read(&files, &root(), "templates/html/t.yaml", "ext/flm.yaml#p", &identity()) {
            Err(LoadError::Load { .. }) => {}
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
