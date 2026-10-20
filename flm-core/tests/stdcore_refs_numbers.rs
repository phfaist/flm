//! References to numbered things: the identifier a reference point is given
//! under each anchor policy, and the text a `\ref` shows for a target this
//! document numbered.
//!
//! No feature of the standard package declares a counter yet, so the tests
//! build a registry from that package plus a small feature written here. It
//! declares three counters and three counter styles into the two slots of the
//! `numbering` feature, as a peer, and one command per kind of reference
//! point:
//!
//! - `\tsec{title}` — a section-like point: counter `tsec`, identified by its
//!   title (`AnchorPolicy::Slug`), showing its title;
//! - `\tthm{title}` — a theorem-like point: counter `tthm`, numbered within
//!   `tsec`, identified by the number this document gave it
//!   (`AnchorPolicy::FromValue`), showing its title;
//! - `\teqn{text}` — an equation-like point: counter `teqn`, standing on its
//!   own, identified by its label or by its position
//!   (`AnchorPolicy::Auto`), showing nothing of its own;
//! - `\tnote{text}` — a point that can never be numbered, identified by its
//!   first label (`AnchorPolicy::FromFirstLabel`), showing its text;
//! - `\texp{text}` — a point that states its identifier outright
//!   (`AnchorPolicy::Explicit`), the one policy that computes nothing, which
//!   no standard element uses.
//!
//! Each takes `\label` fields after its argument, as a heading does, and
//! lowers to the content of that argument, so that a document of them reads as
//! ordinary text.

mod common;

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::error::Severity;

use flm_core::lang::text::plain_text_of;
use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuildError,
    RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ContentRef, Label, NumberSource, RefPoint, Referenceable,
};
use flm_core::model::addr::TreeId;
use flm_core::model::element::{ElementSpec, element_of};
use flm_core::model::names::{Name, RefKind};
use flm_core::stdcore::backends::lowering::lower_arg_named_or_nothing;
use flm_core::stdcore::backends::pres::{self, LinkPayload, payload_of};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::numbering::{
    COUNTERS_SLOT, CompiledNumberWriter, CounterDecl, CounterStyleDecl, Delimiters, FormatError,
    FormatOptions, JoinDecl, NUMBER_WRITERS_SLOT, NumberOut, NumberWriter, NumberWriterDecl,
    NumberedValue, NumberingConfig, PluralWords, PrefixWords, STYLES_SLOT, StyleError, Within,
    WriteCx, WriterCx,
};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::dump::diag::expect;
use flm_core::stdcore::features::refs::{
    self, ANCHORS, AnchorCollision, RefsConfig, UndeclaredVariant,
};
use flm_core::stdcore::features::{index, refpoints, refs as refs_feature};
use flm_core::stdcore::{StdcoreConfig, check_feature, package};

use common::stdcore::{identifiers, plan, registry_over};

// ---------------------------------------------------------------------------------------------
// the test feature
// ---------------------------------------------------------------------------------------------

/// Which anchor policy a construct's point declares.
#[derive(Clone, Copy)]
enum Anchor {
    /// The point's first surviving label, and no identifier without one.
    FirstLabel,
    /// The title, under a prefix.
    Slug(&'static str),
    /// The label, else the position among the points of the kind.
    Auto(&'static str),
    /// The number this document gave the point.
    Value(&'static str),
    /// This identifier, as it stands.
    Explicit(&'static str),
}

/// One construct the test feature declares: the command, the kind its points
/// carry, the counter and style it counts in (none for a point that can never
/// be numbered), its anchor policy, and whether its point shows the title.
#[derive(Clone)]
struct Construct {
    command: &'static str,
    number: Option<(&'static str, &'static str)>,
    anchor: Anchor,
    content: bool,
}

/// The name of the one argument every construct takes.
const TITLE: &str = "title";

/// The element value of a construct: it answers the one reference point the
/// node carries.
#[derive(Debug)]
struct Point {
    kind: RefKind,
    number: Option<NumberSource>,
    anchor: AnchorPolicy,
    content: Option<ContentRef>,
}

impl Referenceable for Point {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let (labels, malformed) = refs::point_labels(&refs::label_uses(node));
        vec![RefPoint {
            kind: self.kind.clone(),
            labels,
            malformed,
            computed: 0,
            anchor: self.anchor.clone(),
            number: self.number.clone(),
            content: self.content.clone(),
            accepts: AcceptedRefTypes::Any,
        }]
    }
}

/// The lowering rule of every construct: it emits the content of the
/// construct's one argument, so that a document of them reads as text.
#[derive(Debug)]
struct ShowTitle;

impl Lower for ShowTitle {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(lower_arg_named_or_nothing(cx, lw, &node, TITLE)?))
    }
}

/// The test feature: three counters, three styles, and one command per
/// construct.
struct TestFeature {
    constructs: Vec<Construct>,
}

impl FlmFeature for TestFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.refnum", "Test numbered reference points")
            .docs("Counters, styles and commands written for the reference tests.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        for counter in counters() {
            b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter))?;
        }
        for style in styles() {
            b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style))?;
        }
        b.peer_slot::<NumberWriterDecl>(
            NUMBER_WRITERS_SLOT,
            Arc::new(NumberWriterDecl::new(Arc::new(Marked))),
        )?;
        for construct in &self.constructs {
            let name = format!("t.refnum.{}", construct.command);
            let value = Arc::new(Point {
                kind: RefKind::from(construct.command),
                number: construct.number.map(|(counter, style)| NumberSource {
                    counter: Name::new(counter),
                    instances: Vec::new(),
                    style: Name::new(style),
                    advance: vec![0],
                    number_format: None,
                }),
                anchor: match construct.anchor {
                    Anchor::FirstLabel => AnchorPolicy::FromFirstLabel,
                    Anchor::Slug(prefix) => AnchorPolicy::Slug {
                        prefix: prefix.to_string(),
                        content: ContentRef::ArgumentNamed(TITLE.to_string()),
                    },
                    Anchor::Auto(prefix) => AnchorPolicy::Auto { prefix: prefix.to_string() },
                    Anchor::Value(prefix) => AnchorPolicy::FromValue { prefix: prefix.to_string() },
                    Anchor::Explicit(id) => AnchorPolicy::Explicit(id.to_string()),
                },
                content: construct
                    .content
                    .then(|| ContentRef::ArgumentNamed(TITLE.to_string())),
            });
            let element = Arc::new(
                ElementSpec::new(name.clone())
                    .docs("A test reference point.")
                    .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                    .with_value(value),
            );
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [construct.command],
                    SyntaxDef::Macro {
                        args: vec![ArgDef::mandatory(TITLE), refs::label_fields_arg()],
                    },
                    element,
                )
                .docs("A test reference point."),
            )?;
            b.lowering_rule(
                LoweringRule::new(
                    format!("{name}.lower"),
                    name,
                    LoweringDecl::new(),
                    Arc::new(ShowTitle),
                    DefaultMark::Everywhere,
                )
                .docs("shows the construct's own text"),
            )?;
        }
        Ok(())
    }
}

/// The counters the test feature declares: `tsec` on its own, `tthm` numbered
/// within it, and `teqn` and `teqw` on their own.
fn counters() -> Vec<CounterDecl> {
    vec![
        CounterDecl::new("tsec", "t.style.plain"),
        CounterDecl::new("tthm", "t.style.thm").within(Within::parent("tsec")),
        CounterDecl::new("teqn", "t.style.eqn"),
        CounterDecl::new("teqw", "t.style.eqw"),
    ]
}

/// A number writer written here and registered under `t.refnum.marked`: it
/// writes `#` and the number, and several numbers separated by `, `. No
/// counter style can write that, so a reference showing it can only have gone
/// through this writer.
#[derive(Debug)]
struct Marked;

impl NumberWriter for Marked {
    fn name(&self) -> &Name {
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(|| Name::new("t.refnum.marked"))
    }

    fn compile(&self, _cx: &WriterCx<'_>) -> Result<Arc<dyn CompiledNumberWriter>, StyleError> {
        Ok(Arc::new(Marked))
    }
}

impl CompiledNumberWriter for Marked {
    fn write_one<'a>(
        &self,
        value: &NumberedValue<'a>,
        _options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        out.text("#", Some(value.index()));
        out.content(cx.number(value.value())?, Some(value.index()));
        Ok(())
    }

    fn write_many<'a>(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        for (place, value) in values.iter().enumerate() {
            if place > 0 {
                out.text(", ", None);
            }
            self.write_one(value, options, cx, out)?;
        }
        Ok(())
    }
}

/// The counter styles: the plain number, the theorem style with a word and a
/// variant, and the equation style with delimiters and a word but no variant,
/// so that a reference asking for one falls back to the default words.
fn styles() -> Vec<CounterStyleDecl> {
    let plain = CounterStyleDecl::new("t.style.plain");
    let mut theorem = CounterStyleDecl::new("t.style.thm");
    theorem.prefix_words = PrefixWords::new("Thm.~", "Thms.~")
        .with_variant("capital", PluralWords::new("Theorem~", "Theorems~"));
    let mut equation = CounterStyleDecl::new("t.style.eqn");
    equation.delimiters = Delimiters::new("(", ")");
    equation.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
    equation.join = JoinDecl::compact();
    let mut written = CounterStyleDecl::new("t.style.eqw");
    written.delimiters = Delimiters::new("(", ")");
    written.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
    vec![plain, theorem, equation, written.with_number_writer("t.refnum.marked")]
}

/// The identifier the construct `\texp` states outright: one text, so that
/// two such points in one document collide.
const EXPLICIT_ID: &str = "thing-1";

/// The six constructs.
fn constructs() -> Vec<Construct> {
    vec![
        Construct {
            command: "tsec",
            number: Some(("tsec", "t.style.plain")),
            anchor: Anchor::Slug("s"),
            content: true,
        },
        Construct {
            command: "tthm",
            number: Some(("tthm", "t.style.thm")),
            anchor: Anchor::Value("thm"),
            content: true,
        },
        Construct {
            command: "teqn",
            number: Some(("teqn", "t.style.eqn")),
            anchor: Anchor::Auto("eqn"),
            content: false,
        },
        Construct {
            command: "teqw",
            number: Some(("teqw", "t.style.eqw")),
            anchor: Anchor::Auto("eqw"),
            content: false,
        },
        Construct {
            command: "tnote",
            number: None,
            anchor: Anchor::FirstLabel,
            content: true,
        },
        Construct {
            command: "texp",
            number: None,
            anchor: Anchor::Explicit(EXPLICIT_ID),
            content: true,
        },
    ]
}

/// The test feature.
fn test_feature() -> Arc<dyn FlmFeature> {
    Arc::new(TestFeature { constructs: constructs() })
}

/// The registry: the `stdcore` package, which holds the `numbering` feature,
/// plus the test feature.
fn registry() -> Arc<FlmRegistry> {
    registry_of(constructs())
}

/// The registry of [`registry`] in which the constructs `commands` state no
/// number source: their points count in nothing and are never numbered.
fn registry_numbering_none_of(commands: &[&str]) -> Arc<FlmRegistry> {
    let constructs = constructs()
        .into_iter()
        .map(|construct| Construct {
            number: if commands.contains(&construct.command) { None } else { construct.number },
            ..construct
        })
        .collect();
    registry_of(constructs)
}

/// The `stdcore` package plus the test feature over `constructs`.
fn registry_of(constructs: Vec<Construct>) -> Arc<FlmRegistry> {
    try_registry(StdcoreConfig::new(), constructs).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The `stdcore` package under `config` plus the test feature over
/// `constructs`.
fn try_registry(
    config: StdcoreConfig,
    constructs: Vec<Construct>,
) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(TestFeature { constructs }));
    builder.build(&RegistryInputs::new())
}

/// A registry holding no `numbering` feature at all: the lowering machinery,
/// the text backend, the element index, the `refs` feature and the test
/// feature, whose counter and style declarations are dropped as unresolved
/// peer contributions.
fn registry_without_numbering() -> Arc<FlmRegistry> {
    registry_over(vec![
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()),
        check_feature(),
        index::feature(),
        refpoints::feature(),
        refs_feature::feature(),
        test_feature(),
    ])
}

// ---------------------------------------------------------------------------------------------
// running a document
// ---------------------------------------------------------------------------------------------

/// Renders `text` under the `text` workflow, the counters as declared and no
/// option set on the document — which is also the only way to render in a
/// registry that holds no numbering feature, since a document may set no
/// option no feature declares.
fn render(registry: &Arc<FlmRegistry>, text: &str) -> Run {
    run_doc(registry, common::pipeline::doc(registry, &[text]))
}

/// Runs `doc` under the `text` workflow.
fn run_doc(registry: &Arc<FlmRegistry>, doc: Arc<flm_core::model::doc::Doc>) -> Run {
    let plan = plan(registry, text::WORKFLOW, &Overrides::none());
    common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential())
}

/// The anchor identifier of every reference point of the run, in point order.
fn anchor_ids(run: &Run) -> Vec<Option<String>> {
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    anchors.ids.values().cloned().collect()
}

// ---------------------------------------------------------------------------------------------
// the identifiers of the four anchor policies
// ---------------------------------------------------------------------------------------------

/// One document exercising all four policies at once, with the numbering
/// feature there.
const ONE_OF_EACH: &str = "\\tsec{First}\n\n\\tthm{Big}\\label{thm:big}\n\n\
                           \\tthm{Small}\n\n\\teqn{x = 1}\\label{eq:x}\n\n\
                           \\teqn{y = 2}\n\n\\tnote{A note}\\label{note:n}\n\n\
                           \\tnote{Another}\n";

#[test]
fn every_anchor_policy_gives_the_identifier_its_own_rule_says() {
    let run = render(&registry(), ONE_OF_EACH);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [
            // The title, under the prefix the element states.
            Some("s--First".to_string()),
            // The number this document gave the point: the section's number
            // first, then its own. The label plays no part.
            Some("thm-1-1".to_string()),
            Some("thm-1-2".to_string()),
            // The label where there is one, the position otherwise.
            Some(refs::label_anchor(&Label::new("eq", "x"))),
            Some(refs::auto_anchor("eqn", 2)),
            // The first label, and nothing without one.
            Some(refs::label_anchor(&Label::new("note", "n"))),
            None,
        ]
    );
}

/// A point identified by its number that states no number source: it has no
/// number to be identified by, and falls back to its first surviving label, or
/// to no identifier at all.
#[test]
fn a_point_identified_by_its_number_takes_its_label_where_it_is_not_numbered() {
    let run = render(&registry_numbering_none_of(&["tthm"]), ONE_OF_EACH);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let ids = anchor_ids(&run);
    assert_eq!(ids[1], Some(refs::label_anchor(&Label::new("thm", "big"))));
    assert_eq!(ids[2], None);
    // The other policies are untouched by the point not being numbered.
    assert_eq!(ids[0], Some("s--First".to_string()));
}

/// The same document in a registry that holds no numbering feature: nothing
/// is numbered anywhere, so a point identified by its number falls back the
/// same way, and every other policy gives what it always gives.
#[test]
fn without_the_numbering_feature_every_policy_still_gives_an_identifier() {
    let run = render(&registry_without_numbering(), ONE_OF_EACH);
    // The counters and styles the test feature offers are dropped when the
    // registry is built, each slot with one Note; the run itself reports
    // nothing.
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [
            Some("s--First".to_string()),
            Some(refs::label_anchor(&Label::new("thm", "big"))),
            None,
            Some(refs::label_anchor(&Label::new("eq", "x"))),
            Some(refs::auto_anchor("eqn", 2)),
            Some(refs::label_anchor(&Label::new("note", "n"))),
            None,
        ]
    );
}

/// The number a point is identified by is written in decimal whatever the
/// style prints: the theorem style writes `Thm.~1.1`, and the identifier is
/// `thm-1-1`.
#[test]
fn the_identifier_is_the_number_in_decimal_whatever_the_style_writes() {
    let run = render(&registry(), "\\tsec{S}\n\n\\tthm{T}\n");
    assert_eq!(anchor_ids(&run), [Some("s--S".to_string()), Some("thm-1-1".to_string())]);
}

/// The fifth policy, which computes nothing: an element that already holds an
/// identifier states it outright, and the stage takes it as it stands. Two
/// points stating one identifier are an ordinary collision — the first keeps
/// it, the second is reported and gets none — and a reference resolves
/// through the label, not through the identifier, so the first point is still
/// reachable.
#[test]
fn an_identifier_the_element_states_outright_is_taken_as_it_stands() {
    let run = render(
        &registry(),
        "\\texp{One}\\label{note:one}\n\n\\texp{Two}\\label{note:two}\n\n\\ref{note:one}\n",
    );
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.anchor-collision").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let condition = diagnostic.data().downcast_ref::<AnchorCollision>().expect("typed");
    assert_eq!(condition.anchor, EXPLICIT_ID);
    assert_eq!(
        (condition.first.as_str(), condition.second.as_str()),
        ("note:one", "note:two")
    );
    assert_eq!(anchor_ids(&run), [Some(EXPLICIT_ID.to_string()), None]);
    // The reference shows the point's content, there being no number: the
    // policy chooses the identifier and nothing else.
    assert_eq!(last_line(&text_of(&run)), "One");
}


// ---------------------------------------------------------------------------------------------
// what a reference shows for a numbered target
// ---------------------------------------------------------------------------------------------

/// The text a run wrote.
fn text_of(run: &Run) -> String {
    common::lowering::text(run)
}

/// One section, one theorem inside it, three equation-like points and one
/// point that can never be numbered — the targets the reference tests point
/// at.
const TARGETS: &str = "\\tsec{First}\n\n\\tthm{Big}\\label{thm:big}\n\n\
                       \\teqn{x}\\label{eq:a}\n\n\\teqn{y}\\label{eq:b}\n\n\
                       \\teqn{z}\\label{eq:c}\n\n\\tnote{A note}\\label{note:n}\n\n";

/// The text of a document made of [`TARGETS`] and `body`, with the paragraph
/// of the targets themselves dropped: what the references alone wrote. The
/// run must report nothing.
fn shown(body: &str) -> String {
    let (run, text) = shown_with_diagnostics(body);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    text
}

/// The last non-empty line of `text`.
fn last_line(text: &str) -> String {
    text.lines().rfind(|line| !line.trim().is_empty()).unwrap_or("").to_string()
}

/// A reference with no option shows the word its target's counter style
/// declares, then the number, the ancestors' numbers first.
#[test]
fn a_reference_to_a_numbered_target_shows_the_word_and_the_number() {
    assert_eq!(shown("See \\ref{thm:big}."), "See Thm.\u{a0}1.1.");
}

/// `noprefix` drops the word; the delimiters, where the style has any, stay.
#[test]
fn the_noprefix_option_drops_the_word_before_the_number() {
    assert_eq!(shown("See \\ref[noprefix]{thm:big}."), "See 1.1.");
    assert_eq!(shown("See \\ref[noprefix]{eq:a}."), "See (1).");
}

/// `S` asks for the variant `capital` of the style's words.
#[test]
fn the_s_option_asks_for_the_capital_variant_of_the_word() {
    assert_eq!(shown("See \\ref[S]{thm:big}."), "See Theorem\u{a0}1.1.");
}

/// A document made of [`TARGETS`] and `body`, rendered: the run, whose
/// diagnostics a test reads, and the text the references alone wrote.
fn shown_with_diagnostics(body: &str) -> (Run, String) {
    let run = render(&registry(), &format!("{TARGETS}{body}\n"));
    let text = last_line(&text_of(&run));
    (run, text)
}

/// The variant and the style an `flm.refs.undeclared-variant` diagnostic names.
fn undeclared(diagnostic: &flm_core::lang::FlmDiagnostic) -> (String, String) {
    let condition = diagnostic.data().downcast_ref::<UndeclaredVariant>().expect("typed");
    (condition.variant.clone(), condition.style.clone())
}

/// `S` is a preference: on a target whose style declares no `capital`
/// variant, the style's usual words are written and nothing is reported.
#[test]
fn the_s_option_on_a_style_without_the_capital_variant_writes_the_usual_words() {
    assert_eq!(shown("See \\ref[S]{eq:a}."), "See Eq.\u{a0}(1).");
}

/// `capital` named outright is a demand: on a target whose style does not
/// declare it, it is an Error at the reference, naming the variant and the
/// style; the style's usual words are written all the same.
#[test]
fn the_capital_variant_named_on_a_style_without_it_is_an_error() {
    let (run, text) = shown_with_diagnostics("See \\ref[capital]{eq:a}.");
    assert_eq!(text, "See Eq.\u{a0}(1).");
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.undeclared-variant")
        .at("\\ref[capital]{eq:a}")
        .severity(Severity::Error)
        .once();
    assert_eq!(undeclared(diagnostic), ("capital".to_string(), "t.style.eqn".to_string()));
    assert_eq!(identifiers(&run), ["flm.refs.undeclared-variant"]);
}

/// Any other bare key names the variant of its own name, so a misspelled key
/// is a variant no style declares: an Error, and the usual words.
#[test]
fn a_misspelled_variant_is_an_error() {
    let (run, text) = shown_with_diagnostics("See \\ref[captial]{thm:big}.");
    assert_eq!(text, "See Thm.\u{a0}1.1.");
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.undeclared-variant")
        .at("\\ref[captial]{thm:big}")
        .severity(Severity::Error)
        .once();
    assert_eq!(undeclared(diagnostic), ("captial".to_string(), "t.style.thm".to_string()));
}

/// A reference naming a variant outright over targets of two styles, one of
/// which lacks it, is reported exactly once, naming that style; each style
/// writes what it has. The same list under `S` is reported nowhere.
#[test]
fn a_list_over_two_styles_reports_only_the_style_lacking_the_variant() {
    let (run, text) = shown_with_diagnostics("See \\ref[capital]{thm:big,eq:a,eq:b}.");
    assert_eq!(text, "See Theorem\u{a0}1.1 and Eqs.\u{a0}(1,2).");
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.undeclared-variant").once();
    assert_eq!(undeclared(diagnostic), ("capital".to_string(), "t.style.eqn".to_string()));
    assert_eq!(identifiers(&run), ["flm.refs.undeclared-variant"]);
    assert_eq!(
        shown("See \\ref[S]{thm:big,eq:a,eq:b}."),
        "See Theorem\u{a0}1.1 and Eqs.\u{a0}(1,2)."
    );
}

/// A target shown by its content has no counter style, so nothing is
/// checked for it.
#[test]
fn a_variant_asked_of_a_target_shown_by_its_content_is_not_checked() {
    assert_eq!(shown("See \\ref[S]{note:n}."), "See A note.");
    assert_eq!(shown("See \\ref[weird]{note:n}."), "See A note.");
}

/// `\eqref` asks for no variant, and the equation style of the mathematics
/// feature declares `capital`: neither is reported.
#[test]
fn eqref_and_the_capital_wording_of_a_shipped_equation_are_clean() {
    let body = "\\begin{equation}x\\label{eq:m}\\end{equation}\n\n\
                See \\eqref{eq:m} and \\ref[S]{eq:m}.";
    let (run, text) = shown_with_diagnostics(body);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text, "See (1) and Equation\u{a0}(1).");
}

/// Two targets of one style and one set of ancestors are written as one list,
/// under the plural word.
#[test]
fn a_pair_of_numbered_targets_is_written_as_one_list() {
    assert_eq!(shown("See \\ref{eq:a,eq:b}."), "See Eqs.\u{a0}(1,2).");
}

/// Three consecutive numbers collapse into a range.
#[test]
fn three_consecutive_numbered_targets_are_written_as_a_range() {
    assert_eq!(shown("See \\ref{eq:a,eq:b,eq:c}."), "See Eqs.\u{a0}(1–3).");
}

/// Targets shown in different styles are written in groups, one call to the
/// formatter each, in the order the styles were first met; the groups are the
/// items of one list, and two items are joined by ` and `.
#[test]
fn targets_of_two_styles_are_written_as_two_groups() {
    assert_eq!(shown("See \\ref{eq:a,thm:big}."), "See Eq.\u{a0}(1) and Thm.\u{a0}1.1.");
    // The order is the order the reference named them.
    assert_eq!(shown("See \\ref{thm:big,eq:a}."), "See Thm.\u{a0}1.1 and Eq.\u{a0}(1).");
}

/// Three items or more — here two groups of numbers and a target with no
/// number — are joined by `, ` and, before the last, `, and `.
#[test]
fn three_items_mixing_two_styles_and_an_unnumbered_target_read_as_a_list() {
    assert_eq!(
        shown("See \\ref{eq:a,thm:big,note:n}."),
        "See Eq.\u{a0}(1), Thm.\u{a0}1.1, and A note."
    );
}

/// A group of several numbers of one style is one item of the list, written
/// with that style's own joining texts inside it: the equation style here
/// joins its numbers compactly, so the group reads `Eqs. (1,2)`, and the list
/// then joins that item and the theorem with ` and `.
#[test]
fn a_group_of_two_numbers_is_one_item_of_the_list() {
    assert_eq!(
        shown("See \\ref{eq:a,eq:b,thm:big}."),
        "See Eqs.\u{a0}(1,2) and Thm.\u{a0}1.1."
    );
}

/// A target this document did not number comes after the numbers, showing its
/// own content, whatever place the reference gave it.
#[test]
fn an_unnumbered_target_comes_after_the_numbers_showing_its_content() {
    assert_eq!(shown("See \\ref{note:n,eq:a}."), "See Eq.\u{a0}(1) and A note.");
}

/// A target nothing resolves comes after the numbers too, showing the label
/// as written; it is reported as any unresolved reference is.
#[test]
fn an_unresolved_target_keeps_its_own_look_after_the_numbers() {
    let document = format!("{TARGETS}See \\ref{{eq:a,sec:nope}}.\n");
    let run = render(&registry(), &document);
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
    assert_eq!(last_line(&text_of(&run)), "See Eq.\u{a0}(1) and sec:nope.");
}

/// The options apply to every target of the reference alike, which is what
/// makes `\ref[noprefix]{a,b}` read as one list of bare numbers.
#[test]
fn the_display_options_apply_to_every_target_of_the_reference() {
    assert_eq!(shown("See \\ref[noprefix]{eq:a,eq:b}."), "See (1,2).");
    assert_eq!(shown("See \\ref[noprefix]{thm:big,eq:a}."), "See 1.1 and (1).");
}

/// A target this document numbered shows its number, not its content: the
/// theorem's title is `Big`, and a reference to it shows `Thm. 1.1`. Where it
/// states no number source it shows the content instead, and a point with no
/// content at all shows the label the reference was written with.
#[test]
fn the_number_comes_before_the_content_and_the_content_before_the_label() {
    let document = format!("{TARGETS}See \\ref{{thm:big}} and \\ref{{eq:a}}.\n");
    let numbered = render(&registry(), &document);
    assert_eq!(last_line(&text_of(&numbered)), "See Thm.\u{a0}1.1 and Eq.\u{a0}(1).");
    let unnumbered = render(&registry_numbering_none_of(&["tthm", "teqn"]), &document);
    assert_eq!(last_line(&text_of(&unnumbered)), "See Big and eq:a.");
}

/// In a registry holding no numbering feature at all, the same references
/// show the target's content where it has any — the theorem's title — and
/// otherwise the label as written, each such target reported as having
/// nothing to show. The rule's read of the counter table answers nothing, and
/// the two targets of one reference are still one list, joined by the same
/// texts.
#[test]
fn without_the_numbering_feature_a_reference_shows_the_content_or_the_label() {
    let document = format!("{TARGETS}See \\ref{{thm:big}} and \\ref{{eq:a,eq:b}}.\n");
    let run = render(&registry_without_numbering(), &document);
    assert_eq!(identifiers(&run), ["flm.refs.nothing-to-show", "flm.refs.nothing-to-show"]);
    assert_eq!(last_line(&text_of(&run)), "See Big and eq:a and eq:b.");
}

/// A `\ref` naming a text that is not spelled `type:label` among several
/// targets is reported on its own and shown as written, after the numbers.
#[test]
fn a_target_that_is_no_label_is_reported_and_shown_as_written() {
    let document = format!("{TARGETS}See \\ref{{eq:a,nolabel}}.\n");
    let run = render(&registry(), &document);
    assert_eq!(identifiers(&run), ["flm.refs.bad-label"]);
    assert_eq!(last_line(&text_of(&run)), "See Eq.\u{a0}(1) and nolabel.");
}

/// Every link the run's first lowered tree holds, in document order: the link
/// target and the plain text inside it.
fn links(run: &Run) -> Vec<(String, String)> {
    let lowered = common::lowering::lowered(run, TreeId::tree(0));
    lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::LINK)
        .map(|node| {
            let payload = payload_of::<LinkPayload>(&node).expect("a link payload");
            (payload.href.clone(), plain_text_of(std::iter::once(node)))
        })
        .collect()
}

/// With one target the whole text is one link, the word included.
#[test]
fn one_numbered_target_is_one_link_around_the_whole_text() {
    let run = render(&registry(), &format!("{TARGETS}See \\ref{{thm:big}}.\n"));
    assert_eq!(links(&run), [("#thm-1-1".to_string(), "Thm.\u{a0}1.1".to_string())]);
}

/// With several, each number links to its own target and the word links to
/// the first of them, because the styles here keep the word inside the link.
/// The delimiters and the text between two numbers link to nothing, so `(1,2)`
/// is two links with the punctuation outside them.
#[test]
fn each_number_of_several_links_to_its_own_target() {
    let run = render(&registry(), &format!("{TARGETS}See \\ref{{eq:a,eq:b}}.\n"));
    let first = format!("#{}", refs::label_anchor(&Label::new("eq", "a")));
    let second = format!("#{}", refs::label_anchor(&Label::new("eq", "b")));
    assert_eq!(
        links(&run),
        [
            (first.clone(), "Eqs.\u{a0}".to_string()),
            (first, "1".to_string()),
            (second, "2".to_string()),
        ]
    );
}

/// A target with no number of its own is a link like any other, around what
/// it shows.
#[test]
fn an_unnumbered_target_among_several_is_a_link_around_its_content() {
    let run = render(&registry(), &format!("{TARGETS}See \\ref{{eq:a,note:n}}.\n"));
    assert_eq!(
        links(&run),
        [
            (format!("#{}", refs::label_anchor(&Label::new("eq", "a"))), "Eq.\u{a0}(1)".to_string()),
            (
                format!("#{}", refs::label_anchor(&Label::new("note", "n"))),
                "A note".to_string()
            ),
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// a reference inside a value
// ---------------------------------------------------------------------------------------------

/// A `\ref` written inside a heading's title, and a `\ref` to that heading.
///
/// The title is copied into a value when the reference points are scanned, and
/// a reference to the heading splices that value: the `\ref` inside it is
/// lowered a second time, over the value alone. The counter table is of
/// document scope and is computed once, so the reference inside the value must
/// still find the number of the point it names.
const REF_IN_A_TITLE: &str = "\\tsec{First}\n\n\\tthm{Big}\\label{thm:big}\n\n\
                              \\section{About \\ref{thm:big}}\\label{sec:h}\n\n\
                              Then \\ref{sec:h}.\n";

#[test]
fn a_reference_inside_a_heading_title_shows_the_number() {
    let run = render(&registry(), REF_IN_A_TITLE);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("About Thm.\u{a0}1.1\n"), "{text}");
}

#[test]
fn a_reference_inside_a_spliced_value_shows_the_number_too() {
    let run = render(&registry(), REF_IN_A_TITLE);
    assert_eq!(last_line(&text_of(&run)), "Then About Thm.\u{a0}1.1.");
}

/// A style may be written by a number writer of its own, and the references
/// feature needs to know nothing about it: it asks the counter table for the
/// numbers of the targets, as it does for every other style, and shows what
/// comes back.
#[test]
fn a_reference_to_targets_of_a_style_with_a_number_writer_shows_what_it_wrote() {
    let document = "\\teqw{x}\\label{eqw:a}\n\n\\teqw{y}\\label{eqw:b}\n\n\
                    See \\ref{eqw:a,eqw:b}.\n";
    let run = render(&registry(), document);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(last_line(&text_of(&run)), "See #1, #2.");
}

// ---------------------------------------------------------------------------------------------
// the texts that join the items of a reference
// ---------------------------------------------------------------------------------------------

/// The default joining texts with the word `und` in place of `and`.
fn und() -> JoinDecl {
    JoinDecl { and: Some("und".to_string()), ..JoinDecl::default() }
}

/// The package configuration that writes `und` in both places a host writes
/// the word: in the equation style's own joining texts, through the numbering
/// feature's configuration, and in the texts of a reference list, through the
/// references feature's.
fn und_everywhere() -> StdcoreConfig {
    let mut equation = styles()
        .into_iter()
        .find(|style| style.name.as_str() == "t.style.eqn")
        .expect("the test feature declares the equation style");
    equation.join = und();
    StdcoreConfig::new()
        .numbering(NumberingConfig::new().style(equation))
        .refs(RefsConfig::new().join(und()))
}

/// One [`JoinDecl`], written once for a counter style and once for the
/// reference list, changes the word in both: inside the group of equations
/// and between the items of the list.
#[test]
fn one_join_declaration_changes_the_word_in_a_style_and_in_the_list() {
    let registry = try_registry(und_everywhere(), constructs()).expect("the registry builds");
    let run = render(&registry, &format!("{TARGETS}See \\ref{{eq:a,eq:b,thm:big}}.\n"));
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        last_line(&text_of(&run)),
        "See Eqs.\u{a0}(1 und 2) und Thm.\u{a0}1.1."
    );
    let run = render(&registry, &format!("{TARGETS}See \\ref{{eq:a,thm:big,note:n}}.\n"));
    assert_eq!(last_line(&text_of(&run)), "See Eq.\u{a0}(1), Thm.\u{a0}1.1, und A note.");
}

/// A joining text of the list that is not valid FLM fails the registry build,
/// as a counter style's text does.
#[test]
fn a_joining_text_that_does_not_parse_fails_the_registry_build() {
    let join = JoinDecl { pair_mid: Some("{".to_string()), ..JoinDecl::default() };
    let config = StdcoreConfig::new().refs(RefsConfig::new().join(join));
    let message = match try_registry(config, constructs()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    };
    assert!(message.contains("a text that joins the targets of a reference"), "{message}");
}

/// The configuration enters the registry digest: two registries that join a
/// reference list differently are told apart.
#[test]
fn the_registry_digest_changes_with_the_joining_texts() {
    let plain = registry();
    let config = StdcoreConfig::new().refs(RefsConfig::new().join(und()));
    let other = try_registry(config, constructs()).expect("the registry builds");
    assert_ne!(plain.digest(), other.digest());
    assert_eq!(plain.digest(), registry().digest());
    assert_ne!(RefsConfig::new().digest_bytes(), RefsConfig::new().join(und()).digest_bytes());
}

/// A counter style whose `ranges` is off writes consecutive numbers one by
/// one, sorted, each a link of its own; the same reference under the style as
/// the test feature declares it writes a range.
#[test]
fn a_style_without_ranges_writes_every_number_of_a_reference_alone() {
    let body = "See \\ref{eq:c,eq:a,eq:b}.";
    let run = render(&registry(), &format!("{TARGETS}{body}\n"));
    assert_eq!(last_line(&text_of(&run)), "See Eqs.\u{a0}(1–3).");

    let mut equation = styles()
        .into_iter()
        .find(|style| style.name.as_str() == "t.style.eqn")
        .expect("the test feature declares the equation style");
    equation.ranges = false;
    let config = StdcoreConfig::new().numbering(NumberingConfig::new().style(equation));
    let registry = try_registry(config, constructs()).expect("the registry builds");
    let run = render(&registry, &format!("{TARGETS}{body}\n"));
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(last_line(&text_of(&run)), "See Eqs.\u{a0}(1,2,3).");
    let linked: Vec<String> = links(&run).into_iter().map(|(_, text)| text).collect();
    assert_eq!(linked, ["Eqs.\u{a0}", "1", "2", "3"]);
}

/// Each item of the list links to its own target and the joining texts link
/// to nothing: `Eq. (1)`, then ` and `, then the theorem.
#[test]
fn the_joining_texts_link_to_nothing() {
    let run = render(&registry(), &format!("{TARGETS}See \\ref{{eq:a,thm:big}}.\n"));
    let linked: Vec<String> = links(&run).into_iter().map(|(_, text)| text).collect();
    assert_eq!(linked, ["Eq.\u{a0}(1)", "Thm.\u{a0}1.1"]);
    assert_eq!(last_line(&text_of(&run)), "See Eq.\u{a0}(1) and Thm.\u{a0}1.1.");
}

/// Two targets with no number are two items, joined as any two items are, in
/// the order the reference names them.
#[test]
fn two_unnumbered_targets_are_joined_by_and() {
    let document = format!("{TARGETS}See \\ref{{thm:big,note:n}}.\n");
    let run = render(&registry_numbering_none_of(&["tthm"]), &document);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(last_line(&text_of(&run)), "See Big and A note.");
}
