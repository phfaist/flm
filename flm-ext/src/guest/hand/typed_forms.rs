//! The typed conversions of the guest's own types that no test of an
//! extension reaches, each checked against the dispatcher's conversion of the
//! same value's JSON form (the twins of `tests/guest_forms.rs`'s rows, which
//! cannot reach the crate's conversion traits). A conversion from the API's
//! value ([`FromApi`]) writes what the dispatcher's reply writes
//! ([`IntoReply`]); a conversion into the API's value ([`IntoApi`]) gives
//! what the dispatcher reads from the guest's JSON form ([`FromArg`]), or the
//! host's reader of that form. Where a conversion lends objects, the value is
//! built the same way in two tables of one call, so that the objects each
//! side lends have the same handles.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use flm_core::core::registry::{PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::addr::{NodeAddr as CoreNodeAddr, TreeId};
use flm_core::model::aspects::PointId as CorePointId;
use flm_core::model::doc::FragmentLayout;
use flm_core::stdlib::features::refs::Anchors as CoreAnchors;
use flm_core::stdlib::{self, StdlibConfig};
use flm_core::techy::core::node::NodeTree;

use crate::adapter::aspects::{ref_point_from_value, ref_point_to_core};
use crate::adapter::dispatch::extra::{anchors_id, LowererTable, PointArg};
use crate::adapter::handles::{CallTable, Entry, Handle, HandleKind, TableEntry};
use crate::adapter::node::{NodeFacade, TreeFacade};
use crate::adapter::tables::{AnchorsFacade, Seen};
use crate::adapter::typed::{FromApi, IntoApi};
use crate::adapter::values::{citation_results_from_value, handle_to_value, line_frame_to_value, Args, FromArg, IntoReply, JsonForm};
use crate::api;
use crate::api::stdlib::features::math as api_math;
use crate::guest;

/// A parsed tree of `text`.
fn tree(text: &str) -> Arc<NodeTree<Flm>> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default()).expect("the text parses");
    Arc::clone(fragment.tree())
}

/// The root of `tree`, as the host's node.
fn root(tree: &Arc<NodeTree<Flm>>) -> Box<dyn api::techy::node::Node<'static>> {
    Box::new(NodeFacade::shared(Arc::clone(tree), tree.root().id()))
}

/// `tree`, as the host's tree.
fn shared(tree: &Arc<NodeTree<Flm>>) -> Box<dyn api::techy::node::Tree> {
    Box::new(TreeFacade::new(Arc::clone(tree)))
}

/// The address of the root of `tree`, the first tree of a document.
fn root_addr(tree: &Arc<NodeTree<Flm>>) -> CoreNodeAddr {
    CoreNodeAddr { tree: TreeId::tree(0), node: tree.root().id() }
}

/// The guest's node of the root of `tree`, lent into `table`.
fn guest_root(table: &mut CallTable<'static>, tree: &Arc<NodeTree<Flm>>) -> guest::techy::node::Node {
    let handle = table.lend(Entry::Node(root(tree)));
    JsonForm::from_value(&handle_to_value(handle)).expect("the guest reads a node's handle")
}

/// The dispatcher's reading of `value` as its first argument.
fn from_arg<'c, T: FromArg<'c>>(table: &mut CallTable<'c>, value: Value) -> T {
    let args = json!([value]);
    T::from_arg(table, &Args::new(&args).expect("a list"), 0).expect("the dispatcher reads the guest's form")
}

/// The guest's value of `api` in one table and the dispatcher's reply of
/// `twin` (the same value, built again) in another of the same call, both
/// written as JSON.
fn from_api_and_reply<A: IntoReply<'static>, G: FromApi<'static, A> + JsonForm>(api: A, twin: A) -> (Value, Value) {
    let (mut typed, mut replied) = (CallTable::new(1), CallTable::new(1));
    let guest = G::from_api(api, &mut typed).expect("the typed conversion");
    (guest.to_value(), twin.into_reply(&mut replied).expect("the dispatcher's reply"))
}

#[test]
fn a_walk_entry_and_its_address_built_from_the_api_are_the_dispatcher_s_reply() {
    let parsed = tree("abc");
    let entry = || api::core::run::WalkEntry { addr: root_addr(&parsed), node: root(&parsed) };
    let (typed, replied) = from_api_and_reply::<_, guest::core::run::WalkEntry>(entry(), entry());
    assert_eq!(typed, replied);
    let (typed, replied) = from_api_and_reply::<_, guest::core::lowering::NodeAddr>(root_addr(&parsed), root_addr(&parsed));
    assert_eq!(typed, replied);
}

#[test]
fn a_table_a_lowerer_answers_is_the_dispatcher_s_reply() {
    let tables: [fn() -> LowererTable<'static>; 4] = [
        || LowererTable::Projection(json!({ "k": [1] })),
        || LowererTable::Counters(Handle { kind: HandleKind::Counters, call: 1, index: 0 }),
        || LowererTable::Anchors(Handle { kind: HandleKind::Anchors, call: 1, index: 0 }),
        || LowererTable::Owned(api::core::data::table::TableValue::Projection(json!("p"))),
    ];
    for table in tables {
        let (typed, replied) = from_api_and_reply::<_, guest::core::data::table::TableValue>(table(), table());
        assert_eq!(typed, replied);
    }
}

#[test]
fn a_counter_record_and_a_number_segment_built_from_the_api_are_the_dispatcher_s_reply() {
    let parsed = tree("abc");
    let record = api::stdlib::features::numbering::CounterRecord {
        counter: "equation".to_string(),
        value: vec![1, -2],
        style: "arabic".to_string(),
        instance: Some(root_addr(&parsed)),
    };
    let (typed, replied) = from_api_and_reply::<_, guest::stdlib::features::numbering::CounterRecord>(record.clone(), record);
    assert_eq!(typed, replied);
    for link in [Some(CorePointId { addr: root_addr(&parsed), index: 2 }), None] {
        let segment = || api::stdlib::features::numbering::NumberSegment { content: shared(&parsed), link };
        let (typed, replied) = from_api_and_reply::<_, guest::stdlib::features::numbering::NumberSegment>(segment(), segment());
        assert_eq!(typed, replied);
    }
}

/// An equation line of `parsed`, with a tag and a line break.
fn line(parsed: &Arc<NodeTree<Flm>>) -> api_math::EquationLine<'static> {
    api_math::EquationLine {
        content: vec![root(parsed)],
        labels: vec![],
        tag: Some(api_math::Tag { starred: true, content: vec![root(parsed)] }),
        nonumber: false,
        linebreak: Some(api_math::LineBreak { spacing: Some("2pt".to_string()), span: api::techy::node::Span { start: 0, end: 1 } }),
    }
}

/// A line's frame of `parsed`, showing the `shows`-th of nothing, a number
/// or a tag.
fn frame(parsed: &Arc<NodeTree<Flm>>, shows: usize) -> api_math::LineFrame<'static> {
    let shows = match shows {
        0 => api_math::LineShows::Nothing,
        1 => api_math::LineShows::Number { segments: vec![shared(parsed)], text: "1".to_string() },
        _ => api_math::LineShows::Tag { starred: false, content: vec![root(parsed)] },
    };
    api_math::LineFrame { environment: root(parsed), index: 0, line: line(parsed), anchor: Some("eq-1".to_string()), shows }
}

#[test]
fn an_equation_line_and_a_line_frame_built_from_the_api_are_the_dispatcher_s_reply() {
    let parsed = tree("x");
    let (typed, replied) = from_api_and_reply::<_, guest::stdlib::features::math::EquationLine>(line(&parsed), line(&parsed));
    assert_eq!(typed, replied);
    for shows in 0..3 {
        let (typed, replied) = from_api_and_reply::<_, guest::stdlib::features::math::LineFrame>(frame(&parsed, shows), frame(&parsed, shows));
        assert_eq!(typed, replied);
    }
}

#[test]
fn a_line_frame_given_to_the_api_is_what_the_dispatcher_reads() {
    use guest::stdlib::features::math::LineFrame;
    let parsed = tree("x");
    for shows in 0..3 {
        // The guest's frame, read from what the host wrote, in each table.
        let (mut typed, mut read) = (CallTable::new(1), CallTable::new(1));
        let frame_in = |table: &mut CallTable<'static>| {
            LineFrame::from_value(&line_frame_to_value(table, frame(&parsed, shows))).expect("the guest reads the host's frame")
        };
        let (guest, twin) = (frame_in(&mut typed), frame_in(&mut read));
        let converted = (&guest).into_api(&mut typed).expect("the typed conversion");
        let host: api_math::LineFrame<'static> = from_arg(&mut read, twin.to_value());
        assert_eq!(converted.into_reply(&mut typed).expect("a reply"), host.into_reply(&mut read).expect("a reply"));
        let converted = [guest].as_slice().into_api(&mut typed).expect("the typed conversion");
        let host: Vec<api_math::LineFrame<'static>> = from_arg(&mut read, json!([twin.to_value()]));
        assert_eq!(converted.into_reply(&mut typed).expect("a reply"), host.into_reply(&mut read).expect("a reply"));
    }
}

#[test]
fn a_presentation_tree_given_to_the_api_is_what_the_dispatcher_reads() {
    use guest::core::lowering::PresTree;
    let mut table = CallTable::new(1);
    let pres = PresTree::Node {
        element: "par".to_string(),
        payload: json!({ "k": 1 }),
        children: vec![PresTree::Text("x".to_string()), PresTree::Node { element: "span".to_string(), payload: Value::Null, children: vec![] }],
    };
    let converted = (&pres).into_api(&mut table).expect("the typed conversion");
    let host: api::core::lowering::PresTree = from_arg(&mut table, pres.to_value());
    assert_eq!(converted, host);
}

#[test]
fn a_point_given_to_the_api_reads_back_as_the_dispatcher_reads_its_data() {
    let parsed = tree("abc");
    let point = CorePointId { addr: root_addr(&parsed), index: 0 };
    let anchors = CoreAnchors { ids: BTreeMap::from([(point, Some("eq-1".to_string()))]) };
    let mut table = CallTable::new(1);
    let seen = Seen::Rule { id: TreeId::tree(0), tree: &parsed };
    let handle = table.lend(Entry::Anchors(TableEntry::Lent(Box::new(AnchorsFacade::new(&anchors, seen)))));
    let guest = guest::model::aspects::PointId {
        addr: guest::core::lowering::NodeAddr { tree: TreeId::tree(0).to_string(), node: parsed.root().id().index() },
        index: 0,
    };
    let data = guest.to_value();
    let parts = guest.into_api(&mut table).expect("the typed conversion");
    let typed = anchors_id(&mut table, handle, &parts).expect("the point reads back");
    let read = anchors_id(&mut table, handle, &PointArg::Data(&data)).expect("the point reads back");
    assert_eq!((typed.as_deref(), read.as_deref()), (Some("eq-1"), Some("eq-1")));
}

#[test]
fn a_reference_point_s_number_given_to_the_api_is_what_the_host_reads() {
    use guest::model::aspects::{AnchorPolicy, NumberSource, RefPoint};
    let parsed = tree("abc");
    let mut table = CallTable::new(1);
    let node = guest_root(&mut table, &parsed);
    let point = RefPoint {
        kind: "equation".to_string(),
        labels: vec![],
        computed: 1,
        malformed: vec![],
        anchor: AnchorPolicy::Auto { prefix: "a-".to_string() },
        number: Some(NumberSource {
            counter: "equation".to_string(),
            instances: vec![node].into(),
            style: "arabic".to_string(),
            advance: vec![1],
            number_format: Some("(#)".to_string()),
        }),
        content: None,
        accepts: api::model::aspects::AcceptedRefTypes::Any,
    };
    let data = point.to_value();
    let converted = point.into_api(&mut table).expect("the typed conversion");
    let read = ref_point_from_value(&mut table, &data).expect("the host reads the guest's point");
    let at = parsed.root();
    assert_eq!(ref_point_to_core(converted, &at).expect("a point"), ref_point_to_core(read, &at).expect("a point"));
}

#[test]
fn a_citation_answer_given_to_the_api_is_what_the_host_reads() {
    use guest::stdlib::features::citations::{CitationAnswer, CitationEntry, CitationResult};
    let parsed = tree("Ann, 2001");
    let mut table = CallTable::new(1);
    let core_entry = flm_core::stdlib::features::citations::CitationEntry::new(Arc::clone(&parsed));
    let lent = table.lend(Entry::CitationEntry(Box::new(crate::adapter::citations::CitationEntryFacade(core_entry))));
    let entry = CitationEntry::from_value(&handle_to_value(lent)).expect("the guest reads the entry");
    let answer = CitationAnswer::new(entry).with_identity("ann2001").with_warning("a warning");
    let results: Vec<CitationResult> = vec![Ok(Some(answer)), Ok(None)];
    let data = results.to_value();
    let converted = results.into_api(&mut table).expect("the typed conversion");
    let read = citation_results_from_value(&mut table, &data).expect("the host reads the guest's results");
    let shape = |results: &[api::stdlib::features::citations::CitationResult]| -> Vec<Option<(Option<String>, Vec<String>)>> {
        results
            .iter()
            .map(|result| match result {
                Ok(answer) => answer.as_ref().map(|answer| (answer.identity.clone(), answer.warnings.clone())),
                Err(error) => panic!("an answer, not `{error}`"),
            })
            .collect()
    };
    assert_eq!(shape(&converted), shape(&read));
    assert_eq!(shape(&converted)[0], Some((Some("ann2001".to_string()), vec!["a warning".to_string()])));
}
