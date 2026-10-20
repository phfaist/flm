//! The `latex-recomposed` backend end to end, over a registry built from
//! `flm_core::stdcore::package`: the plan a `latex-recomposed` workflow
//! builds, and the recomposed LaTeX and style file of one document per property as
//! goldens under `tests/goldens/latex/` — text and escaping in both Unicode
//! modes, groups, comments kept and dropped, paragraph breaks, the literal
//! spellings, the formatting commands, headings, references under every
//! option, mathematics inline, display and as environments, verbatim content
//! in and out of arguments, unattributed callables, an accepted hole, a
//! document of two trees, the `always` definition expanded, the packages
//! skipped, an unknown requirement, the same bytes under both execution
//! paths, the same bytes whatever heading levels are set as run-in headings,
//! a label of a type its element refuses, an equation line's labels in a
//! registry without the references feature, and the same labels left out
//! altogether under the render option that drops them there. The last two are
//! the only goldens of the directory produced by a registry that is not the
//! standard package: `math_only_features` builds them, holding mathematics,
//! block structure, the `lowering` and `check` features, the LaTeX family and
//! this backend, and no references feature.
//!
//! Every golden holds the recomposed LaTeX, then the style file, each under a
//! marker line that is itself a LaTeX comment ([`golden_text`]).

mod common;

use serde_json::json;

use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::{Overrides, PlanError};
use flm_core::core::registry::{FlmFeature, FlmRegistry};
use flm_core::core::run::{Run, RunOptions};
use flm_core::dump::diag::{expect, expect_none};
use flm_core::dump::to_json;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use flm_core::model::element::ElementSpec;
use flm_core::model::names::{ElementName, Name};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::UnicodeMode;
use flm_core::stdcore::backends::pres::{self, DroppedPayload};
use flm_core::stdcore::backends::latex::construct::{DEFINITION, RAW};
use flm_core::stdcore::backends::latex::{
    ExtraPreamble, LatexDefinitionKind, LatexDefinitionPayload, LatexPackageDef, LatexRawPayload,
    packages, preamble,
};
use flm_core::stdcore::backends::latex_recomposed::{
    self, BACKEND, LatexRecomposedRenderOptions, OUTPUT_TABLE, StyleFile, WORKFLOW,
};
use flm_core::stdcore::features::headings::HeadingsConfig;
use flm_core::stdcore::features::math::MathRecomposeOptions;
use flm_core::stdcore::features::refs::{
    NO_PREFIX_KEY, RefsRecomposeOptions, SAFE_LABELS, SafeLabelMode,
};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use techy::core::node::NodeRef;
use techy::error::Severity;

use common::latex::{
    COMMANDS_ON_LINES_OF_THEIR_OWN, artifacts, assert_golden, blank_lines_in_math, body, golden_text, style,
    style_if_any, output,
};
use common::naming::{FixedNamer, JobNamer};
use flm_core::core::artifact::ArtifactNamer;
use flm_core::core::region::STYLE_PACKAGE;
use flm_core::core::registry::{FeatureChain, RegistryBuilder, RegistryInputs};
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use common::lowering::PresSite;
use common::stdcore::{identifiers, run_workflow, try_plan};

// --- helpers -------------------------------------------------------------------------------

/// The registry every test starts from: the package under the spine's
/// configuration (its three user definitions), nothing else.
fn registry() -> Arc<FlmRegistry> {
    common::spine::registry()
}

/// A render request carrying `options` on top of the workflow's defaults.
fn options(options: OptionValues) -> Overrides {
    Overrides { render_options: options, ..Overrides::none() }
}

/// A render request with the backend's options set as given.
fn backend_options(render: LatexRecomposedRenderOptions) -> Overrides {
    options(OptionValues::new().with(render))
}

/// A render request with the `refs` feature's recomposition options set as
/// given.
fn refs_options(refs: RefsRecomposeOptions) -> Overrides {
    options(OptionValues::new().with(refs))
}

/// Runs the workflow over one document of one tree per text under
/// `overrides`, sequentially.
fn recompose(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
    run_workflow(registry, WORKFLOW, texts, overrides)
}

/// [`recompose`] over the default registry with no overrides.
fn recomposed(text: &str) -> Run {
    recompose(&registry(), &[text], &Overrides::none())
}

// --- the plan -------------------------------------------------------------------------------

#[test]
fn the_workflow_plans_the_raw_tree_the_safe_labels_the_checks_and_the_three_backend_stages() {
    // A configuration holding one `always` definition: the plan's only rewrite
    // is that expansion.
    let config = StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![
        Definition::macro_("version", Vec::new()).text("1.0").expand(ExpandPolicy::Always),
    ]));
    let registry = common::stdcore::registry(config);
    let plan = common::stdcore::plan(&registry, WORKFLOW, &Overrides::none());
    let names = |names: &[flm_core::model::names::StageName]| {
        names.iter().map(|name| name.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(names(&plan.rewrite_names()), ["flm.input.splice", "flm.userdefs.expand-always"]);
    // The workflow demands `flm.validated` beside its output, so the three
    // checks are here beside the backend's own stages, and with them the
    // stages they read: the reference scan, anchors and resolutions.
    assert_eq!(
        names(&plan.stage_names()),
        [
            "flm.blocks.raw",
            "flm.blocks.check-contract",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.refs.safe-labels",
            "flm.refs.value-safe-labels",
            "flm.theorems.check-proofs",
            "flm.graphics.inspect",
            "flm.graphics.publish.latex-recomposed",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.meta.build",
            "flm.latex-recomposed.pieces",
            "flm.latex-recomposed.lower",
            "flm.latex-recomposed.emit",
            "flm.latex-recomposed.regions",
            "flm.latex-recomposed.final",
            "flm.math-passthrough.check",
        ]
    );
    // Without any definition the only rewrite left is the expansion of the
    // definitions marked as always expanded, which the contract check requires
    // in every plan. No check forces a decision about the *per-plan*
    // definitions: that one the backend's own rules make.
    let bare = common::stdcore::registry(StdcoreConfig::new());
    let plan = common::stdcore::plan(&bare, WORKFLOW, &Overrides::none());
    assert_eq!(names(&plan.rewrite_names()), ["flm.input.splice", "flm.userdefs.expand-always"]);
    assert_eq!(plan.stage_names().len(), 25);
}

// --- text and escaping ---------------------------------------------------------------------

const TEXT_DOCUMENT: &str = "\
Café — naïve “quotes” ½ ≤ α and 𝟙, then ธ (unknown).

Specials in text: a_b, c^d, 50 & more, item #1; the literal spellings
\\% \\# \\& \\$ \\{ \\} \\textbackslash{} and~here, then a sentence.\\@ Done.
";

#[test]
fn text_is_escaped_and_declared_under_the_declare_mode() {
    let run = recomposed(TEXT_DOCUMENT);
    let text = body(&run);
    assert!(text.contains("Café — naïve “quotes” ½ ≤ α and 𝟙, then ธ"), "{text}");
    assert!(text.contains("a\\_b, c\\textasciicircum{}d, 50 \\& more, item \\#1"), "{text}");
    assert!(text.contains("\\% \\# \\& \\$ \\{ \\} \\textbackslash{} and~here"), "{text}");
    assert!(text.contains("sentence.\\@ Done."), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\newunicodechar{é}{\\'e}"), "{style}");
    assert!(style.contains("\\newunicodechar{𝟙}{\\ensuremath{\\mathds{1}}}"), "{style}");
    assert!(style.contains("\\RequirePackage{dsfont}"), "{style}");
    assert!(style.contains("\\RequirePackage{newunicodechar}"), "{style}");
    assert!(!style.contains("ธ"), "{style}");
    expect(run.diagnostics().iter(), "flm.latex-recomposed.unknown-character").once();
    assert_golden("text-declare.tex", &run);
}

/// A Cyrillic word: the letters have LaTeX spellings that belong to a font
/// encoding the document does not select, so the spelling selects it itself.
const CYRILLIC_DOCUMENT: &str = "The Russian word Язык means language.\n";

#[test]
fn a_cyrillic_spelling_selects_its_font_encoding_in_both_unicode_modes() {
    // `\cyrya` and its siblings are declared in the `T2A` font encoding and
    // stop the compilation while another encoding is selected, so the
    // spelling selects `T2A` around itself and the engine's own encoding
    // stays the document's. The `fontenc` request differs by engine — `T2A`
    // beside `TU` under LuaLaTeX and XeLaTeX, beside `T1` under pdfLaTeX — so
    // the style file writes it inside a test of the engine.
    let declared = recomposed(CYRILLIC_DOCUMENT);
    assert!(body(&declared).contains("word Язык means"), "{}", body(&declared));
    let (declared_style, _) = style(&declared);
    assert!(
        declared_style.contains("\\newunicodechar{Я}{{\\fontencoding{T2A}\\selectfont\\CYRYA}}"),
        "{declared_style}"
    );
    assert_fontenc_t2a_branches(&declared_style);
    assert_eq!(declared.diagnostics().iter().count(), 0);
    assert_golden("text-cyrillic-declare.tex", &declared);

    let replaced = recompose(&registry(), &[CYRILLIC_DOCUMENT], &backend_options(
        LatexRecomposedRenderOptions {
            unicode: UnicodeMode::Replace,
            ..LatexRecomposedRenderOptions::default()
        },
    ));
    let text = body(&replaced);
    assert!(text.contains("{\\fontencoding{T2A}\\selectfont\\CYRYA}"), "{text}");
    assert!(!text.contains('Я'), "{text}");
    let (replaced_style, _) = style(&replaced);
    assert_fontenc_t2a_branches(&replaced_style);
    assert!(!replaced_style.contains("newunicodechar"), "{replaced_style}");
    assert_golden("text-cyrillic-replace.tex", &replaced);
}

/// The style file `style` requests `iftex`, and writes the `fontenc` request
/// of the `T2A` encoding once per branch of a test of the engine, each under
/// the comment line naming the identifier behind it.
fn assert_fontenc_t2a_branches(style: &str) {
    assert!(style.contains("\\RequirePackage{iftex}\n"), "{style}");
    for line in [
        "% flm.latex.pkg.untechxt.fontenc-t2a\n\\RequirePackage[T2A,TU]{fontenc}\n",
        "% flm.latex.pkg.untechxt.fontenc-t2a\n\\RequirePackage[T2A,T1]{fontenc}\n",
    ] {
        assert!(style.contains(line), "{line} missing from {style}");
    }
}

/// A letter of the `T1` font encoding and a Cyrillic letter in one document:
/// the two `fontenc` requests are one `\RequirePackage` line per engine.
const FONTENC_DOCUMENT: &str =
    "The Polish word język and the Russian word язык both mean language.\n";

#[test]
fn two_fontenc_requests_are_one_line_per_engine_and_the_default_encoding_is_named_per_engine() {
    // `\k{e}` is declared in `T1` and `\cyrya` in `T2A`, so two chunks of the
    // table are needed; LaTeX loads a package once, so they are one line per
    // engine whose options are the encodings that engine needs, and the
    // comment line above it names the identifiers behind it. Under pdfLaTeX
    // that is both chunks; under LuaLaTeX and XeLaTeX the `T1` chunk asks for
    // nothing and the `T2A` one loads `T2A` beside `TU`. Which encoding ends a
    // list decides nothing, because the command after the block says, per
    // engine, which encoding text is set in. The compile suite compiles this
    // golden like every other, and measures the encoding the Latin text is
    // set in under each engine.
    let run = recomposed(FONTENC_DOCUMENT);
    let (style, _) = style(&run);
    assert!(style.contains("\\newunicodechar{ę}{\\k{e}}"), "{style}");
    assert!(
        style.contains("\\newunicodechar{я}{{\\fontencoding{T2A}\\selectfont\\cyrya}}"),
        "{style}"
    );
    assert!(style.contains("\\RequirePackage{iftex}\n"), "{style}");
    let fontenc = "\\iftutex\n\
                   % flm.latex.pkg.untechxt.fontenc-t2a\n\
                   \\RequirePackage[T2A,TU]{fontenc}\n\
                   \\else\n\
                   % flm.latex.pkg.untechxt.fontenc-t1, flm.latex.pkg.untechxt.fontenc-t2a\n\
                   \\RequirePackage[T1,T2A]{fontenc}\n\
                   \\fi\n";
    assert!(style.contains(fontenc), "{style}");
    assert_eq!(style.matches("{fontenc}").count(), 2, "{style}");
    let commands = "\\iftutex\n\
                    \\renewcommand{\\encodingdefault}{TU}\n\
                    \\else\n\
                    \\renewcommand{\\encodingdefault}{T1}\n\
                    \\fi\n";
    let block = style.find("% --- packages ---").expect("the package block");
    let block_end = style.find(fontenc).expect("the fontenc test") + fontenc.len();
    let command = style.find(commands).expect("the default-encoding commands");
    assert!(block < block_end && block_end <= command, "{style}");
    assert_eq!(run.diagnostics().iter().count(), 0);
    assert_golden("text-fontenc-both.tex", &run);
}

/// One character of every preamble chunk the Unicode-to-LaTeX table has that
/// is not a plain package request of the older roster: the three double-struck
/// and script fonts, the five Cyrillic font encodings beyond `T2A`, the
/// thousands sign read as a text symbol, the STIX symbols, the `wasy` glyph
/// and `tipx`.
const CHUNKS_DOCUMENT: &str =
    "Spellings from every font the table reaches: 𝟙 𝕒 𝟘 𝒶, the Cyrillic \
     letters Ѣ Ӿ Ҍ Ѳ ѯ ҂, the symbols ⊫ ⌕, and the phonetic ƞ.\n";

#[test]
fn every_chunk_of_the_table_reaches_the_style_file_in_both_unicode_modes() {
    // The goldens of this test are compiled by `latex_compile`, so the
    // declarations the style file makes — a math alphabet, a symbol font, a
    // text symbol — are compile-checked beside the package requests.
    let declared = recomposed(CHUNKS_DOCUMENT);
    let (declared_style, _) = style(&declared);
    for line in [
        r"\RequirePackage{bbm}",
        r"\RequirePackage{tipx}",
        // The five Cyrillic font encodings beyond `T2A` are five definitions
        // of the roster derived from untechxt's chunks, and one line per
        // engine, in the order the requests arrive in; the command below the
        // block is what makes each engine's own encoding the one text is set
        // in, so the place `T1` or `TU` takes in that list means nothing.
        r"\RequirePackage[OT2,TU,T2B,T2C,T2D,X2]{fontenc}",
        r"\RequirePackage[OT2,T1,T2B,T2C,T2D,X2]{fontenc}",
        r"\renewcommand{\encodingdefault}{TU}",
        r"\renewcommand{\encodingdefault}{T1}",
    ] {
        assert!(declared_style.contains(line), "{line} missing from {declared_style}");
    }
    assert_eq!(declared_style.matches("{fontenc}").count(), 2, "{declared_style}");
    for declaration in [
        r"\DeclareMathAlphabet{\UnxTBbold}{U}{bbold}{m}{n}",
        r"\DeclareMathAlphabet{\UnxTScr}{U}{dutchcal}{m}{n}",
        r#"\DeclareTextSymbol{\UnxTCyrThousands}{T2D}{"9E}"#,
        r"\DeclareSymbolFont{UnxTstixit}{LS1}{stix}{m}{it}",
        r#"\DeclareMathSymbol{\UnxTVDash}{\mathrel}{UnxTstixit}{"F8}"#,
        r"\DeclareSymbolFont{UnxTwasy}{U}{wasy}{m}{n}",
    ] {
        assert!(
            declared_style.contains(declaration),
            "{declaration} missing from {declared_style}"
        );
    }
    // A declaration is guarded, so two style files in one LaTeX job do not
    // clash over it.
    assert!(declared_style.contains(r"\ifcsname UnxTBbold\endcsname\else"), "{declared_style}");
    assert_eq!(declared.diagnostics().iter().count(), 0);
    assert_golden("text-chunks-declare.tex", &declared);

    let replaced = recompose(&registry(), &[CHUNKS_DOCUMENT], &backend_options(
        LatexRecomposedRenderOptions {
            unicode: UnicodeMode::Replace,
            ..LatexRecomposedRenderOptions::default()
        },
    ));
    let text = body(&replaced);
    assert!(text.contains(r"\ensuremath{\UnxTBbold{0}}"), "{text}");
    assert!(text.contains(r"{\fontencoding{T2D}\selectfont\UnxTCyrThousands}"), "{text}");
    let (replaced_style, _) = style(&replaced);
    assert!(!replaced_style.contains("newunicodechar"), "{replaced_style}");
    assert!(replaced_style.contains(r"\DeclareSymbolFont{UnxTwasy}{U}{wasy}{m}{n}"), "{replaced_style}");
    assert_eq!(replaced.diagnostics().iter().count(), 0);
    assert_golden("text-chunks-replace.tex", &replaced);
}

#[test]
fn the_per_tree_projection_shows_the_chunks_the_tree_needs() {
    // The projection of `flm.output.latex-recomposed` carries what the tree's
    // spellings need in the preamble beside the characters it declares, in the
    // chunk table's own order, so that a dump shows a change in what a
    // document requires and not only in what it declares.
    let run = recomposed(FONTENC_DOCUMENT);
    let projected = to_json(&common::stdcore::projection(&run, OUTPUT_TABLE)).expect("json");
    assert!(projected.contains(r#""chunks":["fontenc-t1","fontenc-t2a"]"#), "{projected}");
    // A document whose characters need nothing shows the empty list.
    let plain = recomposed("Plain text, no spelling needed.\n");
    let projected = to_json(&common::stdcore::projection(&plain, OUTPUT_TABLE)).expect("json");
    assert!(projected.contains(r#""chunks":[]"#), "{projected}");
}

#[test]
fn text_is_spelled_in_place_under_the_replace_mode_with_an_override() {
    // `\mathbf` is the LaTeX kernel's, so this override needs nothing in the
    // preamble — which is what lets the document below compile as it stands.
    // An override whose spelling needs a package is written just the same, and
    // that package is the user's own template's to load: an override
    // contributes nothing to the style file.
    let render = LatexRecomposedRenderOptions {
        unicode: UnicodeMode::Replace,
        unicode_overrides: [('𝟙', String::from("\\ensuremath{\\mathbf{1}}"))]
            .into_iter()
            .collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry(), &[TEXT_DOCUMENT], &backend_options(render));
    let text = body(&run);
    assert!(text.contains("Caf\\'e {\\textemdash} na\\\"ive"), "{text}");
    assert!(text.contains(" \\ensuremath{\\mathbf{1}}, then"), "{text}");
    assert!(text.contains("\\ensuremath{\\langle}\\texttt{U+0E18}\\ensuremath{\\rangle}"), "{text}");
    // The table's own spelling of `𝟙` needs `dsfont`; the override's needs
    // nothing, so the style file requests nothing — and, there being nothing
    // else to put in one, no style file is written at all.
    assert!(style_if_any(&run).is_none(), "{:?}", style_if_any(&run));
    expect(run.diagnostics().iter(), "flm.latex-recomposed.unknown-character").once();
    assert_golden("text-replace.tex", &run);
}

#[test]
fn an_override_under_the_declare_mode_declares_its_spelling_and_asks_for_no_package() {
    let render = LatexRecomposedRenderOptions {
        unicode_overrides: [('α', String::from("\\ensuremath{\\alpha}"))]
            .into_iter()
            .collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry(), &["α only"], &backend_options(render));
    assert_eq!(output(&run), "α only%%\n"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\newunicodechar{α}{\\ensuremath{\\alpha}}"), "{style}");
    // `newunicodechar` is requested because a character is declared; nothing
    // else is, whatever the override's spelling needs — that is the user's own
    // template's to load.
    assert!(style.contains("\\RequirePackage{newunicodechar}"), "{style}");
    assert_eq!(style.matches("\\RequirePackage").count(), 1, "{style}");
}

// --- structure: groups, comments, paragraph breaks, literals -------------------------------

#[test]
fn groups_and_comments_are_written_as_they_stand_and_comments_can_be_dropped() {
    let document = "\
A {grouped} word and a [bracketed] one. %% a comment ends the line
Then {nested {groups} here}. %% another
";
    let run = recomposed(document);
    let text = body(&run);
    assert!(text.contains("A {grouped} word and a [bracketed] one. %% a comment ends the line\n"));
    assert!(text.contains("Then {nested {groups} here}. %% another\n"), "{text}");
    assert_golden("groups-comments.tex", &run);
    let render = LatexRecomposedRenderOptions {
        drop_comments: true,
        ..LatexRecomposedRenderOptions::default()
    };
    let dropped = recompose(&registry(), &[document], &backend_options(render));
    let text = body(&dropped);
    assert!(!text.contains("comment"), "{text}");
    assert!(text.contains("one. Then {nested"), "{text}");
    assert_golden("comments-dropped.tex", &dropped);
}

#[test]
fn a_run_of_blank_lines_is_one_paragraph_break() {
    let document = "First paragraph.\n\n\n\nSecond paragraph.\n\n  \n\nThird.\n";
    let run = recomposed(document);
    assert_eq!(
        output(&run),
        "First paragraph.\n\nSecond paragraph.\n\nThird.\n%%\n"
    );
    assert_golden("paragraphs.tex", &run);
}

#[test]
fn every_literal_spelling_is_written_as_the_author_wrote_it() {
    let document = "\\% \\# \\& \\$ \\{ \\} \\textbackslash \\ x~y \\@ and \\textbackslash\nz \\textbackslash{}w";
    let run = recomposed(document);
    assert_eq!(
        output(&run),
        "\\% \\# \\& \\$ \\{ \\} \\textbackslash \\ x~y \\@ and \\textbackslash\nz \\textbackslash{}w%%\n"
    );
    // Every spelling is written back as the author wrote it, so nothing is
    // declared and no style file is needed.
    assert!(style_if_any(&run).is_none(), "{:?}", style_if_any(&run));
    assert_golden("literals.tex", &run);
}

// --- the formatting commands ---------------------------------------------------------------

#[test]
fn the_formatting_commands_keep_their_own_names() {
    let document = "\\emph{one}, \\textbf{two}, \\textit{three} and \\texttt{four \\emph{five}}.";
    let run = recomposed(document);
    assert_eq!(
        output(&run),
        "\
         \\emph{one}, \\textbf{two}, \\textit{three} and \\texttt{four \\emph{five}}.%%\n"
    );
    assert_golden("textstyles.tex", &run);
}

// --- headings ------------------------------------------------------------------------------

#[test]
fn headings_keep_their_command_their_star_and_their_labels_as_safe_keys() {
    let document = "\
\\section{Introduction}\\label{sec:intro}\\label{sec:one}

\\subsection*{Starred}

\\subsubsection  {Spaced} text

\\paragraph{Para}\\label{sec:para} and \\subparagraph{Sub}

Back to \\ref{sec:one}.
";
    let run = recomposed(document);
    let text = body(&run);
    assert!(
        text.contains(
            "\\flmHeading[toc=true]\n\\section{Introduction}{\\flmLabels{\\label{ref1}\\label{ref2}}}\n"
        ),
        "{text}"
    );
    // Every heading carries the group, empty as `{\flmLabels{}}`.
    assert!(text.contains("\\flmHeading[toc=true]\n\\subsection*{Starred}{\\flmLabels{}}\n"), "{text}");
    assert!(text.contains("\\flmHeading[toc=true]\n\\subsubsection{Spaced}{\\flmLabels{}} text\n"), "{text}");
    assert!(
        text.contains("\\flmHeading[toc=true]\n\\paragraph{Para}{\\flmLabels{\\label{ref3}}} and "),
        "{text}"
    );
    assert!(text.contains("Back to \\flmRef{ref2}.\n"), "{text}");
    let (style, _) = style(&run);
    // The heading piece pulls the pieces it calls into.
    for piece in ["flm.latex.def.heading", "flm.latex.def.labels", "flm.latex.def.label-display-text", "flm.latex.def.zref-clever-type"] {
        assert!(style.contains(piece), "{piece} missing from {style}");
    }
    assert_golden("headings.tex", &run);
}

#[test]
fn the_heading_hook_is_left_out_on_request_and_the_labels_stay_hooked() {
    let document = "\\section*{Starred}\\label{sec:one}\n\n\\section{Plain}\n\nSee \\ref{sec:one}.\n";
    let run = recompose(&registry(), &[document], &backend_options(LatexRecomposedRenderOptions {
        heading_hook: false,
        ..LatexRecomposedRenderOptions::default()
    }));
    let text = body(&run);
    assert!(text.contains("\\section*{Starred}\\flmLabels{\\label{ref1}}\n"), "{text}");
    // With no hook there is nothing to take an empty group, so none is written.
    assert!(text.contains("\\section{Plain}\n"), "{text}");
    assert!(!text.contains("\\flmHeading"), "{text}");
    let (style, _) = style(&run);
    assert!(!style.contains("flm.latex.def.heading"), "{style}");
    assert_golden("headings-no-hook.tex", &run);
}

#[test]
fn every_hook_is_left_out_on_request_and_the_recomposed_output_is_plain_latex() {
    let document = "\\section*{Starred}\\label{sec:one}\n\nSee \\ref{sec:one}.\n";
    let run = recompose(&registry(), &[document], &backend_options(LatexRecomposedRenderOptions {
        heading_hook: false,
        label_hooks: false,
        ..LatexRecomposedRenderOptions::default()
    }));
    let text = body(&run);
    assert!(text.contains("\\section*{Starred}\\label{ref1}\n"), "{text}");
    assert!(text.contains("See \\zcref{ref1}.\n"), "{text}");
    assert!(!text.contains("\\flm"), "{text}");
    let (style, _) = style(&run);
    // Only the package the recomposed LaTeX names itself.
    assert!(style.contains("\\RequirePackage{zref-clever}"), "{style}");
    assert!(!style.contains("flm.latex.def."), "{style}");
    assert_golden("hooks-off.tex", &run);
}

#[test]
fn a_reference_to_an_unnumbered_heading_is_what_the_hook_is_for() {
    // The compile-checked case: `\section*` and `\paragraph` are numbered by
    // no class at its default depth, so a plain `\label` after them would
    // record whatever was numbered last. The hook gives each its title.
    let document = "\
\\section*{An unnumbered section}\\label{sec:starred}

\\paragraph{A paragraph heading}\\label{sec:para} Body text.

See \\ref{sec:starred} and \\ref{sec:para}.
";
    let run = recomposed(document);
    let text = body(&run);
    assert!(
        text.contains(
            "\\flmHeading[toc=true]\n\\section*{An unnumbered section}{\\flmLabels{\\label{ref1}}}\n"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "\\flmHeading[toc=true]\n\\paragraph{A paragraph heading}{\\flmLabels{\\label{ref2}}} Body text.\n"
        ),
        "{text}"
    );
    assert!(text.contains("See \\flmRef{ref1} and \\flmRef{ref2}.\n"), "{text}");
    assert_golden("headings-unnumbered.tex", &run);
}

#[test]
fn an_alternative_title_rides_the_heading_hook_as_its_key_alt() {
    // `alt=` is relayed as the hook's key `alt`, after `toc`, its value the
    // pieces as the author wrote them; a heading without one keeps the hook's
    // `toc` alone. The page keeps the full title: the sectioning command's
    // argument is the title, and a reference is written as before.
    let document = "\
\\section[alt={Short \\emph{title}}]{A long and \\emph{fancy} title}\\label{sec:long}

\\subsection*[alt=Starred short]{A starred long title}

\\section{No alternative}

See \\ref{sec:long}.
";
    let run = recomposed(document);
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    let text = body(&run);
    // An optional argument holding a command is braced whole by the emitter,
    // as every optional argument is, so that a `]` inside cannot end it; LaTeX
    // strips those braces when it reads the argument.
    assert!(
        text.contains(
            "\\flmHeading[{toc=true,alt={Short \\emph{title}}}]\n\\section{A long and \\emph{fancy} title}{\\flmLabels{\\label{ref1}}}\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("\\flmHeading[toc=true,alt={Starred short}]\n\\subsection*{A starred long title}{\\flmLabels{}}\n"),
        "{text}"
    );
    assert!(text.contains("\\flmHeading[toc=true]\n\\section{No alternative}{\\flmLabels{}}\n"), "{text}");
    assert!(text.contains("See \\flmRef{ref1}.\n"), "{text}");
    assert_golden("headings-alt.tex", &run);

    // Whitespace around a braced value: the group's content is relayed.
    let run = recomposed("\\section[toc=true, alt= {Spaced} ]{T}\n");
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    let text = body(&run);
    assert!(text.contains("\\flmHeading[toc=true,alt={Spaced}]\n\\section{T}{\\flmLabels{}}\n"), "{text}");

    // Without the hook nothing carries the key, as nothing carries `toc`.
    let run = recompose(&registry(), &[document], &backend_options(LatexRecomposedRenderOptions {
        heading_hook: false,
        ..LatexRecomposedRenderOptions::default()
    }));
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    let text = body(&run);
    assert!(text.contains("\\section{A long and \\emph{fancy} title}\\flmLabels{\\label{ref1}}\n"), "{text}");
    assert!(text.contains("\\subsection*{A starred long title}\n"), "{text}");
    assert!(!text.contains("Short") && !text.contains("alt="), "{text}");
}

// --- the style file only where it is needed ------------------------------------------------

/// A document that needs nothing in a style file: plain text, no heading, no
/// label, no reference, no verbatim content, no character to declare.
const PLAIN_DOCUMENT: &str = "Plain text with \\emph{emphasis} and nothing else.\n";

#[test]
fn a_document_that_needs_nothing_gets_no_style_file() {
    let run = recomposed(PLAIN_DOCUMENT);
    assert!(style_if_any(&run).is_none(), "{:?}", style_if_any(&run));
    // The artifact set holds the document alone, and its first line is the
    // text, not a comment naming a style file.
    assert_eq!(artifacts(&run).iter().count(), 1);
    assert_eq!(body(&run), "Plain text with \\emph{emphasis} and nothing else.\n%%\n");
    assert_golden("style-when-needed.tex", &run);
}

#[test]
fn the_style_file_is_written_whatever_it_holds_on_request() {
    let run = recompose(&registry(), &[PLAIN_DOCUMENT], &backend_options(
        LatexRecomposedRenderOptions {
            style_file: StyleFile::Always,
            ..LatexRecomposedRenderOptions::default()
        },
    ));
    let (style, path) = style(&run);
    assert!(path.ends_with(".sty"), "{path}");
    assert!(style.ends_with("\\endinput\n"), "{style}");
    assert!(body(&run).starts_with("% flm: style file flmrecomposed."), "{}", body(&run));
    assert_golden("style-always.tex", &run);
}

// --- references ----------------------------------------------------------------------------

const REFS_DOCUMENT: &str = "\
See \\ref{eq:b} before it, \\ref[S,cap=Figure]{sec:a}, and \\ref{nope}.

\\section{A}\\label{sec:a}

\\begin{equation}
  x = 1 \\label{eq:b}
\\end{equation}

Again \\ref{sec:a} and \\ref{eq:b}.
";

#[test]
fn references_are_allocated_in_document_order_and_written_through_the_hook() {
    let run = recomposed(REFS_DOCUMENT);
    let text = body(&run);
    assert!(text.contains("See \\flmRef{ref1} before it, \\flmRef[S,cap={Figure}]{ref2}, and \\flmRef{ref3}."), "{text}");
    assert!(text.contains("\\flmHeading[toc=true]\n\\section{A}{\\flmLabels{\\label{ref2}}}\n"), "{text}");
    assert!(text.contains("  x = 1\\flmLabels{\\label{ref1}}\n\\end{equation}\n"), "{text}");
    assert!(text.contains("Again \\flmRef{ref2} and \\flmRef{ref1}."), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage{zref-clever}"), "{style}");
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    // `nope` is not spelled `type:label` and is named like any other.
    assert_eq!(table.allocated, 3);
    assert_eq!(table.label_of("ref", "ref1"), Some("eq:b"));
    assert_eq!(table.label_of("ref", "ref3"), Some("nope"));
    // `nope` is not spelled `type:label` and is reported where it was written:
    // here it is a `\ref` target, so the reference scan reports it as it walks
    // the references. The references to `eq:b` resolve — an equation line is a
    // reference point — and nothing else is reported.
    assert_eq!(identifiers(&run), ["flm.refs.bad-label"]);
    assert_golden("refs-default.tex", &run);
}

/// A document whose every reference has a target, with options outside the
/// reference hook's translation (`cap=true`, `ref=vref`) beside `S`: the
/// options are written back as written, and the compile suite proves that the
/// recomposed document compiles with the hook's default dropping them.
///
/// [`REFS_DOCUMENT`] cannot be compiled: it references `nope`, which nothing
/// labels, on purpose — it exercises an untyped safe name — so those goldens
/// pin bytes only.
const REFS_COMPILABLE_DOCUMENT: &str = "\
\\section{A}\\label{sec:a}

\\begin{equation}
  x = 1 \\label{eq:b}
\\end{equation}

See \\ref[S,cap=true]{sec:a}, \\ref[ref=vref]{eq:b} and \\ref{eq:b}.
";

#[test]
fn reference_options_the_package_accepts_are_written_through_unchanged() {
    let run = recomposed(REFS_COMPILABLE_DOCUMENT);
    let text = body(&run);
    // A flag option and two keyval options, each value braced as the emitter
    // braces a mandatory argument — which `zref-clever` reads as the value.
    assert!(
        text.contains("See \\flmRef[S,cap={true}]{ref1}, \\flmRef[ref={vref}]{ref2} and \\flmRef{ref2}."),
        "{text}"
    );
    // Every reference resolves: `sec:a` to the heading, `eq:b` to the line of
    // the equation that carries it.
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("refs-options-compiled.tex", &run);
}

/// A document whose references name several targets at once, so that the
/// compile suite proves that one `\flmRef` carrying a list of keys is what
/// `zref-clever` reads as a list of targets.
const REFS_MANY_TARGETS_DOCUMENT: &str = "\
\\section{A}\\label{sec:a}

\\section{B}\\label{sec:b}

\\begin{equation}
  x = 1 \\label{eq:c}
\\end{equation}

See \\ref{sec:a,sec:b}, \\ref[noprefix]{sec:a,sec:b,eq:c} and \\ref{eq:c}.
";

#[test]
fn several_targets_of_one_reference_are_written_as_one_reference_command() {
    let run = recomposed(REFS_MANY_TARGETS_DOCUMENT);
    let text = body(&run);
    // One `\flmRef` per reference, its argument the safe keys of every target
    // separated by commas, in the order the document wrote them.
    assert!(
        text.contains(
            "See \\flmRef{ref1,ref2}, \\flmRef[noprefix]{ref1,ref2,ref3} and \\flmRef{ref3}."
        ),
        "{text}"
    );
    // Each target is a label of its own, allocated once at its first mention.
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 3);
    assert_eq!(table.label_of("ref", "ref1"), Some("sec:a"));
    assert_eq!(table.label_of("ref", "ref2"), Some("sec:b"));
    assert_eq!(table.label_of("ref", "ref3"), Some("eq:c"));
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("refs-many-targets.tex", &run);
}

/// A document written in FLM's own reference vocabulary: a reference with no
/// option, one with `noprefix` — the key that turns off the name a reference
/// puts before its target's number or title — and one with `S`, the
/// capitalized variant. `zref-clever` has no key named `noprefix`; the hook's
/// default translates it, which the compile suite measures over this golden.
const REFS_FLM_OPTIONS_DOCUMENT: &str = "\
\\section{A}\\label{sec:a}

See \\ref{sec:a}, \\ref[noprefix]{sec:a} and \\ref[S]{sec:a}.
";

#[test]
fn flms_own_reference_options_are_written_as_the_document_wrote_them() {
    let run = recomposed(REFS_FLM_OPTIONS_DOCUMENT);
    let text = body(&run);
    // FLM's vocabulary reaches the recomposed LaTeX untouched: nothing here
    // translates `noprefix`, and the hook's default in the style file is what
    // maps it onto `zref-clever`'s `noname`, so a user who redefines `\flmRef`
    // sees the options as the document wrote them.
    assert!(
        text.contains("See \\flmRef{ref1}, \\flmRef[noprefix]{ref1} and \\flmRef[S]{ref1}."),
        "{text}"
    );
    let (style, _) = style(&run);
    // The translation is in the style file's definition of the hook, which is
    // the only place it is: the compile suite measures what it does, and this
    // holds the two names together where the compile suite does not run. The key
    // the hook translates is read from the feature that reads it in a
    // document, so the two spellings cannot drift apart.
    assert!(
        style.contains(&format!("{{{NO_PREFIX_KEY}}}")) && style.contains("{noname}"),
        "the hook's default does not translate the option: {style}"
    );
    // Every reference resolves, so the compile suite can hold the document to
    // an undefined-reference-free run.
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("refs-options-translated.tex", &run);

    // With the hooks off there is no hook to translate anything: the same
    // option bytes are written, around `\zcref` instead of `\flmRef`, which
    // is what that option's "you own this" covers.
    let plain = recompose(&registry(), &[REFS_FLM_OPTIONS_DOCUMENT], &backend_options(
        LatexRecomposedRenderOptions { label_hooks: false, ..LatexRecomposedRenderOptions::default() },
    ));
    let text = body(&plain);
    assert!(
        text.contains("See \\zcref{ref1}, \\zcref[noprefix]{ref1} and \\zcref[S]{ref1}."),
        "{text}"
    );
    assert!(!text.contains("\\flmRef"), "{text}");
}

#[test]
fn a_raw_label_type_is_written_as_written_and_allocates_nothing() {
    let refs = RefsRecomposeOptions {
        safe_label_types: [(String::from("sec"), SafeLabelMode::Raw)].into_iter().collect(),
        ..RefsRecomposeOptions::default()
    };
    let run = recompose(&registry(), &[REFS_DOCUMENT], &refs_options(refs));
    let text = body(&run);
    assert!(text.contains("\\flmRef[S,cap={Figure}]{sec:a}"), "{text}");
    assert!(text.contains("\\flmHeading[toc=true]\n\\section{A}{\\flmLabels{\\label{sec:a}}}\n"), "{text}");
    assert!(text.contains("x = 1\\flmLabels{\\label{ref1}}\n\\end{equation}\n"), "{text}");
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    // The `sec` type is raw; `eq:b` and the untyped `nope` are named.
    assert_eq!(table.allocated, 2);
    assert_golden("refs-raw-types.tex", &run);
}

#[test]
fn the_protection_wraps_a_reference_and_requests_the_package_of_nocasechange() {
    let refs = RefsRecomposeOptions { protect: true, ..RefsRecomposeOptions::default() };
    let run = recompose(&registry(), &[REFS_DOCUMENT], &refs_options(refs));
    let text = body(&run);
    assert!(
        text.contains(
            "See \\NoCaseChange{\\protect\\flmRef{ref1}} before it, \
             \\NoCaseChange{\\protect\\flmRef[S,cap={Figure}]{ref2}}"
        ),
        "{text}"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage{textcase}"), "{style}");
    assert!(style.contains("\\RequirePackage{zref-clever}"), "{style}");
    assert_golden("refs-protect.tex", &run);
}

/// A document whose references are all `\hyperref`: one written before the
/// heading it names, so that the safe key is allocated at that first mention,
/// one carrying markup and one carrying a character that has to be escaped.
/// Every reference resolves, so the compile suite compiles it.
const HYPERREF_DOCUMENT: &str = "\
See \\hyperref[sec:a]{the \\emph{first} part} and \\ref{sec:a}.

\\section{A}\\label{sec:a}

Also \\hyperref[sec:a]{50\\% here}.
";

#[test]
fn a_hyperref_is_written_through_its_hook_around_the_safe_key_and_the_author_s_text() {
    let run = recomposed(HYPERREF_DOCUMENT);
    let text = body(&run);
    // The author's text is recomposed as content: the formatting command is
    // written back as a command and the literal spelling as the escape it is.
    assert!(
        text.contains("See \\flmHyperref{ref1}{the \\emph{first} part} and \\flmRef{ref1}."),
        "{text}"
    );
    assert!(text.contains("Also \\flmHyperref{ref1}{50\\% here}."), "{text}");
    // The key is allocated at the first mention, which here is a `\hyperref`:
    // the safe-labels stage walks references of both spellings.
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 1);
    assert_eq!(table.label_of("ref", "ref1"), Some("sec:a"));
    let (style, _) = style(&run);
    // The hook's own definition is in the style file, and it asks whether the
    // document loaded `hyperref` rather than loading it: whether a recomposed
    // document carries links stays its author's decision.
    assert!(style.contains(&format!("\\ProvideDocumentCommand\\{}", preamble::HYPERREF_COMMAND)), "{style}");
    assert!(!style.contains("\\RequirePackage{hyperref}"), "{style}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("refs-hyperref.tex", &run);
}

#[test]
fn a_hyperref_with_the_hooks_off_is_the_package_s_own_command_and_requests_the_package() {
    let run = recompose(
        &registry(),
        &[HYPERREF_DOCUMENT],
        &backend_options(LatexRecomposedRenderOptions {
            label_hooks: false,
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    let text = body(&run);
    // Everything the hooks did is the author's now, this choice included: the
    // recomposed LaTeX writes the command the hook's default would have run
    // and the style file loads the package that defines it.
    // The optional argument is written the way the emitter writes every
    // optional argument: plain, since the key is text and holds no command.
    assert!(
        text.contains("See \\hyperref[ref1]{the \\emph{first} part} and \\zcref{ref1}."),
        "{text}"
    );
    assert!(!text.contains("\\flmHyperref"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage{hyperref}"), "{style}");
    assert!(!style.contains(&format!("\\{}", preamble::HYPERREF_COMMAND)), "{style}");
    assert_golden("refs-hyperref-no-hook.tex", &run);
}

#[test]
fn a_hyperref_to_a_raw_label_type_is_written_with_the_label_as_the_document_wrote_it() {
    // The one path where the key a `\hyperref` hands its command is the
    // author's own label text rather than an allocated name: the label type
    // `sec` is written raw, so nothing is allocated for it.
    let refs = RefsRecomposeOptions {
        safe_label_types: [(String::from("sec"), SafeLabelMode::Raw)].into_iter().collect(),
        ..RefsRecomposeOptions::default()
    };
    let run = recompose(&registry(), &[HYPERREF_DOCUMENT], &refs_options(refs.clone()));
    let text = body(&run);
    assert!(
        text.contains("See \\flmHyperref{sec:a}{the \\emph{first} part} and \\flmRef{sec:a}."),
        "{text}"
    );
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 0);
    // And with the hooks off, where the label stands in the package command's
    // own optional argument: the raw label is written as the document wrote
    // it there too, and the brackets are plain, the emitter bracing an
    // optional argument only where a node of it is neither text nor a raw
    // piece free of `]`.
    let hooks_off = options(
        OptionValues::new()
            .with(LatexRecomposedRenderOptions {
                label_hooks: false,
                ..LatexRecomposedRenderOptions::default()
            })
            .with(refs),
    );
    let plain = recompose(&registry(), &[HYPERREF_DOCUMENT], &hooks_off);
    let plain_text = body(&plain);
    assert!(
        plain_text.contains("See \\hyperref[sec:a]{the \\emph{first} part} and \\zcref{sec:a}."),
        "{plain_text}"
    );
}

#[test]
fn the_protection_wraps_a_hyperref_as_it_wraps_a_reference() {
    let refs = RefsRecomposeOptions { protect: true, ..RefsRecomposeOptions::default() };
    let run = recompose(&registry(), &[HYPERREF_DOCUMENT], &refs_options(refs));
    let text = body(&run);
    // The whole reference is kept out of a command that rewrites the case of
    // its argument, the author's text included, as the reference
    // implementation keeps it.
    assert!(
        text.contains(
            "See \\NoCaseChange{\\protect\\flmHyperref{ref1}{the \\emph{first} part}} and \
             \\NoCaseChange{\\protect\\flmRef{ref1}}."
        ),
        "{text}"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage{textcase}"), "{style}");
}

#[test]
fn the_text_of_a_reference_s_options_is_escaped_and_a_command_in_a_value_is_lowered() {
    let run = recomposed(
        "\\section{A}\\label{sec:a}\n\nSee \\ref[cap=Fig_1,S,x={a b},y=\\emph{z}]{sec:a}.",
    );
    let text = body(&run);
    // A value holding a command makes the argument `[{…}]`; text alone leaves
    // it plain (the goldens of the other reference tests).
    assert!(
        text.contains("See \\flmRef[{cap={Fig\\_1},S,x={a b},y={\\emph{z}}}]{ref1}."),
        "{text}"
    );
    let plain = recomposed("\\section{A}\\label{sec:a}\n\nSee \\ref[cap=Fig_1,S]{sec:a}.");
    assert!(body(&plain).contains("See \\flmRef[cap={Fig\\_1},S]{ref1}."), "{}", body(&plain));
    assert_eq!(run.diagnostics().iter().count(), 0, "{}", run.diagnostics().render_all());
    assert_golden("refs-options-escaped.tex", &run);
}

// --- mathematics ---------------------------------------------------------------------------

#[test]
fn inline_and_display_mathematics_are_written_byte_for_byte() {
    let document = "Inline \\(a_b & \\alpha^2\\) then \\[ \\int_0^1 f(x)\\,dx = 1 \\] and $ stays.";
    let run = recomposed(document);
    let text = body(&run);
    assert!(text.contains("Inline \\(a_b & \\alpha^2\\) then \\[ \\int_0^1 f(x)\\,dx = 1 \\] and \\$ stays."), "{text}");
    assert_golden("math-inline-display.tex", &run);
}

#[test]
fn a_text_command_is_written_as_its_command_with_its_argument_recomposed_as_text() {
    // Four things the argument may hold, each written as it is written in text
    // anywhere else: a special character escaped, a user definition written as
    // its call beside its definition, mathematics of its own, and a command on
    // a line of a display-math environment.
    let document = "\
Inline \\(x = \\text{50\\% of it}\\), \\(y = \\textbf{\\hello{world}}\\) and \
\\(z = \\mbox{if \\(w\\)}\\).

\\begin{align}
  a &= b \\text{ for every } c \\\\
  d &= e.
\\end{align}";
    let run = recomposed(document);
    let text = body(&run);
    assert!(text.contains("\\text{50\\% of it}"), "{text}");
    assert!(text.contains("\\textbf{\\flmUDhello{world}}"), "{text}");
    assert!(text.contains("\\mbox{if \\(w\\)}"), "{text}");
    assert_golden("math-text.tex", &run);
}

/// The environments' cases, each written back: every one a well-formed body —
/// no empty segment, no tag or label on an unnumbered line — since the `math`
/// feature reports those as parse errors.
const MATH_DOCUMENT: &str = "\
\\begin{equation}
  E = mc^2 \\label{eq:e}
\\end{equation}

\\begin{equation*} a = b \\end{equation*}

\\begin{align}
  x &= 0; \\\\[1ex]
  z &= 1.
\\end{align}

\\begin{align*} ABC\\end{align*}

\\begin{align}
   x &= 0;
   \\label{eq:one}\\\\
   z &= 1;
   \\label{eq:two}
\\end{align}

\\begin{align}
   \\label{eq:three}
   x &= 0.
\\end{align}

\\begin{gather}
  a \\tag{A} \\\\
  b \\tag*{B} \\\\
  c \\nonumber \\\\ [d]
\\end{gather}

\\begin{align}A + B = C\\ .\\end{align}

\\begin{align}
  a \\\\
  b \\\\
\\end{align}
";

#[test]
fn the_environments_are_rebuilt_line_by_line_with_labels_at_the_end_of_their_line() {
    let run = recomposed(MATH_DOCUMENT);
    let text = body(&run);
    assert!(text.contains("\\begin{equation}\n  E = mc^2\\flmLabels{\\label{ref1}}\n\\end{equation}\n"), "{text}");
    assert!(text.contains("\\begin{equation*} a = b \\end{equation*}\n"), "{text}");
    assert!(text.contains("\\begin{align}\n  x &= 0; \\\\[1ex]\n  z &= 1.\n\\end{align}\n"), "{text}");
    assert!(text.contains("\\begin{align*} ABC\\end{align*}\n"), "{text}");
    // A label taken off a line of its own ends the visible line before it; the
    // newline that ended the content is written back after the break.
    assert!(text.contains("\\begin{align}\n   x &= 0;\\flmLabels{\\label{ref2}}\\\\\n   z &= 1;\\flmLabels{\\label{ref3}}\n\\end{align}\n"), "{text}");
    assert!(text.contains("\\begin{align}\n   x &= 0.\\flmLabels{\\label{ref4}}\n\\end{align}\n"), "{text}");
    // `\\nonumber` is a construct: the emitter writes a space after it only
    // where a letter follows.
    assert!(text.contains("\\begin{gather}\n  a  \\tag*{(A)}\\\\\n  b  \\tag*{B}\\\\\n  c \\nonumber\\\\ [d]\n\\end{gather}\n"), "{text}");
    assert!(text.contains("\\begin{align}A + B = C\\ .\\end{align}\n"), "{text}");
    // A final `\\` followed only by whitespace ends no line and is not written;
    // the whitespace after it is the last line's, as written.
    assert!(text.contains("\\begin{align}\n  a \\\\\n  b\n\\end{align}\n"), "{text}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage{amsmath}"), "{style}");
    assert_golden("math-environments.tex", &run);
}

/// Comments inside equation lines: after a line's content, on the line a
/// label is taken off, and after the final `\\`.
const MATH_COMMENTS_DOCUMENT: &str = "\
\\begin{align}
  a &= 1 %% one
  \\label{eq:a} \\\\
  b &= 2 \\\\ %% two
\\end{align}
";

#[test]
fn a_comment_in_an_equation_line_is_written_as_everywhere_and_can_be_dropped() {
    let run = recomposed(MATH_COMMENTS_DOCUMENT);
    let text = body(&run);
    // The comment after the final `\\` stays where the source put it; the
    // `\\` itself is not written.
    assert!(
        text.contains(
            "\\begin{align}\n  a &= 1 %% one\n   \\flmLabels{\\label{ref1}}\\\\\n  b &= 2  %% two\n\\end{align}\n"
        ),
        "{text}"
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("math-comments.tex", &run);
    let render = LatexRecomposedRenderOptions {
        drop_comments: true,
        ..LatexRecomposedRenderOptions::default()
    };
    let dropped = recompose(&registry(), &[MATH_COMMENTS_DOCUMENT], &backend_options(render));
    let text = body(&dropped);
    assert!(!text.contains("%% "), "{text}");
    // A dropped comment takes the newline that ended it with it, as everywhere.
    assert!(
        text.contains("\\begin{align}\n  a &= 1  \\flmLabels{\\label{ref1}}\\\\\n  b &= 2  \\end{align}\n"),
        "{text}"
    );
    assert_golden("math-comments-dropped.tex", &dropped);
}

#[test]
fn a_command_on_a_line_of_its_own_leaves_no_blank_line_in_an_environment() {
    // A `\label` or a final `\\` taken out of a line of its own lets the
    // whitespace before and after it meet; the `math` library's newline rule
    // writes one newline there, since LaTeX refuses a blank line inside
    // mathematics. After a comment, which ends its own line, the newline
    // after the label goes too.
    let run = recomposed(COMMANDS_ON_LINES_OF_THEIR_OWN);
    let text = body(&run);
    assert_eq!(blank_lines_in_math(&text), Vec::<usize>::new(), "{text}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(text.contains("\\end{aligned}\\flmLabels{\\label{ref1}}\n\\end{gather}"), "{text}");
    assert!(text.contains("  a &= b\\\\\n  c &= d\n\\end{align}"), "{text}");
    assert!(text.contains("  a = b\n  + c\\flmLabels{"), "{text}");
    assert!(text.contains("  a &= b \\\\ %% note\n  c &= d\\flmLabels{"), "{text}");
    assert!(text.contains("  x \\to \\infty\\flmLabels{"), "{text}");
    // A dropped comment writes nothing and ends no line.
    let render = LatexRecomposedRenderOptions {
        drop_comments: true,
        ..LatexRecomposedRenderOptions::default()
    };
    let dropped = recompose(&registry(), &[COMMANDS_ON_LINES_OF_THEIR_OWN], &backend_options(render));
    let text = body(&dropped);
    assert_eq!(blank_lines_in_math(&text), Vec::<usize>::new(), "{text}");
    assert!(text.contains("  a &= b \\\\ \n  c &= d\\flmLabels{"), "{text}");
}

/// A document holding a comment beside a kept user macro, one inside a
/// display-math environment and one in text, and a comment that ends the
/// input with no newline after it.
const COMMENT_NEIGHBORS_DOCUMENT: &str = "\
\\begin{align}
  a &= \\alpha %% in mathematics
  \\label{eq:a}\\\\
  b &= 2
\\end{align}

Inline \\(x + y %% inside inline mathematics
 z\\) and text \\hello{3} %% in text
done.

The last line ends in a comment %% at the end of the input";

#[test]
fn a_comment_beside_a_kept_user_macro_ends_its_line_wherever_it_stands() {
    // One writer writes every comment of a recomposed output, and it ends the
    // comment's line itself. That holds in text, inside inline mathematics
    // and on an equation line alike — and beside a kept user macro, whose
    // definition construct is staged among the same pieces. An equation
    // line whose content ends in a comment has its labels on the next line,
    // after that comment's newline.
    let run = recomposed(COMMENT_NEIGHBORS_DOCUMENT);
    let text = body(&run);
    assert!(
        text.contains("  a &= \\alpha %% in mathematics\n  \\flmLabels{\\label{ref1}}\\\\\n"),
        "{text}"
    );
    assert!(
        text.contains("Inline \\(x + y %% inside inline mathematics\n z\\) and text \\flmUDhello{3} %% in text\ndone.\n"),
        "{text}"
    );
    // A comment at the end of the input has no whitespace to write back, and
    // the newline is written all the same: the `%%` that ends the tree would
    // otherwise be swallowed by the comment.
    assert!(
        text.ends_with("ends in a comment %% at the end of the input\n%%\n"),
        "{text}"
    );
    assert_golden("comment-neighbors.tex", &run);
    let render = LatexRecomposedRenderOptions {
        drop_comments: true,
        ..LatexRecomposedRenderOptions::default()
    };
    let dropped =
        recompose(&registry(), &[COMMENT_NEIGHBORS_DOCUMENT], &backend_options(render));
    let text = body(&dropped);
    assert!(!text.contains("%% "), "{text}");
    // A dropped comment writes nothing and takes the whitespace that followed
    // it with it, here and everywhere else.
    assert!(text.contains("  a &= \\alpha \\flmLabels{\\label{ref1}}\\\\\n"), "{text}");
    assert!(
        text.contains("Inline \\(x + y z\\) and text \\flmUDhello{3} done.\n"),
        "{text}"
    );
    // With the last comment gone the tree's text does not end with a newline,
    // and the `%%` that ends the tree stands at the end of that line, as it
    // does for any tree whose last line did not end.
    assert!(text.ends_with("The last line ends in a comment %%\n"), "{text}");
    assert_golden("comment-neighbors-dropped.tex", &dropped);
}

/// A tag holding two characters LaTeX reads specially, a text command, and a
/// double hyphen, which is text the typographic substitution turns into a dash.
const TAG_TEXT_DOCUMENT: &str = "\
\\begin{align}
  a &= 1 \\tag{50\\% \\& more}\\\\
  b &= 2 \\tag*{\\emph{A}}\\\\
  c &= 3 \\tag{a -- b}
\\end{align}
";

#[test]
fn a_tag_holds_text_content_which_is_escaped_and_lowered_like_any_other() {
    // The tag's argument is inline content of the document, not characters:
    // `\%` and `\&` are the characters themselves and the recomposed LaTeX escapes
    // them, and `\emph` is a command the rules lower into its own LaTeX. The
    // parentheses of an unstarred tag are written here, as text, and the
    // command is always `\tag*` so that LaTeX adds none of its own.
    let run = recomposed(TAG_TEXT_DOCUMENT);
    let text = body(&run);
    assert!(text.contains("  a &= 1 \\tag*{(50\\% \\& more)}\\\\\n"), "{text}");
    assert!(text.contains("  b &= 2 \\tag*{\\emph{A}}\\\\\n"), "{text}");
    // The argument is read in text mode, so the block-structure pass lays it out
    // as the inline region it is and the double hyphen becomes an en dash. Here
    // that is invisible: the recomposed LaTeX writes an en dash back as `--`,
    // which is how LaTeX spells it.
    assert!(text.contains("  c &= 3\\tag*{(a -- b)}\n"), "{text}");
    assert_golden("math-tag-text.tex", &run);
    // The text backend writes a formula line by line too, and a tag's content
    // is content there as well: it writes the content as it stands, which is
    // what an author would have typed. That backend flattens content standing
    // in a formula to its text, so the emphasis of the second tag contributes
    // its letter alone.
    let rendered = run_workflow(
        &registry(),
        flm_core::stdcore::backends::text::WORKFLOW,
        &[TAG_TEXT_DOCUMENT],
        &Overrides::none(),
    );
    let output = rendered
        .table(&flm_core::stdcore::backends::text::TEXT_OUTPUT)
        .expect("declared")
        .expect("written");
    assert!(output.text.contains("\\tag*{(50% & more)}"), "{}", output.text);
    assert!(output.text.contains("\\tag*{A}"), "{}", output.text);
    // Where the substitution shows: this backend prints the character itself, so
    // the tag's double hyphen arrives as an en dash. That is the one
    // user-visible effect of reading the argument in text mode.
    assert!(output.text.contains("\\tag*{(a \u{2013} b)}"), "{}", output.text);
}

#[test]
fn an_untyped_label_is_named_like_any_other_and_written_like_any_other() {
    // A label that is not spelled `type:label` names nothing in the document,
    // but it is still a label LaTeX must be able to resolve: it is allocated a
    // safe name from the same counter, keyed by its text, and written by that
    // name. Writing its text instead would put `\label{a\_b}` in the
    // auxiliary file, which LaTeX cannot read back. The reference scan carries
    // the malformed spelling on the line's reference point and reports it, as
    // it does in every workflow — and the recomposed LaTeX is written all the
    // same.
    let run = recomposed("\\begin{align}\n  a &= 1 \\label{a_b}\n\\end{align}\n");
    let text = body(&run);
    assert!(
        text.contains("\\begin{align}\n  a &= 1\\flmLabels{\\label{ref1}}\n\\end{align}\n"),
        "{text}"
    );
    assert_eq!(identifiers(&run), ["flm.refs.bad-label"]);
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 1);
    assert_eq!(table.label_of("ref", "ref1"), Some("a_b"));
    assert_golden("math-untyped-label.tex", &run);
}

#[test]
fn a_construct_reading_a_label_and_a_text_that_is_none_names_them_in_the_written_order() {
    // Safe names are allocated at a label's first mention in document order,
    // and a text that is no label counts as a mention like any other. A
    // construct that read both keeps them in two lists — the parsed labels and
    // the texts that are none — so the stage asks the record to put them back
    // in the order the document wrote them. Written first is named first,
    // whichever kind it is.
    let run = recomposed("\\section{A}\\label{bad}\\label{sec:a}

See \\ref{sec:a}.
");
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 2);
    assert_eq!(table.label_of("ref", "ref1"), Some("bad"));
    assert_eq!(table.label_of("ref", "ref2"), Some("sec:a"));

    // And the other way round, to pin that it is the writing that decides and
    // not the kind.
    let run = recomposed("\\section{A}\\label{sec:a}\\label{bad}

See \\ref{sec:a}.
");
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.label_of("ref", "ref1"), Some("sec:a"));
    assert_eq!(table.label_of("ref", "ref2"), Some("bad"));
}

#[test]
fn two_labels_on_one_equation_line_are_one_group_and_both_resolve() {
    // `amsmath` keeps one label per line; `\flmLabels` runs the first and
    // makes the rest aliases of it, so a reference to either prints the
    // line's number. The compile check exercises exactly this golden.
    let document = "\
\\begin{align}
  a &= b \\label{eq:one}\\label{eq:two}\\\\
  c &= d \\label{eq:three}
\\end{align}

See \\ref{eq:one}, \\ref{eq:two} and \\ref{eq:three}.
";
    let run = recomposed(document);
    let text = body(&run);
    assert!(
        text.contains("  a &= b \\flmLabels{\\label{ref1}\\label{ref2}}\\\\\n"),
        "{text}"
    );
    assert!(text.contains("  c &= d\\flmLabels{\\label{ref3}}\n"), "{text}");
    assert!(text.contains("See \\flmRef{ref1}, \\flmRef{ref2} and \\flmRef{ref3}.\n"), "{text}");
    // Each of the three references resolves to the line that carries its
    // label, here in FLM as it will in the compiled LaTeX.
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden("math-two-labels.tex", &run);
}

#[test]
fn a_reference_to_an_equation_line_is_written_as_the_key_of_that_line() {
    // The label is pinned to its own line, here and in LaTeX: the third line
    // carries it, its key is written at that line's end, and the reference
    // names the same key. The compile suite compiles this golden and checks
    // that LaTeX resolves it.
    let document = "\\begin{align}\n  a &= b \\\\\n  c &= d \\\\\n  e &= f \\label{eq:third}\n\
                    \\end{align}\n\nSee \\ref{eq:third}.\n";
    let run = recomposed(document);
    let text = body(&run);
    assert!(text.contains("  c &= d \\\\\n"), "{text}");
    assert!(text.contains("  e &= f\\flmLabels{\\label{ref1}}\n\\end{align}\n"), "{text}");
    assert!(text.contains("See \\flmRef{ref1}."), "{text}");
    // The reference resolves in FLM too, to the line that carries the label.
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 1);
    assert_eq!(table.label_of("ref", "ref1"), Some("eq:third"));
    assert_golden("math-line-reference.tex", &run);
}

#[test]
fn the_drop_option_changes_nothing_where_the_references_feature_is_loaded() {
    // Philippe's ruling: `drop_math_labels_without_refs` says what to do
    // *without* the references feature, so where that feature is loaded the
    // option is not consulted at all and nothing is reported about it. This
    // pins the whole of that: the recomposed LaTeX and its style file are byte
    // for byte the same with the option set and unset, hash included.
    let document = "\
\\section{Top}\\label{sec:top}

\\begin{align}
  a &= b \\label{eq:one}\\\\
  c &= d \\nonumber\\\\
  e &= f \\tag{A}
\\end{align}

See \\ref{eq:one} and \\ref{sec:top}.
";
    let unset = recompose(&registry(), &[document], &Overrides::none());
    let set = recompose(
        &registry(),
        &[document],
        &backend_options(LatexRecomposedRenderOptions {
            drop_math_labels_without_refs: true,
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    assert_eq!(golden_text(&unset), golden_text(&set));
    assert_eq!(identifiers(&unset), Vec::<String>::new());
    assert_eq!(identifiers(&set), Vec::<String>::new());
    // And what those identical bytes hold, so that the equality above cannot
    // pass over an output that lost its labels twice.
    let text = body(&set);
    let table = set.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 2);
    let section = table.safe_name("ref", "sec:top").expect("a key for the heading's label");
    let equation = table.safe_name("ref", "eq:one").expect("a key for the equation's label");
    assert!(
        text.contains(&format!("\\section{{Top}}{{\\flmLabels{{\\label{{{section}}}}}}}")),
        "{text}"
    );
    assert!(
        text.contains(&format!("  a &= b \\flmLabels{{\\label{{{equation}}}}}\\\\\n")),
        "{text}"
    );
    assert!(text.contains("  c &= d \\nonumber\\\\\n"), "{text}");
    assert!(text.contains("  e &= f\\tag*{(A)}\n"), "{text}");
    assert!(
        text.contains(&format!("See \\flmRef{{{equation}}} and \\flmRef{{{section}}}.")),
        "{text}"
    );
}

#[test]
fn a_body_reported_as_malformed_is_recomposed_as_written() {
    // An empty line, and a label on a line of a starred environment: the first
    // an Error of the `math-passthrough` feature's check, the second a
    // parse-time Error of the `math` feature, neither changing the recomposed
    // LaTeX.
    let run = recomposed(
        "\\begin{align}\n  a &= 1 \\\\\n  \\\\\n  b &= 2\n\\end{align}\n\n\\begin{align*}\n  c \\label{eq:c}\n\\end{align*}\n",
    );
    let text = body(&run);
    assert!(text.contains("\\begin{align}\n  a &= 1 \\\\\n  \\\\\n  b &= 2\n\\end{align}\n"), "{text}");
    // A line of a starred environment will carry no number, so the environment
    // turns it into a point that keeps no label, and the safe-labels stage
    // allocates no key for `eq:c`: the rule writes it as it was written, which
    // is what it writes for any label the table holds no key for. The
    // recomposed file is LaTeX's to refuse.
    assert!(text.contains("\\begin{align*}\n  c\\flmLabels{\\label{eq:c}}\n\\end{align*}\n"), "{text}");
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 0, "nothing in this document is a label a point kept");
    expect(run.diagnostics().iter(), "flm.math-passthrough.empty-line").count(1);
    expect(run.diagnostics().iter(), "flm.math.label-without-number").count(1);
    assert_golden("math-malformed.tex", &run);
}

#[test]
fn a_reference_to_a_label_on_a_line_with_no_number_is_unresolved_in_the_recomposed_latex() {
    // The same rule on this backend as on the text one: a `\nonumber` line and
    // a line of a starred environment keep no label, so the reference naming
    // one resolves to nothing and the stage allocates no key for it. The only
    // key allocated here is the one the numbered line's label takes, and the
    // reference to it is written by that key.
    let run = recomposed(
        "\\begin{align}\n  a &= b \\label{eq:kept}\\\\\n  c &= d \\nonumber\\label{eq:none}\n\
         \\end{align}\n\n\\begin{align*}\n  e &= f \\label{eq:starred}\n\\end{align*}\n\n\
         See \\ref{eq:kept}, \\ref{eq:none} and \\ref{eq:starred}.\n",
    );
    expect(run.diagnostics().iter(), "flm.math.label-without-number").count(2);
    expect(run.diagnostics().iter(), "flm.refs.unresolved").at("\\ref{eq:none}").once();
    expect(run.diagnostics().iter(), "flm.refs.unresolved").at("\\ref{eq:starred}").once();
    // The stage allocates no key for a label the line did not keep — but a
    // `\ref` mentions its target whether or not it resolves, so `eq:none` and
    // `eq:starred` are named by their references instead, and the `\label` the
    // rule writes then finds the same key. The recomposed LaTeX therefore
    // still pairs each `\label` with the `\ref`s naming it, and it is LaTeX,
    // not this library, that decides what a reference to an unnumbered line
    // prints.
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 3);
    assert_eq!(table.label_of("ref", "ref1"), Some("eq:kept"));
    assert_eq!(table.label_of("ref", "ref2"), Some("eq:none"));
    assert_eq!(table.label_of("ref", "ref3"), Some("eq:starred"));
    let text = body(&run);
    assert!(text.contains("a &= b \\flmLabels{\\label{ref1}}\\\\"), "{text}");
    assert!(text.contains("c &= d\\flmLabels{\\label{ref2}}\\nonumber"), "{text}");
    assert!(text.contains("e &= f\\flmLabels{\\label{ref3}}"), "{text}");
    assert!(text.contains("See \\flmRef{ref1}, \\flmRef{ref2} and \\flmRef{ref3}."), "{text}");
}

#[test]
fn the_surround_texts_are_written_around_a_displayed_group_and_not_an_inline_one() {
    let math = MathRecomposeOptions {
        surround_display_math: (String::from("\\mathbefore "), String::from("\\mathafter ")),
        emit_flm_math_environment_macro: false,
    };
    let run = recompose(&registry(), &["A \\[x\\] B \\(y\\) C"], &options(OptionValues::new().with(math)));
    let text = body(&run);
    assert!(text.contains("A \\mathbefore \\[x\\]\\mathafter  B \\(y\\) C"), "{text}");
}

#[test]
fn the_math_options_surround_the_environment_or_write_it_through_the_macro() {
    let math = MathRecomposeOptions {
        surround_display_math: (String::from("\\mathbefore "), String::from("\\mathafter ")),
        emit_flm_math_environment_macro: false,
    };
    let run = recompose(
        &registry(),
        &["\\begin{align}ABC\\end{align}"],
        &options(OptionValues::new().with(math)),
    );
    assert!(body(&run).contains("\\mathbefore \\begin{align}ABC\\end{align}\\mathafter %%\n"), "{}", body(&run));
    assert_golden("math-surround.tex", &run);
    let math = MathRecomposeOptions {
        surround_display_math: (String::new(), String::new()),
        emit_flm_math_environment_macro: true,
    };
    let run = recompose(
        &registry(),
        &["\\begin{align*} ABC\\end{align*} and \\begin{equation}\n x \\label{eq:x}\n\\end{equation}"],
        &options(OptionValues::new().with(math)),
    );
    let text = body(&run);
    assert!(text.contains("\\flmMathEnvironment{align*}{}{ ABC}"), "{text}");
    assert!(text.contains("\\flmMathEnvironment{equation}{}{\n x\\flmLabels{\\label{ref1}}\n}"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentCommand\\flmMathEnvironment"), "{style}");
    assert!(style.contains("\\RequirePackage{amsmath}"), "{style}");
    assert_golden("math-macro.tex", &run);
}

// --- verbatim ------------------------------------------------------------------------------

#[test]
fn the_verbatim_callables_are_written_through_escverb_and_the_code_environment() {
    let document = "\
\\section{Code \\verbcode{f(x)}}

Inline \\verbtext+a{b}\\c+ text, the code \\verbcode[python]+f(x) = 1+ and the
identifier \\verba[rust]+struct+, then \\begin{verbatimtext}raw {text} 100%\\end{verbatimtext}
in the same paragraph, and \\emph{inside \\verbtext{an argument}} too.

\\begin{verbatimcode}[python]
def f(x):
    return {x} + 1
\\end{verbatimcode}

\\begin{verbatimcode}
no language
\\end{verbatimcode}
";
    let run = recomposed(document);
    let text = body(&run);
    assert!(text.contains("\\flmHeading[toc=true]\n\\section{Code \\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{f\\(x\\)}}{\\flmLabels{}}\n"), "{text}");
    assert!(text.contains("Inline \\EscVerb[formatcom=\\flmFmtVRB{verbtext}]{a\\{b\\}\\\\c} text"), "{text}");
    assert!(text.contains("\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{f\\(x\\)\\ \\=\\ 1}"), "{text}");
    assert!(text.contains("\\EscVerb[formatcom=\\flmFmtVRB{verba}]{struct}"), "{text}");
    assert!(text.contains("\\EscVerb[formatcom=\\flmFmtVRB{verbtext}]{raw\\ \\{text\\}\\ 100\\%}"), "{text}");
    assert!(text.contains("\\emph{inside \\EscVerb[formatcom=\\flmFmtVRB{verbtext}]{an\\ argument}}"), "{text}");
    assert!(text.contains("\\begin{flmVerbatimCode}[python]\ndef f(x):\n    return {x} + 1\n\\end{flmVerbatimCode}\n"), "{text}");
    assert!(text.contains("\\begin{flmVerbatimCode}[]\nno language\n\\end{flmVerbatimCode}\n"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage{fvextra}"), "{style}");
    assert!(style.contains("\\ProvideDocumentEnvironment{flmVerbatimCode}"), "{style}");
    assert_golden("verbatim.tex", &run);
    // The tag is trimmed, and a blank one is no tag at all — the same answer
    // `language_of` gives every other backend, so one document reports one
    // language whatever reads it.
    let padded = recomposed(
        "\\begin{verbatimcode}[  python  ]\nf(x)\n\\end{verbatimcode}\n\n\
         \\begin{verbatimcode}[   ]\ng(x)\n\\end{verbatimcode}\n",
    );
    let padded = body(&padded);
    assert!(padded.contains("\\begin{flmVerbatimCode}[python]\nf(x)\n"), "{padded}");
    assert!(padded.contains("\\begin{flmVerbatimCode}[]\ng(x)\n"), "{padded}");
}

// --- unattributed callables and accepted holes ---------------------------------------------

#[test]
fn an_unattributed_macro_and_environment_are_written_as_written_beside_the_error() {
    let document = "See \\nope{} and \\begin{nowhere}inside \\emph{x}\\end{nowhere} here, \\nope and \\nope{x}.";
    let run = recomposed(document);
    assert_eq!(
        output(&run),
        "\
         See \\nope{} and \\begin{nowhere}inside \\emph{x}\\end{nowhere} here, \\nope and \\nope{x}.%%\n"
    );
    assert!(run.diagnostics().has_errors());
    assert_golden("unknown.tex", &run);
}

/// The deliberately unknown element `x.box`, with no rule of its own.
const BOX: &str = "x.box";

/// A feature `t.heading` contributing its own definition piece under the
/// identifier of the heading hook's piece.
fn heading_redefining_feature() -> Arc<dyn FlmFeature> {
    common::pipeline::feature("t.heading", |b| {
        b.contribute_to(
            preamble::SLOT,
            Arc::new(preamble::LatexPreambleDef::new("heading", "% t.heading's heading hook", "the heading hook")),
        )
    })
}

/// The registry of the spine with [`heading_redefining_feature`] and the host's `chains`.
fn heading_redefining_registry(chains: &[&str]) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(common::spine::config()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(heading_redefining_feature());
    for text in chains {
        builder.feature_order(FeatureChain::parse(text, "test").expect("a chain"));
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

#[test]
fn of_two_pieces_under_one_identifier_the_style_file_writes_the_one_of_the_feature_placed_last() {
    let standard = preamble::heading_piece().content;
    let document = ["\\section{A}\n"];

    // `t.heading` is placed after the `latex` feature: its piece alone is written.
    let run = recompose(&heading_redefining_registry(&[]), &document, &Overrides::none());
    let (file, _) = style(&run);
    assert_eq!(file.matches("% t.heading's heading hook").count(), 1, "{file}");
    assert!(!file.contains(&standard), "{file}");

    // The chain places `t.heading` first: the `latex` feature's piece alone is written.
    let run = recompose(&heading_redefining_registry(&["t.heading < latex"]), &document, &Overrides::none());
    let (file, _) = style(&run);
    assert_eq!(file.matches(standard.as_str()).count(), 1, "{file}");
    assert!(!file.contains("% t.heading's heading hook"), "{file}");
}

/// The test-only feature `x`: the block element `\box{content}`.
fn x_feature() -> Arc<dyn FlmFeature> {
    common::pipeline::feature("x", move |b| {
        let element = Arc::new(
            ElementSpec::new(BOX)
                .docs("The deliberately unknown element.")
                .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
        );
        b.element(ElementDef::new(
            "x",
            ["box"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
            element,
        ))?;
        Ok(())
    })
}

#[test]
fn an_element_with_no_rule_is_refused_at_plan_build_and_an_accepted_hole_is_a_comment() {
    let registry = common::spine::registry_with(vec![x_feature()]);
    match try_plan(&registry, WORKFLOW, &Overrides::none()) {
        Err(PlanError::Incomplete { backend, missing, .. }) => {
            assert_eq!(backend.as_str(), BACKEND);
            assert_eq!(missing, [ElementName::from(BOX)]);
        }
        other => panic!("expected the refusal, got {other:?}"),
    }
    let hole = Overrides {
        unsupported: [(BACKEND.into(), vec![ElementName::from(BOX)])].into_iter().collect(),
        ..Overrides::none()
    };
    let run = recompose(&registry, &["Before \\box{inside} after.\n\nThen \\box{x}\nnext."], &hole);
    assert_eq!(
        output(&run),
        "\
         Before \n% flm: unsupported element x.box\n after.\n\nThen \n% flm: unsupported element x.box\nnext.%%\n"
    );
    expect(run.diagnostics().iter(), "flm.lowering.unsupported-element").count(2);
    assert_golden("hole.tex", &run);
}

/// A rule leaving out what it was given, a `flm.pres.dropped` mark standing
/// for it.
struct DroppingRule;

impl Lower for DroppingRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mark = DroppedPayload { condition: Name::from("x.left-out"), detail: String::from("content outside \\cell") };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::DROPPED), mark, Vec::new())?]))
    }
}

#[test]
fn a_dropped_mark_is_a_comment_line_written_as_the_unsupported_stand_in_is() {
    let feature = common::pipeline::feature("x", |b| {
        b.element(ElementDef::new(
            "x",
            ["drop"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
            Arc::new(ElementSpec::new("x.drop").docs("left out, a mark standing for it")),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "x.drop.lower.default",
            "x.drop",
            LoweringDecl::new().produces(pres::DROPPED),
            Arc::new(DroppingRule),
            DefaultMark::Everywhere,
        ))?;
        Ok(())
    });
    let registry = common::spine::registry_with(vec![feature]);
    let run = recompose(&registry, &["Before \\drop{inside} after.\n\nThen \\drop{x}\nnext."], &Overrides::none());
    assert_eq!(
        output(&run),
        "Before \n% flm: dropped content outside \\cell\n after.\n\nThen \n% flm: dropped content outside \\cell\nnext.%%\n"
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
}

// --- documents of several trees, definitions, packages ------------------------------------

#[test]
fn a_document_of_two_trees_is_one_recomposed_output_with_one_safe_label_count() {
    let run = recompose(
        &registry(),
        &["\\section{One}\\label{sec:one}\nSee \\ref{sec:two}.", "\\section{Two}\\label{sec:two}\nSee \\ref{sec:one}."],
        &Overrides::none(),
    );
    assert_eq!(
        output(&run),
        "\
         \\flmHeading[toc=true]\n\\section{One}{\\flmLabels{\\label{ref1}}}\nSee \\flmRef{ref2}.%%\n\
         \\flmHeading[toc=true]\n\\section{Two}{\\flmLabels{\\label{ref2}}}\nSee \\flmRef{ref1}.%%\n"
    );
    let fragments = run.table(OUTPUT_TABLE).expect("declared").expect("written");
    assert_eq!(fragments.trees.len(), 2);
    assert_golden("two-trees.tex", &run);
}

#[test]
fn an_always_definition_is_expanded_in_the_recomposed_latex() {
    let config = StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![
        Definition::macro_("version", Vec::new()).text("1.0").expand(ExpandPolicy::Always),
    ]));
    let run = recompose(&common::stdcore::registry(config), &["Version \\version, done."], &Overrides::none());
    assert_eq!(output(&run), "Version 1.0, done.%%\n"
    );
    assert_golden("always-definition.tex", &run);
}

#[test]
fn skipped_packages_are_left_out_of_the_style_file() {
    let render = LatexRecomposedRenderOptions {
        skip_packages: ["amsmath", "zref-clever"].into_iter().map(String::from).collect(),
        style_stem: String::from("mydoc"),
        ..LatexRecomposedRenderOptions::default()
    };
    let document = "\\ref{sec:a} \\section{A}\\label{sec:a} \\begin{align} x \\end{align} \\verbtext{v}";
    let run = recompose(&registry(), &[document], &backend_options(render));
    let (style, path) = style(&run);
    assert!(!style.contains("\\RequirePackage{amsmath}"), "{style}");
    assert!(!style.contains("\\RequirePackage{zref-clever}"), "{style}");
    assert!(style.contains("\\RequirePackage{fvextra}"), "{style}");
    assert!(path.starts_with("mydoc."), "{path}");
    assert!(body(&run).starts_with("% flm: style file mydoc."), "{}", body(&run));
    assert!(style.contains("\\ProvidesPackage{mydoc."), "{style}");
    assert_golden("skip-packages.tex", &run);
}

/// Skipping `fontenc` leaves out its line under every engine and every
/// default-encoding command, and with them the test of the engine, which
/// nothing else in the block needs. An author's own `fontenc` request with
/// `T1` brings the command back for pdfLaTeX alone, the one engine whose own
/// encoding that request loads.
#[test]
fn skipping_fontenc_leaves_out_its_lines_and_every_default_encoding_command() {
    let skip = || ["fontenc"].into_iter().map(String::from).collect();
    let render = LatexRecomposedRenderOptions {
        skip_packages: skip(),
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry(), &[FONTENC_DOCUMENT], &backend_options(render));
    let (skipped, _) = style(&run);
    assert!(
        skipped.contains("\\newunicodechar{я}{{\\fontencoding{T2A}\\selectfont\\cyrya}}"),
        "{skipped}"
    );
    assert!(!skipped.contains("{fontenc}"), "{skipped}");
    assert!(!skipped.contains("\\encodingdefault"), "{skipped}");
    assert!(!skipped.contains("iftex"), "{skipped}");
    assert!(!skipped.contains("\\iftutex"), "{skipped}");
    assert_eq!(run.diagnostics().iter().count(), 0);

    let render = LatexRecomposedRenderOptions {
        skip_packages: skip(),
        extra_packages: vec![LatexPackageDef::new("fontenc", "the author's own").options(["T1"])],
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry(), &[FONTENC_DOCUMENT], &backend_options(render));
    let (requested, _) = style(&run);
    assert_eq!(requested.matches("{fontenc}").count(), 1, "{requested}");
    assert!(
        requested.contains("% flm.latex.pkg.fontenc\n\\RequirePackage[T1]{fontenc}\n"),
        "{requested}"
    );
    assert!(requested.contains("\\RequirePackage{iftex}\n"), "{requested}");
    assert!(
        requested.contains("\\iftutex\n\\else\n\\renewcommand{\\encodingdefault}{T1}\n\\fi\n"),
        "{requested}"
    );
    assert!(!requested.contains("{TU}"), "{requested}");
}

// --- the packages and the preamble block the author asks for ------------------------------

/// The author's own package requests are written after the ones the
/// document's content made, in the order given, each written once; a request
/// the document already made collapses into it, since what LaTeX reads is the
/// package name with its options and not the identifier the request arrived
/// under.
#[test]
fn the_author_s_package_requests_come_last_and_are_written_once() {
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("amsmath", "the author's own mathematics").options(["fleqn"]),
            LatexPackageDef::new("xcolor", "the author's own colors"),
            LatexPackageDef::new("xcolor", "asked for twice"),
            LatexPackageDef::new("zref-clever", "asked for by the document as well"),
        ],
        ..LatexRecomposedRenderOptions::default()
    };
    let document = "\\ref{sec:a} \\section{A}\\label{sec:a}";
    let run = recompose(&registry(), &[document], &backend_options(render));
    let (style, _) = style(&run);
    let zref = style.find("\\RequirePackage{zref-clever}").expect("the document's own request");
    let amsmath = style.find("\\RequirePackage[fleqn]{amsmath}").expect("the author's first");
    let xcolor = style.find("\\RequirePackage{xcolor}").expect("the author's second");
    assert!(zref < amsmath && amsmath < xcolor, "{style}");
    assert_eq!(style.matches("\\RequirePackage{xcolor}").count(), 1, "{style}");
    assert_eq!(style.matches("\\RequirePackage{zref-clever}").count(), 1, "{style}");
}

/// An explicit request wins over `skip_packages`, and carries the author's
/// options and no others: skipping a package and then requesting it is how an
/// author states an option list this library's own requests cannot add to.
#[test]
fn a_package_the_author_both_skips_and_asks_for_keeps_the_author_s_options() {
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("amsmath", "the author's own mathematics")
                .options(["fleqn,leqno"]),
        ],
        skip_packages: ["amsmath"].into_iter().map(String::from).collect(),
        extra_preamble: ExtraPreamble {
            at_end: String::from("\\relax\n"),
            ..ExtraPreamble::default()
        },
        ..LatexRecomposedRenderOptions::default()
    };
    // The document itself requires `amsmath` (the `align` environment), and
    // that request is the one the skip list stops.
    let document = "\\begin{align} x \\end{align}";
    let run = recompose(&registry(), &[document], &backend_options(render));
    let (written, _) = style(&run);
    assert_eq!(written.matches("{amsmath}").count(), 1, "{written}");
    assert!(written.contains("\\RequirePackage[fleqn,leqno]{amsmath}\n"), "{written}");
    assert!(written.ends_with("\\relax\n\\endinput\n"), "{written}");
    // A package skipped and not asked for is left out as it always was.
    let render = LatexRecomposedRenderOptions {
        skip_packages: ["amsmath"].into_iter().map(String::from).collect(),
        extra_preamble: ExtraPreamble {
            at_end: String::from("\\relax\n"),
            ..ExtraPreamble::default()
        },
        ..LatexRecomposedRenderOptions::default()
    };
    let skipped = recompose(&registry(), &[document], &backend_options(render));
    let (skipped_style, _) = style(&skipped);
    assert!(!skipped_style.contains("amsmath"), "{skipped_style}");
}

/// The author's own requests for a skipped package are merged with **each
/// other** by the rule that governs every other package — one line, the
/// options the union in the order first seen — and with none of this
/// library's, which is what the skip list stops. So "one `\RequirePackage`
/// line per package" holds for this pair too.
#[test]
fn two_requests_for_a_skipped_package_are_one_line_of_the_author_s_own_options() {
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("amsmath", "the author's own mathematics").options(["fleqn"]),
            LatexPackageDef::new("amsmath", "and the equation numbers")
                .id("flm.latex.pkg.amsmath-2")
                .options(["leqno", "fleqn"]),
        ],
        skip_packages: ["amsmath"].into_iter().map(String::from).collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    // The document's own request for `amsmath` is the one the skip list
    // stops; the author's two are merged into one line carrying their options
    // and neither the document's nor the family's.
    let document = "\\begin{align} x \\end{align}";
    let run = recompose(&registry(), &[document], &backend_options(render));
    let (written, _) = style(&run);
    assert_eq!(written.matches("{amsmath}").count(), 1, "{written}");
    assert!(
        written.contains(
            "% flm.latex.pkg.amsmath, flm.latex.pkg.amsmath-2\n\
             \\RequirePackage[fleqn,leqno]{amsmath}\n"
        ),
        "{written}"
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

/// And they conflict by the same rule: two of the author's requests for a
/// skipped package that give one option key two values are the same Error,
/// and leave the package unrequested.
#[test]
fn two_conflicting_requests_for_a_skipped_package_are_the_same_error() {
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("geometry", "the author's page").options(["papersize=a4"]),
            LatexPackageDef::new("geometry", "and again, differently")
                .id("flm.latex.pkg.geometry-2")
                .options(["papersize=letter"]),
        ],
        skip_packages: ["geometry"].into_iter().map(String::from).collect(),
        extra_preamble: ExtraPreamble {
            at_end: String::from("\\relax\n"),
            ..ExtraPreamble::default()
        },
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry(), &[PLAIN_DOCUMENT], &backend_options(render));
    let (written, _) = style(&run);
    assert!(!written.contains("geometry"), "{written}");
    expect(run.diagnostics().iter(), "flm.latex-recomposed.package-options-conflict")
        .data(json!({
            "package": "geometry",
            "key": "papersize",
            "first": "papersize=a4",
            "second": "papersize=letter",
        }))
        .once();
}

/// Two requests for one package that give one option key two values cannot be
/// merged: the run reports an Error and the style file requests that package
/// not at all.
#[test]
fn one_option_key_with_two_values_is_an_error_and_no_line_at_all() {
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("geometry", "the author's page").options(["papersize=a4"]),
            LatexPackageDef::new("geometry", "and again, differently")
                .id("flm.latex.pkg.geometry-2")
                .options(["papersize=letter"]),
            LatexPackageDef::new("xcolor", "unaffected by the conflict"),
        ],
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry(), &[PLAIN_DOCUMENT], &backend_options(render));
    let (style, _) = style(&run);
    assert!(!style.contains("geometry"), "{style}");
    assert!(style.contains("\\RequirePackage{xcolor}"), "{style}");
    expect(run.diagnostics().iter(), "flm.latex-recomposed.package-options-conflict")
        .data(json!({
            "package": "geometry",
            "key": "papersize",
            "first": "papersize=a4",
            "second": "papersize=letter",
        }))
        .once();
}

/// Either option makes the style file exist, whatever the style-file mode
/// says: what the author asked for is written in that file and nowhere else.
/// Any one of the three preamble pieces is enough.
#[test]
fn a_document_that_needs_nothing_gets_the_style_file_its_author_asked_for() {
    let block = "% mine\n\\newcommand\\authnorm[1]{\\lVert #1 \\rVert}";
    let run = recompose(
        &registry(),
        &[PLAIN_DOCUMENT],
        &backend_options(LatexRecomposedRenderOptions {
            extra_preamble: ExtraPreamble {
                at_end: String::from(block),
                ..ExtraPreamble::default()
            },
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    let (written, _) = style(&run);
    // Written as given, with the one line break `\endinput` needs.
    assert!(written.ends_with(&format!("{block}\n\\endinput\n")), "{written}");
    assert!(body(&run).starts_with("% flm: style file flmrecomposed."), "{}", body(&run));
    // The piece before the packages alone does it too, and stands at the top
    // of the file where nothing of the library's precedes it.
    let run = recompose(
        &registry(),
        &[PLAIN_DOCUMENT],
        &backend_options(LatexRecomposedRenderOptions {
            extra_preamble: ExtraPreamble {
                before_packages: String::from("\\PassOptionsToPackage{fleqn}{amsmath}"),
                ..ExtraPreamble::default()
            },
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    let (written, _) = style(&run);
    assert!(
        written.ends_with(
            "% --- the author's own preamble: before the packages ---\n\
             \\PassOptionsToPackage{fleqn}{amsmath}\n\\endinput\n"
        ),
        "{written}"
    );
    // And a file for a document that needs nothing else: the request the
    // author made is written there and nowhere else.
    let run = recompose(
        &registry(),
        &[PLAIN_DOCUMENT],
        &backend_options(LatexRecomposedRenderOptions {
            extra_packages: vec![LatexPackageDef::new("amsmath", "the author's")],
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    let (written, _) = style(&run);
    assert!(written.contains("\\RequirePackage{amsmath}\n"), "{written}");
    assert!(written.ends_with("\\endinput\n"), "{written}");
}

/// The whole of it in one document the compile suite compiles: a package
/// asked for with an option, and all three pieces of the author's own LaTeX —
/// one passing an option to a package the block below it requests, one
/// defining a hook the family would otherwise define, and one defining a
/// command the document's own mathematics names. The package is what that
/// command's body needs, which is the case the two options exist for.
#[test]
fn the_author_s_package_and_three_preamble_pieces_stand_in_their_places() {
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![
            LatexPackageDef::new("amsmath", "what the author's own definition needs")
                .options(["fleqn"]),
        ],
        extra_preamble: ExtraPreamble {
            // Before the packages, which is where an option can still reach a
            // package the block below requests.
            before_packages: String::from(
                "% passed to a package the block below requests\n\
                 \\PassOptionsToPackage{leqno}{amsmath}\n",
            ),
            // After them and before everything the library defines, so this
            // definition of the reference hook is the one the document uses:
            // the family's own is a `\ProvideDocumentCommand` and finds the
            // name taken.
            after_packages: String::from(
                "% the reference hook, defined before the family's own\n\
                 \\ProvideDocumentCommand\\flmRef{O{} m}{(\\ref{#2})}\n",
            ),
            // At the end, where it can use anything above it.
            at_end: String::from(
                "% the author's own preamble, written as it stands\n\
                 \\newcommand\\authnorm[1]{\\lVert #1 \\rVert}\n",
            ),
        },
        ..LatexRecomposedRenderOptions::default()
    };
    let document = "\
\\section{A}\\label{sec:a}

See \\ref{sec:a} and \\(\\authnorm{x} = 1\\).
";
    let run = recompose(&registry(), &[document], &backend_options(render));
    let (style, _) = style(&run);
    assert!(style.contains("\\RequirePackage[fleqn]{amsmath}\n"), "{style}");
    let before_at = style
        .find(
            "% --- the author's own preamble: before the packages ---\n\
             % passed to a package the block below requests\n\
             \\PassOptionsToPackage{leqno}{amsmath}\n",
        )
        .expect("the piece before the packages");
    let packages_at = style.find("% --- packages ---").expect("the package block");
    let after_at = style
        .find(
            "% --- the author's own preamble: after the packages ---\n\
             % the reference hook, defined before the family's own\n\
             \\ProvideDocumentCommand\\flmRef{O{} m}{(\\ref{#2})}\n",
        )
        .expect("the piece after the packages");
    let pieces_at = style
        .find("% --- commands and environments the library defines for the document ---")
        .expect("the pieces");
    assert!(before_at < packages_at, "{style}");
    assert!(packages_at < after_at && after_at < pieces_at, "{style}");
    assert!(
        style.ends_with(
            "% --- the author's own preamble: at the end ---\n\
             % the author's own preamble, written as it stands\n\
             \\newcommand\\authnorm[1]{\\lVert #1 \\rVert}\n\\endinput\n"
        ),
        "{style}"
    );
    assert!(body(&run).contains("\\(\\authnorm{x} = 1\\)"), "{}", body(&run));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_golden("author-preamble.tex", &run);
}

#[test]
fn every_required_piece_s_packages_are_requested_by_the_style_file() {
    let registry = registry();
    let document = "\\ref{sec:a} \\section{A}\\label{sec:a} \\begin{align} x \\end{align} \\verbtext{v}";
    let refs = RefsRecomposeOptions { protect: true, ..RefsRecomposeOptions::default() };
    let run = recompose(&registry, &[document], &refs_options(refs));
    let (style, _) = style(&run);
    let services = registry.services();
    let mut seen = 0;
    for piece in preamble::pieces(services) {
        if !style.contains(&format!("% {}\n", piece.id)) {
            continue;
        }
        seen += 1;
        for id in &piece.requires {
            // A piece may need another piece as well as a package; the
            // closure has already brought it in, so only the packages are
            // checked here.
            if id.as_str().starts_with(preamble::ID_PREFIX) {
                assert!(style.contains(&format!("% {id}\n")), "{} needs {id}:\n{style}", piece.id);
                continue;
            }
            let def = packages::packages_named(services, id.as_str())
                .next()
                .expect("a known package");
            assert!(
                style.contains(&format!("{{{}}}", def.package)),
                "{} requires {} which the style file lacks:\n{style}",
                piece.id,
                def.package
            );
        }
    }
    assert!(seen >= 2, "the verbatim and protection pieces are required:\n{style}");
}

// --- an unknown requirement ----------------------------------------------------------------

/// A rule requiring an identifier no feature contributed.
struct NeedyRule;

impl Lower for NeedyRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from("flm.latex.pkg.nothing"));
        lw.require(&Name::from("flm.latex.def.nothing"));
        let raw = lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new("\\needy"), Vec::new())?;
        Ok(Lowering::Emit(vec![raw]))
    }
}

#[test]
fn an_unknown_requirement_is_reported_and_its_line_omitted() {
    let feature = common::pipeline::feature("x", |b| {
        b.element(ElementDef::new(
            "x",
            ["needy"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(ElementSpec::new("x.needy").docs("needs what nobody has")),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "x.needy.lower.default",
            "x.needy",
            LoweringDecl::new().produces(RAW),
            Arc::new(NeedyRule),
            DefaultMark::Everywhere,
        ))?;
        Ok(())
    });
    let registry = common::spine::registry_with(vec![feature]);
    let run = recompose(&registry, &["a \\needy{} b"], &Overrides::none());
    assert_eq!(output(&run), "a \\needy{} b%%\n"
    );
    expect(run.diagnostics().iter(), "flm.latex-recomposed.unknown-requirement").count(2);
    // Neither identifier resolves, so the style file would hold nothing and
    // none is written.
    assert!(style_if_any(&run).is_none(), "{:?}", style_if_any(&run));
}

// --- determinism ---------------------------------------------------------------------------

#[test]
fn the_recomposed_latex_is_the_same_bytes_under_both_execution_paths() {
    let registry = registry();
    let doc = common::pipeline::doc(&registry, &[common::spine::FEATURES_DOCUMENT, MATH_DOCUMENT]);
    let plan = common::stdcore::plan(&registry, WORKFLOW, &Overrides::none());
    let sequential = common::pipeline::run(&doc, &plan, &RunOptions::sequential());
    let parallel = common::pipeline::run(&doc, &plan, &RunOptions::default());
    assert_eq!(golden_text(&sequential), golden_text(&parallel));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
}

// --- what a registry may hold --------------------------------------------------------------

/// The standard content features, the machinery features, the `lowering`
/// feature and the text backend: a text-only host's registry.
fn text_only_features() -> Vec<Arc<dyn FlmFeature>> {
    vec![
        flm_core::stdcore::features::textstyles::feature(),
        flm_core::stdcore::features::literals::feature(),
        Arc::new(flm_core::stdcore::features::blocks::Blocks::default()),
        flm_core::stdcore::features::index::feature(),
        flm_core::stdcore::features::refpoints::feature(),
        flm_core::stdcore::features::math::feature(),
        flm_core::math_passthrough::feature(),
        flm_core::stdcore::features::refs::feature(),
        flm_core::stdcore::features::headings::feature(),
        flm_core::stdcore::features::verbatim::feature(),
        Arc::new(flm_core::stdcore::features::userdefs::Userdefs::new(UserdefsConfig::new())),
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        flm_core::stdcore::backends::text::feature(),
        // The text backend's workflow demands the validation collection beside
        // its output, and the `check` feature is what declares that collection.
        flm_core::stdcore::check_feature(),
    ]
}

#[test]
fn the_content_features_build_without_the_latex_backend() {
    // Each content feature registers its own recomposition rule, declaring
    // the backend and the constructs it stages as peer declarations; without
    // the LaTeX family and the backend those declarations stay unresolved, the
    // rules are set aside with one Note per unresolved name, and the registry
    // builds and renders text. The `math-passthrough` feature's rule for
    // `flm.math` is marked for every backend and is not set aside.
    let registry = common::pipeline::registry(text_only_features());
    assert!(!registry.has_feature("latex-recomposed"));
    let notes: Vec<&str> = registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    assert!(!notes.is_empty());
    assert!(notes.iter().all(|id| *id == "flm.registry.unresolved-peer"), "{notes:?}");
    assert!(!registry.build_diagnostics().has_errors());
    let mut set_aside: Vec<&str> =
        registry.set_aside_rules().map(|entry| entry.rule.name.as_str()).collect();
    set_aside.sort_unstable();
    assert_eq!(
        set_aside,
        [
            "flm.heading.lower.latex-recomposed",
            "flm.heading.number.lower.latex-recomposed",
            "flm.hyperref.lower.latex-recomposed",
            "flm.literal.lower.latex-recomposed",
            "flm.math.environment.lower.passthrough-latex-recomposed",
            "flm.math.eqref.lower.latex-recomposed",
            "flm.math.lower.passthrough-latex-recomposed",
            "flm.math.text.lower.latex-recomposed",
            "flm.ref.lower.latex-recomposed",
            "flm.sentence-spacing.lower.latex-recomposed",
            "flm.textstyle.lower.latex-recomposed",
            "flm.verbatim.environment.lower.latex-recomposed",
            "flm.verbatim.macro.lower.latex-recomposed",
        ]
    );
    assert!(registry.peer_backends().iter().all(|entry| !entry.resolved));
    let plan = common::stdcore::plan(
        &registry,
        flm_core::stdcore::backends::text::WORKFLOW,
        &Overrides::none(),
    );
    assert!(plan.active_backends().contains(flm_core::stdcore::backends::text::BACKEND));
}

/// A lean host's registry: the mathematics feature, the block structure every
/// document has, the `lowering` feature, the `check` feature the workflows
/// demand, and the backend features named. No `refs`, so nothing resolves a
/// reference and no safe LaTeX key is allocated for a label.
fn math_only_features(latex: bool) -> Vec<Arc<dyn FlmFeature>> {
    let mut features: Vec<Arc<dyn FlmFeature>> = vec![
        flm_core::stdcore::features::math::feature(),
        flm_core::math_passthrough::feature(),
        Arc::new(flm_core::stdcore::features::blocks::Blocks::default()),
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::check_feature(),
    ];
    if latex {
        features.push(flm_core::stdcore::backends::latex::feature());
        features.push(latex_recomposed::feature());
        features.push(flm_core::stdcore::backends::pieces::feature());
    } else {
        features.push(flm_core::stdcore::backends::pieces::feature());
        features.push(flm_core::stdcore::backends::text::feature());
    }
    features
}

/// The one-line document the three tests below render: one display-math
/// environment whose line carries a `\label`.
const LABELED_LINE: &str = "\\begin{equation}
  E = mc^2 \\label{eq:e}
\\end{equation}
";

/// The document the recomposing tests below render: three lines of an `align`,
/// the first labeled, the second unnumbered, the third tagged — so that one
/// document shows what happens to a label and what does not happen to a
/// `\nonumber` or a `\tag`.
const LABELED_TAGGED_LINES: &str = "\\begin{align}
  a &= b \\label{eq:one}\\\\
  c &= d \\nonumber\\\\
  e &= f \\tag{A}
\\end{align}
";

#[test]
fn mathematics_renders_text_without_the_references_feature() {
    // Ruling: a lean host may hand mathematics to a browser's own formatter and
    // wants neither references nor labels. The mathematics feature declares the
    // safe-labels table as a peer table and reads it optionally, so a registry
    // without `refs` builds; `\label` on a line is accepted and the text render
    // prints the environment's source.
    let registry = common::pipeline::registry(math_only_features(false));
    assert!(!registry.has_feature("refs"));
    assert!(!registry.build_diagnostics().has_errors());
    let run = common::stdcore::run_workflow(
        &registry,
        flm_core::stdcore::backends::text::WORKFLOW,
        &[LABELED_LINE],
        &Overrides::none(),
    );
    let text = common::lowering::text(&run);
    assert!(text.contains("E = mc^2"), "{text}");
    assert!(!run.diagnostics().has_errors(), "{:?}", identifiers(&run));
}

#[test]
fn mathematics_recomposes_a_labeled_line_as_written_without_the_references_feature() {
    // The same registry with the LaTeX family and the recomposing backend: the
    // environment rule is selected — its peer declarations of the backend and
    // the constructs resolve — and its optional read of the safe-labels table
    // sees no table, so the label is written as the document wrote it, for a
    // host that resolves labels itself.
    let registry = common::pipeline::registry(math_only_features(true));
    assert!(!registry.has_feature("refs"));
    assert!(!registry.build_diagnostics().has_errors());
    // The unresolved peer tables are those of the `math` feature's rules
    // (the counter table its `\eqref` rule for every backend reads, the
    // safe-labels table, the resolutions table and the value names its
    // `\eqref` rules read) and those of the `math-passthrough` feature's
    // rules (the anchors and counter tables its rule for the display-math
    // environments reads, the safe-labels table its recomposition rule
    // reads); none of them sets a rule aside, because all are read
    // optionally, and no rule is set aside at all.
    let unresolved: Vec<(String, String)> = registry
        .peer_tables()
        .iter()
        .filter(|entry| !entry.resolved)
        .map(|entry| (entry.owner.to_string(), entry.name.to_string()))
        .collect();
    assert_eq!(
        unresolved,
        [
            ("math".to_string(), "flm.counters".to_string()),
            ("math".to_string(), "flm.labels.safe".to_string()),
            ("math".to_string(), "flm.refs.resolutions".to_string()),
            ("math".to_string(), "flm.labels.safe.value".to_string()),
            ("math-passthrough".to_string(), "flm.anchors".to_string()),
            ("math-passthrough".to_string(), "flm.counters".to_string()),
            ("math-passthrough".to_string(), "flm.labels.safe".to_string())
        ]
    );
    assert!(!registry.build_diagnostics().has_errors());
    assert_eq!(registry.set_aside_rules().count(), 0);
    // Every peer backend resolves: the one the rules declare is the
    // recomposing backend, which this registry holds.
    let unresolved_backends: Vec<(String, String)> = registry
        .peer_backends()
        .iter()
        .filter(|entry| !entry.resolved)
        .map(|entry| (entry.owner.to_string(), entry.name.to_string()))
        .collect();
    assert!(unresolved_backends.is_empty(), "{unresolved_backends:?}");
    let notes: Vec<String> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.registry.unresolved-peer")
        .map(|d| d.message())
        .collect();
    // One Note per unresolved declaration, and no other: the `math`
    // feature's of the counter table, the safe-labels table, the resolutions
    // table and the value names, of the two slots it contributes its counter
    // and its style to, and of the two argument roles of the references
    // feature that its `\label` and `\eqref` hold; the `math-passthrough`
    // feature's of the anchors table, the counter table and the safe-labels
    // table. A name two features declare is named by two Notes, one per
    // feature.
    assert_eq!(notes.len(), 11, "{notes:?}");
    for (name, count) in [
        ("flm.labels.safe", 2),
        ("flm.labels.safe.value", 1),
        ("flm.anchors", 1),
        ("flm.counters", 2),
        ("flm.refs.resolutions", 1),
        ("flm.numbering.counters", 1),
        ("flm.numbering.styles", 1),
        ("flm.refs.defined-label", 1),
        ("flm.refs.target", 1),
    ] {
        let quoted = format!("`{name}`");
        let naming = notes.iter().filter(|note| note.contains(&quoted)).count();
        assert_eq!(naming, count, "{name}: {notes:?}");
    }
    let run = common::stdcore::run_workflow(
        &registry,
        latex_recomposed::WORKFLOW,
        &[LABELED_LINE],
        &Overrides::none(),
    );
    let text = body(&run);
    // Philippe's ruling: every label of the recomposed LaTeX stands inside
    // `\flmLabels`, in mathematics as in a heading, with the references
    // feature and without it. Without it nothing allocated a key, so what
    // stands inside the `\label` is the label as the document wrote it.
    assert!(
        text.contains(
            "\\begin{equation}\n  E = mc^2\\flmLabels{\\label{eq:e}}\n\\end{equation}\n"
        ),
        "{text}"
    );
    assert!(!run.diagnostics().has_errors(), "{:?}", identifiers(&run));
}

#[test]
fn the_label_hook_means_the_same_without_the_references_feature_as_with_it() {
    // The hook option is about the group around a set of labels and about
    // nothing else, so it reads the same here as in any other registry: on,
    // the group; off, the bare `\label` commands the group would have held.
    // Without the references feature the only difference is what stands
    // inside each `\label` — the label as written, nothing having allocated
    // a key for it.
    let registry = common::pipeline::registry(math_only_features(true));
    let written = |label_hooks: bool| {
        let run = common::stdcore::run_workflow(
            &registry,
            latex_recomposed::WORKFLOW,
            &[LABELED_LINE],
            &backend_options(LatexRecomposedRenderOptions {
                label_hooks,
                ..LatexRecomposedRenderOptions::default()
            }),
        );
        assert!(!run.diagnostics().has_errors(), "{:?}", identifiers(&run));
        body(&run)
    };
    let hooked = written(true);
    assert!(hooked.contains("  E = mc^2\\flmLabels{\\label{eq:e}}\n"), "{hooked}");
    let bare = written(false);
    assert!(bare.contains("  E = mc^2\\label{eq:e}\n"), "{bare}");
    assert!(!bare.contains("flmLabels"), "{bare}");
}

#[test]
fn mathematics_without_the_references_feature_keeps_every_label_in_the_hook() {
    // The compiled golden of the same: three lines, the first labeled, the
    // second unnumbered, the third tagged. The label stands inside
    // `\flmLabels` as it does with the references feature, which is what
    // makes the style file request the labels piece here too; the `\nonumber`
    // and the `\tag` are not labels and are written as they always are.
    let registry = common::pipeline::registry(math_only_features(true));
    let run = common::stdcore::run_workflow(
        &registry,
        latex_recomposed::WORKFLOW,
        &[LABELED_TAGGED_LINES],
        &Overrides::none(),
    );
    let text = body(&run);
    assert!(text.contains("  a &= b \\flmLabels{\\label{eq:one}}\\\\\n"), "{text}");
    assert!(text.contains("  c &= d \\nonumber\\\\\n"), "{text}");
    assert!(text.contains("  e &= f\\tag*{(A)}\n"), "{text}");
    assert!(!run.diagnostics().has_errors(), "{:?}", identifiers(&run));
    assert_golden("math-labels-no-refs.tex", &run);
}

#[test]
fn the_drop_option_writes_nothing_for_a_lines_labels_without_the_references_feature() {
    // The same document and the same registry with `drop_math_labels_without_refs`
    // set: the `\label` is gone and nothing took its place — no command and no
    // group — while the `\nonumber` and the `\tag` are untouched.
    let registry = common::pipeline::registry(math_only_features(true));
    let run = common::stdcore::run_workflow(
        &registry,
        latex_recomposed::WORKFLOW,
        &[LABELED_TAGGED_LINES],
        &backend_options(LatexRecomposedRenderOptions {
            drop_math_labels_without_refs: true,
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    let text = body(&run);
    assert!(text.contains("  a &= b \\\\\n"), "{text}");
    assert!(text.contains("  c &= d \\nonumber\\\\\n"), "{text}");
    assert!(text.contains("  e &= f\\tag*{(A)}\n"), "{text}");
    assert!(!text.contains("\\label"), "{text}");
    assert!(!text.contains("flmLabels"), "{text}");
    assert!(!run.diagnostics().has_errors(), "{:?}", identifiers(&run));
    assert_golden("math-labels-dropped.tex", &run);
}

/// A registry holding both headings and mathematics without the references
/// feature, with the LaTeX family and the recomposing backend: what the option
/// that drops an equation line's labels is measured against, since it must
/// leave a heading's labels alone.
fn headings_and_math_features() -> Vec<Arc<dyn FlmFeature>> {
    vec![
        flm_core::stdcore::features::headings::feature(),
        flm_core::stdcore::features::math::feature(),
        flm_core::math_passthrough::feature(),
        Arc::new(flm_core::stdcore::features::blocks::Blocks::default()),
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::check_feature(),
        flm_core::stdcore::backends::latex::feature(),
        latex_recomposed::feature(),
        flm_core::stdcore::backends::pieces::feature(),
    ]
}

#[test]
fn the_drop_option_leaves_a_headings_labels_alone() {
    // The option names mathematics and reaches nothing else: Philippe's ruling
    // asks for no option of the kind for headings. In a registry holding both
    // constructs and no references feature, with the option set, the heading
    // keeps its labels inside `\flmLabels` — the label as the document wrote
    // it, nothing having allocated a key — and the equation line writes none.
    let registry = common::pipeline::registry(headings_and_math_features());
    assert!(!registry.has_feature("refs"));
    assert!(!registry.build_diagnostics().has_errors());
    let document = "\
\\section{Top}\\label{sec:a}

\\begin{equation}
  E = mc^2 \\label{eq:e}
\\end{equation}
";
    let run = common::stdcore::run_workflow(
        &registry,
        latex_recomposed::WORKFLOW,
        &[document],
        &backend_options(LatexRecomposedRenderOptions {
            drop_math_labels_without_refs: true,
            ..LatexRecomposedRenderOptions::default()
        }),
    );
    let text = body(&run);
    assert!(text.contains("\\section{Top}{\\flmLabels{\\label{sec:a}}}"), "{text}");
    assert!(text.contains("\\begin{equation}\n  E = mc^2\n\\end{equation}\n"), "{text}");
    assert!(!text.contains("\\label{eq:e}"), "{text}");
    assert!(!run.diagnostics().has_errors(), "{:?}", identifiers(&run));
}

/// A registry holding headings and the machinery they need, without the
/// references feature: nothing computes anchors, so the heading rule reads the
/// anchors table it declares as a peer table and sees none.
fn headings_only_features(latex: bool) -> Vec<Arc<dyn FlmFeature>> {
    let mut features: Vec<Arc<dyn FlmFeature>> = vec![
        flm_core::stdcore::features::headings::feature(),
        Arc::new(flm_core::stdcore::features::blocks::Blocks::default()),
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::check_feature(),
    ];
    if latex {
        features.push(flm_core::stdcore::backends::latex::feature());
        features.push(latex_recomposed::feature());
        features.push(flm_core::stdcore::backends::pieces::feature());
    } else {
        features.push(flm_core::stdcore::backends::pieces::feature());
        features.push(flm_core::stdcore::backends::text::feature());
    }
    features
}

/// The document the two tests below render: one heading, unlabeled — they are
/// about the anchor, not about labels. A heading in such a registry *can*
/// carry a `\label`: the field is declared by the heading's own element
/// (`headings::heading_args`), not by the references feature.
const A_HEADING: &str = "\\section{Top}\n\nSome content.\n";

/// The one unresolved peer declaration of a registry holding `headings`
/// without `refs`: the anchors table, declared by the headings feature.
fn unresolved_peer_tables(registry: &FlmRegistry) -> Vec<(String, String)> {
    registry
        .peer_tables()
        .iter()
        .filter(|entry| !entry.resolved)
        .map(|entry| (entry.owner.to_string(), entry.name.to_string()))
        .collect()
}

/// The messages of a registry's `flm.registry.unresolved-peer` Notes.
fn unresolved_peer_notes(registry: &FlmRegistry) -> Vec<String> {
    registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.registry.unresolved-peer")
        .map(|d| d.message())
        .collect()
}

#[test]
fn headings_render_text_without_the_references_feature() {
    // Ruling: without the references feature a heading is printed with no
    // anchor and no diagnostic at the heading; the one Note per unresolved
    // peer declaration at registry build is the whole report.
    let registry = common::pipeline::registry(headings_only_features(false));
    assert!(!registry.has_feature("refs"));
    assert!(!registry.build_diagnostics().has_errors());
    // The three tables the feature declares as peer tables are unresolved
    // here: the anchors table because nothing computes anchors, the counter
    // table because nothing numbers anything, and the safe-labels table
    // because the recomposition rule's whole backend is absent too.
    assert_eq!(
        unresolved_peer_tables(&registry),
        [
            ("headings".to_string(), "flm.anchors".to_string()),
            ("headings".to_string(), "flm.counters".to_string()),
            ("headings".to_string(), "flm.labels.safe".to_string())
        ]
    );
    // The recomposition rules — the heading's and its number's — are set aside
    // for want of the LaTeX backend, as they always were; the general rule is
    // not, because an optional read sets nothing aside — which is what keeps
    // the heading renderable here.
    let set_aside: Vec<&str> =
        registry.set_aside_rules().map(|entry| entry.rule.name.as_str()).collect();
    assert_eq!(
        set_aside,
        ["flm.heading.lower.latex-recomposed", "flm.heading.number.lower.latex-recomposed"]
    );
    let notes = unresolved_peer_notes(&registry);
    assert!(
        notes.contains(
            &"feature `headings` refers to the table `flm.anchors` and expects another feature to declare it, but no feature of this registry declares it; no rule of that feature depends on it"
                .to_string()
        ),
        "{notes:?}"
    );
    let run = common::stdcore::run_workflow(
        &registry,
        flm_core::stdcore::backends::text::WORKFLOW,
        &[A_HEADING],
        &Overrides::none(),
    );
    let text = common::lowering::text(&run);
    assert!(text.contains("Top"), "{text}");
    assert!(text.contains("Some content."), "{text}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn headings_recompose_without_the_references_feature() {
    // The same registry with the LaTeX family and the recomposing backend: the
    // recomposition rule's peer declarations of the backend and the constructs
    // resolve, its optional read of the safe-labels table sees no table, and
    // the heading is written with an empty label group.
    let registry = common::pipeline::registry(headings_only_features(true));
    assert!(!registry.has_feature("refs"));
    assert!(!registry.build_diagnostics().has_errors());
    assert_eq!(
        unresolved_peer_tables(&registry),
        [
            ("headings".to_string(), "flm.anchors".to_string()),
            ("headings".to_string(), "flm.counters".to_string()),
            ("headings".to_string(), "flm.labels.safe".to_string())
        ]
    );
    assert_eq!(registry.set_aside_rules().count(), 0);
    // One Note per unresolved declaration: the three tables, the three
    // slots the feature contributes its counters, its counter styles and its
    // parent set to, which no feature of this registry owns, and the argument
    // role of the references feature that the `\label` field holds.
    assert_eq!(unresolved_peer_notes(&registry).len(), 7);
    let run = common::stdcore::run_workflow(
        &registry,
        latex_recomposed::WORKFLOW,
        &[A_HEADING],
        &Overrides::none(),
    );
    let text = body(&run);
    assert!(text.contains("\\flmHeading[toc=true]\n\\section{Top}{\\flmLabels{}}"), "{text}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn the_backend_without_a_content_feature_builds_and_refuses_only_the_plan() {
    // The converse: the backend depends on no content feature, so a registry
    // holding the backend but not `textstyles` builds; what it cannot do is
    // plan the `latex-recomposed` workflow for a document that could hold a
    // `flm.textstyle` — and without `textstyles`, no document can.
    let mut features: Vec<Arc<dyn FlmFeature>> = text_only_features()
        .into_iter()
        .filter(|feature| feature.info().name.as_str() != "textstyles")
        .collect();
    features.push(flm_core::stdcore::backends::latex::feature());
    features.push(latex_recomposed::feature());
    let registry = common::pipeline::registry(features);
    assert!(!registry.build_diagnostics().has_errors());
    // Four declarations go unresolved: the counter table, declared by the
    // `math`, `math-passthrough`, `refs` and `headings` features, none of
    // whose owners this registry holds.
    // Every declaration about the recomposed
    // LaTeX resolves, and so does every peer table of the references feature's
    // own (this registry holds `refs`). The names are asserted rather than the
    // Notes' prose.
    let unresolved: Vec<(String, String)> = {
        let backends = registry
            .peer_backends()
            .iter()
            .filter(|entry| !entry.resolved)
            .map(|entry| (entry.owner.to_string(), entry.name.to_string()));
        let tables = registry
            .peer_tables()
            .iter()
            .filter(|entry| !entry.resolved)
            .map(|entry| (entry.owner.to_string(), entry.name.to_string()));
        let elements = registry
            .peer_elements()
            .iter()
            .filter(|entry| !entry.resolved)
            .map(|entry| (entry.owner.to_string(), entry.name.to_string()));
        backends.chain(tables).chain(elements).collect()
    };
    assert_eq!(
        unresolved,
        [
            ("math".to_string(), "flm.counters".to_string()),
            ("math-passthrough".to_string(), "flm.counters".to_string()),
            ("refs".to_string(), "flm.counters".to_string()),
            ("headings".to_string(), "flm.counters".to_string())
        ]
    );
    // One Note per unresolved declaration, and nothing else at all: the four
    // above, the two slots each of the mathematics and headings features
    // contributes its counters and its counter styles to, and the slot the
    // headings feature contributes its parent set to, which are a slot's own
    // kind of peer declaration and are counted with the build's diagnostics
    // rather than with the tables.
    let diagnostics: Vec<(String, String)> = registry
        .build_diagnostics()
        .iter()
        .map(|d| (d.identifier().to_string(), d.message()))
        .collect();
    assert_eq!(diagnostics.len(), 9, "{diagnostics:?}");
    assert!(
        diagnostics.iter().all(|(identifier, _)| identifier == "flm.registry.unresolved-peer"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|(_, message)| message.contains("flm.counters")),
        "{diagnostics:?}"
    );
    assert_eq!(registry.set_aside_rules().count(), 0);
    assert!(registry.rule_idx("flm.textstyle.lower.latex-recomposed").is_none());
    let plan = common::stdcore::plan(&registry, latex_recomposed::WORKFLOW, &Overrides::none());
    assert!(plan.active_backends().contains(latex_recomposed::BACKEND));
}

// --- what the configuration and the checks of other plans leave untouched ------------------

#[test]
fn the_recomposed_latex_is_the_same_bytes_whatever_levels_are_set_as_run_in_headings() {
    // A run-in heading — one set at the start of the paragraph it introduces
    // rather than on a line of its own — is the text backend's rendering of
    // a listed level; whether `\paragraph` runs in is the document class's
    // business in LaTeX, so the recomposition rule writes the command as
    // written under any `HeadingsConfig::run_in_levels`.
    let document = "\
\\section{Top}

\\paragraph{Note}\\label{sec:note} Some content.

\\subparagraph{Detail} More content, see \\ref{sec:note}.
";
    let default = common::stdcore::registry(StdcoreConfig::new());
    let run_in = common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])),
    );
    let plain = recompose(&default, &[document], &Overrides::none());
    let configured = recompose(&run_in, &[document], &Overrides::none());
    let text = body(&plain);
    assert!(
        text.contains("\\flmHeading[toc=true]\n\\paragraph{Note}{\\flmLabels{\\label{ref1}}} Some content.\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "\\flmHeading[toc=true]\n\\subparagraph{Detail}{\\flmLabels{}} More content, see \\flmRef{ref1}.\n"
        ),
        "{text}"
    );
    assert_eq!(golden_text(&plain), golden_text(&configured));
    assert_eq!(identifiers(&plain), identifiers(&configured));
    // The setting is live in the second registry: the text backend sets the
    // two headings run in there and as blocks in the first.
    let run_in_text = common::spine::text_at(&run_in, document, None);
    assert!(run_in_text.contains("Note. Some content."), "{run_in_text}");
    let default_text = common::spine::text_at(&default, document, None);
    assert!(!default_text.contains("Note. Some content."), "{default_text}");
}

#[test]
fn a_label_of_a_type_its_element_refuses_is_reported_and_still_recomposed() {
    // A heading accepts the label types `sec` and `topic` only, so `eq:sum` on
    // a section is reported as `flm.refs.label-type-not-accepted` — by this
    // workflow as by `check`, since a render demands the validation collection
    // beside its output. The recomposed LaTeX is written all the same: the label is
    // allocated a safe key and written like any other, and the reference to it
    // is written around the same key. The refused label defines nothing, which
    // is what leaves the reference unresolved.
    let document = "\
\\section{Sum}\\label{eq:sum}

See \\ref{eq:sum}.
";
    let run = recomposed(document);
    let text = body(&run);
    assert!(text.contains("\\flmHeading[toc=true]\n\\section{Sum}{\\flmLabels{\\label{ref1}}}\n"), "{text}");
    assert!(text.contains("See \\flmRef{ref1}.\n"), "{text}");
    let table = run.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.allocated, 1);
    assert_eq!(table.label_of("ref", "ref1"), Some("eq:sum"));
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.label-type-not-accepted").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(
        identifiers(&run),
        ["flm.refs.unresolved", "flm.refs.label-type-not-accepted"]
    );
    assert_golden("refs-refused-type.tex", &run);
}

#[test]
fn a_recomposed_macro_is_written_guarded_and_a_bare_one_unconditionally() {
    // The two spellings the emitter writes for a recomposed macro, pinned over
    // definition nodes built by hand. Under its own prefix the name is
    // guarded, so a definition of it the user made before the style file is
    // loaded wins; under the name the document spells — which is what the
    // render option `bare_names` asks for — it is made unconditionally, since
    // the point of listing the name is to replace what else is defined under
    // it. `\long\def` says how many arguments it takes by writing the
    // parameters out, and has no opinion about the name, where
    // `\providecommand` refuses one whose first three letters are `end`.
    let registry = registry();
    let mut site = PresSite::new(&registry);
    let body = site.text("Hi");
    let node = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDaround", 2)
            .with_source_name("around"),
        vec![body],
    );
    let bare_body = site.text("Ho");
    let bare = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "around", 2),
        vec![bare_body],
    );
    let none_body = site.text("He");
    let none = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDplain", 0)
            .with_source_name("plain"),
        vec![none_body],
    );
    let tree = site.finish(vec![node, bare, none]);
    let emitted =
        latex_recomposed::emit_tree(&tree, &LatexRecomposedRenderOptions::default(), None);
    let fragment = emitted.fragment.expect("the tree emits");
    // A definition node is written nowhere in flow.
    assert_eq!(fragment.text, "");
    let definition = fragment.definitions.first().expect("the definition is recorded");
    assert_eq!(definition.name, "\\flmUDaround");
    assert_eq!(definition.source_name, "\\around");
    assert_eq!(definition.text, "\\providecommand\\flmUDaround[2]{Hi}\n");
    assert_eq!(fragment.definitions[1].text, "\\long\\def\\around#1#2{Ho}\n");
    // A definition of no arguments writes no count at all.
    assert_eq!(fragment.definitions[2].text, "\\providecommand\\flmUDplain{He}\n");
}

#[test]
fn a_robust_definition_is_written_through_the_document_command_interface() {
    // The spelling the render option `robust_names` asks for, pinned over
    // definition nodes built by hand. A macro written robustly takes the
    // document command interface with one `+m` per parameter — the `+` makes
    // each argument long, so it may hold a paragraph break — which makes an
    // engine-protected command: LaTeX writes the call itself into a table of
    // contents or an auxiliary file rather than what it stands for. The guard
    // is the same one an expandable definition has (`\Provide…`), and a name
    // the document owns takes the unconditional `\Declare…` for the reason
    // `bare_names` always did. A definition of no arguments writes an empty
    // argument specification, and a packed one writes a single `+m` holding
    // every argument.
    let registry = registry();
    let mut site = PresSite::new(&registry);
    let body = site.text("Hi");
    let node = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDaround", 2)
            .with_source_name("around")
            .robust(),
        vec![body],
    );
    let bare_body = site.text("Ho");
    let bare = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "around", 2).robust(),
        vec![bare_body],
    );
    let none_body = site.text("He");
    let none = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDplain", 0)
            .with_source_name("plain")
            .robust(),
        vec![none_body],
    );
    let packed_body = site.text("Hu");
    let packed = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDbig", 12)
            .with_source_name("big")
            .packed()
            .robust(),
        vec![packed_body],
    );
    let tree = site.finish(vec![node, bare, none, packed]);
    let emitted =
        latex_recomposed::emit_tree(&tree, &LatexRecomposedRenderOptions::default(), None);
    let fragment = emitted.fragment.expect("the tree emits");
    expect_none(emitted.diags.iter(), "flm.latex-recomposed.stage-failed");
    assert_eq!(
        fragment.definitions[0].text,
        "\\ProvideDocumentCommand\\flmUDaround{+m +m}{Hi}\n"
    );
    assert_eq!(fragment.definitions[1].text, "\\DeclareDocumentCommand\\around{+m +m}{Ho}\n");
    assert_eq!(fragment.definitions[2].text, "\\ProvideDocumentCommand\\flmUDplain{}{He}\n");
    assert_eq!(fragment.definitions[3].text, "\\ProvideDocumentCommand\\flmUDbig{+m}{Hu}\n");
}

#[test]
fn a_packed_definition_names_one_parameter_whatever_its_argument_count() {
    // The head of a packed definition, pinned over definition nodes built by
    // hand. A definition whose call hands every argument over in one group
    // names exactly **one** LaTeX parameter whatever its argument count says,
    // since that one parameter holds all of them; its body reads argument n
    // out of it with the style file's accessor. A bare name takes `\long\def`
    // with the one parameter written out. What the call writes into that one
    // parameter is pinned by the goldens
    // (`tests/goldens/latex/userdefs-packed*.tex`) and compiled by
    // `tests/latex_compile.rs`.
    let registry = registry();
    let mut site = PresSite::new(&registry);
    let body = site.text("Hi");
    let node = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDbig", 12)
            .with_source_name("big")
            .packed(),
        vec![body],
    );
    let bare_body = site.text("Ho");
    let bare = site.pres(
        DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "big", 12).packed(),
        vec![bare_body],
    );
    let tree = site.finish(vec![node, bare]);
    let emitted =
        latex_recomposed::emit_tree(&tree, &LatexRecomposedRenderOptions::default(), None);
    let fragment = emitted.fragment.expect("the tree emits");
    expect_none(emitted.diags.iter(), "flm.latex-recomposed.stage-failed");
    assert_eq!(fragment.definitions[0].text, "\\providecommand\\flmUDbig[1]{Hi}\n");
    assert_eq!(fragment.definitions[1].text, "\\long\\def\\big#1{Ho}\n");
}

// --- the style file named through the naming service ---------------------------------------

/// The spine's registry with `namer` contributed to the naming service's slot
/// `flm.artifacts.namer`.
fn registry_naming(namer: Arc<dyn ArtifactNamer>) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(common::spine::config()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(NAMER_SLOT, namer);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// A document whose recomposed LaTeX needs a style file.
const NEEDS_A_STYLE_FILE: &str = "\\section{A}\\label{sec:a} See \\ref{sec:a} and \\verbtext{v}.";

/// The eight hexadecimal digits of the style file's hash in `path`, which is
/// `<prefix><hash>.sty`.
fn hash_in<'p>(path: &'p str, prefix: &str) -> &'p str {
    let hash = path
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(".sty"))
        .unwrap_or_else(|| panic!("`{path}` is `{prefix}<hash>.sty`"));
    assert!(hash.len() == 8 && hash.chars().all(|ch| ch.is_ascii_hexdigit()), "{path}");
    hash
}

/// The region `style-package` the run wrote: the LaTeX package name a
/// compose template loads.
fn style_package(run: &Run) -> String {
    let regions = run.table(latex_recomposed::REGIONS).expect("declared").expect("written");
    regions.get(STYLE_PACKAGE).expect("the style file's region").text()
}

#[test]
fn the_naming_service_names_the_style_file_and_its_package() {
    let run = recompose(&registry_naming(Arc::new(JobNamer)), &[NEEDS_A_STYLE_FILE], &Overrides::none());
    expect_none(run.diagnostics().iter(), "flm.latex-recomposed.style-path");
    let (style, path) = style(&run);
    let hash = hash_in(&path, "job-flmrecomposed.");
    let package = format!("job-flmrecomposed.{hash}");
    assert!(body(&run).starts_with(&format!("% flm: style file {package}.sty\n")), "{}", body(&run));
    assert!(style.contains(&format!("\\ProvidesPackage{{{package}}}")), "{style}");
    assert_eq!(style_package(&run), package);
    // The artifact's identity carries the stem and the hash, and it gives its extension.
    let artifact = latex_recomposed::style_artifact(artifacts(&run)).expect("the style file");
    assert_eq!(artifact.id.to_string(), format!("latex-recomposed:flmrecomposed.{hash}"));
    assert_eq!(artifact.extension.as_deref(), Some("sty"));

    // Without a naming service, the file has the name it had: the same hash, which
    // the content without its header decides.
    let plain = recompose(&registry(), &[NEEDS_A_STYLE_FILE], &Overrides::none());
    let (_, plain_path) = common::latex::style(&plain);
    assert_eq!(plain_path, format!("flmrecomposed.{hash}.sty"));
    assert_eq!(style_package(&plain), format!("flmrecomposed.{hash}"));
}

#[test]
fn the_style_stem_names_the_artifact_and_the_naming_service_prefixes_it() {
    let render = LatexRecomposedRenderOptions {
        style_stem: String::from("mine"),
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose(&registry_naming(Arc::new(JobNamer)), &[NEEDS_A_STYLE_FILE], &backend_options(render));
    let (style, path) = style(&run);
    let hash = hash_in(&path, "job-mine.");
    assert!(style.contains(&format!("\\ProvidesPackage{{job-mine.{hash}}}")), "{style}");
}

#[test]
fn a_path_latex_cannot_load_is_refused_and_the_file_named_as_without_a_naming_service() {
    let run = recompose(&registry_naming(Arc::new(FixedNamer("files/x.sty"))), &[NEEDS_A_STYLE_FILE], &Overrides::none());
    let refused = expect(run.diagnostics().iter(), "flm.latex-recomposed.style-path").severity(Severity::Error).once();
    assert_eq!(
        refused.message().to_string(),
        "the style file must be a file beside the LaTeX document, named `NAME.sty`, and the program's \
         naming of files places it at `files/x.sty`"
    );
    let (style, path) = style(&run);
    let hash = hash_in(&path, "flmrecomposed.");
    assert!(style.contains(&format!("\\ProvidesPackage{{flmrecomposed.{hash}}}")), "{style}");
    assert!(body(&run).starts_with(&format!("% flm: style file flmrecomposed.{hash}.sty\n")), "{}", body(&run));

    // Another extension is refused alike.
    let run = recompose(&registry_naming(Arc::new(FixedNamer("style.tex"))), &[NEEDS_A_STYLE_FILE], &Overrides::none());
    expect(run.diagnostics().iter(), "flm.latex-recomposed.style-path").once();
    hash_in(&common::latex::style(&run).1, "flmrecomposed.");
}
