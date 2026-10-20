//! The `latex-rendered` backend's core, end to end over the standard package
//! and over hand-built presentation trees: the fixed skeleton, paragraphs,
//! blocks, spans, the key-value argument, decorations at their marks, the
//! three marks, a placed tree, the four construct elements, the breaking of
//! long lines, and the style file (its header, the engine check, the base
//! packages, the core chunk, the `hyperref` hook, the author's packages and
//! text, skipped packages, conflicting options, unknown requirements, and its
//! name through the program's naming of files).
//!
//! Goldens live under `tests/goldens/latex-rendered/`: the document, then the
//! style file, each under a marker line that is itself a LaTeX comment
//! ([`golden_text`]); `tests/latex_rendered_compile.rs` compiles every one
//! with `lualatex`.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::artifact::ArtifactNamer;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::Overrides;
use flm_core::core::region::STYLE_PACKAGE;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::dump::diag::{expect, expect_none};
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::addr::TreeId;
use flm_core::model::element::ElementSpec;
use flm_core::model::names::{ElementName, Name};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::construct::{
    ARG_SLOT, DEFINITION, ENVIRONMENT, LatexArgShape, LatexDefinitionKind,
    LatexDefinitionPayload, LatexEnvironmentPayload, LatexMacroPayload, MACRO, RAW,
};
use flm_core::stdcore::backends::latex::{ExtraPreamble, LatexPackageDef, LatexRawPayload};
use flm_core::stdcore::backends::latex_rendered::{
    DOCUMENT, LatexRenderedRenderOptions, REGIONS, SKELETON_HEADER, WORKFLOW, emit_tree,
    style_artifact,
};
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::backends::pres::{
    self, BlockPayload, DecoMarkPayload, DecorationPayload, DocRole, DroppedPayload, LinkPayload,
    ParPayload, SpanPayload, SpanStyle, SubtreePayload, UnknownKind, UnknownPayload,
    UnsupportedPayload,
};
use techy::core::node::{BuildId, NodeRef};

use common::lowering::PresSite;
use common::naming::{FixedNamer, JobNamer};

// --- helpers -------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The render request every run of this file makes, with `options` set.
fn overrides(options: OptionValues) -> Overrides {
    Overrides {
        render_options: options,
        ..Overrides::none()
    }
}

/// [`overrides`] with the backend's options set as given.
fn with_options(options: LatexRenderedRenderOptions) -> Overrides {
    overrides(OptionValues::new().with(options))
}

/// Runs the workflow over one document of one tree per text.
fn render_with(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
    common::stdcore::run_workflow(registry, WORKFLOW, texts, overrides)
}

/// Runs the workflow over `text` with the default registry and options.
fn render(text: &str) -> Run {
    render_with(&registry(), &[text], &overrides(OptionValues::new()))
}

/// The entry artifact's text.
fn document(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
        panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The document's content: what stands between `\begin{document}` and
/// `\end{document}`.
fn content(run: &Run) -> String {
    let text = document(run);
    let start = text.find("\\begin{document}\n").expect("the skeleton") + "\\begin{document}\n".len();
    let end = text.rfind("\\end{document}").expect("the skeleton");
    String::from(&text[start..end])
}

/// The messages of the diagnostics of `run` reporting `id`, in order.
fn messages(run: &Run, id: &str) -> Vec<String> {
    run.diagnostics().iter().filter(|report| report.identifier() == id).map(|report| report.message()).collect()
}

/// The style file's text and its path.
fn style(run: &Run) -> (String, String) {
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let style = style_artifact(set).expect("the style file");
    let text = String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
    let path = style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
    (text, path)
}

/// The golden text of a run: the document, then the style file, each under a
/// marker line.
fn golden_text(run: &Run) -> String {
    let (style, path) = style(run);
    format!(
        "% ===== latex-rendered:document =====\n{}% ===== latex-rendered:style {path} =====\n{style}",
        document(run)
    )
}

/// Asserts the run's golden text against `tests/goldens/latex-rendered/<name>`.
fn assert_golden(name: &str, run: &Run) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/goldens/latex-rendered");
    path.push(name);
    assert_text_golden(path, &golden_text(run));
}

/// Writes a hand-built tree under `options` and returns its LaTeX, asserting
/// that nothing was reported.
fn emit_with(options: &LatexRenderedRenderOptions, build: impl FnOnce(&mut PresSite) -> Vec<BuildId>) -> String {
    let registry = registry();
    let mut site = PresSite::new(&registry);
    let children = build(&mut site);
    let tree = site.finish(children);
    let emitted = emit_tree(&tree, options);
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    emitted
        .segments
        .iter()
        .map(|segment| match segment {
            flm_core::stdcore::backends::latex_rendered::LatexSegment::Latex(latex) => latex.clone(),
            flm_core::stdcore::backends::latex_rendered::LatexSegment::Tree(tree) => format!("<tree {tree}>"),
        })
        .collect()
}

/// [`emit_with`] under the default options.
fn emit(build: impl FnOnce(&mut PresSite) -> Vec<BuildId>) -> String {
    emit_with(&LatexRenderedRenderOptions::default(), build)
}

/// A paragraph of the text `text`, with the anchor `anchor`.
fn par(site: &mut PresSite, anchor: Option<&str>, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(pres::PAR, ParPayload { anchor: anchor.map(String::from) }, vec![text])
}

/// A span of the style `style` holding `text`.
fn span(site: &mut PresSite, style: Option<SpanStyle>, text: &str) -> BuildId {
    let text = site.text(text);
    let payload = SpanPayload { style, role: None, kinds: Vec::new(), anchor: None, doc_role: None };
    site.pres(pres::SPAN, payload, vec![text])
}

// --- the fixed skeleton, paragraphs, blocks, spans ------------------------------------------

/// A document of paragraphs, blocks and spans: two theorems (blocks with a
/// role, kinds, an anchor, run-in and final decorations), emphasis, bold and
/// a reference.
const PARAGRAPHS_BLOCKS_SPANS: &str = "\
First paragraph with \\emph{emphasis}, \\textbf{bold} and \\textit{italic} text.

\\begin{theorem}[Pythagoras]\\label{thm:p}
In a right triangle the square of the hypotenuse is the sum of the squares of the two other \
sides, a statement long enough to be broken over two lines.
\\end{theorem}
\\begin{proof}[*thm:p]
See \\ref{thm:p}.
\\end{proof}

Last paragraph.";

#[test]
fn the_fixed_skeleton_is_a_complete_article_document_loading_the_style_file() {
    let run = render(PARAGRAPHS_BLOCKS_SPANS);
    expect_none(run.diagnostics().iter(), "flm.latex-rendered.unknown-requirement");
    let text = document(&run);
    let (_, path) = style(&run);
    let package = path.strip_suffix(".sty").expect("a .sty file");
    assert!(path.starts_with("flmx.") && path.len() == "flmx.12345678.sty".len(), "{path}");
    assert!(
        text.starts_with(&format!(
            "{SKELETON_HEADER}{path}\n\\documentclass{{article}}\n\\usepackage{{{package}}}\n\\begin{{document}}\n"
        )),
        "{text}"
    );
    assert!(text.ends_with("\\end{document}\n"), "{text}");
    let regions = run.table(REGIONS).expect("declared").expect("written");
    assert_eq!(regions.get(STYLE_PACKAGE).expect("the region").text(), package);
    let body = content(&run);
    assert!(body.starts_with("First paragraph with \\flmxSpan[style=emph]{emphasis}, \\flmxSpan[style=bold]{bold}"), "{body}");
    assert!(body.contains("\\begin{flmxBlock}[role=flm.theorems.theorem, kinds={theoremlike theorem}, anchor="), "{body}");
    assert!(body.contains("\\flmxDecoRunin[stop={.}]{"), "{body}");
    assert!(body.contains("\\flmxDecoFinal{"), "{body}");
    assert_golden("paragraphs-blocks-spans.tex", &run);
}

#[test]
fn a_paragraph_is_a_latex_paragraph_its_anchor_a_target_first() {
    let out = emit(|site| {
        let first = par(site, None, "One  two\nthree.");
        let second = par(site, Some("p-2"), "Anchored.");
        let empty = par(site, None, "  \n ");
        let anchored_empty = par(site, Some("p-4"), "");
        vec![first, second, empty, anchored_empty]
    });
    assert_eq!(out, "One two\nthree.\n\n\\flmxTarget{p-2}Anchored.\n\n\\flmxTarget{p-4}\n");
}

#[test]
fn blocks_nest_with_their_role_kinds_anchor_and_document_role() {
    let out = emit(|site| {
        let inner_par = par(site, None, "Inner.");
        let inner = site.pres(
            pres::BLOCK,
            BlockPayload { role: Some(Name::from("x.inner")), kinds: Vec::new(), anchor: None, doc_role: None },
            vec![inner_par],
        );
        let before = par(site, None, "Before.");
        let after = par(site, None, "After.");
        let outer = site.pres(
            pres::BLOCK,
            BlockPayload {
                role: Some(Name::from("flm.theorems.theorem")),
                kinds: vec![Name::from("theoremlike"), Name::from("theorem")],
                anchor: Some(String::from("thm-1")),
                doc_role: Some(DocRole::Endnotes),
            },
            vec![before, inner, after],
        );
        let last = par(site, None, "Last.");
        vec![outer, last]
    });
    assert_eq!(
        out,
        "\\begin{flmxBlock}[role=flm.theorems.theorem, kinds={theoremlike theorem}, anchor=thm-1, doc-role=endnotes]\n\
         Before.\n\n\
         \\begin{flmxBlock}[role=x.inner]\n\
         Inner.\n\
         \\end{flmxBlock}\n\n\
         After.\n\
         \\end{flmxBlock}\n\n\
         Last.\n"
    );
}

#[test]
fn a_span_of_every_style_writes_its_style_and_a_span_with_role_and_kinds_writes_them() {
    let out = emit(|site| {
        let mut children = Vec::new();
        for style in SpanStyle::ALL {
            children.push(span(site, Some(style), style.as_str()));
            children.push(site.text(" "));
        }
        let inner = site.text("named");
        children.push(site.pres(
            pres::SPAN,
            SpanPayload {
                style: Some(SpanStyle::Bold),
                role: Some(Name::from("flm.theorems.name")),
                kinds: vec![Name::from("a"), Name::from("b")],
                anchor: Some(String::from("s-1")),
                doc_role: Some(DocRole::Footnote),
            },
            vec![inner],
        ));
        vec![site.pres(pres::PAR, ParPayload { anchor: None }, children)]
    });
    assert_eq!(
        out,
        "\\flmxSpan[style=emph]{emph} \\flmxSpan[style=bold]{bold} \\flmxSpan[style=italic]{italic} \
         \\flmxSpan[style=monospace]{monospace}\n\
         \\flmxSpan[style=underline]{underline} \\flmxSpan[style=small-caps]{small-caps} \
         \\flmxSpan[style=superscript]{superscript}\n\
         \\flmxSpan[style=subscript]{subscript} \\flmxSpan[style=unresolved-reference]{unresolved-reference} \
         \\flmxSpan[style=bold, role=flm.theorems.name, kinds={a b}, anchor=s-1, doc-role=footnote]{named}\n"
    );
}

// --- the key-value argument -------------------------------------------------------------------

#[test]
fn a_value_is_braced_where_it_must_be_and_an_empty_key_is_left_out() {
    let out = emit(|site| {
        let content = site.text("text");
        let link = site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("https://example.org/a b?x=1&y=2#top~%"),
                kind: Name::from("url"),
                doc_role: None,
                label: Some(String::from("a, b] 50% #1 {c}")),
                shows_target: false,
            },
            vec![content],
        );
        let plain = span(site, None, "plain");
        let odd = site.text("odd");
        let odd = site.pres(
            pres::SPAN,
            SpanPayload {
                style: None,
                role: None,
                kinds: vec![Name::from("x,y")],
                anchor: Some(String::from("a]b")),
                doc_role: None,
            },
            vec![odd],
        );
        let space = site.text(" ");
        let space2 = site.text(" ");
        vec![site.pres(pres::PAR, ParPayload { anchor: None }, vec![link, space, plain, space2, odd])]
    });
    assert_eq!(
        out,
        "\\flmxLink[kind=url, href={https://example.org/a b?x=1&y=2\\#top\\string~\\%}, \
         label={a, b] 50\\% \\#1 \\{c\\}}]{text}\n\\flmxSpan{plain} \
         \\flmxSpan[kinds={{x,y}}, anchor={a]b}]{odd}\n"
    );
}

// --- decorations and marks ------------------------------------------------------------------

/// A paragraph carrying a decoration of the element `deco` holding `text`,
/// its mark standing after `before`.
fn decorated(site: &mut PresSite, deco: &str, text: &str, before: &str, after: &str) -> BuildId {
    let deco_text = site.text(text);
    let decoration = site.pres(deco, DecorationPayload, vec![deco_text]);
    let before = site.text(before);
    let mark = site.pres(pres::DECO_MARK, DecoMarkPayload { slot: 1 }, Vec::new());
    let after = site.text(after);
    site.pres_attached(pres::PAR, ParPayload { anchor: None }, vec![before, mark, after], vec![(deco, decoration)])
}

#[test]
fn a_decoration_of_each_kind_is_written_at_its_mark() {
    let out = emit(|site| {
        vec![
            decorated(site, pres::DECO_RUNIN, "Theorem 1", "", "Text."),
            decorated(site, pres::DECO_RUNIN, "Why?", "", "Text."),
            decorated(site, pres::DECO_FINAL, "QED", "Last line.", ""),
            decorated(site, pres::DECO_MARGIN, "AB", "Commented ", "text."),
        ]
    });
    assert_eq!(
        out,
        "\\flmxDecoRunin[stop={.}]{Theorem 1}Text.\n\n\
         \\flmxDecoRunin{Why?}Text.\n\n\
         Last line.\\flmxDecoFinal{QED}\n\n\
         Commented \\flmxDecoMargin{AB}text.\n"
    );
    let options = LatexRenderedRenderOptions { run_in_stop: String::new(), ..Default::default() };
    let out = emit_with(&options, |site| vec![decorated(site, pres::DECO_RUNIN, "Lemma", "", "Text.")]);
    assert_eq!(out, "\\flmxDecoRunin{Lemma}Text.\n");
}

#[test]
fn the_three_marks_write_their_arguments_as_text() {
    let out = emit(|site| {
        let dropped = site.pres(
            pres::DROPPED,
            DroppedPayload { condition: Name::from("x.left-out"), detail: String::from("content outside \\cell") },
            Vec::new(),
        );
        let unsupported = site.pres(
            pres::UNSUPPORTED,
            UnsupportedPayload { element: ElementName::from("x.box"), message: String::from("no rule") },
            Vec::new(),
        );
        let unknown = site.pres(
            pres::UNKNOWN,
            UnknownPayload { kind: UnknownKind::Macro, name: String::from("foo"), spelling: String::from("\\foo") },
            Vec::new(),
        );
        let space = site.text(" ");
        let space2 = site.text(" ");
        vec![site.pres(pres::PAR, ParPayload { anchor: None }, vec![dropped, space, unsupported, space2, unknown])]
    });
    assert_eq!(
        out,
        "\\flmxDropped{x.left-out}{content outside \\textbackslash{}cell} \
         \\flmxUnsupported{x.box}{no rule} \\flmxUnknown[kind=macro]{foo}{\\textbackslash{}foo}\n"
    );
}

// --- line breaking ------------------------------------------------------------------------

#[test]
fn a_long_line_is_broken_at_a_space_of_text_and_never_inside_an_argument() {
    let words = "word ".repeat(60);
    let out = emit(|site| {
        let text = site.text(&words);
        let content = site.text("x");
        let link = site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("#a"),
                kind: Name::from("ref"),
                doc_role: None,
                label: Some(words.clone()),
                shows_target: false,
            },
            vec![content],
        );
        vec![site.pres(pres::PAR, ParPayload { anchor: None }, vec![text, link])]
    });
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 3, "{out}");
    for line in &lines[..lines.len() - 1] {
        assert!(line.len() <= 101 + "word".len(), "{line}");
        assert!(!line.ends_with(' '), "{line:?}");
    }
    let last = lines.last().expect("a line");
    // The key-value argument is never broken, so the last line holds the
    // whole link, however long its label.
    assert!(last.contains("\\flmxLink[kind=ref, href={\\#a}, target=a, label={word word"), "{last}");
    assert!(last.ends_with("word }]{x}"), "{last}");
}

// --- a test feature: placed trees, constructs, requirements ------------------------------------

/// What the rule of an element of the test feature stages.
struct XRule(&'static str);

impl Lower for XRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let staged = match self.0 {
            "mac" => {
                let body = lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new("\\fbox{#1}"), Vec::new())?;
                let definition = lw.pres_slots_with(
                    cx,
                    &ElementName::from(DEFINITION),
                    LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "xbox", 1),
                    vec![(String::from(pres::BODY_SLOT), vec![body])],
                )?;
                let content = lw.lower_arg_named(cx, &node, "content")?;
                let call = lw.pres_slots_with(
                    cx,
                    &ElementName::from(MACRO),
                    LatexMacroPayload::new("xbox", [LatexArgShape::Mandatory]),
                    vec![(String::from(ARG_SLOT), content)],
                )?;
                vec![definition, call]
            }
            "opt" => {
                let width = lw.text(cx, "3cm")?;
                let content = lw.lower_arg_named(cx, &node, "content")?;
                vec![lw.pres_slots_with(
                    cx,
                    &ElementName::from(MACRO),
                    LatexMacroPayload::new("makebox", [LatexArgShape::Optional, LatexArgShape::Mandatory]),
                    vec![(String::from(ARG_SLOT), vec![width]), (String::from(ARG_SLOT), content)],
                )?]
            }
            "env" => {
                let content = lw.lower_arg_named(cx, &node, "content")?;
                vec![lw.pres_slots_with(
                    cx,
                    &ElementName::from(ENVIRONMENT),
                    LatexEnvironmentPayload::new("center", []),
                    vec![(String::from(pres::BODY_SLOT), content)],
                )?]
            }
            "rawhere" => {
                vec![lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new("\\hfill"), Vec::new())?]
            }
            "word" => vec![lw.pres_slots_with(
                cx,
                &ElementName::from(MACRO),
                LatexMacroPayload::new("LaTeX", []),
                Vec::new(),
            )?],
            "placefirst" => {
                vec![lw.pres_with(cx, &ElementName::from(pres::SUBTREE), SubtreePayload { tree: TreeId::tree(0) }, Vec::new())?]
            }
            "place" => {
                vec![lw.pres_with(cx, &ElementName::from(pres::SUBTREE), SubtreePayload { tree: TreeId::tree(1) }, Vec::new())?]
            }
            "needy" => {
                lw.require(&Name::from("x.nothing"));
                lw.require(&Name::from("flm.latex.def.nothing"));
                lw.require(&Name::from("flm.latex.pkg.nothing"));
                lw.require(&Name::from("flm.theorems.style"));
                vec![lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new("\\relax"), Vec::new())?]
            }
            other => unreachable!("{other}"),
        };
        Ok(Lowering::Emit(staged))
    }
}

/// The test feature `x`: `\mac{…}` (a definition and its call), `\opt{…}`
/// (a macro with an optional and a mandatory argument), `\env{…}` (an
/// environment), `\rawhere` (a raw piece), `\word` (`\LaTeX`, a macro
/// without arguments), `\place` (the second tree placed), `\placefirst` (the
/// first tree placed)
/// and `\needy` (requirements no feature defines).
fn x_feature() -> Arc<dyn flm_core::core::registry::FlmFeature> {
    common::pipeline::feature("x", |b| {
        for (name, content, produces) in [
            ("mac", true, vec![DEFINITION, MACRO, RAW]),
            ("opt", true, vec![MACRO]),
            ("env", true, vec![ENVIRONMENT]),
            ("rawhere", false, vec![RAW]),
            ("word", false, vec![MACRO]),
            ("place", false, vec![pres::SUBTREE]),
            ("placefirst", false, vec![pres::SUBTREE]),
            ("needy", false, vec![RAW]),
        ] {
            let args = if content { vec![ArgDef::mandatory("content")] } else { Vec::new() };
            let element = format!("x.{name}");
            b.element(ElementDef::new(
                "x",
                [name],
                SyntaxDef::Macro { args },
                Arc::new(ElementSpec::new(element.as_str()).docs("a test element")),
            ))?;
            let mut decl = LoweringDecl::new();
            for produced in produces {
                decl = decl.produces(produced);
            }
            b.lowering_rule(LoweringRule::new(
                format!("{element}.lower.default").as_str(),
                element.as_str(),
                decl,
                Arc::new(XRule(name)),
                DefaultMark::Everywhere,
            ))?;
        }
        Ok(())
    })
}

/// The standard package with the test feature.
fn x_registry() -> Arc<FlmRegistry> {
    common::spine::registry_with(vec![x_feature()])
}

#[test]
fn a_placed_tree_is_written_where_it_is_placed_and_not_again() {
    let run = render_with(&x_registry(), &["Before \\place{} after.", "Placed."], &overrides(OptionValues::new()));
    expect_none(run.diagnostics().iter(), "flm.latex-rendered.missing-subtree");
    assert_eq!(content(&run), "Before\n\nPlaced.\n\nafter.\n");
}

#[test]
fn trees_placing_each_other_are_written_at_the_top_level_and_the_cycle_reported() {
    let run = render_with(&x_registry(), &["A \\place{} a.", "B \\placefirst{} b."], &overrides(OptionValues::new()));
    assert_eq!(
        messages(&run, "flm.latex-rendered.subtree-cycle"),
        vec![String::from(
            "the document tree `tree#0` must not be placed inside itself, but it is, directly or through other trees"
        )],
        "{}",
        run.diagnostics().render_all()
    );
    // The placement that closes the cycle writes nothing where it stands.
    assert_eq!(content(&run), "A\n\nB\n\n\nb.\n\na.\n");
}

#[test]
fn a_macro_without_arguments_is_kept_apart_from_a_letter_that_follows() {
    let run = render_with(
        &x_registry(),
        &["\\word{}is, \\word{} and \\word{}é, \\word{}.\\word{}\\word{}"],
        &overrides(OptionValues::new()),
    );
    assert_eq!(run.diagnostics().iter().count(), 0, "{}", run.diagnostics().render_all());
    assert_eq!(content(&run), "\\LaTeX is, \\LaTeX and \\LaTeX é, \\LaTeX.\\LaTeX\\LaTeX\n");
}

#[test]
fn the_construct_elements_are_written_natively_and_a_definition_once_in_the_style_file() {
    let run = render_with(
        &x_registry(),
        &["A \\mac{boxed} and \\mac{again}, \\opt{wide}\\rawhere{} end.\n\n\\env{Centered.}\n\nAfter."],
        &overrides(OptionValues::new()),
    );
    assert_eq!(run.diagnostics().iter().count(), 0, "{}", run.diagnostics().render_all());
    assert_eq!(
        content(&run),
        "A \\xbox{boxed} and \\xbox{again}, \\makebox[3cm]{wide}\\hfill end.\n\n\
         \\begin{center}\n\
         Centered.\n\
         \\end{center}\n\n\
         After.\n"
    );
    let (style, _) = style(&run);
    assert_eq!(style.matches("\\providecommand\\xbox[1]{\\fbox{#1}}\n").count(), 1, "{style}");
    assert!(style.contains("% the definitions the document's rules staged\n\\providecommand"), "{style}");
    assert_golden("constructs.tex", &run);
}

// --- the style file ---------------------------------------------------------------------------

#[test]
fn the_style_file_holds_the_header_the_engine_check_the_base_packages_the_core_and_hyperref() {
    let run = render("Text.");
    let (style, path) = style(&run);
    let name = path.strip_suffix(".sty").expect("a .sty file");
    let expected_start = format!(
        "\\NeedsTeXFormat{{LaTeX2e}}[2022/06/01]\n\
         \\ProvidesPackage{{{name}}}\n\
         % the definitions of a document written by FLM's latex-rendered backend\n\
         % the engine: LuaLaTeX\n\\RequirePackage{{iftex}}\n\\RequireLuaTeX\n\
         % the packages\n\
         % flm.latex.pkg.fontspec\n\\RequirePackage{{fontspec}}\n\
         % flm.latex.pkg.amsmath\n\\RequirePackage{{amsmath}}\n\
         % flm.latex.pkg.xcolor\n\\RequirePackage{{xcolor}}\n\
         % hyperref, loaded at the end of the preamble\n\
         \\AddToHook{{begindocument/before}}{{\\RequirePackage[colorlinks=true, linkcolor=flmxLinkColor, \
         urlcolor=flmxUrlColor, citecolor=flmxCiteColor]{{hyperref}}}}\n\
         % flm.latex.def.flmx.core\n"
    );
    assert!(style.starts_with(&expected_start), "{style}");
    assert!(style.ends_with("\\endinput\n"), "{style}");
    assert!(!style.contains("\\ifpdftex") && !style.contains("\\iftutex"), "no engine test");
    assert!(!style.contains("flm.latex.def.flmx.marks"), "the marks chunk only where a mark is written");
    // The hash is of the file without its header.
    let body = &style[style.find("% the engine").expect("the body")..];
    let hash = &name["flmx.".len()..];
    assert_eq!(hash, flm_core::stdcore::backends::latex::style::content_hash(body));
}

#[test]
fn the_author_s_packages_and_text_are_written_and_a_skipped_package_left_out() {
    let options = LatexRenderedRenderOptions {
        skip_packages: ["xcolor".to_string(), "hyperref".to_string()].into_iter().collect(),
        extra_packages: vec![
            LatexPackageDef::new("geometry", "the page").options(["a4paper", "margin=2cm"]),
            LatexPackageDef::new("xcolor", "the author's colors").options(["dvipsnames"]),
        ],
        extra_preamble: ExtraPreamble {
            before_packages: String::from("\\PassOptionsToPackage{hyphens}{url}"),
            after_packages: String::from("\\usepackage{hyperref}\n"),
            at_end: String::from("\\renewcommand\\familydefault{\\sfdefault}"),
        },
        ..Default::default()
    };
    let run = render_with(&registry(), &["Text."], &with_options(options));
    let (style, _) = style(&run);
    assert!(style.contains(
        "\\RequireLuaTeX\n% the author's text before the packages\n\\PassOptionsToPackage{hyphens}{url}\n% the packages\n"
    ), "{style}");
    assert!(style.contains(
        "% flm.latex.pkg.amsmath\n\\RequirePackage{amsmath}\n\
         % flm.latex.pkg.geometry\n\\RequirePackage[a4paper,margin=2cm]{geometry}\n\
         % flm.latex.pkg.xcolor\n\\RequirePackage[dvipsnames]{xcolor}\n\
         % the author's text after the packages\n\\usepackage{hyperref}\n% flm.latex.def.flmx.core\n"
    ), "{style}");
    assert!(!style.contains("\\AddToHook"), "{style}");
    assert!(style.ends_with("% the author's text at the end\n\\renewcommand\\familydefault{\\sfdefault}\n\\endinput\n"), "{style}");
    assert_golden("style-options.tex", &run);
}

#[test]
fn two_values_for_one_option_are_an_error_and_the_package_is_not_requested() {
    let options = LatexRenderedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("geometry", "a").options(["papersize=a4"]),
            LatexPackageDef::new("geometry", "b").options(["papersize=letter"]),
        ],
        ..Default::default()
    };
    let run = render_with(&registry(), &["Text."], &with_options(options));
    expect(run.diagnostics().iter(), "flm.latex-rendered.conflicting-package-options").count(1);
    let reports = messages(&run, "flm.latex-rendered.conflicting-package-options");
    assert_eq!(
        reports,
        ["two requests for the LaTeX package `geometry` give the option `papersize` the values \
          `papersize=a4` and `papersize=letter`"]
    );
    assert!(!style(&run).0.contains("{geometry}"));
}

#[test]
fn an_identifier_known_nowhere_is_an_error_and_a_stylesheet_is_another_backend_s() {
    let run = render_with(&x_registry(), &["A \\needy{} b"], &overrides(OptionValues::new()));
    expect(run.diagnostics().iter(), "flm.latex-rendered.unknown-requirement").count(3);
    let mut reports = messages(&run, "flm.latex-rendered.unknown-requirement");
    reports.sort();
    assert_eq!(
        reports,
        [
            "the requirement `flm.latex.def.nothing` names no LaTeX package request, no definition \
             piece, and no style or script definition",
            "the requirement `flm.latex.pkg.nothing` names no LaTeX package request, no definition \
             piece, and no style or script definition",
            "the requirement `x.nothing` names no LaTeX package request, no definition piece, and \
             no style or script definition",
        ]
    );
    assert!(content(&run).contains("A \\relax b"), "{}", content(&run));
}

/// The standard package with `namer` contributed to the naming of files.
fn registry_naming(namer: Arc<dyn ArtifactNamer>) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(NAMER_SLOT, namer);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

#[test]
fn the_style_file_is_named_through_the_naming_of_files_or_refused_where_not_loadable() {
    let run = render_with(&registry_naming(Arc::new(JobNamer)), &["Text."], &overrides(OptionValues::new()));
    let (text, path) = style(&run);
    let hash = path.strip_prefix("job-flmx.").and_then(|rest| rest.strip_suffix(".sty")).expect("job-flmx.HASH.sty");
    assert_eq!(hash.len(), 8);
    assert!(text.contains(&format!("\\ProvidesPackage{{job-flmx.{hash}}}")), "{text}");
    assert!(document(&run).contains(&format!("\\usepackage{{job-flmx.{hash}}}")));
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    assert_eq!(style_artifact(set).expect("the style file").id.to_string(), format!("latex-rendered:flmx.{hash}"));

    let run = render_with(&registry_naming(Arc::new(FixedNamer("files/x.sty"))), &["Text."], &overrides(OptionValues::new()));
    expect(run.diagnostics().iter(), "flm.latex-rendered.style-path").count(1);
    assert_eq!(
        messages(&run, "flm.latex-rendered.style-path"),
        ["the naming of files places the style file at `files/x.sty`, which is not a bare file name \
          ending in `.sty`; the document loads the style file by its package name"]
    );
    let (_, path) = style(&run);
    assert!(path.starts_with("flmx.") && path.ends_with(".sty"), "{path}");
}

// --- the rendering guide's copy of the contract ---------------------------------------------

#[test]
fn the_rendering_guide_holds_the_same_contract_table() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("rendering.md");
    let Ok(guide) = std::fs::read_to_string(&path) else {
        // A packaged crate ships no guide beside its sources.
        eprintln!("skipped: {} is not present", path.display());
        return;
    };
    let table = flm_core::stdcore::backends::latex_rendered::contract::print_rows();
    assert!(
        guide.contains(table.trim_end()),
        "`docs/rendering.md` does not hold the contract table as `contract::print_rows()` \
         prints it"
    );
}
