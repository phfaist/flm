//! The FLM source of a page ([`source`]).

use std::collections::BTreeSet;

use flm_core::core::registry::HOST_FEATURE;
use flm_core::lang::mint::CallableKind;
use serde::Serialize;
use serde_json::{Map, Value};

use super::build::namespace_label;
use super::paths::{self, LANGUAGE_OWNER};
use super::texts::{self, EmbeddedText, TextSource};
use super::{CommandPageView, Node, Page, ProgramExtensions, ProgramPaths, ProgramView, RootView};
use crate::env::{
    ArgumentView, CommandRef, CommandView, ElementValueView, EntityView, Environment, FeatureView, NamespaceView,
    PackageView, ParsingView, ScopedPackageView, SettingsView, StageStatusView, UseSiteView, WorkflowView, split_docs,
};

/// The options of [`source`]. Make one with `SourceOptions::default()`, then set its fields;
/// a later option (a heading level, the addresses of a site) has its place here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SourceOptions {
    /// The documentation texts to write as plain text rather than as FLM: each is escaped
    /// as the prose the generator writes itself, so that FLM reads it as these characters.
    /// A program that renders the pages lists here the texts that do not parse as FLM
    /// (written by an extension, for example), so that the page still renders. A text is
    /// matched whole: a documentation text, or the summary of one (its first paragraph), as
    /// [`texts`](super::texts) lists the texts a page embeds. Empty by default: every
    /// documentation text is written as FLM.
    pub plain_texts: BTreeSet<String>,
}

/// Writes `page` as an FLM document.
///
/// The document begins with a front matter. It gives the page's title, which the document's
/// metadata reads as FLM and a compose template shows (the default template `simple` of the
/// `text` and `html` outputs writes it first). It uses the extension `flmdoc`, whose markup
/// the page is written in, and no math processor (`math: false`), since a page holds no
/// mathematics:
///
/// ```text
/// ---
/// title: "Macro \\textbackslash{}section"
/// flm:
///   use: [flmdoc]
///   math: false
/// ---
/// ```
///
/// Then comes the page's content, written in the constructs of `flmdoc`, so that the
/// documentation structure of the page is the structure of its FLM tree:
///
/// - **Each documented thing** (a feature, a command, a setting, a stage, …) is a
///   `docitem`, with its signature (`\docsignature`) and its parameters (`docparams`, with
///   one `\docparam` each; an environment's body is the parameter `body`).
/// - **Each documentation text** is in the construct that says what it is: its summary (its
///   first paragraph) in `\docsummary`, the rest in `docdetails`, a parameter's text in its
///   `\docparam`, the summary of a listed thing in its `\docentry`. The summary of a page is
///   in `\docsummary` too. Nothing is joined to a text.
/// - **A fact with one value** is a `\docfield`, whose label the page writes; a fact whose
///   value is a list is a `\docfield` holding a `docinlinelist`; a type is a `\docjsontype`
///   holding the minimal form of its JSON Schema.
/// - **A fact composed from structure** (who declared a thing, where a command is available,
///   where a package comes from, a stage's status in the plan, the notes of a parameter, of
///   a listed thing and of an item of an inline list) is a key–value of the construct of
///   the thing it describes, and `flmdoc` writes its words.
/// - **Each list of things** is a `docentries`, holding the title and the introduction of
///   the part of the page that it is; a list of names inside a value or a sentence is a
///   `docinlinelist`. A part of a page that holds items (the plan, its batches) has a
///   heading of its own (`\section*`, `\subsection*`).
/// - **Every link** to another page is `\docref{<path>}{<text>}`.
///
/// Every documentation text of the environment is written as it is, since it is FLM with
/// the backtick of `flmdoc` already; a text of [`SourceOptions::plain_texts`] is written as
/// plain text instead. [`texts`](super::texts) lists these texts, with whose text each is.
/// Every name and value is written as code (`\verbcode`), and every path, form and signature
/// is a raw-text argument of a construct of `flmdoc` (`\docref`, `\docparam`, `docitem`,
/// `\docsignature`, `\docjsontype`): either is written between braces when the braces of
/// the text pair up, else between a character that the text does not hold. The text this
/// module writes itself (the labels of facts, the titles and introductions of lists, the
/// headings and the explanatory paragraphs) has the characters that mean something in FLM
/// escaped (`\%`, `\{`, …). A page's source holds a `\docref` only to a page of the same
/// tree.
pub fn source(page: &Page, options: &SourceOptions) -> String {
    write(page, options).text
}

/// The documentation texts that `page` embeds as FLM, with their sources, as the walk that
/// writes its source records them ([`texts`](super::texts)).
pub(super) fn embedded(page: &Page) -> Vec<EmbeddedText> {
    write(page, &SourceOptions::default()).texts
}

/// Writes `page` with `options`: its source, and the documentation texts it embeds.
fn write<'a>(page: &'a Page, options: &SourceOptions) -> Doc<'a> {
    let SourceOptions { plain_texts } = options;
    let mut doc = Doc { plain: plain_texts.clone(), env: Some(page.tree.environment()), ..Doc::default() };
    doc.front_matter(&page.title);
    match &page.body {
        Node::Index => index(&mut doc, page),
        Node::Root(view) => root(&mut doc, page, view),
        Node::Package(view) => package(&mut doc, page, view),
        Node::Feature(view) => feature(&mut doc, page, view),
        Node::Command(view) => command(&mut doc, view),
        Node::ScopedPackage(view) => scoped_package(&mut doc, view),
        Node::Settings(view) => settings(&mut doc, page, view),
        Node::Parsing(view) => parsing(&mut doc, page, view),
        #[cfg(feature = "flm-host")]
        Node::FrontMatter(view) => front_matter(&mut doc, page, view),
        Node::Workflow(view) => workflow(&mut doc, view),
        Node::Namespace(view) => namespace(&mut doc, page, view),
        Node::Entity(view) => entity(&mut doc, view),
        Node::Program(view) => program(&mut doc, page, view),
        Node::ProgramPaths(view) => program_paths(&mut doc, page, view),
        Node::ProgramExtensions(view) => program_extensions(&mut doc, page, view),
    }
    doc
}

/// The key–values of a construct of `flmdoc`, in the order given: the facts of a
/// `docitem`, a `\docentry`, a `\docparam` or an `\item` of a `docinlinelist`, or the
/// options of a `docentries`.
#[derive(Debug, Clone, Default)]
struct Keys {
    written: Vec<String>,
}

impl Keys {
    /// No key.
    fn new() -> Keys {
        Keys::default()
    }

    /// The key `key` with the content `value` (in FLM), written as a group so that it may
    /// hold a comma; nothing for an empty value.
    fn value(mut self, key: &str, value: &str) -> Keys {
        if !value.is_empty() {
            self.written.push(format!("{key}={{{value}}}"));
        }
        self
    }

    /// The key `key` with the value `word`, one word with no whitespace.
    fn word(mut self, key: &str, word: &str) -> Keys {
        self.written.push(format!("{key}={word}"));
        self
    }

    /// The flag `key`, when `on`.
    fn flag(mut self, key: &str, on: bool) -> Keys {
        if on {
            self.written.push(key.to_string());
        }
        self
    }

    /// The keys as the optional argument of a construct, `[…]`; nothing for no key.
    fn optional(&self) -> String {
        if self.written.is_empty() { String::new() } else { format!("[{}]", self.written.join(", ")) }
    }
}

/// One thing a `docentries` lists: its facts, its name and its summary, in FLM.
struct Entry {
    keys: Keys,
    name: String,
    summary: String,
}

impl Entry {
    /// The thing named `name`, with the summary `summary` and no fact.
    fn new(name: String, summary: String) -> Entry {
        Entry { keys: Keys::new(), name, summary }
    }

    /// The thing named `name`, with the facts `keys` and no summary.
    fn with(keys: Keys, name: String) -> Entry {
        Entry { keys, name, summary: String::new() }
    }
}

/// One parameter a `docparams` lists: its facts, its form, its name, its role, and its
/// text in FLM.
struct Param {
    keys: Keys,
    form: String,
    name: String,
    role: String,
    text: String,
}

/// An FLM document being written, block by block, with the documentation texts to write as
/// plain text ([`SourceOptions::plain_texts`]) and the record of the documentation texts it
/// embeds ([`texts`](super::texts)).
#[derive(Default)]
struct Doc<'a> {
    text: String,
    plain: BTreeSet<String>,
    /// The environment the page describes, which gives the source of each text; `None` in
    /// the unit tests, where every text has the source [`TextSource::Flm`].
    env: Option<&'a Environment>,
    /// The documentation texts embedded so far, each pair of a text and a source once.
    texts: Vec<EmbeddedText>,
}

impl Doc<'_> {
    /// Records `text`, a documentation text or the summary of one that the page embeds as
    /// FLM, with its source: every such text goes through here, by [`Doc::flm`] or
    /// [`Doc::parts`].
    fn record(&mut self, text: &str, source: &TextSource) {
        if !text.is_empty() && !self.texts.iter().any(|known| known.text == text && known.source == *source) {
            self.texts.push(EmbeddedText { text: text.to_string(), source: source.clone() });
        }
    }

    /// The source of `text`, a documentation text that `owner` declares for itself or for
    /// what it declares ([`texts::declared`]).
    fn declared(&self, owner: Option<&str>, text: &str) -> TextSource {
        self.env.map_or(TextSource::Flm, |env| texts::declared(env, owner, text))
    }

    /// The source of `text`, the documentation text of the workflow `name` or its summary
    /// ([`texts::workflow`]).
    fn workflow_source(&self, name: &str, text: &str) -> TextSource {
        self.env.map_or(TextSource::Flm, |env| texts::workflow(env, name, text))
    }

    /// The source of `text`, the documentation text of the element type `name` or its
    /// summary ([`texts::element_type`]).
    fn element_type_source(&self, name: &str, text: &str) -> TextSource {
        self.env.map_or(TextSource::Flm, |env| texts::element_type(env, name, text))
    }

    /// The source of `text`, the summary of the entry `name` of the namespace `namespace`,
    /// whose owner is `owner` ([`texts::entry`]).
    fn entry_source(&self, namespace: &str, name: &str, owner: Option<&str>, text: &str) -> TextSource {
        self.env.map_or(TextSource::Flm, |env| texts::entry(env, namespace, name, owner, text))
    }

    /// `text`, a documentation text or the summary of one, from `source`, as the page writes
    /// it: as it is, since it is FLM, unless it is among the texts to write as plain text,
    /// which is then escaped ([`esc`]). The text is recorded ([`Doc::record`]).
    fn flm(&mut self, text: &str, source: &TextSource) -> String {
        self.record(text, source);
        if self.plain.contains(text) { esc(text) } else { text.to_string() }
    }

    /// The two parts of `docs`, a documentation text from `source` that the page writes
    /// whole: its summary, which the page writes on its own, then the rest; both as plain
    /// text when the text or its summary is among the texts to write so. The text and its
    /// summary are recorded ([`Doc::record`]).
    fn parts(&mut self, docs: &str, source: &TextSource) -> (String, String) {
        let (summary, rest) = split_docs(docs);
        self.record(docs, source);
        self.record(summary, source);
        if self.plain.contains(docs) || self.plain.contains(summary) {
            (esc(summary), esc(rest))
        } else {
            (summary.to_string(), rest.to_string())
        }
    }

    /// The front matter, with the title `title` (plain text), which the document's
    /// metadata reads as FLM, so it is escaped.
    fn front_matter(&mut self, title: &str) {
        let title = serde_json::to_string(&esc(title)).unwrap_or_else(|_| String::from("\"\""));
        self.text.push_str(&format!("---\ntitle: {title}\nflm:\n  use: [flmdoc]\n  math: false\n---\n"));
    }

    /// Appends one block: a paragraph or a construct, followed by a blank line. Nothing for
    /// an empty text.
    fn block(&mut self, flm: &str) {
        let flm = flm.trim();
        if !flm.is_empty() {
            self.text.push_str(flm);
            self.text.push_str("\n\n");
        }
    }

    /// An unnumbered heading of level `level` (1 for a part of the page, 2 for a part of a
    /// part), its title in FLM.
    fn heading(&mut self, level: usize, title: &str) {
        let command = match level {
            1 => "section",
            _ => "subsection",
        };
        self.block(&format!("\\{command}*{{{title}}}"));
    }

    /// The beginning of the `docitem` of the thing of the kind `kind` named `name`, with the
    /// facts `keys`.
    fn begin_item(&mut self, kind: &str, name: &str, keys: Keys) {
        self.block(&format!("\\begin{{docitem}}{}{}{}", keys.optional(), raw(kind), raw(name)));
    }

    fn end_item(&mut self) {
        self.block("\\end{docitem}");
    }

    /// The summary `summary` (in FLM) of a page or of a text, in `\docsummary`; nothing for
    /// an empty summary.
    fn summary(&mut self, summary: &str) {
        if !summary.is_empty() {
            self.block(&format!("\\docsummary{{{summary}}}"));
        }
    }

    /// A documentation text from `source` written whole: its summary in `\docsummary`, then
    /// the rest in `docdetails` ([`Doc::parts`]).
    fn docs(&mut self, docs: &str, source: &TextSource) {
        let (summary, rest) = self.parts(docs, source);
        self.summary(&summary);
        if !rest.trim().is_empty() {
            self.block(&format!("\\begin{{docdetails}}\n{}\n\\end{{docdetails}}", rest.trim()));
        }
    }

    /// A fact with the label `label` and the value `value` (both in FLM), in `\docfield`;
    /// nothing for an empty value.
    fn field(&mut self, label: &str, value: &str) {
        if !value.is_empty() {
            self.block(&format!("\\docfield{{{label}}}{{{value}}}"));
        }
    }

    /// A list of `entries`, in `docentries` with the options `options` (its title, its
    /// introduction, the level of its title); nothing for no entry.
    fn entries(&mut self, options: Keys, entries: &[Entry]) {
        if entries.is_empty() {
            return;
        }
        let mut text = format!("\\begin{{docentries}}{}\n", options.optional());
        for Entry { keys, name, summary } in entries {
            text.push_str(&format!("\\docentry{}{{{name}}}{{{summary}}}\n", keys.optional()));
        }
        text.push_str("\\end{docentries}");
        self.block(&text);
    }

    /// A list of `params` under `title` (`Arguments` when `None`), in `docparams`; nothing
    /// for no parameter.
    fn params(&mut self, title: Option<&str>, params: &[Param]) {
        if params.is_empty() {
            return;
        }
        let mut text = match title {
            Some(title) => format!("\\begin{{docparams}}[{title}]\n"),
            None => String::from("\\begin{docparams}\n"),
        };
        for Param { keys, form, name, role, text: description } in params {
            text.push_str(&format!("\\docparam{}{}{}{}{{{description}}}\n", keys.optional(), raw(form), raw(name), raw(role)));
        }
        text.push_str("\\end{docparams}");
        self.block(&text);
    }
}

/// `items`, each with its facts and its content in FLM, as a `docinlinelist`; nothing for no
/// item.
fn inline_list(items: impl IntoIterator<Item = (Keys, String)>) -> String {
    let mut text = String::new();
    for (keys, item) in items {
        let keys = keys.optional();
        // An item beginning with `[` would be read as the facts of `\item`.
        let separator = if keys.is_empty() && item.starts_with('[') { "{}" } else { "" };
        text.push_str(&format!("\\item{keys} {separator}{item} "));
    }
    if text.is_empty() { text } else { format!("\\begin{{docinlinelist}} {}\\end{{docinlinelist}}", text) }
}

/// `items`, each in FLM and with no fact, as a `docinlinelist` ([`inline_list`]).
fn plain_list(items: impl IntoIterator<Item = String>) -> String {
    inline_list(items.into_iter().map(|item| (Keys::new(), item)))
}

/// The names `names` as code, as a `docinlinelist` ([`inline_list`]).
fn code_list<'a>(names: impl IntoIterator<Item = &'a String>) -> String {
    plain_list(names.into_iter().map(|name| code(name)))
}

/// `text`, written so that FLM reads it as these characters: the characters that mean
/// something in FLM are escaped (`\%`, `\{`, `\textbackslash{}`, …), those that have no
/// escape (`~`, and the backtick of `flmdoc`) are written as verbatim text, and an empty
/// group separates two hyphens in a row, which FLM would show as a dash.
pub(super) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // Two hyphens in a row would be read as a dash: an empty group keeps them apart.
            '-' if out.ends_with('-') => out.push_str("{}-"),
            '\\' => out.push_str("\\textbackslash{}"),
            '{' | '}' | '%' | '$' | '&' | '#' => {
                out.push('\\');
                out.push(c);
            }
            '~' | '`' => out.push_str(&format!("\\verbtext{{{c}}}")),
            _ => out.push(c),
        }
    }
    out
}

/// `text` as inline code: `\verbcode` and the text between delimiters it does not hold.
pub(super) fn code(text: &str) -> String {
    format!("\\verbcode{}", raw(text))
}

/// `text` between delimiters it does not hold, as a verbatim argument (`\verbcode`, and
/// the raw-text arguments of `flmdoc`) reads it: between braces when the braces of the
/// text pair up, else between the first of `+|!=~^` that it does not hold, else between
/// another character it does not hold.
pub(super) fn raw(text: &str) -> String {
    let (open, close) = delimiters(text);
    format!("{open}{text}{close}")
}

fn delimiters(text: &str) -> (char, char) {
    let mut depth: i64 = 0;
    let mut balanced = true;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    balanced = false;
                }
            }
            _ => {}
        }
    }
    if balanced && depth == 0 {
        return ('{', '}');
    }
    "+|!=~^/:;@\"',.?-_*&#$%"
        .chars()
        .find(|c| !text.contains(*c))
        .or_else(|| ('!'..='~').find(|c| !c.is_ascii_alphanumeric() && !"{}[]()<>\\".contains(*c) && !text.contains(*c)))
        .map_or(('{', '}'), |c| (c, c))
}

/// `text` as a code span of `flmdoc`, for a summary: between runs of backticks longer than
/// any run it holds, with a space inside each end when the text begins or ends with a
/// backtick or a space.
pub(super) fn code_span(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = text.starts_with(['`', ' ']) || text.ends_with(['`', ' ']);
    if pad { format!("{fence} {text} {fence}") } else { format!("{fence}{text}{fence}") }
}

/// A link to the page `path`, showing `text` (in FLM).
fn link(path: &str, text: &str) -> String {
    format!("\\docref{}{{{text}}}", raw(path))
}

/// A link to the page `path`, showing `name` as code.
fn link_code(path: &str, name: &str) -> String {
    link(path, &code(name))
}

/// The facts saying that `feature` declares a thing (`declared-by`): a link to the
/// feature's page, or, for the language's own name or the host's ([`HOST_FEATURE`]), neither
/// of which is a feature with a page, the name as code, declared by the language or by the
/// program.
fn declared_by(feature: &str) -> Keys {
    match feature {
        LANGUAGE_OWNER => Keys::new().value("declared-by", &code(feature)).word("declarer", "language"),
        HOST_FEATURE => Keys::new().value("declared-by", &code(feature)).word("declarer", "program"),
        _ => Keys::new().value("declared-by", &link_code(&paths::feature(feature), feature)),
    }
}

/// The facts saying who declared the workflow of `view`: the feature that declares it
/// ([`declared_by`]); for a workflow that no feature declares, given to the registry on
/// behalf of the host, who wrote its fields as data (cargo feature `flm-host`), without a
/// link, since the host is no feature and has no page.
fn workflow_declarer(doc: &Doc, view: &WorkflowView) -> Keys {
    #[cfg(feature = "flm-host")]
    if view.owner == HOST_FEATURE {
        let fields = doc.env.and_then(Environment::host).and_then(|host| host.workflows.get(&view.name));
        let mut writers: Vec<(Keys, String)> = Vec::new();
        for origin in fields.into_iter().flat_map(|fields| fields.values()) {
            let writer = writer(origin);
            if !writers.iter().any(|(_, known)| *known == writer.1) {
                writers.push(writer);
            }
        }
        if !writers.is_empty() {
            return Keys::new().value("declared-by", &inline_list(writers)).word("declarer", "data");
        }
    }
    #[cfg(not(feature = "flm-host"))]
    let _ = doc;
    declared_by(&view.owner)
}

/// Who wrote a setting, from where it was written (`origin`), as an item of an inline list:
/// the package, as the page `packages/` names it, or the origin as the host writes it (the
/// program's defaults, a place in a front matter, the option of the command line).
#[cfg(feature = "flm-host")]
fn writer(origin: &crate::host::OriginView) -> (Keys, String) {
    match &origin.package {
        Some(package) => (Keys::new().flag("package", true), link_code(&paths::package(package), package)),
        None => (Keys::new(), esc(&origin.text)),
    }
}

/// The heading of a list of commands of the kind `kind`.
fn kind_plural(kind: CallableKind) -> &'static str {
    match kind {
        CallableKind::Macro => "Macros",
        CallableKind::Environment => "Environments",
        CallableKind::Specials => "Special character sequences",
    }
}

/// How the command `name` of the kind `kind` is written: `\name`, `\begin{name}`, or the
/// trigger of a special character sequence.
fn written(kind: CallableKind, name: &str) -> String {
    match kind {
        CallableKind::Macro => format!("\\{name}"),
        CallableKind::Environment => format!("\\begin{{{name}}}"),
        CallableKind::Specials => name.to_string(),
    }
}

/// The JSON form of a value, as text: a string as itself, anything else as compact JSON.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// The lowercase word that serializing `value` gives (a layout, a mode, a binding).
fn word(value: &impl Serialize) -> String {
    serde_json::to_value(value).ok().as_ref().map(value_text).unwrap_or_default()
}

/// The keys of a JSON Schema that `\docjsontype` reads.
const SCHEMA_KEYS: [&str; 8] = ["type", "enum", "const", "items", "additionalProperties", "anyOf", "oneOf", "$ref"];

/// The type that the JSON Schema `schema` describes, as `\docjsontype` holding the minimal
/// form of the schema ([`minimal_schema`]).
fn json_type(schema: &Value) -> String {
    let minimal = serde_json::to_string(&minimal_schema(schema)).unwrap_or_default();
    format!("\\docjsontype{}", raw(&minimal))
}

/// The minimal form of the JSON Schema `schema`: only the keys that say what type it
/// describes ([`SCHEMA_KEYS`]), at every depth; `additionalProperties` only when it is a
/// schema.
fn minimal_schema(schema: &Value) -> Value {
    let Value::Object(map) = schema else { return schema.clone() };
    let mut minimal = Map::new();
    for key in SCHEMA_KEYS {
        let Some(value) = map.get(key) else { continue };
        let value = match (key, value) {
            ("items" | "additionalProperties", Value::Object(_)) => minimal_schema(value),
            ("additionalProperties", _) => continue,
            ("anyOf" | "oneOf", Value::Array(options)) => Value::Array(options.iter().map(minimal_schema).collect()),
            _ => value.clone(),
        };
        minimal.insert(key.to_string(), value);
    }
    Value::Object(minimal)
}

/// An index page: its summary, then its children with their summaries.
fn index(doc: &mut Doc, page: &Page) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    let entries: Vec<Entry> = page
        .children
        .iter()
        .map(|child| Entry::new(link(&child.path, &esc(&child.title)), doc.flm(&child.summary, &child.source)))
        .collect();
    doc.entries(Keys::new(), &entries);
}

fn root(doc: &mut Doc, page: &Page, view: &RootView) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    root_profile(doc, view);
    let packages: Vec<Entry> = view.packages.iter().map(package_entry).collect();
    doc.entries(Keys::new().value("title", "Packages"), &packages);
    let features: Vec<Entry> = view
        .features
        .iter()
        .map(|feature| {
            let source = doc.declared(Some(&feature.name), &feature.summary);
            Entry::new(link_code(&paths::feature(&feature.name), &feature.name), doc.flm(&feature.summary, &source))
        })
        .collect();
    doc.entries(Keys::new().value("title", "Features"), &features);
    let workflows: Vec<Entry> = view
        .workflows
        .iter()
        .map(|workflow| {
            let source = doc.workflow_source(&workflow.name, &workflow.summary);
            Entry {
                keys: Keys::new().value("outputs", &code_list(&workflow.outputs)),
                name: link_code(&paths::workflow(&workflow.name), &workflow.name),
                summary: doc.flm(&workflow.summary, &source),
            }
        })
        .collect();
    doc.entries(
        Keys::new().value("title", "Workflows").value(
            "intro",
            "Each workflow says what a build produces, and is also the profile of the same name: building a document \
             with a profile runs the workflow of that name.",
        ),
        &workflows,
    );
    doc.heading(1, "Going further");
    let sections = plain_list(page.children.iter().map(|child| link(&child.path, &esc(&child.title))));
    doc.block(&format!(
        "The documentation has these sections, each listing its pages: {sections}. A command's page is found under the \
         feature that declares it, by its kind: the macro {} of the feature {} has the page {}.",
        code("\\section"),
        code("headings"),
        code("features/headings/macros/section"),
    ));
    doc.block(&format!(
        "A name that is not the path of a page is looked up across the pages: {} shows the pages of the commands \
         written {}, and {} the page of the entity of that name. The option {} gives the pages as JSON.",
        code("flm info \\section"),
        code("\\section"),
        code("flm info flm.refs.resolve"),
        code("--json"),
    ));
}

/// The facts of `/` about the profile and the math processor in view (cargo feature
/// `flm-host`), or about the workflow in view.
fn root_profile(doc: &mut Doc, view: &RootView) {
    #[cfg(feature = "flm-host")]
    if let Some(profile) = &view.profile {
        doc.field("Profile in view", &code(&profile.name));
        doc.field("Workflow it runs", &link_code(&paths::workflow(&profile.workflow), &profile.workflow));
        doc.field("Outputs", &code_list(&profile.outputs));
        doc.field("Package of the profile, in use", &profile.package.as_deref().map(code).unwrap_or_default());
        match &view.math {
            Some(math) => {
                doc.field("Math processor", &link_code(&paths::package(&math.package), &math.package));
                doc.field("Feature of the math processor", &link_code(&paths::feature(&math.feature), &math.feature));
            }
            None => doc.block("No package processes the mathematics of a document."),
        }
        return;
    }
    match &view.workflow {
        Some(workflow) => {
            doc.field("Workflow in view", &link_code(&paths::workflow(workflow), workflow));
            doc.block("The page of the workflow in view shows the plan built from it.");
        }
        None => doc.block("No workflow is in view: no page shows a plan."),
    }
}

/// One package as `/` lists it: a link, with where it comes from (cargo feature
/// `flm-host`).
fn package_entry(package: &PackageView) -> Entry {
    let name = link_code(&paths::package(&package.name), &package.name);
    #[cfg(feature = "flm-host")]
    if let Some(origin) = &package.origin {
        return Entry::with(origin_keys(origin), name);
    }
    Entry::with(Keys::new(), name)
}

/// The facts saying where a package comes from (`origin`, with `extension`).
#[cfg(feature = "flm-host")]
fn origin_keys(origin: &crate::host::PackageOrigin) -> Keys {
    use crate::host::PackageOrigin;
    match origin {
        PackageOrigin::Compiled => Keys::new().word("origin", "compiled"),
        PackageOrigin::Extension(extension) => {
            Keys::new().word("origin", "extension").value("extension", &code(&extension.identity))
        }
        PackageOrigin::Bundled(extension) => Keys::new().word("origin", "bundled").value("extension", &code(&extension.identity)),
    }
}

fn package(doc: &mut Doc, page: &Page, view: &PackageView) {
    #[cfg(feature = "flm-host")]
    let keys = view.origin.as_ref().map(origin_keys).unwrap_or_default();
    #[cfg(not(feature = "flm-host"))]
    let keys = Keys::new();
    doc.begin_item("package", &view.name, keys);
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    #[cfg(feature = "flm-host")]
    if let Some(crate::host::PackageOrigin::Extension(extension) | crate::host::PackageOrigin::Bundled(extension)) = &view.origin {
        doc.field("Package of the extension", &code(&extension.package));
        doc.field("Description", &extension.description.as_deref().map(esc).unwrap_or_default());
        doc.field("Author", &extension.author.as_deref().map(esc).unwrap_or_default());
        doc.field("Engine", &extension.engine.as_deref().map(code).unwrap_or_default());
    }
    doc.field("Uses", &plain_list(view.uses.iter().map(|name| link_code(&paths::package(name), name))));
    let features = view.features.iter().map(|name| match view.not_in_registry.contains(name) {
        true => (Keys::new().flag("missing", true), code(name)),
        false => (Keys::new(), link_code(&paths::feature(name), name)),
    });
    doc.field("Features", &inline_list(features));
    doc.end_item();
}

fn feature(doc: &mut Doc, page: &Page, view: &FeatureView) {
    doc.begin_item("feature", &view.name, Keys::new());
    let source = doc.declared(Some(&view.name), &view.docs);
    doc.docs(&view.docs, &source);
    doc.field("Title", &esc(&view.title));
    doc.field("Version", &view.version.as_deref().map(code).unwrap_or_default());
    doc.field(
        "Package",
        &view.package.as_deref().map(|package| link_code(&paths::package(package), package)).unwrap_or_default(),
    );
    doc.end_item();
    let elements: Vec<Entry> = view
        .elements
        .iter()
        .map(|element| {
            let source = doc.element_type_source(&element.name, &element.summary);
            Entry::new(link_code(&paths::entity("element", &element.name, None), &element.name), doc.flm(&element.summary, &source))
        })
        .collect();
    doc.entries(
        Keys::new().value("title", "Element types").value(
            "intro",
            "Each node of a document's tree carries an element, which says what the node means. The feature registers \
             these element types; the page of each lists its values and the commands written for them:",
        ),
        &elements,
    );
    for declared in &view.declares {
        let namespace = declared.namespace.as_str();
        let entries: Vec<Entry> = declared
            .entries
            .iter()
            .map(|entry| {
                let shown = match namespace {
                    "settings-key" => entry.name.strip_prefix(&format!("{}.", view.name)).unwrap_or(&entry.name),
                    _ => entry.name.as_str(),
                };
                let name = link_code(&paths::entity(namespace, &entry.name, entry.owner.as_deref()), shown);
                let source = doc.entry_source(namespace, &entry.name, entry.owner.as_deref().or(Some(&view.name)), &entry.summary);
                Entry::new(name, doc.flm(&entry.summary, &source))
            })
            .collect();
        let options = match namespace {
            "scoped-package" => Keys::new().value("title", "Scoped packages").value(
                "intro",
                "Sets of commands that the feature defines only inside certain environment bodies or arguments:",
            ),
            "settings-key" => Keys::new().value("title", "Settings").value(
                "intro",
                &format!(
                    "The feature's settings keys, described on the page {}:",
                    link(&paths::settings(&view.name), &esc(&format!("Settings of feature {}", view.name)))
                ),
            ),
            _ => Keys::new().value("title", namespace_label(namespace).1),
        };
        doc.entries(options, &entries);
    }
    let lists: Vec<Entry> =
        page.children.iter().map(|child| Entry::with(Keys::new(), link(&child.path, &esc(&child.title)))).collect();
    doc.entries(Keys::new().value("title", "Lists of commands"), &lists);
}

/// One element value, declared by `owner`, as its element type's page lists it: its
/// summary, the commands written for it, and its aspects.
fn value_entry(doc: &mut Doc, value: &ElementValueView, owner: Option<&str>) -> Entry {
    let source = doc.declared(owner, &value.docs);
    let summary = doc.flm(&value.summary, &source);
    let keys = Keys::new()
        .value("written", &inline_list(value.commands.iter().map(command_ref)))
        .flag("unwritten", value.commands.is_empty())
        .value("aspects", &code_list(&value.aspects));
    Entry { keys, name: String::new(), summary }
}

/// A command written for an element value, as an item of an inline list: a link to the
/// command's page (for a tack-on field, the field, with a link to the page of the command
/// that accepts it).
fn command_ref(reference: &CommandRef) -> (Keys, String) {
    let within = reference.within.as_deref();
    match &reference.field_of {
        Some(owner) => (
            Keys::new().value("field", &code(&format!("\\{}", reference.name))),
            link_code(&paths::command(&reference.feature, within, owner.kind, &owner.name), &written(owner.kind, &owner.name)),
        ),
        None => (
            Keys::new(),
            link_code(&paths::command(&reference.feature, within, reference.kind, &reference.name), &written(reference.kind, &reference.name)),
        ),
    }
}

fn command(doc: &mut Doc, view: &CommandPageView) {
    for declaration in &view.declarations {
        command_declaration(doc, view, declaration);
    }
}

fn command_declaration(doc: &mut Doc, page: &CommandPageView, view: &CommandView) {
    let kind = page.kind;
    let name = written(kind, &page.name);
    let feature = view.feature.as_str();
    let mut keys = declared_by(feature);
    if let Some(package) = &view.within {
        keys = keys.value("within", &link_code(&paths::scoped_package(&page.feature, package), package));
    }
    let keys = keys.value("modes", &code_list(&view.modes)).value("places", &inline_list(view.available_in.iter().map(use_site)));
    doc.begin_item(&word(&kind), if kind == CallableKind::Environment { &page.name } else { &name }, keys);
    doc.block(&format!("\\docsignature{}", raw(&signature(kind, &page.name, &view.arguments))));
    let source = doc.declared(Some(feature), &view.docs);
    doc.docs(&view.docs, &source);
    let mut arguments: Vec<Param> =
        view.arguments.iter().filter(|argument| argument.fields.is_empty()).map(|argument| param(doc, argument, feature, None)).collect();
    if let Some(body) = &view.body {
        let mut keys = Keys::new().word("layout", &word(&body.layout));
        if let Some(mode) = &body.mode {
            keys = keys.word("mode", mode);
        }
        let keys = keys
            .value("scoped", &code_list(&body.scoped))
            .flag("kept", body.opaque_wrapper)
            .value("reader", body.reader.as_deref().unwrap_or_default());
        arguments.push(Param { keys, form: String::from("…"), name: String::from("body"), role: String::new(), text: String::new() });
    }
    doc.params(None, &arguments);
    let mut fields = Vec::new();
    for argument in &view.arguments {
        for field in &argument.fields {
            let form = format!("\\{}{}", field.name, field.arguments.iter().map(|argument| argument.written.as_str()).collect::<String>());
            let source = doc.declared(Some(feature), &field.docs);
            let text = doc.flm(&field.docs, &source);
            let keys = Keys::new().flag("repeatable", field.repeatable);
            fields.push(Param { keys, form, name: field.name.clone(), role: String::new(), text });
            let owner = format!("\\{}", field.name);
            fields.extend(field.arguments.iter().map(|argument| param(doc, argument, feature, Some(&owner))));
        }
    }
    doc.params(Some("Fields written after it"), &fields);
    doc.field("Element type", &link_code(&paths::entity("element", &view.element.name, None), &view.element.name));
    doc.end_item();
}

/// The signature of the command written `name` of the kind `kind` with `arguments`: its
/// name, then each argument's written form.
fn signature(kind: CallableKind, name: &str, arguments: &[ArgumentView]) -> String {
    let mut text = written(kind, name);
    for argument in arguments {
        // A macro's name made of letters would run into an argument that begins with a
        // letter.
        if kind == CallableKind::Macro
            && text.ends_with(|c: char| c.is_ascii_alphabetic())
            && argument.written.starts_with(|c: char| c.is_ascii_alphabetic())
            && text[1..].chars().all(|c| c.is_ascii_alphabetic())
        {
            text.push(' ');
        }
        text.push_str(&argument.written);
    }
    if kind == CallableKind::Environment {
        text.push_str(&format!("\n…\n\\end{{{name}}}"));
    }
    text
}

/// One argument of a command that the feature `feature` declares, as a parameter: its
/// written form, its name, its role and its text, with the facts of its layout, mode and
/// scoped packages when they are not the default, of its opaque wrapper when the
/// declaration places one, and of `field`, the field it belongs to.
fn param(doc: &mut Doc, argument: &ArgumentView, feature: &str, field: Option<&str>) -> Param {
    let layout = word(&argument.layout);
    let mut keys = Keys::new();
    if layout != "inline" {
        keys = keys.word("layout", &layout);
    }
    if let Some(mode) = &argument.mode {
        keys = keys.word("mode", mode);
    }
    let keys = keys
        .value("scoped", &code_list(&argument.scoped))
        .flag("kept", argument.opaque_wrapper)
        .value("field", &field.map(code).unwrap_or_default());
    let source = doc.declared(Some(feature), &argument.docs);
    let text = doc.flm(&argument.docs, &source);
    Param {
        keys,
        form: argument.written.clone(),
        name: argument.name.clone().unwrap_or_default(),
        role: argument.role.clone().unwrap_or_default(),
        text,
    }
}

/// One place that makes a scoped package available, as an item of the inline list of
/// `places`: a link to the command's page, with the part of the command (the body, an
/// argument by its name or by its position) and the field it belongs to.
fn use_site(site: &UseSiteView) -> (Keys, String) {
    let command = match site.kind {
        Some(kind) => link_code(&paths::command(&site.feature, site.within.as_deref(), kind, &site.command), &written(kind, &site.command)),
        None => code(&site.command),
    };
    let keys = if site.field.is_none() && site.part == "body" {
        Keys::new().flag("body", true)
    } else {
        Keys::new().word("argument", site.part.strip_prefix('#').unwrap_or(&site.part))
    };
    let keys = keys.value("field", &site.field.as_ref().map(|field| code(&format!("\\{field}"))).unwrap_or_default());
    (keys, command)
}

fn scoped_package(doc: &mut Doc, view: &ScopedPackageView) {
    let keys = declared_by(&view.owner).value("places", &inline_list(view.available_in.iter().map(use_site)));
    doc.begin_item("scoped-package", &view.name, keys);
    let source = doc.declared(Some(&view.owner), &view.docs);
    doc.docs(&view.docs, &source);
    doc.end_item();
    for (kind, views) in [
        (CallableKind::Macro, &view.commands.macros),
        (CallableKind::Environment, &view.commands.environments),
        (CallableKind::Specials, &view.commands.specials),
    ] {
        let mut entries: Vec<Entry> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for command in views {
            let names: Vec<&String> = match &command.trigger {
                Some(trigger) => vec![trigger],
                None => command.names.iter().collect(),
            };
            for name in names {
                if seen.contains(name) {
                    continue;
                }
                seen.push(name.clone());
                let link = link_code(&paths::command(&view.owner, Some(&view.name), kind, name), &written(kind, name));
                let source = doc.declared(Some(&command.feature), &command.docs);
                entries.push(Entry::new(link, doc.flm(&command.summary, &source)));
            }
        }
        doc.entries(Keys::new().value("title", kind_plural(kind)), &entries);
    }
}

fn settings(doc: &mut Doc, page: &Page, view: &SettingsView) {
    // A feature that the registry does not hold (switched off by a settings source) has no
    // page.
    let keys = if page.tree.environment().registry().feature(&view.feature).is_some() {
        declared_by(&view.feature)
    } else {
        Keys::new().value("declared-by", &inline_list([(Keys::new().flag("missing", true), code(&view.feature))]))
    };
    #[cfg(feature = "flm-host")]
    let keys = match &view.switched_off {
        Some(origin) => Keys::new().value("declared-by", &code(&view.feature)).value("switched-off", &esc(&origin.text)),
        None => keys,
    };
    doc.begin_item("settings", &view.feature, keys);
    if let Some(description) = &view.description {
        let source = doc.declared(Some(&view.feature), description);
        doc.docs(description, &source);
    }
    doc.end_item();
    for key in &view.keys {
        doc.begin_item("setting", &key.name, Keys::new());
        let description = key.description.as_deref().unwrap_or_default();
        let source = doc.declared(Some(&view.feature), description);
        doc.docs(description, &source);
        doc.field("Type", &json_type(&key.schema));
        doc.field("Default", &key.default.as_ref().map(|value| code(&value_text(value))).unwrap_or_default());
        doc.field("Where it may be set", &code(&word(&key.bind)));
        if !key.marks.is_empty() {
            doc.field("Marks", &code_list(&key.marks));
        }
        #[cfg(feature = "flm-host")]
        if let Some(in_effect) = &key.in_effect {
            doc.field("Value in effect", &code(&value_text(&in_effect.value)));
            doc.field("Written in", &esc(&in_effect.origin.text));
        }
        doc.end_item();
    }
}

fn parsing(doc: &mut Doc, page: &Page, view: &ParsingView) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    for key in &view.keys {
        doc.begin_item("setting", &key.name, Keys::new());
        doc.docs(key.description.as_deref().unwrap_or_default(), &TextSource::Flm);
        doc.field("Value", &code(&value_text(&key.value)));
        doc.field("Type", &json_type(&key.schema));
        doc.field("Default", &key.default.as_ref().map(|value| code(&value_text(value))).unwrap_or_default());
        doc.end_item();
    }
}

#[cfg(feature = "flm-host")]
fn front_matter(doc: &mut Doc, page: &Page, view: &crate::host::FrontMatterView) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    for key in &view.keys {
        doc.begin_item("key", &key.name, Keys::new());
        doc.docs(key.description.as_deref().unwrap_or_default(), &TextSource::Flm);
        doc.field("Type", &json_type(&key.schema));
        doc.end_item();
    }
}

fn workflow(doc: &mut Doc, view: &WorkflowView) {
    let keys = workflow_declarer(doc, view);
    doc.begin_item("workflow", &view.name, keys);
    let source = doc.workflow_source(&view.name, &view.docs);
    doc.docs(&view.docs, &source);
    if let Some(extends) = &view.declared.extends {
        doc.field("Extends", &link_code(&paths::workflow(extends), extends));
    }
    if view.chain.len() > 1 {
        doc.field("Chain", &plain_list(view.chain.iter().map(|name| link_code(&paths::workflow(name), name))));
    }
    let combined = &view.combined;
    doc.field("Outputs", &plain_list(combined.outputs.iter().map(|name| link_code(&paths::entity("output", name, None), name))));
    doc.field("Demands", &plain_list(combined.demands.iter().map(|name| code(name.as_ref()))));
    doc.field("Prefers", &plain_list(combined.prefer.iter().map(|name| link_code(&paths::entity("stage", name, None), name))));
    doc.end_item();
    for (index, (backend, selection)) in combined.lowering.iter().filter(|(_, selection)| !selection.prefer.is_empty()).enumerate() {
        let entries: Vec<Entry> = selection
            .prefer
            .iter()
            .map(|rule| Entry::with(Keys::new(), link_code(&paths::entity("lowering-rule", rule, None), rule)))
            .collect();
        let intro = format!(
            "The preferred lowering rules on the backend {}, the most preferred first:",
            link_code(&paths::entity("backend", backend, None), backend)
        );
        let title = if index == 0 { "Preferred lowering rules" } else { "" };
        doc.entries(Keys::new().value("title", title).value("intro", &intro), &entries);
    }
    for (index, (backend, elements)) in combined.unsupported.iter().filter(|(_, elements)| !elements.is_empty()).enumerate() {
        let entries: Vec<Entry> =
            elements.iter().map(|element| Entry::with(Keys::new(), link_code(&paths::entity("element", element, None), element))).collect();
        let intro = format!(
            "The elements accepted as not renderable on the backend {}:",
            link_code(&paths::entity("backend", backend, None), backend)
        );
        let title = if index == 0 { "Accepted as not renderable" } else { "" };
        doc.entries(Keys::new().value("title", title).value("intro", &intro), &entries);
    }
    let Some(plan) = &view.plan else { return };
    doc.heading(1, "The plan");
    doc.block("The plan in view was built from this workflow.");
    doc.field("Demands", &plain_list(plan.demands.iter().map(|name| link_code(&paths::entity("capability", name, None), name))));
    let mut batch: Option<Option<usize>> = None;
    for stage in &plan.stages {
        if batch != Some(stage.batch) {
            batch = Some(stage.batch);
            doc.heading(2, &match stage.batch {
                None => String::from("Rewrite stages"),
                Some(number) => format!("Batch {number}"),
            });
        }
        let keys = declared_by(&stage.owner).value("page", &link(&paths::entity("stage", &stage.name, None), "the stage's page"));
        doc.begin_item("stage", &stage.name, keys);
        let reads = stage.reads.iter().map(|read| {
            let writer = read.writer.as_ref().map(|writer| link_code(&paths::entity("stage", writer, None), writer));
            let keys = Keys::new().value("writer", &writer.unwrap_or_default()).flag("optional", read.optional);
            (keys, link_code(&paths::entity("table", &read.table, None), &read.table))
        });
        doc.field("Reads", &inline_list(reads));
        doc.field("Writes", &plain_list(stage.writes.iter().map(|table| link_code(&paths::entity("table", table, None), table))));
        doc.field(
            "Requires",
            &plain_list(stage.requires.iter().map(|name| link_code(&paths::entity("capability", name, None), name))),
        );
        doc.field(
            "Reads the options",
            &plain_list(stage.options.iter().map(|name| link_code(&paths::entity("option-type", name, None), name))),
        );
        doc.end_item();
    }
    let excluded: Vec<Entry> = plan
        .excluded
        .iter()
        .map(|excluded| {
            let keys = Keys::new()
                .value("by", &link_code(&paths::entity("stage", &excluded.excluded_by, None), &excluded.excluded_by))
                .value("over", &link_code(&paths::entity("capability", &excluded.over, None), &excluded.over));
            Entry::with(keys, link_code(&paths::entity("stage", &excluded.stage, None), &excluded.stage))
        })
        .collect();
    doc.entries(Keys::new().value("title", "Ruled out").word("level", "2"), &excluded);
    if !plan.warnings.is_empty() {
        doc.heading(2, "Warnings");
        for warning in &plan.warnings {
            let Ok(Value::Object(mut fields)) = serde_json::to_value(warning) else { continue };
            let kind = fields.remove("kind").as_ref().map(value_text).unwrap_or_default();
            doc.begin_item("warning", &kind, Keys::new());
            data_fields(doc, &fields);
            doc.end_item();
        }
    }
    if !plan.lowering.is_empty() {
        doc.heading(2, "Lowering");
        for backend in &plan.lowering {
            let intro = format!(
                "The lowering rule chosen for each element on the backend {}:",
                link_code(&paths::entity("backend", &backend.backend, None), &backend.backend)
            );
            let entries: Vec<Entry> = backend
                .rules
                .iter()
                .map(|choice| {
                    let keys = Keys::new().value("rule", &link_code(&paths::entity("lowering-rule", &choice.rule, None), &choice.rule));
                    Entry::with(keys, code(&choice.element))
                })
                .collect();
            doc.entries(Keys::new().value("intro", &intro), &entries);
        }
    }
    let externals: Vec<Entry> =
        plan.externals.iter().map(|table| Entry::with(Keys::new(), link_code(&paths::entity("table", table, None), table))).collect();
    doc.entries(Keys::new().value("title", "External tables").word("level", "2"), &externals);
}

fn namespace(doc: &mut Doc, page: &Page, view: &NamespaceView) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    let entries: Vec<Entry> = view
        .entries
        .iter()
        .map(|entry| {
            let name = link_code(&paths::entity(&view.name, &entry.name, entry.owner.as_deref()), &entry.name);
            // A settings key's name begins with its feature's name, which owns it. The host
            // owns what it gives the registry as data, and has no page.
            let owner = entry.owner.as_deref().filter(|owner| *owner != LANGUAGE_OWNER && view.name != "settings-key");
            let keys = match owner {
                Some(HOST_FEATURE) => Keys::new().flag("data", true),
                Some(owner) => Keys::new().value("owner", &link_code(&paths::feature(owner), owner)),
                None => Keys::new(),
            };
            let source = doc.entry_source(&view.name, &entry.name, entry.owner.as_deref(), &entry.summary);
            Entry { keys, name, summary: doc.flm(&entry.summary, &source) }
        })
        .collect();
    doc.entries(Keys::new(), &entries);
}

fn entity(doc: &mut Doc, view: &EntityView) {
    for declaration in &view.declarations {
        let keys = declaration.owner.as_deref().map(declared_by).unwrap_or_default();
        let keys = match &declaration.plan {
            Some(status) => stage_status(keys, status),
            None => keys,
        };
        doc.begin_item(&view.namespace, &view.name, keys);
        let source = doc.declared(declaration.owner.as_deref(), &declaration.docs);
        doc.docs(&declaration.docs, &source);
        if let Some(Value::Object(fields)) = &declaration.declaration {
            let fields: Map<String, Value> =
                fields.iter().filter(|(key, _)| !["name", "docs", "owner"].contains(&key.as_str())).map(|(key, value)| (key.clone(), value.clone())).collect();
            data_fields(doc, &fields);
        }
        if let Some(element) = &declaration.lowers {
            doc.field("Lowers", &link_code(&paths::entity("element", element, None), element));
        }
        if let Some(refined) = &declaration.refines {
            doc.field("Refines", &link_code(&paths::entity("capability", refined, None), refined));
        }
        doc.field("Members", &plain_list(declaration.members.iter().map(|name| link_code(&paths::entity("capability", name, None), name))));
        doc.field("Written by", &plain_list(declaration.written_by.iter().map(|name| link_code(&paths::entity("stage", name, None), name))));
        let values: Vec<Entry> =
            declaration.values.iter().map(|value| value_entry(doc, value, declaration.owner.as_deref())).collect();
        doc.entries(Keys::new().value("intro", "Its values, each with the commands written for it:"), &values);
        doc.end_item();
    }
}

/// The fields of a declaration or a record shown as data, each a fact labeled by its key
/// ([`field_label`]): a list of names, or of things with a name, as an inline list of code;
/// any other value as code. An empty value is left out.
fn data_fields(doc: &mut Doc, fields: &Map<String, Value>) {
    for (key, value) in fields {
        let empty = match value {
            Value::Null | Value::Bool(false) => true,
            Value::String(text) => text.is_empty(),
            Value::Array(items) => items.is_empty(),
            Value::Object(map) => map.is_empty(),
            _ => false,
        };
        if empty {
            continue;
        }
        let named = |item: &Value| item.get("name").and_then(Value::as_str).map(str::to_string);
        let shown = match value {
            Value::Array(items) if items.iter().all(Value::is_string) => plain_list(items.iter().map(|item| code(&value_text(item)))),
            Value::Array(items) if items.iter().all(|item| named(item).is_some()) => {
                plain_list(items.iter().filter_map(named).map(|name| code(&name)))
            }
            _ => code(&value_text(value)),
        };
        doc.field(&esc(&field_label(key)), &shown);
    }
}

/// The label of a field of a serialized declaration: its key with a capital first letter
/// and spaces for underscores (`type_name` gives `Type name`).
fn field_label(key: &str) -> String {
    let spaced = key.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => spaced,
    }
}

/// `keys` with the facts saying whether the plan in view schedules a stage (`plan`, with
/// `workflow`, `batch`, `by` and `over`), for the stage's page.
fn stage_status(keys: Keys, status: &StageStatusView) -> Keys {
    let (workflow, keys) = match status {
        StageStatusView::Scheduled { workflow, stage } => {
            let keys = keys.word("plan", "scheduled");
            (workflow, match stage.batch {
                Some(batch) => keys.word("batch", &batch.to_string()),
                None => keys,
            })
        }
        StageStatusView::RuledOut { workflow, by, over } => (
            workflow,
            keys.word("plan", "ruled-out")
                .value("by", &link_code(&paths::entity("stage", by, None), by))
                .value("over", &link_code(&paths::entity("capability", over, None), over)),
        ),
        StageStatusView::NotScheduled { workflow } => (workflow, keys.word("plan", "not-scheduled")),
    };
    let workflow = workflow.as_ref().map(|workflow| link_code(&paths::workflow(workflow), workflow));
    keys.value("workflow", &workflow.unwrap_or_default())
}

fn program(doc: &mut Doc, page: &Page, view: &ProgramView) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    doc.field("Version", &code(&view.version));
    if !view.commands.is_empty() {
        doc.heading(1, "Commands");
        for command in &view.commands {
            doc.begin_item("command", &command.name, Keys::new());
            doc.summary(&esc(&command.help));
            let options: Vec<Param> = command
                .options
                .iter()
                .map(|option| {
                    let keys = Keys::new().value("default", &option.default.as_deref().map(code).unwrap_or_default());
                    // A positional argument has no names: its value's name is its form, and
                    // it is not written a second time as the value.
                    let value_name = option.value_name.clone().unwrap_or_default();
                    let (form, name) =
                        if option.names.is_empty() { (value_name, String::new()) } else { (option.names.join(", "), value_name) };
                    Param { keys, form, name, role: String::new(), text: esc(&option.help) }
                })
                .collect();
            doc.params(Some("Options"), &options);
            doc.end_item();
        }
    }
    let variables: Vec<Entry> =
        view.environment_variables.iter().map(|variable| Entry::new(code(&variable.name), esc(&variable.help))).collect();
    doc.entries(Keys::new().value("title", "Environment variables"), &variables);
    let more: Vec<Entry> = page
        .children
        .iter()
        .map(|child| Entry::new(link(&child.path, &esc(&child.title)), doc.flm(&child.summary, &child.source)))
        .collect();
    doc.entries(Keys::new().value("title", "More about the program"), &more);
}

fn program_paths(doc: &mut Doc, page: &Page, view: &ProgramPaths) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    let folder = |folder: &super::ProgramFolder| {
        let keys = Keys::new().value("from", &esc(&folder.origin)).flag("absent", !folder.exists);
        Entry::with(keys, code(&folder.path))
    };
    let search_path: Vec<Entry> = view.search_path.iter().map(folder).collect();
    doc.entries(
        Keys::new().value("title", "Search path").value("intro", "The folders in which an extension is looked for, in the order searched:"),
        &search_path,
    );
    if let Some(cache) = &view.cache_folder {
        doc.entries(Keys::new().value("title", "Citation cache").value("intro", "The folder of the citation cache:"), &[folder(cache)]);
    }
    if let Some(images) = &view.image_folders {
        let entries: Vec<Entry> = images.folders.iter().map(|path| Entry::with(Keys::new(), code(path))).collect();
        doc.entries(Keys::new().value("title", "Image folders").value("intro", "The folders in which an image's file is looked for:"), &entries);
        doc.block(&esc(&images.rule));
    }
}

fn program_extensions(doc: &mut Doc, page: &Page, view: &ProgramExtensions) {
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.summary(&summary);
    for (title, lead, names) in [
        ("Compiled into the program", "The packages compiled into the program:", &view.compiled),
        ("Found on the search path", "The extensions found in the folders of the search path:", &view.found),
        ("Bundled into the program", "The extensions bundled into the program:", &view.bundled),
    ] {
        if names.is_empty() {
            doc.heading(1, title);
            doc.block("None.");
        } else {
            let entries: Vec<Entry> = names.iter().map(|name| Entry::with(Keys::new(), code(name))).collect();
            doc.entries(Keys::new().value("title", title).value("intro", lead), &entries);
        }
    }
    let profiles: Vec<Entry> = view
        .compiled
        .iter()
        .chain(&view.found)
        .chain(&view.bundled)
        .filter_map(|name| name.strip_prefix("profile-").map(|profile| Entry::with(Keys::new().value("profile", &code(profile)), code(name))))
        .collect();
    if profiles.is_empty() {
        doc.heading(1, "Profiles");
        doc.block("None of them provides a profile of its own.");
    } else {
        let intro = format!(
            "A package or an extension whose name begins with {} provides the packages of the profile named by the rest:",
            code("profile-")
        );
        doc.entries(Keys::new().value("title", "Profiles").value("intro", &intro), &profiles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delimiter_is_one_the_text_does_not_hold() {
        assert_eq!(raw("{title}"), "{{title}}");
        assert_eq!(raw("\\{"), "+\\{+");
        assert_eq!(raw("a+b}"), "|a+b}|");
        assert_eq!(code("\\section"), "\\verbcode{\\section}");
        assert_eq!(raw(""), "{}");
    }

    #[test]
    fn prose_escapes_the_characters_that_mean_something() {
        assert_eq!(esc("50% of {a} & \\b ~`"), "50\\% of \\{a\\} \\& \\textbackslash{}b \\verbtext{~}\\verbtext{`}");
        assert_eq!(esc("--use, ---"), "-{}-use, -{}-{}-");
        assert_eq!(field_label("type_name"), "Type name");
    }

    #[test]
    fn a_code_span_is_longer_than_any_run_it_holds() {
        assert_eq!(code_span("a"), "`a`");
        assert_eq!(code_span("a`b"), "``a`b``");
        assert_eq!(code_span("`"), "`` ` ``");
    }

    #[test]
    fn the_rest_of_a_text_follows_its_first_paragraph() {
        assert_eq!(split_docs("One.\n\nTwo.\n\nThree."), ("One.", "Two.\n\nThree."));
        assert_eq!(split_docs("One."), ("One.", ""));
    }

    #[test]
    fn an_inline_item_beginning_with_a_bracket_is_not_read_as_its_facts() {
        let list = inline_list([(Keys::new(), String::from("[options]")), (Keys::new().flag("optional", true), String::from("[x]"))]);
        assert_eq!(list, "\\begin{docinlinelist} \\item {}[options] \\item[optional] [x] \\end{docinlinelist}");
    }

    #[test]
    fn a_text_to_write_as_plain_text_is_escaped() {
        let broken = "Half {as wide, 50%.\n\nThen `code`.";
        let plain = BTreeSet::from([broken.to_string(), String::from("Half {as wide, 50%.")]);
        let mut doc = Doc { plain, ..Doc::default() };
        doc.docs(broken, &TextSource::Flm);
        assert_eq!(
            doc.text,
            "\\docsummary{Half \\{as wide, 50\\%.}\n\n\\begin{docdetails}\nThen \\verbtext{`}code\\verbtext{`}.\n\\end{docdetails}\n\n"
        );
        assert_eq!(doc.flm("Half {as wide, 50%.", &TextSource::Flm), "Half \\{as wide, 50\\%.");
        // A text that is not listed is written as it is, since it is FLM.
        assert_eq!(doc.flm("Then `code`.", &TextSource::Flm), "Then `code`.");
        let mut doc = Doc::default();
        doc.docs("One `\\x`.\n\nTwo.", &TextSource::Flm);
        assert_eq!(doc.text, "\\docsummary{One `\\x`.}\n\n\\begin{docdetails}\nTwo.\n\\end{docdetails}\n\n");
    }

    #[test]
    fn every_embedded_text_is_recorded_with_its_source_once() {
        let mut doc = Doc::default();
        let feature = TextSource::Feature { feature: String::from("loud"), package: None };
        doc.docs("One.\n\nTwo.", &feature);
        doc.flm("One.", &feature);
        doc.flm("One.", &TextSource::Flm);
        doc.flm("", &feature);
        let recorded: Vec<(&str, &TextSource)> = doc.texts.iter().map(|text| (text.text.as_str(), &text.source)).collect();
        assert_eq!(recorded, [("One.\n\nTwo.", &feature), ("One.", &feature), ("One.", &TextSource::Flm)]);
    }
}
