//! The cross tests of the guest's own types (refresh section 6.3): each
//! guest type's conversion, written once in `src/guest/hand/`, agrees with
//! the host's converter of the same shape. Where the host writes the shape,
//! its converter writes a value into a call's table, the guest's type reads
//! it and writes it back unchanged, and the host's reader (where it has one)
//! reads that; where the host only reads the shape, the guest's type writes a
//! value, the host's reader reads it, and the guest's type reads its own form
//! back. A staged node cannot be made outside the crate, so a list of staged
//! nodes is shown to reach the host's reader at its handle (a node lent at
//! that index is refused as an object of another kind); likewise a citation
//! entry (their accepting halves, which need the crate's insides, are unit
//! tests under `src/guest/`). The types that hold handler objects are
//! tested as the record-and-lend methods build them: here what needs no
//! host (a data aspect, a helper's declaration, a backend of library
//! stages, the refusals), and under `src/guest/registry.rs` what a lending
//! answers, with the client's test hook in place of the host. Last, the
//! module page's sections.

#![cfg(feature = "guest")]

use std::sync::Arc;

use serde_json::{json, Value};

use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::model::doc::FragmentLayout;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_core::techy::core::node::NodeTree;
use flm_ext::adapter::durable::{DurableKind, DurableToken};
use flm_ext::adapter::handles::{CallTable, Entry, Handle, HandleKind};
use flm_ext::adapter::node::NodeFacade;
use flm_ext::adapter::values::{self as host, handle_from_value, handle_to_value, JsonForm};
use flm_ext::api;
use flm_ext::guest;
use flm_ext::ExtError;

/// A parsed tree of `text`.
fn tree(text: &str) -> Arc<NodeTree<Flm>> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
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

/// The root of `tree` lent into `table`, as its handle's JSON.
fn lend_root(table: &mut CallTable<'static>, tree: &Arc<NodeTree<Flm>>) -> Value {
    handle_to_value(table.lend(Entry::Node(root(tree))))
}

/// The guest's node for the handle `value`.
fn guest_node(value: &Value) -> guest::techy::node::Node {
    guest::techy::node::Node::from_value(value).expect("the guest reads a node's handle")
}

/// The handle of `value` with its kind changed to `kind`, as the host writes
/// a handle: the guest's object of that kind at the same index.
fn as_kind(value: &Value, kind: HandleKind) -> Value {
    let handle = handle_from_value(value).expect("a handle");
    handle_to_value(Handle { kind, ..handle })
}

/// A handle of `kind` the host writes, for a test whose host object cannot
/// be made outside the crate.
fn handle_of_kind(kind: HandleKind) -> Value {
    handle_to_value(Handle { kind, call: 1, index: 0 })
}

/// The guest's type read from what the host wrote, and written back
/// unchanged.
fn read_back<T: JsonForm>(written: &Value) -> T {
    let guest = T::from_value(written).unwrap_or_else(|error| panic!("the guest reads `{written}`: {error}"));
    assert_eq!(guest.to_value(), *written, "the guest writes back what it read");
    guest
}

/// The guest's type written, then read back as itself.
fn round_trip<T: JsonForm + PartialEq + std::fmt::Debug>(value: &T) {
    assert_eq!(&T::from_value(&value.to_value()).expect("the guest reads its own form"), value);
}

/// The host's argument of the code `code`, named `name`, which the host
/// writes with its code (one built from a standard kind, as
/// `ArgDef::mandatory` builds it, is written with its structured syntax).
fn host_arg(code: &str, name: &str) -> api::lang::mint::ArgDef {
    let mut arg = api::lang::mint::arg_def_from_code(code).expect("a code");
    arg.name = Some(name.to_string());
    arg
}

/// The detail of an `ExtError::Argument` or `ExtError::Value`.
fn detail(error: &ExtError) -> &str {
    match error {
        ExtError::Argument { detail, .. } | ExtError::Value { detail } => detail,
        other => panic!("an argument or a value error, not `{other}`"),
    }
}

/// The host's reader of a list of staged nodes reaches the guest's handle:
/// lent as a node at that index, it is refused as one of another kind.
fn reaches_the_staged_handle(answer: Result<impl Sized, ExtError>) {
    let error = answer.err().expect("the node lent where a staged node is expected is refused");
    assert_eq!(detail(&error), "an object of the kind `staged` is expected here, not one of the kind `node`");
}

#[test]
fn an_expansion_crosses_both_ways() {
    use guest::lang::rewrites::userdefs::{Expansion, Staged};
    let mut table = CallTable::new(1);
    let written = host::expansion_to_value(&mut table, &api::lang::rewrites::userdefs::Expansion::Default);
    let read: Expansion = read_back(&written);
    assert_eq!(read, Expansion::Default);
    assert!(matches!(host::expansion_from_value(&mut table, &read.to_value()), Ok(api::lang::rewrites::userdefs::Expansion::Default)));
    let node = lend_root(&mut table, &tree("a"));
    let staged = Staged::from_value(&as_kind(&node, HandleKind::Staged)).expect("a staged handle");
    let emit = Expansion::Emit(vec![staged]);
    round_trip(&emit);
    reaches_the_staged_handle(host::expansion_from_value(&mut table, &emit.to_value()));
    let error = Expansion::from_value(&json!("descend")).expect_err("a word of another shape");
    assert_eq!(detail(&error), "an expansion must be `\"default\"` or `{\"emit\": [...]}`, not `\"descend\"`");
}

#[test]
fn a_lowering_crosses_both_ways() {
    use guest::core::lowering::Lowering;
    use guest::lang::rewrites::userdefs::Staged;
    let mut table = CallTable::new(1);
    let written = host::lowering_to_value(&mut table, &api::core::lowering::Lowering::Default);
    let read: Lowering = read_back(&written);
    assert!(matches!(host::lowering_from_value(&mut table, &read.to_value()), Ok(api::core::lowering::Lowering::Default)));
    let node = lend_root(&mut table, &tree("a"));
    let emit = Lowering::Emit(vec![Staged::from_value(&as_kind(&node, HandleKind::Staged)).expect("a staged handle")]);
    round_trip(&emit);
    reaches_the_staged_handle(host::lowering_from_value(&mut table, &emit.to_value()));
}

#[test]
fn a_restage_crosses_both_ways() {
    use guest::lang::rewrites::userdefs::Staged;
    use guest::techy::transform::Restage;
    let mut table = CallTable::new(1);
    let written = host::restage_to_value(&mut table, &api::techy::transform::Restage::Descend);
    let read: Restage = read_back(&written);
    assert!(matches!(host::restage_from_value(&mut table, &read.to_value()), Ok(api::techy::transform::Restage::Descend)));
    let node = lend_root(&mut table, &tree("a"));
    let emit = Restage::Emit(vec![Staged::from_value(&as_kind(&node, HandleKind::Staged)).expect("a staged handle")]);
    round_trip(&emit);
    reaches_the_staged_handle(host::restage_from_value(&mut table, &emit.to_value()));
}

#[test]
fn a_presentation_tree_written_by_the_guest_is_read_by_the_host() {
    use guest::core::lowering::PresTree;
    use guest::lang::rewrites::userdefs::Staged;
    let mut table = CallTable::new(1);
    let tree_value = PresTree::Node {
        element: "par".to_string(),
        payload: json!({ "k": 1 }),
        children: vec![PresTree::Text("x".to_string()), PresTree::Node { element: "span".to_string(), payload: json!({}), children: vec![] }],
    };
    round_trip(&tree_value);
    match host::pres_tree_from_value(&mut table, &tree_value.to_value()).expect("the host reads the guest's tree") {
        api::core::lowering::PresTree::Node { element, payload, children } => {
            assert_eq!((element.as_str(), payload, children.len()), ("par", json!({ "k": 1 }), 2));
            assert!(matches!(&children[0], api::core::lowering::PresTree::Text(text) if text == "x"));
        }
        _ => panic!("a node of the tree"),
    }
    let node = lend_root(&mut table, &tree("a"));
    let build = PresTree::Build(Staged::from_value(&as_kind(&node, HandleKind::Staged)).expect("a staged handle"));
    round_trip(&build);
    reaches_the_staged_handle(host::pres_tree_from_value(&mut table, &build.to_value()));
}

#[test]
fn a_diagnostic_written_by_the_guest_is_read_by_the_host() {
    use guest::techy::error::Diagnostic;
    let mut table = CallTable::new(1);
    let node = guest_node(&lend_root(&mut table, &tree("abc")));
    let diagnostic = Diagnostic::warning("flm.test.cross", "a message").with_data(json!({ "n": 1 })).at(&node);
    round_trip(&diagnostic);
    let read = host::diagnostic_from_value(&mut table, &diagnostic.to_value()).expect("the host reads the guest's diagnostic");
    assert_eq!((read.id.as_str(), read.message.as_str(), read.data, read.at.is_some()), ("flm.test.cross", "a message", json!({ "n": 1 }), true));
    assert_eq!(read.severity, api::techy::error::Severity::Warning);
    let unplaced = Diagnostic::error("flm.test.cross", "no place");
    let read = host::diagnostic_from_value(&mut table, &unplaced.to_value()).expect("the host reads it");
    assert!(read.at.is_none());
}

#[test]
fn a_walk_entry_and_a_node_address_written_by_the_host_are_read_by_the_guest() {
    let mut table = CallTable::new(1);
    let parsed = tree("abc");
    let addr = NodeAddr { tree: TreeId::tree(0), node: parsed.root().id() };
    let written = host::node_addr_to_value(&addr);
    let guest_addr: guest::core::lowering::NodeAddr = read_back(&written);
    let entry = api::core::run::WalkEntry { addr, node: root(&parsed) };
    let written = host::walk_entry_to_value(&mut table, entry);
    let read: guest::core::run::WalkEntry = read_back(&written);
    assert_eq!(read.addr, guest_addr);
}

#[test]
fn a_point_identifier_and_a_counter_record_written_by_the_host_are_read_by_the_guest() {
    let parsed = tree("abc");
    let addr = NodeAddr { tree: TreeId::tree(0), node: parsed.root().id() };
    let point = flm_core::model::aspects::PointId { addr, index: 2 };
    let read: guest::model::aspects::PointId = read_back(&host::point_id_to_value(&point));
    assert_eq!(read.index, 2);
    let record = api::stdcore::features::numbering::CounterRecord {
        counter: "equation".to_string(),
        value: vec![1, -2],
        style: "arabic".to_string(),
        instance: Some(addr),
    };
    let read: guest::stdcore::features::numbering::CounterRecord = read_back(&host::counter_record_to_value(&record));
    assert_eq!((read.value.as_slice(), read.instance), (&[1, -2][..], Some(read_back(&host::node_addr_to_value(&addr)))));
}

#[test]
fn a_number_segment_in_the_hosts_shape_is_read_by_the_guest() {
    // The host's reply for a segment is `IntoReply`, private to the crate: its
    // shape is built here from the host's writers of a handle and a point.
    let mut table = CallTable::new(1);
    let parsed = tree("abc");
    let node = lend_root(&mut table, &parsed);
    let point = flm_core::model::aspects::PointId { addr: NodeAddr { tree: TreeId::tree(0), node: parsed.root().id() }, index: 0 };
    let written = json!({ "content": node, "link": host::point_id_to_value(&point) });
    let read: guest::stdcore::features::numbering::NumberSegment = read_back(&written);
    assert_eq!(read.link.map(|link| link.index), Some(0));
    read_back::<guest::stdcore::features::numbering::NumberSegment>(&json!({ "content": node, "link": null }));
}

#[test]
fn a_table_written_by_the_host_is_read_by_the_guest() {
    use guest::core::data::table::TableValue;
    let mut table = CallTable::new(1);
    let written = host::table_value_to_value(&mut table, api::core::data::table::TableValue::Projection(json!({ "k": [1] })));
    assert!(matches!(read_back::<TableValue>(&written), TableValue::Projection(value) if value == json!({ "k": [1] })));
    // The tables lent as handles: their objects are the adapter's; the
    // handle is written as the host writes it.
    for (key, kind) in [
        ("ext", HandleKind::ExtTableRead),
        ("trees", HandleKind::TreesTable),
        ("lowered", HandleKind::LoweredTrees),
        ("counters", HandleKind::Counters),
        ("anchors", HandleKind::Anchors),
    ] {
        read_back::<TableValue>(&json!({ key: handle_of_kind(kind) }));
    }
    let error = TableValue::from_value(&json!({ "ext": handle_of_kind(HandleKind::Node) })).expect_err("a node is no table");
    assert_eq!(detail(&error), "an object of the kind `ext_table_read` is expected here, not one of the kind `node`");
}

#[test]
fn an_equation_line_and_a_line_frame_cross_both_ways() {
    use api::stdcore::features::math as math;
    let mut table = CallTable::new(1);
    let parsed = tree("x");
    let line = || math::EquationLine {
        content: vec![root(&parsed)],
        labels: vec![],
        tag: Some(math::Tag { starred: true, content: vec![root(&parsed)] }),
        nonumber: false,
        linebreak: Some(math::LineBreak { spacing: Some("2pt".to_string()), span: api::techy::node::Span { start: 0, end: 1 } }),
    };
    let written = host::equation_line_to_value(&mut table, line());
    let read: guest::stdcore::features::math::EquationLine = read_back(&written);
    assert_eq!(read.linebreak.and_then(|brk| brk.spacing).as_deref(), Some("2pt"));
    assert!(read.tag.is_some_and(|tag| tag.starred));
    let shows = || {
        vec![
            math::LineShows::Nothing,
            math::LineShows::Number { segments: vec![root(&parsed)], text: "1".to_string() },
            math::LineShows::Tag { starred: false, content: vec![root(&parsed)] },
        ]
    };
    for shows in shows() {
        let frame = math::LineFrame { environment: root(&parsed), index: 0, line: line(), anchor: Some("eq-1".to_string()), shows };
        let written = host::line_frame_to_value(&mut table, frame);
        let read: guest::stdcore::features::math::LineFrame = read_back(&written);
        let back = host::line_frame_from_value(&mut table, &read.to_value()).expect("the host reads the guest's frame");
        assert_eq!((back.index, back.anchor.as_deref()), (0, Some("eq-1")));
    }
}

#[test]
fn a_frame_reader_is_a_stage_context_or_a_lowerer() {
    use guest::stdcore::features::math::FrameReader;
    assert!(matches!(read_back::<FrameReader>(&handle_of_kind(HandleKind::StageCx)), FrameReader::Stage(_)));
    assert!(matches!(read_back::<FrameReader>(&handle_of_kind(HandleKind::Lowerer)), FrameReader::Rule(_)));
    let error = FrameReader::from_value(&handle_of_kind(HandleKind::Node)).expect_err("a node reads no frames");
    assert_eq!(detail(&error), "an object of the kind `stage_cx` or `lowerer` is expected here, not one of the kind `node`");
}

#[test]
fn a_reference_point_written_by_the_guest_is_read_by_the_host() {
    use guest::model::aspects::{AnchorPolicy, ContentRef, NumberSource, RefPoint};
    let mut table = CallTable::new(1);
    let node = guest_node(&lend_root(&mut table, &tree("abc")));
    let anchors = [
        AnchorPolicy::FromFirstLabel,
        AnchorPolicy::Explicit("id-1".to_string()),
        AnchorPolicy::Slug { prefix: "eq-".to_string(), content: ContentRef::Node(node) },
        AnchorPolicy::FromValue { prefix: "v-".to_string() },
        AnchorPolicy::Auto { prefix: "a-".to_string() },
    ];
    for anchor in anchors {
        let point = RefPoint {
            kind: "equation".to_string(),
            labels: vec![api::model::aspects::Label::from_value(&json!({ "ref_type": "eq", "ref_label": "one" })).expect("a label")],
            computed: 1,
            malformed: vec![],
            anchor,
            number: Some(NumberSource {
                counter: "equation".to_string(),
                instances: vec![node],
                style: "arabic".to_string(),
                advance: vec![1],
                number_format: None,
            }),
            content: Some(ContentRef::Body),
            accepts: api::model::aspects::AcceptedRefTypes::Any,
        };
        round_trip(&point);
        let read = flm_ext::adapter::aspects::ref_point_from_value(&mut table, &point.to_value()).expect("the host reads the guest's point");
        assert_eq!((read.labels.len(), read.computed, read.number.is_some(), read.content.is_some()), (1, 1, true, true));
    }
}

#[test]
fn a_content_reference_written_by_the_guest_is_read_by_the_host() {
    use guest::model::aspects::ContentRef;
    let mut table = CallTable::new(1);
    let node = guest_node(&lend_root(&mut table, &tree("abc")));
    for content in [
        ContentRef::Argument(1),
        ContentRef::ArgumentNamed("title".to_string()),
        ContentRef::Body,
        ContentRef::Node(node),
        ContentRef::Synthesized(node),
    ] {
        round_trip(&content);
        let read = flm_ext::adapter::aspects::content_ref_from_value(&mut table, &content.to_value()).expect("the host reads it");
        let same = matches!(
            (&content, &read),
            (ContentRef::Argument(1), api::model::aspects::ContentRef::Argument(1))
                | (ContentRef::ArgumentNamed(_), api::model::aspects::ContentRef::ArgumentNamed(_))
                | (ContentRef::Body, api::model::aspects::ContentRef::Body)
                | (ContentRef::Node(_), api::model::aspects::ContentRef::Node(_))
                | (ContentRef::Synthesized(_), api::model::aspects::ContentRef::Synthesized(_))
        );
        assert!(same, "the host reads `{}` as the guest wrote it", content.to_value());
    }
}

#[test]
fn citation_results_written_by_the_guest_are_read_by_the_host() {
    use guest::stdcore::features::citations::{CitationAnswer, CitationEntry, CitationRequest, CitationResult};
    let mut table = CallTable::new(1);
    let failure = api::stdcore::features::citations::CitationProviderError::from_value(&json!({ "error": "no such key" })).expect("an error");
    let results: Vec<CitationResult> = vec![Ok(None), Err(failure)];
    let read = host::citation_results_from_value(&mut table, &results.to_value()).expect("the host reads the guest's results");
    assert!(matches!(read.as_slice(), [Ok(None), Err(_)]));
    let node = lend_root(&mut table, &tree("a"));
    let entry = CitationEntry::from_value(&as_kind(&node, HandleKind::CitationEntry)).expect("an entry's handle");
    let answer = CitationAnswer::new(entry).with_identity("knuth").with_warning("a warning");
    round_trip(&answer);
    let results: Vec<CitationResult> = vec![Ok(Some(answer))];
    for result in &results {
        assert!(CitationResult::from_value(&result.to_value()).expect("the guest reads its result").is_ok());
    }
    let error = host::citation_results_from_value(&mut table, &results.to_value()).err().expect("a node lent where an entry is expected is refused");
    assert_eq!(detail(&error), "an object of the kind `citation_entry` is expected here, not one of the kind `node`");
    // A request, in the shape of the host's `citation_requests_to_value`
    // (`flm-core`'s request has no constructor outside `flm-core`).
    let request: CitationRequest = read_back(&json!({ "key": "knuth84" }));
    assert_eq!(request.key, "knuth84");
    assert_eq!(host::citation_requests_to_value(&[]), json!([]));
}

#[test]
fn a_host_handle_is_the_reader_of_files_or_a_named_service() {
    use guest::host::HostHandle;
    assert!(matches!(read_back::<HostHandle>(&handle_of_kind(HandleKind::Files)), HostHandle::Files(_)));
    assert!(matches!(read_back::<HostHandle>(&handle_of_kind(HandleKind::HostService)), HostHandle::Service(_)));
    let error = HostHandle::from_value(&handle_of_kind(HandleKind::Node)).expect_err("a node is no service");
    assert_eq!(detail(&error), "an object of the kind `files` or `host_service` is expected here, not one of the kind `node`");
}

#[test]
fn an_argument_s_role_crosses_both_ways() {
    use guest::lang::mint::ArgDef;
    let mut table = CallTable::new(1);
    let written = host::arg_def_to_value(&host_arg("{", "key").role("wc.key"));
    assert_eq!(written["role"], json!("wc.key"));
    let read: ArgDef = read_back(&written);
    assert_eq!(read.to_value()["role"], json!("wc.key"));
    let guest_arg = ArgDef::named("target", "{").role("flm.refs.target");
    round_trip(&guest_arg);
    let args = host::arg_defs_from_value(&json!([guest_arg.to_value(), read.to_value(), ArgDef::new("[").to_value()]), &mut |value: &Value| {
        host::held_from_table(&mut table, value)
    })
    .expect("the host reads the guest's arguments");
    let roles: Vec<Option<&str>> = args.iter().map(|arg| arg.role.as_ref().map(|name| name.as_str())).collect();
    assert_eq!(roles, [Some("flm.refs.target"), Some("wc.key"), None]);
}

#[test]
fn an_argument_crosses_both_ways_as_data_or_as_a_helpers_handle() {
    use api::lang::mint::ArgDef as HostArg;
    use guest::lang::mint::ArgDef;
    let mut table = CallTable::new(1);
    let written = host::arg_def_to_value(&host_arg("{", "x").blocks());
    let read: ArgDef = read_back(&written);
    assert_eq!(read.code(), Some("{"));
    let guest_arg = ArgDef::optional("o").docs("an option");
    let args = host::arg_defs_from_value(&json!([guest_arg.to_value(), read.to_value()]), &mut |value: &Value| host::held_from_table(&mut table, value))
        .expect("the host reads the guest's arguments");
    assert_eq!(args.iter().map(|arg| arg.name.as_deref()).collect::<Vec<_>>(), [Some("o"), Some("x")]);
    // A helper's argument, lent by the host: the guest writes its handle back.
    let lent = handle_to_value(table.lend(Entry::ArgDef(HostArg::optional("label"))));
    let read: ArgDef = read_back(&lent);
    assert_eq!(read.code(), None);
    let args = host::arg_defs_from_value(&json!([read.to_value()]), &mut |value: &Value| host::held_from_table(&mut table, value))
        .expect("the host resolves the helper's argument");
    assert_eq!(args[0].name.as_deref(), Some("label"));
    // A setter called on it keeps its field beside the handle, which the
    // guest reads back and the host's reader refuses.
    let changed = read.mode(api::lang::mint::Mode::Math).scoped("x.pkg").docs("a label");
    let mut expected = lent.clone();
    expected["mode"] = json!("math");
    expected["scoped"] = json!(["x.pkg"]);
    expected["docs"] = json!("a label");
    assert_eq!(changed.to_value(), expected);
    round_trip(&changed);
    let error = host::arg_defs_from_value(&json!([changed.to_value()]), &mut |value: &Value| host::held_from_table(&mut table, value))
        .expect_err("a helper's argument written with changes is refused");
    assert_eq!(
        detail(&error),
        "the argument `label` that a helper made must be used as the helper made it, without the fields `mode`, `scoped`, \
         `docs` set on it"
    );
}

#[test]
fn a_definition_crosses_both_ways() {
    use api::lang::rewrites::userdefs::{Definition as HostDefinition, DefinitionKind};
    let mut table = CallTable::new(1);
    let definition = HostDefinition::new(DefinitionKind::Macro, "greet", vec![host_arg("{", "who")])
        .content("Hello, #1!")
        .default("who", "world")
        .extra_placeholder("extra", true);
    let written = host::definition_to_value(&definition);
    let read: guest::lang::rewrites::userdefs::Definition = read_back(&written);
    assert_eq!((read.name.as_str(), read.content.as_deref(), read.args.len()), ("greet", Some("Hello, #1!"), 1));
    let back = host::definition_from_value(&read.to_value(), &mut |value: &Value| host::held_from_table(&mut table, value))
        .expect("the host reads the guest's definition");
    assert_eq!(host::definition_to_value(&back), written);
    let built = guest::lang::rewrites::userdefs::Definition::new(DefinitionKind::Environment, "box", vec![]).docs("a box").overriding();
    round_trip(&built);
    let back = host::definition_from_value(&built.to_value(), &mut |value: &Value| host::held_from_table(&mut table, value)).expect("it reads");
    assert_eq!((back.docs.as_str(), back.overrides), ("a box", true));
}

#[test]
fn a_definition_s_constructors_write_what_the_api_s_make() {
    use api::lang::rewrites::userdefs::Definition as HostDefinition;
    use guest::lang::rewrites::userdefs::Definition;
    let mut table = CallTable::new(1);
    let host_args = || vec![host_arg("{", "who")];
    let guest_args = || vec![read_back::<guest::lang::mint::ArgDef>(&host::arg_def_to_value(&host_arg("{", "who")))];
    let cases = [
        ("macro_", Definition::macro_("greet", guest_args()), HostDefinition::macro_("greet", host_args())),
        ("environment", Definition::environment("box", guest_args()), HostDefinition::environment("box", host_args())),
        ("specials", Definition::specials("~~", guest_args()), HostDefinition::specials("~~", host_args())),
        ("text", Definition::macro_("greet", guest_args()).text("Hello, #1!"), HostDefinition::macro_("greet", host_args()).text("Hello, #1!")),
        ("math", Definition::macro_("vec", guest_args()).math(r"\mathbf{#1}"), HostDefinition::macro_("vec", host_args()).math(r"\mathbf{#1}")),
    ];
    for (constructor, guest_definition, host_definition) in cases {
        let expected = host::definition_to_value(&host_definition);
        assert_eq!(guest_definition.to_value(), expected, "`Definition::{constructor}` writes what the api's makes");
        let read = host::definition_from_value(&guest_definition.to_value(), &mut |value: &Value| host::held_from_table(&mut table, value))
            .unwrap_or_else(|error| panic!("the host reads the definition of `Definition::{constructor}`: {error}"));
        assert_eq!(host::definition_to_value(&read), expected, "the host reads back what `Definition::{constructor}` wrote");
    }
}

#[test]
fn a_syntax_written_by_the_guest_is_read_by_the_host() {
    use guest::lang::mint::{ArgDef, SyntaxDef};
    let mut table = CallTable::new(1);
    let syntaxes = [
        SyntaxDef::Macro { args: vec![ArgDef::mandatory("a")] },
        SyntaxDef::Environment { args: vec![ArgDef::optional("o")], body: api::lang::mint::BodyDef::new() },
        SyntaxDef::Specials { trigger: "~".to_string(), args: vec![] },
        SyntaxDef::Synthesized,
    ];
    for syntax in syntaxes {
        let written = syntax.to_value();
        assert_eq!(SyntaxDef::from_value(&written).expect("the guest reads its own form").to_value(), written);
        let read = host::syntax_def_from_value(&written, &mut |value: &Value| host::held_from_table(&mut table, value)).expect("the host reads it");
        let same = matches!(
            (&syntax, &read),
            (SyntaxDef::Macro { .. }, api::lang::mint::SyntaxDef::Macro { .. })
                | (SyntaxDef::Environment { .. }, api::lang::mint::SyntaxDef::Environment { .. })
                | (SyntaxDef::Specials { .. }, api::lang::mint::SyntaxDef::Specials { .. })
                | (SyntaxDef::Synthesized, api::lang::mint::SyntaxDef::Synthesized)
        );
        assert!(same, "the host reads `{written}` as the guest wrote it");
    }
}

#[test]
fn a_body_with_a_parser_of_its_own_written_by_the_guest_is_refused_by_the_host() {
    use guest::lang::mint::SyntaxDef;
    let mut table = CallTable::new(1);
    let body = api::lang::mint::BodyDef::new().parser(flm_core::lang::mint::FlmUnknownEnvironmentBehavior::new());
    let written = SyntaxDef::Environment { args: vec![], body }.to_value();
    let refused = host::syntax_def_from_value(&written, &mut |value: &Value| host::held_from_table(&mut table, value));
    assert!(matches!(refused, Err(ExtError::Value { .. })), "a parser the shape cannot name is refused");
    let verbatim = SyntaxDef::Environment { args: vec![], body: api::lang::mint::BodyDef::new().verbatim() };
    let read = host::syntax_def_from_value(&verbatim.to_value(), &mut |value: &Value| host::held_from_table(&mut table, value))
        .expect("the verbatim parser crosses");
    assert!(matches!(read, api::lang::mint::SyntaxDef::Environment { body, .. } if body.parser.is_some()));
}

#[test]
fn a_compiled_template_and_a_template_language_cross_in_the_form_they_were_read() {
    use guest::lang::rewrites::userdefs::{CompiledTemplate, TemplateLanguage};
    let compiled = host::durable_to_value(DurableToken { kind: DurableKind::CompiledTemplate, table: 3, index: 1 });
    read_back::<CompiledTemplate>(&compiled);
    read_back::<CompiledTemplate>(&handle_of_kind(HandleKind::CompiledTemplate));
    let language = host::durable_to_value(DurableToken { kind: DurableKind::TemplateLanguage, table: 3, index: 0 });
    let read: TemplateLanguage = read_back(&language);
    assert_eq!(host::durable_from_value(&read.to_value()).map(host::durable_to_value).ok(), Some(language.clone()));
    let error = CompiledTemplate::from_value(&language).expect_err("a language is no compiled template");
    assert_eq!(detail(&error), "a durable token of the kind `compiled_template` is expected here, not one of the kind `template_language`");
    let error = TemplateLanguage::from_value(&compiled).expect_err("a compiled template is no language");
    assert_eq!(detail(&error), "a durable token of the kind `template_language` is expected here, not one of the kind `compiled_template`");
}

#[test]
fn value_bindings_are_given_where_placeholder_bindings_are_taken() {
    use guest::lang::rewrites::userdefs::{PlaceholderBindings, ValueBindings};
    // The api's `ValueBindings` is a `PlaceholderBindings`: the host's arm
    // takes a handle of either kind, and the guest sends the value bindings'
    // own handle.
    let written = handle_of_kind(HandleKind::ValueBindings);
    let bindings = ValueBindings::from_value(&written).expect("a handle of value bindings");
    assert_eq!(PlaceholderBindings::from(bindings).to_value(), written);
}

#[test]
fn an_element_spec_with_a_data_aspect_crosses_inside_a_declaration_a_fallback_and_a_scoped_package() {
    use api::model::aspects::{BlockRole, FixedBlockRole};
    use guest::lang::mint::{ArgDef, ElementDef, FallbackDef, ScopedPackageDef, SyntaxDef};
    use guest::model::element::{Aspect, ElementSpec};
    let mut table = CallTable::new(1);
    let spec = ElementSpec::new("x.box")
        .docs("A box.")
        .with(Aspect::BlockRoleAspect(FixedBlockRole(BlockRole::Block)))
        .expect("a data aspect needs no register")
        .with_value(json!({ "width": 3 }));
    assert_eq!(spec.to_value(), json!({ "name": "x.box", "docs": "A box.", "aspects": [{ "BlockRoleAspect": "block" }], "value": { "width": 3 } }));
    let def = ElementDef::new("x", ["box"], SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] }, spec.clone());
    let read = host::element_def_from_table(&mut table, &def.to_value()).expect("the host reads the declaration");
    let element = read.element().expect("a data declaration");
    assert_eq!((element.name.as_str(), element.aspects().len()), ("x.box", 1));
    let fallback = FallbackDef::new([api::lang::mint::Mode::Text], api::lang::mint::FallbackKind::Environment, spec);
    let read = host::fallback_def_from_table(&mut table, &fallback.to_value()).expect("the host reads the fallback");
    assert_eq!(read.element.name, "x.box");
    let scoped = ScopedPackageDef::new("x.scoped", vec![def], "Scoped.");
    let read = host::scoped_package_def_from_table(&mut table, &scoped.to_value()).expect("the host reads the scoped package");
    assert_eq!((read.name.as_str(), read.elements.len()), ("x.scoped", 1));
    assert!(format!("{fallback:?}").starts_with("FallbackDef"), "{fallback:?}");
}

#[test]
fn a_handler_object_given_while_no_register_runs_is_refused() {
    use guest::lang::rewrites::userdefs::{Expand, ExpandCx, Expander, Expansion};
    use guest::model::element::{Aspect, ElementSpec};
    struct Hook;
    impl Expand for Hook {
        fn expand(&self, _: &guest::techy::node::Node, _: &mut ExpandCx, _: &mut Expander) -> Result<Expansion, ExtError> {
            Ok(Expansion::Default)
        }
    }
    let error = ElementSpec::new("x.box").with(Aspect::Expand(Arc::new(Hook))).expect_err("no register runs");
    assert_eq!(
        error,
        ExtError::Handler { detail: "the expansion hook of the element `x.box` must be attached while a feature's `register` runs".to_string(), data: Value::Null }
    );
    assert_eq!(format!("{:?}", Aspect::Expand(Arc::new(Hook))), "Expand(\"…\")");
}

#[test]
fn a_helpers_declaration_is_read_back_as_itself() {
    use guest::lang::mint::ElementDef;
    let lent = handle_of_kind(HandleKind::ElementDef);
    let def: ElementDef = read_back(&lent);
    assert!(def.owner().is_none() && def.names().is_none() && def.syntax().is_none() && def.element().is_none());
    // `docs` on it asks the host; outside a WebAssembly module there is none.
    assert!(matches!(def.docs("text"), Err(ExtError::Unsupported { .. })));
}

#[test]
fn a_backend_of_library_stages_crosses_and_a_changed_stage_is_refused() {
    use flm_ext::guest::abi::ToArg;
    use guest::core::registry::{BackendDef, BackendStage, LibraryStage};
    let mut table = CallTable::new(1);
    let mut stage = |backend: &str| {
        let lent = handle_to_value(table.lend(Entry::LibraryStage(api::stdcore::backends::lowering::LoweringStage::new(backend))));
        LibraryStage::from_value(&lent).expect("a library stage")
    };
    let (html, text) = (stage("html"), stage("text"));
    let def = BackendDef::new("x", vec!["x.box".to_string()], BackendStage::Library(html), BackendStage::Library(html), BackendStage::Library(html))
        .expect("library stages need no register")
        .docs("A backend.");
    assert_eq!(format!("{def:?}"), "BackendDef { name: \"x\", native: [\"x.box\"], .. }");
    let read = host::backend_def_from_table(&mut table, &def.arg().expect("unchanged")).expect("the host reads the backend");
    assert_eq!((read.name.as_str(), read.docs.as_str()), ("x", "A backend."));
    let mut changed = def;
    changed.final_stage = BackendStage::Library(text);
    assert_eq!(
        changed.arg(),
        Err(ExtError::Argument {
            method: String::new(),
            detail: "the stage `final_stage` of the backend `x` must be the stage given to `BackendDef::new`".to_string()
        })
    );
}

#[test]
fn the_module_page_carries_the_sections_and_the_mapping_rules() {
    let page = include_str!("../src/guest/mod.rs");
    let headings = [
        "//! 1. **Module paths.**",
        "//! 2. **A struct or enum of plain data**",
        "//! 3. **An enum of unit variants**",
        "//! 4. **A reference to an object with methods**",
        "//! 5. **A method generic over a type**",
        "//! 6. **A free function**",
        "//! 7. **A trait an extension implements**",
        "//! 8. **`Result<T, E>`**",
        "//! 9. **`Option<T>`**",
        "//! 10. **Values are JSON.**",
        "//! 11. **A diagnostic**",
        "//! 12. **A `const` name**",
    ];
    for heading in headings {
        assert!(page.contains(heading), "the module page lacks {heading:?}");
    }
    let sections = [
        "# What the guest client is",
        "# An extension in Rust",
        "# The packages and the features",
        "# The mapping rules",
        "# The API modules",
        "# The limits",
        "# The rehearsal",
        "# Failures",
        "# The digest",
    ];
    let mut last = 0;
    for section in sections {
        let at = page.find(&format!("//! {section}\n")).unwrap_or_else(|| panic!("the module page lacks the section {section:?}"));
        assert!(at > last, "the section {section:?} is out of order");
        last = at;
    }
    // The first paragraph states the move from a crate written against `flm_ext::api`,
    // and names the places where it is not mechanical (ruling S8).
    let first = page.split("\n//!\n").next().unwrap_or_default();
    for named in ["`flm_ext::api::a::b` becomes `flm_ext::guest::a::b`", "ElementDef::docs", "ElementDef::modes", "ArgDef"] {
        assert!(first.contains(named), "the first paragraph lacks {named:?}");
    }
}
