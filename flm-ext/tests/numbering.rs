//! The numbering of an extension's construct (plan E4, section 9.3): a
//! counter and a counter style contributed as data, the number a rule reads
//! back through the `Counters` façade, the `\ref` that prints the style's
//! word, and a point read back from data. The rules act through the
//! dispatcher alone (the JSON double), as a script's do; one test reads the
//! table through the Rust façade.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::names::{Name, RefKind};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::numbering::{Delimiters, NumberingOptions, PrefixWords, Within};
use flm_ext::adapter::handles::Entry;
use flm_ext::api::core::data::table::TableValue;
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::model::aspects::{AcceptedRefTypes, AnchorPolicy, ContentRef, NumberSource, PointId, RefPoint, Referenceable};
use flm_ext::api::stdcore::features::numbering::{FormatOptions, COUNTERS, COUNTERS_SLOT, PARENT_SETS_SLOT, STYLES_SLOT};
use flm_ext::api::stdcore::features::refs;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::Double;
use common::json_double::{module, JsonDouble};
use common::{registry_with, text_of, try_registry_with};

const COUNTER: &str = "ex.counter";
const STYLE: &str = "ex.style";
const PART: &str = "ex.part";

/// The reference points of a construct, numbered by `counter` in the style
/// [`STYLE`], or not numbered.
struct Points {
    counter: Option<&'static str>,
}

impl Referenceable for Points {
    fn points<'c>(&self, node: &'c dyn Node<'c>) -> Result<Vec<RefPoint<'c>>, ExtError> {
        let uses = refs::label_uses(node);
        let (labels, malformed) = refs::point_labels(&uses);
        let number = self.counter.map(|counter| NumberSource {
            counter: Name::from(counter),
            instances: Vec::new(),
            style: Name::from(STYLE),
            advance: vec![0],
            number_format: None,
        });
        Ok(vec![RefPoint {
            kind: RefKind::from("exbox"),
            labels,
            computed: 0,
            malformed,
            anchor: AnchorPolicy::FromFirstLabel,
            number,
            content: Some(ContentRef::Argument(0)),
            accepts: AcceptedRefTypes::Any,
        }])
    }
}

/// What the rule of `\exbox` writes before the content.
#[derive(Clone)]
enum Shows {
    /// The number, formatted with these options (data).
    Number(Value),
    /// The record of the point, as data.
    Record,
    /// The error of `record` for the point given as data.
    Refused(Value),
    /// The anchor of the point, as data (S18).
    Anchor,
}

/// The package `ex`: the counter [`COUNTER`] declared with `within` and the
/// style [`STYLE`] with `style` (its other fields), both through `peer_slot`;
/// any further slot values `more`; the constructs `\exbox{…}` (numbered by
/// [`COUNTER`]), `\exnone{…}` (not numbered) and `\expart{…}` (numbered by
/// [`PART`], which `more` declares where a test uses it), each followed by
/// `\label` fields, whose rule writes what `shows` says, then `: ` and the
/// content.
fn exercises(within: Value, style: Value, more: Vec<(&'static str, Value)>, shows: Shows) -> JsonDouble {
    JsonDouble::new("ex").feature("boxes", move |js, b| {
        let mut style = style.clone();
        style["name"] = json!(STYLE);
        js.call(b, "peer_slot", json!([STYLES_SLOT, style]))?;
        js.call(b, "peer_slot", json!([COUNTERS_SLOT, { "name": COUNTER, "default_style": STYLE, "within": within }]))?;
        for (slot, value) in &more {
            js.call(b, "peer_slot", json!([slot, value]))?;
        }
        let fields = js.call(&module("stdcore/features/refs"), "label_fields_arg", json!([]))?;
        for (name, counter) in [("exbox", Some(COUNTER)), ("exnone", None), ("expart", Some(PART))] {
            let element = format!("ex.{name}");
            js.call(b, "element", json!([{ "names": [name], "syntax": { "macro": { "args": ["{", fields] } }, "element": { "name": element } }]))?;
            let points = js.lend(Entry::Referenceable(Arc::new(Points { counter })));
            js.call(b, "attach", json!([element, { "Referenceable": points }]))?;
            let shows = shows.clone();
            let body = js.lend_lower(move |js, node, cx, lw| {
                let counters = js.call(lw, "get", json!([COUNTERS]))?;
                let point = json!({ "addr": js.call(lw, "addr", json!([]))?, "index": 0 });
                let mut emit: Vec<Value> = Vec::new();
                let shown = match &shows {
                    Shows::Number(options) => {
                        let segments = js.call(&counters["counters"], "format", json!([point, options]))?;
                        for segment in segments.as_array().cloned().unwrap_or_default() {
                            let staged = js.call(lw, "splice", json!([cx, segment["content"]]))?;
                            emit.extend(staged.as_array().cloned().unwrap_or_default());
                        }
                        None
                    }
                    Shows::Record => Some(js.call(&counters["counters"], "record", json!([point]))?.to_string()),
                    Shows::Anchor => {
                        let anchors = js.call(lw, "get", json!([refs::ANCHORS]))?;
                        Some(js.call(&anchors["anchors"], "id", json!([point]))?.to_string())
                    }
                    Shows::Refused(bad) => Some(match js.call(&counters["counters"], "record", json!([bad])) {
                        Ok(value) => format!("accepted {value}"),
                        Err(error) => error.to_string(),
                    }),
                };
                if let Some(shown) = shown {
                    emit.push(js.call(lw, "text", json!([cx, shown]))?);
                }
                emit.push(js.call(lw, "text", json!([cx, ": "]))?);
                let content = js.call(lw, "lower_arg", json!([cx, node, 0]))?;
                emit.extend(content.as_array().cloned().unwrap_or_default());
                Ok(json!({ "emit": emit }))
            });
            let rule = js.call(
                &module("core/lowering"),
                "LoweringRule::new",
                json!([format!("{element}.lower.default"), element, { "reads_opt": [COUNTERS, refs::ANCHORS] }, body, "everywhere"]),
            )?;
            js.call(b, "lowering_rule", json!([rule]))?;
        }
        Ok(())
    })
}

/// The style's prefix words `Exercise~`, `Exercises~`, as data.
fn words() -> Value {
    json!({ "prefix_words": PrefixWords::new("Exercise~", "Exercises~") })
}

/// The text of `source` under a registry of the standard package and `package`.
fn text(package: JsonDouble, source: &str) -> String {
    let registry = registry_with([package.package()]);
    let (text, diagnostics) = text_of(&registry, source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The text backend wraps its lines; the tests compare the words.
    text.replace('\n', " ").trim_end().to_string()
}

/// The counter [`PART`], as data.
fn part() -> (&'static str, Value) {
    (COUNTERS_SLOT, json!({ "name": PART, "default_style": STYLE }))
}

#[test]
fn a_construct_is_numbered_and_ref_prints_the_style_s_word_and_the_number() {
    let out = text(
        exercises(Value::Null, words(), Vec::new(), Shows::Number(json!({}))),
        "\\exbox{A}\\label{ex:a} \\exbox{B} \\exbox{C}\\label{ex:c} See \\ref{ex:c} and \\ref{ex:a}.",
    );
    assert_eq!(out, "Exercise\u{a0}1: A Exercise\u{a0}2: B Exercise\u{a0}3: C See Exercise\u{a0}3 and Exercise\u{a0}1.");
}

#[test]
fn without_word_and_without_delimiters_take_effect() {
    let style = json!({ "prefix_words": PrefixWords::new("Exercise~", "Exercises~"), "delimiters": Delimiters::new("(", ")") });
    let source = "\\exbox{A}";
    let shown = |options: Value| text(exercises(Value::Null, style.clone(), Vec::new(), Shows::Number(options)), source);
    assert_eq!(shown(json!({})), "Exercise\u{a0}(1): A");
    assert_eq!(shown(json!({ "with_word": false })), "(1): A");
    assert_eq!(shown(json!({ "with_word": false, "with_delimiters": false })), "1: A");
}

#[test]
fn record_answers_null_for_an_unnumbered_point() {
    let out = text(exercises(Value::Null, words(), Vec::new(), Shows::Record), "\\exbox{A} \\exnone{B} \\exbox{C}");
    assert_eq!(
        out,
        "{\"counter\":\"ex.counter\",\"value\":[1],\"style\":\"ex.style\",\"instance\":null}: A null: B \
         {\"counter\":\"ex.counter\",\"value\":[2],\"style\":\"ex.style\",\"instance\":null}: C"
    );
}

#[test]
fn a_point_of_another_tree_or_past_the_end_is_refused() {
    let refused = |bad: Value| text(exercises(Value::Null, words(), Vec::new(), Shows::Refused(bad)), "\\exbox{A}");
    assert_eq!(
        refused(json!({ "addr": { "tree": "tree#7", "node": 0 }, "index": 0 })),
        "`record`: the address of the point must be in the tree being lowered (`tree#0`), not in `tree#7`: A"
    );
    let past = refused(json!({ "addr": { "tree": "tree#0", "node": 4000 }, "index": 0 }));
    assert!(past.starts_with("`record`: the address of the point must name a node of the tree `tree#0`: it has "), "{past}");
    assert!(past.ends_with(" nodes, not a node 4000: A"), "{past}");
    assert_eq!(
        refused(json!({ "addr": { "tree": "tree#0" }, "index": 0 })),
        "`record`: a point's address must have the field `node`: A"
    );
}

#[test]
fn within_parent_numbers_within_another_counter() {
    let within = serde_json::to_value(Within::parent(PART)).expect("data");
    let out = text(
        exercises(within, words(), vec![part()], Shows::Number(json!({ "with_word": false }))),
        "\\expart{One} \\exbox{A} \\exbox{B} \\expart{Two} \\exbox{C}",
    );
    assert_eq!(out, "1: One 1.1: A 1.2: B 2: Two 2.1: C");
}

#[test]
fn a_parent_set_resolves_a_within_parent_set() {
    let within = serde_json::to_value(Within::parent_set("ex.levels")).expect("data");
    let set = json!({ "name": "ex.levels", "counters": [PART] });
    let out = text(
        exercises(within, words(), vec![part(), (PARENT_SETS_SLOT, set)], Shows::Number(json!({ "with_word": false }))),
        "\\expart{One} \\exbox{A} \\expart{Two} \\exbox{B}",
    );
    assert_eq!(out, "1: One 1.1: A 2: Two 2.1: B");
}

#[test]
fn the_document_s_initial_value_changes_the_first_number() {
    let registry = registry_with([exercises(Value::Null, words(), Vec::new(), Shows::Number(json!({ "with_word": false }))).package()]);
    let source = Arc::new(FlmSource::new("\\exbox{A} \\exbox{B}").with_origin(FlmOrigin::none()));
    let fragment =
        parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default()).expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let options = OptionValues::new().with(NumberingOptions::new().initial(COUNTER, 4));
    let plan = Arc::new(Plan::for_workflow(&registry, text::WORKFLOW, &Overrides::none()).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::with_options(content, options)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(out.replace('\n', " ").trim_end(), "5: A 6: B");
}

// --- the Rust façade -------------------------------------------------------------------------------

/// A rule's body that reads the counters through the Rust façade and writes
/// the record's value and the number, unworded, before the content.
struct ThroughTheFacade(Arc<Mutex<Vec<String>>>);

impl Lower for ThroughTheFacade {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let point = PointId { addr: lw.addr(), index: 0 };
        let (record, segments) = match lw.get(COUNTERS)? {
            Some(TableValue::Counters(counters)) => {
                (counters.record(point), counters.format(point, &FormatOptions::new().without_word())?)
            }
            _ => return Err(ExtError::Handler { detail: "no counters table".to_string(), data: Value::Null }),
        };
        self.0.lock().expect("the log").push(format!("{:?}", record.map(|record| record.value)));
        let mut emit = Vec::new();
        for segment in segments.unwrap_or_default() {
            emit.extend(lw.splice(cx, &*segment.content)?);
        }
        emit.push(lw.text(cx, ": ")?);
        emit.extend(lw.lower_arg(cx, node, 0)?);
        Ok(Lowering::Emit(emit))
    }
}

#[test]
fn the_rust_facade_reads_the_record_and_formats_the_number() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let body = Arc::new(ThroughTheFacade(Arc::clone(&log)));
    let package = Double::new("ex")
        .feature("boxes", move |b| {
            b.peer_slot(STYLES_SLOT, flm_ext::api::core::registry::Contributed::Data(json!({ "name": STYLE, "delimiters": Delimiters::new("[", "]") })))?;
            b.peer_slot(COUNTERS_SLOT, flm_ext::api::core::registry::Contributed::Data(json!({ "name": COUNTER, "default_style": STYLE })))?;
            let spec = flm_ext::api::model::element::ElementSpec::new("ex.exbox")
                .with(flm_ext::api::model::element::Aspect::Referenceable(Arc::new(Points { counter: Some(COUNTER) })))
                .expect("one aspect");
            let args = vec![flm_ext::api::lang::mint::ArgDef::mandatory("content"), refs::label_fields_arg()];
            b.element(flm_ext::api::lang::mint::ElementDef::new(
                b.feature(),
                ["exbox"],
                flm_ext::api::lang::mint::SyntaxDef::Macro { args },
                spec,
            ))?;
            let decl = LoweringDecl::new().reads_opt(COUNTERS);
            b.lowering_rule(LoweringRule::new("ex.exbox.lower.default", "ex.exbox", decl, body.clone(), DefaultMark::Everywhere))
        })
        .package();
    let registry = try_registry_with([package]).expect("the registry builds");
    let (out, diagnostics) = text_of(&registry, "\\exbox{A} \\exbox{B}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(out.replace('\n', " ").trim_end(), "[1]: A [2]: B");
    assert_eq!(*log.lock().expect("the log"), ["Some([1])", "Some([2])"]);
}

#[test]
fn a_stage_reads_the_counters_and_a_point_outside_the_document_is_refused() {
    // The stage's side of section 9.2: the table answered by the stage's
    // context, a point read back against the document's trees.
    let found = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&found);
    let mut package = exercises(Value::Null, words(), Vec::new(), Shows::Record);
    package = package.feature("looking", move |js, b| {
        let log = Arc::clone(&log);
        js.call(b, "table", json!(["ex.found", "What the stage found."]))?;
        let stage = js.lend_stage(
            &json!({ "name": "ex.look", "effect": "compute", "scope": "document", "writes": ["ex.found"] }),
            &json!({ "reads": [COUNTERS, refs::ANCHORS] }),
            move |js, cx, out| {
                let counters = js.call(cx, "get", json!([COUNTERS]))?;
                let walk = js.call(cx, "walk", json!([]))?;
                // The record of the first walked node that holds one: the
                // point of `\exbox{A}`.
                let mut record = Value::Null;
                let mut addr = Value::Null;
                for entry in walk.as_array().cloned().unwrap_or_default() {
                    let found = js.call(&counters["counters"], "record", json!([{ "addr": entry["addr"], "index": 0 }]))?;
                    if !found.is_null() {
                        (record, addr) = (found, entry["addr"].clone());
                        break;
                    }
                }
                let mut seen = vec![record.to_string()];
                // S18: the anchors table a stage's context answers.
                let anchors = js.call(cx, "get", json!([refs::ANCHORS]))?;
                let anchor = js.call(&anchors["anchors"], "id", json!([{ "addr": addr, "index": 0 }]))?;
                let bad = json!({ "addr": { "tree": "tree#99", "node": 0 }, "index": 0 });
                seen.push(match js.call(&counters["counters"], "record", json!([bad])) {
                    Ok(value) => format!("accepted {value}"),
                    Err(error) => error.to_string(),
                });
                seen.push(anchor.to_string());
                log.lock().expect("the log").extend(seen.clone());
                let piece = js.call(out, "table", json!(["ex.found"]))?;
                js.call(&piece, "push", json!([seen]))?;
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([stage]))?;
        Ok(())
    });
    let registry = registry_with([package.package()]);
    let source = Arc::new(FlmSource::new("\\exbox{A}\\label{ex:one}").with_origin(FlmOrigin::none()));
    let fragment =
        parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default()).expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = flm_core::core::plan::PlanRequest::new().demand("ex.found");
    let plan = Arc::new(Plan::build(&registry, &request).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let found = found.lock().expect("the log").clone();
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].contains("\"counter\":\"ex.counter\""), "{found:?}");
    assert_eq!(found[2], "\"ex-one\"");
    assert_eq!(found[1], "`record`: the address of the point must be in a tree of the document, not in `tree#99`");
}

#[test]
fn a_rule_reads_the_anchor_of_a_labeled_point_and_none_for_an_unlabeled_one() {
    // S18: the anchors table a rule's lowerer answers, recorded and asked of
    // the lowerer again, and `Anchors.id` through the dispatcher.
    let shown = text(exercises(Value::Null, words(), Vec::new(), Shows::Anchor), "\\exbox{A}\\label{ex:one}\n\n\\exbox{B}");
    assert_eq!(shown, "\"ex-one\": A  null: B");
}
