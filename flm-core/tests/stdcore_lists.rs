//! The `lists` feature on the text backend: the three shipped kinds, the
//! label beside an item, nesting, the short forms and `label=`, tags, the
//! numbers and references to items, the diagnostics, a registry without the
//! numbering feature and one without the references feature, a kind added by
//! configuration, and the documents that must neither panic nor loop.
//!
//! Everything runs over the standard package unless a test builds a registry
//! of its own. What a list prints is the text backend's list: each label
//! padded to the widest label of its list plus one column (none where every
//! label is empty), the items' blocks hanging at that column, one blank line
//! between two entries.

mod common;

use std::sync::Arc;

use flm_core::lang::text::plain_text;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::addr::TreeId;
use flm_core::model::aspects::Label;
use flm_core::stdcore::backends::lowering::feature as lowering_feature;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::lists::{self, ListKind, ListsConfig};
use flm_core::stdcore::features::numbering::{
    FormatOptions, NumberingOptions, WithinJoinerSpec, COUNTERS,
};
use flm_core::stdcore::features::refs::{self, LABELS};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::features::{index, refpoints, numbering};
use flm_core::stdcore::{check_feature, package, StdcoreConfig};
use techy::error::Severity;

use common::layout::assert_no_trailing_ws;
use common::lowering::{lowered, text as printed};
use common::stdcore::{golden_path, identifiers, registry, registry_over, reports, run_workflow};

// ---------------------------------------------------------------------------------------------
// running a document
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Renders the one tree `document` under the `text` workflow of `registry`.
fn render_with(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// Renders `document` under the `text` workflow of the standard package.
fn render(document: &str) -> Run {
    render_with(&std(), document)
}

/// What `document` prints under the standard package, checked for trailing
/// whitespace; the run must report nothing.
fn clean(document: &str) -> String {
    let run = render(document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{document}");
    let out = printed(&run);
    assert_no_trailing_ws(&out);
    out
}

/// The identifiers of the Errors of a run, in order.
fn errors(run: &Run) -> Vec<String> {
    reports(run)
        .into_iter()
        .filter(|(_, severity)| *severity == Severity::Error)
        .map(|(id, _)| id)
        .collect()
}

/// The text `\u{a0}` stands for: FLM's `~`, a no-break space, which the text
/// backend keeps as it is.
const NBSP: &str = "\u{a0}";

/// The number the run recorded for the item labeled `item:<name>`, written
/// without the word before it, or `-` for an item the run did not number.
fn number_of(run: &Run, name: &str) -> String {
    let labels = run.table(&LABELS).expect("declared").expect("collected");
    let point = *labels.by_label.get(&Label::new("item", name)).unwrap_or_else(|| panic!("no item `{name}`"));
    let counters = run.table(&COUNTERS).expect("declared").expect("numbered");
    match counters.format(point, &FormatOptions::new().without_word()) {
        None => "-".to_string(),
        Some(written) => written
            .expect("written")
            .iter()
            .map(|segment| plain_text(segment.content()))
            .collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// the three kinds
// ---------------------------------------------------------------------------------------------

#[test]
fn an_itemize_writes_a_bullet_beside_every_item() {
    assert_eq!(clean("\\begin{itemize}\\item First\\item Second\\end{itemize}"), "• First\n\n• Second\n");
}

#[test]
fn an_enumerate_numbers_its_items_from_one() {
    assert_eq!(clean("\\begin{enumerate}\\item Alpha\\item Beta\\end{enumerate}"), "1. Alpha\n\n2. Beta\n");
}

#[test]
fn a_description_writes_each_item_s_term_as_its_label() {
    assert_eq!(
        clean("\\begin{description}\\item[Entropy] A measure.\\item[Energy] Another.\\end{description}"),
        "Entropy A measure.\n\nEnergy  Another.\n"
    );
}

#[test]
fn the_list_is_staged_as_one_presentation_list_whose_kind_tokens_are_its_shape_name_and_labels() {
    let run = render("\\begin{enumerate}\\item a\\item b\\end{enumerate}");
    let tree = &lowered(&run, TreeId::tree(0)).tree;
    let list = tree.root().children().iter().next().expect("the list");
    assert_eq!(list.ext().element.name.as_str(), flm_core::stdcore::backends::pres::LIST);
    let payload = flm_core::stdcore::backends::pres::payload_of::<
        flm_core::stdcore::backends::pres::ListPayload,
    >(&list)
    .expect("a list payload");
    assert_eq!(
        payload.kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(),
        ["numbered", "enumerate", "labeled"]
    );
    assert_eq!(flm_core::stdcore::backends::pres::list_entries(&list).len(), 2);
}

// ---------------------------------------------------------------------------------------------
// nesting
// ---------------------------------------------------------------------------------------------

#[test]
fn a_list_nested_three_deep_takes_each_depth_s_template_and_the_outer_count_continues() {
    let document = "\\begin{enumerate}\
                    \\item one\
                    \\item two\
                    \\begin{enumerate}\\item two-a\
                    \\begin{enumerate}\\item deep\\item deeper\\end{enumerate}\
                    \\item two-b\\end{enumerate}\
                    \\item three\
                    \\end{enumerate}";
    assert_eq!(
        clean(document),
        "1. one\n\n2. two\n\n   (i)  two-a\n\n        a- deep\n\n        b- deeper\n\n   \
         (ii) two-b\n\n3. three\n"
    );
}

#[test]
fn two_sibling_inner_lists_each_restart() {
    let document = "\\begin{enumerate}\\item one\
                    \\begin{enumerate}\\item\\label{item:a} a\\item b\\end{enumerate}\
                    \\begin{enumerate}\\item\\label{item:c} c\\item\\label{item:d} d\\end{enumerate}\
                    \\item\\label{item:two} two\\end{enumerate}";
    let run = render(document);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(number_of(&run, "a"), "(i)");
    assert_eq!(number_of(&run, "c"), "(i)");
    assert_eq!(number_of(&run, "d"), "(ii)");
    assert_eq!(number_of(&run, "two"), "2");
}

#[test]
fn only_lists_of_the_same_kind_count_as_depth() {
    // An `enumerate` in an `itemize` in an `enumerate` is at depth 2, and an
    // `itemize` in an `enumerate` at depth 1.
    let document = "\\begin{enumerate}\\item one\
                    \\begin{itemize}\\item bullet\
                    \\begin{enumerate}\\item inner\\end{enumerate}\\end{itemize}\\end{enumerate}";
    assert_eq!(clean(document), "1. one\n\n   • bullet\n\n     (i) inner\n");
}

#[test]
fn a_nesting_joiner_writes_the_enclosing_item_before_a_nested_one() {
    let config = ListsConfig::new().environment(
        "enumerate",
        ListKind::numbered(["#{arabic}.", "(#{alph})"])
            .nesting(WithinJoinerSpec::Template("#{_parent}".to_string())),
    );
    let registry = registry(StdcoreConfig::new().lists(config));
    let run = render_with(
        &registry,
        "\\begin{enumerate}\\item a\\item b\\begin{enumerate}\\item\\label{item:x} x\\end{enumerate}\
         \\end{enumerate} \\ref{item:x}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), format!("1. a\n\n2. b\n\n   2.(a) x\n\nItem{NBSP}2.(a)\n"));
}

#[test]
fn a_list_nested_in_a_tagged_first_item_is_an_outermost_instance() {
    // The tagged item has no number to nest within: the nested list counts
    // from one on its own, in the style of its depth.
    let run = render(
        "\\begin{enumerate}\\item[T] tagged \\begin{enumerate}\\item\\label{item:x} x\
         \\item\\label{item:y} y\\end{enumerate}\\item\\label{item:z} z\\end{enumerate} \\ref{item:x}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(number_of(&run, "x"), "(i)");
    assert_eq!(number_of(&run, "y"), "(ii)");
    assert_eq!(number_of(&run, "z"), "1");
    assert_eq!(
        printed(&run),
        format!("T  tagged\n\n   (i)  x\n\n   (ii) y\n\n1. z\n\nItem{NBSP}(i)\n")
    );
}

#[test]
fn a_list_nested_in_a_tagged_item_nests_within_no_number_of_the_list_around_it() {
    let config = ListsConfig::new().environment(
        "enumerate",
        ListKind::numbered(["#{arabic}.", "(#{alph})"])
            .nesting(WithinJoinerSpec::Template("#{_parent}".to_string())),
    );
    let registry = registry(StdcoreConfig::new().lists(config));
    let run = render_with(
        &registry,
        "\\begin{enumerate}\\item a\
         \\item[T] t \\begin{enumerate}\\item\\label{item:x} x\\end{enumerate}\
         \\item b \\begin{enumerate}\\item\\label{item:y} y\\end{enumerate}\
         \\item c \\begin{enumerate}\\item[U] u \
         \\begin{enumerate}\\item\\label{item:w} w\\end{enumerate}\\end{enumerate}\\end{enumerate}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // Inside `T`, which follows item 1: no part of the list around it.
    assert_eq!(number_of(&run, "x"), "(a)");
    // Inside item 2: within it.
    assert_eq!(number_of(&run, "y"), "2.(a)");
    // Inside `U`, inside item 3: within item 3, at the depth of three lists.
    assert_eq!(number_of(&run, "w"), "3.(a)");
}

#[test]
fn a_list_written_before_the_first_item_is_an_outermost_instance() {
    let run = render(
        "\\begin{enumerate}\\begin{enumerate}\\item\\label{item:x} x\\end{enumerate}\
         \\item a\\end{enumerate}",
    );
    assert_eq!(errors(&run), ["flm.lists.content-before-item"]);
    assert_eq!(number_of(&run, "x"), "(i)");
}

// ---------------------------------------------------------------------------------------------
// the list's own template: label= and the short forms
// ---------------------------------------------------------------------------------------------

#[test]
fn a_label_template_writes_the_list_s_items() {
    assert_eq!(
        clean("\\begin{enumerate}[label=(#{alph})]\\item a\\item b\\end{enumerate}"),
        "(a) a\n\n(b) b\n"
    );
}

#[test]
fn the_short_forms_are_the_templates_of_latex_s_enumerate_package() {
    for (short, first, second) in
        [("(i)", "(i)", "(ii)"), ("1.", "1.", "2."), ("a-", "a-", "b-"), ("I)", "I)", "II)")]
    {
        let document = format!("\\begin{{enumerate}}[{short}]\\item x\\item y\\end{{enumerate}}");
        let out = clean(&document);
        assert!(out.starts_with(first) && out.contains(&format!("\n{second}")), "{short}: {out}");
    }
}

#[test]
fn a_short_form_without_a_number_character_is_a_fixed_label() {
    for fixed in ["-", "*", "+"] {
        let document = format!("\\begin{{itemize}}[{fixed}]\\item x\\item y\\end{{itemize}}");
        assert_eq!(clean(&document), format!("{fixed} x\n\n{fixed} y\n"));
    }
}

#[test]
fn a_short_form_with_two_number_characters_is_an_error_naming_both_readings() {
    let run = render("\\begin{enumerate}[i.a]\\item x\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.bad-label-form"]);
    let message = run.diagnostics().iter().next().expect("one").to_string();
    assert!(message.contains("#{roman}.a") && message.contains("i.#{alph}"), "{message}");
    // The depth's template is used.
    assert_eq!(printed(&run), "1. x\n");
}

#[test]
fn a_unicode_bullet_is_written_with_label_and_is_no_short_form() {
    assert_eq!(clean("\\begin{itemize}[label={\u{25e6}}]\\item x\\end{itemize}"), "\u{25e6} x\n");
    let run = render("\\begin{itemize}[\u{25e6}]\\item x\\end{itemize}");
    assert_eq!(errors(&run), ["flm.lists.unknown-key"]);
    assert_eq!(printed(&run), "• x\n");
}

#[test]
fn an_unknown_key_is_an_error_and_the_label_beside_it_still_applies() {
    let run = render("\\begin{enumerate}[start=3, label=#{Roman}.]\\item x\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.unknown-key"]);
    assert_eq!(printed(&run), "I. x\n");
}

#[test]
fn a_key_without_a_value_is_an_error() {
    let run = render("\\begin{enumerate}[label, x=1]\\item[tag, y=1] x\\end{enumerate}");
    assert_eq!(
        errors(&run),
        [
            "flm.lists.missing-value",
            "flm.lists.unknown-key",
            "flm.lists.missing-value",
            "flm.lists.unknown-key"
        ]
    );
    assert_eq!(printed(&run), "1. x\n");
    // A bracket holding no `=` is a short form or nothing: `[label]` is not a
    // key without a value but a text that is no short form.
    let run = render("\\begin{enumerate}[label]\\item x\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.unknown-key"]);
}

#[test]
fn the_python_spelling_of_a_template_reaches_the_unknown_key_error() {
    let run = render("\\begin{enumerate}[(${alph})]\\item x\\end{enumerate}");
    // The dollar sign is refused by the parse, and the bracket is neither a
    // short form nor a key.
    assert_eq!(errors(&run), ["flm.lang.dollar-math-disabled", "flm.lists.unknown-key"]);
    assert_eq!(printed(&run), "1. x\n");
}

#[test]
fn a_template_that_does_not_compile_is_reported_once_by_whoever_compiles_it() {
    // A numbered list under the numbering feature: the numbering stage
    // compiles the template, reports it once, and numbers the items in the
    // depth's style.
    let run = render("\\begin{enumerate}[label=#{nosuch}]\\item x\\item y\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.numbering.bad-number-format"]);
    assert_eq!(printed(&run), "1. x\n\n2. y\n");
    // An unnumbered list: the list's rule compiles it.
    let run = render("\\begin{itemize}[label=#{nosuch}]\\item x\\end{itemize}");
    assert_eq!(errors(&run), ["flm.lists.bad-label"]);
    assert_eq!(printed(&run), "• x\n");
}

#[test]
fn a_template_naming_a_number_on_an_unnumbered_list_is_an_error() {
    for bracket in ["(i)", "label=(#{alph})"] {
        let run = render(&format!("\\begin{{itemize}}[{bracket}]\\item x\\end{{itemize}}"));
        assert_eq!(errors(&run), ["flm.lists.label-names-number"], "{bracket}");
        assert_eq!(printed(&run), "• x\n");
    }
}

#[test]
fn an_empty_bracket_is_as_if_absent() {
    assert_eq!(clean("\\begin{enumerate}[]\\item x\\end{enumerate}"), "1. x\n");
}

#[test]
fn a_label_value_is_read_from_the_tree_s_characters_where_they_share_a_run_with_the_key() {
    // The key and the value are one run of characters, which the
    // block-structure pass rewrites (`--` into a dash): the value is the
    // value's own characters, never the whole run.
    assert_eq!(
        clean("\\begin{itemize}[label=--]\\item a\\item b\\end{itemize}"),
        "\u{2013} a\n\n\u{2013} b\n"
    );
    assert_eq!(clean("\\begin{itemize}[label=a--b]\\item x\\end{itemize}"), "a\u{2013}b x\n");
    assert_eq!(
        clean("\\begin{enumerate}[label=--]\\item\\label{item:x} x\\end{enumerate} \\ref{item:x}"),
        format!("\u{2013} x\n\nItem{NBSP}\u{2013}\n")
    );
}

#[test]
fn a_label_value_followed_by_a_run_of_spaces_is_the_value_alone() {
    // Two spaces after the comma: the run of characters is rewritten to one
    // space, and the next key is still read as a key of its own.
    let run =
        render("\\begin{enumerate}[label=(#{alph}),  start=2]\\item x\\item y\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.unknown-key"]);
    assert_eq!(printed(&run), "(a) x\n\n(b) y\n");
}

#[test]
fn a_label_template_passed_through_a_user_definition_is_the_argument_given() {
    let el = Definition::macro_("el", vec![ArgDef::mandatory("tpl")])
        .text("\\begin{enumerate}[label=#{tpl}]\\item\\label{item:a} a\\item b\\end{enumerate}");
    let registry =
        registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![el])));
    let run = render_with(&registry, "\\el{*} \\ref{item:a}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), format!("* a\n\n* b\n\nItem{NBSP}*\n"));
}

// ---------------------------------------------------------------------------------------------
// tags
// ---------------------------------------------------------------------------------------------

#[test]
fn a_tag_is_written_as_the_whole_bracket_or_as_the_tag_key() {
    assert_eq!(
        clean("\\begin{itemize}\\item[My tag] a\\item[tag={Other}] b\\item[tag={x=y}] c\\end{itemize}"),
        "My tag a\n\nOther  b\n\nx=y    c\n"
    );
}

#[test]
fn a_tagged_enumerate_item_counts_nothing() {
    let run = render(
        "\\begin{enumerate}\\item[Custom]\\label{item:c} c\\item\\label{item:a} a\\item b\\end{enumerate}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "Custom c\n\n1.     a\n\n2.     b\n");
    assert_eq!(number_of(&run, "c"), "-");
    assert_eq!(number_of(&run, "a"), "1");
}

// ---------------------------------------------------------------------------------------------
// references to items
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reference_shows_the_item_s_own_label() {
    let document = "\\begin{enumerate}\\item one\
                    \\begin{enumerate}\\item a\\item\\label{item:x} b\\end{enumerate}\\end{enumerate}\
                    \\begin{enumerate}[label=(#{alph})]\\item a\\item\\label{item:y} b\\end{enumerate}\
                    \\begin{enumerate}\\item[Custom]\\label{item:t} t\\end{enumerate}\
                    \\begin{description}\\item[Entropy]\\label{item:d} d\\end{description}\
                    \\begin{itemize}\\item\\label{item:u} u\\end{itemize}\
                    \\ref{item:x}; \\ref{item:y}; \\ref[noprefix]{item:x}; \\ref{item:t}; \\ref{item:d}; \
                    \\ref{item:u}.";
    let out = clean(document);
    let last = out.trim_end().rsplit("\n\n").next().expect("the paragraph");
    assert_eq!(
        last,
        format!("Item{NBSP}(ii); Item{NBSP}(b); (ii); Custom; Entropy; \u{2022}.")
    );
}

#[test]
fn a_reference_to_an_item_of_a_list_writing_its_own_fixed_label_shows_that_label() {
    let out = clean(
        "\\begin{itemize}[label={--}]\\item\\label{item:a} a\\end{itemize}\
         \\begin{itemize}[-]\\item\\label{item:b} b\\end{itemize}\
         \\ref{item:a}; \\ref{item:b}.",
    );
    assert_eq!(out, "\u{2013} a\n\n- b\n\n\u{2013}; -.\n");
}

#[test]
fn an_unbraced_tag_is_its_own_characters() {
    // The key and the value are one run of characters, which the
    // block-structure pass rewrites: the tag is the value's characters alone,
    // and a reference to the item shows them as the item does.
    let out = clean("\\begin{itemize}\\item[tag=a--b]\\label{item:x} x\\end{itemize} \\ref{item:x}");
    assert_eq!(out, "a\u{2013}b x\n\na\u{2013}b\n");
}

#[test]
fn a_fixed_label_on_a_numbered_list_still_numbers_the_items() {
    let run = render(
        "\\begin{enumerate}[label={*}]\\item\\label{item:a} a\\item\\label{item:b} b\\end{enumerate} \
         \\ref{item:b}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), format!("* a\n\n* b\n\nItem{NBSP}*\n"));
    // The record holds the number; the template writes the star.
    let counters = run.table(&COUNTERS).expect("declared").expect("numbered");
    let labels = run.table(&LABELS).expect("declared").expect("collected");
    let point = labels.by_label[&Label::new("item", "b")];
    assert_eq!(counters.record(point).expect("a record").value.number(), 2);
}

#[test]
fn an_explicit_empty_label_shows_no_label_and_still_numbers() {
    let run = render(
        "\\begin{enumerate}[label={}]\\item a\\item\\label{item:b} b\\end{enumerate} \\ref{item:b}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // No column is kept for labels that are all empty. The reference is the
    // word and an empty number: it ends in the word's no-break space.
    assert_eq!(printed(&run), format!("a\n\nb\n\nItem{NBSP}\n"));
    assert_eq!(number_of(&run, "b"), "");
}

#[test]
fn a_reference_inside_an_item_resolves() {
    assert_eq!(
        clean("\\begin{enumerate}\\item\\label{item:a} first\\item see \\ref{item:a}\\end{enumerate}"),
        format!("1. first\n\n2. see Item{NBSP}1\n")
    );
}

#[test]
fn an_item_is_anchored_on_its_label_or_on_its_position() {
    let run = render("\\begin{itemize}\\item\\label{item:x} a\\item b\\end{itemize}");
    let projected = format!("{:?}", common::stdcore::projection(&run, refs::ANCHORS));
    assert!(projected.contains("\"item-x\""), "{projected}");
    assert_eq!(projected.matches("\"item--").count(), 1, "{projected}");
}

#[test]
fn a_label_of_another_type_is_refused_by_the_label_check() {
    let run = render("\\begin{itemize}\\item\\label{sec:x} a\\end{itemize}");
    assert_eq!(errors(&run), ["flm.refs.label-type-not-accepted"]);
}

// ---------------------------------------------------------------------------------------------
// depth, trees, and the documents that must neither panic nor loop
// ---------------------------------------------------------------------------------------------

/// `depth` lists of `kind` nested one in the first item of the other, the
/// innermost item labeled `item:deep`.
fn nested(kind: &str, depth: usize) -> String {
    let open = format!("\\begin{{{kind}}}\\item ");
    let close = format!("\\end{{{kind}}}");
    format!("{}\\label{{item:deep}} deep{}", open.repeat(depth), close.repeat(depth))
}

#[test]
fn a_list_nested_six_deep_uses_the_last_template_past_the_third() {
    let run = render(&format!("{} \\ref{{item:deep}}", nested("enumerate", 6)));
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(number_of(&run, "deep"), "a");
    assert_eq!(printed(&run), format!("1. (i) a- a- a- a- deep\n\nItem{NBSP}a\n"));
}

#[test]
fn a_list_nested_twenty_deep_renders() {
    let run = render(&nested("enumerate", 20));
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(number_of(&run, "deep"), "a");
    let run = render(&nested("itemize", 20));
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn two_numbered_lists_in_two_trees_each_number_from_one() {
    let run = run_workflow(
        &std(),
        text::WORKFLOW,
        &[
            "\\begin{enumerate}\\item a\\item\\label{item:b} b\\end{enumerate}",
            "\\begin{enumerate}\\item\\label{item:c} c\\end{enumerate} \\ref{item:b}",
        ],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(number_of(&run, "b"), "2");
    assert_eq!(number_of(&run, "c"), "1");
    assert!(printed(&run).ends_with(&format!("Item{NBSP}2\n")));
}

#[test]
fn content_before_the_first_item_is_an_error_and_an_item_with_no_label() {
    let run = render("\\begin{enumerate} %% a comment\n stray \\item a\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.content-before-item"]);
    assert_eq!(printed(&run), "   stray\n\n1. a\n");
    // Whitespace and comments alone are skipped.
    assert_eq!(clean("\\begin{enumerate}\n %% a comment\n\n \\item a\\end{enumerate}"), "1. a\n");
}

#[test]
fn an_empty_item_is_an_item() {
    let run = render("\\begin{enumerate}\\item\\item b\\end{enumerate}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    assert_no_trailing_ws(&out);
    assert_eq!(out, "1.\n\n2. b\n");
}

#[test]
fn a_list_with_no_items_prints_nothing() {
    assert_eq!(clean("Before.\\begin{enumerate}\\end{enumerate}After."), "Before.\n\nAfter.\n");
}

#[test]
fn a_description_item_without_a_tag_is_an_error_and_shows_no_label() {
    let run = render("\\begin{description}\\item[Term] a\\item b\\end{description}");
    assert_eq!(errors(&run), ["flm.lists.missing-tag"]);
    assert_eq!(printed(&run), "Term a\n\n     b\n");
}

#[test]
fn an_item_outside_a_list_s_body_begins_no_item() {
    // Inside a list but not directly in its body: the block-structure pass
    // keeps the block where it stands, and the item's rule reports it.
    let run = render("\\begin{itemize}\\item a \\emph{\\item b}\\end{itemize}");
    assert_eq!(errors(&run), ["flm.blocks.block-in-inline", "flm.lists.item-outside-list"]);
    // Outside every list, `\item` is not defined at all.
    let run = render("\\item x");
    assert_eq!(errors(&run), ["core.specs.unresolvable-command"]);
}

#[test]
fn a_list_inside_a_heading_s_title_is_kept_in_place_and_reported_by_the_block_structure() {
    let run = render("\\section{A \\begin{itemize}\\item x\\end{itemize}}");
    assert_eq!(errors(&run), ["flm.blocks.block-in-inline"]);
    assert!(printed(&run).contains("• x"));
}

#[test]
fn the_labels_are_the_same_however_the_run_is_executed() {
    let document = format!(
        "{}\\begin{{enumerate}}[label=(#{{alph}})]\\item a\\item b\\end{{enumerate}}{}",
        nested("enumerate", 4),
        nested("itemize", 3)
    );
    let registry = std();
    let doc = common::pipeline::doc(&registry, &[&document]);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential =
        common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
}

// ---------------------------------------------------------------------------------------------
// registries without the numbering or the references feature, and configured kinds
// ---------------------------------------------------------------------------------------------

/// The features a text render needs, the lists feature among them, with
/// `extra` added. The `refpoints` feature is not among them: the lists feature
/// needs it only through the numbering or the references feature, so a caller
/// adding either adds it too.
fn minimal(extra: Vec<Arc<dyn flm_core::core::registry::FlmFeature>>) -> Arc<FlmRegistry> {
    let mut features = vec![
        lowering_feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()) as Arc<dyn flm_core::core::registry::FlmFeature>,
        check_feature(),
        index::feature(),
        lists::feature(),
    ];
    features.extend(extra);
    registry_over(features)
}

#[test]
fn without_the_numbering_feature_a_numbered_list_is_an_error_and_bullets_still_show() {
    let registry = minimal(vec![refpoints::feature(), refs::feature()]);
    // The counter and the three styles of `enumerate` are dropped, with a
    // Note for each of the two slots they were declared into.
    let notes: Vec<String> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.severity() == Severity::Note)
        .map(|d| d.to_string())
        .collect();
    for slot in ["flm.numbering.styles", "flm.numbering.counters"] {
        assert!(
            notes.iter().any(|note| note.contains("feature `lists`") && note.contains(slot)),
            "{notes:#?}"
        );
    }
    let run = render_with(
        &registry,
        "\\begin{enumerate}\\item a\\item[T] b\\end{enumerate}\
         \\begin{itemize}\\item x\\end{itemize}\
         \\begin{enumerate}[label={*}]\\item s\\end{enumerate}",
    );
    assert_eq!(errors(&run), ["flm.lists.no-numbering"]);
    assert_eq!(printed(&run), "  a\n\nT b\n\n• x\n\n* s\n");
}

#[test]
fn without_the_references_feature_the_labels_and_numbers_still_show() {
    let registry = minimal(vec![refpoints::feature(), numbering::feature()]);
    let run = render_with(
        &registry,
        "\\begin{enumerate}\\item\\label{item:a} a\\item[T] b\\item c\\end{enumerate}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "1. a\n\nT  b\n\n2. c\n");
}

#[test]
fn without_the_numbering_and_the_references_features_lists_need_no_reference_points() {
    let registry = minimal(vec![]);
    let run = render_with(
        &registry,
        "\\begin{itemize}\\item x\\item[T] y\\end{itemize}\
         \\begin{enumerate}[label={*}]\\item s\\end{enumerate}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "• x\n\nT y\n\n* s\n");
}

#[test]
fn a_kind_added_by_configuration_is_an_environment_with_its_own_counter() {
    let config = ListsConfig::new()
        .environment("steps", ListKind::numbered(["Step~#{arabic}:"]))
        .environment("dashes", ListKind::unnumbered(["--"]));
    let registry = registry(StdcoreConfig::new().lists(config));
    let run = render_with(
        &registry,
        "\\begin{steps}\\item\\label{item:s} Mix.\\item Bake.\\end{steps}\
         \\begin{dashes}\\item one\\end{dashes} \\ref{item:s}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        printed(&run),
        format!("Step{NBSP}1: Mix.\n\nStep{NBSP}2: Bake.\n\n\u{2013} one\n\nItem{NBSP}Step{NBSP}1:\n")
    );
}

#[test]
fn a_configuration_mistake_fails_the_registry_build() {
    let build = |config: ListsConfig| {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new().lists(config)));
        builder.use_package(flm_core::math_passthrough::package());
        builder.build(&RegistryInputs::new()).map(|_| ()).map_err(|error| error.render())
    };
    let message = build(ListsConfig::new().environment("bullets", ListKind::unnumbered(["(#{roman})"])))
        .expect_err("an unnumbered kind naming a number");
    assert!(message.contains("`bullets`") && message.contains("must not write a number"), "{message}");
    let message = build(ListsConfig::new().environment("steps", ListKind::numbered(Vec::<String>::new())))
        .expect_err("a numbered kind with no template");
    assert!(message.contains("`steps`"), "{message}");
    let message = build(ListsConfig::new().environment("bad", ListKind::unnumbered(["#{nosuch}"])))
        .expect_err("a template that does not compile");
    assert!(message.contains("`bad`"), "{message}");
}

#[test]
fn the_outermost_lists_can_be_numbered_within_the_sections_by_an_option() {
    use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
    use flm_core::stdcore::features::numbering::Within;
    let registry = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(1))),
    );
    let options = NumberingOptions::new()
        .within("flm.lists.enumerate", Within::parent("flm.headings.section"));
    let doc = common::pipeline::doc_with_options(
        &registry,
        &["\\section{A}\\begin{enumerate}\\item\\label{item:a} a\\end{enumerate}\
           \\section{B}\\begin{enumerate}\\item b\\item\\label{item:c} c\\end{enumerate}"],
        flm_core::model::options::OptionValues::new().with(options),
    );
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let run = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(number_of(&run, "a"), "1.1");
    assert_eq!(number_of(&run, "c"), "2.2");
}

// ---------------------------------------------------------------------------------------------
// the Python implementation's rows
// ---------------------------------------------------------------------------------------------

// The text renderer's rows `test_itemize`, `test_enumerate` and
// `test_nested_enumeration` of the Python implementation's
// `test_fragmentrenderer_text.py`, in this backend's list: labels padded to
// one column and no indentation of the list itself, and — by the depth rule
// that counts lists of one kind only — the bullet of the first depth for an
// `itemize` nested in an `enumerate`.
#[test]
fn the_python_text_rows_render_as_this_backend_writes_a_list() {
    assert_eq!(
        clean("\\begin{itemize}\n\\item First item\n\\item Second item\n\\end{itemize}"),
        "• First item\n\n• Second item\n"
    );
    assert_eq!(
        clean("\\begin{enumerate}\n\\item Alpha\n\\item Beta\n\\item Gamma\n\\end{enumerate}"),
        "1. Alpha\n\n2. Beta\n\n3. Gamma\n"
    );
    assert_eq!(
        clean(
            "\\begin{enumerate}\n\\item First\n\\item Second\n  \\begin{itemize}\n  \\item Sub A\n  \
             \\item Sub B\n  \\end{itemize}\n\\end{enumerate}"
        ),
        "1. First\n\n2. Second\n\n   • Sub A\n\n   • Sub B\n"
    );
}

// The text renderer's row `test_enumerate_roman` of the Python implementation's
// `test_feature_enumeration.py`: the tag template `i.` is a short form here
// too; the labels are padded on the right to one column, where the Python
// renderer aligned them on the right.
#[test]
fn the_python_roman_short_form_row() {
    assert_eq!(
        clean("\\begin{enumerate}[i.]\\item First\\item Second\\end{enumerate}"),
        "i.  First\n\nii. Second\n"
    );
}

// The Python implementation's `test_blocks_paragraphs_correct_number`
// (`test_flmenvironment.py`), whole: its document is four blocks there and
// six here, since a display-mathematics environment is a block here and
// splits the paragraph around it; the list is one block in both.
#[test]
fn the_python_block_count_document_is_six_blocks_the_list_one_of_them() {
    let document = "\n     \n\nTest\nsome\ninput \\textbf{string} here. Does this\n\
                    \\begin{equation}\n a = b\n\\end{equation}\nwork?\n\n\
                    \\textbf{A   new} paragraph is started    here.\n\n\
                    \\begin{enumerate}\n\\item one\n\\item two\n\\end{enumerate}\n\n      .\n";
    let run = render(document);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let blocks: Vec<String> = lowered(&run, TreeId::tree(0))
        .tree
        .root()
        .children()
        .iter()
        .map(|node| node.ext().element.name.to_string())
        .collect();
    use flm_core::stdcore::backends::pres::{LIST, MATH_PASSTHROUGH, PAR};
    assert_eq!(blocks, [PAR, MATH_PASSTHROUGH, PAR, PAR, LIST, PAR]);
}

// ---------------------------------------------------------------------------------------------
// the golden
// ---------------------------------------------------------------------------------------------

/// A document exercising the three kinds, nesting, a template, the short
/// forms, tags and references.
const EVERY_KIND: &str = "Lists of every kind.\n\n\
    \\begin{itemize}\n\\item A bullet.\n\\item Another, holding a list:\n\
    \\begin{itemize}\\item nested\\item again \\begin{itemize}\\item deeper\\end{itemize}\\end{itemize}\n\
    \\end{itemize}\n\n\
    \\begin{enumerate}\n\\item\\label{item:first} First.\n\\item Second, with steps:\n\
    \\begin{enumerate}\\item one \\item\\label{item:two} two\\end{enumerate}\n\
    \\item Third, whose text is long enough to wrap onto a second line at the width of eighty \
    columns.\n\\end{enumerate}\n\n\
    \\begin{enumerate}[label=(#{alph})]\\item\\label{item:a} a\\item b\\end{enumerate}\n\n\
    \\begin{enumerate}[I.]\\item one\\item two\\end{enumerate}\n\n\
    \\begin{itemize}[+]\\item plus\\item[Custom] tagged\\end{itemize}\n\n\
    \\begin{description}\n\\item[Entropy]\\label{item:entropy} A measure of disorder.\n\
    \\item[Energy] What is conserved.\n\\end{description}\n\n\
    See \\ref{item:first}, \\ref{item:two}, \\ref[noprefix]{item:two}, \\ref{item:a} and \
    \\ref{item:entropy}.\n";

#[test]
fn the_text_golden_of_every_kind() {
    let run = render(EVERY_KIND);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    assert_no_trailing_ws(&out);
    assert_text_golden(golden_path("lists.txt"), &out);
}

// ---------------------------------------------------------------------------------------------
// the recomposed LaTeX
// ---------------------------------------------------------------------------------------------

#[test]
fn the_recomposed_latex_writes_the_environment_its_items_their_tags_and_their_labels() {
    use flm_core::stdcore::backends::latex_recomposed;
    let run = run_workflow(
        &std(),
        latex_recomposed::WORKFLOW,
        &["\\begin{enumerate}[(i)]\n%% before\n\\item[Tag]\\label{item:x} a\n\\item b \\ref{item:x}\n\
           \\end{enumerate}"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let body = common::latex::body(&run);
    assert!(
        body.contains(
            "\\begin{enumerate}[label={(\\roman*)}]\n%% before\n\\item[Tag]\\flmLabels{\\label{ref1}} a\n\
             \\item b \\flmRef{ref1}\n\\end{enumerate}"
        ),
        "{body}"
    );
}

#[test]
fn the_recomposed_latex_reports_a_list_s_mistakes_as_the_other_backends_do() {
    use flm_core::stdcore::backends::{html, latex_recomposed};
    let document = "\\begin{itemize}[\u{2022}]\\item a\\end{itemize}\
                    \\begin{enumerate}[i.a]\\item b\\end{enumerate}\
                    \\begin{description}\\item c\\end{description}\
                    \\begin{enumerate} stray \\item d\\end{enumerate}\
                    \\begin{itemize}\\item[x=1] e\\end{itemize}";
    let expected = [
        "flm.lists.unknown-key",
        "flm.lists.bad-label-form",
        "flm.lists.missing-tag",
        "flm.lists.content-before-item",
        "flm.lists.unknown-key",
    ];
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
        let run = run_workflow(&std(), workflow, &[document], &Overrides::none());
        assert_eq!(errors(&run), expected, "{workflow}");
    }
}

// The recomposer's rows `test_itemize` and `test_enumerate` of the Python
// implementation's `test_feature_enumeration.py`: a list is written back as
// the document wrote it.
#[test]
fn the_recomposed_latex_writes_a_plain_list_back_as_written() {
    use flm_core::stdcore::backends::latex_recomposed;
    for document in [
        "\\begin{itemize}\\item First\\item Second\\end{itemize}",
        "\\begin{enumerate}\\item Alpha\\item Beta\\end{enumerate}",
    ] {
        let run = run_workflow(&std(), latex_recomposed::WORKFLOW, &[document], &Overrides::none());
        assert_eq!(identifiers(&run), Vec::<String>::new());
        let body = common::latex::body(&run);
        assert!(body.contains(document), "{body}");
    }
}

// ---------------------------------------------------------------------------------------------
// the HTML backend
// ---------------------------------------------------------------------------------------------

#[test]
fn the_general_rule_serves_the_html_backend() {
    use flm_core::stdcore::backends::html;
    let run = run_workflow(
        &std(),
        html::WORKFLOW,
        &["\\begin{enumerate}\\item a\\begin{itemize}\\item b\\end{itemize}\\end{enumerate}\
           \\begin{description}\\item[T] c\\end{description}"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let page = common::html::page(&run);
    for tag in ["<ol", "<ul", "<dl"] {
        assert!(page.contains(tag), "{tag}: {page}");
    }
    // The same labels as on the text backend.
    for label in [
        "<span class=\"flm-pres-list-label\">1.</span>",
        "<span class=\"flm-pres-list-label\">\u{2022}</span>",
        // A description list's term begins the entry, so it carries the
        // item's anchor.
        "<dt class=\"flm-pres-list-label\" id=\"item--3ouhf\">T</dt>",
    ] {
        assert!(page.contains(label), "{label}: {page}");
    }
}

// ---------------------------------------------------------------------------------------------
// reference templates: `ref=`, the short form's, the kind's by depth
// ---------------------------------------------------------------------------------------------

/// The last paragraph `document` prints under the standard package, which
/// must report nothing.
fn last_paragraph(document: &str) -> String {
    let out = clean(document);
    out.trim_end().rsplit("\n\n").next().expect("a paragraph").to_string()
}

#[test]
fn a_reference_to_an_enumerate_item_writes_the_depth_s_reference_template() {
    let document = "\\begin{enumerate}\\item one\\item\\label{item:b} two\
                    \\begin{enumerate}\\item a\\item\\label{item:ii} b\
                    \\begin{enumerate}\\item x\\item\\label{item:bb} y\\end{enumerate}\
                    \\end{enumerate}\\end{enumerate}\
                    See \\ref{item:b}, \\ref{item:ii}, \\ref{item:bb}.";
    let out = clean(document);
    // The labels beside the items are the label templates, as before.
    assert!(out.starts_with("1. one\n\n2. two\n\n   (i)  a\n\n   (ii) b\n\n        a- x\n\n        b- y"), "{out}");
    assert!(
        out.ends_with(&format!("See Item{NBSP}2, Item{NBSP}(ii), Item{NBSP}b.\n")),
        "{out}"
    );
}

#[test]
fn ref_alone_label_alone_and_both_set_their_templates() {
    // `ref=` alone: the label is the depth's, the reference the key's.
    assert_eq!(
        clean("\\begin{enumerate}[ref=(#{alph})]\\item a\\item\\label{item:b} b\\end{enumerate} \\ref{item:b}"),
        format!("1. a\n\n2. b\n\nItem{NBSP}(b)\n")
    );
    // `label=` alone sets both to the same text, nothing dropped.
    assert_eq!(
        clean("\\begin{enumerate}[label=#{alph}.]\\item a\\item\\label{item:b} b\\end{enumerate} \\ref{item:b}"),
        format!("a. a\n\nb. b\n\nItem{NBSP}b.\n")
    );
    // Both: each its own.
    assert_eq!(
        clean(
            "\\begin{enumerate}[label=#{Roman}:, ref=#{Roman}]\\item a\\item\\label{item:b} b\
             \\end{enumerate} \\ref{item:b}"
        ),
        format!("I:  a\n\nII: b\n\nItem{NBSP}II\n")
    );
}

#[test]
fn a_short_form_s_reference_drops_its_trailing_full_stops_colons_and_hyphens() {
    for (form, label, reference) in
        [("1.:", "2.:", "2"), ("(i)", "(ii)", "(ii)"), ("a-", "b-", "b"), ("1.", "2.", "2")]
    {
        let out = clean(&format!(
            "\\begin{{enumerate}}[{form}]\\item a\\item\\label{{item:b}} b\\end{{enumerate}} \\ref{{item:b}}"
        ));
        let entry = out.split("\n\n").nth(1).expect("the second item");
        assert!(entry.starts_with(label), "{form}: {out}");
        assert!(out.ends_with(&format!("Item{NBSP}{reference}\n")), "{form}: {out}");
    }
    // A fixed short form has no number: its reference shows the label.
    assert_eq!(
        clean("\\begin{enumerate}[-]\\item a\\item\\label{item:b} b\\end{enumerate} \\ref{item:b}"),
        format!("- a\n\n- b\n\nItem{NBSP}-\n")
    );
}

#[test]
fn a_nested_item_under_a_joining_template_is_labeled_and_referred_to_through_its_two_templates() {
    let config = ListsConfig::new().environment(
        "enumerate",
        ListKind::numbered(["#{arabic}.", "(#{alph})"])
            .ref_labels(["#{arabic}", "(#{roman})"])
            .nesting(WithinJoinerSpec::Template("#{_parent}.".to_string())),
    );
    let registry = registry(StdcoreConfig::new().lists(config));
    let run = render_with(
        &registry,
        "\\begin{enumerate}\\item a\\item b\\begin{enumerate}\\item\\label{item:x} x\
         \\item\\label{item:y} y\\end{enumerate}\\end{enumerate} \\ref{item:x}; \\ref{item:y}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The enclosing number is written through its reference template, `2`,
    // then the joiner's full stop, beside the item and in a reference alike.
    assert_eq!(
        printed(&run),
        format!("1. a\n\n2. b\n\n   2.(a) x\n\n   2.(b) y\n\nItem{NBSP}2.(i); Item{NBSP}2.(ii)\n")
    );
}

#[test]
fn a_reference_to_several_items_joins_their_reference_numbers() {
    let document = "\\begin{enumerate}\\item\\label{item:a} a\\item\\label{item:b} b\
                    \\item\\label{item:c} c\\end{enumerate}\
                    \\ref{item:a,item:c}; \\ref{item:a,item:b,item:c}";
    assert_eq!(last_paragraph(document), format!("Items{NBSP}1,3; Items{NBSP}1\u{2013}3"));
}

#[test]
fn a_ref_on_an_unnumbered_list_is_a_note_and_changes_nothing() {
    for bracket in ["ref=#{arabic}", "label={--}, ref=*", "ref=#{nosuch}"] {
        let run = render(&format!(
            "\\begin{{itemize}}[{bracket}]\\item\\label{{item:a}} a\\item b\\end{{itemize}} \\ref{{item:a}}"
        ));
        assert_eq!(
            reports(&run),
            [("flm.lists.ref-ignored".to_string(), Severity::Note)],
            "{bracket}"
        );
        let expected = if bracket.starts_with("label") { "\u{2013}" } else { "\u{2022}" };
        assert_eq!(printed(&run), format!("{expected} a\n\n{expected} b\n\n{expected}\n"), "{bracket}");
    }
}

#[test]
fn a_ref_that_does_not_compile_is_reported_once_by_whoever_compiles_it() {
    // Under the numbering feature, the stage compiles the reference template
    // and numbers the item in the depth's style; the label is the depth's.
    let run = render("\\begin{enumerate}[ref=#{nosuch}]\\item\\label{item:a} x\\end{enumerate} \\ref{item:a}");
    assert_eq!(errors(&run), ["flm.numbering.bad-number-format"]);
    assert_eq!(printed(&run), format!("1. x\n\nItem{NBSP}1\n"));
    // A label that does not compile beside a reference template that does:
    // the list's rule compiles the label, and the depth's label is used.
    let run = render("\\begin{enumerate}[label=#{nosuch}, ref=#{alph}]\\item\\label{item:a} x\\end{enumerate} \\ref{item:a}");
    assert_eq!(errors(&run), ["flm.lists.bad-label"]);
    assert_eq!(printed(&run), format!("1. x\n\nItem{NBSP}a\n"));
    // Without the numbering feature, the list's rule compiles both.
    let registry = minimal(vec![refpoints::feature(), refs::feature()]);
    let run = render_with(&registry, "\\begin{enumerate}[label={*}, ref=#{nosuch}]\\item x\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.bad-label"]);
    assert_eq!(printed(&run), "* x\n");
}

#[test]
fn an_unnumbered_kind_s_reference_templates_are_ignored() {
    // Neither compiled nor checked: one naming a number builds, and a
    // reference to an item shows the label beside it.
    for refs in [["#{arabic}"], ["*"]] {
        let config = ListsConfig::new().environment("dashes", ListKind::unnumbered(["-"]).ref_labels(refs));
        let registry = registry(StdcoreConfig::new().lists(config));
        let run = render_with(&registry, "\\begin{dashes}\\item\\label{item:a} a\\end{dashes} \\ref{item:a}");
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{refs:?}");
        assert_eq!(printed(&run), "- a\n\n-\n", "{refs:?}");
    }
}

#[test]
fn a_reference_to_an_item_of_an_unnumbered_list_whose_own_label_is_refused_shows_the_depth_s_label() {
    for (bracket, error) in [
        ("label=#{arabic}", "flm.lists.label-names-number"),
        ("1.", "flm.lists.label-names-number"),
        ("label=#{nosuch}", "flm.lists.bad-label"),
    ] {
        let run = render(&format!(
            "\\begin{{itemize}}[{bracket}]\\item\\label{{item:a}} x\\end{{itemize}} \\ref{{item:a}}"
        ));
        assert_eq!(errors(&run), [error], "{bracket}");
        assert_eq!(printed(&run), "\u{2022} x\n\n\u{2022}\n", "{bracket}");
    }
}

#[test]
fn a_ref_without_a_value_is_an_error() {
    let run = render("\\begin{enumerate}[ref, label=(#{alph})]\\item x\\end{enumerate}");
    assert_eq!(errors(&run), ["flm.lists.missing-value"]);
    assert_eq!(printed(&run), "(a) x\n");
}
