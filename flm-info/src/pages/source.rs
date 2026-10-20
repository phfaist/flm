//! The FLM source of a page ([`source`]).

use std::collections::BTreeSet;

use flm_core::core::registry::HOST_FEATURE;
use flm_core::lang::mint::CallableKind;
use serde::Serialize;
use serde_json::Value;

use super::build::namespace_label;
use super::paths::{self, LANGUAGE_OWNER};
use super::texts::{self, EmbeddedText, TextSource};
use super::{CommandPageView, Node, Page, ProgramExtensions, ProgramPaths, ProgramView, RootView};
use crate::env::{
    ArgumentView, CommandRef, CommandView, ElementTypeView, ElementValueView, EntityView, Environment, FeatureView,
    NamespaceView, PackageView, ParsingView, ScopedPackageView, SettingsView, StageStatusView, UseSiteView,
    WorkflowView, split_docs,
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
/// Then comes the page's content: its parts as unnumbered sections (`\section*`), the parts
/// of a part as unnumbered subsections, each documented item (a feature, a command,
/// a setting, a stage, …) in the construct `docitem` of `flmdoc` with its signature
/// (`\docsignature`), its summary (`\docsummary`), its arguments (`docparams`) and its facts
/// (`\docfield`), and every link to another page as `\docref{<path>}{<text>}`.
///
/// Every documentation text of the environment is written as it is, since it is FLM with
/// the backtick of `flmdoc` already: its first paragraph in `\docsummary`, then the rest;
/// a text of [`SourceOptions::plain_texts`] is written as plain text instead.
/// [`texts`](super::texts) lists these texts, with whose text each is.
/// Every name and value in the prose is written as code (`\verbcode`), and every path, form
/// and signature is a raw-text argument of a construct of `flmdoc` (`\docref`, `\docparam`,
/// `docitem`, `\docsignature`): either is written between braces when the braces of the text
/// pair up, else between a character that the text does not hold. The text this module
/// writes itself has the characters that mean something in FLM escaped (`\%`, `\{`, …). A
/// page's source holds a `\docref` only to a page of the same tree.
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
        #[cfg(feature = "flm-main")]
        Node::FrontMatter(view) => front_matter(&mut doc, page, view),
        Node::Workflow(view) => workflow(&mut doc, view),
        Node::Namespace(view) => namespace(&mut doc, view),
        Node::Entity(view) => entity(&mut doc, view),
        Node::Program(view) => program(&mut doc, page, view),
        Node::ProgramPaths(view) => program_paths(&mut doc, page, view),
        Node::ProgramExtensions(view) => program_extensions(&mut doc, page, view),
    }
    doc
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

    /// `text`, a documentation text or the summary of one, from `source`, as the page writes
    /// it: as it is, since it is FLM, unless it is among the texts to write as plain text,
    /// which is then escaped ([`esc`]). The text is recorded ([`Doc::record`]).
    fn flm(&mut self, text: &str, source: &TextSource) -> String {
        self.record(text, source);
        if self.plain.contains(text) { esc(text) } else { text.to_string() }
    }

    /// Appends one block holding `text`, a documentation text or the summary of one, from
    /// `source`, as [`Doc::flm`] writes it.
    fn text_block(&mut self, text: &str, source: &TextSource) {
        let flm = self.flm(text, source);
        self.block(&flm);
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

    /// A bulleted list of `items`, each in FLM; nothing for no item.
    fn list(&mut self, items: &[String]) {
        if items.is_empty() {
            return;
        }
        let mut text = String::from("\\begin{itemize}\n");
        for item in items {
            // An item beginning with `[` would be read as the optional label of `\item`.
            let separator = if item.starts_with('[') { "{}" } else { "" };
            text.push_str(&format!("\\item {separator}{item}\n"));
        }
        text.push_str("\\end{itemize}");
        self.block(&text);
    }

    fn begin_item(&mut self, kind: &str, name: &str) {
        self.block(&format!("\\begin{{docitem}}{}{}", raw(kind), raw(name)));
    }

    fn end_item(&mut self) {
        self.block("\\end{docitem}");
    }

    /// A documentation text from `source` written whole: its summary in `\docsummary`, then
    /// the rest ([`Doc::parts`]).
    fn docs(&mut self, docs: &str, source: &TextSource) {
        let (summary, rest) = self.parts(docs, source);
        if !summary.is_empty() {
            self.block(&format!("\\docsummary{{{summary}}}"));
        }
        self.block(&rest);
    }

    fn field(&mut self, key: &str, value: &str) {
        if !value.is_empty() {
            self.block(&format!("\\docfield{{{key}}}{{{value}}}"));
        }
    }

    /// A list of parameters under `title` (`Arguments` when `None`): each a form, a name, a
    /// role and a text in FLM.
    fn params(&mut self, title: Option<&str>, params: &[(String, String, String, String)]) {
        if params.is_empty() {
            return;
        }
        let mut text = match title {
            Some(title) => format!("\\begin{{docparams}}[{title}]\n"),
            None => String::from("\\begin{docparams}\n"),
        };
        for (form, name, role, description) in params {
            text.push_str(&format!("\\docparam{}{}{}{{{description}}}\n", raw(form), raw(name), raw(role)));
        }
        text.push_str("\\end{docparams}");
        self.block(&text);
    }
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

/// A link to the page of the feature `feature`, or its name as code when it is the
/// language's own name or the host's ([`HOST_FEATURE`]), neither of which is a feature with
/// a page.
fn feature_link(feature: &str) -> String {
    match feature {
        LANGUAGE_OWNER => format!("the language itself ({})", code(feature)),
        HOST_FEATURE => format!("the program, as data ({})", code(feature)),
        _ => format!("the feature {}", link_code(&paths::feature(feature), feature)),
    }
}

/// Who declared the workflow of `view`: the feature that declares it ([`feature_link`]);
/// for a workflow that no feature declares, given to the registry on behalf of the host,
/// who wrote its fields as data (cargo feature `flm-main`), without a link, since the host
/// is no feature and has no page.
fn workflow_declarer(doc: &Doc, view: &WorkflowView) -> String {
    #[cfg(feature = "flm-main")]
    if view.owner == HOST_FEATURE {
        let fields = doc.env.and_then(Environment::host).and_then(|host| host.workflows.get(&view.name));
        let mut writers: Vec<String> = Vec::new();
        for origin in fields.into_iter().flat_map(|fields| fields.values()) {
            let writer = writer(origin);
            if !writers.contains(&writer) {
                writers.push(writer);
            }
        }
        if !writers.is_empty() {
            return format!("written as data by {}", writers.join(", "));
        }
    }
    #[cfg(not(feature = "flm-main"))]
    let _ = doc;
    feature_link(&view.owner)
}

/// Who wrote a setting, from where it was written (`origin`), as a phrase in FLM: the
/// package, as the page `packages/` names it, or the origin as the host writes it (the
/// program's defaults, a place in a front matter, the option of the command line).
#[cfg(feature = "flm-main")]
fn writer(origin: &crate::host::OriginView) -> String {
    match &origin.package {
        Some(package) => format!("the package {}", link_code(&paths::package(package), package)),
        None => esc(&origin.text),
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

/// A list of names as code, separated by commas.
fn codes<'a>(names: impl IntoIterator<Item = &'a String>) -> String {
    names.into_iter().map(|name| code(name)).collect::<Vec<_>>().join(", ")
}

/// A one-line rendering of the type a JSON Schema describes: `string`, `integer or null`,
/// `one of "a", "b"`, `list of string`, `map to string`, or the name of a definition it
/// refers to.
fn type_text(schema: &Value) -> String {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let values: Vec<String> = values.iter().map(|value| serde_json::to_string(value).unwrap_or_default()).collect();
        return format!("one of {}", values.join(", "));
    }
    if let Some(value) = schema.get("const") {
        return serde_json::to_string(value).unwrap_or_default();
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return reference.rsplit('/').next().unwrap_or(reference).to_string();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = schema.get(key).and_then(Value::as_array) {
            return options.iter().map(type_text).collect::<Vec<_>>().join(" or ");
        }
    }
    let one = |name: &str| match name {
        "array" => match schema.get("items") {
            Some(items) => format!("list of {}", type_text(items)),
            None => String::from("list"),
        },
        "object" => match schema.get("additionalProperties").filter(|value| value.is_object()) {
            Some(values) => format!("map to {}", type_text(values)),
            None => String::from("map"),
        },
        other => other.to_string(),
    };
    match schema.get("type") {
        Some(Value::String(name)) => one(name),
        Some(Value::Array(names)) => names.iter().filter_map(Value::as_str).map(one).collect::<Vec<_>>().join(" or "),
        _ => String::from("any value"),
    }
}

/// An index page: its summary, then its children with their summaries.
fn index(doc: &mut Doc, page: &Page) {
    doc.text_block(&page.summary, &TextSource::Flm);
    let items: Vec<String> = page
        .children
        .iter()
        .map(|child| {
            let link = link(&child.path, &esc(&child.title));
            if child.summary.is_empty() { link } else { format!("{link}: {}", doc.flm(&child.summary, &child.source)) }
        })
        .collect();
    if page.kind == super::PageKind::NamespaceIndex {
        for item in items {
            doc.block(&item);
        }
    } else {
        doc.list(&items);
    }
}

fn root(doc: &mut Doc, page: &Page, view: &RootView) {
    doc.text_block(&page.summary, &TextSource::Flm);
    root_profile(doc, view);
    doc.heading(1, "Packages");
    let packages: Vec<String> = view.packages.iter().map(package_item).collect();
    doc.list(&packages);
    doc.heading(1, "Features");
    let features: Vec<String> = view
        .features
        .iter()
        .map(|feature| {
            let link = link_code(&paths::feature(&feature.name), &feature.name);
            let summary = if feature.summary.is_empty() {
                esc(&feature.title)
            } else {
                let source = doc.declared(Some(&feature.name), &feature.summary);
                doc.flm(&feature.summary, &source)
            };
            format!("{link}: {summary}")
        })
        .collect();
    doc.list(&features);
    doc.heading(1, "Workflows");
    doc.block(
        "Each workflow says what a build produces, and is also the profile of the same name: building a document \
         with a profile runs the workflow of that name.",
    );
    let workflows: Vec<String> = view
        .workflows
        .iter()
        .map(|workflow| {
            let mut item = link_code(&paths::workflow(&workflow.name), &workflow.name);
            if !workflow.outputs.is_empty() {
                item.push_str(&format!(" (outputs: {})", codes(&workflow.outputs)));
            }
            if !workflow.summary.is_empty() {
                let source = doc.workflow_source(&workflow.name, &workflow.summary);
                item.push_str(&format!(": {}", doc.flm(&workflow.summary, &source)));
            }
            item
        })
        .collect();
    doc.list(&workflows);
    doc.heading(1, "Going further");
    let mut sections: Vec<String> = page.children.iter().map(|child| link(&child.path, &esc(&child.title))).collect();
    let last = sections.pop().unwrap_or_default();
    let sections = if sections.is_empty() { last } else { format!("{} and {last}", sections.join(", ")) };
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

/// The sentences of `/` about the profile and the math processor in view (cargo feature
/// `flm-main`), or about the workflow in view.
fn root_profile(doc: &mut Doc, view: &RootView) {
    #[cfg(feature = "flm-main")]
    if let Some(profile) = &view.profile {
        let mut text = format!(
            "The profile in view is {}: a document built with it runs the workflow {}",
            code(&profile.name),
            link_code(&paths::workflow(&profile.workflow), &profile.workflow),
        );
        if !profile.outputs.is_empty() {
            text.push_str(&format!(", which produces the outputs {}", codes(&profile.outputs)));
        }
        text.push('.');
        if let Some(package) = &profile.package {
            text.push_str(&format!(" The profile's own package, {}, is in use.", code(package)));
        }
        doc.block(&text);
        doc.block(&match &view.math {
            Some(math) => format!(
                "The mathematics of a document is processed by the package {}, through its feature {}.",
                link_code(&paths::package(&math.package), &math.package),
                link_code(&paths::feature(&math.feature), &math.feature),
            ),
            None => String::from("No package processes the mathematics of a document."),
        });
        return;
    }
    doc.block(&match &view.workflow {
        Some(workflow) => format!(
            "The workflow in view is {}: its page shows the plan built from it.",
            link_code(&paths::workflow(workflow), workflow)
        ),
        None => String::from("No workflow is in view: no page shows a plan."),
    });
}

/// One package as `/` lists it: a link, then where it comes from (cargo feature
/// `flm-main`).
fn package_item(package: &PackageView) -> String {
    let link = link_code(&paths::package(&package.name), &package.name);
    #[cfg(feature = "flm-main")]
    if let Some(origin) = &package.origin {
        return format!("{link} ({})", origin_text(origin));
    }
    link
}

/// Where a package comes from, as a phrase in FLM.
#[cfg(feature = "flm-main")]
fn origin_text(origin: &crate::host::PackageOrigin) -> String {
    use crate::host::PackageOrigin;
    match origin {
        PackageOrigin::Compiled => String::from("compiled into the program"),
        PackageOrigin::Extension(extension) => format!("from the extension {}", code(&extension.identity)),
        PackageOrigin::Bundled(extension) => format!("from the extension {}, bundled into the program", code(&extension.identity)),
    }
}

fn package(doc: &mut Doc, page: &Page, view: &PackageView) {
    doc.begin_item("package", &view.name);
    let summary = doc.flm(&page.summary, &TextSource::Flm);
    doc.block(&format!("\\docsummary{{{summary}}}"));
    #[cfg(feature = "flm-main")]
    if let Some(origin) = &view.origin {
        use crate::host::PackageOrigin;
        doc.field("Origin", &origin_text(origin));
        if let PackageOrigin::Extension(extension) | PackageOrigin::Bundled(extension) = origin {
            doc.field("Package of the extension", &code(&extension.package));
            doc.field("Description", &extension.description.as_deref().map(esc).unwrap_or_default());
            doc.field("Author", &extension.author.as_deref().map(esc).unwrap_or_default());
            doc.field("Engine", &extension.engine.as_deref().map(code).unwrap_or_default());
        }
    }
    let uses: Vec<String> = view.uses.iter().map(|name| link_code(&paths::package(name), name)).collect();
    doc.field("Uses", &uses.join(", "));
    let features: Vec<String> = view
        .features
        .iter()
        .map(|name| match view.not_in_registry.contains(name) {
            true => format!("{} (not in the registry)", code(name)),
            false => link_code(&paths::feature(name), name),
        })
        .collect();
    doc.field("Features", &features.join(", "));
    doc.end_item();
}

fn feature(doc: &mut Doc, page: &Page, view: &FeatureView) {
    doc.begin_item("feature", &view.name);
    let source = doc.declared(Some(&view.name), &view.docs);
    doc.docs(&view.docs, &source);
    doc.field("Title", &esc(&view.title));
    doc.field("Version", &view.version.as_deref().map(code).unwrap_or_default());
    doc.field(
        "Package",
        &view.package.as_deref().map(|package| link_code(&paths::package(package), package)).unwrap_or_default(),
    );
    doc.end_item();
    if !view.elements.is_empty() {
        doc.heading(1, "Element types");
        doc.block(
            "Each node of a document's tree carries an element, which says what the node means. The feature registers \
             these element types, each with the commands written for it:",
        );
        for element in &view.elements {
            element_type(doc, element, &view.name);
        }
    }
    for declared in &view.declares {
        let namespace = declared.namespace.as_str();
        let items: Vec<String> = declared
            .entries
            .iter()
            .map(|entry| {
                let shown = match namespace {
                    "settings-key" => entry.name.strip_prefix(&format!("{}.", view.name)).unwrap_or(&entry.name),
                    _ => entry.name.as_str(),
                };
                let link = link_code(&paths::entity(namespace, &entry.name, entry.owner.as_deref()), shown);
                if entry.summary.is_empty() {
                    return link;
                }
                let source = match namespace {
                    "workflow" => doc.workflow_source(&entry.name, &entry.summary),
                    _ => doc.declared(entry.owner.as_deref().or(Some(&view.name)), &entry.summary),
                };
                format!("{link}: {}", doc.flm(&entry.summary, &source))
            })
            .collect();
        match namespace {
            "scoped-package" => {
                doc.heading(1, "Scoped packages");
                doc.block("Sets of commands that the feature defines only inside certain environment bodies or arguments:");
            }
            "settings-key" => {
                doc.heading(1, "Settings");
                doc.block(&format!(
                    "The feature's settings keys, described on the page {}:",
                    link(&paths::settings(&view.name), &esc(&format!("Settings of feature {}", view.name)))
                ));
            }
            _ => {
                doc.heading(1, namespace_label(namespace).1);
            }
        }
        doc.list(&items);
    }
    if !page.children.is_empty() {
        doc.heading(1, "Lists of commands");
        let items: Vec<String> = page.children.iter().map(|child| link(&child.path, &esc(&child.title))).collect();
        doc.list(&items);
    }
}

/// One element type of the feature `feature`: a link to its page, then its values with the
/// commands written for each.
fn element_type(doc: &mut Doc, element: &ElementTypeView, feature: &str) {
    doc.field("Element type", &link_code(&paths::entity("element", &element.name, None), &element.name));
    let items: Vec<String> = element.values.iter().map(|value| value_item(doc, value, Some(feature))).collect();
    doc.list(&items);
}

/// One element value, declared by `owner`: its summary, then the commands written for it.
fn value_item(doc: &mut Doc, value: &ElementValueView, owner: Option<&str>) -> String {
    let commands: Vec<String> = value.commands.iter().map(command_ref).collect();
    let source = doc.declared(owner, &value.docs);
    let summary = doc.flm(&value.summary, &source);
    if commands.is_empty() {
        add_sentence(&summary, "No command is written for this value.")
    } else {
        add_sentence(&summary, &format!("Written as {}.", commands.join(", ")))
    }
}

/// A link to the page of a command (for a tack-on field, to the page of the command that
/// accepts it).
fn command_ref(reference: &CommandRef) -> String {
    let within = reference.within.as_deref();
    match &reference.field_of {
        Some(owner) => format!(
            "the field {} after {}",
            code(&format!("\\{}", reference.name)),
            link_code(&paths::command(&reference.feature, within, owner.kind, &owner.name), &written(owner.kind, &owner.name)),
        ),
        None => link_code(
            &paths::command(&reference.feature, within, reference.kind, &reference.name),
            &written(reference.kind, &reference.name),
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
    doc.begin_item(&word(&kind), if kind == CallableKind::Environment { &page.name } else { &name });
    doc.block(&format!("\\docsignature{}", raw(&signature(kind, &page.name, &view.arguments))));
    let feature = view.feature.as_str();
    let source = doc.declared(Some(feature), &view.docs);
    doc.docs(&view.docs, &source);
    let arguments: Vec<(String, String, String, String)> = view
        .arguments
        .iter()
        .filter(|argument| argument.fields.is_empty())
        .map(|argument| param(doc, argument, feature, None))
        .collect();
    doc.params(None, &arguments);
    let mut fields = Vec::new();
    for argument in &view.arguments {
        for field in &argument.fields {
            let form = format!("\\{}{}", field.name, field.arguments.iter().map(|argument| argument.written.as_str()).collect::<String>());
            let source = doc.declared(Some(feature), &field.docs);
            let mut text = doc.flm(&field.docs, &source);
            if field.repeatable {
                text = add_sentence(&text, "It may be written more than once.");
            }
            fields.push((form, field.name.clone(), String::new(), text));
            let owner = format!("\\{}", field.name);
            fields.extend(field.arguments.iter().map(|argument| param(doc, argument, feature, Some(&owner))));
        }
    }
    doc.params(Some("Fields written after it"), &fields);
    let mut declared = feature_link(&view.feature);
    if let Some(package) = &view.within {
        declared.push_str(&format!(", in the scoped package {}", link_code(&paths::scoped_package(&page.feature, package), package)));
    }
    doc.field("Declared by", &declared);
    doc.field("Available in", &available(&view.modes, &view.available_in));
    // The element value's documentation text whole: its first paragraph in the fact, which
    // is one paragraph, the rest after it.
    let source = doc.declared(Some(feature), &view.element.docs);
    let (summary, rest) = doc.parts(&view.element.docs, &source);
    let mut element = link_code(&paths::entity("element", &view.element.name, None), &view.element.name);
    if !summary.is_empty() {
        element.push_str(&format!(": {summary}"));
    }
    doc.field("Element", &element);
    doc.block(&rest);
    doc.field("Aspects", &codes(&view.element.aspects));
    if let Some(body) = &view.body {
        let mut notes = vec![format!("laid out as {}", code(&word(&body.layout)))];
        if let Some(mode) = &body.mode {
            notes.push(format!("read in {} mode", code(mode)));
        }
        if !body.scoped.is_empty() {
            notes.push(format!("with the scoped packages {} available inside", codes(&body.scoped)));
        }
        if body.raw {
            notes.push(String::from("raw text"));
        }
        if let Some(reader) = &body.reader {
            notes.push(format!("read as {reader}"));
        }
        doc.field("Body", &notes.join("; "));
    }
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
/// written form, its name, its role and its text, with a note of its layout, mode and scoped
/// packages when they are not the default; `field` is the field it belongs to.
fn param(doc: &mut Doc, argument: &ArgumentView, feature: &str, field: Option<&str>) -> (String, String, String, String) {
    let mut notes = Vec::new();
    let layout = word(&argument.layout);
    if layout != "inline" {
        notes.push(format!("laid out as {}", code(&layout)));
    }
    if let Some(mode) = &argument.mode {
        notes.push(format!("read in {} mode", code(mode)));
    }
    if !argument.scoped.is_empty() {
        notes.push(format!("with the scoped packages {} available inside", codes(&argument.scoped)));
    }
    if let Some(field) = field {
        notes.push(format!("an argument of the field {}", code(field)));
    }
    let source = doc.declared(Some(feature), &argument.docs);
    let mut text = doc.flm(&argument.docs, &source);
    for note in notes {
        text = join_note(&text, &note);
    }
    (argument.written.clone(), argument.name.clone().unwrap_or_default(), argument.role.clone().unwrap_or_default(), text)
}

/// `text` followed by `note` in parentheses (`note` alone, in parentheses, after an empty
/// text).
fn join_note(text: &str, note: &str) -> String {
    if text.is_empty() { format!("({note})") } else { format!("{text} ({note})") }
}

/// `text` followed by the sentence `sentence`, `text` ended with a period when it ends
/// without final punctuation (a summary often does); `sentence` alone after an empty text.
fn add_sentence(text: &str, sentence: &str) -> String {
    let text = text.trim_end();
    if text.is_empty() {
        sentence.to_string()
    } else if text.ends_with(['.', '!', '?']) {
        format!("{text} {sentence}")
    } else {
        format!("{text}. {sentence}")
    }
}

/// Where a command is available: its parsing modes, and, for a command of a scoped
/// package, the places that make the package available.
fn available(modes: &[String], sites: &[UseSiteView]) -> String {
    let mut text = match modes {
        [] => String::new(),
        [mode] => format!("{} mode", code(mode)),
        _ => format!("{} modes", codes(modes)),
    };
    if !sites.is_empty() {
        let places: Vec<String> = sites.iter().map(use_site).collect();
        if !text.is_empty() {
            text.push_str(", ");
        }
        text.push_str(&format!("inside {}", places.join(", ")));
    }
    text
}

/// One place that makes a scoped package available, with a link to the command's page.
fn use_site(site: &UseSiteView) -> String {
    let command = match site.kind {
        Some(kind) => link_code(&paths::command(&site.feature, site.within.as_deref(), kind, &site.command), &written(kind, &site.command)),
        None => code(&site.command),
    };
    match (&site.field, site.part.as_str()) {
        (None, "body") => format!("the body of {command}"),
        (None, part) => format!("the argument {} of {command}", code(part)),
        (Some(field), part) => format!("the argument {} of the field {} of {command}", code(part), code(&format!("\\{field}"))),
    }
}

fn scoped_package(doc: &mut Doc, view: &ScopedPackageView) {
    doc.begin_item("scoped-package", &view.name);
    let source = doc.declared(Some(&view.owner), &view.docs);
    doc.docs(&view.docs, &source);
    doc.field("Declared by", &feature_link(&view.owner));
    doc.field("Available in", &available(&[], &view.available_in));
    doc.end_item();
    for (kind, views) in [
        (CallableKind::Macro, &view.commands.macros),
        (CallableKind::Environment, &view.commands.environments),
        (CallableKind::Specials, &view.commands.specials),
    ] {
        let mut items: Vec<String> = Vec::new();
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
                if command.summary.is_empty() {
                    items.push(link);
                } else {
                    let source = doc.declared(Some(&command.feature), &command.docs);
                    items.push(format!("{link}: {}", doc.flm(&command.summary, &source)));
                }
            }
        }
        if !items.is_empty() {
            doc.heading(1, kind_plural(kind));
            doc.list(&items);
        }
    }
}

fn settings(doc: &mut Doc, page: &Page, view: &SettingsView) {
    if let Some(description) = &view.description {
        let source = doc.declared(Some(&view.feature), description);
        doc.text_block(description, &source);
    }
    // A feature that the registry does not hold (switched off by a settings source) has no
    // page.
    let declared = if page.tree.environment().registry().feature(&view.feature).is_some() {
        feature_link(&view.feature)
    } else {
        format!("the feature {} (not in the registry)", code(&view.feature))
    };
    #[cfg(feature = "flm-main")]
    if let Some(origin) = &view.switched_off {
        doc.field("Declared by", &format!("the feature {}, which is switched off", code(&view.feature)));
        doc.field("Switched off", &format!("in {}", esc(&origin.text)));
    } else {
        doc.field("Declared by", &declared);
    }
    #[cfg(not(feature = "flm-main"))]
    doc.field("Declared by", &declared);
    for key in &view.keys {
        doc.begin_item("setting", &key.name);
        let description = key.description.as_deref().unwrap_or_default();
        let source = doc.declared(Some(&view.feature), description);
        doc.docs(description, &source);
        doc.field("Type", &code(&type_text(&key.schema)));
        doc.field("Default", &key.default.as_ref().map(|value| code(&value_text(value))).unwrap_or_default());
        doc.field("Where it may be set", &code(&word(&key.bind)));
        if key.merge_entries {
            doc.field("Combined", "entry by entry, over every source that writes it");
        }
        #[cfg(feature = "flm-main")]
        if let Some(in_effect) = &key.in_effect {
            doc.field("Value in effect", &code(&value_text(&in_effect.value)));
            doc.field("Written in", &esc(&in_effect.origin.text));
        }
        doc.end_item();
    }
}

fn parsing(doc: &mut Doc, page: &Page, view: &ParsingView) {
    doc.text_block(&page.summary, &TextSource::Flm);
    for key in &view.keys {
        doc.begin_item("setting", &key.name);
        doc.docs(key.description.as_deref().unwrap_or_default(), &TextSource::Flm);
        doc.field("Value", &code(&value_text(&key.value)));
        doc.field("Type", &code(&type_text(&key.schema)));
        doc.field("Default", &key.default.as_ref().map(|value| code(&value_text(value))).unwrap_or_default());
        doc.end_item();
    }
}

#[cfg(feature = "flm-main")]
fn front_matter(doc: &mut Doc, page: &Page, view: &crate::host::FrontMatterView) {
    doc.text_block(&page.summary, &TextSource::Flm);
    for key in &view.keys {
        doc.begin_item("key", &key.name);
        doc.docs(key.description.as_deref().unwrap_or_default(), &TextSource::Flm);
        doc.field("Type", &code(&type_text(&key.schema)));
        doc.end_item();
    }
}

fn workflow(doc: &mut Doc, view: &WorkflowView) {
    doc.begin_item("workflow", &view.name);
    let source = doc.workflow_source(&view.name, &view.docs);
    doc.docs(&view.docs, &source);
    let declarer = workflow_declarer(doc, view);
    doc.field("Declared by", &declarer);
    if let Some(extends) = &view.declared.extends {
        doc.field("Extends", &link_code(&paths::workflow(extends), extends));
    }
    if view.chain.len() > 1 {
        let chain: Vec<String> = view.chain.iter().map(|name| link_code(&paths::workflow(name), name)).collect();
        doc.field("Chain", &chain.join(", "));
    }
    let combined = &view.combined;
    let outputs: Vec<String> = combined.outputs.iter().map(|name| link_code(&paths::entity("output", name, None), name)).collect();
    doc.field("Outputs", &outputs.join(", "));
    let demands: Vec<String> = combined.demands.iter().map(ToString::to_string).collect();
    doc.field("Demands", &codes(&demands));
    let prefer: Vec<String> = combined.prefer.iter().map(|name| link_code(&paths::entity("stage", name, None), name)).collect();
    doc.field("Prefers", &prefer.join(", "));
    if !combined.lowering.is_empty() {
        doc.field("Preferred lowering rules", &code(&serde_json::to_string(&combined.lowering).unwrap_or_default()));
    }
    if !combined.unsupported.is_empty() {
        doc.field("Accepted as not renderable", &code(&serde_json::to_string(&combined.unsupported).unwrap_or_default()));
    }
    doc.end_item();
    let Some(plan) = &view.plan else { return };
    doc.heading(1, "The plan");
    doc.block(&format!(
        "The plan in view was built from this workflow. It demands the capabilities {}.",
        plan.demands.iter().map(|name| link_code(&paths::entity("capability", name, None), name)).collect::<Vec<_>>().join(", ")
    ));
    let mut batch: Option<Option<usize>> = None;
    for stage in &plan.stages {
        if batch != Some(stage.batch) {
            batch = Some(stage.batch);
            doc.heading(2, &match stage.batch {
                None => String::from("Rewrite stages"),
                Some(number) => format!("Batch {number}"),
            });
        }
        doc.begin_item("stage", &stage.name);
        doc.field(
            "Declared by",
            &format!("{}; see {}", feature_link(&stage.owner), link(&paths::entity("stage", &stage.name, None), "the stage's page")),
        );
        let reads: Vec<String> = stage
            .reads
            .iter()
            .map(|read| {
                let mut notes = Vec::new();
                if let Some(writer) = &read.writer {
                    notes.push(format!("written by {}", link_code(&paths::entity("stage", writer, None), writer)));
                }
                if read.optional {
                    notes.push(String::from("may be absent"));
                }
                let text = link_code(&paths::entity("table", &read.table, None), &read.table);
                if notes.is_empty() { text } else { format!("{text} ({})", notes.join("; ")) }
            })
            .collect();
        doc.field("Reads", &reads.join(", "));
        let writes: Vec<String> = stage.writes.iter().map(|table| link_code(&paths::entity("table", table, None), table)).collect();
        doc.field("Writes", &writes.join(", "));
        let requires: Vec<String> =
            stage.requires.iter().map(|name| link_code(&paths::entity("capability", name, None), name)).collect();
        doc.field("Requires", &requires.join(", "));
        let options: Vec<String> = stage.options.iter().map(|name| link_code(&paths::entity("option-type", name, None), name)).collect();
        doc.field("Reads the options", &options.join(", "));
        doc.end_item();
    }
    if !plan.excluded.is_empty() {
        doc.heading(2, "Ruled out");
        let items: Vec<String> = plan
            .excluded
            .iter()
            .map(|excluded| {
                format!(
                    "{}, by {}, which provides {} as well",
                    link_code(&paths::entity("stage", &excluded.stage, None), &excluded.stage),
                    link_code(&paths::entity("stage", &excluded.excluded_by, None), &excluded.excluded_by),
                    link_code(&paths::entity("capability", &excluded.over, None), &excluded.over),
                )
            })
            .collect();
        doc.list(&items);
    }
    if !plan.warnings.is_empty() {
        doc.heading(2, "Warnings");
        let items: Vec<String> =
            plan.warnings.iter().map(|warning| code(&serde_json::to_string(warning).unwrap_or_default())).collect();
        doc.list(&items);
    }
    if !plan.lowering.is_empty() {
        doc.heading(2, "Lowering");
        for backend in &plan.lowering {
            doc.block(&format!(
                "The lowering rule chosen for each element on the backend {}:",
                link_code(&paths::entity("backend", &backend.backend, None), &backend.backend)
            ));
            let items: Vec<String> = backend
                .rules
                .iter()
                .map(|choice| format!("{}: {}", code(&choice.element), link_code(&paths::entity("lowering-rule", &choice.rule, None), &choice.rule)))
                .collect();
            doc.list(&items);
        }
    }
    if !plan.externals.is_empty() {
        doc.heading(2, "External tables");
        let items: Vec<String> = plan.externals.iter().map(|table| link_code(&paths::entity("table", table, None), table)).collect();
        doc.list(&items);
    }
}

fn namespace(doc: &mut Doc, view: &NamespaceView) {
    doc.block(&view.what);
    let items: Vec<String> = view
        .entries
        .iter()
        .map(|entry| {
            let mut item = link_code(&paths::entity(&view.name, &entry.name, entry.owner.as_deref()), &entry.name);
            // A settings key's name begins with its feature's name, which owns it. The host
            // owns what it gives the registry as data, and has no page.
            let owner = entry.owner.as_deref().filter(|owner| *owner != LANGUAGE_OWNER && view.name != "settings-key");
            match owner {
                Some(HOST_FEATURE) => item.push_str(" (given as data)"),
                Some(owner) => item.push_str(&format!(" ({})", link_code(&paths::feature(owner), owner))),
                None => {}
            }
            if !entry.summary.is_empty() {
                // A feature is the owner of its own text; a workflow's text may be written as
                // data.
                let source = match view.name.as_str() {
                    "feature" => doc.declared(Some(&entry.name), &entry.summary),
                    "workflow" => doc.workflow_source(&entry.name, &entry.summary),
                    _ => doc.declared(entry.owner.as_deref(), &entry.summary),
                };
                item.push_str(&format!(": {}", doc.flm(&entry.summary, &source)));
            }
            item
        })
        .collect();
    doc.list(&items);
}

fn entity(doc: &mut Doc, view: &EntityView) {
    for declaration in &view.declarations {
        doc.begin_item(&view.namespace, &view.name);
        let source = doc.declared(declaration.owner.as_deref(), &declaration.docs);
        doc.docs(&declaration.docs, &source);
        if let Some(owner) = &declaration.owner {
            doc.field("Declared by", &feature_link(owner));
        }
        if let Some(Value::Object(fields)) = &declaration.declaration {
            for (key, value) in fields {
                let empty = match value {
                    Value::Null | Value::Bool(false) => true,
                    Value::String(text) => text.is_empty(),
                    Value::Array(items) => items.is_empty(),
                    Value::Object(map) => map.is_empty(),
                    _ => false,
                };
                if empty || ["name", "docs", "owner"].contains(&key.as_str()) {
                    continue;
                }
                let named = |item: &Value| item.get("name").and_then(Value::as_str).map(str::to_string);
                let shown = match value {
                    Value::Array(items) if items.iter().all(Value::is_string) => {
                        items.iter().map(|item| code(&value_text(item))).collect::<Vec<_>>().join(", ")
                    }
                    Value::Array(items) if items.iter().all(|item| named(item).is_some()) => {
                        items.iter().filter_map(named).map(|name| code(&name)).collect::<Vec<_>>().join(", ")
                    }
                    _ => code(&value_text(value)),
                };
                doc.field(&esc(&field_label(key)), &shown);
            }
        }
        if let Some(element) = &declaration.lowers {
            doc.field("Lowers", &link_code(&paths::entity("element", element, None), element));
        }
        if let Some(refined) = &declaration.refines {
            doc.field("Refines", &link_code(&paths::entity("capability", refined, None), refined));
        }
        let members: Vec<String> =
            declaration.members.iter().map(|name| link_code(&paths::entity("capability", name, None), name)).collect();
        doc.field("Members", &members.join(", "));
        let writers: Vec<String> =
            declaration.written_by.iter().map(|name| link_code(&paths::entity("stage", name, None), name)).collect();
        doc.field("Written by", &writers.join(", "));
        if let Some(status) = &declaration.plan {
            stage_status(doc, status);
        }
        if !declaration.values.is_empty() {
            doc.block("Its values, each with the commands written for it:");
            let items: Vec<String> =
                declaration.values.iter().map(|value| value_item(doc, value, declaration.owner.as_deref())).collect();
            doc.list(&items);
        }
        doc.end_item();
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

/// Whether the plan in view schedules a stage, as a fact of the stage's page.
fn stage_status(doc: &mut Doc, status: &StageStatusView) {
    let (workflow, text) = match status {
        StageStatusView::Scheduled { workflow, stage } => (
            workflow,
            match stage.batch {
                Some(batch) => format!("scheduled, in batch {batch}"),
                None => String::from("scheduled, as a rewrite stage"),
            },
        ),
        StageStatusView::RuledOut { workflow, by, over } => (
            workflow,
            format!(
                "ruled out by {}, which provides {} as well",
                link_code(&paths::entity("stage", by, None), by),
                link_code(&paths::entity("capability", over, None), over),
            ),
        ),
        StageStatusView::NotScheduled { workflow } => (workflow, String::from("not scheduled: nothing the plan needs requires it")),
    };
    let key = match workflow {
        Some(workflow) => format!("In the plan of the workflow {}", link_code(&paths::workflow(workflow), workflow)),
        None => String::from("In the plan in view"),
    };
    doc.field(&key, &text);
}

fn program(doc: &mut Doc, page: &Page, view: &ProgramView) {
    doc.text_block(&page.summary, &TextSource::Flm);
    doc.field("Version", &code(&view.version));
    if !view.commands.is_empty() {
        doc.heading(1, "Commands");
        for command in &view.commands {
            doc.begin_item("command", &command.name);
            if !command.help.is_empty() {
                doc.block(&format!("\\docsummary{{{}}}", esc(&command.help)));
            }
            let options: Vec<(String, String, String, String)> = command
                .options
                .iter()
                .map(|option| {
                    let mut text = esc(&option.help);
                    if let Some(default) = &option.default {
                        text = join_note(&text, &format!("default: {}", code(default)));
                    }
                    // A positional argument has no names: its value's name is its form, and
                    // it is not written a second time as the value.
                    let value_name = option.value_name.clone().unwrap_or_default();
                    let (form, value) = if option.names.is_empty() { (value_name, String::new()) } else { (option.names.join(", "), value_name) };
                    (form, value, String::new(), text)
                })
                .collect();
            doc.params(Some("Options"), &options);
            doc.end_item();
        }
    }
    if !view.environment_variables.is_empty() {
        doc.heading(1, "Environment variables");
        let items: Vec<String> = view
            .environment_variables
            .iter()
            .map(|variable| {
                if variable.help.is_empty() { code(&variable.name) } else { format!("{}: {}", code(&variable.name), esc(&variable.help)) }
            })
            .collect();
        doc.list(&items);
    }
    let items: Vec<String> = page
        .children
        .iter()
        .map(|child| format!("{}: {}", link(&child.path, &esc(&child.title)), doc.flm(&child.summary, &child.source)))
        .collect();
    doc.heading(1, "More about the program");
    doc.list(&items);
}

fn program_paths(doc: &mut Doc, page: &Page, view: &ProgramPaths) {
    doc.text_block(&page.summary, &TextSource::Flm);
    let folder = |folder: &super::ProgramFolder| {
        let mut text = code(&folder.path);
        if !folder.origin.is_empty() {
            text.push_str(&format!(" ({})", esc(&folder.origin)));
        }
        if !folder.exists {
            text.push_str(", which does not exist");
        }
        text
    };
    doc.heading(1, "Search path");
    doc.block("The folders in which an extension is looked for, in the order searched:");
    let items: Vec<String> = view.search_path.iter().map(folder).collect();
    doc.list(&items);
    if let Some(cache) = &view.cache_folder {
        doc.heading(1, "Citation cache");
        doc.block(&format!("The folder of the citation cache: {}.", folder(cache)));
    }
    if let Some(images) = &view.image_folders {
        doc.heading(1, "Image folders");
        doc.block("The folders in which an image's file is looked for:");
        let items: Vec<String> = images.folders.iter().map(|path| code(path)).collect();
        doc.list(&items);
        doc.block(&esc(&images.rule));
    }
}

fn program_extensions(doc: &mut Doc, page: &Page, view: &ProgramExtensions) {
    doc.text_block(&page.summary, &TextSource::Flm);
    for (title, lead, names) in [
        ("Compiled into the program", "The packages compiled into the program:", &view.compiled),
        ("Found on the search path", "The extensions found in the folders of the search path:", &view.found),
        ("Bundled into the program", "The extensions bundled into the program:", &view.bundled),
    ] {
        doc.heading(1, title);
        if names.is_empty() {
            doc.block("None.");
        } else {
            doc.block(lead);
            let items: Vec<String> = names.iter().map(|name| code(name)).collect();
            doc.list(&items);
        }
    }
    let profiles: Vec<String> = view
        .compiled
        .iter()
        .chain(&view.found)
        .chain(&view.bundled)
        .filter_map(|name| name.strip_prefix("profile-").map(|profile| (name, profile)))
        .map(|(name, profile)| format!("{} provides the profile {}", code(name), code(profile)))
        .collect();
    doc.heading(1, "Profiles");
    if profiles.is_empty() {
        doc.block("None of them provides a profile of its own.");
    } else {
        doc.block(&format!(
            "A package or an extension whose name begins with {} provides the packages of the profile named by the rest:",
            code("profile-")
        ));
        doc.list(&profiles);
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
    fn a_schema_type_is_one_line() {
        let schema = serde_json::json!({"type": ["integer", "null"]});
        assert_eq!(type_text(&schema), "integer or null");
        let schema = serde_json::json!({"type": "array", "items": {"enum": ["a", "b"]}});
        assert_eq!(type_text(&schema), "list of one of \"a\", \"b\"");
        assert_eq!(type_text(&serde_json::json!({"$ref": "#/$defs/Theme"})), "Theme");
    }

    #[test]
    fn a_sentence_follows_a_text_ended_with_a_period() {
        assert_eq!(add_sentence("A paragraph.", "More."), "A paragraph. More.");
        assert_eq!(add_sentence("a paragraph", "More."), "a paragraph. More.");
        assert_eq!(add_sentence("", "More."), "More.");
    }

    #[test]
    fn a_list_item_beginning_with_a_bracket_is_not_read_as_the_label() {
        let mut doc = Doc::default();
        doc.list(&[String::from("[options]"), String::from("plain")]);
        assert_eq!(doc.text, "\\begin{itemize}\n\\item {}[options]\n\\item plain\n\\end{itemize}\n\n");
    }

    #[test]
    fn a_text_to_write_as_plain_text_is_escaped() {
        let broken = "Half {as wide, 50%.\n\nThen `code`.";
        let plain = BTreeSet::from([broken.to_string(), String::from("Half {as wide, 50%.")]);
        let mut doc = Doc { plain, ..Doc::default() };
        doc.docs(broken, &TextSource::Flm);
        assert_eq!(doc.text, "\\docsummary{Half \\{as wide, 50\\%.}\n\nThen \\verbtext{`}code\\verbtext{`}.\n\n");
        assert_eq!(doc.flm("Half {as wide, 50%.", &TextSource::Flm), "Half \\{as wide, 50\\%.");
        // A text that is not listed is written as it is, since it is FLM.
        assert_eq!(doc.flm("Then `code`.", &TextSource::Flm), "Then `code`.");
        let mut doc = Doc::default();
        doc.docs("One `\\x`.\n\nTwo.", &TextSource::Flm);
        assert_eq!(doc.text, "\\docsummary{One `\\x`.}\n\nTwo.\n\n");
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
