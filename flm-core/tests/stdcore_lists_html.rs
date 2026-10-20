//! The `lists` feature on the `html` backend: the three shipped kinds, nesting,
//! tags, a list's own label template and its short forms, the anchor of every
//! item and the references to it, a list beside the other features inside an
//! item, a list standing in a paragraph's run of text, and a list environment
//! a host adds.
//!
//! What a list writes is the general lowering rule's `flm.pres.list` — kind
//! tokens and entries — which this backend writes as `<ol>`, `<ul>` or `<dl>`
//! by the structural kind token, with each label as a span at the start of its
//! entry (a description list's `<dt>`) and each item's anchor as the
//! identifier of the element that begins the entry. Every golden this suite
//! writes is parsed by `tests/html_validity.rs` in the same test run, which
//! also holds that every link into the page names an identifier the page has.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{html, pres, text};
use flm_core::stdcore::features::lists::{ListKind, ListsConfig};

use common::html::{assert_golden, fragment, page};
use common::lists::ACROSS_FEATURES;
use common::stdcore::{identifiers, registry, run_workflow};

// ---------------------------------------------------------------------------------------------
// running a document
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// The run of the `html` workflow of `registry` over `texts`, one tree each.
fn render_with(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, html::WORKFLOW, texts, &Overrides::none())
}

/// The HTML of `texts` under the standard package; the run must report
/// nothing.
fn clean(texts: &[&str]) -> String {
    let run = render_with(&std(), texts);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    fragment(&run)
}

// ---------------------------------------------------------------------------------------------
// the kinds
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_kinds_are_an_ordered_an_unordered_and_a_description_list() {
    let out = clean(&["\\begin{enumerate}\\item First\\item Second\\end{enumerate}\n\n\
                       \\begin{itemize}\\item First\\item Second\\end{itemize}\n\n\
                       \\begin{description}\\item[Entropy] A measure.\\item[Energy] Another.\
                       \\end{description}"]);
    // The structural token first, the environment's name second, then the
    // token that says every entry carries its own label.
    for list in [
        "<ol class=\"flm-pres-list\" data-flm-kind=\"numbered enumerate labeled\">",
        "<ul class=\"flm-pres-list\" data-flm-kind=\"unnumbered itemize labeled\">",
        "<dl class=\"flm-pres-list\" data-flm-kind=\"terms description labeled\">",
    ] {
        assert!(out.contains(list), "{list}: {out}");
    }
    assert_golden("lists-kinds.html", &out);
}

// The two HTML rows of the Python implementation's renderer tests for a list,
// an enumeration and an itemization. That renderer wrote every list as a
// description list with the label as its `<dt>`; this backend writes an
// ordered or an unordered list, the label as a span at the start of each
// entry, and each entry carries its item's anchor.
#[test]
fn the_python_rows_render_as_this_backend_writes_a_list() {
    let out = clean(&[
        "\\begin{enumerate}\\item First\\item Second\\end{enumerate}",
        "\\begin{itemize}\\item Alpha\\item Beta\\end{itemize}",
    ]);
    assert_golden("lists-python-rows.html", &out);
}

#[test]
fn a_host_s_list_environment_is_written_by_its_structure_not_by_its_name() {
    // A numbered kind and a kind with no template, both named nothing the
    // backend knows: the first is an ordered list and the second a
    // description list, because the structural token says so.
    let config = StdcoreConfig::new().lists(
        ListsConfig::new()
            .environment("steps", ListKind::numbered(["Step #{arabic}:"]))
            .environment("glossary", ListKind::unnumbered(Vec::<String>::new())),
    );
    let run = render_with(
        &registry(config),
        &["\\begin{steps}\\item Mix.\\item Bake.\\end{steps}\n\n\
           \\begin{glossary}\\item[Flour] Ground grain.\\end{glossary}"],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let out = fragment(&run);
    assert!(out.contains("<ol class=\"flm-pres-list\" data-flm-kind=\"numbered steps labeled\">"));
    assert!(out.contains("<span class=\"flm-pres-list-label\">Step 2:</span>"), "{out}");
    assert!(out.contains("<dl class=\"flm-pres-list\" data-flm-kind=\"terms glossary labeled\">"));
    assert_golden("lists-configured.html", &out);
}

#[test]
fn a_list_environment_named_as_a_reserved_kind_token_fails_the_registry_build() {
    use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
    use flm_core::stdcore::backends::pres::LIST_RESERVED_KINDS;
    use flm_core::stdcore::package;
    // A stylesheet selecting `terms` would match an environment of that name
    // too, so none of the four words names a list environment.
    for word in LIST_RESERVED_KINDS {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(
            StdcoreConfig::new().lists(ListsConfig::new().environment(word, ListKind::unnumbered(["-"]))),
        ));
        builder.use_package(flm_core::math_passthrough::package());
        let message = builder.build(&RegistryInputs::new()).map(|_| ()).expect_err(word).render();
        assert!(message.contains(&format!("the list environment name `{word}` is reserved: a list environment must not be named")), "{message}");
        assert!(message.contains("the words that the output uses to mark the kinds of lists"), "{message}");
    }
}

// ---------------------------------------------------------------------------------------------
// nesting, tags and templates
// ---------------------------------------------------------------------------------------------

#[test]
fn nested_lists_each_write_their_own_labels() {
    let out = clean(&["\\begin{enumerate}\n\
                       \\item One\n\
                       \\item Two\n\
                       \\begin{enumerate}\n\
                       \\item Two-one\n\
                       \\begin{enumerate}\\item Deep\\end{enumerate}\n\
                       \\item Two-two\n\
                       \\end{enumerate}\n\
                       \\item Three\n\
                       \\begin{itemize}\n\
                       \\item Bullet\n\
                       \\begin{itemize}\\item Dash\\end{itemize}\n\
                       \\begin{enumerate}\\item Inside a bullet\\end{enumerate}\n\
                       \\end{itemize}\n\
                       \\end{enumerate}"]);
    // The outer count goes on past the nested list, each depth has its own
    // template, and a list of another kind starts again at depth 1.
    for label in ["3.", "(i)", "(ii)", "a-", "\u{2022}", "-"] {
        assert!(
            out.contains(&format!("<span class=\"flm-pres-list-label\">{label}</span>")),
            "{label}: {out}"
        );
    }
    assert_golden("lists-nested.html", &out);
}

#[test]
fn a_tag_is_the_label_of_its_item() {
    let out = clean(&["\\begin{enumerate}\n\
                       \\item[Custom] Tagged, and counting nothing.\n\
                       \\item First\n\
                       \\item[tag={\\emph{Rich} tag}] With markup.\n\
                       \\item Second\n\
                       \\end{enumerate}\n\n\
                       \\begin{itemize}\\item[\\(\\star\\)] Star.\\item Bullet.\\end{itemize}\n\n\
                       \\begin{description}\\item[\\textbf{Bold} term] Its description.\
                       \\end{description}"]);
    for label in [
        "<span class=\"flm-pres-list-label\">Custom</span>",
        "<span class=\"flm-pres-list-label\">1.</span>",
        "<span class=\"flm-pres-list-label\">2.</span>",
        "<span class=\"flm-pres-list-label\"><em class=\"flm-pres-span\" data-flm-style=\"emph\">Rich</em> tag</span>",
    ] {
        assert!(out.contains(label), "{label}: {out}");
    }
    assert_golden("lists-tags.html", &out);
}

#[test]
fn a_list_s_own_template_writes_its_labels() {
    let out = clean(&["\\begin{enumerate}[label=(#{alph})]\\item a\\item b\\end{enumerate}\n\n\
                       \\begin{enumerate}[(i)]\\item a\\item b\\end{enumerate}\n\n\
                       \\begin{enumerate}[1.]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[A)]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[label={*}]\\item a\\end{enumerate}\n\n\
                       \\begin{itemize}[label={--}]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[-]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[*]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[+]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[label={}]\\item a\\end{itemize}"]);
    // `--` is an en dash here as everywhere in running text.
    for label in ["(a)", "(b)", "(i)", "(ii)", "1.", "A)", "*", "\u{2013}", "-", "+"] {
        assert!(
            out.contains(&format!("<span class=\"flm-pres-list-label\">{label}</span>")),
            "{label}: {out}"
        );
    }
    assert_golden("lists-templates.html", &out);
}

// ---------------------------------------------------------------------------------------------
// anchors and references
// ---------------------------------------------------------------------------------------------

#[test]
fn every_item_carries_its_anchor_and_a_reference_lands_on_it() {
    let out = clean(&["\\begin{enumerate}\n\
                       \\item\\label{item:a} First, before \\ref{item:b} and \\ref{item:t}.\n\
                       \\item\\label{item:b}\\label{item:bb} Second.\n\
                       \\item Unlabeled.\n\
                       \\item[{Tag}]\\label{item:t} Tagged.\n\
                       \\end{enumerate}\n\n\
                       \\begin{description}\\item[Term]\\label{item:d} Its text.\\end{description}\n\n\
                       See \\ref{item:a}, \\ref[noprefix]{item:bb} and \\ref{item:d}."]);
    // A labeled item is anchored on its first label, an unlabeled one on its
    // position; a description list's term carries the anchor.
    for anchor in [
        "<li id=\"item-a\">",
        "<li id=\"item-b\">",
        "<li id=\"item-t\">",
        "<dt class=\"flm-pres-list-label\" id=\"item-d\">",
    ] {
        assert!(out.contains(anchor), "{anchor}: {out}");
    }
    assert!(out.contains("<li id=\"item--"), "{out}");
    for link in ["href=\"#item-a\"", "href=\"#item-b\"", "href=\"#item-t\"", "href=\"#item-d\""] {
        assert!(out.contains(link), "{link}: {out}");
    }
    assert_golden("lists-references.html", &out);
}

#[test]
fn a_registry_without_the_references_feature_writes_no_item_anchor() {
    use flm_core::core::registry::FlmFeature;
    use flm_core::stdcore::backends::lowering::feature as lowering_feature;
    use flm_core::stdcore::check_feature;
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::{index, refpoints, lists, numbering};
    let without_refs = common::stdcore::registry_over(vec![
        lowering_feature(),
        flm_core::stdcore::backends::pieces::feature(),
        html::feature(),
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        check_feature(),
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        lists::feature(),
    ]);
    let run = render_with(&without_refs, &["\\begin{enumerate}\\item a\\item b\\end{enumerate}"]);
    let out = fragment(&run);
    assert!(out.contains("<li><span class=\"flm-pres-list-label\">1.</span>"), "{out}");
    assert!(!out.contains(" id=\""), "{out}");
}

// ---------------------------------------------------------------------------------------------
// items the anchors table or the counter table holds nothing for
// ---------------------------------------------------------------------------------------------

/// The runs of `document` under the `html` and the `text` workflows of the
/// standard package.
fn html_and_text(document: &str) -> (Run, Run) {
    let text = run_workflow(&std(), text::WORKFLOW, &[document], &Overrides::none());
    (render_with(&std(), &[document]), text)
}

/// The tables the run's `flm.run.missing-entry` reports name, in order.
fn missing_tables(run: &Run) -> Vec<String> {
    run.diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.run.missing-entry")
        .map(|d| {
            let message = d.to_string();
            let table = message.split("in table `").nth(1).expect("a table named");
            table.split('`').next().expect("the table's name").to_string()
        })
        .collect()
}

#[test]
fn a_list_inside_a_label_template_is_written_and_reported_once() {
    // A label template holding a list: the list stands inside a value the
    // lowering synthesized, whose items no anchors stage saw. The list is
    // reported once and written, its items with no identifier.
    let document = "\\begin{itemize}[label={\\begin{itemize}\\item x\\end{itemize}}]\\item y\\end{itemize}";
    let (html, text) = html_and_text(document);
    for run in [&html, &text] {
        assert_eq!(
            identifiers(run),
            ["flm.blocks.block-in-inline", "flm.run.missing-entry"],
            "{}",
            run.diagnostics().render_all()
        );
        assert_eq!(missing_tables(run), ["flm.anchors"]);
    }
    let out = fragment(&html);
    assert_eq!(out.matches("class=\"flm-pres-list\"").count(), 2, "{out}");
    // The inner list's item carries no identifier.
    assert!(out.contains("<li><span class=\"flm-pres-list-label\">\u{2022}</span>"), "{out}");
    assert!(!out.contains("flm-pres-unsupported"), "{out}");
    assert_eq!(common::lowering::text(&text).trim_end(), "\u{2022}x y");
}

#[test]
fn a_numbered_list_inside_a_label_template_shows_no_number_and_is_reported_once_per_table() {
    // The items of a numbered list inside a synthesized value were seen
    // neither by the numbering stage nor by the anchors stage: the list is
    // reported once for each table, and its items show no number.
    let document = "\\begin{enumerate}[label={\\begin{enumerate}\\item x\\item z\\end{enumerate}#{arabic}}]\
                    \\item y\\end{enumerate}";
    let (html, text) = html_and_text(document);
    for run in [&html, &text] {
        assert_eq!(
            identifiers(run),
            ["flm.blocks.block-in-inline", "flm.run.missing-entry", "flm.run.missing-entry"],
            "{}",
            run.diagnostics().render_all()
        );
        assert_eq!(missing_tables(run), ["flm.counters", "flm.anchors"]);
    }
    // The inner items carry neither a label nor an identifier; the outer
    // item's label, the only one, holds them and its number.
    let out = fragment(&html);
    assert_eq!(out.matches("<li>").count(), 2, "{out}");
    assert_eq!(out.matches("class=\"flm-pres-list-label\"").count(), 1, "{out}");
    assert_eq!(common::lowering::text(&text).trim_end(), "xz1 y");
}

#[test]
fn an_item_the_numbering_stage_did_not_number_shows_no_label_and_is_reported_by_the_stage_alone() {
    use flm_core::core::run::{Externals, RunOptions};
    use flm_core::model::options::OptionValues;
    use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
    use flm_core::stdcore::features::numbering::{NumberingOptions, Within};
    // Lists numbered within the sections, and one standing before the first
    // section: the numbering stage sees its item and gives it no number,
    // which the stage reports; the list's rule adds nothing.
    let registry = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(1))),
    );
    let options = NumberingOptions::new()
        .within("flm.lists.enumerate", Within::parent("flm.headings.section"));
    let doc = common::pipeline::doc_with_options(
        &registry,
        &["\\begin{enumerate}\\item a\\end{enumerate}\\section{A}\\begin{enumerate}\\item b\\end{enumerate}"],
        OptionValues::new().with(options),
    );
    let plan = common::stdcore::plan(&registry, html::WORKFLOW, &Overrides::none());
    let run = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    assert_eq!(
        identifiers(&run),
        ["flm.numbering.parent-uninitialized"],
        "{}",
        run.diagnostics().render_all()
    );
    let out = fragment(&run);
    assert!(out.contains("<span class=\"flm-pres-list-label\">1.1.</span>"), "{out}");
    assert_eq!(out.matches("class=\"flm-pres-list-label\"").count(), 1, "{out}");
}

// ---------------------------------------------------------------------------------------------
// beside the other features
// ---------------------------------------------------------------------------------------------

#[test]
fn an_item_holds_a_formula_a_reference_a_verbatim_block_and_a_nested_list() {
    let out = clean(&[ACROSS_FEATURES]);
    // The list in running text ends the paragraph before it and a new one
    // begins after it, as a block in running text does.
    assert!(out.contains("<p class=\"flm-pres-par\">Before the list, a list in running text</p>"));
    assert!(out.contains("<p class=\"flm-pres-par\">and the sentence goes on.</p>"), "{out}");
    assert!(out.contains("<pre class=\"flm-pres-verbatim\""), "{out}");
    assert!(out.contains("href=\"#item-code\""), "{out}");
    assert_golden("lists-across-features.html", &out);
}

#[test]
fn a_page_of_lists_carries_the_lists_stylesheet_once() {
    let run = render_with(&std(), &[ACROSS_FEATURES]);
    let page = page(&run);
    assert_eq!(page.matches("/* flm.html.style.lists */").count(), 1, "{page}");
    // The stylesheet lays a list whose entries carry their own labels out as
    // a grid with one column of labels, each entry handing its cells up to it
    // and its marker switched off, and sets a list of terms apart, through
    // the kind tokens the vocabulary names.
    let selector = |token: &str| format!(".flm-pres-list[data-flm-kind~=\"{token}\"]");
    let grid = format!("{} {{\n    display: grid;", selector(pres::LIST_LABELED));
    assert!(page.contains(&grid), "{page}");
    // The subgrid and the switched-off marker are declared in the entry's own
    // rule, the text between its selector and the closing brace.
    let entry = format!("{} > li {{", selector(pres::LIST_LABELED));
    let start = page.find(&entry).unwrap_or_else(|| panic!("{page}")) + entry.len();
    let rule = &page[start..start + page[start..].find('}').unwrap_or_else(|| panic!("{page}"))];
    assert!(rule.contains("grid-template-columns: subgrid;"), "{rule}");
    assert!(rule.contains("list-style: none;"), "{rule}");
    assert!(page.contains(&format!("{} > dt {{", selector(pres::LIST_LABELED))), "{page}");
    assert!(page.contains(&format!("{} {{", selector(pres::LIST_TERMS))), "{page}");
    assert!(page.contains(&format!("{} > dt {{", selector(pres::LIST_TERMS))), "{page}");
}

// ---------------------------------------------------------------------------------------------
// one label on every backend, and documents that must not panic
// ---------------------------------------------------------------------------------------------

/// The labels of a document whose every item is one word, as the `text`
/// backend prints them: each line of an entry is the label, padding, the word.
fn text_labels(document: &str) -> Vec<String> {
    let run = run_workflow(
        &std(),
        flm_core::stdcore::backends::text::WORKFLOW,
        &[document],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    common::lowering::text(&run)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim_end().rsplit_once(' ').map_or("", |(label, _)| label.trim()).to_string())
        .collect()
}

/// The labels of `html`, in order: the content of every label span and term.
fn html_labels(html: &str) -> Vec<String> {
    let mut labels = Vec::new();
    for piece in html.split("class=\"flm-pres-list-label\"").skip(1) {
        let start = piece.find('>').expect("the tag's end") + 1;
        let end = piece[start..].find("</").expect("the label's end") + start;
        labels.push(piece[start..end].to_string());
    }
    labels
}

#[test]
fn the_labels_beside_the_items_are_the_text_backend_s() {
    let document = "\\begin{enumerate}\\item a\\item b\\item[T] c\\item d\\end{enumerate}\n\n\
                    \\begin{enumerate}[label=(#{alph})]\\item a\\item b\\end{enumerate}\n\n\
                    \\begin{enumerate}[I.]\\item a\\item b\\end{enumerate}\n\n\
                    \\begin{itemize}[+]\\item a\\end{itemize}\n\n\
                    \\begin{itemize}\\item a\\item[x] b\\end{itemize}\n\n\
                    \\begin{description}\\item[Term] a\\end{description}";
    let html = clean(&[document]);
    let expected = ["1.", "2.", "T", "3.", "(a)", "(b)", "I.", "II.", "+", "\u{2022}", "x", "Term"];
    assert_eq!(html_labels(&html), expected);
    assert_eq!(text_labels(document), expected);
}

#[test]
fn a_list_nested_twenty_deep_is_written() {
    for kind in ["enumerate", "itemize", "description"] {
        let open = format!("\\begin{{{kind}}}\\item[t] ");
        let close = format!("\\end{{{kind}}}");
        let document = format!("{}deep{}", open.repeat(20), close.repeat(20));
        let out = clean(&[&document]);
        assert_eq!(out.matches("class=\"flm-pres-list\"").count(), 20, "{kind}");
    }
}

// ---------------------------------------------------------------------------------------------
// reference templates
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reference_to_an_item_writes_its_reference_template_and_the_label_its_label_template() {
    let run = render_with(
        &std(),
        &["\\begin{enumerate}\\item\\label{item:a} a\
           \\begin{enumerate}\\item\\label{item:i} i\\end{enumerate}\\end{enumerate}\n\n\
           \\begin{enumerate}[1.:]\\item\\label{item:c} c\\end{enumerate}\n\n\
           \\begin{enumerate}[label=#{Alph}:, ref=#{Alph}]\\item\\label{item:d} d\\end{enumerate}\n\n\
           \\begin{itemize}[ref=#{arabic}]\\item\\label{item:e} e\\end{itemize}\n\n\
           See \\ref{item:a}, \\ref{item:i}, \\ref{item:c}, \\ref{item:d} and \\ref{item:e}."],
    );
    assert_eq!(identifiers(&run), ["flm.lists.ref-ignored"]);
    let out = fragment(&run);
    for label in ["1.", "(i)", "1.:", "A:", "\u{2022}"] {
        assert!(out.contains(&format!("<span class=\"flm-pres-list-label\">{label}</span>")), "{label}: {out}");
    }
    for reference in ["Item&#xA0;1<", "Item&#xA0;(i)<", "Item&#xA0;1<", "Item&#xA0;A<", ">\u{2022}<"] {
        assert!(out.contains(reference), "{reference}: {out}");
    }
    assert_golden("lists-ref-templates.html", &out);
}
