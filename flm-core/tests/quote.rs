//! The `quote` feature: the configuration and its refusals, the environments
//! and their section commands, the block and the sections a quotation is
//! lowered to, the lines of a lines section, the three errors and their
//! recoveries, and documents that must neither panic nor loop; over the
//! `text` workflow.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::doc::FragmentLayout;
use flm_core::model::element::element_value;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::text::{self, TextRenderOptions};
use flm_core::stdcore::features::quote::{
    QuoteConfig, QuoteElement, QuoteEnvironment, QuoteSection, SectionElement, sections_package,
};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::serialize::SerialValue;

use common::lowering::{lowered_projection, text as printed};
use common::stdcore::{identifiers, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries, runs and the shape of a lowered tree
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The standard package with the quote feature under `config`.
fn with_config(config: QuoteConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().quote(config))
}

/// What the registry build under `config` reported, rendered; a build that
/// succeeds is a test failure.
fn build_error(config: QuoteConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().quote(config)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

/// Runs the `text` workflow of `registry` over `document`.
fn render(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// The text `document` renders to under the default configuration.
fn text_of(document: &str) -> String {
    printed(&render(&std(), document)).trim_end().to_string()
}

/// A lowered tree's projection written compactly, one node per term: a
/// presentation node as its name after `flm.pres.` with its payload's role,
/// kinds and indentation in braces and its body in parentheses; text as a
/// quoted string; any other node as its element's name.
fn shape(value: &SerialValue) -> String {
    let field = |value: &SerialValue, key: &str| -> Option<SerialValue> {
        match value {
            SerialValue::Map(entries) => {
                entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
            }
            _ => None,
        }
    };
    let string = |value: Option<SerialValue>| match value {
        Some(SerialValue::Str(s)) => Some(s),
        _ => None,
    };
    match value {
        SerialValue::List(items) => items.iter().map(shape).collect::<Vec<_>>().join(" "),
        _ => {
            if let Some(chars) = string(field(value, "chars")) {
                return format!("{chars:?}");
            }
            let element = string(field(value, "elem")).unwrap_or_default();
            let name = element.strip_prefix("flm.pres.").unwrap_or(&element).to_string();
            let mut facts = Vec::new();
            if let Some(payload) = field(value, "payload") {
                if let Some(role) = string(field(&payload, "role")) {
                    facts.push(role);
                }
                if let Some(SerialValue::List(kinds)) = field(&payload, "kinds") {
                    facts.extend(kinds.iter().filter_map(|kind| match kind {
                        SerialValue::Str(kind) => Some(kind.clone()),
                        _ => None,
                    }));
                }
                if let Some(indent) = string(field(&payload, "indent_left")) {
                    facts.push(indent);
                }
            }
            let facts = if facts.is_empty() { String::new() } else { format!("{{{}}}", facts.join(" ")) };
            let body = match field(value, "body") {
                Some(SerialValue::List(items)) if items.is_empty() => String::new(),
                Some(body) => format!("({})", shape(&body)),
                None => String::new(),
            };
            format!("{name}{facts}{body}")
        }
    }
}

/// The compact shape of the lowered tree of `document` under `registry`,
/// and the identifiers of what the run reported.
fn lowered(registry: &Arc<FlmRegistry>, document: &str) -> (String, Vec<String>) {
    let run = render(registry, document);
    (shape(&lowered_projection(&run)), identifiers(&run))
}

/// [`lowered`] under the default configuration, asserting that nothing is
/// reported.
fn clean(document: &str) -> String {
    let (shape, reported) = lowered(&std(), document);
    assert_eq!(reported, Vec::<String>::new(), "{document}");
    shape
}

// ---------------------------------------------------------------------------------------------
// the configuration and the declarations (P1, P2, P4; tests 1, 6, 21)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_default_configuration_declares_the_three_environments() {
    let registry = std();
    for name in ["quote", "blockquote", "address"] {
        let document = format!("\\begin{{{name}}}\\end{{{name}}}");
        let source = Arc::new(FlmSource::new(document.as_str()).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(
            registry.lang(),
            source,
            FragmentLayout::default(),
            &ParseOptions::default(),
        )
        .expect("a parse");
        let values: Vec<String> = fragment
            .tree()
            .root()
            .descendants()
            .filter_map(|node| element_value::<QuoteElement>(&node).map(|value| value.name.clone()))
            .collect();
        assert_eq!(values, [name], "{document}");
    }
}

#[test]
fn a_configured_environment_renders_under_its_own_sections() {
    let config = QuoteConfig::new()
        .environment("myquote", QuoteEnvironment::new([QuoteSection::Text]))
        .environment("aside", QuoteEnvironment::new([]).bare(QuoteSection::Block));
    let registry = with_config(config);
    let (shape, reported) = lowered(&registry, "\\begin{myquote}\\text{A passage.}\\end{myquote}");
    assert_eq!(shape, r#"block{flm.quote myquote}(block{flm.quote.text}(par("A passage.")))"#);
    assert_eq!(reported, Vec::<String>::new());
    let (shape, reported) = lowered(&registry, "\\begin{aside}\nOne.\n\nTwo.\n\\end{aside}");
    assert_eq!(
        shape,
        r#"block{flm.quote aside}(block{flm.quote.block}(par("One.") par("Two.")))"#
    );
    assert_eq!(reported, Vec::<String>::new());
}

#[test]
fn two_environments_accepting_one_section_command_each_hold_it_in_their_own_package() {
    // `quote` and `myquote` both accept `\text`: the one element is declared
    // under the one name in two scoped packages, which the registry admits.
    let config =
        QuoteConfig::new().environment("myquote", QuoteEnvironment::new([QuoteSection::Text]));
    let registry = with_config(config);
    for environment in ["quote", "myquote"] {
        let document = format!("\\begin{{{environment}}}\\text{{x}}\\end{{{environment}}}");
        let (shape, reported) = lowered(&registry, &document);
        assert_eq!(
            shape,
            format!(r#"block{{flm.quote {environment}}}(block{{flm.quote.text}}(par("x")))"#)
        );
        assert_eq!(reported, Vec::<String>::new());
    }
    assert_eq!(sections_package("lines"), "flm.quote.sections.lines");
}

#[test]
fn a_section_command_the_environment_does_not_accept_is_no_command_there() {
    let config =
        QuoteConfig::new().environment("myquote", QuoteEnvironment::new([QuoteSection::Text]));
    let (shape, reported) =
        lowered(&with_config(config), "\\begin{myquote}\\attributed{x}\\end{myquote}");
    // `\attributed` is an unknown macro there: bare content, reported, shown.
    assert!(shape.starts_with("block{flm.quote myquote}(par(unknown"), "{shape}");
    assert!(reported.contains(&String::from("flm.quote.bare-content")), "{reported:?}");
    assert!(reported.iter().any(|id| id.ends_with("unresolvable-command")), "{reported:?}");
}

#[test]
fn an_environment_named_as_a_declared_one_a_reserved_kind_or_admitting_nothing_is_refused() {
    let clash = build_error(
        QuoteConfig::new().environment("itemize", QuoteEnvironment::new([QuoteSection::Text])),
    );
    assert!(clash.contains("itemize"), "{clash}");
    let theorem = build_error(
        QuoteConfig::new().environment("theorem", QuoteEnvironment::new([QuoteSection::Text])),
    );
    assert!(theorem.contains("theorem"), "{theorem}");
    let reserved = build_error(
        QuoteConfig::new().environment("numbered", QuoteEnvironment::new([QuoteSection::Text])),
    );
    // The name blamed stands before the last colon; the reserved list follows it.
    let blamed = reserved.rsplit_once(':').map_or("", |(blamed, _)| blamed);
    assert!(blamed.contains("`numbered`"), "{reserved}");
    let nothing = build_error(QuoteConfig::new().environment("empty", QuoteEnvironment::new([])));
    assert!(nothing.contains("`empty`"), "{nothing}");
}

#[test]
fn no_quote_environment_declares_nothing_and_still_builds() {
    let registry = with_config(QuoteConfig::empty());
    let (_, reported) = lowered(&registry, "\\begin{quote}x\\end{quote}");
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert!(reported[0].ends_with("unknown-environment"), "{reported:?}");
}

// ---------------------------------------------------------------------------------------------
// the block and its sections (P3; tests 1, 2)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_quotation_is_one_block_holding_one_block_per_section_in_the_order_written() {
    assert_eq!(
        clean("\\begin{quote}\\text{To be or not to be.}\\attributed{Shakespeare}\\end{quote}"),
        concat!(
            r#"block{flm.quote quote}(block{flm.quote.text}(par("To be or not to be.")) "#,
            r#"block{flm.quote.attributed}(par("Shakespeare")))"#
        )
    );
    assert_eq!(
        clean(
            "\\begin{quote}\n  \\attributed{A}\n  %% a comment\n  \\block{B}\n\n  \\lines{C}\n  \
             \\text{D}\n\\end{quote}"
        ),
        concat!(
            r#"block{flm.quote quote}(block{flm.quote.attributed}(par("A")) "#,
            r#"block{flm.quote.block}(par("B")) "#,
            r#"block{flm.quote.lines}(lines(line{0tab}("C"))) "#,
            r#"block{flm.quote.text}(par("D")))"#
        )
    );
}

#[test]
fn a_block_section_holds_its_paragraphs_and_a_passage_one_paragraph() {
    assert_eq!(
        clean("\\begin{blockquote}\nFirst paragraph.\n\nSecond paragraph.\n\\end{blockquote}"),
        concat!(
            r#"block{flm.quote blockquote}(block{flm.quote.block}("#,
            r#"par("First paragraph.") par("Second paragraph.")))"#
        )
    );
    assert_eq!(
        clean("\\begin{quote}\\block{one line}\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.block}(par("one line")))"#
    );
    assert_eq!(
        clean("\\begin{quote}\\block{two\n\nparagraphs}\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.block}(par("two") par("paragraphs")))"#
    );
    assert_eq!(
        clean("\\begin{quote}\\text{  spaced out  }\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.text}(par("spaced out")))"#
    );
}

// ---------------------------------------------------------------------------------------------
// the lines (P5, P5b; tests 3, 4)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_address_is_one_lines_section_of_indented_lines() {
    assert_eq!(
        clean("\\begin{address}\n123 Main St\\\\\n\\indent Suite 4\\\\\nCity, ST 12345\n\\end{address}"),
        concat!(
            r#"block{flm.quote address}(block{flm.quote.lines}(lines("#,
            r#"line{0tab}("123 Main St") line{1tab}("Suite 4") line{0tab}("City, ST 12345"))))"#
        )
    );
}

#[test]
fn a_line_is_trimmed_and_indented_by_its_leading_indents() {
    assert_eq!(
        clean("\\begin{quote}\\lines{  a  \\\\  b  }\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.lines}(lines(line{0tab}("a") line{0tab}("b"))))"#
    );
    assert_eq!(
        clean("\\begin{quote}\\lines{\\indent\\indent x\\\\ \\indent %% c\n \\indent y}\\end{quote}"),
        concat!(
            r#"block{flm.quote quote}(block{flm.quote.lines}(lines("#,
            r#"line{2tab}("x") line{2tab}("y"))))"#
        )
    );
}

#[test]
fn empty_lines_and_an_empty_lines_section() {
    assert_eq!(
        clean("\\begin{quote}\\lines{a\\\\\\\\b}\\end{quote}"),
        concat!(
            r#"block{flm.quote quote}(block{flm.quote.lines}(lines("#,
            r#"line{0tab}("a") line{0tab} line{0tab}("b"))))"#
        )
    );
    assert_eq!(
        clean("\\begin{quote}\\lines{a\\\\}\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.lines}(lines(line{0tab}("a") line{0tab})))"#
    );
    assert_eq!(
        clean("\\begin{quote}\\lines{}\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.lines}(lines))"#
    );
}

#[test]
fn an_indent_after_the_start_of_its_line_is_reported_and_left_out() {
    let (shape, reported) = lowered(&std(), "\\begin{quote}\\lines{a \\indent b}\\end{quote}");
    assert_eq!(
        shape,
        r#"block{flm.quote quote}(block{flm.quote.lines}(lines(line{0tab}("a " "b"))))"#
    );
    assert_eq!(reported, ["flm.quote.indent-mid-line"]);
}

// ---------------------------------------------------------------------------------------------
// the errors (P6; test 5)
// ---------------------------------------------------------------------------------------------

#[test]
fn bare_content_in_quote_is_reported_once_per_run_and_shown_in_place() {
    let (shape, reported) = lowered(&std(), "\\begin{quote}Bare text.\\text{x}\\end{quote}");
    assert_eq!(
        shape,
        r#"block{flm.quote quote}(par("Bare text.") block{flm.quote.text}(par("x")))"#
    );
    assert_eq!(reported, ["flm.quote.bare-content"]);
    let (shape, reported) = lowered(
        &std(),
        "\\begin{quote}\nOne.\n\n\\begin{itemize}\\item a\\end{itemize}\n\\text{x}\nTwo.\n\\end{quote}",
    );
    assert!(shape.starts_with(r#"block{flm.quote quote}(par("One.") list"#), "{shape}");
    assert!(shape.ends_with(r#"block{flm.quote.text}(par("x")) par("Two."))"#), "{shape}");
    // Two runs: the paragraph with the list, and the paragraph after `\text`.
    assert_eq!(reported, ["flm.quote.bare-content"; 2]);
    // Whitespace and a comment alone between two sections are no run.
    let (_, reported) =
        lowered(&std(), "\\begin{quote}\\text{a}\n  %% c\n\n\\text{b}\n\\end{quote}");
    assert_eq!(reported, Vec::<String>::new());
}

#[test]
fn a_section_inside_another_section_is_reported_and_shows_its_content() {
    let (shape, reported) = lowered(&std(), "\\begin{quote}\\lines{\\text{x}}\\end{quote}");
    assert_eq!(
        shape,
        r#"block{flm.quote quote}(block{flm.quote.lines}(lines(line{0tab}("x"))))"#
    );
    // Outside section position the command is inline content: one mistake,
    // one report.
    assert_eq!(reported, ["flm.quote.nested-section"]);
    let (shape, reported) =
        lowered(&std(), "\\begin{quote}\\text{a \\lines{b\\\\c}}\\end{quote}");
    assert_eq!(shape, r#"block{flm.quote quote}(block{flm.quote.text}(par("a " "b" "c")))"#);
    assert_eq!(reported, ["flm.quote.nested-section"]);
}

#[test]
fn sections_beside_inline_bare_content_render_with_no_report() {
    // A verse: its bare content lines, closed by an attribution. The body is
    // inline content, where each section command is inline too.
    let config = QuoteConfig::new()
        .environment("verse", QuoteEnvironment::new([QuoteSection::Attributed]).bare(QuoteSection::Lines))
        .environment("motto", QuoteEnvironment::new([QuoteSection::Attributed]).bare(QuoteSection::Text));
    let registry = with_config(config);
    let (shape, reported) = lowered(
        &registry,
        "\\begin{verse}\nRoses are red,\\\\\n\\indent violets are blue.\n\\attributed{Anonymous}\n\\end{verse}",
    );
    assert_eq!(
        shape,
        concat!(
            r#"block{flm.quote verse}(block{flm.quote.lines}(lines("#,
            r#"line{0tab}("Roses are red,") line{1tab}("violets are blue."))) "#,
            r#"block{flm.quote.attributed}(par("Anonymous")))"#
        )
    );
    assert_eq!(reported, Vec::<String>::new());
    let (shape, reported) =
        lowered(&registry, "\\begin{motto}Carpe diem. \\attributed{Horace}\\end{motto}");
    assert_eq!(
        shape,
        concat!(
            r#"block{flm.quote motto}(block{flm.quote.text}(par("Carpe diem.")) "#,
            r#"block{flm.quote.attributed}(par("Horace")))"#
        )
    );
    assert_eq!(reported, Vec::<String>::new());
}

#[test]
fn an_enclosing_quotations_section_inside_a_nested_one_is_bare_content_there() {
    // Scoped packages stack: `\text`, the enclosing `quote`'s, stays a command
    // inside the nested `blockquote`, which does not accept it. It is bare
    // content of that body — here read as a block section, so no report —
    // and shows its content.
    let (shape, reported) = lowered(
        &std(),
        "\\begin{quote}\\block{\\begin{blockquote}\\text{x}\\end{blockquote}}\\end{quote}",
    );
    assert_eq!(
        shape,
        concat!(
            r#"block{flm.quote quote}(block{flm.quote.block}("#,
            r#"block{flm.quote blockquote}(block{flm.quote.block}(par("x")))))"#
        )
    );
    assert_eq!(reported, Vec::<String>::new());
    // In a nested environment reading no bare content, it is reported once, as
    // bare content, and not a second time as a nested section.
    let config = QuoteConfig::new().environment("inner", QuoteEnvironment::new([QuoteSection::Lines]));
    let (shape, reported) = lowered(
        &with_config(config),
        "\\begin{quote}\\block{\\begin{inner}\\text{x}\\end{inner}}\\end{quote}",
    );
    assert_eq!(
        shape,
        concat!(
            r#"block{flm.quote quote}(block{flm.quote.block}("#,
            r#"block{flm.quote inner}(par("x"))))"#
        )
    );
    assert_eq!(reported, ["flm.quote.bare-content"]);
}

// ---------------------------------------------------------------------------------------------
// the text output (P16, P5b; test 7)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_output_prints_the_sections_as_blocks() {
    assert_eq!(
        text_of("\\begin{quote}\\text{To be or not to be.}\\attributed{Shakespeare}\\end{quote}"),
        "To be or not to be.\n\nShakespeare"
    );
    assert_eq!(
        text_of("\\begin{address}\n123 Main St\\\\\nCity, ST 12345\n\\end{address}"),
        "123 Main St\nCity, ST 12345"
    );
    assert_eq!(
        text_of("\\begin{quote}\\lines{Line one\\\\\n\\indent Line two}\\end{quote}"),
        "Line one\n    Line two"
    );
}

#[test]
fn the_tab_width_is_a_render_option() {
    let registry = std();
    let options = TextRenderOptions { tab_width: 2, ..TextRenderOptions::default() };
    let overrides = Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() };
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["\\begin{quote}\\lines{Line one\\\\\n\\indent Line two\\\\\\indent\\indent three}\\end{quote}"],
        &overrides,
    );
    assert_eq!(printed(&run).trim_end(), "Line one\n  Line two\n    three");
}

#[test]
fn a_quotation_renders_on_the_page_and_in_the_recomposed_latex() {
    let registry = std();
    let document = "\\begin{quote}\\text{a}\\lines{b\\\\\\indent c}\\attributed{d}\\end{quote}";
    for workflow in ["html", "latex-recomposed"] {
        let run = run_workflow(&registry, workflow, &[document], &Overrides::none());
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{workflow}");
    }
    // The recomposed LaTeX writes the environment back, each section
    // through its hook (`tests/quote_latex.rs`).
    let run = run_workflow(&registry, "latex-recomposed", &[document], &Overrides::none());
    let written = "\\begin{quote}\\flmQuoteText{a}\\flmQuoteLines{b\\\\\\relax\n\\flmIndent{1\\flmTabWidth}c}\
                   \\flmQuoteAttributed{d}\\end{quote}";
    assert!(common::latex::body(&run).contains(written), "{}", common::latex::body(&run));
}

// ---------------------------------------------------------------------------------------------
// quotations among the other features (tests 10, 19)
// ---------------------------------------------------------------------------------------------

#[test]
fn text_inside_a_formula_inside_a_quotation_is_the_mathematics_one() {
    let (shape, reported) =
        lowered(&std(), "\\begin{quote}\\text{a \\(\\text{x}\\) b}\\end{quote}");
    assert_eq!(reported, Vec::<String>::new());
    // The formula's `\text` is the `math` feature's own element, lowered as
    // content inside the formula, not as a quotation's text.
    assert!(shape.contains(r#"math-passthrough(math-text("x"))"#), "{shape}");
    assert_eq!(shape.matches("flm.quote.text").count(), 1, "{shape}");
}

#[test]
fn a_formula_in_a_lines_section_and_a_list_in_a_block_section() {
    let (shape, reported) =
        lowered(&std(), "\\begin{quote}\\lines{\\(x^2\\) \\\\ y}\\end{quote}");
    assert_eq!(reported, Vec::<String>::new());
    assert!(shape.starts_with(r#"block{flm.quote quote}(block{flm.quote.lines}(lines(line{0tab}(math-passthrough"#), "{shape}");
    assert_eq!(
        text_of("\\begin{quote}\\block{\\begin{itemize}\\item a\\item b\\end{itemize}}\\end{quote}"),
        "• a\n\n• b"
    );
    assert_eq!(
        text_of("\\begin{itemize}\\item \\begin{quote}\\text{q}\\end{quote}\\end{itemize}"),
        "• q"
    );
}

// ---------------------------------------------------------------------------------------------
// execution settings, and documents that must neither panic nor loop (test 20)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_is_the_same_sequentially_and_in_parallel() {
    let registry = std();
    let texts = [
        "\\begin{quote}\\text{a}\\lines{b\\\\\\indent c}\\attributed{d}\\end{quote}",
        "\\begin{address}\nx\\\\y\n\\end{address}",
        "\\begin{blockquote}\nOne.\n\nTwo.\n\\end{blockquote}",
    ];
    let doc = common::pipeline::doc(&registry, &texts);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential =
        common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    assert_eq!(identifiers(&sequential), Vec::<String>::new());
}

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let registry = std();
    let mut nested = String::from("x");
    for _ in 0..10 {
        nested = format!("\\begin{{quote}}\\block{{{nested}}}\\end{{quote}}");
    }
    let run = render(&registry, &nested);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run).trim_end(), "x");
    assert_eq!(
        clean("\\begin{quote}\\lines{\\\\}\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.lines}(lines(line{0tab} line{0tab})))"#
    );
    assert_eq!(
        clean("\\begin{quote}\\attributed{}\\end{quote}"),
        r#"block{flm.quote quote}(block{flm.quote.attributed})"#
    );
    for document in [
        "\\begin{quote}",
        "\\begin{quote}\\text{",
        "\\begin{quote}\\lines{\\indent",
        "\\begin{address}\\\\\\\\\\\\\\end{address}",
    ] {
        let run = render(&registry, document);
        let _ = printed(&run);
    }
}

#[test]
fn the_section_element_carries_its_kind() {
    let registry = std();
    let source = Arc::new(
        FlmSource::new("\\begin{quote}\\text{a}\\lines{b}\\attributed{c}\\block{d}\\end{quote}")
            .with_origin(FlmOrigin::none()),
    );
    let fragment =
        parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("a parse");
    let kinds: Vec<QuoteSection> = fragment
        .tree()
        .root()
        .descendants()
        .filter_map(|node| element_value::<SectionElement>(&node).map(|value| value.section))
        .collect();
    assert_eq!(kinds, QuoteSection::ALL);
}
