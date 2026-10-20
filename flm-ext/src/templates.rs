//! Reading compose templates from an extension's files, for every engine: one
//! template from its description's path ([`read`]), and every template of the
//! `templates/` folder ([`read_folder`]).
//!
//! A compose template's description stands under `templates/OUTPUT/`, in one
//! of two shapes: `templates/OUTPUT/NAME.yaml`, the template written as a
//! file (a description alone), or `templates/OUTPUT/NAME/flmtemplate.yaml`
//! ([`TEMPLATE_FILE`]), the template written as the folder `NAME/` holding
//! the description and the files that are the template's own. The
//! template's name is `NAME` in both shapes: in the first, the name of the
//! file without the suffix `.yaml` (`fancy.v2.yaml` gives `fancy.v2`), so
//! that `templates/html/flmtemplate.yaml` is the template `flmtemplate`. A
//! description of neither shape is refused, and so is a name written both
//! ways. A name beginning with `.` (`.DS_Store`) is left out, under
//! `templates/` and under each `templates/OUTPUT/` alike.
//! [`Template::from_data`] makes the template from the description.
//!
//! Nothing is read from a listing of the description's folder: every file a
//! template reads is one whose path the description, or an include, gives,
//! relative to the description's folder (in the folder form, the template's
//! folder). The description's key `main` gives the template's text, read
//! through [`ExtensionFiles::read_text`]; without `main`, the text is empty.
//! The files the description lists under `artifacts` (a stylesheet, a logo
//! the template ships) are read through [`ExtensionFiles::read_bytes`]; a
//! folder it lists under `artifacts` with the key `dir` is listed through
//! [`ExtensionFiles::list`], at every depth, and each of its files read the
//! same way.
//!
//! Every template read is given the handle of the files it may include
//! ([`Template::files`]): the files of the extension
//! ([`ExtensionFiles::shared`]), read by a path relative to the
//! description's folder when the template is applied.

use std::sync::Arc;

use flm_core::stdcore::features::compose::{Template, TemplateFiles};

use crate::engine::{ExtensionIdentity, LoadError};
use crate::paths::{self, PathErrorKind};
use crate::source::{ExtensionFiles, FilesError};

/// The folder of an extension's templates, one subfolder per output.
pub const TEMPLATES_FOLDER: &str = "templates";

/// The name of the description of a compose template written as a folder
/// (`templates/OUTPUT/NAME/flmtemplate.yaml`).
pub const TEMPLATE_FILE: &str = "flmtemplate.yaml";

/// The name `NAME` of the compose template described at `path` (normalized)
/// when `path` is of one of the two shapes, `templates/OUTPUT/NAME.yaml`
/// (`NAME` not empty) or `templates/OUTPUT/NAME/flmtemplate.yaml`; `None` for
/// any other path.
fn template_name(path: &str) -> Option<&str> {
    match path.split('/').collect::<Vec<_>>()[..] {
        [TEMPLATES_FOLDER, _, file] => file.strip_suffix(".yaml").filter(|name| !name.is_empty()),
        [TEMPLATES_FOLDER, _, name, TEMPLATE_FILE] => Some(name),
        _ => None,
    }
}

/// The compose template described at `path` (a data file of the extension,
/// `templates/OUTPUT/NAME.yaml` or `templates/OUTPUT/NAME/flmtemplate.yaml`):
/// its description through [`read_value`](ExtensionFiles::read_value), made
/// by [`Template::from_data`]. `package` is the factory name. `path` is
/// normalized first (a path from a package object may be written
/// `./templates/html/fancy.yaml`), and must be of one of the two shapes, its
/// `OUTPUT` the output the description gives; the template's name is `NAME`
/// in both (in the first, the name of the file without the suffix `.yaml`).
/// The text is the file that the description's `main` gives, its path
/// joined to the description's folder
/// and normalized, read through [`read_text`](ExtensionFiles::read_text); no
/// other file of the folder is looked for. Each file the
/// description lists under `artifacts` is read through
/// [`read_bytes`](ExtensionFiles::read_bytes), its path joined to the
/// description's folder and normalized. Each folder it lists (the key `dir`)
/// is joined the same way and listed through [`list`](ExtensionFiles::list)
/// at every depth: a name under which `list` finds a folder is entered, any
/// other is read as a file; a name beginning with `.` (`.DS_Store`) is left
/// out at every depth, file or folder. The folder's files reach
/// [`Template::from_data`] by their paths relative to the folder, in lexical
/// order. The template's [`files`](Template::files) are the extension's files
/// ([`ExtensionFiles::shared`]): the path an include names is joined to the
/// description's folder and normalized, and read through
/// [`read_bytes`](ExtensionFiles::read_bytes) as UTF-8 text; a path that
/// leaves the extension's folder or is absolute, a file that cannot be read
/// and a file that is not UTF-8 text are refused, the text naming the
/// template and the path as the include writes it.
///
/// # Errors
///
/// [`LoadError::Load`], naming the extension and the path: a path that is
/// absolute or leaves the extension's folder; a path of neither shape; no
/// file at `path`; a
/// description that cannot be read (a description that
/// [`Template::from_data`] refuses: its own text, after "in the extension
/// `STEM`, `PATH`: "); a description whose `output` is not the path's
/// `OUTPUT`. A text, or a listed file, whose path leaves the
/// extension's folder, or that cannot be read, is refused by
/// [`Template::from_data`] as a file that cannot be read, with the text "the
/// path `F` leaves the extension's folder" (or "must be relative"), or the
/// reading's own text. A listed folder is refused, after "in the extension
/// `STEM`, `PATH`: ", as "the shipped folder `DIR` of the compose template
/// `NAME` " followed by "is not a folder of the extension" (nothing is at
/// the path, or a file is), "holds no file" (none, or only names beginning
/// with `.`), "leaves the extension's folder", "must be a relative path",
/// "lists `ENTRY`, which cannot be read" (a name the listing gives and under
/// which nothing can be read, such as a dangling symbolic link), or "cannot
/// be read: " and the reading's own text.
pub fn read(
    files: &dyn ExtensionFiles,
    path: &str,
    package: &str,
    extension: &ExtensionIdentity,
) -> Result<Template, LoadError> {
    let stem = &extension.stem;
    let short = package.strip_prefix(&format!("{}#", extension.identity)).unwrap_or(package);
    let load = |detail: String| LoadError::Load { detail };
    let path = paths::normalize(path).map_err(|error| {
        load(match error.kind {
            PathErrorKind::Backslash => format!(
                "the template description `{path}` of the package `{short}` of the extension `{stem}` holds `\\`; a \
                 path inside the extension uses `/`"
            ),
            PathErrorKind::LeavesRoot | PathErrorKind::Absolute => format!(
                "the template description `{path}` of the package `{short}` of the extension `{stem}` leaves the \
                 extension's folder"
            ),
        })
    })?;
    let Some(name) = template_name(&path) else {
        return Err(load(format!(
            "the template description `{path}` of the package `{short}` of the extension `{stem}` must stand at \
             `{TEMPLATES_FOLDER}/OUTPUT/NAME.yaml` or `{TEMPLATES_FOLDER}/OUTPUT/NAME/{TEMPLATE_FILE}`"
        )));
    };
    let description = match files.read_value(&path) {
        Ok(Some(description)) => description,
        Ok(None) => {
            return Err(load(format!(
                "the template description `{path}` of the package `{short}` of the extension `{stem}` is not a file of \
                 the extension"
            )));
        }
        Err(error) => {
            return Err(load(format!(
                "the description `{path}` of the compose template `{name}` of the extension `{stem}` cannot be read: {}",
                error.detail
            )));
        }
    };
    // `Template::from_data` states its own condition, naming the template and
    // the file.
    let text = |main: &str| {
        let joined = paths::join(&path, main).map_err(|error| error.to_string())?;
        files.read_text(&joined).map_err(|error| error.detail)
    };
    let listed = |file: &str| {
        let joined = paths::join(&path, file).map_err(|error| error.to_string())?;
        files.read_bytes(&joined).map_err(|error| error.detail)
    };
    let folders = |dir: &str| {
        let refused = |reason: &str| format!("the shipped folder `{dir}` of the compose template `{name}` {reason}");
        let joined = paths::join(&path, dir).map_err(|error| match error.kind {
            PathErrorKind::LeavesRoot => refused("leaves the extension's folder"),
            PathErrorKind::Absolute => refused("must be a relative path"),
            PathErrorKind::Backslash => refused("holds `\\`; a path inside the extension uses `/`"),
        })?;
        let unreadable = |error: FilesError| refused(&format!("cannot be read: {}", error.detail));
        let Some(names) = files.list(&joined).map_err(unreadable)? else {
            return Err(refused("is not a folder of the extension"));
        };
        let mut found = Vec::new();
        folder_files(files, &joined, "", names, &mut found).map_err(|error| match error {
            FolderError::Files(error) => unreadable(error),
            FolderError::Unread(entry) => refused(&format!("lists `{entry}`, which cannot be read")),
        })?;
        found.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Some(found))
    };
    let included = IncludedFiles { files: files.shared(), description: path.clone(), template: name.to_string() };
    let template = Template::from_data(package, name, description, &text, &listed, &folders, Some(Arc::new(included)))
        .map_err(|detail| load(format!("in the extension `{stem}`, `{path}`: {detail}")))?;
    let output = &template.output;
    if path.split('/').nth(1) != Some(output.as_str()) {
        return Err(load(format!(
            "the description `{path}` of the compose template `{name}` of the extension `{stem}` gives the output \
             `{output}` and must stand under `{TEMPLATES_FOLDER}/{output}/`"
        )));
    }
    Ok(template)
}

/// The files a compose template of an extension may include
/// ([`Template::files`]): the extension's files, by a path relative to the
/// folder of the template's description.
struct IncludedFiles {
    /// The extension's files.
    files: Arc<dyn ExtensionFiles + Send + Sync>,
    /// The path of the template's description (normalized).
    description: String,
    /// The template's name, for the messages.
    template: String,
}

impl TemplateFiles for IncludedFiles {
    fn read_text(&self, path: &str) -> Result<Option<String>, String> {
        let template = &self.template;
        let refused = |reason: &str| format!("the compose template `{template}` includes `{path}`, {reason}");
        let joined = paths::join(&self.description, path).map_err(|error| match error.kind {
            PathErrorKind::LeavesRoot => refused("which leaves the extension's folder"),
            PathErrorKind::Absolute => refused("which must be a relative path"),
            PathErrorKind::Backslash => refused("which holds `\\`; a path inside the extension uses `/`"),
        })?;
        let Some(bytes) = self.files.read_bytes(&joined).map_err(|error| {
            refused(&format!("and the file cannot be read: {}", error.detail))
        })?
        else {
            return Ok(None);
        };
        String::from_utf8(bytes).map(Some).map_err(|_| refused("and the file is not UTF-8 text"))
    }
}

/// Why the files of a shipped folder cannot be gathered ([`folder_files`]).
enum FolderError {
    /// A listing or a read failed.
    Files(FilesError),
    /// The entry at this path, relative to the shipped folder, is listed, and
    /// reading it finds nothing (a dangling symbolic link).
    Unread(String),
}

/// Adds to `found` every file under the folder `folder` (a normalized path of
/// the extension) whose entries are `names`, each by its path relative to the
/// shipped folder (`within` the path of `folder` relative to it, the empty
/// text for the shipped folder itself) and its bytes, entering every subfolder
/// and leaving out every name beginning with `.`.
fn folder_files(
    files: &dyn ExtensionFiles,
    folder: &str,
    within: &str,
    names: Vec<String>,
    found: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), FolderError> {
    for name in names.into_iter().filter(|name| !name.starts_with('.')) {
        let full = if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") };
        let relative = if within.is_empty() { name } else { format!("{within}/{name}") };
        match files.list(&full).map_err(FolderError::Files)? {
            Some(entries) => folder_files(files, &full, &relative, entries, found)?,
            None => match files.read_bytes(&full).map_err(FolderError::Files)? {
                Some(bytes) => found.push((relative, bytes)),
                None => return Err(FolderError::Unread(relative)),
            },
        }
    }
    Ok(())
}

/// Every template of the `templates/` folder: each entry of `templates/` must
/// be a folder (`templates/OUTPUT/`), and each entry of that folder, in the
/// listing's order, that is a compose template is read with [`read`]: a file
/// `NAME.yaml` (a name under which [`list`](ExtensionFiles::list) finds no
/// folder) and a folder `NAME` whose listing holds [`TEMPLATE_FILE`], each
/// the template `NAME`. A folder whose listing does not hold
/// [`TEMPLATE_FILE`] and any other file are left out. A file
/// `flmtemplate.yaml` is the template `flmtemplate`, like a folder
/// `flmtemplate/` that holds [`TEMPLATE_FILE`]. A name beginning with `.`
/// (`.DS_Store`) is left out at both levels, under `templates/` and under
/// `templates/OUTPUT/`. None when there is no `templates/` folder.
///
/// # Errors
///
/// Every problem found: an entry of `templates/` that is not a folder, a
/// folder that cannot be listed, a name written both as a file and as a
/// folder (one problem, and neither is read), and each problem of [`read`].
pub fn read_folder(
    files: &dyn ExtensionFiles,
    package: &str,
    extension: &ExtensionIdentity,
) -> Result<Vec<Template>, Vec<LoadError>> {
    let stem = &extension.stem;
    let unlisted = |folder: &str, detail: String| LoadError::Load {
        detail: format!("the folder `{folder}` of the extension `{stem}` cannot be read: {detail}"),
    };
    let outputs = match files.list(TEMPLATES_FOLDER) {
        Ok(Some(outputs)) => outputs,
        Ok(None) => return Ok(Vec::new()),
        Err(error) => return Err(vec![unlisted(TEMPLATES_FOLDER, error.detail)]),
    };
    let mut templates = Vec::new();
    let mut errors = Vec::new();
    for output in outputs.iter().filter(|name| !name.starts_with('.')) {
        let folder = format!("{TEMPLATES_FOLDER}/{output}");
        let names = match files.list(&folder) {
            Ok(Some(names)) => names,
            Ok(None) => {
                errors.push(LoadError::Load {
                    detail: format!(
                        "`{folder}` of the extension `{stem}` must be a folder holding the templates of one output \
                         (`{TEMPLATES_FOLDER}/OUTPUT/`)"
                    ),
                });
                continue;
            }
            Err(error) => {
                errors.push(unlisted(&folder, error.detail));
                continue;
            }
        };
        // The path of each template's description and the template's name,
        // in the listing's order.
        let mut descriptions = Vec::new();
        let mut template_names = Vec::new();
        for entry in names.iter().filter(|name| !name.starts_with('.')) {
            let at = format!("{folder}/{entry}");
            match files.list(&at) {
                Ok(Some(inner)) if inner.iter().any(|name| name == TEMPLATE_FILE) => {
                    descriptions.push(format!("{at}/{TEMPLATE_FILE}"));
                    template_names.push(entry.as_str());
                }
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Some(name) = entry.strip_suffix(".yaml") {
                        descriptions.push(at);
                        template_names.push(name);
                    }
                }
                Err(error) => errors.push(unlisted(&at, error.detail)),
            }
        }
        for (index, (description, &name)) in descriptions.iter().zip(&template_names).enumerate() {
            if template_names.iter().filter(|&&other| other == name).count() > 1 {
                if !template_names[..index].contains(&name) {
                    errors.push(LoadError::Load {
                        detail: format!(
                            "the compose template `{name}` of the extension `{stem}` must be written as the file \
                             `{folder}/{name}.yaml` or as the folder `{folder}/{name}/` holding `{TEMPLATE_FILE}`, and \
                             it is written both ways"
                        ),
                    });
                }
                continue;
            }
            match read(files, description, package, extension) {
                Ok(template) => templates.push(template),
                Err(error) => errors.push(error),
            }
        }
    }
    if errors.is_empty() { Ok(templates) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MemoryFiles;
    use flm_core::stdcore::features::compose::TemplateArtifact;
    use serde_json::json;

    fn identity() -> ExtensionIdentity {
        ExtensionIdentity { identity: String::from("ext/flm.yaml"), stem: String::from("ext") }
    }

    fn description(file: &str) -> serde_json::Value {
        json!({
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
        match read(&MemoryFiles::new().with_value(path, description), path, "ext/flm.yaml#p", &identity()) {
            Err(LoadError::Load { detail }) => detail,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_template_folder_gives_the_name_and_the_folder_of_every_path() {
        let mut fancy = description("../../../img/logo.png");
        fancy["main"] = json!("fancy.html");
        let files = MemoryFiles::new()
            .with_value("templates/html/fancy/flmtemplate.yaml", fancy)
            .with_text("templates/html/fancy/fancy.html", "<link href=\"{{ artifacts.style }}\">")
            .with_text("templates/html/fancy/style.css", "p { color: {{ config.color }}; }")
            .with_bytes("img/logo.png", &[0x89, b'P', b'N', b'G', 0xff]);
        let template = read(&files, "templates/html/fancy/flmtemplate.yaml", "ext/flm.yaml#p", &identity()).unwrap();
        assert_eq!((template.name.as_str(), template.text.as_str()), ("fancy", "<link href=\"{{ artifacts.style }}\">"));
        let artifacts: Vec<(&str, &str, &str, bool, &[u8])> = template
            .artifacts
            .iter()
            .map(|artifact| match artifact {
                TemplateArtifact::File { name, extension, media_type, apply, bytes } => {
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
        let refused = |logo: &str| match read(&files(logo), "templates/html/fancy.yaml", "ext/flm.yaml#p", &identity()) {
            Err(LoadError::Load { detail }) => detail,
            other => panic!("expected a refusal, got {other:?}"),
        };
        assert_eq!(
            refused("../../../logo.png"),
            "in the extension `ext`, `templates/html/fancy.yaml`: the file `../../../logo.png` of the artifact \
             `logo` of the compose template `fancy` cannot be read: the path `../../../logo.png` leaves the \
             extension's folder"
        );
        assert_eq!(
            refused("logo.png"),
            "in the extension `ext`, `templates/html/fancy.yaml`: the file `logo.png` of the artifact `logo` of the \
             compose template `fancy` is not a file of the compose template's package"
        );
    }

    #[test]
    fn a_text_leaving_the_extension_s_folder_is_refused() {
        let description = json!({ "output": "html", "engine": "minijinja", "main": "../../../../x.html", "accepts": "all" });
        let detail = refusal_of("templates/html/fancy/flmtemplate.yaml", description);
        assert!(detail.contains("`../../../../x.html`"), "{detail}");
    }

    #[test]
    fn a_description_of_neither_shape_is_refused() {
        for path in ["other/fancy.yaml", "./flmtemplate.yaml"] {
            refusal_of(path, json!({ "output": "html", "engine": "content-only", "accepts": "all" }));
        }
    }

    #[test]
    fn a_description_whose_output_is_not_its_folder_s_is_refused() {
        refusal_of("templates/latex/odd.yaml", json!({ "output": "html", "engine": "content-only", "accepts": "all" }));
    }

    #[test]
    fn a_flmtemplate_yaml_file_under_the_output_folder_is_the_template_flmtemplate() {
        let description = json!({ "output": "html", "engine": "content-only", "accepts": "all" });
        let files = MemoryFiles::new().with_value("templates/html/flmtemplate.yaml", description);
        let templates = read_folder(&files, "ext/flm.yaml#p", &identity()).unwrap();
        let names: Vec<&str> = templates.iter().map(|template| template.name.as_str()).collect();
        assert_eq!(names, ["flmtemplate"]);
    }

    /// Files whose listing of `templates/html/icons` also names `gone.svg`,
    /// under which nothing can be read, as a dangling symbolic link.
    struct Dangling(MemoryFiles);

    impl ExtensionFiles for Dangling {
        fn read_text(&self, path: &str) -> Result<Option<String>, FilesError> {
            self.0.read_text(path)
        }
        fn read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
            self.0.read_bytes(path)
        }
        fn read_value(&self, path: &str) -> Result<Option<serde_json::Value>, FilesError> {
            self.0.read_value(path)
        }
        fn list(&self, path: &str) -> Result<Option<Vec<String>>, FilesError> {
            let mut names = self.0.list(path)?;
            if path == "templates/html/icons" {
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
        let description = json!({ "output": "html", "engine": "content-only", "accepts": "all", "artifacts": { "icons": { "dir": "icons" } } });
        let files = Dangling(
            MemoryFiles::new()
                .with_value("templates/html/t.yaml", description)
                .with_bytes("templates/html/icons/check.svg", b"<svg/>"),
        );
        match read(&files, "templates/html/t.yaml", "ext/flm.yaml#p", &identity()) {
            Err(LoadError::Load { detail }) => assert_eq!(
                detail,
                "in the extension `ext`, `templates/html/t.yaml`: the shipped folder `icons` of the compose template \
                 `t` lists `gone.svg`, which cannot be read"
            ),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
