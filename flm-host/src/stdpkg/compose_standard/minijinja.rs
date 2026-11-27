//! The template engine that the feature `compose-standard` contributes for the package
//! `std`: [`MinijinjaEngine`] (`minijinja`).
//!
//! A template engine is the code that applies a compose template written in its language
//! to what the compose template reads ([`TemplateEngine`]); a compose template refers to
//! its template engine by name.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use minijinja::syntax::SyntaxConfig;
use minijinja::value::{Kwargs, Serde, Value as JinjaValue};
use minijinja::{AutoEscape, Environment, ErrorKind, State, UndefinedBehavior};
use serde_json::Value;

use flm_core::core::artifact::media_type_of;
use flm_core::stdlib::features::compose::{ShippedFile, Template, TemplateContext, TemplateEngine, TemplateShipper};
use flm_ext::paths::{QualifiedPath, Root, WrittenPath};

use crate::roots::{Roots, RootsFile};

/// The template engine `minijinja`: it applies a compose template written in the template
/// language of the `minijinja` crate, which is close to Jinja's.
///
/// The compose template reads what the [documentation of the feature
/// `compose`](flm_core::stdlib::features::compose#what-a-compose-template-reads) lists. The engine applies it with
/// these rules:
///
/// - on HTML, every value that the compose template writes is escaped for HTML, except a
///   region and a piece's text, which are the backend's own HTML;
/// - a value that nothing provides is an error, not an empty text;
/// - a line that holds only a statement leaves no line in the result, and the compose
///   template's last line break is kept;
/// - a compose template whose output itself uses the usual delimiters states others
///   ([`Delimiters`](flm_core::stdlib::features::compose::Delimiters));
/// - the value `artifacts` maps the name of each file the compose template ships to the
///   file's path (`{{ artifacts.style }}`), as plain text;
/// - a shipped file that the compose template applies as a template is applied by the
///   same rules, with the same delimiters
///   ([`TemplateEngine::render_text`], whose default serves this engine);
/// - the filter `tojson` writes a value as JSON (`{{ config | tojson }}`);
/// - the filter `escape_for` writes a text for its place in the page (see
///   [below](#writing-a-text-inside-a-style-a-script-or-a-comment));
/// - the function `include_artifact_url` ships a file and gives its path (see
///   [below](#shipping-a-file)).
///
/// # Reading a file
///
/// The functions `include_raw`, `include` and `include_artifact_url` read a file when the
/// compose template is applied, and so do the statements `include`, `extends` and
/// `import`. Each takes the path of the file with its root ([`flm_ext::paths`]), and
/// reads it through the roots of the preparation ([`Roots`]):
///
/// - `ext+NAME:PATH`, a file of the extension that the use name `NAME` finds, the path
///   relative to the extension's folder (`ext+basetemplate:templates/html/base/base.html`);
/// - `std:PATH`, a file of the program's own files (`std:templates/html/simple/simple.html`);
/// - `doc:PATH`, a file of the document's folder (the folder of the input file; the current
///   folder for a text given as the input), by a path read as a path written in the
///   document is: relative, absolute, `~`, a `..` that climbs. An absolute path is a path
///   of the document's folder (`/home/me/notes.css` is `doc:/home/me/notes.css`);
/// - `cwd:PATH`, a file of the folder in which the program runs;
/// - `https://…`, a file at an address on the network, downloaded through the program's
///   downloads and kept in its cache ([`crate::download`]), for `include_raw` and
///   `include_artifact_url` only;
/// - a *bare path*, with no root, is relative to the folder of the file that holds the
///   call or the statement: the compose template's text, a partial it includes, a file it
///   ships and applies as a template, a file of another package that it extends. `self:PATH`
///   is relative to the base folder of that file's root, the folder of its extension or the
///   program's own files.
///
/// Each text applied has its place, the qualified path of the file it was read from
/// ([`Template::source`], the field `source` of
/// [`TemplateArtifact::File`](flm_core::stdlib::features::compose::TemplateArtifact::File)),
/// and a file that the function `include` or a statement applies has its own place. A text
/// given to `eval` has no place: a bare path or `self:` inside it fails the application,
/// and the text names its files with `doc:`, `ext+NAME:` or `std:`.
///
/// A read of the document's folder and of an extension found in a folder goes through the
/// file reader of the build under way, so the file is recorded among the files the build
/// read, and the live preview builds again when it changes. A file that is not there fails
/// the application; `include_raw` and `include` also fail on a file that is not UTF-8
/// text. A message about a file gives it by its qualified path. The statements and
/// `include` never read an address `https:`, and such a path fails the application, so
/// that a path never makes the engine apply a downloaded text as a template. A template
/// that passes a downloaded text to `eval`, as in
/// `{{ eval(include_raw("https://example.org/part.html")) }}`, applies it: its author
/// wrote that combination. A download that gives no file fails the application; the use
/// of a kept copy of a download past its expiry is a warning ([`TemplateShipper::warn`]).
///
/// - `{{ include_raw("dist/script.js") }}` gives the file's text as it is, never escaped,
///   so that nothing in it is read as a statement or a value;
///   `{{ include_raw("https://example.org/fonts.css") }}` gives the downloaded text.
/// - `{{ include("doc:footer.html", html=true, NAME=VALUE, ...) }}` applies the file as a
///   template of this engine, by the same rules and with the same delimiters, and gives the
///   result as it is.
/// - `{{ eval(config.footer, html=true, NAME=VALUE, ...) }}` applies the text `config.footer`
///   the same way.
///
/// The statement `include` (`{% include "partials/head.html" %}`) applies the file it
/// names as a template of this engine, by the same rules, with the same values. The
/// statements `extends` and `import`, and an `include` of a list of files to try in turn,
/// read their files the same way; a file one of them cannot find is reported as a missing
/// file, naming the last file looked for. A compose template of one extension extends a
/// text of another: `{% extends "ext+basetemplate:templates/html/base/base.html" %}`.
/// The statement `import` gives the variables that the imported file sets (`{% import
/// "partials/colors.html" as colors %}`, then `{{ colors.accent }}`). The engine is built
/// without the statements `macro` and `call`, so a compose template cannot define a
/// macro.
///
/// # The inner templates of `include` and `eval`
///
/// A call of `include` or `eval` stands in an *outer text* and applies an *inner
/// template*. The inner template reads:
///
/// - the values of the application: `regions`, `pieces`, `metadata`, `config` and
///   `artifacts`;
/// - the named values of the call (`NAME=VALUE`);
/// - the named values given to its outer text, when that text is itself an inner
///   template: a named value is passed on to the inner templates below.
///
/// A named value of the call takes precedence over a value of the same name given to the
/// outer text, and both over a value of the application. The inner template does not read
/// a variable that its outer text set for itself (with `set`, or as a loop variable). The name `html` is the argument's own and cannot be passed as a value. The
/// engine's functions and filters are available in an inner template.
///
/// The argument `html` says whether the inner template is HTML: with `html=true`, every
/// value it writes is escaped for HTML; with `html=false`, every value is written as it
/// is. Without `html`, the inner template is taken as its outer text is. A stylesheet
/// applied inside an HTML page is given `html=false`.
///
/// A failure in an inner template fails the application. Its report gives the place of
/// the call in the outer text, the inner template (by its qualified path for `include`,
/// the text given for `eval`), then the failure with its line in the inner template. Inner
/// templates nest to at most eight levels.
///
/// # Shipping a file
///
/// `{{ include_artifact_url("doc:logo.png", media_type="image/png") }}` reads the bytes
/// of a file, by its path as `include_raw` does, ships it beside
/// the output's main file ([`TemplateShipper`]), and gives the file's path in the output,
/// as plain text, as `artifacts` gives the paths of the description's files. The file need
/// not be text. The file's name in the output begins with the stem of its name, and its
/// extension is the one of its name; the name of a file at an address is the last part of
/// the address's path, without the query (`font.woff2` for
/// `https://example.org/a/font.woff2?v=2`). Its media type is the argument `media_type`,
/// which is optional, or else the one the extension gives ([`media_type_of`]), or else, for
/// an address, the one the server states. A file whose name has no extension fails the
/// application, and so does a file with no media type by these rules.
///
/// A file shipped twice in one application is written once, under one path. The function
/// works in the compose template's text, in a shipped file applied as a template and in an
/// inner template.
///
/// # Writing a text inside a style, a script or a comment
///
/// Inside a `<style>` or `<script>` element and inside a comment, HTML reads no character
/// entity, so a text written there is not escaped for HTML. The filter `escape_for` takes
/// any value, reads it as the text the engine would write, and gives that text encoded for
/// its place, as it is written:
///
/// - `{{ css | escape_for("html-style") }}`, for a stylesheet inside a `<style>` element:
///   every `</` is written `<\/`. A stylesheet reads `\/` as `/`, and `</` can stand only in
///   a string, a comment or an address, so the stylesheet means the same and cannot end the
///   element;
/// - `{{ code | escape_for("html-script") }}`, for a script inside a `<script>` element:
///   `<!--`, `<script` and `</script`, in any letter case, get their `<` written `\x3C`;
/// - `{{ title | escape_for("html-comment") }}`, for a text inside a comment: a space is
///   written between two adjacent hyphens (`--` gives `- -`), and a text that begins with
///   `-` or `>` gets a space before it. The text then cannot end the comment, wherever it is
///   placed, beside another encoded text included. The text is altered: HTML has no
///   faithful form for it.
///
/// Another place fails the application. The first two places are for a whole stylesheet
/// or script; a value placed inside a script is written with `tojson`.
#[derive(Clone, Debug)]
pub struct MinijinjaEngine {
    /// The reader of the qualified paths of the preparation.
    roots: Roots,
}

impl MinijinjaEngine {
    /// Returns the engine that reads the files a compose template includes and ships
    /// through `roots`, the reader of the qualified paths of the preparation.
    pub fn new(roots: Roots) -> MinijinjaEngine {
        MinijinjaEngine { roots }
    }
}

/// The name of [`MinijinjaEngine`], `minijinja`.
pub const MINIJINJA: &str = "minijinja";

/// The name of a text given to `eval`, which has no place.
const EVAL: &str = "eval";

/// The most inner templates of `include` and `eval` nested in one another.
const MOST_NESTED: usize = 8;

/// The part of the engine's recursion limit that one level of inner template takes from
/// the levels below it: each inner template is applied with the recursion limit of its
/// outer text less this part, which bounds the statements `include` of each level.
const NESTED_COST: usize = 50;

impl TemplateEngine for MinijinjaEngine {
    fn name(&self) -> &str {
        MINIJINJA
    }

    fn render(
        &self,
        template: &Template,
        context: &TemplateContext,
        shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<String, String> {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        let html = context.html;
        env.set_auto_escape_callback(move |_| escaping(html));
        let mut syntax = SyntaxConfig::builder();
        syntax.trim_blocks(true).lstrip_blocks(true).keep_trailing_newline(true);
        if let Some(delimiters) = &template.delimiters {
            syntax
                .block_delimiters(delimiters.block.0.clone(), delimiters.block.1.clone())
                .variable_delimiters(delimiters.variable.0.clone(), delimiters.variable.1.clone())
                .comment_delimiters(delimiters.comment.0.clone(), delimiters.comment.1.clone());
        }
        env.set_syntax(syntax.build().map_err(|error| error.to_string())?);
        let text = |text: &str| {
            if html { JinjaValue::from_safe_string(String::from(text)) } else { JinjaValue::from(text) }
        };
        let regions: Vec<(String, JinjaValue)> =
            context.regions.iter().map(|(name, value)| (name.clone(), text(value))).collect();
        let pieces: Vec<(String, JinjaValue)> = context
            .pieces
            .iter()
            .map(|piece| {
                let value: Vec<(&str, JinjaValue)> = vec![
                    ("text", text(&piece.text)),
                    ("words", JinjaValue::from(piece.words.as_str())),
                    ("place", JinjaValue::from(piece.place.as_str())),
                ];
                (piece.name.clone(), JinjaValue::from_pairs(value))
            })
            .collect();
        let artifacts = context.artifacts.iter().map(|(name, path)| (name.clone(), JinjaValue::from(path.as_str())));
        let values: Vec<(String, JinjaValue)> = vec![
            (String::from("regions"), JinjaValue::from_pairs(regions)),
            (String::from("pieces"), JinjaValue::from_pairs(pieces)),
            (String::from("metadata"), JinjaValue::from(Serde(&context.metadata))),
            (String::from("config"), JinjaValue::from(Serde(Value::Object(context.config.clone())))),
            (String::from("artifacts"), JinjaValue::from_pairs(artifacts)),
        ];
        let includes = Includes {
            template: template.name.clone(),
            roots: self.roots.clone(),
            html,
            shipper: Arc::clone(shipper),
            values: Arc::new(values),
            passed: Arc::default(),
            own: Arc::default(),
            places: Arc::default(),
            joined: Arc::default(),
        };
        // The text is known to the environment by its place, which a bare path in it is
        // read beside; a text with no place by the compose template's name.
        let name = match template.source.as_deref().map(WrittenPath::parse) {
            Some(Ok(WrittenPath::Qualified(place))) => includes.placed(place),
            _ => template.name.clone(),
        };
        let loader = includes.clone();
        env.set_loader(move |name| loader.load(name));
        let joining = includes.clone();
        env.set_path_join_callback(move |written, parent| Cow::Owned(joining.join(written, parent)));
        let raw = includes.clone();
        env.add_function("include_raw", move |state: &State, path: &str, kwargs: Kwargs| raw.include_raw(state, path, &kwargs));
        let included = includes.clone();
        env.add_function("include", move |state: &State, path: &str, kwargs: Kwargs| included.include(state, path, &kwargs));
        let evaluated = includes.clone();
        env.add_function("eval", move |state: &State, text: &str, kwargs: Kwargs| evaluated.eval(state, text, &kwargs));
        let shipping = includes.clone();
        env.add_function("include_artifact_url", move |state: &State, path: &str, kwargs: Kwargs| {
            shipping.include_artifact_url(state, path, &kwargs)
        });
        let encoding = includes.clone();
        env.add_filter("escape_for", move |value: JinjaValue, place: &str| encoding.escape_for(&value, place));
        let describe = |error: &minijinja::Error| describe(error, &includes.own());
        env.add_template_owned(name.clone(), template.text.clone()).map_err(|error| describe(&error))?;
        let compiled = env.get_template(&name).map_err(|error| describe(&error))?;
        compiled.render(JinjaValue::from_pairs(includes.values.iter().cloned())).map_err(|error| describe(&error))
    }
}

/// The escaping of a text that is HTML where `html` says so.
fn escaping(html: bool) -> AutoEscape {
    if html { AutoEscape::Html } else { AutoEscape::None }
}

/// The texts that the engine's own functions wrote for the failures of one application,
/// which [`describe`] writes in place of the engine's own wording of those failures.
#[derive(Debug, Default, Clone)]
struct Own {
    /// The texts of the failures that the functions returned.
    written: Vec<String>,
    /// The text of the last file that the environment's loader did not find, which
    /// [`describe`] writes for the engine's "template not found".
    missing: Option<String>,
}

/// The reads and the inner templates of one application of a compose template: the reads
/// of the statements `include`, `extends` and `import` (the environment's loader, after
/// its join of a path to the text that writes it) and of the functions `include_raw`,
/// `include` and `include_artifact_url`, and the inner templates of `include` and `eval`.
#[derive(Clone)]
struct Includes {
    /// The compose template's name, for the messages.
    template: String,
    /// The reader of the qualified paths of the preparation.
    roots: Roots,
    /// Whether the compose template is HTML: the escaping of its environment.
    html: bool,
    /// The handle through which `include_artifact_url` ships a file.
    shipper: Arc<dyn TemplateShipper>,
    /// The values of the application, which every inner template reads.
    values: Arc<Vec<(String, JinjaValue)>>,
    /// Each inner template being applied, the innermost last.
    passed: Arc<Mutex<Vec<Passed>>>,
    /// The texts of the failures written by the functions.
    own: Arc<Mutex<Own>>,
    /// The place of each text applied that has one, by the name under which the
    /// environment applies it: its place, written `ROOT:PATH`.
    places: Arc<Mutex<BTreeMap<String, QualifiedPath>>>,
    /// The file that each path a statement writes was joined to, by the name that the join
    /// gave the environment (the file's qualified path, or the path as written where it
    /// has no meaning), or the text of why the path has no meaning.
    joined: Arc<Mutex<BTreeMap<String, Result<Found, String>>>>,
}

/// A file that a path written in a text names: its qualified path, and the path to write
/// for a file of the package where the path was written absolute in a file of a package.
#[derive(Debug, Clone)]
struct Found {
    /// The file's qualified path.
    path: QualifiedPath,
    /// `self:PATH`, for an absolute path `/PATH` written in a file of a package.
    own: Option<String>,
}

/// An inner template being applied: the escaping of its environment and the named values
/// passed on to the inner templates below it.
#[derive(Clone, Default)]
struct Passed {
    /// Whether its environment escapes for HTML.
    html: bool,
    /// The named values given to it, and those passed on from its outer text.
    values: BTreeMap<String, JinjaValue>,
}

/// The level of an inner template being applied: dropping it ends the level.
struct Level<'a>(&'a Includes);

impl Drop for Level<'_> {
    fn drop(&mut self) {
        self.0.passed.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).pop();
    }
}

impl Includes {
    /// The texts written so far for the failures of this application.
    fn own(&self) -> Own {
        self.own.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    /// The engine's error of the text `text`, kept among the texts that [`describe`]
    /// writes as they are.
    fn error(&self, text: String) -> minijinja::Error {
        self.own.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).written.push(text.clone());
        minijinja::Error::new(ErrorKind::InvalidOperation, text)
    }

    /// Keeps `place` as the place of the text that the environment applies under its
    /// name, `ROOT:PATH`, and returns that name.
    fn placed(&self, place: QualifiedPath) -> String {
        let name = place.to_string();
        self.places.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(name.clone(), place);
        name
    }

    /// The place of the text that the environment applies under the name `name`, `None`
    /// for a text with no place.
    fn place_of(&self, name: &str) -> Option<QualifiedPath> {
        self.places.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(name).cloned()
    }

    /// The file that the path `written` names, written in the text that the environment
    /// applies under the name `at`, or the text of why the path has no meaning there. An
    /// address `https:` names a file where `reader`, the function that reads it, writes
    /// its text or ships it, and none where the file would be applied as a template
    /// (`reader` is `None` for a statement).
    fn find(&self, written: &str, at: &str, reader: Option<&str>) -> Result<Found, String> {
        let place = self.place_of(at);
        let path = match (WrittenPath::parse(written).map_err(|error| error.to_string())?, &place) {
            (WrittenPath::Qualified(path), _) => path,
            (path, Some(place)) => path.at(place).map_err(|error| error.to_string())?,
            (WrittenPath::Own(rest), None) => {
                return Err(format!(
                    "Invalid root ‘self’ in the path ‘{written}’ {}: write ‘doc:{rest}’, ‘ext+NAME:{rest}’ or ‘std:{rest}’",
                    self.unplaced(at)
                ));
            }
            (WrittenPath::Bare(rest), None) => {
                return Err(format!(
                    "Missing root in the path ‘{written}’ {}: write ‘doc:{rest}’, ‘ext+NAME:{rest}’ or ‘std:{rest}’",
                    self.unplaced(at)
                ));
            }
        };
        // A text from the network is never applied as a template.
        if *path.root() == Root::Https && !matches!(reader, Some("include_raw" | "include_artifact_url")) {
            let applier = reader.map_or(String::from("a statement"), |reader| format!("‘{reader}’"));
            return Err(format!(
                "Invalid path ‘{path}’ for {applier}: a downloaded file may not be applied as a template; use \
                 ‘include_raw(\"{path}\")’ for its text"
            ));
        }
        // An absolute path is read as a path of the document's folder (`doc:` and the path).
        let in_package = place.is_some_and(|place| place.root().is_base_folder());
        let absolute = *path.root() == Root::Doc && written.starts_with('/');
        let own = (in_package && absolute).then(|| format!("self:{}", written.trim_start_matches('/')));
        Ok(Found { path, own })
    }

    /// Where a text with no place stands, the text that the environment applies under the
    /// name `at`, for the messages.
    fn unplaced(&self, at: &str) -> String {
        if at == EVAL {
            String::from("in a text given to ‘eval’")
        } else {
            format!("in compose template ‘{}’ (its text has no file)", self.template)
        }
    }

    /// The file that the path `written` names in the text being applied in `state`, for
    /// the function `function`.
    fn found(&self, state: &State, written: &str, function: &str) -> Result<Found, minijinja::Error> {
        self.find(written, state.name(), Some(function)).map_err(|text| self.error(text))
    }

    /// The environment's join of the path `written` to the text that the environment
    /// applies under the name `parent`: the name under which its loader reads the file, the
    /// file's qualified path, or the path as written where it has no meaning.
    fn join(&self, written: &str, parent: &str) -> String {
        let found = self.find(written, parent, None);
        let name = match &found {
            Ok(found) => found.path.to_string(),
            Err(_) => written.to_string(),
        };
        self.joined.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(name.clone(), found);
        name
    }

    /// The text of the failure of a read of the file `found`, where there is no such file.
    fn missing(&self, found: &Found) -> String {
        match &found.own {
            Some(own) => format!("Missing file ‘{}’; write ‘{own}’ for a file of the package", found.path),
            None => format!("Missing file ‘{}’", found.path),
        }
    }

    /// The environment's loader: the text of the template file under the name `name` that
    /// its join gave, `Ok(None)` when there is no such file (the engine's "template not
    /// found", which `ignore missing` skips).
    fn load(&self, name: &str) -> Result<Option<String>, minijinja::Error> {
        let joined = self.joined.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(name).cloned();
        let found = joined.unwrap_or_else(|| self.find(name, "", None)).map_err(|text| self.error(text))?;
        match self.roots.read(&found.path) {
            Ok(Some(file)) => {
                let text = String::from_utf8(file.bytes).map_err(|_| self.error(not_text(&found)))?;
                self.placed(found.path);
                Ok(Some(text))
            }
            Ok(None) => {
                let missing = self.missing(&found);
                self.own.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).missing = Some(missing);
                Ok(None)
            }
            Err(error) => Err(self.error(error.to_string())),
        }
    }

    /// The file `found`, its warning handed to the shipper.
    fn read_file(&self, found: &Found) -> Result<RootsFile, minijinja::Error> {
        match self.roots.read(&found.path) {
            Ok(Some(mut file)) => {
                if let Some(warning) = file.warning.take() {
                    self.shipper.warn(warning);
                }
                Ok(file)
            }
            Ok(None) => Err(self.error(self.missing(found))),
            Err(error) => Err(self.error(error.to_string())),
        }
    }

    /// The text of the file `found`.
    fn read_text(&self, found: &Found) -> Result<String, minijinja::Error> {
        String::from_utf8(self.read_file(found)?.bytes).map_err(|_| self.error(not_text(found)))
    }

    /// Refuses the argument `scope` of a call of `function` with the path `path`.
    fn no_scope(&self, kwargs: &Kwargs, function: &str, path: &str) -> Result<(), minijinja::Error> {
        if !kwargs.has("scope") {
            return Ok(());
        }
        Err(self.error(match WrittenPath::parse(path) {
            Ok(WrittenPath::Bare(bare)) => format!(
                "Invalid argument ‘scope’ of ‘{function}’: write ‘doc:{bare}’ for a file of the document's folder, \
                 ‘{bare}’ for a file beside this one"
            ),
            _ => format!("Invalid argument ‘scope’ of ‘{function}’: write ‘{function}(\"{path}\")’"),
        }))
    }

    /// The function `include_raw`: the text of the file at `path`, as a safe value.
    fn include_raw(&self, state: &State, path: &str, kwargs: &Kwargs) -> Result<JinjaValue, minijinja::Error> {
        self.no_scope(kwargs, "include_raw", path)?;
        kwargs.assert_all_used()?;
        let found = self.found(state, path, "include_raw")?;
        self.read_text(&found).map(JinjaValue::from_safe_string)
    }

    /// The function `include_artifact_url`: the file at `path`, shipped through the
    /// shipper, by its path in the output, as plain text.
    fn include_artifact_url(&self, state: &State, path: &str, kwargs: &Kwargs) -> Result<JinjaValue, minijinja::Error> {
        self.no_scope(kwargs, "include_artifact_url", path)?;
        let given = kwargs.get::<Option<&str>>("media_type")?;
        kwargs.assert_all_used()?;
        let found = self.found(state, path, "include_artifact_url")?;
        let of = || format!("file ‘{}’", found.path);
        let name = Path::new(found.path.file_name());
        let (Some(stem), Some(extension)) =
            (name.file_stem().and_then(OsStr::to_str), name.extension().and_then(OsStr::to_str))
        else {
            return Err(self.error(format!("Missing extension of {}", of())));
        };
        let known = given.or_else(|| media_type_of(extension)).map(String::from);
        let unknown = || self.error(format!("Unknown media type of {}: give it as ‘media_type=\"…\"’", of()));
        // Only the server of an address states a media type.
        if known.is_none() && *found.path.root() != Root::Https {
            return Err(unknown());
        }
        let file = self.read_file(&found)?;
        let Some(media_type) = known.or(file.media_type) else { return Err(unknown()) };
        let file = ShippedFile {
            stem: String::from(stem),
            extension: String::from(extension),
            media_type,
            bytes: file.bytes,
        };
        match self.shipper.ship(file) {
            Ok(shipped) => Ok(JinjaValue::from(shipped)),
            Err(error) => Err(self.error(format!("Failed to ship {}: {error}", of()))),
        }
    }

    /// The filter `escape_for`: the text of `value` encoded for the place `place`, as a
    /// safe value.
    fn escape_for(&self, value: &JinjaValue, place: &str) -> Result<JinjaValue, minijinja::Error> {
        let text = value.to_string();
        let encoded = match place {
            "html-style" => text.replace("</", "<\\/"),
            "html-script" => escape_for_script(&text),
            "html-comment" => escape_for_comment(&text),
            _ => {
                return Err(self.error(format!(
                    "Invalid place ‘{place}’ of ‘escape_for’ (the places are ‘html-style’, ‘html-script’ and ‘html-comment’)"
                )));
            }
        };
        Ok(JinjaValue::from_safe_string(encoded))
    }

    /// The function `include`: the file at `path` applied as an inner template, at its own
    /// place.
    fn include(&self, state: &State, path: &str, kwargs: &Kwargs) -> Result<JinjaValue, minijinja::Error> {
        self.no_scope(kwargs, "include", path)?;
        let found = self.found(state, path, "include")?;
        let text = self.read_text(&found)?;
        let failed = || format!("Failed to apply file ‘{}’", found.path);
        let name = self.placed(found.path.clone());
        self.apply(state, &name, &text, kwargs, failed)
    }

    /// The function `eval`: `text` applied as an inner template.
    fn eval(&self, state: &State, text: &str, kwargs: &Kwargs) -> Result<JinjaValue, minijinja::Error> {
        if kwargs.has("scope") {
            return Err(self.error(String::from("Invalid argument ‘scope’ of ‘eval’: ‘eval’ takes no scope")));
        }
        let failed = || format!("Failed to apply the text ‘{}’ given to ‘eval’", text.replace('\n', "\\n"));
        self.apply(state, EVAL, text, kwargs, failed)
    }

    /// Applies `text`, an inner template called `name`, with the arguments `kwargs` of its
    /// call in the outer text being applied in `state`, and returns the result as a safe
    /// value. A failure is written as `failed()`, with the inner failure as its cause.
    fn apply(
        &self,
        state: &State,
        name: &str,
        text: &str,
        kwargs: &Kwargs,
        failed: impl Fn() -> String,
    ) -> Result<JinjaValue, minijinja::Error> {
        let outer = !matches!(state.auto_escape(), AutoEscape::None);
        let html = kwargs.get::<Option<bool>>("html")?.unwrap_or(outer);
        let (escaped, mut passed) = {
            let levels = self.passed.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if levels.len() >= MOST_NESTED {
                return Err(self.error(format!("Recursion limit reached ({MOST_NESTED} levels) while applying ‘{name}’")));
            }
            match levels.last() {
                Some(level) => (level.html, level.values.clone()),
                None => (self.html, BTreeMap::new()),
            }
        };
        let names: Vec<String> = kwargs.args().filter(|key| *key != "html").map(String::from).collect();
        for key in names {
            let value = kwargs.get::<JinjaValue>(&key)?;
            passed.insert(key, value);
        }
        let mut values: BTreeMap<String, JinjaValue> = self.values.iter().cloned().collect();
        values.extend(passed.iter().map(|(key, value)| (key.clone(), value.clone())));
        let mut env = state.env().clone();
        // `escaped` is the escaping of the outer text's environment, which a block
        // `autoescape` at the place of the call does not change.
        if html != escaped {
            // The templates the outer text's statements loaded were made with its escaping.
            env.clear_templates();
            env.set_auto_escape_callback(move |_| escaping(html));
        }
        env.set_recursion_limit(state.env().recursion_limit().saturating_sub(NESTED_COST));
        self.passed.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(Passed { html, values: passed });
        let _level = Level(self);
        match env.render_named_str(name, text, JinjaValue::from_pairs(values)) {
            Ok(written) => Ok(JinjaValue::from_safe_string(written)),
            Err(error) => Err(self.error(failed()).with_source(error)),
        }
    }
}

/// The text of the failure of a read of the file `found`, which is not UTF-8 text.
fn not_text(found: &Found) -> String {
    format!("File ‘{}’ must be UTF-8 text", found.path)
}

/// Returns `text` encoded for a script inside a `<script>` element: the `<` of each
/// `<!--`, `<script` and `</script`, in any letter case, written `\x3C`.
fn escape_for_script(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for (index, c) in text.char_indices() {
        let rest = &text.as_bytes()[index..];
        let starts = |opening: &[u8]| rest.len() >= opening.len() && rest[..opening.len()].eq_ignore_ascii_case(opening);
        if c == '<' && (starts(b"<!--") || starts(b"<script") || starts(b"</script")) {
            encoded.push_str("\\x3C");
        } else {
            encoded.push(c);
        }
    }
    encoded
}

/// Returns `text` encoded for the inside of a comment: a space between two adjacent
/// hyphens, and a space before a text that begins with `-` or `>`.
fn escape_for_comment(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len() + 1);
    if text.starts_with(['-', '>']) {
        encoded.push(' ');
    }
    let mut hyphen = false;
    for c in text.chars() {
        if c == '-' && hyphen {
            encoded.push(' ');
        }
        hyphen = c == '-';
        encoded.push(c);
    }
    encoded
}

/// Returns the engine's error as one text: its message, with the line of the template
/// where it occurred, followed by the messages of its causes. A failure that the engine's
/// functions wrote is written in their words (`own`), with its line, and so is the engine's
/// "template not found", as the last file the loader did not find.
fn describe(error: &minijinja::Error, own: &Own) -> String {
    let mut parts = Vec::new();
    let mut next: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(cause) = next {
        let engine = cause.downcast_ref::<minijinja::Error>();
        let written = engine.and_then(|engine| match (engine.kind(), &own.missing) {
            (ErrorKind::TemplateNotFound, Some(missing)) => Some(missing.as_str()),
            _ => engine.detail().filter(|detail| own.written.iter().any(|text| text == detail)),
        });
        let text = match (engine, written) {
            (Some(engine), Some(written)) => match engine.name() {
                Some(name) => format!("{written} (in {name}:{})", engine.line().unwrap_or(0)),
                None => written.to_string(),
            },
            _ => cause.to_string(),
        };
        parts.push(text);
        next = cause.source();
    }
    parts.join(": ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flm_core::stdlib::features::compose::{Accepts, ShippedFile};
    use flm_ext::paths::ExtensionsByName;
    use serde_json::{Map, json};

    use crate::cache::ServiceCaches;
    use crate::download::{Downloader, Downloads, Response};
    use crate::files::{BuildFiles, MemoryFiles, ServiceFiles};

    /// The place of the text of the compose template `t`, a file of the program's own files.
    const PLACE: &str = "std:templates/html/t/t.html";

    /// The program's own files of the tests: the folder of the compose template `t`.
    fn std_files() -> flm_ext::MemoryFiles {
        let at = |path: &str| format!("templates/html/t/{path}");
        flm_ext::MemoryFiles::new()
            .with_text(&at("partials/head.html"), "<h1>{{ config.title }}</h1>{% include \"foot.html\" %}")
            .with_text(&at("partials/foot.html"), "<footer>{{ regions.content }}</footer>")
            .with_text(&at("dist/x.js"), "if (a < b && c) { show(\"{{ regions.content }}\"); }")
            .with_text(&at("vars.css"), ":root { --x: {{ config.title }}; }")
            .with_text(&at("broken.html"), "{{ nothing.here }}")
            .with_text(&at("outer.txt"), "{% set own = 1 %}{{ include(\"inner.txt\", b=\"B\") }}")
            .with_text(&at("leak.txt"), "{% set own = 1 %}{{ eval(\"{{ own }}\") }}")
            .with_text(&at("inner.txt"), "[{{ a }} {{ b }} {{ config.title }} {{ regions.content }}]")
            .with_text(&at("names/note.html"), "{% set note = include_raw(\"note.txt\") %}")
            .with_text(&at("names/note.txt"), "the imported file's note")
            .with_text(&at("note.txt"), "the caller's note")
    }

    /// A handle that ships nothing: the engine ships no file through it.
    struct NoShipping;

    impl TemplateShipper for NoShipping {
        fn warn(&self, detail: String) {
            panic!("the tests report no warning: {detail}");
        }

        fn ship(&self, _file: ShippedFile) -> Result<String, String> {
            Err(String::from("the tests ship no file"))
        }
    }

    /// A handle that keeps the files shipped through it, each once, as the composer's does,
    /// and gives each the path `<stem>-<number>.<extension>`.
    #[derive(Default)]
    struct Shelf(Mutex<Vec<ShippedFile>>);

    impl TemplateShipper for Shelf {
        fn warn(&self, detail: String) {
            panic!("the tests report no warning: {detail}");
        }

        fn ship(&self, file: ShippedFile) -> Result<String, String> {
            let mut shipped = self.0.lock().unwrap();
            let number = match shipped.iter().position(|kept| *kept == file) {
                Some(number) => number,
                None => {
                    shipped.push(file.clone());
                    shipped.len() - 1
                }
            };
            Ok(format!("{}-{number}.{}", file.stem, file.extension))
        }
    }

    /// The engine over the program's own files [`std_files`] and the document's folder
    /// `doc` of the files `files`, read through `reader`.
    fn engine_over(files: MemoryFiles) -> (MinijinjaEngine, ServiceFiles) {
        let reader = ServiceFiles::new(Arc::new(files));
        let roots = Roots::new(ExtensionsByName::new(), Arc::new(std_files()), reader.clone(), String::from("doc"));
        (MinijinjaEngine::new(roots), reader)
    }

    /// A stand-in for the network: it answers `https://example.org/fonts.css` with a
    /// stylesheet and `https://example.org/a/font.woff2?v=2` with the bytes of a font, as
    /// the server states them, and records each address requested.
    #[derive(Default)]
    struct Network(Mutex<Vec<String>>);

    impl Downloader for Network {
        fn get(&self, url: &str) -> Result<Response, String> {
            self.0.lock().unwrap().push(url.to_string());
            let (bytes, media_type): (&[u8], &str) = match url {
                "https://example.org/fonts.css" => (b"@font-face { font-family: X; }", "text/css"),
                "https://example.org/a/font.woff2?v=2" => (b"wOF2\x00\xff", "font/woff2"),
                _ => return Err(String::from("the server answered the status 404 (Not Found)")),
            };
            Ok(Response { bytes: bytes.to_vec(), media_type: Some(String::from(media_type)) })
        }
    }

    /// The engine over the program's own files [`std_files`], whose downloads go to
    /// `network`.
    fn engine_downloading(network: &Arc<Network>) -> MinijinjaEngine {
        let reader = ServiceFiles::new(Arc::new(MemoryFiles::new()));
        let roots = Roots::new(ExtensionsByName::new(), Arc::new(std_files()), reader, String::from("doc"));
        let downloader: Arc<dyn Downloader> = network.clone();
        MinijinjaEngine::new(roots.with_downloads(Downloads::new(downloader), ServiceCaches::default()))
    }

    /// The engine, over an empty document's folder.
    fn engine() -> MinijinjaEngine {
        engine_over(MemoryFiles::new()).0
    }

    /// The handle [`NoShipping`].
    fn shipper() -> Arc<dyn TemplateShipper> {
        Arc::new(NoShipping)
    }

    /// The compose template `t` of the text `text`, at the place [`PLACE`] where `placed`
    /// says so.
    fn template(text: &str, placed: bool) -> Template {
        let mut config = Map::new();
        config.insert(String::from("title"), json!("A <b>&</b> B"));
        Template {
            name: String::from("t"),
            package: String::from("test"),
            output: String::from("html"),
            engine: String::from(MINIJINJA),
            text: String::from(text),
            source: placed.then(|| String::from(PLACE)),
            accepts: Accepts::All,
            requires: Vec::new(),
            config,
            settings: None,
            delimiters: None,
            artifacts: Vec::new(),
        }
    }

    /// What the compose template `template` reads, on HTML where `html` says so.
    fn context(template: &Template, html: bool) -> TemplateContext {
        TemplateContext {
            html,
            regions: vec![(String::from("content"), String::from("<p>Text & more</p>"))],
            pieces: Vec::new(),
            metadata: json!({}),
            config: template.config.clone(),
            artifacts: Vec::new(),
        }
    }

    fn render(text: &str, placed: bool) -> Result<String, String> {
        let template = template(text, placed);
        engine().render(&template, &context(&template, true), &shipper())
    }

    #[test]
    fn a_partial_in_a_subfolder_includes_its_sibling_by_a_bare_path() {
        // `partials/head.html` includes `foot.html`, beside it in `partials/`.
        assert_eq!(
            render("<body>{% include \"partials/head.html\" %}</body>", true).unwrap(),
            "<body><h1>A &lt;b&gt;&amp;&lt;&#x2f;b&gt; B</h1><footer><p>Text & more</p></footer></body>"
        );
        // `ignore missing` skips a file that is not there, as the engine's include does.
        assert_eq!(render("[{% include \"none.html\" ignore missing %}]", true).unwrap(), "[]");
    }

    #[test]
    fn a_text_of_std_reads_a_file_of_std_by_self_and_by_a_bare_path() {
        let raw = "if (a < b && c) { show(\"{{ regions.content }}\"); }";
        let text = "{{ include_raw(\"self:templates/html/t/dist/x.js\") }}|{{ include_raw(\"./dist/x.js\") }}";
        assert_eq!(render(text, true).unwrap(), format!("{raw}|{raw}"));
    }

    #[test]
    fn a_function_called_in_an_imported_file_reads_a_bare_path_beside_that_file() {
        // `names/note.html` reads `note.txt` beside it, not beside `t.html`, which imports it.
        let text = "{% import \"names/note.html\" as names %}{{ names.note }}";
        assert_eq!(render(text, true).unwrap(), "the imported file's note");
    }

    #[test]
    fn a_raw_include_is_written_as_it_is_in_a_page_and_in_a_stylesheet() {
        let raw = "if (a < b && c) { show(\"{{ regions.content }}\"); }";
        assert_eq!(render("<script>{{ include_raw(\"dist/x.js\") }}</script>", true).unwrap(), format!("<script>{raw}</script>"));
        // A shipped file applied as a template, not HTML, at its own place: the same text,
        // and an included template applied with no escaping, as the file itself.
        let template = template("", false);
        let text = "/* {{ include_raw(\"dist/x.js\") }} */\n{% include \"vars.css\" %}";
        let place = Some("std:templates/html/t/style.css");
        let applied = engine().render_text(&template, text, place, &context(&template, false), &shipper()).unwrap();
        assert_eq!(applied, format!("/* {raw} */\n:root {{ --x: A <b>&</b> B; }}"));
    }

    #[test]
    fn the_filter_tojson_writes_a_value_as_json() {
        let template = template("", false);
        let written = engine().render_text(&template, "setup({{ config | tojson }});", None, &context(&template, false), &shipper());
        assert_eq!(written.unwrap(), "setup({\"title\": \"A \\u003cb\\u003e\\u0026\\u003c/b\\u003e B\"});");
    }

    #[test]
    fn an_include_that_cannot_be_read_fails_with_its_condition() {
        // A file that is not there, by its qualified path.
        let missing = render("{% include \"none.html\" %}", true).unwrap_err();
        assert!(missing.contains("std:templates/html/t/none.html"), "{missing}");
        assert!(render("\n{{ include_raw(\"none.js\") }}", true).is_err());
        // A path that climbs above the program's own files.
        let refused = render("{{ include_raw(\"../../../../x.js\") }}", true).unwrap_err();
        assert!(refused.contains("../../../../x.js"), "{refused}");
        let refused = render("{% include \"../../../../x.js\" %}", true).unwrap_err();
        assert!(refused.contains("../../../../x.js"), "{refused}");
        // A compose template whose text has no place reads no bare path.
        assert!(render("{{ include_raw(\"dist/x.js\") }}", false).is_err());
        assert!(render("{% include \"partials/head.html\" %}", false).is_err());
        // A failure inside an included template.
        assert!(render("{% include \"broken.html\" %}", true).is_err());
        // A missing file included from an included template.
        let nested = render("{% include \"partials/lost.html\" ignore missing %}{{ include_raw(\"gone.txt\") }}", true);
        assert!(nested.is_err());
    }

    #[test]
    fn a_file_of_the_document_folder_is_included_raw_and_as_a_template_and_its_read_is_recorded() {
        let files = MemoryFiles::new().with("doc/notes.css", ":root { --t: {{ config.title }}; }");
        let (engine, reader) = engine_over(files.clone());
        let reads = BuildFiles::new(Arc::new(files));
        let _attached = reader.attach(reads.clone());
        let template = template("", false);
        let text = "{{ include_raw(\"doc:notes.css\") }}\n{{ include(\"doc:./notes.css\") }}";
        let applied = engine.render_text(&template, text, None, &context(&template, false), &shipper()).unwrap();
        assert_eq!(applied, ":root { --t: {{ config.title }}; }\n:root { --t: A <b>&</b> B; }");
        let read: Vec<String> = reads.list().into_iter().map(|read| read.path).collect();
        assert_eq!(read, ["doc/notes.css", "doc/notes.css"]);
    }

    #[test]
    fn an_inner_template_reads_the_application_and_the_named_values_and_not_the_outer_variables() {
        // `inner.txt` reads `a`, given to its outer text `outer.txt`, `b`, given by its own
        // call, and the values of the application.
        let applied = render("{% set own = 1 %}{{ include(\"outer.txt\", html=false, a=\"A\") }}", true);
        assert_eq!(applied.unwrap(), "[A B A <b>&</b> B <p>Text & more</p>]");
        // A variable that the outer text set, in the compose template and in an inner
        // template.
        assert!(render("{% set own = 1 %}{{ eval(\"{{ own }}\") }}", true).is_err());
        assert!(render("{{ include(\"leak.txt\") }}", true).is_err());
    }

    #[test]
    fn in_a_page_an_inner_template_escapes_its_values_unless_given_html_false() {
        let applied = render("{{ eval(\"{{ config.title }}\", html=false) }}|{{ eval(\"{{ config.title }}\") }}", true);
        assert_eq!(applied.unwrap(), "A <b>&</b> B|A &lt;b&gt;&amp;&lt;&#x2f;b&gt; B");
        let blocked = "{% autoescape false %}{{ eval(\"{{ config.title }}\", html=false) }}|{{ eval(\"{{ config.title }}\", html=true) }}{% endautoescape %}";
        assert_eq!(render(blocked, true).unwrap(), "A <b>&</b> B|A &lt;b&gt;&amp;&lt;&#x2f;b&gt; B");
    }

    #[test]
    fn a_bare_path_in_a_text_given_to_eval_is_refused() {
        // The message gives the roots to write.
        let gives_the_roots = |refused: &str| ["doc:dist/x.js", "ext+NAME:dist/x.js", "std:dist/x.js"].iter().all(|root| refused.contains(root));
        let refused = render("{{ eval(\"{{ include_raw('dist/x.js') }}\") }}", true).unwrap_err();
        assert!(gives_the_roots(&refused), "{refused}");
        let refused = render("{{ eval(\"{% include 'dist/x.js' %}\") }}", true).unwrap_err();
        assert!(gives_the_roots(&refused), "{refused}");
        let refused = render("{{ eval(\"{{ include_raw('self:dist/x.js') }}\") }}", true).unwrap_err();
        assert!(gives_the_roots(&refused), "{refused}");
        // A path with its root is read.
        let read = render("{{ eval(\"{{ include_raw('std:templates/html/t/vars.css') }}\") }}", true);
        assert_eq!(read.unwrap(), ":root { --x: {{ config.title }}; }");
    }

    #[test]
    fn a_call_with_scope_fails_with_the_form_to_write() {
        let refused = render("{{ include_raw(\"dist/x.js\", scope=\"template\") }}", true).unwrap_err();
        // The message gives the form to write.
        assert!(refused.contains("doc:dist/x.js"), "{refused}");
        assert!(render("{{ include(\"vars.css\", scope=\"document\") }}", true).is_err());
    }

    #[test]
    fn each_place_of_escape_for_encodes_what_would_end_the_element_or_the_comment() {
        let template = template("", false);
        let escape = |place: &str, text: &str| {
            let mut config = Map::new();
            config.insert(String::from("text"), json!(text));
            let context = TemplateContext { config, ..context(&template, true) };
            let applied = format!("{{{{ config.text | escape_for(\"{place}\") }}}}");
            engine().render_text(&template, &applied, None, &context, &shipper()).unwrap()
        };
        assert_eq!(escape("html-style", "a::after { content: \"</style>\"; } </"), "a::after { content: \"<\\/style>\"; } <\\/");
        assert_eq!(escape("html-script", "s = \"</SCRIPT><!-- <script\";"), "s = \"\\x3C/SCRIPT>\\x3C!-- \\x3Cscript\";");
        assert_eq!(escape("html-comment", "-a -- b --- c-->"), " -a - - b - - - c- ->");
        assert_eq!(escape("html-comment", ">"), " >");
        let plain = "a < b && c > d; /* x */ - y";
        for place in ["html-style", "html-script", "html-comment"] {
            assert_eq!(escape(place, plain), plain, "{place}");
        }
    }

    #[test]
    fn include_artifact_url_called_twice_for_a_file_of_the_document_folder_ships_it_once_under_one_path() {
        let files = MemoryFiles::new().with("doc/img/logo.png", "\u{1}PNG");
        let (engine, _) = engine_over(files);
        let template = template("", false);
        let shelf = Arc::new(Shelf::default());
        let shipper: Arc<dyn TemplateShipper> = shelf.clone();
        let text = "<img src=\"{{ include_artifact_url(\"doc:img/logo.png\") }}\">\n\
                    <img src=\"{{ include_artifact_url(\"doc:./img/logo.png\") }}\">";
        let applied = engine.render(&Template { text: String::from(text), ..template.clone() }, &context(&template, true), &shipper);
        assert_eq!(applied.unwrap(), "<img src=\"logo-0.png\">\n<img src=\"logo-0.png\">");
        let shipped = shelf.0.lock().unwrap().clone();
        let logo = ShippedFile {
            stem: String::from("logo"),
            extension: String::from("png"),
            media_type: String::from("image/png"),
            bytes: b"\x01PNG".to_vec(),
        };
        assert_eq!(shipped, [logo]);
    }

    #[test]
    fn include_raw_of_an_address_writes_the_downloaded_text() {
        let network = Arc::new(Network::default());
        let template = template("<style>{{ include_raw(\"https://example.org/fonts.css\") }}</style>", true);
        let applied = engine_downloading(&network).render(&template, &context(&template, true), &shipper());
        assert_eq!(applied.unwrap(), "<style>@font-face { font-family: X; }</style>");
    }

    #[test]
    fn include_artifact_url_of_an_address_called_twice_ships_one_file_under_one_path() {
        let network = Arc::new(Network::default());
        let address = "https://example.org/a/font.woff2?v=2";
        let text = format!("{{{{ include_artifact_url(\"{address}\") }}}}|{{{{ include_artifact_url(\"{address}\") }}}}");
        let template = template(&text, true);
        let shelf = Arc::new(Shelf::default());
        let shipper: Arc<dyn TemplateShipper> = shelf.clone();
        let applied = engine_downloading(&network).render(&template, &context(&template, true), &shipper);
        assert_eq!(applied.unwrap(), "font-0.woff2|font-0.woff2");
        let font = ShippedFile {
            stem: String::from("font"),
            extension: String::from("woff2"),
            media_type: String::from("font/woff2"),
            bytes: b"wOF2\x00\xff".to_vec(),
        };
        assert_eq!(*shelf.0.lock().unwrap(), [font]);
    }

    #[test]
    fn include_of_an_address_is_refused() {
        let network = Arc::new(Network::default());
        for text in ["{{ include(\"https://example.org/fonts.css\") }}", "{% include \"https://example.org/fonts.css\" %}"] {
            let template = template(text, true);
            let refused = engine_downloading(&network).render(&template, &context(&template, true), &shipper());
            assert!(refused.unwrap_err().contains("https://example.org/fonts.css"), "{text}");
        }
        assert!(network.0.lock().unwrap().is_empty());
    }
}
