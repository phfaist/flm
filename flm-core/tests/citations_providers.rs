//! The `citations` feature's providers: the call handing a provider all the
//! keys of a document, the answer naming the work a key stands for, the works
//! numbered after the providers were asked, and the entry built from pieces.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FeatureChain, FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{FlmOrigin, FlmSource};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::citations::{
    CitationAnswer, CitationEntry, CitationPlacement, CitationProvider,
    CitationRequest, CitationResult, CitationsConfig, EntryPiece, ProviderCx, CITATIONS, PROVIDERS_SLOT,
};
use flm_core::stdcore::StdcoreConfig;

use common::citations::{registry_over_with, registry_with, FromMap};
use common::stdcore::{identifiers, package_features_but, run_workflow};

// --- providers of the tests' own ---------------------------------------------------------------

/// The keys of every call a [`Batch`] provider was handed, call by call.
#[derive(Debug, Clone, Default)]
struct Calls(Arc<Mutex<Vec<Vec<String>>>>);

impl Calls {
    /// The calls so far, each its keys in order.
    fn all(&self) -> Vec<Vec<String>> {
        self.0.lock().expect("not poisoned").clone()
    }
}

/// A provider written for all the keys: it records each call's keys, and
/// answers the keys of its map — each with the entry of its FLM text and, where
/// the map says so, an identity — and "not mine" for any other key.
struct Batch {
    calls: Calls,
    works: BTreeMap<String, (String, Option<String>)>,
}

impl Batch {
    /// The provider of `works`, triples of a key, its FLM text and its
    /// identity, recording into `calls`.
    fn of(calls: &Calls, works: &[(&str, &str, Option<&str>)]) -> Batch {
        let works = works
            .iter()
            .map(|(key, text, identity)| (key.to_string(), (text.to_string(), identity.map(str::to_string))))
            .collect();
        Batch { calls: calls.clone(), works }
    }
}

impl CitationProvider for Batch {
    fn answer(&self, _request: &CitationRequest, _cx: &ProviderCx<'_>) -> CitationResult {
        panic!("a provider written for all the keys is asked for all the keys")
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &ProviderCx<'_>) -> Vec<CitationResult> {
        self.calls.0.lock().expect("not poisoned").push(requests.iter().map(|r| r.key.clone()).collect());
        requests
            .iter()
            .map(|request| {
                let Some((text, identity)) = self.works.get(&request.key) else { return Ok(None) };
                let source = FlmSource::new(text.as_str()).with_origin(FlmOrigin::new("refs.flm"));
                let answer = CitationAnswer::new(cx.parse_entry(Arc::new(source))?);
                Ok(Some(match identity {
                    Some(identity) => answer.with_identity(identity.clone()),
                    None => answer,
                }))
            })
            .collect()
    }
}

/// A provider answering one result fewer than it was asked for.
struct Short;

impl CitationProvider for Short {
    fn answer(&self, _request: &CitationRequest, _cx: &ProviderCx<'_>) -> CitationResult {
        Ok(None)
    }

    fn answer_all(&self, requests: &[CitationRequest], _cx: &ProviderCx<'_>) -> Vec<CitationResult> {
        requests.iter().skip(1).map(|_| Ok(None)).collect()
    }
}

/// A provider answering the key `a` with the entry built from `pieces`.
struct Built(Vec<EntryPiece>);

impl CitationProvider for Built {
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
        if request.key != "a" {
            return Ok(None);
        }
        cx.build_entry(&self.0).map(|entry| Some(CitationAnswer::new(entry)))
    }
}

// --- helpers -----------------------------------------------------------------------------------

/// The standard package, the default configuration, with `providers`.
fn with(providers: Vec<Arc<dyn CitationProvider>>) -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new(), providers)
}

/// The run of `workflow` over the one document `document`.
fn run(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> Run {
    run_workflow(registry, workflow, &[document], &Overrides::none())
}

/// The text a text run wrote, its trailing whitespace trimmed.
fn text_of(run: &Run) -> String {
    common::lowering::text(run).trim_end().to_string()
}

/// What an output of `workflow` wrote for `document`, as text: the text
/// output's, the page, or the recomposed LaTeX.
fn output_of(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> String {
    let run = run(registry, workflow, document);
    assert!(identifiers(&run).is_empty(), "{workflow}: {}", run.diagnostics().render_all());
    match workflow {
        text::WORKFLOW => text_of(&run),
        html::WORKFLOW => common::html::page(&run),
        _ => common::latex::output(&run),
    }
}

// --- the call for all the keys -----------------------------------------------------------------

#[test]
fn a_provider_written_for_all_the_keys_receives_them_in_one_call_in_order_of_first_appearance() {
    let calls = Calls::default();
    let provider = Batch::of(&calls, &[("a", "Knuth", None), ("b", "Lamport", None), ("c", "Shor", None)]);
    let run = run(&with(vec![Arc::new(provider)]), text::WORKFLOW, "X \\cite{c,a}. Y \\cite{b}\\cite{a}. Z \\cite{c}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(calls.all(), [["c", "a", "b"]]);
    assert_eq!(text_of(&run), "X\u{a0}[1,2]. Y\u{a0}[2,3]. Z\u{a0}[1].\n\nReferences\n----------\n\n[1] Shor\n\n[2] Knuth\n\n[3] Lamport");
}

#[test]
fn the_keys_a_provider_answers_not_mine_reach_the_next_provider_in_one_call() {
    let (first, second) = (Calls::default(), Calls::default());
    let providers: Vec<Arc<dyn CitationProvider>> = vec![
        Arc::new(Batch::of(&first, &[("b", "Lamport", None)])),
        Arc::new(Batch::of(&second, &[("a", "Knuth", None), ("d", "Gottesman", None)])),
    ];
    let run = run(&with(providers), text::WORKFLOW, "X \\cite{a,b}. Y \\cite{c}\\cite{d}.");
    assert_eq!(first.all(), [["a", "b", "c", "d"]]);
    assert_eq!(second.all(), [["a", "c", "d"]]);
    // `c` is known to neither: it has no entry, and is numbered as any other.
    assert_eq!(identifiers(&run), ["flm.citations.no-entry"]);
}

/// The standard package, then a feature for each of `providers`, added in
/// this order, each contributing its provider, with the host's provider
/// `host` and the `chains` of the feature order.
fn with_features(
    providers: Vec<(&'static str, Arc<dyn CitationProvider>)>,
    host: Arc<dyn CitationProvider>,
    chains: &[&str],
) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    for (name, provider) in providers {
        builder.feature(common::pipeline::feature(name, move |b| {
            b.contribute_to(PROVIDERS_SLOT, Arc::clone(&provider))
        }));
    }
    builder.contribute_to(PROVIDERS_SLOT, host);
    for text in chains {
        builder.feature_order(FeatureChain::parse(text, "test").expect("a chain"));
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

#[test]
fn the_provider_of_the_feature_placed_last_is_asked_first_and_the_hosts_last() {
    let (first, second, host) = (Calls::default(), Calls::default(), Calls::default());
    let providers = |chains: &[&str]| {
        with_features(
            vec![
                ("t.first", Arc::new(Batch::of(&first, &[("a", "Knuth", None)])) as Arc<dyn CitationProvider>),
                ("t.second", Arc::new(Batch::of(&second, &[("a", "Lamport", None)]))),
            ],
            Arc::new(Batch::of(&host, &[("a", "Shor", None), ("b", "Gottesman", None)])),
            chains,
        )
    };
    let default = run(&providers(&[]), text::WORKFLOW, "X \\cite{a,b}.");
    assert_eq!(second.all(), [["a", "b"]]);
    assert_eq!(first.all(), [["b"]]);
    assert_eq!(host.all(), [["b"]]);
    assert_eq!(text_of(&default), "X\u{a0}[1,2].\n\nReferences\n----------\n\n[1] Lamport\n\n[2] Gottesman");
    // A chain placing `t.second` first lets `t.first` answer.
    let chained = run(&providers(&["t.second < t.first"]), text::WORKFLOW, "X \\cite{a}.");
    assert_eq!(text_of(&chained), "X\u{a0}[1].\n\nReferences\n----------\n\n[1] Knuth");
}

#[test]
fn a_provider_written_for_one_key_is_asked_key_by_key_for_the_keys_left_to_it() {
    let calls = Calls::default();
    let (one_by_one, asked) = {
        let asked = common::citations::Asked::default();
        let inner: Arc<dyn CitationProvider> = Arc::new(FromMap::of(&[("a", "Knuth"), ("c", "Shor")]));
        (Arc::new(common::citations::Counting { asked: asked.clone(), inner }), asked)
    };
    let providers: Vec<Arc<dyn CitationProvider>> =
        vec![Arc::new(Batch::of(&calls, &[("b", "Lamport", None)])), one_by_one];
    let run = run(&with(providers), text::WORKFLOW, "X \\cite{a,b,c}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(calls.all(), [["a", "b", "c"]]);
    assert_eq!(asked.keys(), ["a", "c"]);
}

#[test]
fn a_provider_answering_a_different_number_of_results_fails_every_key_of_its_call() {
    let run = run(&with(vec![Arc::new(Short), Arc::new(FromMap::of(&[("a", "Knuth")]))]), text::WORKFLOW, "X \\cite{a,b}.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.provider-failed", "flm.citations.provider-failed", "flm.citations.no-entry", "flm.citations.no-entry"],
        "{}",
        run.diagnostics().render_all()
    );
    assert!(run.diagnostics().render_all().contains(
            "a source of bibliography entries must return one result for each key it is given, and it returned 1 result for 2 keys"
        ), "{}", run.diagnostics().render_all());
}

// --- the entry built from pieces ---------------------------------------------------------------

/// The pieces of the entry the tests build: every kind of piece, characters
/// special to FLM among the text.
fn pieces() -> Vec<EntryPiece> {
    vec![
        EntryPiece::text("D. Knuth, "),
        EntryPiece::Emphasized(vec![EntryPiece::text("The Art of "), EntryPiece::Strong(vec![EntryPiece::text("Computer")])]),
        EntryPiece::text(" Programming, 100% {sure} #1 & $\\"),
        EntryPiece::Source(Arc::new(FlmSource::new("\\(x^2\\) and \\emph{y}").with_origin(FlmOrigin::new("record")))),
        EntryPiece::text(" "),
        EntryPiece::Link {
            address: "https://doi.org/10.1000/a%20b".to_string(),
            content: vec![EntryPiece::text("DOI")],
        },
    ]
}

/// The same entry, written in FLM.
const WRITTEN: &str = "D. Knuth, \\emph{The Art of \\textbf{Computer}} Programming, 100\\% \\{sure\\} \\#1 \\& \\$\
                       \\textbackslash{}\\(x^2\\) and \\emph{y} \\href{https://doi.org/10.1000/a%20b}{DOI}";

#[test]
fn an_entry_built_from_pieces_is_shown_on_every_output_as_the_same_content_written_in_flm() {
    let built = with(vec![Arc::new(Built(pieces()))]);
    let written = with(vec![Arc::new(FromMap::of(&[("a", WRITTEN)]))]);
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
        let document = "See \\cite{a}.";
        assert_eq!(output_of(&built, workflow, document), output_of(&written, workflow, document), "{workflow}");
    }
    assert!(output_of(&built, html::WORKFLOW, "See \\cite{a}.").contains("href=\"https://doi.org/10.1000/a%20b\""));
}

#[test]
fn without_the_feature_owning_links_a_link_shows_its_text() {
    let pieces = vec![
        EntryPiece::text("Knuth, "),
        EntryPiece::Link { address: "https://example.com".to_string(), content: vec![EntryPiece::text("the book")] },
    ];
    let registry = registry_over_with(package_features_but(StdcoreConfig::new(), "href"), vec![Arc::new(Built(pieces))]);
    let run = run(&registry, text::WORKFLOW, "See \\cite{a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "See\u{a0}[1].\n\nReferences\n----------\n\n[1] Knuth, the book");
}

#[test]
fn without_the_feature_owning_emphasis_an_emphasized_group_shows_its_content() {
    let pieces = vec![EntryPiece::text("Knuth, "), EntryPiece::Emphasized(vec![EntryPiece::text("TAOCP")])];
    let registry =
        registry_over_with(package_features_but(StdcoreConfig::new(), "textstyles"), vec![Arc::new(Built(pieces))]);
    let run = run(&registry, text::WORKFLOW, "See \\cite{a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "See\u{a0}[1].\n\nReferences\n----------\n\n[1] Knuth, TAOCP");
}

#[test]
fn a_source_piece_that_does_not_parse_is_the_providers_error() {
    let pieces = vec![EntryPiece::Source(Arc::new(FlmSource::new("\\emph{open").with_origin(FlmOrigin::new("record"))))];
    let run = run(&with(vec![Arc::new(Built(pieces))]), text::WORKFLOW, "See \\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.citations.provider-failed", "flm.citations.no-entry"]);
}

/// A provider building its entry by hand from a value tree, which an answer
/// wraps: the entry is the tree's content.
#[test]
fn an_answer_wraps_an_entry_made_by_hand() {
    struct ByHand;
    impl CitationProvider for ByHand {
        fn answer(&self, _request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
            let entry: CitationEntry = common::citations::text_entry(cx, "By hand")?;
            Ok(Some(CitationAnswer::new(entry)))
        }
    }
    let run = run(&with(vec![Arc::new(ByHand)]), text::WORKFLOW, "See \\cite{a}.");
    assert_eq!(text_of(&run), "See\u{a0}[1].\n\nReferences\n----------\n\n[1] By hand");
}

// --- the works numbered after the providers were asked -----------------------------------------

/// The works of a run: every key with its number, identifier and identity, in
/// order of first appearance.
fn works(run: &Run) -> Vec<(String, usize, String, String)> {
    let table = run.table(&CITATIONS).expect("declared").expect("written");
    table
        .keys()
        .map(|(key, work)| (key.to_string(), work.number, work.identifier.clone(), work.identity.clone()))
        .collect()
}

/// A provider naming `arxiv:1211.1037` for both spellings of the key, and
/// answering `b` under its own name.
fn arxiv(calls: &Calls) -> Batch {
    Batch::of(
        calls,
        &[
            ("arXiv:1211.1037", "Preprint", Some("arxiv:1211.1037")),
            ("arxiv:1211.1037", "Preprint", Some("arxiv:1211.1037")),
            ("b", "Lamport", None),
        ],
    )
}

#[test]
fn two_keys_answered_with_one_identity_are_one_work() {
    let registry = with(vec![Arc::new(arxiv(&Calls::default()))]);
    let document = "X \\cite{arXiv:1211.1037}. Y \\cite{b}. Z \\cite{arXiv:1211.1037, arxiv:1211.1037}\\cite{b}.";
    let run = run(&registry, text::WORKFLOW, document);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let identity = |s: &str| s.to_string();
    assert_eq!(
        works(&run),
        [
            (identity("arXiv:1211.1037"), 1, identity("citation-1"), identity("arxiv:1211.1037")),
            (identity("b"), 2, identity("citation-2"), identity("b")),
            (identity("arxiv:1211.1037"), 1, identity("citation-1"), identity("arxiv:1211.1037")),
        ]
    );
    // One number per work in a chunk, one entry in the list, the first key's.
    assert_eq!(
        text_of(&run),
        "X\u{a0}[1]. Y\u{a0}[2]. Z\u{a0}[1,2].\n\nReferences\n----------\n\n[1] Preprint\n\n[2] Lamport"
    );
    // On the page, one identifier, one entry carrying it.
    let page = common::html::page(&run_workflow(&registry, html::WORKFLOW, &[document], &Overrides::none()));
    assert_eq!(page.matches("id=\"citation-1\"").count(), 1, "{page}");
    assert_eq!(page.matches("href=\"#citation-1\"").count(), 2, "{page}");
    assert!(!page.contains("citation-3"), "{page}");
}

#[test]
fn a_mark_citing_one_work_under_two_keys_is_singular_after_r() {
    let registry = with(vec![Arc::new(arxiv(&Calls::default()))]);
    let run = run(&registry, text::WORKFLOW, "See \\R\\cite{arXiv:1211.1037,arxiv:1211.1037}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert!(text_of(&run).starts_with("See ref.\u{a0}[1]."), "{}", text_of(&run));
}

#[test]
fn in_place_every_item_is_shown_as_written() {
    let registry = registry_with(
        StdcoreConfig::new().citations(CitationsConfig::new().placement(CitationPlacement::InPlace)),
        vec![Arc::new(arxiv(&Calls::default()))],
    );
    let run = run(&registry, text::WORKFLOW, "Z \\cite{arXiv:1211.1037,arxiv:1211.1037}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "Z\u{a0}[Preprint][Preprint].");
}

#[test]
fn the_works_are_numbered_by_the_first_appearance_of_any_of_their_keys() {
    // `a` and `A` name one work; `x` has no answer and is its own work.
    let calls = Calls::default();
    let provider = Batch::of(&calls, &[("a", "Knuth", Some("knuth")), ("A", "Knuth", Some("knuth")), ("b", "Lamport", None)]);
    let run = run(&with(vec![Arc::new(provider)]), text::WORKFLOW, "\\cite{b} \\cite{x} \\cite{A} \\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.citations.no-entry"]);
    let numbers: Vec<(String, usize)> = works(&run).into_iter().map(|(key, number, _, _)| (key, number)).collect();
    assert_eq!(
        numbers,
        [("b".to_string(), 1), ("x".to_string(), 2), ("A".to_string(), 3), ("a".to_string(), 3)]
    );
    assert_eq!(works(&run)[1].3, "x", "a key with no answer names itself");
}

#[test]
fn two_providers_answering_one_identity_for_two_keys_make_one_work() {
    let providers: Vec<Arc<dyn CitationProvider>> = vec![
        Arc::new(Batch::of(&Calls::default(), &[("doi:10.1/X", "From the first", Some("doi:10.1/x"))])),
        Arc::new(Batch::of(&Calls::default(), &[("doi:10.1/x", "From the second", Some("doi:10.1/x"))])),
    ];
    let run = run(&with(providers), text::WORKFLOW, "\\cite{doi:10.1/x} and \\cite{doi:10.1/X}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "\u{a0}[1] and\u{a0}[1].\n\nReferences\n----------\n\n[1] From the second");
}

#[test]
fn the_numbers_identifiers_and_list_are_the_same_on_one_thread_and_in_parallel() {
    let registry = with(vec![Arc::new(arxiv(&Calls::default()))]);
    let texts = ["A \\cite{b}\\cite{arXiv:1211.1037}.", "B \\cite{arxiv:1211.1037} \\cite{c}.", "C \\cite{b}."];
    let doc = common::pipeline::doc(&registry, &texts);
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let plan = common::stdcore::plan(&registry, workflow, &Overrides::none());
        let under = |options: &RunOptions| {
            let run = common::pipeline::run_with(&doc, &plan, Externals::none(), options);
            let output = if workflow == text::WORKFLOW { text_of(&run) } else { common::html::page(&run) };
            (works(&run), output, identifiers(&run))
        };
        assert_eq!(under(&RunOptions::sequential()), under(&RunOptions::new()), "{workflow}");
    }
}

#[test]
fn the_recomposed_latex_writes_the_keys_as_typed() {
    let registry = with(vec![Arc::new(arxiv(&Calls::default()))]);
    let latex = output_of(&registry, latex_recomposed::WORKFLOW, "Z \\cite{arXiv:1211.1037,arxiv:1211.1037}\\cite{b}.");
    assert!(latex.contains("\\flmCite{arXiv:1211.1037,arxiv:1211.1037}\\flmCite{b}"), "{latex}");
}

/// The identifier of every diagnostic of `run` with the source range it
/// stands at, in the chained view's order.
fn located(run: &Run) -> Vec<(String, std::ops::Range<usize>)> {
    run.diagnostics().iter().map(|d| (d.identifier().to_string(), d.span().range())).collect()
}

/// The range of the first `\cite` call of `document` that names `key`, from
/// its backslash to its closing brace.
fn first_citation_of(document: &str, key: &str) -> std::ops::Range<usize> {
    let mut from = 0;
    while let Some(at) = document[from..].find("\\cite{").map(|at| from + at) {
        let close = at + document[at..].find('}').expect("a closed call");
        let keys = &document[at + "\\cite{".len()..close];
        if keys.split(',').any(|k| k.trim() == key) {
            return at..close + 1;
        }
        from = close;
    }
    panic!("`{key}` is not cited in {document:?}")
}

/// Asserts that every diagnostic of `run` is one of `expected`, in order, and
/// stands within the first citation of its key.
fn assert_located(run: &Run, document: &str, expected: &[(&str, &str)]) {
    let found = located(run);
    assert_eq!(
        found.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        expected.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        "{}",
        run.diagnostics().render_all()
    );
    for ((id, range), (_, key)) in found.iter().zip(expected) {
        let citation = first_citation_of(document, key);
        assert!(citation.start <= range.start && range.end <= citation.end, "{id} at {range:?}, not within {citation:?}");
    }
}

#[test]
fn a_key_whose_entry_is_refused_stands_for_the_work_its_answer_names() {
    // `a`'s entry cites, and is refused; `a` and `b` are answered as one work, `w`.
    let provider = || Batch::of(&Calls::default(), &[("a", "Knuth \\cite{z}", Some("w")), ("b", "Lamport", Some("w"))]);
    let registry = with(vec![Arc::new(provider())]);
    for (document, line) in [
        ("X \\cite{a,b}. Y \\cite{b}.", "[1] [dropped: bibliography entry]"),
        ("X \\cite{b}. Y \\cite{a,b}.", "[1] Lamport"),
    ] {
        let run = run(&registry, text::WORKFLOW, document);
        assert_located(&run, document, &[("flm.citations.entry-cites", "a"), ("flm.citations.no-entry", "a")]);
        let mut works = works(&run);
        works.sort();
        assert_eq!(
            works,
            [
                ("a".to_string(), 1, "citation-1".to_string(), "w".to_string()),
                ("b".to_string(), 1, "citation-1".to_string(), "w".to_string()),
            ],
            "{document}"
        );
        // One number in the chunk citing both, one line in the list: the
        // entry of the work's first-appearing key.
        let text = text_of(&run);
        assert!(text.contains(". Y\u{a0}[1]."), "{text}");
        assert!(text.ends_with(&format!("References\n----------\n\n{line}")), "{text}");
    }
}

#[test]
fn a_key_no_provider_answered_is_a_key_of_the_work_whose_identity_is_spelled_as_it() {
    // No provider claims `x`; `y` is answered with the identity `x`.
    let registry = with(vec![Arc::new(Batch::of(&Calls::default(), &[("y", "Lamport", Some("x"))]))]);
    for (document, line) in [
        ("X \\cite{x,y}. Y \\cite{y}.", "[1] [dropped: bibliography entry]"),
        ("X \\cite{y}. Y \\cite{x,y}.", "[1] Lamport"),
    ] {
        let run = run(&registry, text::WORKFLOW, document);
        assert_located(&run, document, &[("flm.citations.no-entry", "x")]);
        let mut works = works(&run);
        works.sort();
        assert_eq!(
            works,
            [
                ("x".to_string(), 1, "citation-1".to_string(), "x".to_string()),
                ("y".to_string(), 1, "citation-1".to_string(), "x".to_string()),
            ],
            "{document}"
        );
        let text = text_of(&run);
        assert!(text.contains(". Y\u{a0}[1]."), "{text}");
        assert!(text.ends_with(&format!("References\n----------\n\n{line}")), "{text}");
    }
}

/// A provider answering the keys `a` and `b` with an entry of their own name,
/// `a`'s answer carrying the warning `about a`.
struct Warns;

impl CitationProvider for Warns {
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
        if request.key != "a" && request.key != "b" {
            return Ok(None);
        }
        let entry = cx.parse_entry(Arc::new(FlmSource::new(request.key.as_str()).with_origin(FlmOrigin::new("refs.flm"))))?;
        let answer = CitationAnswer::new(entry);
        Ok(Some(if request.key == "a" { answer.with_warning("about a") } else { answer }))
    }
}

#[test]
fn a_warning_of_an_answer_is_reported_at_the_key_s_first_citation_and_changes_nothing_else() {
    let registry = with(vec![Arc::new(Warns)]);
    let document = "X \\cite{b}. Y \\cite{a}. Z \\cite{a,b}.";
    let run = run(&registry, text::WORKFLOW, document);
    assert_located(&run, document, &[("flm.citations.provider-warning", "a")]);
    let rendered = run.diagnostics().render_all();
    assert!(rendered.contains("about a"), "{rendered}");
    let text = text_of(&run);
    assert!(text.starts_with("X\u{a0}[1]. Y\u{a0}[2]. Z\u{a0}[1,2]."), "{text}");
    assert!(text.ends_with("References\n----------\n\n[1] b\n\n[2] a"), "{text}");
    // The same document whose provider has nothing to say reports nothing.
    let run = run_workflow(&with(vec![Arc::new(Batch::of(&Calls::default(), &[("a", "a", None), ("b", "b", None)]))]), text::WORKFLOW, &[document], &Overrides::none());
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
}
