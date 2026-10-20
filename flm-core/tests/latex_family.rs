//! The LaTeX family (`flm_core::stdcore::backends::latex`) as the standard
//! package registers it: the four construct elements and the two requirement
//! slots with their standard contributions; the escaping under both Unicode
//! modes over the study's probe string, with an override and an unknown
//! character; the record of declared characters; the payload round trips
//! through the serialization; the sink's discipline; the style file of a
//! small requirement set as a golden.

mod common;

use std::sync::Arc;

use flm_core::core::registry::FlmRegistry;
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::session::PayloadReaders;
use flm_core::lang::{FlmNodeExt, PresPayload};
use flm_core::model::names::Name;
use flm_core::model::options::TypeKey;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::construct::{self, ALL, arg_slots, payload_readers};
use flm_core::stdcore::backends::latex::escape::{
    Declared, EscapeConfig, UnicodeMode, escape_into, spelling_for,
};
use flm_core::stdcore::backends::latex::packages::{
    self, LatexPackageDef, chunk_package_id, merge_requests, packages_named,
};
use flm_core::stdcore::backends::latex::preamble::{self, LatexPreambleDef, piece_named};
use flm_core::stdcore::backends::latex::sink::{LatexRaw, LatexSink, comment_percent};
use flm_core::stdcore::backends::latex_rendered;
use flm_core::stdcore::backends::latex::style::{
    ExtraPreamble, RecomposedDefinition, assemble_style, content_hash, style_body,
};
use flm_core::stdcore::backends::latex::{
    FEATURE_NAME, LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload,
    LatexEnvironmentPayload, LatexMacroPayload, LatexRawPayload,
};
use flm_core::stdcore::backends::pres;
use flm_core::lang::FlmGroupType;
use techy::core::node::NodeRef;

use common::dump::round_trip_value;
use common::latex::golden_path;
use common::stdcore::registry;

/// The Python study's probe string.
const PROBE: &str = "Café — naïve “quotes” ½ ≤ α";

fn package() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

// --- registration --------------------------------------------------------------------------

#[test]
fn the_standard_package_registers_the_four_construct_elements_under_latex() {
    let registry = package();
    assert!(registry.has_feature(FEATURE_NAME));
    let expected: [(&str, TypeKey); 4] = [
        (construct::MACRO, TypeKey::of::<LatexMacroPayload>()),
        (construct::ENVIRONMENT, TypeKey::of::<LatexEnvironmentPayload>()),
        (construct::RAW, TypeKey::of::<LatexRawPayload>()),
        (construct::DEFINITION, TypeKey::of::<LatexDefinitionPayload>()),
    ];
    for (name, payload) in expected {
        let entry = registry.presentation_element(name).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(entry.owner.as_str(), FEATURE_NAME, "{name}");
        assert_eq!(entry.payload, payload, "{name}");
        assert!(!registry.elements_named(name).is_empty(), "{name} is not an element");
        assert!(!pres::ALL.contains(&name), "{name} is the family's, not the shared vocabulary's");
    }
    assert_eq!(ALL.len(), 4);
}

#[test]
fn the_standard_package_declares_both_slots_with_the_standard_contributions() {
    let registry = package();
    let services = registry.services();
    for slot in [packages::SLOT, preamble::SLOT] {
        let decl = services.slot(slot).unwrap_or_else(|| panic!("{slot} is not declared"));
        assert_eq!(decl.owner.as_str(), FEATURE_NAME);
    }
    let roster: Vec<&str> = packages::packages(services).map(|d| d.id.as_str()).collect();
    // The family's requests, then the one the `latex-rendered` backend adds
    // (`fontspec`), since that feature stands after the family in the package.
    let mut expected: Vec<String> =
        packages::standard_packages().iter().map(|d| d.id.as_str().to_string()).collect();
    expected.push(latex_rendered::FONTSPEC.to_string());
    assert_eq!(roster, expected);
    // The features' definition pieces of the `latex-rendered` backend
    // (`flm.latex.def.flmx.<feature>`) stand among the features' others; they
    // are counted apart.
    let rendered_chunks: Vec<String> =
        latex_rendered::chunks().iter().map(|d| d.id.as_str().to_string()).collect();
    let flmx_pieces: Vec<&str> = preamble::pieces(services)
        .map(|d| d.id.as_str())
        .filter(|id| id.starts_with("flm.latex.def.flmx.") && !rendered_chunks.iter().any(|c| c == id))
        .collect();
    assert_eq!(
        flmx_pieces,
        [
            "flm.latex.def.flmx.headings.number",
            "flm.latex.def.flmx.xnotes",
            "flm.latex.def.flmx.defterm",
            "flm.latex.def.flmx.theorems",
            "flm.latex.def.flmx.quote",
            "flm.latex.def.flmx.floats",
            "flm.latex.def.flmx.cells.styles",
            "flm.latex.def.flmx.citations",
            "flm.latex.def.flmx.meta",
            "flm.latex.def.flmx.toc",
        ]
    );
    let pieces: Vec<&str> =
        preamble::pieces(services).map(|d| d.id.as_str()).filter(|id| !flmx_pieces.contains(id)).collect();
    // The `xnotes`, `href`, `defterm`, `theorems`, `quote`, `graphics`,
    // `floats`, `cells` and `citations` features stand before the family in the package and contribute their
    // pieces first: the definition of the one kind of xnote, `footnote`, then
    // the three hyperlink hooks, then the defined terms' hooks and requests,
    // then `\noproofref` and the proof title hook of each of the two forms of
    // the theorems, then the hooks of a quotation's sections, then the image
    // hook, then the float's hook environment, then the table hook, then the
    // citation hooks.
    assert_eq!(
        pieces[..11],
        [
            "flm.latex.def.xnotes.footnote",
            "flm.latex.def.href",
            "flm.latex.def.defterm",
            "flm.latex.def.theorems",
            "flm.latex.def.theorems.authored",
            "flm.latex.def.theorems.resolved",
            "flm.latex.def.quote",
            "flm.latex.def.graphics",
            "flm.latex.def.floats",
            "flm.latex.def.cells",
            "flm.latex.def.citations"
        ]
    );
    // After the features' pieces come the family's own, then the chunks of
    // the `latex-rendered` backend, which stands after the family.
    let rendered: Vec<String> =
        latex_rendered::chunks().iter().map(|d| d.id.as_str().to_string()).collect();
    assert_eq!(&pieces[pieces.len() - rendered.len()..], rendered);
    let pieces = &pieces[11..pieces.len() - rendered.len()];
    let expected_pieces: Vec<String> =
        preamble::standard_pieces().iter().map(|d| d.id.as_str().to_string()).collect();
    assert_eq!(pieces, expected_pieces);
    // The family's own eleven come first, then one piece per snippet chunk of
    // the Unicode-to-LaTeX table.
    assert_eq!(&pieces[..11], [
        preamble::REFS,
        preamble::HYPERREF,
        preamble::LABEL_DISPLAY_TEXT,
        preamble::ZREF_CLEVER_TYPE,
        preamble::LABELS,
        preamble::HEADING,
        preamble::VERBATIM,
        preamble::MATH_ENVIRONMENT,
        preamble::IF_BLANK,
        preamble::USER_ARGS,
        preamble::TAB_WIDTH,
    ]);
    assert_eq!(
        &pieces[11..],
        preamble::chunk_pieces().iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
    let zref = packages_named(services, packages::ZREF_CLEVER).next().expect("zref-clever");
    assert_eq!(zref.package, "zref-clever");
    assert!(packages_named(services, "flm.latex.pkg.nothing").next().is_none());
    let verbatim = piece_named(services, preamble::VERBATIM).expect("the verbatim piece");
    assert_eq!(verbatim.requires, [Name::from(packages::FVEXTRA)]);
    assert!(piece_named(services, "flm.latex.def.nothing").is_none());
}

#[test]
fn every_piece_requires_only_identifiers_the_registry_knows() {
    let registry = package();
    let services = registry.services();
    for piece in preamble::pieces(services) {
        for id in &piece.requires {
            // A piece names packages and pieces alike, told apart by their
            // prefix as a lowering rule's own requirements are.
            let known = if id.as_str().starts_with(preamble::ID_PREFIX) {
                piece_named(services, id.as_str()).is_some()
            } else {
                packages_named(services, id.as_str()).next().is_some()
            };
            assert!(known, "{} requires the unknown identifier {id}", piece.id);
        }
    }
}

// --- one owner per registered token ---------------------------------------------------

/// The text of a fragment with its comments removed (from an unescaped
/// `%` to the end of the line) and with no white space left.
fn stripped(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let mut escaped = false;
        for c in line.chars() {
            if c == '%' && !escaped {
                break;
            }
            escaped = c == '\\' && !escaped;
            if !c.is_whitespace() {
                out.push(c);
            }
        }
    }
    out
}

/// The braced group starting at `text[at]` (an opening brace): its
/// content, and the index after its closing brace.
fn group(text: &str, at: usize) -> Option<(&str, usize)> {
    if !text[at..].starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, c) in text[at..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[at + 1..at + offset], at + offset + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Every registration `\flmxDefine<Kind>{<token>}` of `text` (comments
/// stripped), as `<Kind>` and `<token>`; a registration over `#1` inside
/// `\clist_map_inline:nn{<tokens>}{...}` counts once per token of the
/// list. The `...Default` commands register no token and are left out,
/// as are the commands' own definitions.
fn registrations(text: &str) -> Vec<(String, String)> {
    const DEFINE: &str = "\\flmxDefine";
    const MAP: &str = "\\clist_map_inline:nn";
    let text = stripped(text);
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(DEFINE).map(|at| at + from) {
        let name_start = at + DEFINE.len();
        let name_end = text[name_start..]
            .find(|c: char| !c.is_ascii_alphabetic())
            .map_or(text.len(), |end| end + name_start);
        from = name_end;
        let kind = &text[name_start..name_end];
        if kind.is_empty() || kind.ends_with("Default") || text[..at].ends_with("\\NewDocumentCommand") {
            continue;
        }
        let Some((token, _)) = group(&text, name_end) else { continue };
        if !token.starts_with('#') {
            found.push((kind.to_string(), token.to_string()));
            continue;
        }
        // A registration over the item of a list map: the list's tokens.
        let Some(map) = text[..at].rfind(MAP) else { continue };
        let Some((list, after)) = group(&text, map + MAP.len()) else { continue };
        if group(&text, after).is_some_and(|(body, _)| body.starts_with(&text[at..name_end])) {
            for item in list.split(',').filter(|item| !item.is_empty()) {
                found.push((kind.to_string(), item.to_string()));
            }
        }
    }
    found
}

#[test]
fn the_scan_finds_literal_and_mapped_registrations() {
    let text = "% \\flmxDefineSpan{commented}{}\n\\NewDocumentCommand \\flmxDefineLink { m +m }\n\
                \\flmxDefineSpan { a.b }%\n  {#1}\n\\flmxDefineSpanDefault{#1}\n\
                \\clist_map_inline:nn { x, y-z }\n  { \\flmxDefineLink {#1} { [##1] } }\n";
    let found = registrations(text);
    let pairs: Vec<(&str, &str)> = found.iter().map(|(kind, token)| (kind.as_str(), token.as_str())).collect();
    assert_eq!(pairs, [("Span", "a.b"), ("Link", "x"), ("Link", "y-z")]);
}

/// One owner per token: across the `latex-rendered` backend's chunks and
/// every feature's definition piece the standard package registers (every
/// `flm.latex.def.flmx.*` piece of the preamble slot), no
/// `\flmxDefine<Kind>{<token>}` is registered twice, since the style file
/// writes the later registration over the earlier one.
#[test]
fn no_token_is_registered_twice_across_the_shipped_fragments() {
    let registry = package();
    let services = registry.services();
    let pieces: Vec<&LatexPreambleDef> = preamble::pieces(services)
        .filter(|piece| piece.id.as_str().starts_with("flm.latex.def.flmx."))
        .collect();
    assert!(pieces.len() > latex_rendered::chunks().len(), "{}", pieces.len());
    let mut seen: Vec<(String, String, String)> = Vec::new();
    for piece in &pieces {
        for (kind, token) in registrations(&piece.content) {
            if let Some((_, _, owner)) = seen.iter().find(|(k, t, _)| *k == kind && *t == token) {
                panic!("\\flmxDefine{kind}{{{token}}} is registered by {owner} and by {}", piece.id);
            }
            seen.push((kind, token, piece.id.to_string()));
        }
    }
    // The scan sees the links chunk's mapped kinds and the features' pieces'.
    for token in ["ref", "url", "term", "toc", "cite", "xnote", "xnote-back"] {
        assert!(seen.iter().any(|(kind, t, _)| kind == "Link" && t == token), "{token}: {seen:?}");
    }
}

// --- escaping ------------------------------------------------------------------------------

fn escaped(text: &str, config: &EscapeConfig) -> (String, Declared, Vec<char>) {
    let mut out = String::new();
    let mut declared = Declared::new();
    let unknown = escape_into(&mut out, text, config, &mut declared);
    (out, declared, unknown)
}

#[test]
fn the_specials_are_escaped_as_the_family_documents() {
    let (out, _, _) = escaped("$ & # ^ _ % { } \\ ~", &EscapeConfig::default());
    assert_eq!(
        out,
        "\\$ \\& \\# \\textasciicircum{} \\_ \\% \\{ \\} \\textbackslash{} \\textasciitilde{}"
    );
}

#[test]
fn replace_mode_spells_the_probe_string_in_place_and_declares_nothing() {
    let (out, declared, unknown) = escaped(PROBE, &EscapeConfig::new(UnicodeMode::Replace));
    assert_eq!(
        out,
        "Caf\\'e {\\textemdash} na\\\"ive {\\textquotedblleft}quotes{\\textquotedblright} \
         {\\textonehalf} \\ensuremath{\\leq} \\ensuremath{\\alpha}"
    );
    assert!(unknown.is_empty());
    assert!(declared.chars().is_empty());
    // Every spelling of the probe string is one the LaTeX kernel prints.
    assert!(declared.needs().is_empty());
    assert!(!declared.packages().contains(&Name::from(packages::NEWUNICODECHAR)));
}

#[test]
fn declare_mode_keeps_the_probe_string_and_records_every_character_once() {
    let config = EscapeConfig::new(UnicodeMode::Declare);
    let (out, declared, unknown) = escaped(&format!("{PROBE} {PROBE}"), &config);
    assert_eq!(out, format!("{PROBE} {PROBE}"));
    assert!(unknown.is_empty());
    let chars: Vec<char> = declared.chars().keys().copied().collect();
    assert_eq!(chars, ['½', 'é', 'ï', 'α', '—', '“', '”', '≤']);
    assert_eq!(declared.chars()[&'α'], "\\ensuremath{\\alpha}");
    let ids = declared.packages();
    let packages: Vec<&str> = ids.iter().map(Name::as_str).collect();
    assert_eq!(packages, [packages::NEWUNICODECHAR]);
}

#[test]
fn an_override_replaces_the_spelling_in_both_modes() {
    let replace = EscapeConfig::new(UnicodeMode::Replace).with_override('α', "\\upalpha");
    let (out, _, _) = escaped("α", &replace);
    assert_eq!(out, "{\\upalpha}");
    let declare = EscapeConfig::new(UnicodeMode::Declare).with_override('α', "\\upalpha");
    let (out, declared, _) = escaped("α", &declare);
    assert_eq!(out, "α");
    assert_eq!(declared.chars()[&'α'], "\\upalpha");
}

#[test]
fn the_table_s_corrections_stand_and_a_user_override_wins_over_them() {
    assert_eq!(spelling_for('𝟙', &EscapeConfig::default()).unwrap().0, "\\ensuremath{\\mathds{1}}");
    assert_eq!(spelling_for('⅞', &EscapeConfig::default()).unwrap().0, "\\nicefrac{7}{8}");
    let user = EscapeConfig::default().with_override('𝟙', "\\mathbbm{1}");
    let (spelling, needs) = spelling_for('𝟙', &user).unwrap();
    assert_eq!(spelling, "\\mathbbm{1}");
    // An override needs nothing of the table's: its packages are the user's.
    assert_eq!(needs, None);
    let (_, declared, _) = escaped("𝟙 ⅞", &EscapeConfig::new(UnicodeMode::Replace));
    assert_eq!(declared.needs().chunks().map(|c| c.id()).collect::<Vec<_>>(), [
        "dsfont", "nicefrac"
    ]);
    assert!(declared.packages().contains(&chunk_package_id("dsfont")));
    assert!(declared.packages().contains(&chunk_package_id("nicefrac")));
}

#[test]
fn an_unknown_character_is_reported_and_written_by_mode() {
    let (out, _, unknown) = escaped("ธ", &EscapeConfig::new(UnicodeMode::Declare));
    assert_eq!((out.as_str(), unknown.as_slice()), ("ธ", &['ธ'][..]));
    let (out, _, unknown) = escaped("ธ", &EscapeConfig::new(UnicodeMode::Replace));
    assert_eq!(out, "\\ensuremath{\\langle}\\texttt{U+0E18}\\ensuremath{\\rangle}");
    assert_eq!(unknown, ['ธ']);
}

// --- payloads through the serialization ---------------------------------------------------

fn round_trip<P: PresPayload + PartialEq + std::fmt::Debug>(element: &str, payload: P) {
    let registry = package();
    let lang = registry.lang();
    let spec = registry.presentation_element(element).expect("registered").element.clone();
    let projection = payload.project();
    let ext = FlmNodeExt::new(Arc::clone(&spec)).with_payload(Arc::new(payload));
    let (written, read) = round_trip_value(lang, vec![spec], payload_readers(), ext);
    assert_eq!(common::dump::field(&written, "payload"), Some(&projection));
    let read = read.expect("the extension reads back").payload.expect("with its payload");
    assert!(read.downcast_ref::<P>().is_some(), "{element}: the payload reads back as its type");
    assert_eq!(read.project(), projection);
}

#[test]
fn every_construct_payload_round_trips_through_its_registered_reader() {
    round_trip(
        construct::MACRO,
        LatexMacroPayload::new("section", [LatexArgShape::Star, LatexArgShape::Mandatory]),
    );
    round_trip(
        construct::MACRO,
        LatexMacroPayload::new("flmUDaround", [LatexArgShape::Mandatory; 3]),
    );
    round_trip(
        construct::DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDaround", 3)
            .with_source_name("around"),
    );
    round_trip(
        construct::ENVIRONMENT,
        LatexEnvironmentPayload::new("flmVerbatimCode", [LatexArgShape::Optional]).verbatim(),
    );
    round_trip(construct::RAW, LatexRawPayload::new("\n\n"));
    round_trip(construct::DEFINITION, LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "hello", 2));
    // A definition of more than nine arguments and the call of it: both carry
    // the packed flag, which says that the arguments are handed over in one
    // group, so the two agree about the shape after a round trip too.
    round_trip(
        construct::DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDbig", 12)
            .with_source_name("big")
            .packed(),
    );
    round_trip(
        construct::MACRO,
        LatexMacroPayload::new("flmUDbig", [LatexArgShape::Mandatory; 12]).packed(),
    );
    round_trip(
        construct::ENVIRONMENT,
        LatexEnvironmentPayload::new("flmUEpackednote", [LatexArgShape::Mandatory; 10]).packed(),
    );
    // An environment that takes its own body as its last argument, alone and
    // together with the packing: the flag is the definition's, and the call of
    // it carries none, since a call writes the body where it always did.
    round_trip(
        construct::DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEwrap", 1)
            .with_source_name("wrap")
            .body_argument(),
    );
    round_trip(
        construct::DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Environment, "flmUEpackedwrap", 10)
            .with_source_name("packedwrap")
            .packed()
            .body_argument(),
    );
    // A macro asked to be robust, alone and packed: the flag belongs to the
    // definition's head, and the call of such a definition carries none, being
    // written exactly as the call of an expandable one is.
    round_trip(
        construct::DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDhello", 1)
            .with_source_name("hello")
            .robust(),
    );
    round_trip(
        construct::DEFINITION,
        LatexDefinitionPayload::new(LatexDefinitionKind::Macro, "flmUDbig", 12)
            .with_source_name("big")
            .packed()
            .robust(),
    );
}

#[test]
fn the_payload_readers_cover_exactly_the_four_elements() {
    let readers: PayloadReaders = payload_readers();
    let mut elements: Vec<&str> = readers.elements().map(|e| e.as_str()).collect();
    elements.sort_unstable();
    let mut expected = ALL.to_vec();
    expected.sort_unstable();
    assert_eq!(elements, expected);
}

// --- the sink's discipline -----------------------------------------------------------------

/// The first node of the tree matching `pick`, in document order.
fn find<'t>(root: NodeRef<'t, flm_core::lang::Flm>, pick: impl Fn(&NodeRef<'t, flm_core::lang::Flm>) -> bool) -> NodeRef<'t, flm_core::lang::Flm> {
    root.descendants().find(|node| pick(node)).expect("the node is in the tree")
}

#[test]
fn raw_bytes_come_only_from_a_raw_payload_or_a_source_recomposition() {
    let registry = package();
    let fragment = common::parse(registry.lang(), "See \\(a_b & c\\) now.");
    let math = find(fragment.tree().root(), |node| {
        matches!(node.group_type(), Some(FlmGroupType::Math(_)))
    });
    let raw = LatexRaw::from_source(&[math]);
    assert_eq!(raw.as_str(), "\\(a_b & c\\)");
    let mut sink = LatexSink::new(EscapeConfig::default());
    sink.text("a_b & c");
    sink.raw(&raw).expect("source bytes are written as they stand");
    // An escaped per cent is the character, and a raw piece may hold it.
    sink.raw(&LatexRaw::from_raw_payload(&LatexRawPayload::new("\\%")))
        .expect("an escaped per cent starts no comment");
    // The text is escaped; the raw bytes are not.
    assert_eq!(sink.finish().text, "a\\_b \\& c\\(a_b & c\\)\\%");
}

#[test]
fn a_raw_piece_a_rule_assembled_may_not_start_a_comment() {
    // The one writer of a comment in a recomposed output is the sink's comment
    // method, which ends the comment's line itself; a `%` in a piece a rule
    // built is a mistake in that rule, and the sink refuses it instead of
    // writing a comment nothing would terminate.
    let mut sink = LatexSink::new(EscapeConfig::default());
    let refused = sink
        .raw(&LatexRaw::from_raw_payload(&LatexRawPayload::new("x %% note")))
        .expect_err("a bare per cent starts a comment");
    assert_eq!(refused.at, 2);
    assert_eq!(refused.text, "x %% note");
    // Nothing was written for the refused piece.
    assert_eq!(sink.finish().text, "");
    // The rule the check applies, stated once.
    assert_eq!(comment_percent("100\\% sure"), None);
    assert_eq!(comment_percent("\\\\% a line break, then a comment"), Some(2));
    assert_eq!(comment_percent("\\\\\\% a line break, then a per cent"), None);
}

#[test]
fn the_sink_hands_back_what_the_style_file_must_declare() {
    let mut sink = LatexSink::new(EscapeConfig::new(UnicodeMode::Declare));
    sink.control_word("emph");
    sink.open_brace();
    sink.text("naïve 𝟙 ธ");
    sink.close_brace();
    sink.escverb("x{y}");
    let output = sink.finish();
    assert_eq!(output.text, "\\emph{naïve 𝟙 ธ}x\\{y\\}");
    assert_eq!(output.unknown, ['ธ']);
    assert_eq!(output.declared.chars().len(), 2);
    assert!(output.declared.packages().contains(&chunk_package_id("dsfont")));
}

#[test]
fn the_argument_slots_of_a_hand_built_node_pair_with_their_shapes() {
    // A node with no slots at all: every shape pairs with `None`.
    let registry = package();
    let fragment = common::parse(registry.lang(), "plain");
    let root = fragment.tree().root();
    let shapes = [LatexArgShape::Star, LatexArgShape::Mandatory];
    let pairs = arg_slots(&root, &shapes);
    assert_eq!(pairs.len(), 2);
    assert!(pairs.iter().all(|(_, slot)| slot.is_none()));
    assert_eq!(pairs[0].0, LatexArgShape::Star);
}

// --- the style file ------------------------------------------------------------------------

#[test]
fn the_style_file_of_a_small_requirement_set_matches_its_golden() {
    let registry = package();
    let services = registry.services();
    let mut declared = Declared::new();
    escape_into(&mut String::new(), "café 𝟙", &EscapeConfig::new(UnicodeMode::Declare), &mut declared);
    let pieces: Vec<&LatexPreambleDef> = [preamble::VERBATIM, preamble::REFS]
        .iter()
        .map(|id| piece_named(services, id).expect("a standard piece"))
        .collect();
    // The caller resolves the requirement identifiers, the pieces' packages
    // and the declared characters' packages into one ordered list.
    let mut ids: Vec<Name> = vec![Name::from(packages::AMSMATH), Name::from(packages::ZREF_CLEVER)];
    ids.extend(
        pieces
            .iter()
            .flat_map(|piece| piece.requires.iter())
            .filter(|id| !id.as_str().starts_with(preamble::ID_PREFIX))
            .cloned(),
    );
    ids.extend(declared.packages().iter().cloned());
    for id in &ids {
        assert!(packages_named(services, id).next().is_some(), "{id} is a standard package");
    }
    let defs: Vec<&LatexPackageDef> =
        ids.iter().flat_map(|id| packages_named(services, id)).collect();
    // One `\RequirePackage` line per package, as the final stage merges them.
    let (packages, conflicts) = merge_requests(defs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    let definitions = vec![RecomposedDefinition::new(
        "\\flmUDhello",
        "\\hello",
        "\\providecommand\\flmUDhello[1]{Hello, #1!}",
    )];
    let body = style_body(&packages, &declared, &pieces, &definitions, &ExtraPreamble::default());
    let name = format!("flmrecomposed.{}", content_hash(&body));
    let style = assemble_style(&name, &packages, &declared, &pieces, &definitions, &ExtraPreamble::default());
    assert!(style.ends_with(&body));
    assert_text_golden(golden_path("family-style.sty"), &style);
}

#[test]
fn the_style_file_is_deterministic_and_named_for_its_content() {
    let empty = style_body(&[], &Declared::new(), &[], &[], &ExtraPreamble::default());
    assert_eq!(empty, "\\endinput\n");
    assert_eq!(content_hash(&empty), content_hash(&empty));
    let registry = package();
    let amsmath =
        packages_named(registry.services(), packages::AMSMATH).next().expect("amsmath");
    let once = merge_requests([amsmath]).0;
    let with_package = style_body(&once, &Declared::new(), &[], &[], &ExtraPreamble::default());
    assert_ne!(content_hash(&with_package), content_hash(&empty));
    let twice = merge_requests([amsmath, amsmath]).0;
    assert_eq!(with_package, style_body(&twice, &Declared::new(), &[], &[], &ExtraPreamble::default()));
    // The author's own pieces are part of the content the file is named for.
    let at_end = ExtraPreamble { at_end: String::from("\\relax"), ..ExtraPreamble::default() };
    let with_block = style_body(&[], &Declared::new(), &[], &[], &at_end);
    assert_eq!(
        with_block,
        "% --- the author's own preamble: at the end ---\n\\relax\n\\endinput\n"
    );
    assert_ne!(content_hash(&with_block), content_hash(&empty));
    // Two pieces holding the same text in different places are two files.
    let before = ExtraPreamble {
        before_packages: String::from("\\relax"),
        ..ExtraPreamble::default()
    };
    let with_before = style_body(&[], &Declared::new(), &[], &[], &before);
    assert_ne!(content_hash(&with_before), content_hash(&with_block));
}
