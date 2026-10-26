//! The two restricted argument shapes of the language — the characters-only
//! group (`\label{…}`, `\ref{…}`) and the verbatim argument (`\verbcode{…}`) —
//! and the placeholder of an author definition's template inside them: the
//! parsers keep the placeholder escape alive under a template parse and nowhere
//! else, `##` is a literal `#` in both, and the one difference between the two
//! shapes (comments and nested groups) holds under both parses; the one reader
//! of such content, by segments, and the label and target readers built on it;
//! and a reference's key, read from such an argument, set apart from the words
//! of a heading's title.

mod common;

use flm_core::core::plan::Overrides;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::lang::{FlmCallableType, FlmGroupType};
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use techy::core::node::{NodeRef, NodeTree};
use techy::latexlike::Mode;

use common::userdefs::{expanded, recomposed, setup, Setup};
use common::{element, parse, lang};

// --- helpers -------------------------------------------------------------------------------

/// `\lbl{…}`, a macro whose one argument is a characters-only group.
fn chars_macro() -> ElementDef {
    ElementDef::new(
        "t",
        ["lbl"],
        SyntaxDef::Macro { args: vec![ArgDef::new(ArgStdKind::CharsGroup)] },
        element("t.lbl"),
    )
}

/// `\verb{…}`, a macro whose one argument is a verbatim argument delimited by
/// braces.
fn verbatim_macro() -> ElementDef {
    ElementDef::new(
        "t",
        ["verb"],
        SyntaxDef::Macro {
            args: vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: Some(('{', '}')) })],
        },
        element("t.verb"),
    )
}

/// The language and the compiled definitions of a test: the two test macros
/// beside `definitions`.
fn with_definitions(definitions: Vec<Definition>) -> Setup {
    let setup = setup(definitions, vec![chars_macro(), verbatim_macro()]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    setup
}

/// The compiled text-mode template tree of the definition `name`.
fn template_tree<'s>(setup: &'s Setup, name: &str) -> &'s NodeTree<flm_core::lang::Flm> {
    let compiled = setup.definition(name).compiled().expect("compiled");
    compiled.variant(Mode::Text).expect("a text template").template().tree()
}

/// The first node named `name` under `root`.
fn named<'t>(
    root: NodeRef<'t, flm_core::lang::Flm>,
    name: &str,
) -> NodeRef<'t, flm_core::lang::Flm> {
    root.descendants()
        .find(|node| node.name() == Some(name))
        .unwrap_or_else(|| panic!("no node named {name:?}"))
}

/// The shape of a run of nodes, one word per node: `chars:<text>`,
/// `placeholder:<reference>`, `group`, `comment`, or `callable:<name>`.
fn shapes<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, flm_core::lang::Flm>>) -> Vec<String> {
    nodes
        .into_iter()
        .map(|node| {
            if let Some(text) = node.chars() {
                format!("chars:{text}")
            } else if node.callable_type() == Some(FlmCallableType::Placeholder) {
                format!("placeholder:{}", node.name().unwrap_or_default())
            } else if node.is_group() {
                String::from("group")
            } else if node.comment().is_some() {
                String::from("comment")
            } else {
                format!("callable:{}", node.name().unwrap_or_default())
            }
        })
        .collect()
}

/// The shape of the content of the one argument of the first node named
/// `name` under `root`.
fn argument_shapes(root: NodeRef<'_, flm_core::lang::Flm>, name: &str) -> Vec<String> {
    let node = named(root, name);
    shapes(node.argument_content_nodes(0).expect("the argument").iter())
}

// --- a template parse keeps the placeholder alive ------------------------------------------

#[test]
fn a_placeholder_in_a_characters_only_group_is_a_node_of_the_template() {
    let setup = with_definitions(vec![
        Definition::macro_("a", vec![ArgDef::mandatory("x")]).text("\\lbl{#1}"),
        Definition::macro_("b", vec![ArgDef::mandatory("x")]).text("\\lbl{sec:#1}"),
        Definition::macro_("c", vec![ArgDef::mandatory("name")]).text("\\lbl{sec:#{name}.}"),
    ]);
    assert_eq!(argument_shapes(template_tree(&setup, "a").root(), "lbl"), ["placeholder:1"]);
    assert_eq!(
        argument_shapes(template_tree(&setup, "b").root(), "lbl"),
        ["chars:sec:", "placeholder:1"]
    );
    assert_eq!(
        argument_shapes(template_tree(&setup, "c").root(), "lbl"),
        ["chars:sec:", "placeholder:name", "chars:."]
    );
}

#[test]
fn a_placeholder_in_a_verbatim_argument_is_a_node_of_the_template() {
    let setup = with_definitions(vec![
        Definition::macro_("v", vec![ArgDef::mandatory("x")]).text("\\verb{<#1>}"),
    ]);
    assert_eq!(
        argument_shapes(template_tree(&setup, "v").root(), "verb"),
        ["chars:<", "placeholder:1", "chars:>"]
    );
}

#[test]
fn a_placeholder_in_a_nested_group_of_a_characters_only_group_is_a_node_too() {
    let setup = with_definitions(vec![
        Definition::macro_("n", vec![ArgDef::mandatory("x")]).text("\\lbl{a{#1}b}"),
    ]);
    let tree = template_tree(&setup, "n");
    assert_eq!(argument_shapes(tree.root(), "lbl"), ["chars:a", "group", "chars:b"]);
    let group = named(tree.root(), "lbl")
        .argument_content_nodes(0)
        .unwrap()
        .iter()
        .find(|node| node.is_group())
        .unwrap();
    assert_eq!(shapes(group.children().iter()), ["placeholder:1"]);
}

#[test]
fn placeholders_in_the_restricted_shapes_substitute_at_a_use_site() {
    let setup = with_definitions(vec![
        Definition::macro_("a", vec![ArgDef::mandatory("x")]).text("\\lbl{sec:#1}"),
        Definition::macro_("n", vec![ArgDef::mandatory("x")]).text("\\lbl{a{#1}b}"),
        Definition::macro_("v", vec![ArgDef::mandatory("x")]).text("\\verb{<#1>}"),
    ]);
    let (tree, diagnostics, _) = expanded(&setup, "\\a{intro} \\n{x} \\v{y}", 8);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(recomposed(&tree), "\\lbl{sec:intro} \\lbl{a{x}b} \\verb{<y>}");
}

#[test]
fn a_double_escape_is_a_literal_hash_in_both_shapes() {
    let setup = with_definitions(vec![
        Definition::macro_("h", Vec::new()).text("\\lbl{a##b}\\verb{c##d}"),
    ]);
    let tree = template_tree(&setup, "h");
    assert_eq!(argument_shapes(tree.root(), "lbl"), ["chars:a", "chars:#", "chars:b"]);
    assert_eq!(argument_shapes(tree.root(), "verb"), ["chars:c", "chars:#", "chars:d"]);
    let (tree, diagnostics, _) = expanded(&setup, "\\h", 8);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(recomposed(&tree), "\\lbl{a#b}\\verb{c#d}");
}

// --- a document parse reads `#` as a character ---------------------------------------------

#[test]
fn in_a_document_a_hash_is_an_ordinary_character_in_both_shapes() {
    let lang = lang(vec![chars_macro(), verbatim_macro()]);
    let fragment = parse(&lang, "\\lbl{a#1b} \\verb{c#1d}");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let root = fragment.tree().root();
    assert_eq!(argument_shapes(root, "lbl"), ["chars:a#1b"]);
    assert_eq!(argument_shapes(root, "verb"), ["chars:c#1d"]);
}

// --- the one difference between the two shapes, under both parses -------------------------

/// Pins the difference in `root`: `\lbl{a%%c⏎b{x}y}` holds a comment and a
/// nested group; `\verb{a%%c⏎b{x}y}` holds the characters as written.
fn assert_the_difference(root: NodeRef<'_, flm_core::lang::Flm>) {
    assert_eq!(
        argument_shapes(root, "lbl"),
        ["chars:a", "comment", "chars:b", "group", "chars:y"]
    );
    assert_eq!(argument_shapes(root, "verb"), ["chars:a%%c\nb{x}y"]);
}

#[test]
fn comments_and_nested_groups_are_recognized_in_a_characters_only_group_alone() {
    let text = "\\lbl{a%%c\nb{x}y}\\verb{a%%c\nb{x}y}";
    let lang = lang(vec![chars_macro(), verbatim_macro()]);
    let fragment = parse(&lang, text);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_the_difference(fragment.tree().root());
    let setup = with_definitions(vec![Definition::macro_("d", Vec::new()).text(text)]);
    assert_the_difference(template_tree(&setup, "d").root());
}

#[test]
fn a_nested_group_of_a_characters_only_group_reads_commands_again() {
    let lang = lang(vec![chars_macro(), common::macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, "\\lbl{a\\x{b \\emph{c}}}");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let root = fragment.tree().root();
    assert_eq!(argument_shapes(root, "lbl"), ["chars:a\\x", "group"]);
    assert_eq!(named(root, "emph").name(), Some("emph"));
    let lbl = named(root, "lbl");
    let first = lbl.argument_content_nodes(0).unwrap().iter().next().unwrap();
    assert_eq!(first.parent().unwrap().group_type(), Some(FlmGroupType::Content));
}

// --- hard inputs ---------------------------------------------------------------------------

#[test]
fn hard_inputs_neither_panic_nor_loop() {
    let lang = lang(vec![chars_macro(), verbatim_macro()]);
    for text in ["\\lbl{}", "\\lbl", "\\lbl{", "\\lbl{#}", "\\verb{#", "\\lbl{{{{}}}}"] {
        let _ = parse(&lang, text);
    }
    // A characters-only argument holding only a placeholder, an empty one, and
    // `#` alone at the end of an argument (refused at build: the escape takes
    // the closing brace as its name).
    let setup = setup(
        vec![
            Definition::macro_("only", vec![ArgDef::mandatory("x")]).text("\\lbl{#1}"),
            Definition::macro_("empty", Vec::new()).text("\\lbl{}\\verb{}"),
            Definition::macro_("lone", Vec::new()).text("\\lbl{a#}"),
            Definition::macro_("lonev", Vec::new()).text("\\verb{a#}"),
        ],
        vec![chars_macro(), verbatim_macro()],
    );
    assert!(setup.definition("only").compiled().is_some());
    assert!(setup.definition("empty").compiled().is_some());
    assert!(setup.definition("lone").compiled().is_none());
    assert!(setup.definition("lonev").compiled().is_none());
    let (tree, _, _) = expanded(&setup, "\\only{} \\empty", 8);
    assert_eq!(recomposed(&tree), "\\lbl{} \\lbl{}\\verb{}");
}

// --- end to end: the text and HTML backends ------------------------------------------------

/// The standard package with `definitions`.
fn registry(definitions: Vec<Definition>) -> std::sync::Arc<flm_core::core::registry::FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)))
}

/// The text output of `document`, whose run reports nothing.
fn text_output(
    registry: &std::sync::Arc<flm_core::core::registry::FlmRegistry>,
    document: &str,
) -> String {
    let run = common::stdcore::run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none());
    assert!(common::stdcore::identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    common::lowering::text(&run)
}

/// The HTML fragment of `document`, whose run reports nothing.
fn html_output(
    registry: &std::sync::Arc<flm_core::core::registry::FlmRegistry>,
    document: &str,
) -> String {
    let run = common::stdcore::run_workflow(registry, html::WORKFLOW, &[document], &Overrides::none());
    assert!(common::stdcore::identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

#[test]
fn a_placeholder_in_a_label_a_reference_and_verbatim_substitutes_on_text_and_html() {
    let registry = registry(vec![
        Definition::macro_("sec", vec![ArgDef::mandatory("key"), ArgDef::mandatory("title")])
            .text("\\section{#2}\\label{sec:#1}"),
        Definition::macro_("myref", vec![ArgDef::mandatory("key")]).text("(see \\ref{#1})"),
        Definition::macro_("mylink", vec![ArgDef::mandatory("key")])
            .text("\\hyperref[#1]{that section}"),
        Definition::macro_("tag", vec![ArgDef::mandatory("name")]).text("\\verbcode{<#1>}"),
    ]);
    let with_definitions = "\\sec{intro}{Intro}\n\n\\myref{sec:intro}, \\mylink{sec:intro}, \\tag{b}.";
    let written_out = "\\section{Intro}\\label{sec:intro}\n\n(see \\ref{sec:intro}), \
                       \\hyperref[sec:intro]{that section}, \\verbcode{<b>}.";
    let text = text_output(&registry, with_definitions);
    assert_eq!(text, text_output(&registry, written_out));
    assert!(text.contains("<b>"), "{text}");
    assert_eq!(html_output(&registry, with_definitions), html_output(&registry, written_out));
}

#[test]
fn a_literal_hash_in_a_template_label_reaches_the_output() {
    let registry = registry(vec![
        Definition::macro_("code", Vec::new()).text("\\verbcode{a##b}"),
    ]);
    let text = text_output(&registry, "\\code{}");
    assert!(text.contains("a#b"), "{text}");
}

// --- the reader by segments ----------------------------------------------------------------

use flm_core::lang::text::{chars_text, read_chars, CharsSegment};
use flm_core::lang::rewrites::userdefs::UserDefinition;
use flm_core::model::aspects::Label;
use flm_core::stdcore::features::math::eqref::eqref_target;
use flm_core::stdcore::features::refs::{
    encode_anchor, field_text, hyperref_target, label_use, ref_targets,
};

/// The segments of `content` as words: `text:<characters>` or
/// `placeholder:<reference>`.
fn segment_words(content: &[CharsSegment<'_>]) -> Vec<String> {
    content
        .iter()
        .map(|segment| match segment {
            CharsSegment::Text(text) => format!("text:{text}"),
            CharsSegment::Placeholder(node) => {
                format!("placeholder:{}", node.name().unwrap_or_default())
            }
        })
        .collect()
}

/// The segments of the one argument of the first node named `name` under
/// `root`, or the reader's refusal.
fn read_argument(root: NodeRef<'_, flm_core::lang::Flm>, name: &str) -> Result<Vec<String>, String> {
    let node = named(root, name);
    let content = node.argument_content_nodes(0).expect("the argument");
    read_chars(content.iter()).map(|read| segment_words(read.segments())).map_err(|e| e.to_string())
}

#[test]
fn the_reader_reads_plain_characters_as_one_segment() {
    let lang = lang(vec![chars_macro(), verbatim_macro()]);
    let fragment = parse(&lang, "\\lbl{sec:intro} \\verb{a{b}c} \\lbl{}");
    let root = fragment.tree().root();
    assert_eq!(read_argument(root, "lbl").unwrap(), ["text:sec:intro"]);
    assert_eq!(read_argument(root, "verb").unwrap(), ["text:a{b}c"]);
    let empty = root.descendants().filter(|node| node.name() == Some("lbl")).nth(1).unwrap();
    let content = empty.argument_content_nodes(0).unwrap();
    let read = read_chars(content.iter()).unwrap();
    assert!(read.segments().is_empty());
    assert_eq!(read.text().as_deref(), Some(""));
}

#[test]
fn the_reader_joins_a_nested_group_and_skips_a_comment() {
    let lang = lang(vec![chars_macro()]);
    let fragment = parse(&lang, "\\lbl{a{b}c%%x\nd}");
    assert_eq!(read_argument(fragment.tree().root(), "lbl").unwrap(), ["text:abcd"]);
}

#[test]
fn the_reader_yields_a_placeholder_as_a_segment_of_its_own() {
    let setup = with_definitions(vec![
        Definition::macro_("b", vec![ArgDef::mandatory("x")]).text("\\lbl{sec:#1.}"),
        Definition::macro_("n", vec![ArgDef::mandatory("x")]).text("\\lbl{a{#1}b}"),
        Definition::macro_("v", vec![ArgDef::mandatory("x")]).text("\\verb{<#1>}"),
    ]);
    assert_eq!(
        read_argument(template_tree(&setup, "b").root(), "lbl").unwrap(),
        ["text:sec:", "placeholder:1", "text:."]
    );
    assert_eq!(
        read_argument(template_tree(&setup, "n").root(), "lbl").unwrap(),
        ["text:a", "placeholder:1", "text:b"]
    );
    assert_eq!(
        read_argument(template_tree(&setup, "v").root(), "verb").unwrap(),
        ["text:<", "placeholder:1", "text:>"]
    );
    // The convenience of a caller that has no use for a placeholder: no text.
    let tree = template_tree(&setup, "b");
    let content = named(tree.root(), "lbl").argument_content_nodes(0).unwrap();
    let read = read_chars(content.iter()).unwrap();
    assert!(read.has_placeholder());
    assert_eq!(read.text(), None);
    assert_eq!(chars_text(content.iter()), None);
}

#[test]
fn the_reader_refuses_content_holding_a_command() {
    let lang = lang(vec![common::macro_def("t", "emph", 1), common::macro_def("t", "box", 1)]);
    let fragment = parse(&lang, "\\box{a\\emph{b}c}");
    let root = fragment.tree().root();
    let refused = read_argument(root, "box").unwrap_err();
    assert!(refused.contains("command"), "{refused}");
    let content = named(root, "box").argument_content_nodes(0).unwrap();
    assert_eq!(chars_text(content.iter()), None);
}

#[test]
fn the_reader_refuses_content_holding_mathematics() {
    // A formula has no characters: `\(a\)` is refused, as `\(\alpha\)` and a
    // command are, rather than read through as a group giving `a`.
    let lang = lang(vec![common::macro_def("t", "box", 1)]);
    let fragment = parse(&lang, "\\box{x\\(a\\)y}");
    let root = fragment.tree().root();
    let refused = read_argument(root, "box").unwrap_err();
    assert!(refused.contains("mathematics"), "{refused}");
    let content = named(root, "box").argument_content_nodes(0).unwrap();
    assert_eq!(chars_text(content.iter()), None);
}

// --- the label and target readers over the segments ----------------------------------------

/// The standard package's compiled text-mode template of the definition
/// `name`, among `definitions`, with the registry that holds it.
fn std_template(
    definitions: Vec<Definition>,
    name: &str,
) -> (std::sync::Arc<flm_core::core::registry::FlmRegistry>, std::sync::Arc<NodeTree<flm_core::lang::Flm>>) {
    let registry = registry(definitions);
    let tree = registry
        .elements()
        .iter()
        .filter_map(|entry| entry.element.value_as::<UserDefinition>())
        .find(|value| value.definition().name == name)
        .and_then(|value| value.compiled())
        .and_then(|compiled| compiled.variant(Mode::Text))
        .map(|variant| std::sync::Arc::clone(variant.template().tree()))
        .unwrap_or_else(|| panic!("no compiled definition {name:?}"));
    (registry, tree)
}

#[test]
fn a_label_holding_a_placeholder_is_no_label() {
    let (_registry, tree) = std_template(
        vec![Definition::macro_("s", vec![ArgDef::mandatory("key")]).text("\\section{T}\\label{sec:#1}")],
        "s",
    );
    let field = tree.root().descendants().find(|node| node.name() == Some("label")).unwrap();
    assert_eq!(field_text(&field), None);
    assert!(label_use(&field).is_none());
    // The same reader over characters.
    let (_registry, tree) = std_template(
        vec![Definition::macro_("f", Vec::new()).text("\\section{T}\\label{sec:fixed}")],
        "f",
    );
    let field = tree.root().descendants().find(|node| node.name() == Some("label")).unwrap();
    assert_eq!(field_text(&field).as_deref(), Some("sec:fixed"));
    assert_eq!(label_use(&field).unwrap().label, Some(Label::new("sec", "fixed")));
}

#[test]
fn a_reference_target_holding_a_placeholder_names_nothing() {
    let (_registry, tree) = std_template(
        vec![Definition::macro_("r", vec![ArgDef::mandatory("key")])
            .text("\\ref{#1} \\hyperref[#1]{t} \\(\\eqref{#1}\\) \\ref{sec:a} \\hyperref[sec:a]{t}")],
        "r",
    );
    let root = tree.root();
    let refs: Vec<_> = root.descendants().filter(|node| node.name() == Some("ref")).collect();
    let hyperrefs: Vec<_> = root.descendants().filter(|node| node.name() == Some("hyperref")).collect();
    let eqref = root.descendants().find(|node| node.name() == Some("eqref")).unwrap();
    assert!(ref_targets(&refs[0]).is_none());
    assert!(hyperref_target(&hyperrefs[0]).is_none());
    assert!(eqref_target(&eqref).is_none());
    let fixed = ref_targets(&refs[1]).unwrap();
    assert_eq!(fixed[0].label, Some(Label::new("sec", "a")));
    assert_eq!(hyperref_target(&hyperrefs[1]).unwrap().label, Some(Label::new("sec", "a")));
}

// --- a reference's key is apart from the text around it ------------------------------------

/// The identifiers of the page's first-level headings, in order.
fn heading_ids(html: &str) -> Vec<String> {
    html.match_indices("<h1 ")
        .filter_map(|(at, _)| {
            let tag = &html[at..at + html[at..].find('>').expect("a closed tag")];
            tag.split_once(" id=\"").map(|(_, rest)| rest[..rest.find('"').expect("a quote")].to_string())
        })
        .collect()
}

#[test]
fn a_heading_is_identified_from_its_words_without_a_reference_key() {
    // Each title's identifier is encoded from its words alone: the key a
    // `\ref`, `\hyperref` or `\eqref` names is no word of the title, and a
    // `\hyperref`'s own text is. The words "Compare with " end with the space
    // before the reference, and a text with a trailing space takes the
    // encoding's tagged path — a slug, `~` and a tag of the text — so the
    // identifier is the encoding of exactly those words, where it was the
    // encoding of "Compare with sec:a" before the key was set apart.
    let registry = registry(Vec::new());
    let html = html_output(
        &registry,
        "\\section{Target}\\label{sec:a}\n\n\
         \\begin{equation}x\\label{eq:a}\\end{equation}\n\n\
         \\section{Compare with \\ref{sec:a}}\n\n\
         \\section{See \\hyperref[sec:a]{the section}}\n\n\
         \\section{As in \\eqref{eq:a}}\n\nText.",
    );
    let title = |words: &str| format!("sec--{}", encode_anchor(words));
    assert_eq!(
        heading_ids(&html),
        [String::from("sec-a"), title("Compare with "), title("See the section"), title("As in ")],
        "{html}"
    );
    assert!(title("Compare with ").starts_with("sec--Compare_with~"));
    assert_eq!(title("See the section"), "sec--See_the_section");
}

#[test]
fn a_heading_is_identified_without_the_options_of_a_reference() {
    // A `\ref`'s display options are instructions to the reference, no words
    // of the title, as its key is not: the title below is identified from
    // "See " alone, where it was `sec--See_noprefix` with the options read as
    // words.
    let registry = registry(Vec::new());
    let html = html_output(
        &registry,
        "\\section{Target}\\label{sec:a}\n\n\\section{See \\ref[noprefix]{sec:a}}\n\nText.",
    );
    assert_eq!(
        heading_ids(&html),
        [String::from("sec-a"), format!("sec--{}", encode_anchor("See "))],
        "{html}"
    );
}
