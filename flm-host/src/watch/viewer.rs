//! The viewer settings of the live preview: how the author wants the preview shown, kept in
//! a file of the author's own, `watch.json` in the folder `.flm` of the home folder.
//!
//! The viewer settings concern the preview alone, never a document: the editor that a place
//! opens in, the theme of the tools page, whether the page scrolls to what changed, whether
//! the *Problems* tab opens when a build has errors, whether a text output wraps its long
//! lines, and the edge and the sizes of the tools page's panel. They hold across documents,
//! ports and browsers, which the browser's own storage would not. The file never affects a
//! build.
//!
//! **The file** is a JSON object ([`ViewerSettings`] lists its keys and their defaults):
//!
//! ```json
//! {
//!   "editor": "code",
//!   "editor_command": "myeditor --line {line} --column {column} {file}",
//!   "theme": "dark",
//!   "scroll_to_changes": true,
//!   "open_problems_on_errors": false,
//!   "wrap_lines": true,
//!   "panel_side": "right",
//!   "panel_width": 420,
//!   "panel_height": 300
//! }
//! ```
//!
//! A key left out has its default; a key that is not one of these is kept as it is and
//! otherwise ignored. A [`ViewerFile`] reads the file ([`ViewerFile::read`]) and changes it
//! ([`ViewerFile::change`]): a change writes the keys it names into the file as it is at that
//! moment, and keeps every other key, known or not. A file that cannot be read (it is not a
//! JSON object, or a known key has a value of the wrong kind) is never written: the defaults
//! apply and every change is refused, with the reason, until the file can be read. Without a
//! file (no home folder), the settings live in memory.
//!
//! **The editor command** (`editor_command`) is an editor of the author's own: the command
//! that opens a file at a line and a column ([`EditorCommand`]); an empty one is no command.
//! A page never gives it: a change ([`ViewerFile::change`]) refuses it, since the page that
//! asks for a change shares its origin with the document's own scripts. A change that
//! chooses the editor command ([`CUSTOM`]) while there is none writes
//! [`PLACEHOLDER_COMMAND`], a command that opens nothing, for the author to replace by hand.

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::editor::{CUSTOM, EditorCommand};

/// The key of the editor command ([`ViewerSettings::editor_command`]), which a change never
/// sets.
pub const EDITOR_COMMAND: &str = "editor_command";

/// The editor command that a change choosing [`CUSTOM`] writes when there is none,
/// `echo {file}:{line}:{column}`: it opens nothing, and shows how a command is written.
pub const PLACEHOLDER_COMMAND: &str = "echo {file}:{line}:{column}";

/// The theme of the tools page ([`ViewerSettings::theme`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    /// The system's: light or dark as the system is set (`"system"`).
    #[default]
    System,
    /// Light (`"light"`).
    Light,
    /// Dark (`"dark"`).
    Dark,
}

impl Theme {
    const NAMES: [(&'static str, Theme); 3] = [("system", Theme::System), ("light", Theme::Light), ("dark", Theme::Dark)];

    /// The theme's name in the file: `system`, `light` or `dark`.
    pub fn name(self) -> &'static str {
        Theme::NAMES.iter().find(|(_, theme)| *theme == self).map_or("system", |(name, _)| name)
    }
}

/// The edge of the tools page at which its panel is docked ([`ViewerSettings::panel_side`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelSide {
    /// The right edge (`"right"`).
    #[default]
    Right,
    /// The bottom edge (`"bottom"`).
    Bottom,
}

impl PanelSide {
    const NAMES: [(&'static str, PanelSide); 2] = [("right", PanelSide::Right), ("bottom", PanelSide::Bottom)];

    /// The edge's name in the file: `right` or `bottom`.
    pub fn name(self) -> &'static str {
        PanelSide::NAMES.iter().find(|(_, side)| *side == self).map_or("right", |(name, _)| name)
    }
}

/// The viewer settings: each key of the file, as read, or its default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerSettings {
    /// `editor`: the name of the editor a place opens in, one of
    /// [`EDITORS`](super::editor::EDITORS) or [`CUSTOM`] for the
    /// editor command; `None` (the default) for the editor that
    /// [`Editors::choose`](super::editor::Editors::choose) chooses.
    pub editor: Option<String>,
    /// `editor_command`: the command of an editor of the author's own ([`EditorCommand`]);
    /// `None` by default, and for an empty command.
    pub editor_command: Option<EditorCommand>,
    /// `theme`: the theme of the tools page; [`Theme::System`] by default.
    pub theme: Theme,
    /// `scroll_to_changes`: whether the page scrolls to what a build changed and flashes it;
    /// on by default.
    pub scroll_to_changes: bool,
    /// `open_problems_on_errors`: whether the tools page opens its panel on the *Problems*
    /// tab when a build has at least one error and the panel is closed; off by default.
    pub open_problems_on_errors: bool,
    /// `wrap_lines`: whether a text output wraps its long lines; on by default.
    pub wrap_lines: bool,
    /// `panel_side`: the edge at which the panel is docked; [`PanelSide::Right`] by default.
    pub panel_side: PanelSide,
    /// `panel_width`: the panel's width at the right edge, in pixels; `None` (the default)
    /// for the page's own.
    pub panel_width: Option<u32>,
    /// `panel_height`: the panel's height at the bottom edge, in pixels; `None` (the
    /// default) for the page's own.
    pub panel_height: Option<u32>,
}

impl Default for ViewerSettings {
    fn default() -> ViewerSettings {
        ViewerSettings {
            editor: None,
            editor_command: None,
            theme: Theme::System,
            scroll_to_changes: true,
            open_problems_on_errors: false,
            wrap_lines: true,
            panel_side: PanelSide::Right,
            panel_width: None,
            panel_height: None,
        }
    }
}

/// The keys of the file, in the order the file and the pages list them.
const KEYS: [&str; 9] = [
    "editor",
    EDITOR_COMMAND,
    "theme",
    "scroll_to_changes",
    "open_problems_on_errors",
    "wrap_lines",
    "panel_side",
    "panel_width",
    "panel_height",
];

impl ViewerSettings {
    /// The settings that the JSON object `object` writes, each key left out at its default
    /// and every key that is not a viewer setting ignored.
    ///
    /// # Errors
    ///
    /// The reason, in one line, when a key of the settings has a value of the wrong kind.
    pub fn of_json(object: &Map<String, Value>) -> Result<ViewerSettings, String> {
        let mut settings = ViewerSettings::default();
        for (key, value) in object {
            settings.set(key, value)?;
        }
        Ok(settings)
    }

    /// Sets the key `key` to `value`; a key that is not a viewer setting changes nothing.
    fn set(&mut self, key: &str, value: &Value) -> Result<(), String> {
        let written = match value {
            Value::String(text) => text.clone(),
            _ => value.to_string(),
        };
        let invalid = |accepted: &str| Err(format!("Invalid value ‘{written}’ of viewer setting ‘{key}’ ({accepted})"));
        match key {
            "editor" => match value {
                Value::Null => self.editor = None,
                Value::String(name) => self.editor = Some(name.clone()),
                _ => return invalid("an editor’s name"),
            },
            EDITOR_COMMAND => match value {
                Value::Null => self.editor_command = None,
                Value::String(text) if text.trim().is_empty() => self.editor_command = None,
                Value::String(text) => {
                    self.editor_command = Some(EditorCommand::parse(text).map_err(|error| error.to_string())?);
                }
                _ => return invalid("a command"),
            },
            "theme" => match value.as_str().and_then(|name| Theme::NAMES.iter().find(|(known, _)| *known == name)) {
                Some((_, theme)) => self.theme = *theme,
                None => return invalid("‘system’, ‘light’ or ‘dark’"),
            },
            "scroll_to_changes" | "open_problems_on_errors" | "wrap_lines" => {
                let Some(on) = value.as_bool() else {
                    return invalid("true or false");
                };
                match key {
                    "scroll_to_changes" => self.scroll_to_changes = on,
                    "open_problems_on_errors" => self.open_problems_on_errors = on,
                    _ => self.wrap_lines = on,
                }
            }
            "panel_side" => match value.as_str().and_then(|name| PanelSide::NAMES.iter().find(|(known, _)| *known == name)) {
                Some((_, side)) => self.panel_side = *side,
                None => return invalid("‘right’ or ‘bottom’"),
            },
            "panel_width" | "panel_height" => {
                let size = match value {
                    Value::Null => None,
                    _ => match value.as_u64().and_then(|size| u32::try_from(size).ok()) {
                        Some(size) => Some(size),
                        None => return invalid("a whole number of pixels"),
                    },
                };
                if key == "panel_width" {
                    self.panel_width = size;
                } else {
                    self.panel_height = size;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The settings as the preview's pages read them, every key but the editor command
    /// (`editor` is left to the caller, which knows the editor in effect):
    /// `{"theme", "scroll_to_changes", "open_problems_on_errors", "wrap_lines",
    /// "panel_side", "panel_width", "panel_height"}`, a size `null` when it is the page's own.
    pub fn to_json(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert(String::from("theme"), json!(self.theme.name()));
        fields.insert(String::from("scroll_to_changes"), json!(self.scroll_to_changes));
        fields.insert(String::from("open_problems_on_errors"), json!(self.open_problems_on_errors));
        fields.insert(String::from("wrap_lines"), json!(self.wrap_lines));
        fields.insert(String::from("panel_side"), json!(self.panel_side.name()));
        fields.insert(String::from("panel_width"), json!(self.panel_width));
        fields.insert(String::from("panel_height"), json!(self.panel_height));
        fields
    }
}

/// Why a change of the viewer settings was not made ([`ViewerFile::change`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerError {
    /// The change is not one a page may make: it names the editor command, a key that is
    /// not a viewer setting, or a value of the wrong kind. The text says which, in one line.
    Refused(String),
    /// The file cannot be read: nothing is written until it can be.
    Unreadable {
        /// The file.
        path: PathBuf,
        /// Why, in one line.
        reason: String,
    },
    /// The file cannot be written.
    Unwritable {
        /// The file.
        path: PathBuf,
        /// What the system said.
        reason: String,
    },
}

impl fmt::Display for ViewerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewerError::Refused(reason) => f.write_str(reason),
            ViewerError::Unreadable { path, reason } => {
                write!(f, "Unreadable viewer settings file ‘{}’: {reason}", path.display())
            }
            ViewerError::Unwritable { path, reason } => {
                write!(f, "Failed to write viewer settings file ‘{}’: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for ViewerError {}

/// The viewer settings and the file that holds them: see the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerFile {
    /// The file; `None` for settings in memory alone.
    path: Option<PathBuf>,
    /// The settings in effect.
    settings: ViewerSettings,
    /// Why the file cannot be read, when it cannot.
    unreadable: Option<ViewerError>,
}

impl ViewerFile {
    /// Reads the viewer settings from the file at `path`, as the system writes paths
    /// ([`path_in`] gives it from the home folder), or holds them in memory when `path` is
    /// `None`. A file that does not exist gives the defaults, and a change creates it. A file
    /// that cannot be read gives the defaults too, and [`ViewerFile::unreadable`] says why.
    pub fn read(path: Option<PathBuf>) -> ViewerFile {
        let Some(path) = path else {
            return ViewerFile::in_memory();
        };
        let (settings, unreadable) = match read_object(&path).and_then(|object| settings_of(&path, &object)) {
            Ok(settings) => (settings, None),
            Err(error) => (ViewerSettings::default(), Some(error)),
        };
        ViewerFile { path: Some(path), settings, unreadable }
    }

    /// The default settings, in memory alone: a change changes them and writes nothing.
    pub fn in_memory() -> ViewerFile {
        ViewerFile { path: None, settings: ViewerSettings::default(), unreadable: None }
    }

    /// The file, as the system writes paths; `None` for settings in memory alone.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The settings in effect.
    pub fn settings(&self) -> &ViewerSettings {
        &self.settings
    }

    /// Why the file cannot be read ([`ViewerError::Unreadable`]); `None` when it can, or does
    /// not exist.
    pub fn unreadable(&self) -> Option<&ViewerError> {
        self.unreadable.as_ref()
    }

    /// Changes the viewer settings by `changes`, a JSON object of the keys to change and
    /// their new values (`null` for a key's default), and writes them into the file. The
    /// file is read again as it is at this moment, the keys of `changes` are set in it, and
    /// every other key is kept, known or not; the settings in effect become those of the
    /// file written. The folder of the file is made when it does not exist.
    ///
    /// The key `editor` must name one of `editors` (the editors a place can be opened in).
    /// When it names [`CUSTOM`] and the file has no editor command, [`PLACEHOLDER_COMMAND`]
    /// is written as the editor command.
    ///
    /// # Errors
    ///
    /// [`ViewerError::Refused`] for a change that names the editor command, a key that is
    /// not a viewer setting, an editor not among `editors` or a value of the wrong kind;
    /// [`ViewerError::Unreadable`] when the file cannot be read (it is then left as it is);
    /// [`ViewerError::Unwritable`] when it cannot be written. Nothing is changed in any of
    /// these cases.
    pub fn change(&mut self, changes: &Map<String, Value>, editors: &[&str]) -> Result<(), ViewerError> {
        refusal(changes, editors).map_or(Ok(()), |reason| Err(ViewerError::Refused(reason)))?;
        let Some(path) = self.path.clone() else {
            let mut settings = self.settings.clone();
            for (key, value) in changes {
                settings.set(key, value).map_err(ViewerError::Refused)?;
            }
            if chooses_custom(changes) && settings.editor_command.is_none() {
                settings.set(EDITOR_COMMAND, &json!(PLACEHOLDER_COMMAND)).map_err(ViewerError::Refused)?;
            }
            self.settings = settings;
            return Ok(());
        };
        let mut object = match read_object(&path) {
            Ok(object) => object,
            Err(error) => {
                self.unreadable = Some(error.clone());
                return Err(error);
            }
        };
        // The file as it is now must be readable before anything is written into it.
        if let Err(error) = settings_of(&path, &object) {
            self.unreadable = Some(error.clone());
            return Err(error);
        }
        for (key, value) in changes {
            object.insert(key.clone(), value.clone());
        }
        let mut settings = ViewerSettings::of_json(&object).map_err(ViewerError::Refused)?;
        if chooses_custom(changes) && settings.editor_command.is_none() {
            object.insert(String::from(EDITOR_COMMAND), json!(PLACEHOLDER_COMMAND));
            settings = ViewerSettings::of_json(&object).map_err(ViewerError::Refused)?;
        }
        write_object(&path, &object)?;
        self.settings = settings;
        self.unreadable = None;
        Ok(())
    }
}

/// The path of the file of the viewer settings in the home folder `home`:
/// `.flm/watch.json`, beside the folder of the user's extensions (`.flm/extensions`).
pub fn path_in(home: &Path) -> PathBuf {
    home.join(".flm").join("watch.json")
}

/// The reason a page may not make the change `changes`, or `None` when it may: every key
/// must be a viewer setting other than the editor command, with a value of the right kind,
/// and an editor must be one of `editors`.
fn refusal(changes: &Map<String, Value>, editors: &[&str]) -> Option<String> {
    let mut check = ViewerSettings::default();
    for (key, value) in changes {
        if key == EDITOR_COMMAND {
            return Some(format!("Viewer setting ‘{EDITOR_COMMAND}’ may be written only in the file"));
        }
        if !KEYS.contains(&key.as_str()) {
            return Some(format!("Unknown viewer setting ‘{key}’ (viewer settings: ‘{}’)", page_keys().join("’, ‘")));
        }
        if key == "editor"
            && let Some(name) = value.as_str()
            && !editors.contains(&name)
        {
            let listed = if editors.is_empty() { String::from("none") } else { format!("‘{}’", editors.join("’, ‘")) };
            return Some(format!("Unknown editor ‘{name}’ (editors: {listed})"));
        }
        if let Err(reason) = check.set(key, value) {
            return Some(reason);
        }
    }
    None
}

/// Whether `changes` choose the editor command: their `editor` is [`CUSTOM`].
fn chooses_custom(changes: &Map<String, Value>) -> bool {
    changes.get("editor").and_then(Value::as_str) == Some(CUSTOM)
}

/// The keys a page may change: every key but the editor command.
fn page_keys() -> Vec<&'static str> {
    KEYS.iter().copied().filter(|key| *key != EDITOR_COMMAND).collect()
}

/// The JSON object of the file at `path`: an empty object when the file does not exist.
fn read_object(path: &Path) -> Result<Map<String, Value>, ViewerError> {
    let unreadable = |reason: String| ViewerError::Unreadable { path: path.to_owned(), reason };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(unreadable(error.to_string())),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(object)) => Ok(object),
        Ok(_) => Err(unreadable(String::from("the file must hold a JSON object"))),
        Err(error) => Err(unreadable(format!("invalid JSON ({error})"))),
    }
}

/// The settings that the object `object` of the file at `path` writes.
fn settings_of(path: &Path, object: &Map<String, Value>) -> Result<ViewerSettings, ViewerError> {
    ViewerSettings::of_json(object).map_err(|reason| ViewerError::Unreadable { path: path.to_owned(), reason })
}

/// Writes `object` into the file at `path`, as JSON over several lines, making its folder
/// when it does not exist. The text is written into a file beside it first, which then
/// replaces it, so that the file is never left written in part.
fn write_object(path: &Path, object: &Map<String, Value>) -> Result<(), ViewerError> {
    let unwritable = |error: std::io::Error| ViewerError::Unwritable { path: path.to_owned(), reason: error.to_string() };
    let mut text = serde_json::to_string_pretty(object).unwrap_or_default();
    text.push('\n');
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(unwritable)?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(format!(".{}.part", std::process::id()));
    let part = PathBuf::from(part);
    let written = std::fs::write(&part, text).and_then(|()| std::fs::rename(&part, path));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&part);
        return Err(unwritable(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own for the test `name`, empty, under the system's temporary folder.
    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("flm-viewer-{}", std::process::id())).join(name);
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(object) => object,
            _ => panic!("an object"),
        }
    }

    const EDITORS: [&str; 2] = ["code", "custom"];

    #[test]
    fn a_write_keeps_the_keys_it_does_not_know_and_those_it_does_not_name() {
        let home = folder("unknown_keys");
        let path = path_in(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = r#"{"future": {"a": [1, 2]}, "wrap_lines": false, "editor_command": "ed +{line} {file}"}"#;
        std::fs::write(&path, text).unwrap();
        let mut file = ViewerFile::read(Some(path.clone()));
        assert!(!file.settings().wrap_lines);
        file.change(&object(json!({ "scroll_to_changes": false, "editor": "custom" })), &EDITORS).unwrap();
        let written: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({
                "future": {"a": [1, 2]},
                "wrap_lines": false,
                "editor_command": "ed +{line} {file}",
                "scroll_to_changes": false,
                "editor": "custom",
            })
        );
        assert_eq!(file.settings().editor.as_deref(), Some("custom"));
        assert!(!file.settings().scroll_to_changes);
    }

    #[test]
    fn choosing_the_editor_command_when_there_is_none_writes_the_placeholder() {
        let home = folder("placeholder");
        let path = path_in(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // No editor command, then an empty one. (An editor command already there is kept:
        // `a_write_keeps_the_keys_it_does_not_know_and_those_it_does_not_name`.)
        for text in ["{}", r#"{"editor_command": ""}"#] {
            std::fs::write(&path, text).unwrap();
            let mut file = ViewerFile::read(Some(path.clone()));
            file.change(&object(json!({ "editor": "custom" })), &EDITORS).unwrap();
            let written: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(written, json!({ "editor": "custom", "editor_command": PLACEHOLDER_COMMAND }), "{text}");
            assert_eq!(file.settings().editor_command.as_ref().map(EditorCommand::text), Some(PLACEHOLDER_COMMAND));
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_is_never_overwritten() {
        let home = folder("unreadable");
        let path = path_in(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for text in ["{\"theme\": \"dark\",", "[1, 2]", "{\"theme\": \"blue\"}", "{\"editor_command\": \"ed '{file}\"}"] {
            std::fs::write(&path, text).unwrap();
            let mut file = ViewerFile::read(Some(path.clone()));
            assert_eq!(file.settings(), &ViewerSettings::default(), "{text}");
            assert!(matches!(file.unreadable(), Some(ViewerError::Unreadable { .. })), "{text}");
            let refused = file.change(&object(json!({ "wrap_lines": false })), &EDITORS);
            assert!(matches!(refused, Err(ViewerError::Unreadable { .. })), "{text}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
            assert!(file.settings().wrap_lines, "{text}");
        }
        // Once the file can be read again, a change is written.
        std::fs::write(&path, "{\"theme\": \"light\"}").unwrap();
        let mut file = ViewerFile::read(Some(path.clone()));
        std::fs::write(&path, "{\"theme\": ").unwrap();
        assert!(file.change(&object(json!({ "wrap_lines": false })), &EDITORS).is_err());
        assert!(file.unreadable().is_some());
        std::fs::write(&path, "{\"theme\": \"light\"}").unwrap();
        file.change(&object(json!({ "wrap_lines": false })), &EDITORS).unwrap();
        assert_eq!(file.unreadable(), None);
        assert_eq!(file.settings().theme, Theme::Light);
        assert!(!file.settings().wrap_lines);
    }

    #[test]
    fn a_change_never_sets_the_editor_command_an_unknown_key_or_a_wrong_value() {
        let home = folder("refused");
        let path = path_in(&home);
        let mut file = ViewerFile::read(Some(path.clone()));
        for change in [
            json!({ "editor_command": "sh -c {file}" }),
            json!({ "future": 1 }),
            json!({ "theme": "blue" }),
            json!({ "wrap_lines": "yes" }),
            json!({ "panel_width": -3 }),
            json!({ "editor": "nano" }),
            json!({ "theme": "dark", "editor_command": null }),
        ] {
            let refused = file.change(&object(change.clone()), &EDITORS);
            assert!(matches!(refused, Err(ViewerError::Refused(_))), "{change}");
        }
        assert!(!path.exists(), "nothing was written");
        assert_eq!(file.settings(), &ViewerSettings::default());
    }
}
