//! The `math-mathjax` extension of `extensions/js/math-mathjax/`, loaded as a user
//! installs it: its registration (the commands read from MathJax, the
//! `packages` setting, the registry build's time), the page in each of its
//! two modes (`mathml`, rendered by MathJax in the pipeline, and
//! `load-in-browser`), the copies of a formula, the recomposed LaTeX, the LaTeX
//! for LuaLaTeX and the text, the errors its code reports, and the engine limits MathJax needs.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;
use std::time::Instant;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, PackageEntry, PackageFactory, RegistryBuildError, RegistryBuilder, RegistryInputs};
use flm_core::core::settings::{FeatureSettings, PackageSettings};
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
use flm_core::stdcore::backends::html::mathjax::MATHJAX_LOADER;
use flm_core::stdcore::features::toc;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::backends::latex_rendered;
use flm_core::stdcore;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::{json, Value};

use common::{latex_of, text_of};

/// The extension's folder.
const EXTENSION: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../extensions/js/math-mathjax");

/// The extension's modules, as the loader reads them from its folder.
const MODULES: [&str; 8] = [
    "main.js",
    "definitions.js",
    "tex.js",
    "page.js",
    "render.js",
    "lower.js",
    "vendor/mathjax.js",
    "vendor/bundle.js",
];

/// The extension's manifest: the JSON equal to its `flmextension.yaml`.
fn manifest() -> Value {
    json!({ "extension": { "engine": "js", "js": { "main": "./main.js", "limits": { "stack_mib": 16, "memory_mib": 64, "time_s": 120 } } } })
}

/// The golden file `name` under `tests/goldens/math-mathjax/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/math-mathjax/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The extension's package `_`, loaded with the default host settings.
fn factory() -> PackageFactory {
    factory_with(manifest())
}

/// The extension's package `_`, loaded with the default host settings and the
/// manifest `manifest`.
fn factory_with(manifest: Value) -> PackageFactory {
    let files: Vec<(&str, String)> = MODULES
        .iter()
        .map(|path| (*path, std::fs::read_to_string(format!("{EXTENSION}/{path}")).expect("the module reads")))
        .collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (*path, text.as_str())).collect();
    let source = common::js_extension("math-mathjax", &files, manifest);
    common::factory_of(&common::js_loader(&JsHost::new(JsSettings::default()), source), "math-mathjax")
}

/// The feature's configuration `data`, read with its declaration.
fn configuration(factory: &PackageFactory, data: Value) -> FeatureSettings {
    let decl = factory.settings("math-mathjax").expect("the feature declares settings");
    let Value::Object(map) = data else { panic!("not a map") };
    decl.read(&map).expect("the settings read")
}

/// The package built with the configuration `data`.
fn package(data: Value) -> PackageEntry {
    package_of(factory(), data)
}

/// The package of `factory` built with the configuration `data`.
fn package_of(factory: PackageFactory, data: Value) -> PackageEntry {
    let mut settings = PackageSettings::new();
    settings.insert(configuration(&factory, data));
    factory.build(&settings).expect("the package builds")
}

/// The registry of the standard package and the extension configured with
/// `data`, or the build's error.
fn try_registry(data: Value) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    try_registry_of(package(data))
}

/// The registry of the standard package and the extension's package
/// `package`, or the build's error.
fn try_registry_of(package: PackageEntry) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    // The extension's feature is the registry's one feature that processes
    // mathematics.
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(stdcore::StdcoreConfig::new()));
    builder.use_package(package);
    builder.build(&RegistryInputs::new())
}

/// The registry of the standard package and the extension configured with
/// `data`.
fn registry(data: Value) -> Arc<FlmRegistry> {
    try_registry(data).expect("the registry builds")
}

/// The element of each callable node of `source` parsed by `registry`, with
/// the node's source: `(source, element)`.
fn elements_of(registry: &FlmRegistry, source: &str) -> Vec<(String, String)> {
    let text = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), text, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    fragment
        .tree()
        .root()
        .descendants()
        .filter(|node| node.is_callable())
        .map(|node| (node.span_content().trim().to_string(), element_of(&node).name.to_string()))
        .collect()
}

/// The run of `workflow` over the document `source` under `overrides`.
fn run_with(registry: &Arc<FlmRegistry>, workflow: &str, source: &str, overrides: &Overrides) -> Run {
    let text = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), text, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::for_workflow(registry, workflow, overrides).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The HTML page of `source` under `overrides`, and the diagnostics.
fn page_with(registry: &Arc<FlmRegistry>, source: &str, overrides: &Overrides) -> (String, Vec<String>) {
    let run = run_with(registry, html::WORKFLOW, source, overrides);
    let page = run
        .table(html::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (page, common::diagnostics(&run))
}

/// The HTML page of `source`, and the diagnostics.
fn page_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    page_with(registry, source, &Overrides::none())
}

/// The page's body, from `<body` to the end.
fn body_of(page: &str) -> &str {
    &page[page.find("<body").expect("a body")..]
}

/// The text of the page's script definition `math-mathjax.script`: its
/// configuration part.
fn configuration_of(page: &str) -> &str {
    let start = page.find("window.MathJax = {").unwrap_or_else(|| panic!("a MathJax configuration in {page}"));
    let end = start + page[start..].find("};").expect("the configuration closes");
    &page[start..end]
}

/// The document of the plan's tests: inline, displayed and environment
/// formulas, a label, an author's tag, a `\nonumber`, a nested `cases`, a
/// reference inside a text command and a bold text command.
const DOCUMENT: &str = "Inline \\(x^2\\) and \\(\\textbf{bold } x\\).\n\n\\[\\frac{a}{b}\\]\n\n\\begin{align}\n  a &= b \\label{eq:a} \\\\\n  c &= \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases} \\tag*{(T)} \\\\\n  e &= f \\nonumber\n\\end{align}\n\nSee \\(\\text{see \\ref{eq:a}}\\).";

/// The configuration of `load-in-browser` mode.
fn browser() -> Value {
    json!({ "mode": "load-in-browser" })
}

/// The configuration of `mathml` mode.
fn mathml() -> Value {
    json!({ "mode": "mathml" })
}

// --- registration ------------------------------------------------------------------------------

#[test]
fn the_commands_are_read_from_mathjax_and_declared_in_math_mode() {
    // The plan asks for the measure: printed, with `--nocapture`, beside the
    // build of the standard package alone and the number of declarations.
    let started = Instant::now();
    let registry = registry(json!({}));
    let with_extension = started.elapsed();
    let started = Instant::now();
    drop(common::registry_with([]));
    let alone = started.elapsed();
    let declared = registry.elements().iter().filter(|entry| entry.owner.as_str() == "math-mathjax").count();
    eprintln!(
        "the registry build with the extension took {with_extension:?} ({declared} declarations of the extension; \
         the standard package alone: {alone:?})"
    );
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(notes.is_empty(), "{notes:?}");
    let elements = elements_of(&registry, r"\(\frac{\alpha}{\langle x} \text{t}\) \[\begin{cases} a \\ b \end{cases}\]");
    let expected: Vec<(String, String)> = [
        (r"\frac", "math-mathjax.macro"),
        (r"\alpha", "math-mathjax.macro"),
        (r"\langle", "math-mathjax.macro"),
        (r"\text{t}", "flm.math.text"),
        (r"\begin{cases} a \\ b \end{cases}", "math-mathjax.environment"),
        (r"\\", "math-mathjax.macro"),
    ]
    .iter()
    .map(|(source, element)| (source.to_string(), element.to_string()))
    .collect();
    assert_eq!(elements, expected);
    // In text mode, MathJax's commands are not declared.
    let (_, diagnostics) = text_of(&registry, r"\frac");
    assert_eq!(diagnostics, ["core.specs.unresolvable-command: cannot resolve command ‘\\frac’"]);
    // No element of another processing feature is in the registry.
    assert!(registry.elements().iter().all(|entry| !entry.element.name.as_str().starts_with("flm.math-passthrough.")));
    assert!(registry.occurrable_elements().iter().all(|name| !name.as_str().starts_with("flm.math-passthrough.")));
}

#[test]
fn the_packages_setting_bounds_the_commands_declared() {
    let unknown_dv = "core.specs.unresolvable-command: cannot resolve command ‘\\dv’";
    let (_, diagnostics) = text_of(&registry(json!({ "packages": ["base", "ams"] })), r"\(\dv{x}{t}\)");
    assert_eq!(diagnostics, [unknown_dv]);
    let with_physics = registry(json!({ "packages": ["base", "ams", "physics"] }));
    let (text, diagnostics) = text_of(&with_physics, r"\(\dv{x}{t}\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "\\(\\dv{x}{t}\\)\n");
    assert_eq!(elements_of(&with_physics, r"\(\dv{x}{t}\)")[0].1, "math-mathjax.macro");
}

#[test]
fn newcommand_is_an_unknown_command_under_every_package_list() {
    for packages in [json!(null), json!(["base"]), json!(["base", "ams", "physics"])] {
        let data = if packages.is_null() { json!({}) } else { json!({ "packages": packages }) };
        let (_, diagnostics) = text_of(&registry(data), r"\(\newcommand{\x}{y}\)");
        assert_eq!(
            diagnostics.first().map(String::as_str),
            Some("core.specs.unresolvable-command: cannot resolve command ‘\\newcommand’"),
            "{packages}"
        );
    }
}

#[test]
fn the_commands_that_define_number_or_display_are_unknown_inside_a_formula() {
    let registry = registry(json!({}));
    for (source, command) in [
        (r"\(\definecolor{c}{RGB}{1,2,3} x\)", r"\definecolor"),
        (r"\(\mathtoolsset{centercolon} x\)", r"\mathtoolsset"),
        (r"\(x \label{eq:x}\)", r"\label"),
        (r"\(\eqref{eq:x}\)", r"\eqref"),
    ] {
        let (_, diagnostics) = text_of(&registry, source);
        let unknown = format!("core.specs.unresolvable-command: cannot resolve command ‘{command}’");
        assert_eq!(diagnostics.first(), Some(&unknown), "{source}");
    }
    // A display-math environment inside a formula is unknown; an inner one is MathJax's.
    let (_, diagnostics) = text_of(&registry, r"\[\begin{align} a &= b \end{align}\]");
    assert!(diagnostics.first().is_some_and(|d| d.contains("unknown environment ‘align’")), "{diagnostics:?}");
    assert_eq!(elements_of(&registry, r"\[\begin{aligned} a &= b \end{aligned}\]")[0].1, "math-mathjax.environment");
}

#[test]
fn a_package_outside_the_bundle_fails_the_registration() {
    for name in ["newcommand", "bussproofs", "nosuch"] {
        let error = match try_registry(json!({ "packages": ["base", name] })) {
            Ok(_) => panic!("the registry builds with `{name}`"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains(&format!(
                "the MathJax package `{name}` is not in the extension's bundle; the bundled packages are `base`, `ams`, `amscd`, "
            )),
            "{error}"
        );
        assert!(error.contains("`upgreek`, `units`, `verb`, `dsfont`"), "{error}");
    }
}

#[test]
fn an_empty_package_list_registers_no_command_but_the_extension_s_delimiters() {
    for data in [json!({ "packages": [] }), json!({ "mode": "load-in-browser", "packages": [] })] {
        let registry = registry(data);
        let (_, diagnostics) = text_of(&registry, r"\(\alpha\)");
        assert_eq!(diagnostics, ["core.specs.unresolvable-command: cannot resolve command ‘\\alpha’"]);
        // The two delimiters the extension defines belong to no MathJax package.
        let elements = elements_of(&registry, r"\(\llangle x \rrangle\)");
        assert_eq!(
            elements,
            [(r"\llangle".to_string(), "math-mathjax.macro".to_string()), (r"\rrangle".to_string(), "math-mathjax.macro".to_string())]
        );
    }
}

#[test]
fn a_user_definition_of_a_name_mathjax_defines_fails_the_registry_build() {
    let factory = stdcore::factory().expect("the stdcore factory");
    let decl = factory.settings("userdefs").expect("the userdefs feature declares settings");
    let Value::Object(data) = json!({ "macros": { "ket": { "content": { "math": "|\\psi\\rangle" } } } }) else {
        unreachable!()
    };
    let mut stdcore_settings = PackageSettings::new();
    stdcore_settings.insert(decl.read(&data).expect("the definitions read"));
    let mut builder = RegistryBuilder::new();
    builder.use_package(factory.build(&stdcore_settings).expect("stdcore builds"));
    builder.use_package(package(json!({ "packages": ["base", "ams", "physics"] })));
    let error = match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry builds with `\\ket` defined twice"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        error,
        "the registry could not be built: the language phase found 1 error(s):\nConstruct `\\ket` from \
         `math-mathjax` clashes with `\\ket` defined in `userdefs` in math mode; either set `flm: feature_order: \
         ['userdefs < math-mathjax']` in your front matter to choose `math-mathjax`'s definition, or have \
         `math-mathjax` declare `\\ket` as an explicit override."
    );
}

#[test]
fn the_manifest_is_the_one_the_folder_holds() {
    let yaml = std::fs::read_to_string(format!("{EXTENSION}/flmextension.yaml")).expect("the manifest reads");
    assert!(yaml.contains("engine: js"), "{yaml}");
    assert!(yaml.contains("main: ./main.js"), "{yaml}");
    assert!(yaml.contains("limits: { stack_mib: 16, memory_mib: 64, time_s: 120 }"), "{yaml}");
}

// --- the page in `load-in-browser` mode ---------------------------------------------------------

#[test]
fn the_browser_page_carries_the_formulas_as_tex_with_their_holes() {
    let (page, diagnostics) = page_of(&registry(browser()), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    assert_text_golden(golden("browser.page-body.html"), body);
    // The formulas as text inside the extension's spans.
    assert!(body.contains(r#"<span class="math-mathjax-formula" data-flm-display="inline">\(x^2\)</span>"#), "{body}");
    assert!(body.contains(r#"<span class="math-mathjax-formula" data-flm-display="display">\[\frac{a}{b}\]</span>"#), "{body}");
    assert!(body.contains(r#"<span class="math-mathjax-formula" data-flm-display="display" data-flm-env="align">\begin{align}"#), "{body}");
    // A text command's argument and the reference inside it, in a hole.
    assert!(body.contains(r#"<tex-html><span class="flm-pres-math--text" data-flm-command="textbf">bold </span></tex-html>"#), "{body}");
    assert!(body.contains(r##"<tex-html><span class="flm-pres-math--text" data-flm-command="text">see <a class="flm-pres-link" href="#eq-a""##), "{body}");
    // The lines' anchors, the number and the author's tag in holes, the
    // nested `cases` and its line break as written, `\notag` for the line
    // that shows nothing.
    assert!(body.contains(r#"\begin{align}<tex-html><span id="eq-a"></span></tex-html>"#), "{body}");
    assert!(body.contains(r"\tag*{$<tex-html>(1)</tex-html>$}\\"), "{body}");
    assert!(body.contains(r"\tag*{$<tex-html>(T)</tex-html>$}\\"), "{body}");
    assert!(body.contains(r"\begin{cases} 1 &amp; x &gt; 0 \\ 0 &amp; <tex-html>"), "{body}");
    assert!(body.contains(r"e &amp;= f \notag\end{align}"), "{body}");
    // The page loads one MathJax, the extension's.
    assert!(page.contains(r#"data-flm-def="math-mathjax.script""#), "{page}");
    assert!(!page.contains(r#"data-flm-def="flm.html.mathjax""#), "{page}");
    assert_eq!(page.matches("<script").count(), 2, "{page}");
    assert!(page.contains(&format!(r#"src="{MATHJAX_LOADER}""#)), "{page}");
}

#[test]
fn the_browser_configuration_lists_the_packages_the_document_uses() {
    let registry = registry(browser());
    let (page, diagnostics) = page_of(&registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let configuration = configuration_of(&page);
    assert!(configuration.contains("loader: {load: ['[tex]/ams', '[tex]/texhtml']},"), "{configuration}");
    assert!(configuration.contains("packages: ['base', 'ams', 'texhtml'],"), "{configuration}");
    assert!(configuration.contains("startup: {elements: ['.math-mathjax-formula']}"), "{configuration}");
    assert!(configuration.contains("tags: 'none'"), "{configuration}");
    assert!(page.contains("document.addEventListener('flm:region-updated'"), "{page}");
    let (page, diagnostics) = page_of(&registry, &format!("{DOCUMENT}\n\n\\(\\dv{{x}}{{t}}\\)"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let configuration = configuration_of(&page);
    assert!(configuration.contains("packages: ['base', 'ams', 'physics', 'texhtml'],"), "{configuration}");
    assert!(configuration.contains("loader: {load: ['[tex]/ams', '[tex]/physics', '[tex]/texhtml']},"), "{configuration}");
    // A page without a formula loads nothing.
    let (page, diagnostics) = page_of(&registry, "No formula.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(!page.contains("<script"), "{page}");
}

#[test]
fn the_lazy_setting_loads_the_lazy_component() {
    let (page, _) = page_of(&registry(browser()), r"\(x\)");
    assert!(!configuration_of(&page).contains("ui/lazy"), "{page}");
    let (page, diagnostics) = page_of(&registry(json!({ "mode": "load-in-browser", "lazy": true })), r"\(x\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let configuration = configuration_of(&page);
    assert!(configuration.contains("loader: {load: ['[tex]/texhtml', 'ui/lazy']},"), "{configuration}");
    // The in-place update rules stay.
    assert!(page.contains("data-flm-original-html"), "{page}");
}

#[test]
fn the_loader_url_setting_names_the_address_mathjax_is_loaded_from() {
    let registry = registry(json!({ "mode": "load-in-browser", "loader_url": "https://example.org/mathjax/tex-chtml.js" }));
    let (page, diagnostics) = page_of(&registry, r"\(x\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(page.contains(r#"src="https://example.org/mathjax/tex-chtml.js""#), "{page}");
}

#[test]
fn the_author_s_tex_html_tag_is_neutralized() {
    let (page, diagnostics) = page_of(&registry(browser()), r"\(a <tex-html> b </tex-html>\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(body_of(&page).contains(r"\(a &lt;{}tex-html&gt; b &lt;{}/tex-html&gt;\)"), "{page}");
}

#[test]
fn a_comment_inside_a_formula_is_written_as_a_space() {
    let (page, diagnostics) = page_of(&registry(browser()), "\\(\\alpha%%c\n b\\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(body_of(&page).contains(r"\(\alpha b\)"), "{page}");
}

#[test]
fn a_formula_inside_a_text_command_is_written_into_the_surrounding_formula() {
    let (page, diagnostics) = page_of(&registry(browser()), r"\(\text{a \(y\) b}\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // As the HTML backend writes a passthrough formula: the hole closes before
    // the inner formula, whose TeX goes into the outer formula's characters
    // with no delimiters of its own, and a new hole opens after it.
    assert!(
        body_of(&page).contains(concat!(
            r#"<span class="math-mathjax-formula" data-flm-display="inline">\(<tex-html><span class="flm-pres-math--text" data-flm-command="text">a </span></tex-html>"#,
            r#"y<tex-html><span class="flm-pres-math--text" data-flm-command="text"> b</span></tex-html>\)</span>"#,
        )),
        "{page}"
    );
}

#[test]
fn each_copy_of_a_formula_carries_its_text_in_a_span_of_its_own() {
    let registry = registry(browser());
    let overrides = Overrides {
        render_options: OptionValues::new().with(PiecesRenderOptions::default().include(toc::PIECE)),
        ..Overrides::none()
    };
    let (page, diagnostics) =
        page_with(&registry, "\\section{The square \\(x^2\\)}\\label{sec:sq}\n\nSee \\ref{sec:sq}.", &overrides);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let formula = r#"<span class="math-mathjax-formula" data-flm-display="inline">\(x^2\)</span>"#;
    // The heading, the entry of the table of contents, the reference's text.
    assert_eq!(page.matches(formula).count(), 3, "{page}");
    assert_eq!(page.matches(r#"id="sec-sq""#).count(), 1, "{page}");
}

// --- the other backends ------------------------------------------------------------------------

#[test]
fn the_recomposed_latex_is_what_the_standard_package_alone_writes() {
    for data in [mathml(), browser()] {
        let (latex, diagnostics) = latex_of(&registry(data), DOCUMENT);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_text_golden(golden("document.tex"), &latex);
        let (alone, diagnostics) = latex_of(&common::registry_with([]), DOCUMENT);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(latex, alone);
    }
}

/// The LaTeX for LuaLaTeX of `source` (the document, the entry of the backend's artifacts),
/// and the diagnostics.
fn rendered_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = common::run(registry, latex_rendered::WORKFLOW, source);
    let out = run
        .table(latex_rendered::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (out, common::diagnostics(&run))
}

#[test]
fn the_latex_rendered_formulas_are_what_the_standard_package_alone_writes() {
    // The extension's rules stage the formula and the environment as the general rules of
    // the package that carries formulas unread do.
    for data in [mathml(), browser()] {
        let (latex, diagnostics) = rendered_of(&registry(data), DOCUMENT);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(latex.contains("\\(x^2\\)") && latex.contains("\\begin{align*}"), "{latex}");
        assert!(latex.contains("\\textbf{bold }") && latex.contains("\\tag*{(T)}"), "{latex}");
        let (alone, diagnostics) = rendered_of(&common::registry_with([]), DOCUMENT);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(latex, alone);
    }
}

#[test]
fn a_label_on_a_line_of_its_own_leaves_no_blank_line_in_the_latex() {
    // LaTeX refuses a blank line inside mathematics: the whitespace around a
    // `\label` or a final `\\` taken out of a line of its own must not meet,
    // in the recomposed LaTeX nor in the rendered one. The `math` library's
    // newline rule writes one newline there, and the run that ended the
    // line's content after the line's closing part.
    let document = "\\begin{gather}\n  \\begin{aligned}\n    a &\\to b\\ ;\n    \\\\\n    c &\\to d\\ ,\n  \
                    \\end{aligned}\n  \\label{eq:enc}\n\\end{gather}\n";
    let environment = |latex: &str, name: &str| -> String {
        let start = latex.find(&format!("\\begin{{{name}}}")).unwrap_or_else(|| panic!("`{name}` in {latex}"));
        let end = latex.find(&format!("\\end{{{name}}}")).expect("the environment closes");
        latex[start..end].to_string()
    };
    let registry = registry(json!({}));
    let (latex, diagnostics) = latex_of(&registry, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let written = environment(&latex, "gather");
    assert!(written.lines().all(|line| !line.trim().is_empty()), "{written}");
    assert!(written.ends_with("\\end{aligned}\\flmLabels{\\label{ref1}}\n"), "{written}");
    let (latex, diagnostics) = rendered_of(&registry, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let written = environment(&latex, "gather*");
    assert!(written.lines().all(|line| !line.trim().is_empty()), "{written}");
    assert!(written.ends_with("\\end{aligned}\\tag*{(1)}\n"), "{written}");
    // A final `\\` on a line of its own; a label on a line of its own between
    // two lines of one equation line; a comment, then a label on a line of
    // its own, then mathematics; a macro that takes the newline after its
    // name, then a label on a line of its own.
    let documents = [
        ("align", "\\begin{align}\n  a &= b\n  \\\\\n  c &= d\n  \\\\\n\\end{align}\n"),
        ("equation", "\\begin{equation}\n  a = b\n  \\label{eq:x}\n  + c\n\\end{equation}\n"),
        ("align", "\\begin{align}\n  a &= b \\\\ %% note\n  \\label{eq:c}\n  c &= d\n\\end{align}\n"),
        ("equation", "\\begin{equation}\n  x \\to \\infty\n  \\label{eq:inf}\n\\end{equation}\n"),
    ];
    for (name, document) in documents {
        let (latex, diagnostics) = latex_of(&registry, document);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let written = environment(&latex, name);
        assert!(written.lines().all(|line| !line.trim().is_empty()), "{written}");
        let (latex, diagnostics) = rendered_of(&registry, document);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let written = environment(&latex, &format!("{name}*"));
        assert!(written.lines().all(|line| !line.trim().is_empty()), "{written}");
    }
}

#[test]
fn the_text_prints_each_formula_as_written_with_its_numbers() {
    for data in [mathml(), browser()] {
        let (text, diagnostics) = text_of(&registry(data), DOCUMENT);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_text_golden(golden("document.txt"), &text);
    }
}

// --- the page in `mathml` mode ------------------------------------------------------------------

#[test]
fn the_mathml_page_carries_the_formulas_as_mathml_with_their_holes() {
    let (page, diagnostics) = page_of(&registry(mathml()), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    assert_text_golden(golden("mathml.page-body.html"), body);
    // `\(x^2\)`.
    assert!(body.contains(r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#), "{body}");
    // The rows' identifiers: the labeled line's, and the next line's.
    assert!(
        body.contains(r#"<mtable displaystyle="true" class="math-mathjax-lines"><mtr id="eq-a"><mtd class="math-mathjax-tag-space"></mtd>"#),
        "{body}"
    );
    // The line without a number ends with an empty tag cell.
    assert!(body.contains(r#"<mtd class="math-mathjax-tag"></mtd></mtr></mtable>"#), "{body}");
    assert!(body.contains(r#"</mtd></mtr><mtr id="eq--2oqua">"#), "{body}");
    // The number and the author's tag, each in the last cell of its row.
    assert!(
        body.contains(r#"<mtd class="math-mathjax-tag" style="padding-top: 0; padding-bottom: 1.5pt"><mtext>(1)</mtext></mtd></mtr>"#),
        "{body}"
    );
    assert!(
        body.contains(r#"<mtd class="math-mathjax-tag" style="padding-top: 1.5pt; padding-bottom: 1.5pt"><mtext>(T)</mtext></mtd></mtr>"#),
        "{body}"
    );
    // The alignment of the `align`'s columns, as styles of its cells.
    assert!(body.contains(r#"<mtd style="text-align: right; padding-left: 0; padding-right: 0em; padding-top: 0; padding-bottom: 1.5pt"><mi>a</mi></mtd>"#), "{body}");
    // The reference's link inside an `mtext`, the bold text command's span
    // inside an `mtext`.
    assert!(body.contains(r##"<mtext><span class="flm-pres-math--text" data-flm-command="text">see <a class="flm-pres-link" href="#eq-a""##), "{body}");
    assert!(body.contains(r#"<mtext><span class="flm-pres-math--text" data-flm-command="textbf">bold </span></mtext>"#), "{body}");
    // The stylesheet, no script, no `data-latex`.
    assert!(page.contains(r#"data-flm-def="math-mathjax.style""#), "{page}");
    assert!(page.contains("/* math-mathjax.style */"), "{page}");
    assert!(!page.contains("<script"), "{page}");
    assert!(!page.contains("data-latex"), "{page}");
}

#[test]
fn only_the_table_of_an_environment_s_lines_is_the_lines_table() {
    let (page, diagnostics) = page_of(
        &registry(mathml()),
        "\\[\\begin{matrix}a & b\\end{matrix}\\]\n\n\\[\\begin{aligned}a &= b \\\\ c &= d\\end{aligned}\\]\n\n\
         \\begin{equation}\\begin{aligned}a &= b \\\\ c &= d\\end{aligned}\\nonumber\\end{equation}",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    // A displayed matrix or aligned group, and an equation whose one line is
    // an aligned group: no table of theirs is the lines' table.
    assert!(!body.contains("math-mathjax-lines"), "{body}");
    assert!(!body.contains("math-mathjax-tag-space"), "{body}");
    // The equation's line has an anchor though it has no number; it is on
    // the `math` element, not on a row of the aligned group.
    assert!(body.contains(r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="block" id="eq--"#), "{body}");
    assert!(!body.contains("<mtr id="), "{body}");
}

#[test]
fn every_row_of_the_lines_table_has_as_many_cells() {
    let (page, diagnostics) = page_of(
        &registry(mathml()),
        "\\begin{align}\n  a &= b & c &= d \\tag*{X} \\\\\n  e &= f \\nonumber \\\\\n  g\n\\end{align}",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    let start = body.find(r#"class="math-mathjax-lines">"#).expect(body);
    let table = &body[start..start + body[start..].find("</mtable>").unwrap()];
    // Each row: the empty first cell, the widest line's four cells (the
    // shorter lines padded with empty cells), the tag's cell.
    let counts: Vec<usize> = table.split("<mtr").skip(1).map(|row| row.matches("<mtd").count()).collect();
    assert_eq!(counts, [6, 6, 6], "{table}");
    for row in table.split("<mtr").skip(1) {
        assert!(row.contains(r#"<mtd class="math-mathjax-tag-space"></mtd>"#), "{row}");
        assert!(row.trim_end_matches("</mtr>").ends_with("</mtd>"), "{row}");
        let last = row.rfind("<mtd").unwrap();
        assert!(row[last..].starts_with(r#"<mtd class="math-mathjax-tag""#), "{row}");
    }
}

#[test]
fn the_math_font_url_setting_declares_the_font_first() {
    let (page, diagnostics) = page_of(&registry(json!({ "mode": "mathml", "math_font_url": "fonts/a\"b.woff2" })), "\\(x\\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        page.contains("@font-face {\n    font-family: \"math-mathjax-font\";\n    src: url(\"fonts/a\\\"b.woff2\") format(\"woff2\");\n  }"),
        "{page}"
    );
    assert!(page.contains("font-family: \"math-mathjax-font\", \"Latin Modern Math\""), "{page}");
}

#[test]
fn each_copy_of_a_formula_gets_mathml_of_its_own() {
    let registry = registry(mathml());
    let overrides = Overrides {
        render_options: OptionValues::new().with(PiecesRenderOptions::default().include(toc::PIECE)),
        ..Overrides::none()
    };
    let (page, diagnostics) =
        page_with(&registry, "\\section{The square \\(x^2\\)}\\label{sec:sq}\n\nSee \\ref{sec:sq}.", &overrides);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let formula = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#;
    // The heading, the entry of the table of contents, the reference's text:
    // each a `math` element of its own, none with an identifier.
    assert_eq!(page.matches(formula).count(), 3, "{page}");
    assert_eq!(page.matches(r#"id="sec-sq""#).count(), 1, "{page}");
}

#[test]
fn the_mathml_carries_tex_s_spacing_around_a_relation_and_a_bar() {
    // The relation's thick spaces stand before it and before the opening bar; the
    // bar, as every operator, adds no space of its own.
    let (page, diagnostics) = page_of(&registry(mathml()), r"\(a = \lvert 0\rangle\)");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        page.contains(concat!(
            r#"<mi>a</mi><mspace width="0.278em"></mspace><mo lspace="0" rspace="0">=</mo>"#,
            r#"<mspace width="0.278em"></mspace><mo stretchy="false" lspace="0" rspace="0">|</mo><mn>0</mn>"#,
        )),
        "{page}"
    );
}

// --- errors ------------------------------------------------------------------------------------

#[test]
fn an_empty_equation_line_is_reported() {
    let (_, diagnostics) = text_of(&registry(json!({})), "\\begin{align} a \\\\ \\label{eq:x} \\\\ b \\end{align}");
    assert_eq!(
        diagnostics,
        ["math-mathjax.empty-line: line 1 (counting from 0) of the `align` environment must hold mathematics; a `\\label`, `\\tag` or `\\nonumber` alone is not mathematics"]
    );
}

/// S16: the empty line's report is placed at the line's first node (here the
/// blank after the first `\\`), not at the environment's node.
#[test]
fn an_empty_equation_line_is_reported_at_the_line_s_first_node() {
    let document = "\\begin{align} a \\\\ \\label{eq:x} \\\\ b \\end{align}";
    let run = common::run(&registry(json!({})), flm_core::stdcore::backends::text::WORKFLOW, document);
    let spans: Vec<std::ops::Range<usize>> = run.diagnostics().iter().map(|d| d.span().range()).collect();
    let line = document.find("\\\\ \\label").expect("the line") + 2;
    assert_eq!(spans.len(), 1, "{spans:?}");
    assert_eq!(spans[0].start, line, "{spans:?}");
}

#[test]
fn a_tex_error_is_reported_and_the_formula_shown_as_its_source() {
    let (page, diagnostics) = page_of(&registry(mathml()), r"Before \(\frac{a}\) after.");
    assert_eq!(diagnostics, [r"math-mathjax.tex-error: MathJax cannot typeset the formula: Missing argument for \frac"]);
    assert!(
        body_of(&page).contains(r#"Before <span class="flm-pres-span math--mathjax-formula" data-flm-kind="inline">\(\frac{a}\)</span> after."#),
        "{page}"
    );
}

/// The formulas of the experiment's stress set that nest deepest, rebuilt at
/// a smaller depth: fractions, roots and braces nested `depth` times (branch
/// `experiment-quickjs`, `prototypes/experiment-quickjs/corpus/stress.json`,
/// the class `deep`, which nests 64 fractions, 64 roots and 500 braces).
fn deep_formulas(depth: usize) -> String {
    let nested = |open: &str, close: &str, inner: &str| format!("{}{inner}{}", open.repeat(depth), close.repeat(depth));
    format!(
        "\\[{}\\]\n\n\\[{}\\]\n\n\\({}\\)",
        nested("\\frac{1}{", "}", "x"),
        nested("\\sqrt{", "}", "x"),
        nested("{", "}", "x"),
    )
}

/// The depth of the deep formulas of the limits test.
const DEPTH: usize = 40;

#[test]
fn the_deep_formulas_render_under_the_manifest_s_limits_and_fail_under_the_host_s_default_stack() {
    let source = deep_formulas(DEPTH);
    let (page, diagnostics) = page_of(&registry(mathml()), &source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(body_of(&page).matches("<math ").count(), 3, "{page}");
    assert_eq!(body_of(&page).matches("<mfrac>").count(), DEPTH, "{page}");
    assert_eq!(body_of(&page).matches("<msqrt>").count(), DEPTH, "{page}");
    // The same extension with no limits in its manifest: the host's default
    // stack (1 MiB).
    let without_limits = json!({ "extension": { "engine": "js", "js": { "main": "./main.js" } } });
    let registry = try_registry_of(package_of(factory_with(without_limits), mathml())).expect("the registry builds");
    let run = run_with(&registry, html::WORKFLOW, &source, &Overrides::none());
    let identifiers = common::identifiers(&run);
    assert!(identifiers.iter().any(|id| id == "flm.ext.handler-failed"), "{:?}", common::diagnostics(&run));
}

// --- the preamble ------------------------------------------------------------------------------

/// A preamble that defines a color and an arrow, with a comment holding a `<`.
const PREAMBLE: &str = "\\definecolor{myred}{RGB}{200,0,0} % <red>\n\\Newextarrow{\\xMyarrow}{5,5}{0x21C0}";

/// A formula that uses the color and the arrow.
const WITH_PREAMBLE: &str = r"\(\color{myred} a \xMyarrow{f} b\)";

#[test]
fn the_preamble_s_definitions_hold_in_every_formula_and_an_exported_command_is_declared() {
    let registry = registry(json!({ "mode": "mathml", "preamble": PREAMBLE, "export_commands": ["xMyarrow"] }));
    let (page, diagnostics) = page_of(&registry, &format!(r"{WITH_PREAMBLE} and \(\color{{myred}} c\)"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    // The color in both formulas; the arrow, U+21C0, over its argument.
    assert!(body.contains(r##"<mstyle mathcolor="#c80000"><mi>a</mi>"##), "{body}");
    assert!(body.contains(r##"<mstyle mathcolor="#c80000"><mi>c</mi></mstyle>"##), "{body}");
    assert!(body.contains("<mover><mo lspace=\"0\" rspace=\"0\">\u{21C0}</mo><mpadded"), "{body}");
    let element = registry.elements().iter().find(|entry| entry.element.docs.contains(r"\xMyarrow")).expect("declared");
    assert_eq!(element.element.docs, r"`\xMyarrow`: a command that the `preamble` of the feature `math-mathjax` defines");
}

#[test]
fn a_preamble_mathjax_rejects_fails_the_registration() {
    let error = match try_registry(json!({ "preamble": r"\newcommand{\x}{y}" })) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains(
            r"the `preamble` of the feature `math-mathjax` must be TeX that MathJax accepts; MathJax reports: Undefined control sequence \newcommand"
        ),
        "{error}"
    );
}

#[test]
fn an_export_the_preamble_does_not_define_fails_the_registration_and_no_export_declares_an_excluded_command() {
    let error = match try_registry(json!({ "preamble": PREAMBLE, "export_commands": ["xMyarrow", "xOther"] })) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains(
            r"the command `\xOther` of `export_commands` of the feature `math-mathjax` must be a command that its `preamble` or one of its `packages` defines"
        ),
        "{error}"
    );
    // A command a package defines stays as it is: `\definecolor` stays out; and
    // the preamble's arrow, not exported, is unknown.
    let registry = registry(json!({ "preamble": PREAMBLE, "export_commands": ["definecolor", "frac"] }));
    for (source, command) in [(r"\(\definecolor{c}{RGB}{1,2,3} x\)", r"\definecolor"), (WITH_PREAMBLE, r"\xMyarrow")] {
        let (_, diagnostics) = text_of(&registry, source);
        let unknown = format!("core.specs.unresolvable-command: cannot resolve command ‘{command}’");
        assert_eq!(diagnostics.first(), Some(&unknown), "{source}");
    }
    assert_eq!(elements_of(&registry, r"\(\frac{a}{b}\)")[0].1, "math-mathjax.macro");
}

#[test]
fn the_browser_page_processes_the_preamble_once_with_every_package() {
    let registry = registry(json!({ "mode": "load-in-browser", "preamble": PREAMBLE, "export_commands": ["xMyarrow"] }));
    let (page, diagnostics) = page_of(&registry, WITH_PREAMBLE);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(
        page.contains(concat!(
            r#"window.MathJax.tex2mml("\\definecolor{myred}{RGB}{200,0,0} % \u003cred>\n\\Newextarrow{\\xMyarrow}{5,5}{0x21C0}", "#,
            "{display: false});\n    return pageReady();"
        )),
        "{page}"
    );
    let configuration = configuration_of(&page);
    assert!(configuration.contains("packages: ['base', 'ams', 'amscd', "), "{configuration}");
    assert!(configuration.contains(", 'verb', 'dsfont', 'texhtml', 'math-mathjax'],"), "{configuration}");
}

#[test]
fn the_registry_digest_follows_the_preamble_and_the_other_settings() {
    let digest = |data: Value| registry(data).digest();
    let red = r"\definecolor{myred}{RGB}{200,0,0}";
    // The same settings give the same digest.
    assert_eq!(digest(json!({ "preamble": red })), digest(json!({ "preamble": red })));
    // A preamble that defines nothing a formula may use by name still changes
    // the output, and the digest.
    assert_ne!(digest(json!({ "preamble": red })), digest(json!({ "preamble": r"\definecolor{myred}{RGB}{0,0,200}" })));
    // So does the font's address, which no declaration shows.
    assert_ne!(digest(json!({})), digest(json!({ "math_font_url": "fonts/math.woff2" })));
}

// --- the double brackets and `\ensuremath` -----------------------------------------------------

/// The three forms in which a template of `phfqit` writes the double angle
/// brackets: alone, after a size command, and after `\left` and `\right`.
const BRACKETS: &str = r"\(\llangle x \rrangle\) and \(\Bigl\llangle x \Bigr\rrangle\) and \(\left\llangle \frac{a}{b} \right\rrangle\)";

#[test]
fn the_double_brackets_are_math_mode_commands_of_the_extension() {
    let registry = registry(json!({}));
    let elements = elements_of(&registry, r"\(\llangle\rrangle\)");
    assert_eq!(
        elements,
        [(r"\llangle".to_string(), "math-mathjax.macro".to_string()), (r"\rrangle".to_string(), "math-mathjax.macro".to_string())]
    );
    let element = registry.elements().iter().find(|entry| entry.element.docs.contains(r"\llangle")).expect("declared");
    assert_eq!(element.element.docs, r"`\llangle`: a delimiter the math-mathjax extension defines, beyond MathJax's packages");
    // In text mode they are not declared.
    let (_, diagnostics) = text_of(&registry, r"\llangle");
    assert_eq!(diagnostics, ["core.specs.unresolvable-command: cannot resolve command ‘\\llangle’"]);
}

#[test]
fn the_double_brackets_typeset_as_mathml_alone_at_a_size_and_after_left_and_right() {
    let (page, diagnostics) = page_of(&registry(mathml()), BRACKETS);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    assert_eq!(body.matches("<math ").count(), 3, "{body}");
    assert_eq!(body.matches('\u{27EA}').count(), 3, "{body}");
    assert_eq!(body.matches('\u{27EB}').count(), 3, "{body}");
    // Alone, as MathJax writes `\langle`.
    assert!(body.contains("<mo fence=\"false\" stretchy=\"false\" lspace=\"0\" rspace=\"0\">\u{27EA}</mo><mi>x</mi><mo fence=\"false\" stretchy=\"false\" lspace=\"0\" rspace=\"0\">\u{27EB}</mo>"), "{body}");
    // At the size `\Bigl`, `\Bigr`.
    assert!(body.contains("<mo minsize=\"1.623em\" maxsize=\"1.623em\" lspace=\"0\" rspace=\"0\">\u{27EA}</mo>"), "{body}");
    // After `\left`, `\right`: the two fences around the fraction.
    assert!(body.contains("<mrow><mo lspace=\"0\" rspace=\"0\">\u{27EA}</mo><mfrac><mi>a</mi><mi>b</mi></mfrac><mo lspace=\"0\" rspace=\"0\">\u{27EB}</mo></mrow>"), "{body}");
}

#[test]
fn the_double_brackets_reach_the_browser_s_mathjax_with_their_package() {
    let (page, diagnostics) = page_of(&registry(browser()), BRACKETS);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let body = body_of(&page);
    assert!(body.contains(r#"data-flm-display="inline">\(\Bigl\llangle x \Bigr\rrangle\)</span>"#), "{body}");
    assert!(body.contains(r#"data-flm-display="inline">\(\left\llangle \frac{a}{b} \right\rrangle\)</span>"#), "{body}");
    // The page's MathJax defines the extension's package before it reads
    // its TeX configuration, and lists it.
    let configuration = configuration_of(&page);
    assert!(configuration.contains("packages: ['base', 'ams', 'texhtml', 'math-mathjax'],"), "{configuration}");
    assert!(configuration.contains("loader: {load: ['[tex]/ams', '[tex]/texhtml']},"), "{configuration}");
    assert!(
        configuration.contains(concat!(
            r"new tex.TokenMap.DelimiterMap('math-mathjax-delimiters', tex.ParseMethods.default.delimiter, ",
            r"{'\\llangle': '\u27EA', '\\rrangle': '\u27EB'});",
        )),
        "{configuration}"
    );
    assert!(configuration.contains("tex.Configuration.Configuration.create('math-mathjax',"), "{configuration}");
    assert!(configuration.contains("window.MathJax.startup.defaultReady();"), "{configuration}");
    // A page whose formulas use neither bracket neither lists nor defines it.
    let (page, _) = page_of(&registry(browser()), r"\(\langle x \rangle\)");
    assert!(!configuration_of(&page).contains("math-mathjax'"), "{page}");
    assert!(!configuration_of(&page).contains("ready"), "{page}");
}

/// A document that writes `\ensuremath` in text and inside formulas, and the
/// same document written with `\(…\)` and without the command (what the
/// renderers receive; the recomposed LaTeX keeps the command).
const ENSUREMATH: (&str, &str) = (
    r"A \ensuremath{x^2 \text{if} y} here, \(a + \ensuremath{b \text{or} c}\) and \(\text{when \ensuremath{z > 0}}\).",
    r"A \(x^2 \text{if} y\) here, \(a + b \text{or} c\) and \(\text{when \(z > 0\)}\).",
);

#[test]
fn ensuremath_is_an_inline_formula_in_text_and_its_argument_inside_a_formula_in_both_modes_on_every_backend() {
    for data in [mathml(), browser()] {
        let registry = registry(data.clone());
        let (with, diagnostics) = page_of(&registry, ENSUREMATH.0);
        assert!(diagnostics.is_empty(), "{data}: {diagnostics:?}");
        let (without, diagnostics) = page_of(&registry, ENSUREMATH.1);
        assert!(diagnostics.is_empty(), "{data}: {diagnostics:?}");
        assert_eq!(body_of(&with), body_of(&without), "{data}");
        assert!(!with.contains("ensuremath"), "{data}: {with}");
        let (with, _) = text_of(&registry, ENSUREMATH.0);
        let (without, _) = text_of(&registry, ENSUREMATH.1);
        assert_eq!(with, without, "{data}");
        // The rendered LaTeX receives each formula's TeX: in text an inline
        // formula, inside a formula the argument without the command.
        let (with, diagnostics) = rendered_of(&registry, ENSUREMATH.0);
        assert!(diagnostics.is_empty(), "{data}: {diagnostics:?}");
        let (without, _) = rendered_of(&registry, ENSUREMATH.1);
        assert_eq!(with, without, "{data}");
        assert!(!with.contains("ensuremath"), "{data}: {with}");
        // The recomposed LaTeX keeps the command as written, in text and
        // inside a formula, as the standard package alone writes it.
        let (with, diagnostics) = latex_of(&registry, ENSUREMATH.0);
        assert!(diagnostics.is_empty(), "{data}: {diagnostics:?}");
        assert!(with.contains(&format!("\n{}%%", ENSUREMATH.0)), "{data}: {with}");
        let (alone, _) = latex_of(&common::registry_with([]), ENSUREMATH.0);
        assert_eq!(with, alone, "{data}");
    }
}

#[test]
fn a_control_word_before_a_letter_that_did_not_follow_it_in_the_source_is_ended_by_a_space() {
    // `\ensuremath` is written as its argument alone: without the space, the
    // `b` would continue `\alpha` into the undefined `\alphab`.
    let source = r"\(\alpha\ensuremath{b}\)";
    let (page, diagnostics) = page_of(&registry(mathml()), source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(body_of(&page).contains("<mi>b</mi>"), "{page}");
    let (page, diagnostics) = page_of(&registry(browser()), source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(body_of(&page).contains(r#"data-flm-display="inline">\(\alpha b\)</span>"#), "{page}");
}
