//! The template engine that the feature `compose-standard` contributes for the package
//! `stdmain`: [`MinijinjaEngine`] (`minijinja`).
//!
//! A template engine is the code that applies a compose template written in its language
//! to what the compose template reads ([`TemplateEngine`]); a compose template refers to
//! its template engine by name.

use std::sync::{Arc, Mutex};

use minijinja::syntax::SyntaxConfig;
use minijinja::value::Value as JinjaValue;
use minijinja::{AutoEscape, Environment, ErrorKind, UndefinedBehavior};
use serde_json::Value;

use flm_core::stdcore::features::compose::{Template, TemplateContext, TemplateEngine, TemplateFiles};

/// The template engine `minijinja`: it applies a compose template written in the template
/// language of the `minijinja` crate, which is close to Jinja's.
///
/// The compose template reads what the [documentation of the feature
/// `compose`](flm_core::stdcore::features::compose#what-a-compose-template-reads) lists. The engine applies it with
/// these rules:
///
/// - on HTML, every value that the compose template writes is escaped for HTML, except a
///   region and a piece's text, which are the backend's own HTML;
/// - a value that nothing provides is an error, not an empty text;
/// - a line that holds only a statement leaves no line in the result, and the compose
///   template's last line break is kept;
/// - a compose template whose output itself uses the usual delimiters states others
///   ([`Delimiters`](flm_core::stdcore::features::compose::Delimiters));
/// - the value `artifacts` maps the name of each file the compose template ships to the
///   file's path (`{{ artifacts.style }}`), as plain text;
/// - a shipped file that the compose template applies as a template is applied by the
///   same rules, with the same delimiters
///   ([`TemplateEngine::render_text`], whose default serves this engine);
/// - the statement `include` (`{% include "partials/head.html" %}`) applies the file it
///   names as a template of this engine, by the same rules, with the same context, and the
///   function `include_raw` (`{{ include_raw("dist/script.js") }}`) gives the text of the
///   file it names as it is, never escaped, so that nothing in it is read as a statement
///   or a value. Both read the file, when the compose template is applied, through the
///   compose template's files ([`Template::files`]), by a path relative to the folder of
///   its description, and both work in a shipped file applied as a template; a file the
///   compose template ships and does not apply is never read this way. Without files
///   ([`Template::files`] is `None`), both fail. The statements `extends` and
///   `import`, and an `include` of a list of files to try in turn, read their files the
///   same way; a file one of them cannot find is reported in the words of `include`,
///   naming the last file looked for;
/// - the filter `tojson` writes a value as JSON (`{{ config | tojson }}`).
#[derive(Debug, Default, Clone, Copy)]
pub struct MinijinjaEngine;

/// The name of [`MinijinjaEngine`], `minijinja`.
pub const MINIJINJA: &str = "minijinja";

impl TemplateEngine for MinijinjaEngine {
    fn name(&self) -> &str {
        MINIJINJA
    }

    fn render(&self, template: &Template, context: &TemplateContext) -> Result<String, String> {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        env.set_keep_trailing_newline(true);
        let html = context.html;
        env.set_auto_escape_callback(move |_| if html { AutoEscape::Html } else { AutoEscape::None });
        if let Some(delimiters) = &template.delimiters {
            let syntax = SyntaxConfig::builder()
                .block_delimiters(delimiters.block.0.clone(), delimiters.block.1.clone())
                .variable_delimiters(delimiters.variable.0.clone(), delimiters.variable.1.clone())
                .comment_delimiters(delimiters.comment.0.clone(), delimiters.comment.1.clone())
                .build()
                .map_err(|error| error.to_string())?;
            env.set_syntax(syntax);
        }
        let includes = Includes { template: template.name.clone(), files: template.files.clone(), own: Arc::default() };
        let loader = includes.clone();
        env.set_loader(move |path| loader.load(path));
        let raw = includes.clone();
        env.add_function("include_raw", move |path: &str| raw.include_raw(path));
        let describe = |error: &minijinja::Error| describe(error, includes.own());
        env.add_template(&template.name, &template.text).map_err(|error| describe(&error))?;
        let compiled = env.get_template(&template.name).map_err(|error| describe(&error))?;
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
                (piece.name.clone(), JinjaValue::from_iter(value))
            })
            .collect();
        let values: Vec<(&str, JinjaValue)> = vec![
            ("regions", JinjaValue::from_iter(regions)),
            ("pieces", JinjaValue::from_iter(pieces)),
            ("meta", JinjaValue::from_serialize(&context.meta)),
            ("config", JinjaValue::from_serialize(Value::Object(context.config.clone()))),
            ("artifacts", JinjaValue::from_iter(context.artifacts.iter().map(|(name, path)| (name.clone(), JinjaValue::from(path.as_str()))))),
        ];
        compiled.render(JinjaValue::from_iter(values)).map_err(|error| describe(&error))
    }
}

/// The includes of one application of a compose template: the reads of the statement
/// `include` (the environment's loader) and of the function `include_raw`, through the
/// compose template's files.
#[derive(Clone)]
struct Includes {
    /// The compose template's name, for the messages.
    template: String,
    /// Its files.
    files: Option<Arc<dyn TemplateFiles>>,
    /// The text of the last include that failed, which [`describe`] writes in place of
    /// the engine's own wording of that failure.
    own: Arc<Mutex<Option<String>>>,
}

impl Includes {
    /// The text of the file at `path`, read through the compose template's files.
    fn read(&self, path: &str) -> Result<Option<String>, String> {
        match &self.files {
            Some(files) => files.read_text(path),
            None => Err(format!(
                "the compose template `{}` includes `{path}`, and it has no files to include",
                self.template
            )),
        }
    }

    /// Keeps `text` as the text of the last include that failed, and returns it.
    fn failed(&self, text: String) -> String {
        *self.own.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(text.clone());
        text
    }

    /// The text of the last include that failed, if any.
    fn own(&self) -> Option<String> {
        self.own.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    /// The failure of an include of `path`, where there is no such file.
    fn missing(&self, path: &str) -> String {
        self.failed(format!(
            "the compose template `{}` includes `{path}`, and there is no such file in the extension",
            self.template
        ))
    }

    /// The environment's loader: the text of the template file at `path`, `Ok(None)` when
    /// there is no such file (the engine's "template not found", which `ignore missing`
    /// skips).
    fn load(&self, path: &str) -> Result<Option<String>, minijinja::Error> {
        match self.read(path) {
            Ok(Some(text)) => Ok(Some(text)),
            Ok(None) => {
                self.missing(path);
                Ok(None)
            }
            Err(text) => Err(minijinja::Error::new(ErrorKind::InvalidOperation, self.failed(text))),
        }
    }

    /// The function `include_raw`: the text of the file at `path`, as a safe value.
    fn include_raw(&self, path: &str) -> Result<JinjaValue, minijinja::Error> {
        match self.read(path) {
            Ok(Some(text)) => Ok(JinjaValue::from_safe_string(text)),
            Ok(None) => Err(minijinja::Error::new(ErrorKind::InvalidOperation, self.missing(path))),
            Err(text) => Err(minijinja::Error::new(ErrorKind::InvalidOperation, self.failed(text))),
        }
    }
}

/// Returns the engine's error as one text: its message, with the line of the compose
/// template where it occurred, followed by the messages of its causes. A failure of an
/// include is written as `own`, the text of the last include that failed, with its line:
/// an error that carries that text, and the engine's "template not found".
fn describe(error: &minijinja::Error, own: Option<String>) -> String {
    let mut parts = Vec::new();
    let mut next: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(cause) = next {
        let engine = cause.downcast_ref::<minijinja::Error>();
        let text = match (engine, &own) {
            (Some(engine), Some(own)) if engine.kind() == ErrorKind::TemplateNotFound || engine.detail() == Some(own) => {
                match engine.name() {
                    Some(name) => format!("{own} (in {name}:{})", engine.line().unwrap_or(0)),
                    None => own.clone(),
                }
            }
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
    use flm_core::stdcore::features::compose::Accepts;
    use serde_json::{Map, json};

    /// The files of the compose template `t`, in memory; a path with a `..` part is
    /// refused as the extension's files refuse a path leaving the extension's folder.
    struct Files;

    impl TemplateFiles for Files {
        fn read_text(&self, path: &str) -> Result<Option<String>, String> {
            if path.split('/').any(|part| part == "..") {
                return Err(format!("the compose template `t` includes `{path}`, which leaves the extension's folder"));
            }
            Ok(match path {
                "partials/head.html" => Some(String::from("<h1>{{ config.title }}</h1>{% include \"partials/foot.html\" %}")),
                "partials/foot.html" => Some(String::from("<footer>{{ regions.content }}</footer>")),
                "dist/x.js" => Some(String::from("if (a < b && c) { show(\"{{ regions.content }}\"); }")),
                "vars.css" => Some(String::from(":root { --x: {{ config.title }}; }")),
                "broken.html" => Some(String::from("{{ nothing.here }}")),
                _ => None,
            })
        }
    }

    /// The compose template `t` of the text `text`, with the files [`Files`] where `files`
    /// says so.
    fn template(text: &str, files: bool) -> Template {
        let mut config = Map::new();
        config.insert(String::from("title"), json!("A <b>&</b> B"));
        Template {
            name: String::from("t"),
            package: String::from("test"),
            output: String::from("html"),
            engine: String::from(MINIJINJA),
            text: String::from(text),
            accepts: Accepts::All,
            requires: Vec::new(),
            config,
            delimiters: None,
            artifacts: Vec::new(),
            files: files.then(|| Arc::new(Files) as Arc<dyn TemplateFiles>),
        }
    }

    /// What the compose template `template` reads, on HTML where `html` says so.
    fn context(template: &Template, html: bool) -> TemplateContext {
        TemplateContext {
            html,
            regions: vec![(String::from("content"), String::from("<p>Text & more</p>"))],
            pieces: Vec::new(),
            meta: json!({}),
            config: template.config.clone(),
            artifacts: Vec::new(),
        }
    }

    fn render(text: &str, files: bool) -> Result<String, String> {
        let template = template(text, files);
        MinijinjaEngine.render(&template, &context(&template, true))
    }

    #[test]
    fn an_included_template_sees_the_context_and_includes_further() {
        assert_eq!(
            render("<body>{% include \"partials/head.html\" %}</body>", true).unwrap(),
            "<body><h1>A &lt;b&gt;&amp;&lt;&#x2f;b&gt; B</h1><footer><p>Text & more</p></footer></body>"
        );
        // `ignore missing` skips a file that is not there, as the engine's include does.
        assert_eq!(render("[{% include \"none.html\" ignore missing %}]", true).unwrap(), "[]");
    }

    #[test]
    fn a_raw_include_is_written_as_it_is_in_a_page_and_in_a_stylesheet() {
        let raw = "if (a < b && c) { show(\"{{ regions.content }}\"); }";
        assert_eq!(render("<script>{{ include_raw(\"dist/x.js\") }}</script>", true).unwrap(), format!("<script>{raw}</script>"));
        // A shipped file applied as a template, not HTML: the same text, and an included
        // template applied with no escaping, as the file itself.
        let template = template("", true);
        let applied = MinijinjaEngine
            .render_text(&template, "/* {{ include_raw(\"dist/x.js\") }} */\n{% include \"vars.css\" %}", &context(&template, false))
            .unwrap();
        assert_eq!(applied, format!("/* {raw} */\n:root {{ --x: A <b>&</b> B; }}"));
    }

    #[test]
    fn the_filter_tojson_writes_a_value_as_json() {
        let template = template("", false);
        let written = MinijinjaEngine.render_text(&template, "setup({{ config | tojson }});", &context(&template, false));
        assert_eq!(written.unwrap(), "setup({\"title\":\"A \\u003cb\\u003e\\u0026\\u003c/b\\u003e B\"});");
    }

    #[test]
    fn an_include_that_cannot_be_read_fails_with_its_condition() {
        let missing = "the compose template `t` includes `none.html`, and there is no such file in the extension";
        assert_eq!(render("{% include \"none.html\" %}", true).unwrap_err(), format!("{missing} (in t:1)"));
        let missing_raw = "the compose template `t` includes `none.js`, and there is no such file in the extension";
        assert_eq!(render("\n{{ include_raw(\"none.js\") }}", true).unwrap_err(), format!("{missing_raw} (in t:2)"));
        let leaves = "the compose template `t` includes `../x.js`, which leaves the extension's folder";
        assert_eq!(render("{{ include_raw(\"../x.js\") }}", true).unwrap_err(), format!("{leaves} (in t:1)"));
        assert_eq!(render("{% include \"../x.html\" %}", true).unwrap_err(), format!(
            "the compose template `t` includes `../x.html`, which leaves the extension's folder (in t:1)"
        ));
        // A compose template without files includes nothing.
        assert!(render("{{ include_raw(\"dist/x.js\") }}", false).is_err());
        assert!(render("{% include \"partials/head.html\" %}", false).is_err());
        // A failure inside an included template: the engine's own text, then the cause.
        let inside = render("{% include \"broken.html\" %}", true).unwrap_err();
        assert!(inside.contains("broken.html") && inside.contains("undefined"), "{inside}");
        // A missing file included from an included template.
        let nested = render("{% include \"partials/lost.html\" ignore missing %}{{ include_raw(\"gone.txt\") }}", true);
        assert_eq!(
            nested.unwrap_err(),
            "the compose template `t` includes `gone.txt`, and there is no such file in the extension (in t:1)"
        );
    }
}
