//! Lists, lines, verbatim text and images on the `latex-rendered` backend:
//! `flmxList` and `\flmxItem` for lists of every structural kind, nested,
//! with anchors, labels holding markup, items with paragraphs and a verbatim
//! block, and the `depth` key; `flmxLines` and `\flmxLine` with indentations,
//! alignments and every length unit; `\flmxVerbatim` and `flmxVerbatimBlock`
//! of every kind with every special character, a tab and an empty line;
//! `\flmxImage` with every key, published by `reference` and by `copy`, and
//! `data-uri` refused.
//!
//! Goldens live under `tests/goldens/latex-rendered/`, in the shape of
//! `tests/latex_rendered.rs` (the document, then the style file, each under a
//! marker line); `tests/latex_rendered_compile.rs` compiles every one with
//! `lualatex`.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::artifact::{ArtifactNamer, ArtifactRef, NamingScope, RelPath};
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ElementDef, SyntaxDef};
use flm_core::model::element::ElementSpec;
use flm_core::model::names::ElementName;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::latex_rendered::{
    BACKEND, DOCUMENT, LatexRenderedRenderOptions, LatexSegment, WORKFLOW, emit_tree, style_artifact,
};
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::backends::pres::{
    self, ImageDisplay, ImagePayload, Length, LineAlign, LinePayload, LinesPayload, ParPayload,
};
use flm_core::stdcore::features::graphics::{
    self, GraphicsConfig, GraphicsRenderOptions, GraphicsResolveError, GraphicsResolveRequest,
    GraphicsResolver, ImageFile, RESOLVER_SLOT, Strategy,
};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::core::node::NodeRef;

use common::lowering::PresSite;

// --- helpers -------------------------------------------------------------------------------

/// The standard package, `\includegraphics` an ordinary command.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().graphics(GraphicsConfig::new().global(true)))
}

/// The render request with `options` set.
fn overrides(options: OptionValues) -> Overrides {
    Overrides {
        render_options: options,
        ..Overrides::none()
    }
}

/// Runs the workflow over `text` with `registry` and `overrides`, asserting
/// that nothing was reported.
fn render_with(registry: &Arc<FlmRegistry>, text: &str, overrides: &Overrides) -> Run {
    let run = common::stdcore::run_workflow(registry, WORKFLOW, &[text], overrides);
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    run
}

/// Runs the workflow over `text` with `registry` and `overrides`, asserting
/// that no Error was reported: an image given a width or a height is
/// reported as a Warning (an image file should be prepared at its size).
fn render_images(registry: &Arc<FlmRegistry>, text: &str, overrides: &Overrides) -> Run {
    let run = common::stdcore::run_workflow(registry, WORKFLOW, &[text], overrides);
    assert!(!run.diagnostics().has_errors(), "{}", run.diagnostics().render_all());
    run
}

/// Runs the workflow over `text` with the standard package.
fn render(text: &str) -> Run {
    render_with(&registry(), text, &overrides(OptionValues::new()))
}

/// The entry artifact's text.
fn document(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
        panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The content of the document: what stands between `\begin{document}` and
/// `\end{document}`.
fn content(run: &Run) -> String {
    let text = document(run);
    let start = text.find("\\begin{document}\n").expect("the skeleton") + "\\begin{document}\n".len();
    let end = text.rfind("\\end{document}").expect("the skeleton");
    String::from(&text[start..end])
}

/// Asserts the run's document and style file against
/// `tests/goldens/latex-rendered/<name>`, each under a marker line.
fn assert_golden(name: &str, run: &Run) {
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let style = style_artifact(set).expect("the style file");
    let style_text = String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
    let path = style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
    let text = format!(
        "% ===== latex-rendered:document =====\n{}% ===== latex-rendered:style {path} =====\n{style_text}",
        document(run)
    );
    let mut golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    golden.push("tests/goldens/latex-rendered");
    golden.push(name);
    assert_text_golden(golden, &text);
}

/// Writes a hand-built tree under the default options and returns its
/// LaTeX, asserting that nothing was reported.
fn emit(build: impl FnOnce(&mut PresSite) -> Vec<techy::core::node::BuildId>) -> String {
    let registry = registry();
    let mut site = PresSite::new(&registry);
    let children = build(&mut site);
    let tree = site.finish(children);
    let emitted = emit_tree(&tree, &LatexRenderedRenderOptions::default());
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    emitted
        .segments
        .iter()
        .map(|segment| match segment {
            LatexSegment::Latex(latex) => latex.clone(),
            LatexSegment::Tree(tree) => format!("<tree {tree}>"),
        })
        .collect()
}

// --- lists ---------------------------------------------------------------------------------

/// Lists of the three structural kinds and a labeled one, nested to three
/// levels, items with labels holding markup, items that are reference
/// targets, an item of two paragraphs and a verbatim block.
const LISTS: &str = "\
\\section{Lists}

\\begin{enumerate}
\\item\\label{item:first} First, before \\ref{item:term}.
\\item Second, holding a list:
\\begin{itemize}
\\item Inner.
\\item[{\\emph{Rich} tag}] A tag with markup.
\\item Deeper:
\\begin{itemize}
\\item Third level.
\\end{itemize}
\\end{itemize}
\\item Two paragraphs, the first.

The second, then a block:
\\begin{verbatimcode}
let x = {1};
\\end{verbatimcode}
\\end{enumerate}

\\begin{description}
\\item[Entropy]\\label{item:term} A measure.
\\item[Energy] Another.
\\end{description}

\\begin{enumerate}[(i)]
\\item Roman.
\\item See \\ref{item:first}.
\\end{enumerate}";

#[test]
fn lists_write_their_kinds_depth_and_items_with_labels_and_anchors() {
    let run = render(LISTS);
    let latex = content(&run);
    assert!(latex.contains("\\begin{flmxList}[kinds={numbered enumerate labeled}, depth=1]\n"), "{latex}");
    assert!(latex.contains("\\begin{flmxList}[kinds={unnumbered itemize labeled}, depth=2]\n"), "{latex}");
    assert!(latex.contains("\\begin{flmxList}[kinds={unnumbered itemize labeled}, depth=3]\n"), "{latex}");
    assert!(latex.contains("\\begin{flmxList}[kinds={terms description labeled}, depth=1]\n"), "{latex}");
    // Each item on a line of its own, its content on the lines after it, a
    // blank line before the next item.
    assert!(latex.contains("\\flmxItem[label={\\flmxSpan[style=emph]{Rich} tag}, anchor="), "{latex}");
    assert!(latex.contains("]\nA tag with markup.\n\n\\flmxItem[label="), "{latex}");
    assert!(latex.contains("\n\n\\flmxItem[label={Energy}, anchor="), "{latex}");
    // The item's anchor, and the reference to it.
    assert!(latex.contains("\\flmxItem[label={Entropy}, anchor=item-term]\nA measure.\n"), "{latex}");
    assert!(latex.contains("\\flmxLink[kind=ref, href={\\#item-term}, target=item-term]{Entropy}"), "{latex}");
    // An item of two paragraphs and a verbatim block.
    assert!(latex.contains("The second, then a block:\n\n\\begin{flmxVerbatimBlock}[kind=code]\n"), "{latex}");
    assert_golden("lists.tex", &run);
}

// --- lines ---------------------------------------------------------------------------------

/// The lowering rule of the test element `\ylines`: lines of every length
/// unit and alignment.
struct YLines;

impl Lower for YLines {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let length = |text: &str| Length::parse(text).expect("a length");
        let mut lines = Vec::new();
        for (left, right, align, text) in [
            ("0tab", "0tab", LineAlign::Left, "No indentation"),
            ("1tab", "0tab", LineAlign::Left, "One tab"),
            ("0.5tab", "0em", LineAlign::Left, "Half a tab"),
            ("1.5em", "2em", LineAlign::Center, "Centered, one and a half em in"),
            ("10pt", "0pt", LineAlign::Right, "Flush right, ten points in"),
            ("3mm", "1cm", LineAlign::Left, "Millimeters and centimeters"),
            ("0.25in", "0in", LineAlign::Center, "A quarter inch in, centered"),
        ] {
            let text = lw.text(cx, text)?;
            let payload = LinePayload { indent_left: length(left), indent_right: length(right), align };
            lines.push(lw.pres_with(cx, &ElementName::from(pres::LINE), payload, vec![text])?);
        }
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::LINES), LinesPayload, lines)?]))
    }
}

/// The standard package with the test feature `y`, whose `\ylines` writes
/// [`YLines`].
fn y_registry() -> Arc<FlmRegistry> {
    let feature = common::pipeline::feature("y", |b| {
        b.element(ElementDef::new(
            "y",
            ["ylines"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(ElementSpec::new("y.lines").docs("a test element")),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "y.lines.lower.default",
            "y.lines",
            LoweringDecl::new().produces(pres::LINES).produces(pres::LINE),
            Arc::new(YLines),
            DefaultMark::Everywhere,
        ))?;
        Ok(())
    });
    common::spine::registry_with(vec![feature])
}

#[test]
fn lines_write_their_indentations_in_every_unit_and_their_alignment() {
    let run = render_with(&y_registry(), "Before.\n\n\\ylines\n\nAfter.", &overrides(OptionValues::new()));
    let latex = content(&run);
    for expected in [
        "\\begin{flmxLines}\n",
        "\\flmxLine[indent-left=0\\flmxTabWidth, indent-right=0\\flmxTabWidth, align=left]{No indentation}\n",
        "\\flmxLine[indent-left=1\\flmxTabWidth, indent-right=0\\flmxTabWidth, align=left]{One tab}\n",
        "\\flmxLine[indent-left=0.5\\flmxTabWidth, indent-right=0em, align=left]{Half a tab}\n",
        "\\flmxLine[indent-left=1.5em, indent-right=2em, align=center]{Centered, one and a half em in}\n",
        "\\flmxLine[indent-left=10bp, indent-right=0bp, align=right]{Flush right, ten points in}\n",
        "\\flmxLine[indent-left=3mm, indent-right=1cm, align=left]{Millimeters and centimeters}\n",
        "\\flmxLine[indent-left=0.25in, indent-right=0in, align=center]{A quarter inch in, centered}\n",
        "\\end{flmxLines}\n",
    ] {
        assert!(latex.contains(expected), "{expected} in\n{latex}");
    }
    assert_golden("lines.tex", &run);
}

#[test]
fn the_lines_of_a_quotation_write_their_tabs() {
    let run = render("\\begin{quote}\\lines{First line\\\\ \\indent Second line\\\\ \\indent\\indent Third}\\end{quote}");
    let latex = content(&run);
    assert!(latex.contains("\\flmxLine[indent-left=1\\flmxTabWidth, indent-right=0\\flmxTabWidth, align=left]{Second line}"), "{latex}");
    assert!(latex.contains("\\flmxLine[indent-left=2\\flmxTabWidth, indent-right=0\\flmxTabWidth, align=left]{Third}"), "{latex}");
}

// --- verbatim ------------------------------------------------------------------------------

/// Inline verbatim text of the three kinds, with every special character and
/// runs of spaces, and a block holding a tab, an empty line and every
/// special character.
const VERBATIM: &str = "\
Text \\verbtext|{}%&#_$\\~^ x  y| and code \\verbcode[rust]{let x = 1;} and an identifier \\verba{my_name}.
Inline text \\begin{verbatimtext}a  b\\end{verbatimtext} here.

\\begin{verbatimcode}[python]
def f(x):
\treturn {'a': x % 2, \"b\": x & 1}  # ~^_$\\

    pass
\\end{verbatimcode}

After.";

#[test]
fn verbatim_text_is_escaped_inline_and_in_a_block_line_by_line() {
    let run = render(VERBATIM);
    let latex = content(&run);
    assert!(
        latex.contains(
            "\\flmxVerbatim[kind=text]{\\{\\}\\%\\&\\#\\_\\$\\textbackslash{}\\textasciitilde{}\\textasciicircum{}\\ x\\ \\ y}"
        ),
        "{latex}"
    );
    assert!(latex.contains("\\flmxVerbatim[kind=code, lang=rust]{let\\ x\\ =\\ 1;}"), "{latex}");
    assert!(latex.contains("\\flmxVerbatim[kind=a]{my\\_name}"), "{latex}");
    assert!(latex.contains("\\flmxVerbatim[kind=text]{a\\ \\ b}"), "{latex}");
    assert!(latex.contains("\\begin{flmxVerbatimBlock}[kind=code, lang=python]\n\\flmxVerbLine{def\\ f(x):}\n"), "{latex}");
    assert!(latex.contains("\\flmxVerbLine{\\ \\ \\ \\ return\\ \\{'a':\\ x\\ \\%\\ 2,"), "{latex}");
    assert!(latex.contains("\\#\\ \\textasciitilde{}\\textasciicircum{}\\_\\$\\textbackslash{}}\n\\flmxVerbLine{}\n"), "{latex}");
    assert!(latex.contains("\\flmxVerbLine{\\ \\ \\ \\ pass}\n\\end{flmxVerbatimBlock}\n"), "{latex}");
    assert_golden("verbatim.tex", &run);
}

#[test]
fn a_newline_in_inline_verbatim_is_a_space() {
    let run = render("A \\begin{verbatimtext}one\ntwo\\end{verbatimtext} b.");
    let latex = content(&run);
    assert!(latex.contains("\\flmxVerbatim[kind=text]{one\\ two}"), "{latex}");
}

// --- images --------------------------------------------------------------------------------

/// Images of every key the graphics feature writes: a block image in a
/// figure with its width, height and alternative text, and inline images
/// with one dimension.
const IMAGES: &str = "\
\\begin{figure}
\\includegraphics[width=4.5cm, height=3cm, alt={A plot}]{figures/plot.png}
\\caption{A plot.}\\label{figure:plot}
\\end{figure}

An icon \\includegraphics[height=4mm]{icon.png} inline, and \\includegraphics[width=12pt]{figures/plot.png} again.";

/// A resolver over files held in memory, by the path as written.
#[derive(Default)]
struct MemoryFiles {
    files: BTreeMap<String, (&'static str, Vec<u8>)>,
}

impl GraphicsResolver for MemoryFiles {
    fn resolve(&self, request: &GraphicsResolveRequest) -> Result<ImageFile, GraphicsResolveError> {
        match self.files.get(&request.path) {
            Some((media_type, _)) => Ok(ImageFile::new(format!("mem:{}", request.path), *media_type, &request.path)),
            None => Err(GraphicsResolveError::new("no such file")),
        }
    }

    fn read(&self, file: &ImageFile) -> Result<Vec<u8>, GraphicsResolveError> {
        let path = file.identity.strip_prefix("mem:").unwrap_or(&file.identity);
        self.files.get(path).map(|(_, content)| content.clone()).ok_or_else(|| GraphicsResolveError::new("gone"))
    }
}

/// A namer placing every copy of an image in `files/`, named
/// `<name>.<extension>`, and every other artifact at `<name>.<extension>`.
struct FolderNamer;

impl ArtifactNamer for FolderNamer {
    fn url_of(&self, artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
        let extension = artifact.extension.unwrap_or("bin");
        let folder = if artifact.id.owner.as_str() == graphics::FEATURE { "files/" } else { "" };
        RelPath::new(format!("{folder}{}.{extension}", artifact.id.name))
    }
}

/// The standard package with the two images in memory and the folder namer.
fn files_registry() -> Arc<FlmRegistry> {
    let mut files = MemoryFiles::default();
    files.files.insert(String::from("figures/plot.png"), ("image/png", b"PNG-plot".to_vec()));
    files.files.insert(String::from("icon.png"), ("image/png", b"PNG-icon".to_vec()));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().graphics(GraphicsConfig::new().global(true))));
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(RESOLVER_SLOT, Arc::new(files) as Arc<dyn GraphicsResolver>);
    builder.contribute_to(NAMER_SLOT, Arc::new(FolderNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// The render request publishing this backend's images by `strategy`.
fn strategy(strategy: Strategy) -> Overrides {
    let mut chosen = GraphicsRenderOptions::default();
    chosen.strategy.insert(BACKEND.to_string(), strategy);
    overrides(OptionValues::new().with(chosen))
}

#[test]
fn images_by_reference_write_the_path_as_written_and_every_key() {
    let run = render_images(&registry(), IMAGES, &strategy(Strategy::Reference));
    let latex = content(&run);
    assert!(
        latex.contains("\n\\flmxImage[width=4.5cm, height=3cm, alt={A plot}, display=block]{figures/plot.png}\n"),
        "{latex}"
    );
    assert!(latex.contains("An icon \\flmxImage[height=4mm, display=inline]{icon.png} inline"), "{latex}");
    assert!(latex.contains("\\flmxImage[width=12bp, display=inline]{figures/plot.png}"), "{latex}");
    assert_golden("images-reference.tex", &run);
}

#[test]
fn images_by_copy_write_the_copy_s_path_and_the_output_holds_the_copies() {
    let run = render_images(&files_registry(), IMAGES, &strategy(Strategy::Copy));
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let copies: Vec<String> = set
        .iter()
        .filter(|artifact| artifact.id.owner.as_str() == graphics::FEATURE)
        .map(|artifact| format!("files/{}.png", artifact.id.name))
        .collect();
    assert_eq!(copies.len(), 2, "one copy per file: {copies:?}");
    let latex = content(&run);
    for copy in &copies {
        assert!(latex.contains(&format!("]{{{copy}}}")), "{copy} in\n{latex}");
    }
    assert!(!latex.contains("figures/plot.png"), "{latex}");
    assert_golden("images-copy.tex", &run);
}

#[test]
fn an_image_by_data_uri_is_refused_for_this_output() {
    let run = common::stdcore::run_workflow(&files_registry(), WORKFLOW, &[IMAGES], &strategy(Strategy::DataUri));
    expect(run.diagnostics().iter(), "flm.graphics.data-uri-not-shown");
    assert!(content(&run).contains("]{figures/plot.png}"), "published by reference");
}

#[test]
fn an_image_writes_its_anchor_and_an_address_s_special_characters_escaped() {
    let latex = emit(|site| {
        let image = site.pres(
            pres::IMAGE,
            ImagePayload {
                src: String::from("dir/a b_%#~.png"),
                display: ImageDisplay::Inline,
                alt: Some(String::from("x & y")),
                width: Some(Length::parse("2tab").expect("a length")),
                height: None,
                srcset: Vec::new(),
                anchor: Some(String::from("fig-1-img")),
            },
            Vec::new(),
        );
        vec![site.pres(pres::PAR, ParPayload { anchor: None }, vec![image])]
    });
    assert!(
        latex.starts_with("\\flmxImage[width=2\\flmxTabWidth, alt={x \\& y}, display=inline, anchor=fig-1-img]{"),
        "{latex}"
    );
    assert!(latex.contains("\\%"), "{latex}");
}
