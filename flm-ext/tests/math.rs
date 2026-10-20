//! The mirror of the `math` library and of the mathematics writer
//! (`api::stdcore::features::math`, `userdefs::write_math_run`): each function
//! called through the façade from a Rust stage and a Rust rule, and through
//! the dispatcher, over the standard package (which holds `math` and
//! `math-passthrough`) and a hand-made package; each result compared with what
//! the library answers.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::addr::NodeAddr;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::element::{element_of, element_value};
use flm_core::stdcore::backends::latex::construct::{DEFINITION, MACRO, RAW};
use flm_core::stdcore::backends::{latex_recomposed, pres};
use flm_core::stdcore::features::math::{frame as core_frame, lines as core_lines, MathEnvironment};
use flm_core::stdcore::features::numbering::COUNTERS;
use flm_core::stdcore::features::refs::ANCHORS;
use flm_core::techy::core::node::NodeRef;
use flm_ext::adapter::node::NodeFacade;
use flm_ext::adapter::table::ExtTable;
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, Staged};
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::lang::mint::{ElementDef, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::stdcore::features::math::{self, FrameReader, LineFrame, LineShows};
use flm_ext::api::stdcore::features::userdefs::write_math_run;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::json_double::{module, JsonDouble};
use common::{html_of, latex_of, registry_with};

/// A display-math environment with a labeled numbered line, a line break with
/// its spacing, a tagged line and an unnumbered one; a starred environment; an
/// inline and a displayed group.
const DOCUMENT: &str = "A \\(a\\) and \\[b\\] then \\begin{align} x \\label{eq:x} \\\\[2pt] y \\tag{T} \\\\ \
                        z \\nonumber \\end{align} and \\begin{align*} w \\end{align*} end.";

// --- the readers, against the library ----------------------------------------------------------

/// A line's summary: its content's source, its labels, its tag, its
/// no-number mark and its break's spacing.
type LineSummary = (String, Vec<String>, Option<(bool, String)>, bool, Option<Option<String>>);

fn mirror_line(line: &math::EquationLine<'_>) -> LineSummary {
    (
        line.content.iter().map(|node| node.span_content()).collect(),
        line.labels.iter().map(|label| label.text.clone()).collect(),
        line.tag.as_ref().map(|tag| (tag.starred, tag.content.iter().map(|node| node.span_content()).collect())),
        line.nonumber,
        line.linebreak.as_ref().map(|brk| brk.spacing.clone()),
    )
}

fn core_line(line: &core_lines::EquationLine<'_>) -> LineSummary {
    (
        line.content.iter().map(|node| node.span_content().to_string()).collect(),
        line.labels.iter().map(|label| label.text.clone()).collect(),
        line.tag.as_ref().map(|tag| (tag.star, tag.written())),
        line.nonumber,
        line.linebreak.as_ref().map(|brk| brk.spacing.clone()),
    )
}

/// The document run through `workflow`, and every node of its trees with its
/// address.
fn nodes_of(registry: &Arc<FlmRegistry>) -> Run {
    common::run(registry, "text", DOCUMENT)
}

#[test]
fn the_readers_answer_what_the_library_answers() {
    let registry = registry_with([]);
    let run = nodes_of(&registry);
    let (mut groups, mut environments) = (0, 0);
    for (_, tree) in run.frozen_trees() {
        for node in tree.root().descendants() {
            let mirror = NodeFacade::shared(Arc::clone(tree), node.id());
            assert_eq!(math::formula_display(&mirror), core_frame::formula_display(&node));
            if element_of(&node).name.as_str() == "flm.math" {
                groups += 1;
            }
            if element_of(&node).name.as_str() != math::ENVIRONMENT {
                continue;
            }
            environments += 1;
            let value = element_value::<MathEnvironment>(&node).expect("the value");
            assert_eq!(math::environment_numbered(&mirror), value.numbered && value.numbered_equations);
            let lines: Vec<LineSummary> = math::equation_lines(&mirror).iter().map(mirror_line).collect();
            let expected: Vec<LineSummary> = core_lines::equation_lines(&node).iter().map(core_line).collect();
            assert_eq!(lines, expected);
        }
    }
    assert_eq!((groups, environments), (2, 2));
    // The first environment, as data: its three lines.
    let tree = run.frozen_trees().next().expect("a tree").1;
    let env = tree.root().descendants().find(|node| element_of(node).name.as_str() == math::ENVIRONMENT).expect("align");
    let lines: Vec<LineSummary> =
        math::equation_lines(&NodeFacade::shared(Arc::clone(tree), env.id())).iter().map(mirror_line).collect();
    assert_eq!(lines[0].1, ["eq:x"]);
    assert_eq!(lines[0].4, Some(Some("2pt".to_string())));
    assert_eq!(lines[1].2, Some((false, "T".to_string())));
    assert!(lines[2].3);
}

// --- from a stage ------------------------------------------------------------------------------

/// A frame's summary: its anchor, what it shows, and the number's text or
/// the tag's source.
type FrameSummary = (Option<String>, &'static str, String);

fn mirror_frame(frame: &LineFrame<'_>) -> FrameSummary {
    let (kind, text) = match &frame.shows {
        LineShows::Nothing => ("nothing", String::new()),
        LineShows::Number { text, segments } => {
            assert!(segments.iter().all(|segment| segment.parent().is_none()), "each segment is the root of its value");
            ("number", text.clone())
        }
        LineShows::Tag { starred, content } => {
            (if *starred { "tag*" } else { "tag" }, content.iter().map(|node| node.span_content()).collect())
        }
    };
    assert_eq!(math::number_text(frame), (kind == "number").then(|| text.clone()));
    (frame.anchor.clone(), kind, text)
}

fn core_frame_summary(frame: &core_frame::LineFrame<'_>) -> FrameSummary {
    let (kind, text) = match &frame.shows {
        core_frame::LineShows::Nothing => ("nothing", String::new()),
        core_frame::LineShows::Number(_) => ("number", core_frame::number_text(frame).unwrap_or_default()),
        core_frame::LineShows::Tag { starred, content } => (
            if *starred { "tag*" } else { "tag" },
            content.iter().map(|node: &NodeRef<'_, Flm>| node.span_content().to_string()).collect(),
        ),
    };
    (frame.anchor.clone(), kind, text)
}

/// What the stage saw: per environment, its address and its frames read with
/// and without the tables.
type Seen = Arc<Mutex<Vec<(NodeAddr, Vec<FrameSummary>, Vec<FrameSummary>)>>>;

#[test]
fn line_frames_from_a_stage_answers_what_the_library_answers() {
    let seen: Seen = Arc::default();
    let log = Arc::clone(&seen);
    let package = Double::new("mt")
        .feature("frames", move |b| {
            b.table("mt.frames", "the frames the stage saw")?;
            let log = Arc::clone(&log);
            b.stage(stage(
                StageDecl::new("mt.frames", Effect::Compute, Scope::Document).writes("mt.frames"),
                Needs::new().reads(math::ANCHORS).reads(math::COUNTERS),
                move |cx, out| {
                    for entry in cx.walk() {
                        if entry.node.element_name() != math::ENVIRONMENT {
                            continue;
                        }
                        let with = math::line_frames(FrameReader::Stage(cx), true, true, &*entry.node)?;
                        let without = math::line_frames(FrameReader::Stage(cx), false, false, &*entry.node)?;
                        let with: Vec<FrameSummary> = with.iter().map(mirror_frame).collect();
                        out.table("mt.frames")?.push(json!(with.len()))?;
                        log.lock().expect("the log").push((entry.addr, with, without.iter().map(mirror_frame).collect()));
                    }
                    Ok(())
                },
            ))
        })
        .package();
    let registry = registry_with([package]);
    let source = Arc::new(FlmSource::new(DOCUMENT).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::build(&registry, &PlanRequest::new().demand("mt.frames")).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let items = run.table(flm_core::core::data::table::TableHandle::<ExtTable>::new("mt.frames")).expect("declared");
    assert_eq!(items.map(|table| table.items().to_vec()), Some(vec![json!(3), json!(1)]));

    let anchors = run.table(ANCHORS).expect("declared");
    let counters = run.table(COUNTERS).expect("declared");
    let seen = seen.lock().expect("the log");
    assert_eq!(seen.len(), 2);
    for (addr, with, without) in seen.iter() {
        let tree = run.frozen_tree(addr.tree).expect("the tree");
        let node = tree.node(addr.node);
        let expected = core_frame::line_frames(anchors, counters, *addr, &node).expect("the frames");
        assert_eq!(with, &expected.iter().map(core_frame_summary).collect::<Vec<_>>());
        let expected = core_frame::line_frames(None, None, *addr, &node).expect("the frames");
        assert_eq!(without, &expected.iter().map(core_frame_summary).collect::<Vec<_>>());
    }
    let (_, first, without) = &seen[0];
    assert_eq!(first[0].1, "number");
    assert_eq!(first[0].2, "(1)");
    assert!(first[0].0.is_some());
    assert_eq!((first[1].1, first[1].2.as_str()), ("tag", "T"));
    assert_eq!(first[2].1, "nothing");
    assert_eq!((without[0].0.as_ref(), without[0].2.as_str()), (None, ""));
}

// --- from a rule -------------------------------------------------------------------------------

/// The macro `\mtframes`, the element `mt.frames`, which shows the frames of
/// the document's formulas again.
fn frames_element(feature: &str) -> ElementDef {
    ElementDef::new(feature, ["mtframes"], SyntaxDef::Macro { args: Vec::new() }, ElementSpec::new("mt.frames"))
}

/// The root of `node`'s tree, and in it the first display-math environment
/// and the displayed group.
fn formulas<'t>(node: &dyn Node<'t>) -> (Box<dyn Node<'t> + 't>, Box<dyn Node<'t> + 't>) {
    let mut root = node.parent().expect("a parent");
    while let Some(parent) = root.parent() {
        root = parent;
    }
    let (mut env, mut group) = (None, None);
    for candidate in root.descendants() {
        if env.is_none() && candidate.element_name() == math::ENVIRONMENT {
            env = Some(candidate);
        } else if group.is_none() && candidate.element_name() == "flm.math" && math::formula_display(&*candidate) {
            group = Some(candidate);
        }
    }
    (env.expect("an environment"), group.expect("a displayed group"))
}

/// On the recomposed LaTeX: the environment and the displayed group written
/// again through the LaTeX parts and the mathematics writer.
struct LatexAgain;

impl Lower for LatexAgain {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let (env, group) = formulas(node);
        let mut out: Vec<Staged> = math::environment_opening(cx, lw, &*env)?;
        for frame in math::line_frames(FrameReader::Rule(&*lw), false, false, &*env)? {
            out.extend(write_math_run(cx, lw, &frame.line.content)?);
            out.extend(math::line_closing(cx, lw, &frame)?);
        }
        out.extend(math::environment_closing(cx, lw, &*env)?);
        out.extend(math::group_opening(cx, lw, &*group)?);
        out.extend(write_math_run(cx, lw, &group.children())?);
        out.extend(math::group_closing(cx, lw, &*group)?);
        Ok(Lowering::Emit(out))
    }
}

/// Elsewhere: each line's anchor and what it shows at its tag's place, in a
/// span of role `mt.line`.
struct TagsAgain;

impl Lower for TagsAgain {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        let (env, _) = formulas(node);
        let mut out = Vec::new();
        for frame in math::line_frames(FrameReader::Rule(&*lw), true, true, &*env)? {
            let mut content = vec![lw.text(cx, &format!("[{}]", frame.anchor.clone().unwrap_or_default()))?];
            content.extend(math::stage_tag_content(cx, lw, &frame)?);
            out.push(lw.pres(cx, pres::SPAN, &json!({ "role": "mt.line" }), content)?);
        }
        Ok(Lowering::Emit(out))
    }
}

fn again() -> flm_core::core::registry::PackageEntry {
    Double::new("mt")
        .feature("frames", |b| {
            b.element(frames_element(&b.feature()))?;
            b.peer_table(math::ANCHORS, "the anchors")?;
            b.peer_table(math::COUNTERS, "the counters")?;
            b.peer_table("flm.labels.safe", "the safe labels")?;
            b.peer_backend(latex_recomposed::BACKEND, "the recomposed LaTeX")?;
            for element in [RAW, MACRO, DEFINITION] {
                b.peer_element(element, "a LaTeX construct")?;
            }
            let decl = LoweringDecl::new().produces(pres::SPAN).reads_opt(math::ANCHORS).reads_opt(math::COUNTERS);
            b.lowering_rule(LoweringRule::new("mt.frames.lower.default", "mt.frames", decl, Arc::new(TagsAgain), DefaultMark::Everywhere))?;
            let decl = LoweringDecl::new()
                .produces(RAW)
                .produces(MACRO)
                .produces(DEFINITION)
                .reads_opt("flm.labels.safe")
                .option("MathRecomposeOptions")
                .option("LatexRecomposedRenderOptions");
            b.lowering_rule(LoweringRule::new(
                "mt.frames.lower.latex",
                "mt.frames",
                decl,
                Arc::new(LatexAgain),
                DefaultMark::OnBackends(vec![latex_recomposed::BACKEND.into()]),
            ))
        })
        .package()
}

#[test]
fn the_latex_parts_from_a_rule_write_what_the_library_writes() {
    let registry = registry_with([again()]);
    let (latex, diagnostics) = latex_of(&registry, &format!("{DOCUMENT}\n\n\\mtframes"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // The passthrough rules write each formula through the same parts; the
    // rule writes them again, byte for byte.
    let start = latex.find("\\begin{align}").expect("the environment");
    let end = latex[start..].find("\\end{align}").expect("its end") + start + "\\end{align}".len();
    let written = &latex[start..end];
    assert_eq!(latex.matches(written).count(), 2, "{latex}");
    assert!(written.contains("\\\\[2pt]") && written.contains("\\tag*{(T)}") && written.contains("\\nonumber"), "{written}");
    assert_eq!(latex.matches("\\[b\\]").count(), 2, "{latex}");
}

#[test]
fn stage_tag_content_from_a_rule_stages_the_number_and_the_tag() {
    let registry = registry_with([again()]);
    let (page, diagnostics) = html_of(&registry, &format!("{DOCUMENT}\n\n\\mtframes"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let shown: Vec<&str> = page.split("mt-line").skip(1).map(|part| &part[part.find('>').expect("a tag") + 1..]).collect();
    assert_eq!(shown.len(), 3, "{page}");
    assert!(shown[0].starts_with("[eq-") && shown[0].contains("(1)</"), "{page}");
    // `\tag{T}` stages the tag's content in parentheses, after the anchor.
    assert!(shown[1].starts_with("[eq-") && shown[1].contains("](T)</span>"), "{page}");
    assert!(shown[2].starts_with("[eq-") && shown[2].contains("]</"), "{page}");
}

// --- through the dispatcher --------------------------------------------------------------------

#[test]
fn the_dispatcher_offers_every_function_of_the_math_module_and_the_writer() {
    let double = JsonDouble::new("mt").feature("frames", |js, b| {
        js.call(
            b,
            "element",
            json!([{ "owner": "frames", "names": ["mtframes"], "syntax": { "macro": { "args": [] } }, "element": { "name": "mt.frames" } }]),
        )?;
        for (method, name) in [("peer_backend", latex_recomposed::BACKEND), ("peer_table", "flm.labels.safe")] {
            js.call(b, method, json!([name, "a peer"]))?;
        }
        for element in [RAW, MACRO, DEFINITION] {
            js.call(b, "peer_element", json!([element, "a construct"]))?;
        }
        let lower = js.lend_lower(|js, node, cx, lw| {
            let math = module("stdcore/features/math");
            let names = js.call(&math, "names", json!([]))?;
            assert_eq!(names["ENVIRONMENT"], json!("flm.math.environment"));
            assert_eq!(names["TEXT_COMMANDS"].as_array().map(Vec::len), Some(12));
            assert_eq!(names["ENVIRONMENTS"][1], json!(["equation*", false]));
            // The root, then the environment and the displayed group.
            let mut root = js.call(node, "parent", json!([]))?;
            loop {
                let parent = js.call(&root, "parent", json!([]))?;
                if parent.is_null() {
                    break;
                }
                root = parent;
            }
            let all = js.call(&root, "descendants", json!([]))?;
            let (mut env, mut group) = (Value::Null, Value::Null);
            for candidate in all.as_array().expect("a list") {
                let name = js.call(candidate, "element_name", json!([]))?;
                if name == json!("flm.math.environment") && env.is_null() {
                    env = candidate.clone();
                }
                if name == json!("flm.math") && js.call(&math, "formula_display", json!([candidate]))? == json!(true) {
                    group = candidate.clone();
                }
            }
            assert_eq!(js.call(&math, "environment_numbered", json!([env]))?, json!(true));
            let lines = js.call(&math, "equation_lines", json!([env]))?;
            assert_eq!(lines[0]["labels"][0]["label"], json!({ "ref_type": "eq", "ref_label": "x" }));
            assert_eq!(lines[0]["linebreak"], json!({ "spacing": "2pt" }));
            assert_eq!(lines[1]["tag"]["starred"], json!(false));
            assert_eq!(lines[2]["nonumber"], json!(true));
            let frames = js.call(&math, "line_frames", json!([lw, false, false, env]))?;
            assert_eq!(frames[0]["shows"], json!({ "kind": "number", "segments": [], "text": "" }));
            assert_eq!(js.call(&math, "number_text", json!([frames[0]]))?, json!(""));
            assert_eq!(js.call(&math, "number_text", json!([frames[2]]))?, Value::Null);
            let mut out = Vec::new();
            let mut extend = |ids: Value| out.extend(ids.as_array().expect("a list").iter().cloned());
            extend(js.call(&math, "environment_opening", json!([cx, lw, env]))?);
            for frame in frames.as_array().expect("a list") {
                extend(js.call(&module("stdcore/features/userdefs"), "write_math_run", json!([cx, lw, frame["line"]["content"]]))?);
                extend(js.call(&math, "line_closing", json!([cx, lw, frame]))?);
            }
            extend(js.call(&math, "environment_closing", json!([cx, lw, env]))?);
            extend(js.call(&math, "group_opening", json!([cx, lw, group]))?);
            let content = js.call(&group, "children", json!([]))?;
            extend(js.call(&module("stdcore/features/userdefs"), "write_math_run", json!([cx, lw, content]))?);
            extend(js.call(&math, "group_closing", json!([cx, lw, group]))?);
            // A frame's tag, staged through the dispatcher.
            extend(js.call(&math, "stage_tag_content", json!([cx, lw, frames[1]]))?);
            Ok(json!({ "emit": out }))
        });
        let decl = json!({
            "produces": [RAW, MACRO, DEFINITION],
            "reads_opt": ["flm.labels.safe"],
            "options": ["MathRecomposeOptions", "LatexRecomposedRenderOptions"],
        });
        let rule = js.call(
            &module("core/lowering"),
            "LoweringRule::new",
            json!(["mt.frames.lower.latex", "mt.frames", decl, lower, { "on_backends": ["latex-recomposed"] }]),
        )?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let (latex, diagnostics) = latex_of(&registry, &format!("{DOCUMENT}\n\n\\mtframes"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let start = latex.find("\\begin{align}").expect("the environment");
    let end = latex[start..].find("\\end{align}").expect("its end") + start + "\\end{align}".len();
    assert_eq!(latex.matches(&latex[start..end]).count(), 2, "{latex}");
    assert_eq!(latex.matches("\\[b\\]").count(), 2, "{latex}");
    assert!(latex.contains("\\[b\\](T)"), "{latex}");
}

/// The numbered path through the dispatcher: `line_frames` with both tables
/// lends each number's segments as node handles, `stage_tag_content` reads
/// them back from the frame and stages the number.
#[test]
fn the_dispatcher_stages_a_numbered_frame_s_number() {
    let double = JsonDouble::new("mt").feature("frames", |js, b| {
        js.call(
            b,
            "element",
            json!([{ "owner": "frames", "names": ["mtframes"], "syntax": { "macro": { "args": [] } }, "element": { "name": "mt.frames" } }]),
        )?;
        for table in [math::ANCHORS, math::COUNTERS] {
            js.call(b, "peer_table", json!([table, "a peer"]))?;
        }
        let lower = js.lend_lower(|js, node, cx, lw| {
            let math = module("stdcore/features/math");
            let mut root = js.call(node, "parent", json!([]))?;
            loop {
                let parent = js.call(&root, "parent", json!([]))?;
                if parent.is_null() {
                    break;
                }
                root = parent;
            }
            let all = js.call(&root, "descendants", json!([]))?;
            let mut env = Value::Null;
            for candidate in all.as_array().expect("a list") {
                if env.is_null() && js.call(candidate, "element_name", json!([]))? == json!("flm.math.environment") {
                    env = candidate.clone();
                }
            }
            let frames = js.call(&math, "line_frames", json!([lw, true, true, env]))?;
            assert_eq!(frames[0]["shows"]["kind"], json!("number"));
            assert_eq!(frames[0]["shows"]["text"], json!("(1)"));
            assert!(frames[0]["shows"]["segments"].as_array().is_some_and(|segments| !segments.is_empty()), "{frames}");
            assert_eq!(js.call(&math, "number_text", json!([frames[0]]))?, json!("(1)"));
            let mut roots = Vec::new();
            for frame in frames.as_array().expect("a list") {
                let ids = js.call(&math, "stage_tag_content", json!([cx, lw, frame]))?;
                let tree = json!({
                    "element": "flm.pres.span",
                    "payload": { "role": "mt.line" },
                    "children": ids.as_array().expect("a list").iter().map(|id| json!({ "build": id })).collect::<Vec<_>>(),
                });
                roots.push(js.call(&module("core/lowering"), "pres_tree", json!([lw, cx, tree]))?);
            }
            Ok(json!({ "emit": roots }))
        });
        let decl = json!({ "produces": ["flm.pres.span"], "reads_opt": [math::ANCHORS, math::COUNTERS] });
        let rule = js.call(&module("core/lowering"), "LoweringRule::new", json!(["mt.frames.lower.default", "mt.frames", decl, lower, "everywhere"]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let (page, diagnostics) = html_of(&registry, &format!("{DOCUMENT}\n\n\\mtframes"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let shown: Vec<&str> = page.split("mt-line").skip(1).map(|part| &part[part.find('>').expect("a tag") + 1..]).collect();
    assert_eq!(shown.len(), 3, "{page}");
    assert!(shown[0].starts_with("(1)</span>"), "{page}");
    assert!(shown[1].starts_with("(T)</span>"), "{page}");
    assert!(shown[2].starts_with("</span>"), "{page}");
}

// --- E4 (S49): the span of a line record ---------------------------------------------------------

#[test]
fn a_line_record_carries_its_span() {
    use flm_ext::adapter::handles::CallTable;
    use flm_ext::adapter::values::equation_line_to_value;
    let registry = registry_with([]);
    // The content's whitespace is content: a line runs from its first node
    // to its last. A line with no content stands at its `\\`; the empty last
    // line has no span (below).
    let text = "\\begin{align}x+1\\label{eq:a}\\\\[1pt]\\\\y \\\\\\end{align}";
    let source = Arc::new(flm_core::lang::FlmSource::new(text).with_origin(flm_core::lang::FlmOrigin::none()));
    let fragment = flm_core::lang::parse_fragment(
        registry.lang(),
        source,
        flm_core::model::doc::FragmentLayout::default(),
        &flm_core::lang::ParseOptions::default(),
    )
    .expect("the text parses");
    let root = NodeFacade::shared(Arc::clone(fragment.tree()), fragment.tree().root().id());
    let env = root.descendants().into_iter().find(|node| node.element_name() == "flm.math.environment").expect("the environment");
    let mut table = CallTable::new(8200);
    let records: Vec<Value> = math::equation_lines(&*env).into_iter().map(|line| equation_line_to_value(&mut table, line)).collect();
    let slice = |record: &Value| -> Option<String> {
        let span = &record["span"];
        let (start, end) = (span["start"].as_u64()? as usize, span["end"].as_u64()? as usize);
        Some(text[start..end].to_string())
    };
    let label = &records[0]["labels"][0]["span"];
    let (start, end) = (label["start"].as_u64().unwrap_or(0) as usize, label["end"].as_u64().unwrap_or(0) as usize);
    assert_eq!(&text[start..end], "\\label{eq:a}");
    let spans: Vec<Option<String>> = records.iter().map(slice).collect();
    assert_eq!(
        spans,
        [Some("x+1".to_string()), Some("\\\\".to_string()), Some("y ".to_string())],
        "{records:?}"
    );
    // An environment with no content: one line with no node and no `\\`.
    let empty = Arc::new(flm_core::lang::FlmSource::new("\\begin{align}\\end{align}").with_origin(flm_core::lang::FlmOrigin::none()));
    let fragment = flm_core::lang::parse_fragment(
        registry.lang(),
        empty,
        flm_core::model::doc::FragmentLayout::default(),
        &flm_core::lang::ParseOptions::default(),
    )
    .expect("the text parses");
    let root = NodeFacade::shared(Arc::clone(fragment.tree()), fragment.tree().root().id());
    let env = root.descendants().into_iter().find(|node| node.element_name() == "flm.math.environment").expect("the environment");
    let records: Vec<Value> = math::equation_lines(&*env).into_iter().map(|line| equation_line_to_value(&mut table, line)).collect();
    assert_eq!(records.iter().map(|record| record["span"].clone()).collect::<Vec<_>>(), [Value::Null]);
}
