//! The `numbering` feature end to end: what a feature declares, what a
//! document's options change, and what the numbering stage records.
//!
//! The tests build a registry from the `stdcore` package plus a small feature
//! written here. That feature declares counters and counter styles into the
//! two slots, and a command per numbered construct: `\tsec`, `\tsub` and
//! `\tpara` for a hierarchy of three counters, `\teq` and `\teqalt` for two
//! kinds of point counting in one counter under different styles, `\teqsub`
//! and `\teqmore` for a sub-numbered group written as separate constructs, and
//! `\teqlines` for one construct carrying a whole sub-numbered group.
//!
//! Its check stage reads the counter table and joins `flm.validated`, so that
//! every workflow schedules the numbering stage; without a reader nothing
//! would need the table and the stage would not run.

mod common;

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::error::Severity;
use techy::serialize::SerialValue;

use flm_core::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, NumberSource, RefPoint, Referenceable,
};
use flm_core::model::names::{Name, RefKind};
use flm_core::model::options::OptionValues;
use flm_core::model::element::ElementSpec;
use flm_core::core::data::store::StageOut;
use flm_core::core::plan::{Overrides, PlanCx};
use flm_core::core::registry::{
    BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry,
    RegistryBuildError, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions, StageCx};
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::stdcore::backends::pres::read::recompose_source;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, Counters, Delimiters, FormatOptions, ManyNumbers, NumberSegment,
    Ancestor, CompiledWithinJoiner, CounterValue, FormatError, JoinCx, NumberTemplate,
    NumberingConfig, NumberingOptions, Parent, Part, PrefixWords, StyleError, SubLevelDecl,
    ParentSetDecl, Within, WithinCx, parts, PARENT_SETS_SLOT,
    WithinJoiner, WithinJoinerDecl, COUNTERS, COUNTERS_SLOT, STYLES_SLOT, WITHIN_JOINERS_SLOT,
    CompiledNumberWriter, NumberFormat, NumberOut, NumberWriter, NumberWriterDecl, NumberedValue,
    WriteCx, WriterCx, NUMBER_WRITERS_SLOT,
};
use flm_core::stdcore::{StdcoreConfig, CHECK_WORKFLOW, VALIDATED, package};

use common::stdcore::{identifiers, plan, registry as stdcore_registry, reports};

// ---------------------------------------------------------------------------------------------
// the test feature
// ---------------------------------------------------------------------------------------------

/// The capability the check stage of the test feature provides.
const SEEN: &str = "t.num.seen";

/// One numbered construct: the command that writes it, the counter it counts
/// in, the style it is shown in, the levels it advances, and how many points
/// one occurrence carries.
#[derive(Clone)]
struct Construct {
    command: &'static str,
    counter: &'static str,
    style: &'static str,
    advance: Vec<u32>,
    /// How many points one occurrence carries. Every point after the first
    /// advances the deepest level of `advance` alone, which is how the lines
    /// of one construct form a sub-numbered group.
    points: usize,
}

impl Construct {
    fn new(command: &'static str, counter: &'static str, style: &'static str) -> Construct {
        Construct { command, counter, style, advance: vec![0], points: 1 }
    }

    fn advancing(mut self, advance: &[u32]) -> Construct {
        self.advance = advance.to_vec();
        self
    }

    fn lines(mut self, points: usize) -> Construct {
        self.points = points;
        self
    }
}

/// The element value of one numbered construct: it answers the construct's
/// reference points and nothing else.
#[derive(Debug)]
struct Numbered {
    counter: Name,
    style: Name,
    advance: Vec<u32>,
    points: usize,
}

impl Referenceable for Numbered {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        (0..self.points)
            .map(|index| {
                let advance = if index == 0 {
                    self.advance.clone()
                } else {
                    vec![*self.advance.last().unwrap_or(&0)]
                };
                RefPoint {
                    kind: RefKind::from("t.numbered"),
                    labels: Vec::new(),
                    malformed: Vec::new(),
                    computed: 0,
                    anchor: AnchorPolicy::Auto { prefix: "t".to_string() },
                    number: Some(NumberSource {
                        counter: self.counter.clone(),
                        instances: Vec::new(),
                        style: self.style.clone(),
                        advance,
                        number_format: None,
                    }),
                    content: None,
                    accepts: AcceptedRefTypes::Any,
                }
            })
            .collect()
    }
}

use flm_core::lang::Flm;

/// The test feature: it declares counters and styles as a peer, one command
/// per construct, and a check stage reading the counter table.
struct TestFeature {
    counters: Vec<CounterDecl>,
    styles: Vec<CounterStyleDecl>,
    constructs: Vec<Construct>,
}

impl FlmFeature for TestFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.num", "Test numbered constructs")
            .docs("Counters, styles and commands written for the numbering tests.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        for counter in &self.counters {
            b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter.clone()))?;
        }
        for style in &self.styles {
            b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style.clone()))?;
        }
        for construct in &self.constructs {
            let value = Arc::new(Numbered {
                counter: Name::new(construct.counter),
                style: Name::new(construct.style),
                advance: construct.advance.clone(),
                points: construct.points,
            });
            let element = Arc::new(
                ElementSpec::new(format!("t.num.{}", construct.command))
                    .docs(format!("`\\{}`: a numbered test construct.", construct.command))
                    .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                    .with_value(value),
            );
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [construct.command],
                    SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                    element,
                )
                .docs("A numbered test construct."),
            )?;
        }
        b.peer_slot::<WithinJoinerDecl>(
            WITHIN_JOINERS_SLOT,
            Arc::new(WithinJoinerDecl::new(Arc::new(PathJoiner))),
        )?;
        b.peer_slot::<WithinJoinerDecl>(
            WITHIN_JOINERS_SLOT,
            Arc::new(WithinJoinerDecl::new(Arc::new(NeverJoiner))),
        )?;
        for writer in test_writers() {
            b.peer_slot::<NumberWriterDecl>(
                NUMBER_WRITERS_SLOT,
                Arc::new(NumberWriterDecl::new(writer)),
            )?;
        }
        b.capability(CapabilityDef::new(SEEN).docs("The counter table has been read."))?;
        b.collection_member(VALIDATED, SEEN)?;
        b.stage(Arc::new(SeeCounters))?;
        Ok(())
    }
}

/// A within-joiner written here and registered under the name
/// `t.num.path`: it writes every counter of the chain as its name and its
/// number, the counters separated by `:` and the whole ended by `/` —
/// `tsec1:tsub1/`. No built-in form writes that, so a test can see that the
/// joiner and not a template wrote the number.
#[derive(Debug)]
struct PathJoiner;

/// The name `t.num.path` is registered under.
fn path_joiner_name() -> Name {
    Name::new("t.num.path")
}

impl WithinJoiner for PathJoiner {
    fn name(&self) -> &Name {
        // A joiner's name is a constant of the feature that declares it; this
        // one is made once and lent out.
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(path_joiner_name)
    }

    fn compile(&self, _cx: &WithinCx<'_>) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError> {
        Ok(Arc::new(PathJoiner))
    }
}

impl CompiledWithinJoiner for PathJoiner {
    fn counters_written(&self, chain: &[Name]) -> Vec<Name> {
        chain.to_vec()
    }

    fn parts(&self, ancestors: &[Ancestor], cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError> {
        let mut written = Vec::with_capacity(ancestors.len());
        for (place, ancestor) in ancestors.iter().enumerate() {
            let after = if place + 1 == ancestors.len() { "/" } else { ":" };
            let content = cx.joined(&[
                cx.literal(ancestor.counter.as_str())?,
                cx.number_of(ancestor)?,
                cx.literal(after)?,
            ])?;
            written.push(Part::new(
                vec![(ancestor.counter.clone(), ancestor.value.clone())],
                content,
            ));
        }
        Ok(written)
    }
}

/// A within-joiner registered under `t.num.never`: it writes nothing of the
/// ancestors, and asking it for the parts fails the test outright. A counter
/// numbered within another by this joiner, shown in a style whose number
/// writer arranges the ancestors itself, proves that the joiner is not run
/// when the writer does not ask for its parts.
#[derive(Debug)]
struct NeverJoiner;

impl WithinJoiner for NeverJoiner {
    fn name(&self) -> &Name {
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(|| Name::new("t.num.never"))
    }

    fn compile(&self, _cx: &WithinCx<'_>) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError> {
        Ok(Arc::new(NeverJoiner))
    }
}

impl CompiledWithinJoiner for NeverJoiner {
    fn counters_written(&self, _chain: &[Name]) -> Vec<Name> {
        Vec::new()
    }

    fn parts(&self, _ancestors: &[Ancestor], _cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError> {
        panic!("the number writer asked for the parts, which this test says it never does");
    }
}

/// The three number writers the tests register.
fn test_writers() -> Vec<Arc<dyn NumberWriter>> {
    vec![Arc::new(InSection), Arc::new(Spanning), Arc::new(Bare)]
}

/// A number writer registered under `t.num.in-section`: it writes what the
/// default writer writes and then, for a value numbered within the counter
/// `twsec`, ` in Section~` and that section's number in capital roman
/// numerals — `Eq.~(5) in Section~V`.
///
/// Its own text is FLM source and is parsed once, when the writer is compiled.
#[derive(Debug)]
struct InSection;

/// The compiled form of [`InSection`]: its text, parsed.
#[derive(Debug)]
struct InSectionWriter {
    text: Arc<techy::core::node::NodeTree<Flm>>,
}

impl NumberWriter for InSection {
    fn name(&self) -> &Name {
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(|| Name::new("t.num.in-section"))
    }

    fn compile(&self, cx: &WriterCx<'_>) -> Result<Arc<dyn CompiledNumberWriter>, StyleError> {
        Ok(Arc::new(InSectionWriter { text: cx.parse(" in Section~")? }))
    }
}

impl CompiledNumberWriter for InSectionWriter {
    fn write_one<'a>(
        &self,
        value: &NumberedValue<'a>,
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        cx.default_one(value, options, out)?;
        let section = value.ancestors().iter().find(|a| a.counter.as_str() == "twsec");
        if let Some(section) = section {
            out.content(Arc::clone(&self.text), None);
            out.content(cx.number_of_in(section, NumberFormat::CapitalRoman)?, None);
        }
        Ok(())
    }

    fn write_many<'a>(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        cx.default_many(values, options, out)
    }
}

/// A number writer registered under `t.num.span`: with exactly two values it
/// writes one range across them, however far apart the counters they are
/// numbered within stand — `Eqs.~(V.3.1–4.2)` — by leaving out of the range's
/// end the leading parts the two values share. With any other number of values
/// it writes what the default writer writes.
#[derive(Debug)]
struct Spanning;

impl NumberWriter for Spanning {
    fn name(&self) -> &Name {
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(|| Name::new("t.num.span"))
    }

    fn compile(&self, _cx: &WriterCx<'_>) -> Result<Arc<dyn CompiledNumberWriter>, StyleError> {
        Ok(Arc::new(Spanning))
    }
}

impl CompiledNumberWriter for Spanning {
    fn write_one<'a>(
        &self,
        value: &NumberedValue<'a>,
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        cx.default_one(value, options, out)
    }

    fn write_many<'a>(
        &self,
        values: &[NumberedValue<'a>],
        options: &FormatOptions,
        cx: &WriteCx<'a>,
        out: &mut NumberOut<'a>,
    ) -> Result<(), FormatError> {
        let [first, last] = values else {
            return cx.default_many(values, options, out);
        };
        let opening = cx.parts(first)?;
        let closing = cx.parts(last)?;
        // The parts the two ends share, compared by the ancestors they stand
        // for: they are written once, before the range.
        let shared = opening
            .iter()
            .zip(&closing)
            .take_while(|(one, other)| one.stands_for == other.stands_for)
            .count();
        if options.with_word {
            if let Some(word) = cx.word(2, options.variant.as_deref()) {
                // The style says whether the word belongs to the link to the
                // first value.
                let mark = cx.name_in_link().then(|| first.index());
                out.content(Arc::clone(word), mark);
            }
        }
        if options.with_delimiters {
            out.content(Arc::clone(cx.open()), None);
        }
        for part in &opening {
            out.content(Arc::clone(&part.content), Some(first.index()));
        }
        out.content(cx.number(first.value())?, Some(first.index()));
        out.text("\u{2013}", None);
        for part in closing.iter().skip(shared) {
            out.content(Arc::clone(&part.content), Some(last.index()));
        }
        out.content(cx.number(last.value())?, Some(last.index()));
        if options.with_delimiters {
            out.content(Arc::clone(cx.close()), None);
        }
        Ok(())
    }
}

/// A number writer registered under `t.num.bare`: it writes `#` and the
/// number, and never asks for the parts of the counters the value is numbered
/// within, so the within-joiner is never run.
#[derive(Debug)]
struct Bare;

impl NumberWriter for Bare {
    fn name(&self) -> &Name {
        static NAME: std::sync::OnceLock<Name> = std::sync::OnceLock::new();
        NAME.get_or_init(|| Name::new("t.num.bare"))
    }

    fn compile(&self, _cx: &WriterCx<'_>) -> Result<Arc<dyn CompiledNumberWriter>, StyleError> {
        Ok(Arc::new(Bare))
    }
}

impl CompiledNumberWriter for Bare {
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

/// The check stage that reads the counter table, so that every workflow
/// schedules the numbering stage.
struct SeeCounters;

impl Stage for SeeCounters {
    fn decl(&self) -> StageDecl {
        StageDecl::new("t.num.see", Effect::Check, Scope::Document)
            .provides(SEEN)
            .docs("Reads the counter table, and reports nothing.")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(COUNTERS)
    }

    fn run(&self, cx: &StageCx<'_>, _out: &mut StageOut<'_>) {
        let _ = cx.get(COUNTERS);
    }
}

// ---------------------------------------------------------------------------------------------
// the declarations the tests share
// ---------------------------------------------------------------------------------------------

/// The plain style: the number in decimal digits, nothing around it.
fn plain(name: &str) -> CounterStyleDecl {
    CounterStyleDecl::new(name)
}

/// The equation style: `(3)`, with one sub-level written in letters and
/// nothing between the number and the letter, so that `(3, 2)` is `(3b)`.
fn equation_style(name: &str) -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(name);
    style.delimiters = Delimiters::new("(", ")");
    style.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
    style.sub_levels =
        vec![SubLevelDecl::new("", NumberTemplate::formats("#{alph}"))];
    style
}

/// The alternative equation style: the same value written in capital roman
/// numerals, so that a test can see which style a number was written in.
fn alternative_style(name: &str) -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(name);
    style.number_format = NumberTemplate::formats("#{Roman}");
    style.delimiters = Delimiters::new("[", "]");
    style.prefix_words = PrefixWords::new("Alt.~", "Alts.~");
    style
}

/// The roman style: the number in capital roman numerals, nothing around it,
/// so that a number written within one of its counters shows `V`.
fn roman_style(name: &str) -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(name);
    style.number_format = NumberTemplate::formats("#{Roman}");
    style
}

/// The counters and styles the tests share: a hierarchy `tsec` → `tsub` →
/// `tpara`, `teq`, standing on its own, and `tany`, numbered within whichever
/// of `tsec` and `tsub` was advanced last. The counters whose names begin
/// with `tw` are the ones shown in a style with a number writer of its own.
fn declarations() -> (Vec<CounterDecl>, Vec<CounterStyleDecl>) {
    let counters = vec![
        CounterDecl::new("tsec", "t.style.plain"),
        CounterDecl::new("tsub", "t.style.plain")
            .within(Within::parent("tsec")),
        CounterDecl::new("tpara", "t.style.plain")
            .within(Within::parent("tsub")),
        CounterDecl::new("tpath", "t.style.plain")
            .within(Within::parent("tsub").joiner_named("t.num.path")),
        CounterDecl::new("teq", "t.style.eq"),
        CounterDecl::new("tany", "t.style.plain").within(Within::parents(["tsec", "tsub"])),
        CounterDecl::new("twsec", "t.style.plain"),
        CounterDecl::new("tweq", "t.style.insection")
            .within(Within::parent("twsec").displayed_bare()),
        CounterDecl::new("twrsec", "t.style.roman"),
        CounterDecl::new("twrsub", "t.style.plain")
            .within(Within::parent("twrsec")),
        CounterDecl::new("twspan", "t.style.span")
            .within(Within::parent("twrsub")),
        CounterDecl::new("twbare", "t.style.bare")
            .within(Within::parent("twrsub").joiner_named("t.num.never")),
    ];
    let styles = vec![
        plain("t.style.plain"),
        equation_style("t.style.eq"),
        alternative_style("t.style.alt"),
        roman_style("t.style.roman"),
        equation_style("t.style.insection").with_number_writer("t.num.in-section"),
        equation_style("t.style.span").with_number_writer("t.num.span"),
        plain("t.style.bare").with_number_writer("t.num.bare"),
    ];
    (counters, styles)
}

/// The commands the tests write.
fn constructs() -> Vec<Construct> {
    vec![
        Construct::new("tsec", "tsec", "t.style.plain"),
        Construct::new("tsub", "tsub", "t.style.plain"),
        Construct::new("tpara", "tpara", "t.style.plain"),
        Construct::new("tpath", "tpath", "t.style.plain"),
        Construct::new("teq", "teq", "t.style.eq"),
        Construct::new("tany", "tany", "t.style.plain"),
        Construct::new("teqalt", "teq", "t.style.alt"),
        Construct::new("teqsub", "teq", "t.style.eq").advancing(&[0, 1]),
        Construct::new("teqmore", "teq", "t.style.eq").advancing(&[1]),
        Construct::new("teqlines", "teq", "t.style.eq").advancing(&[0, 1]).lines(3),
        Construct::new("teqaltmore", "teq", "t.style.alt").advancing(&[1]),
        Construct::new("teqaltsame", "teq", "t.style.alt").advancing(&[]),
        Construct::new("twsec", "twsec", "t.style.plain"),
        Construct::new("tweq", "tweq", "t.style.insection"),
        Construct::new("twrsec", "twrsec", "t.style.roman"),
        Construct::new("twrsub", "twrsub", "t.style.plain"),
        Construct::new("twspan", "twspan", "t.style.span"),
        Construct::new("twbare", "twbare", "t.style.bare"),
    ]
}

/// The test feature over the shared declarations.
fn test_feature() -> Arc<dyn FlmFeature> {
    let (counters, styles) = declarations();
    Arc::new(TestFeature { counters, styles, constructs: constructs() })
}

/// The registry: the `stdcore` package under `config`, plus `feature`.
fn try_registry(
    config: StdcoreConfig,
    feature: Arc<dyn FlmFeature>,
) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(feature);
    builder.build(&RegistryInputs::new())
}

/// [`try_registry`], a build failure being a test failure.
fn registry_with(config: StdcoreConfig, feature: Arc<dyn FlmFeature>) -> Arc<FlmRegistry> {
    try_registry(config, feature).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The registry the tests share.
fn registry() -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new(), test_feature())
}

/// What the registry build reported, as a rendered text.
fn build_failure(counters: Vec<CounterDecl>, styles: Vec<CounterStyleDecl>) -> String {
    build_failure_under(StdcoreConfig::new(), counters, styles)
}

/// [`build_failure`] with a configuration of the package.
fn build_failure_under(
    config: StdcoreConfig,
    counters: Vec<CounterDecl>,
    styles: Vec<CounterStyleDecl>,
) -> String {
    let feature = Arc::new(TestFeature { counters, styles, constructs: Vec::new() });
    match try_registry(config, feature) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

// ---------------------------------------------------------------------------------------------
// running a document
// ---------------------------------------------------------------------------------------------

/// Runs the `check` workflow over `text` with `options` bound on the document
/// and `render` bound on the request.
fn run_with(
    registry: &Arc<FlmRegistry>,
    text: &str,
    options: NumberingOptions,
    render: Option<NumberingOptions>,
    run_options: &RunOptions,
) -> Run {
    let doc = common::pipeline::doc_with_options(
        registry,
        &[text],
        OptionValues::new().with(options),
    );
    let overrides = match render {
        Some(render) => Overrides {
            render_options: OptionValues::new().with(render),
            ..Overrides::none()
        },
        None => Overrides::none(),
    };
    let plan = plan(registry, CHECK_WORKFLOW, &overrides);
    common::pipeline::run_with(&doc, &plan, Externals::none(), run_options)
}

/// Runs the `check` workflow over `text` with `options` bound on the document.
fn run(registry: &Arc<FlmRegistry>, text: &str, options: NumberingOptions) -> Run {
    run_with(registry, text, options, None, &RunOptions::sequential())
}

/// Renders `text` under the `text` workflow, with `options` bound on the
/// document.
///
/// The test constructs carry no lowering rule, so the commands named in
/// `commands` are given to the plan as accepted holes: the backend writes a
/// stand-in for each and reports it, and the document still reaches its
/// output.
fn render_text(
    registry: &Arc<FlmRegistry>,
    commands: &[&str],
    text: &str,
    options: NumberingOptions,
) -> Run {
    let doc = common::pipeline::doc_with_options(
        registry,
        &[text],
        OptionValues::new().with(options),
    );
    let overrides = Overrides {
        unsupported: [(
            "text".into(),
            commands.iter().map(|command| format!("t.num.{command}").into()).collect(),
        )]
        .into_iter()
        .collect(),
        ..Overrides::none()
    };
    common::pipeline::run_with(
        &doc,
        &plan(registry, text::WORKFLOW, &overrides),
        Externals::none(),
        &RunOptions::sequential(),
    )
}

/// The FLM source a list of segments writes.
fn written(segments: &[NumberSegment]) -> String {
    segments
        .iter()
        .map(|segment| {
            let nodes: Vec<_> = segment.content().root().children().iter().collect();
            recompose_source(&nodes)
        })
        .collect()
}

/// Every point of the run in point order, written as the number it was given
/// without its word, or `-` for a point that was not numbered.
fn numbers(run: &Run) -> Vec<String> {
    let counters = counters_of(run);
    let options = FormatOptions::new().without_word();
    counters
        .points()
        .map(|(point, record)| match record {
            None => "-".to_string(),
            Some(_) => written(
                &counters
                    .format(point, &options)
                    .expect("the point has a record")
                    .expect("the number is written"),
            ),
        })
        .collect()
}

/// An Error expected among [`reports`]: the run refused what it was asked
/// for, and something the document asked to be numbered is not.
fn error(identifier: &str) -> (String, Severity) {
    (identifier.to_string(), Severity::Error)
}

/// A Warning expected among [`reports`]: what was asked for changed nothing,
/// and everything else is numbered as it would have been.
fn warning(identifier: &str) -> (String, Severity) {
    (identifier.to_string(), Severity::Warning)
}

/// The counter table of the run.
fn counters_of(run: &Run) -> &Counters {
    run.table(&COUNTERS).expect("the table is declared").expect("the stage ran")
}

// ---------------------------------------------------------------------------------------------
// counting
// ---------------------------------------------------------------------------------------------

#[test]
fn points_of_one_counter_form_one_sequence() {
    let run = run(&registry(), "\\teq{a} \\teq{b} \\teq{c}", NumberingOptions::new());
    assert_eq!(numbers(&run), ["(1)", "(2)", "(3)"]);
    assert!(identifiers(&run).is_empty(), "{:?}", identifiers(&run));
}

#[test]
fn two_kinds_sharing_one_counter_share_its_sequence_under_their_own_styles() {
    let run = run(&registry(), "\\teq{a} \\teqalt{b} \\teq{c}", NumberingOptions::new());
    assert_eq!(numbers(&run), ["(1)", "[II]", "(3)"]);
}

#[test]
fn a_counter_within_another_restarts_when_that_counter_advances() {
    let run = run(
        &registry(),
        "\\tsec{one} \\tsub{a} \\tsub{b} \\tsec{two} \\tsub{c}",
        NumberingOptions::new(),
    );
    assert_eq!(numbers(&run), ["1", "1.1", "1.2", "2", "2.1"]);
}

#[test]
fn a_point_under_an_ancestor_with_no_value_is_not_numbered_and_reported_once() {
    // A paragraph inside a section that has no subsection yet has no number:
    // the subsection has no value there. The condition is reported once for
    // the counter, at the first such point, and the paragraph counter is not
    // advanced, so the first paragraph after a subsection is the first.
    let run = run(
        &registry(),
        "\\tsec{one} \\tsec{two} \\tpara{p} \\tpara{p} \\tsub{a} \\tpara{q}",
        NumberingOptions::new(),
    );
    assert_eq!(numbers(&run), ["1", "2", "-", "-", "2.1", "2.1.1"]);
    assert_eq!(reports(&run), [error("flm.numbering.parent-uninitialized")]);
}

#[test]
fn an_initial_value_gives_a_counter_a_value_before_its_first_advance() {
    // `0.1` for a subsection before any section.
    let options = NumberingOptions::new().initial("tsec", 0);
    let zero = run(&registry(), "\\tsub{a} \\tsub{b} \\tsec{one} \\tsub{c}", options);
    assert_eq!(numbers(&zero), ["0.1", "0.2", "1", "1.1"]);
    assert_eq!(reports(&zero), []);

    // The first advance goes from the initial value.
    let options = NumberingOptions::new().initial("tsec", 3);
    let three = run(&registry(), "\\tsec{one} \\tsub{a}", options);
    assert_eq!(numbers(&three), ["4", "4.1"]);
}

#[test]
fn a_counter_resumes_where_it_left_off_when_a_combination_comes_round_again() {
    // The options move `teq` inside `tsec`, so `teq` is kept per section, and
    // the same section is never met twice.
    let options = NumberingOptions::new()
        .within("teq", Within::parent("tsec"));
    let run = run(
        &registry(),
        "\\tsec{one} \\teq{a} \\teq{b} \\tsec{two} \\teq{c}",
        options,
    );
    assert_eq!(numbers(&run), ["1", "(1.1)", "(1.2)", "2", "(2.1)"]);
}

#[test]
fn every_counter_below_one_with_no_value_is_reported_in_an_error_of_its_own() {
    // A point whose parent has no value is not numbered and does not advance
    // its counter, which leaves the counter below it without a value too.
    // Each is one Error naming the counter and the ancestor it needs, so three
    // counters below one that never advances give three Errors — and the
    // document still renders, without those numbers.
    let counters = vec![
        CounterDecl::new("tsec", "t.style.plain"),
        CounterDecl::new("tsub", "t.style.plain").within(Within::parent("tsec")),
        CounterDecl::new("tpara", "t.style.plain").within(Within::parent("tsub")),
        CounterDecl::new("tline", "t.style.plain").within(Within::parent("tpara")),
    ];
    let commands = ["tsec", "tsub", "tpara", "tline"];
    let constructs: Vec<Construct> = commands
        .iter()
        .map(|command| Construct::new(command, command, "t.style.plain"))
        .collect();
    let feature =
        Arc::new(TestFeature { counters, styles: vec![plain("t.style.plain")], constructs });
    let registry = registry_with(StdcoreConfig::new(), feature);
    let source = "Before \\tsub{a} \\tpara{p} \\tline{l} \\tsub{b} after.\n";

    let run = run(&registry, source, NumberingOptions::new());
    assert_eq!(
        reports(&run),
        [
            error("flm.numbering.parent-uninitialized"),
            error("flm.numbering.parent-uninitialized"),
            error("flm.numbering.parent-uninitialized"),
        ]
    );
    assert_eq!(numbers(&run), ["-", "-", "-", "-"]);

    let rendered = render_text(&registry, &commands, source, NumberingOptions::new());
    let of_numbering: Vec<(String, Severity)> = reports(&rendered)
        .into_iter()
        .filter(|(identifier, _)| identifier.starts_with("flm.numbering."))
        .collect();
    assert_eq!(of_numbering.len(), 3, "{of_numbering:?}");
    let written = common::lowering::text(&rendered);
    assert!(written.contains("Before") && written.contains("after"), "{written}");
}

#[test]
fn a_counter_the_options_move_within_another_needs_nothing_of_the_one_between() {
    // The options put `tpara` inside `tsec` directly, so `tsub`, which never
    // advances here, is named nowhere and `tpara` is numbered.
    let options = NumberingOptions::new()
        .within("tpara", Within::parent("tsec"));
    let run = run(&registry(), "\\tsec{one} \\tpara{p} \\tpara{q}", options);
    assert_eq!(numbers(&run), ["1", "1.1", "1.2"]);
    assert!(identifiers(&run).is_empty(), "{:?}", identifiers(&run));
}

// ---------------------------------------------------------------------------------------------
// sub-numbered groups (P6)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_sub_numbered_group_written_as_separate_constructs() {
    let run = run(
        &registry(),
        "\\teq{a} \\teq{b} \\teqsub{c} \\teqmore{d} \\teqmore{e} \\teq{f}",
        NumberingOptions::new(),
    );
    assert_eq!(numbers(&run), ["(1)", "(2)", "(3a)", "(3b)", "(3c)", "(4)"]);
}

#[test]
fn a_sub_numbered_group_written_as_the_lines_of_one_construct() {
    let run = run(
        &registry(),
        "\\teq{a} \\teq{b} \\teqlines{c} \\teq{f}",
        NumberingOptions::new(),
    );
    assert_eq!(numbers(&run), ["(1)", "(2)", "(3a)", "(3b)", "(3c)", "(4)"]);
}

#[test]
fn advancing_a_level_a_style_cannot_write_is_an_error_at_the_point() {
    // `t.style.alt` declares no sub-level, so a point shown in it may not
    // advance the first sub-number.
    let run = run(&registry(), "\\teqsub{a} \\teqaltmore{b}", NumberingOptions::new());
    assert_eq!(numbers(&run), ["(1a)", "-"]);
    assert_eq!(reports(&run), [error("flm.numbering.undeclared-sub-level")]);
}

#[test]
fn a_value_carrying_a_sub_number_a_style_cannot_write_is_an_error_at_the_point() {
    // The counter stands at `(1, 1)` because of the group before it, and the
    // point takes that value without advancing anything: the sub-number is
    // there and `t.style.alt` cannot write it. The counter is left where it
    // stood, so the equation after it is `(2)`.
    let run = run(
        &registry(),
        "\\teqsub{a} \\teqaltsame{b} \\teq{c}",
        NumberingOptions::new(),
    );
    assert_eq!(numbers(&run), ["(1a)", "-", "(2)"]);
    assert_eq!(reports(&run), [error("flm.numbering.undeclared-sub-level")]);
}

#[test]
fn a_point_that_advances_nothing_takes_the_counter_where_it_stands() {
    let run = run(&registry(), "\\teq{a} \\teqaltsame{b} \\teq{c}", NumberingOptions::new());
    assert_eq!(numbers(&run), ["(1)", "[I]", "(2)"]);
}

// ---------------------------------------------------------------------------------------------
// the options
// ---------------------------------------------------------------------------------------------

#[test]
fn a_render_request_overrides_what_the_document_said() {
    let document = NumberingOptions::new().initial("teq", 5);
    let render = NumberingOptions::new().initial("teq", 0);
    let run = run_with(
        &registry(),
        "\\teq{a} \\teq{b}",
        document,
        Some(render),
        &RunOptions::sequential(),
    );
    assert_eq!(numbers(&run), ["(1)", "(2)"]);
}

#[test]
fn the_options_may_number_a_counter_within_nothing() {
    let options = NumberingOptions::new()
        .within_nothing("tsub");
    let run = run(&registry(), "\\tsec{one} \\tsub{a} \\tsec{two} \\tsub{b}", options);
    assert_eq!(numbers(&run), ["1", "1", "2", "2"]);
}

#[test]
fn an_option_naming_a_counter_nothing_declares_is_a_warning() {
    let options = NumberingOptions::new().initial("nosuch", 0);
    let run = run(&registry(), "\\teq{a}", options);
    assert_eq!(reports(&run), [warning("flm.numbering.unknown-counter")]);
    assert_eq!(numbers(&run), ["(1)"]);
}

#[test]
fn a_within_option_naming_a_counter_nothing_declares_is_a_warning() {
    // The parent an option names is checked, not only the template that
    // writes it: a misspelled parent whose template names no counter would
    // otherwise silently change what restarts the counter, and one whose
    // template does name it would be refused for the wrong reason. In both
    // cases the counter keeps the hierarchy it was declared with.
    for template in ["X-", "#{nosuch}."] {
        let options = NumberingOptions::new()
            .within("tsub", Within::parent("nosuch").template(template));
        let run = run(&registry(), "\\tsec{one} \\tsub{a} \\tsec{two} \\tsub{b}", options);
        assert_eq!(reports(&run), [warning("flm.numbering.unknown-counter")], "{template}");
        assert_eq!(numbers(&run), ["1", "1.1", "2", "2.1"], "{template}");
    }
}

#[test]
fn options_that_put_two_counters_inside_each_other_number_neither() {
    let options = NumberingOptions::new()
        .within("tsec", Within::parent("tsub"));
    let run = run(&registry(), "\\tsec{one} \\tsub{a}", options);
    assert_eq!(reports(&run), [error("flm.numbering.hierarchy-cycle")]);
    assert_eq!(numbers(&run), ["-", "-"]);
}

#[test]
fn a_within_template_the_options_carry_that_is_not_flm_is_an_error() {
    let options = NumberingOptions::new()
        .within("teq", Within::parent("tsec").template("\\nosuchcommand{"));
    let run = run(&registry(), "\\tsec{one} \\teq{a}", options);
    assert_eq!(reports(&run), [error("flm.numbering.bad-within-joiner")]);
    // The counter keeps the hierarchy it was declared with, which is none.
    assert_eq!(numbers(&run), ["1", "(1)"]);
}

#[test]
fn a_joiner_the_options_carry_that_names_a_counter_it_is_not_within_is_refused() {
    // Refused at the start of the run; the counter keeps the hierarchy it was
    // declared with, which is none.
    let options = NumberingOptions::new()
        .within("teq", Within::parent("tsec").template("#{tsub}."));
    let run = run(&registry(), "\\tsec{one} \\teq{a}", options);
    assert_eq!(reports(&run), [error("flm.numbering.not-an-ancestor")]);
    assert_eq!(numbers(&run), ["1", "(1)"]);
}

#[test]
fn a_number_template_may_wrap_the_number_in_a_command() {
    // The number template is instantiated into a parsed template, so a
    // command's argument may hold the substitution.
    let mut style = plain("t.style.plain");
    style.number_format = NumberTemplate::formats(r"\textbf{#{arabic}}");
    let config = StdcoreConfig::new().numbering(NumberingConfig::new().style(style));
    let registry = registry_with(config, test_feature());
    let options = NumberingOptions::new();
    let run = run(&registry, "\\tsec{one} \\tsec{two}", options);
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), [r"\textbf{1}", r"\textbf{2}"]);
}

#[test]
fn a_within_template_may_wrap_an_ancestor_in_a_command() {
    let registry = registry();
    let template = r"\textbf{#{tsec}}.";
    let options = NumberingOptions::new()
        .within("tsub", Within::parent("tsec").template(template));
    let run = run(&registry, "\\tsec{one} \\tsub{a} \\tsub{b}", options);
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), ["1", r"\textbf{1}.1", r"\textbf{1}.2"]);

    // The markup spans the substitution, so the template is written as one
    // part: the joiner cannot cut it at the substitution and keep each piece
    // valid on its own.
    let style = Arc::new(
        plain("t.style.plain").compile(registry.lang()).expect("the style compiles"),
    );
    let joiner = Within::parent("tsec")
        .template(template)
        .compile(registry.lang(), &[Name::new("tsec")], &[])
        .expect("the joiner compiles");
    let ancestors = vec![Ancestor::new("tsec", CounterValue::new(1), Arc::clone(&style))];
    let written_parts =
        parts(&style, Parent::new(joiner.as_ref(), &ancestors)).expect("the parts are written");
    assert_eq!(written_parts.len(), 1);
    assert_eq!(
        written_parts[0]
            .stands_for
            .iter()
            .map(|(counter, _)| counter.to_string())
            .collect::<Vec<_>>(),
        ["tsec"]
    );
}

#[test]
fn a_dotted_counter_name_is_written_in_full_in_a_within_template() {
    // A counter's name is dotted and owned by the feature that declares it; a
    // `#{…}` names a counter by the whole text between its braces, dots
    // included, with or without keys, and `#{_parent}` names the parent that
    // applies without spelling it.
    let counters = vec![
        CounterDecl::new("t.dot.sec", "t.style.plain"),
        CounterDecl::new("t.dot.sub", "t.style.plain")
            .within(Within::parent("t.dot.sec").template("#[Roman]{t.dot.sec}-")),
        CounterDecl::new("t.dot.para", "t.style.plain")
            .within(Within::parent("t.dot.sub").template(r"\textbf{#{_parent}}:")),
        CounterDecl::new("t.dot.line", "t.style.plain")
            .within(Within::parent("t.dot.para").template("#[bare]{t.dot.para}/")),
    ];
    let constructs = vec![
        Construct::new("dsec", "t.dot.sec", "t.style.plain"),
        Construct::new("dsub", "t.dot.sub", "t.style.plain"),
        Construct::new("dpara", "t.dot.para", "t.style.plain"),
        Construct::new("dline", "t.dot.line", "t.style.plain"),
    ];
    let feature =
        Arc::new(TestFeature { counters, styles: vec![plain("t.style.plain")], constructs });
    let registry = registry_with(StdcoreConfig::new(), feature);
    let declared = run(
        &registry,
        "\\dsec{a} \\dsec{b} \\dsub{c} \\dpara{d} \\dline{e}",
        NumberingOptions::new(),
    );
    assert_eq!(reports(&declared), []);
    assert_eq!(numbers(&declared), ["1", "2", "II-1", r"\textbf{II-1}:1", "1/1"]);

    // The options name the counters in full too.
    let options = NumberingOptions::new()
        .within("t.dot.para", Within::parent("t.dot.sec").template("#{t.dot.sec}."));
    let moved = run(&registry, "\\dsec{a} \\dpara{d}", options);
    assert_eq!(reports(&moved), []);
    assert_eq!(numbers(&moved), ["1", "1.1"]);
}

// ---------------------------------------------------------------------------------------------
// what a point may name
// ---------------------------------------------------------------------------------------------

#[test]
fn a_point_counting_in_a_counter_nothing_declares_is_an_error_reported_once() {
    let feature = Arc::new(TestFeature {
        counters: Vec::new(),
        styles: vec![equation_style("t.style.eq")],
        constructs: vec![Construct::new("teq", "teq", "t.style.eq")],
    });
    let registry = registry_with(StdcoreConfig::new(), feature);
    let run = run(&registry, "\\teq{a} \\teq{b} \\teq{c}", NumberingOptions::new());
    assert_eq!(reports(&run), [error("flm.numbering.undeclared-counter")]);
    assert_eq!(numbers(&run), ["-", "-", "-"]);
}

#[test]
fn a_point_shown_in_a_style_nothing_declares_is_an_error_at_the_point() {
    let feature = Arc::new(TestFeature {
        counters: vec![CounterDecl::new("teq", "t.style.eq")],
        styles: vec![equation_style("t.style.eq")],
        constructs: vec![Construct::new("teq", "teq", "t.style.missing")],
    });
    let registry = registry_with(StdcoreConfig::new(), feature);
    let run = run(&registry, "\\teq{a}", NumberingOptions::new());
    assert_eq!(reports(&run), [error("flm.numbering.undeclared-style")]);
    assert_eq!(numbers(&run), ["-"]);
}

// ---------------------------------------------------------------------------------------------
// what fails the registry build
// ---------------------------------------------------------------------------------------------

#[test]
fn a_counter_declared_twice_fails_the_build() {
    let message = build_failure(
        vec![CounterDecl::new("teq", "s"), CounterDecl::new("teq", "s")],
        vec![plain("s")],
    );
    assert!(message.contains("two features declare the counter `teq`"), "{message}");
}

#[test]
fn a_style_declared_twice_fails_the_build() {
    let message = build_failure(Vec::new(), vec![plain("s"), plain("s")]);
    assert!(message.contains("two features declare the counter style `s`"), "{message}");
}

#[test]
fn a_counter_shown_in_a_style_nothing_declares_fails_the_build() {
    let message = build_failure(vec![CounterDecl::new("teq", "s")], Vec::new());
    assert!(
        message.contains("the counter `teq` is shown in the counter style `s`"),
        "{message}"
    );
}

#[test]
fn a_counter_numbered_within_one_nothing_declares_fails_the_build() {
    let message = build_failure(
        vec![CounterDecl::new("teq", "s").within(Within::parent("tsec"))],
        vec![plain("s")],
    );
    assert!(
        message.contains("the counter `teq` is numbered within `tsec`"),
        "{message}"
    );
}

#[test]
fn a_hierarchy_that_leads_back_to_itself_fails_the_build() {
    let message = build_failure(
        vec![
            CounterDecl::new("a", "s").within(Within::parent("b")),
            CounterDecl::new("b", "s").within(Within::parent("a")),
        ],
        vec![plain("s")],
    );
    assert!(message.contains("numbered within each other"), "{message}");
}

#[test]
fn a_within_template_naming_a_counter_that_is_not_an_ancestor_fails_the_build() {
    let message = build_failure(
        vec![
            CounterDecl::new("tsec", "s"),
            CounterDecl::new("tother", "s"),
            CounterDecl::new("teq", "s").within(Within::parent("tsec").template("#{tother}.")),
        ],
        vec![plain("s")],
    );
    assert!(message.contains("writes the counter `tother`"), "{message}");
}

#[test]
fn a_number_template_naming_a_format_that_does_not_exist_fails_the_build() {
    let mut style = plain("s");
    style.number_format = NumberTemplate::formats("#{nosuchformat}");
    let message = build_failure(Vec::new(), vec![style]);
    assert!(message.contains("nosuchformat"), "{message}");
}

#[test]
fn a_style_text_that_is_not_flm_fails_the_build() {
    let mut style = plain("s");
    style.delimiters = Delimiters::new("\\nosuchcommand{", ")");
    let message = build_failure(Vec::new(), vec![style]);
    assert!(message.contains("the counter style `s`"), "{message}");
}

#[test]
fn a_within_template_that_is_not_flm_fails_the_build() {
    let message = build_failure(
        vec![
            CounterDecl::new("tsec", "s"),
            CounterDecl::new("teq", "s")
                .within(Within::parent("tsec").template("\\nosuchcommand{")),
        ],
        vec![plain("s")],
    );
    assert!(message.contains("\\nosuchcommand{"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// the configuration
// ---------------------------------------------------------------------------------------------

#[test]
fn the_configuration_replaces_a_declared_style_by_name() {
    let config = StdcoreConfig::new()
        .numbering(NumberingConfig::new().style(alternative_style("t.style.eq")));
    let registry = registry_with(config, test_feature());
    let run = run(&registry, "\\teq{a} \\teq{b}", NumberingOptions::new());
    assert_eq!(numbers(&run), ["[I]", "[II]"]);
}

#[test]
fn a_replacement_naming_no_declared_style_fails_the_build() {
    let config =
        StdcoreConfig::new().numbering(NumberingConfig::new().style(plain("t.style.nosuch")));
    let message = build_failure_under(config, Vec::new(), vec![plain("s")]);
    assert!(
        message.contains("replaces the counter style `t.style.nosuch`"),
        "{message}"
    );
}

#[test]
fn the_table_answers_every_declared_style_by_name_the_replacement_where_the_host_replaced_one() {
    // `t.style.roman` is replaced by the host; the document is written in
    // `t.style.eq` only, so `t.style.alt` is a style no record uses.
    let config = StdcoreConfig::new()
        .numbering(NumberingConfig::new().style(alternative_style("t.style.roman")));
    let host = registry_with(config, test_feature());
    let replaced = run(&host, "\\teq{a} \\teq{b}", NumberingOptions::new());
    assert_eq!(reports(&replaced), []);
    let counters = counters_of(&replaced);
    let (second, _) = counters.points().nth(1).expect("two points");
    let in_style = |name: &str| {
        let style = counters.style(&Name::new(name)).expect("the style is declared");
        assert_eq!(style.name(), &Name::new(name));
        written(
            &counters
                .format_in_style(second, style, &FormatOptions::new().without_word())
                .expect("the point has a record")
                .expect("the number is written"),
        )
    };

    // A style a record uses: the very style the record carries.
    let record = counters.record(second).expect("numbered");
    let used = counters.style(&Name::new("t.style.eq")).expect("declared");
    assert!(Arc::ptr_eq(used, &record.style));
    assert_eq!(in_style("t.style.eq"), "(2)");
    // A style no record uses.
    assert_eq!(in_style("t.style.alt"), "[II]");
    // A style the host replaced: the replacement, not the declaration.
    assert_eq!(in_style("t.style.roman"), "[II]");
    let declared = run(&registry(), "\\teq{a} \\teq{b}", NumberingOptions::new());
    let declared_counters = counters_of(&declared);
    let (point, _) = declared_counters.points().nth(1).expect("two points");
    let roman = declared_counters.style(&Name::new("t.style.roman")).expect("declared");
    assert_eq!(
        written(
            &declared_counters
                .format_in_style(point, roman, &FormatOptions::new().without_word())
                .expect("the point has a record")
                .expect("the number is written"),
        ),
        "II"
    );
    // A name no feature declares.
    assert!(counters.style(&Name::new("t.style.nosuch")).is_none());
}

#[test]
fn two_configurations_reach_different_registry_digests() {
    let plain_registry = registry_with(StdcoreConfig::new(), test_feature());
    let config = StdcoreConfig::new()
        .numbering(NumberingConfig::new().style(alternative_style("t.style.eq")));
    let other = registry_with(config, test_feature());
    assert_ne!(plain_registry.digest(), other.digest());
}

// ---------------------------------------------------------------------------------------------
// absence is ordinary
// ---------------------------------------------------------------------------------------------

/// The document the absence tests render on the three backends.
///
/// It holds the two things the absent features serve — a numbered equation
/// line, which states a number source and carries a label, and a reference to
/// that line — so that what the tests exercise is the absence itself and not
/// merely a document that would render under any registry.
const PLAIN_DOCUMENT: &str = concat!(
    "\\section{Introduction}\n\n",
    "\\begin{equation}\n  E = mc^2 \\label{eq:energy}\n\\end{equation}\n\n",
    "Some \\emph{text}, see \\ref{eq:energy}.\n",
);

/// Renders [`PLAIN_DOCUMENT`] on the text, `latex-recomposed` and `html`
/// workflows of `registry` and asserts that each run reports exactly
/// `expected`.
fn renders_on_every_backend(registry: &Arc<FlmRegistry>, expected: &[&str]) {
    for workflow in [text::WORKFLOW, latex_recomposed::WORKFLOW, html::WORKFLOW] {
        let run = common::stdcore::run_workflow(
            registry,
            workflow,
            &[PLAIN_DOCUMENT],
            &Overrides::none(),
        );
        assert_eq!(identifiers(&run), expected, "{workflow}");
    }
}

/// A `\ref` whose command no registered feature declares is reported once,
/// where the document writes it, and the document still renders.
const UNRESOLVABLE_COMMAND: &str = "core.specs.unresolvable-command";

#[test]
fn a_registry_with_numbering_and_no_counter_builds_and_renders_on_every_backend() {
    renders_on_every_backend(&stdcore_registry(StdcoreConfig::new()), &[]);
}

#[test]
fn a_registry_without_numbering_builds_and_renders_on_every_backend() {
    // The reference still resolves; with no counter table behind it and no
    // content on the line, it has nothing to show: the backends that show it
    // report so and show the label the document wrote, and the recomposed
    // LaTeX, which writes the reference for LaTeX to set, reports nothing.
    let registry = without(&["numbering"]);
    for (workflow, expected) in [
        (text::WORKFLOW, &["flm.refs.nothing-to-show"][..]),
        (latex_recomposed::WORKFLOW, &[][..]),
        (html::WORKFLOW, &["flm.refs.nothing-to-show"][..]),
    ] {
        let run = common::stdcore::run_workflow(
            &registry,
            workflow,
            &[PLAIN_DOCUMENT],
            &Overrides::none(),
        );
        assert_eq!(identifiers(&run), expected, "{workflow}");
    }
}

#[test]
fn a_registry_without_refs_builds_and_renders_on_every_backend() {
    // Nothing declares `\ref`, so the command is unattributed and reported
    // once; the equation line is numbered all the same.
    renders_on_every_backend(&without(&["refs"]), &[UNRESOLVABLE_COMMAND]);
}

#[test]
fn a_registry_without_refs_and_without_numbering_builds_and_renders_on_every_backend() {
    renders_on_every_backend(&without(&["refs", "numbering"]), &[UNRESOLVABLE_COMMAND]);
}

#[test]
fn a_counter_declared_where_no_numbering_feature_is_registered_is_dropped_with_a_note() {
    let mut builder = RegistryBuilder::new();
    builder.feature(test_feature());
    let registry = builder
        .build(&RegistryInputs::new())
        .unwrap_or_else(|error| panic!("the build failed:\n{}", error.render()));
    // One Note per slot: the contribution is dropped, and nothing else about
    // the registry changes.
    let notes: Vec<(&str, Severity)> = registry
        .build_diagnostics()
        .iter()
        .map(|d| (d.identifier(), d.severity()))
        .collect();
    assert_eq!(
        notes,
        [
            ("flm.registry.unresolved-peer", Severity::Note),
            ("flm.registry.unresolved-peer", Severity::Note),
            ("flm.registry.unresolved-peer", Severity::Note),
            ("flm.registry.unresolved-peer", Severity::Note),
        ]
    );
    assert!(registry.peer_slots().iter().all(|contribution| !contribution.resolved));
}

/// A registry over every standard feature except the ones named.
fn without(left_out: &[&str]) -> Arc<FlmRegistry> {
    let entry = package(StdcoreConfig::new());
    let mut builder = RegistryBuilder::new();
    for feature in entry.features.iter().chain([&flm_core::math_passthrough::feature()]) {
        if !left_out.iter().any(|name| feature.info().name == *name) {
            builder.feature(Arc::clone(feature));
        }
    }
    builder
        .build(&RegistryInputs::new())
        .unwrap_or_else(|error| panic!("the build failed:\n{}", error.render()))
}

// ---------------------------------------------------------------------------------------------
// the same numbers whatever the run does
// ---------------------------------------------------------------------------------------------

#[test]
fn the_same_document_is_numbered_the_same_way_however_the_run_is_executed() {
    let registry = registry();
    let text = "\\tsec{one} \\teq{a} \\tsub{x} \\teqsub{b} \\teqmore{c} \\tsec{two} \\teq{d}";
    let options = NumberingOptions::new();
    let sequential = numbers(&run_with(
        &registry,
        text,
        options.clone(),
        None,
        &RunOptions::sequential(),
    ));
    let parallel =
        numbers(&run_with(&registry, text, options.clone(), None, &RunOptions::new()));
    assert_eq!(sequential, parallel);
    assert_eq!(sequential, ["1", "(1)", "1.1", "(2a)", "(2b)", "2", "(3)"]);
}

#[test]
fn the_numbers_do_not_depend_on_the_workflow() {
    let registry = registry();
    let text = "\\tsec{one} \\teq{a} \\teq{b}";
    let options = NumberingOptions::new();
    let checked = run(&registry, text, options.clone());
    let commands: Vec<&str> = constructs().iter().map(|construct| construct.command).collect();
    let rendered = render_text(&registry, &commands, text, options);
    assert_eq!(numbers(&checked), numbers(&rendered));
}

// ---------------------------------------------------------------------------------------------
// writing several numbers at once
// ---------------------------------------------------------------------------------------------

/// The points of the run in point order.
fn points(run: &Run) -> Vec<flm_core::model::aspects::PointId> {
    counters_of(run).points().map(|(point, _)| point).collect()
}

/// Each group of `many`, written out.
fn groups(many: &ManyNumbers) -> Vec<String> {
    many.groups.iter().map(|group| written(group)).collect()
}

#[test]
fn several_points_of_one_style_are_written_as_one_text() {
    let run = run(&registry(), "\\teq{a} \\teq{b} \\teq{c}", NumberingOptions::new());
    let counters = counters_of(&run);
    let many = counters
        .format_many(&points(&run), &FormatOptions::new())
        .expect("the numbers are written");
    assert_eq!(groups(&many), ["Eqs.~(1\u{2013}3)"]);
    assert!(many.unnumbered.is_empty());
}

#[test]
fn points_of_two_styles_are_two_groups_the_table_does_not_join() {
    let run = run(&registry(), "\\teq{a} \\teqalt{b} \\teq{c}", NumberingOptions::new());
    let counters = counters_of(&run);
    let many = counters
        .format_many(&points(&run), &FormatOptions::new())
        .expect("the numbers are written");
    // The styles keep the order they were first met in, and a range never
    // spans two of them. Joining the groups is the caller's business.
    assert_eq!(groups(&many), ["Eqs.~(1 and 3)", "Alt.~[II]"]);
}

#[test]
fn points_numbered_within_different_counters_are_different_groups() {
    let options = NumberingOptions::new()
        .within("teq", Within::parent("tsec"));
    let run = run(
        &registry(),
        "\\tsec{one} \\teq{a} \\tsec{two} \\teq{b}",
        options,
    );
    let counters = counters_of(&run);
    let equations: Vec<_> = points(&run)
        .into_iter()
        .filter(|point| {
            counters.record(*point).is_some_and(|record| record.counter.as_str() == "teq")
        })
        .collect();
    let many = counters
        .format_many(&equations, &FormatOptions::new())
        .expect("the numbers are written");
    assert_eq!(groups(&many), ["Eqs.~(1.1 and 2.1)"]);
}

#[test]
fn a_point_with_no_number_comes_last_and_is_written_nowhere() {
    let options = NumberingOptions::new();
    let run = run(&registry(), "\\teq{a} \\tpara{p} \\teq{b}", options);
    let counters = counters_of(&run);
    let all = points(&run);
    let many =
        counters.format_many(&all, &FormatOptions::new()).expect("the numbers are written");
    assert_eq!(groups(&many), ["Eqs.~(1 and 2)"]);
    assert_eq!(many.unnumbered.len(), 1);
    assert!(counters.lookup(many.unnumbered[0]).expect("the point was seen").is_none());
}

#[test]
fn every_segment_links_to_the_point_whose_number_it_writes() {
    let run = run(&registry(), "\\teq{a} \\teq{b}", NumberingOptions::new());
    let counters = counters_of(&run);
    let all = points(&run);
    let many =
        counters.format_many(&all, &FormatOptions::new()).expect("the numbers are written");
    assert_eq!(many.groups.len(), 1);
    let links: Vec<Option<usize>> = many.groups[0]
        .iter()
        .map(|segment| segment.link().map(|point| all.iter().position(|p| *p == point).unwrap()))
        .collect();
    // The word links to the first value, each number to its own point, and the
    // delimiters and the conjunction to nothing.
    assert_eq!(groups(&many), ["Eqs.~(1 and 2)"]);
    assert_eq!(links, [Some(0), None, Some(0), None, Some(1), None]);
}

#[test]
fn one_points_number_links_to_that_point_alone() {
    let run = run(&registry(), "\\teq{a}", NumberingOptions::new());
    let counters = counters_of(&run);
    let point = points(&run)[0];
    let segments = counters
        .format(point, &FormatOptions::new())
        .expect("the point has a record")
        .expect("the number is written");
    assert_eq!(written(&segments), "Eq.~(1)");
    assert_eq!(segments.iter().filter_map(NumberSegment::link).collect::<Vec<_>>(), [point]);
}

#[test]
fn the_table_tells_a_point_it_never_saw_from_one_it_did_not_number() {
    // `teq` within `tsec`, which has no value: the point is not numbered.
    let options = NumberingOptions::new().within("teq", Within::parent("tsec"));
    let run = run(&registry(), "\\teq{a}", options);
    let counters = counters_of(&run);
    let point = points(&run)[0];
    assert!(counters.lookup(point).is_some());
    assert!(counters.lookup(point).expect("seen").is_none());
    assert!(counters.record(point).is_none());
    // A point of the same node that does not exist — the node carries one —
    // stands for a point the stage never saw.
    let never_seen = flm_core::model::aspects::PointId { index: 99, ..point };
    assert!(counters.lookup(never_seen).is_none());
}

// ---------------------------------------------------------------------------------------------
// what the table shows of itself
// ---------------------------------------------------------------------------------------------

#[test]
fn the_table_projects_each_point_with_its_value_and_the_counters_it_is_within() {
    let options = NumberingOptions::new();
    let run = run(&registry(), "\\tsec{one} \\teqsub{a}", options);
    // The projector names the table and puts its own projection under it.
    let projection = common::stdcore::projection(&run, COUNTERS);
    let inner = common::dump::field(&projection, "flm.counters").unwrap_or(&projection).clone();
    let SerialValue::Map(entries) = inner else { panic!("the table projects a map") };
    assert_eq!(entries.len(), 2, "{projection:?}");
    let field = |value: &SerialValue, key: &str| common::dump::field(value, key).cloned();
    let section = &entries[0].1;
    assert_eq!(field(section, "counter"), Some(SerialValue::Str("tsec".into())));
    assert_eq!(field(section, "value"), Some(SerialValue::List(vec![SerialValue::Int(1)])));
    assert_eq!(field(section, "within"), Some(SerialValue::List(Vec::new())));
    let equation = &entries[1].1;
    assert_eq!(field(equation, "style"), Some(SerialValue::Str("t.style.eq".into())));
    assert_eq!(
        field(equation, "value"),
        Some(SerialValue::List(vec![SerialValue::Int(1), SerialValue::Int(1)]))
    );
}

// ---------------------------------------------------------------------------------------------
// a within-joiner registered by name
// ---------------------------------------------------------------------------------------------

#[test]
fn a_counter_declared_within_another_by_a_registered_joiner_is_written_by_it() {
    let options = NumberingOptions::new();
    let run = run(&registry(), "\\tsec{one} \\tsub{a} \\tpath{x}", options);
    assert!(reports(&run).is_empty(), "{:?}", reports(&run));
    assert_eq!(numbers(&run), ["1", "1.1", "tsec1:tsub1/1"]);
}

#[test]
fn the_options_may_number_a_counter_within_another_by_a_registered_joiner() {
    let options = NumberingOptions::new()
        .within("tpara", Within::parent("tsub").joiner_named("t.num.path"));
    let run = run(&registry(), "\\tsec{one} \\tsub{a} \\tpara{x}", options);
    assert!(reports(&run).is_empty(), "{:?}", reports(&run));
    assert_eq!(numbers(&run), ["1", "1.1", "tsec1:tsub1/1"]);
}

#[test]
fn a_within_option_naming_a_joiner_nothing_registers_is_an_error() {
    let options = NumberingOptions::new()
        .within("tpara", Within::parent("tsub").joiner_named("nosuch.joiner"));
    let run = run(&registry(), "\\tsec{one} \\tsub{a} \\tpara{x}", options);
    assert_eq!(reports(&run), [error("flm.numbering.unknown-within-joiner")]);
    // The counter keeps the hierarchy it was declared with.
    assert_eq!(numbers(&run), ["1", "1.1", "1.1.1"]);
}

#[test]
fn a_counter_declared_within_another_by_a_joiner_nothing_registers_fails_the_build() {
    let message = build_failure(
        vec![
            CounterDecl::new("tsec", "s"),
            CounterDecl::new("teq", "s")
                .within(Within::parent("tsec").joiner_named("nosuch.joiner")),
        ],
        vec![plain("s")],
    );
    assert!(message.contains("nosuch.joiner"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// the number writer
// ---------------------------------------------------------------------------------------------

/// The points of the run whose record counts in `counter`, in point order.
fn points_of(run: &Run, counter: &str) -> Vec<flm_core::model::aspects::PointId> {
    let counters = counters_of(run);
    points(run)
        .into_iter()
        .filter(|point| {
            counters.record(*point).is_some_and(|record| record.counter.as_str() == counter)
        })
        .collect()
}

/// The options that switch on every counter shown in a style with a number
/// writer of its own, and the counters they are numbered within.
fn writer_options() -> NumberingOptions {
    NumberingOptions::new()
}

#[test]
fn a_number_writer_of_a_style_decorates_the_default_text() {
    let registry = registry();
    let text = "\\twsec{a} \\twsec{b} \\twsec{c} \\twsec{d} \\twsec{e} \
                \\tweq{1} \\tweq{2} \\tweq{3} \\tweq{4} \\tweq{5}";
    let run = run(&registry, text, writer_options());
    assert!(reports(&run).is_empty(), "{:?}", reports(&run));
    let counters = counters_of(&run);
    let last = *points_of(&run, "tweq").last().expect("five equations");
    let segments = counters
        .format(last, &FormatOptions::new())
        .expect("the point has a record")
        .expect("the number is written");
    // The default writer's own text, then the writer's own: the section's
    // number in capital roman numerals.
    assert_eq!(written(&segments), "Eq.~(5) in Section~V");
}

#[test]
fn a_number_writer_writes_a_range_across_two_parents() {
    let registry = registry();
    // Section V, subsection 3, one equation; then subsection 4 and two more.
    let text = "\\twrsec{a} \\twrsec{b} \\twrsec{c} \\twrsec{d} \\twrsec{e} \
                \\twrsub{i} \\twrsub{ii} \\twrsub{iii} \\twspan{one} \
                \\twrsub{iv} \\twspan{two} \\twspan{three}";
    let run = run(&registry, text, writer_options());
    assert!(reports(&run).is_empty(), "{:?}", reports(&run));
    let counters = counters_of(&run);
    let spans = points_of(&run, "twspan");
    let written_alone: Vec<String> = spans
        .iter()
        .map(|point| {
            written(
                &counters
                    .format(*point, &FormatOptions::new().without_word())
                    .expect("the point has a record")
                    .expect("the number is written"),
            )
        })
        .collect();
    assert_eq!(written_alone, ["(V.3.1)", "(V.4.1)", "(V.4.2)"]);
    // The first of subsection 3 and the last of subsection 4: the writer reads
    // the parts of both, keeps what they share once, and writes one range.
    let many = counters
        .format_many(&[spans[0], spans[2]], &FormatOptions::new())
        .expect("the numbers are written");
    assert_eq!(groups(&many), ["Eqs.~(V.3.1\u{2013}4.2)"]);
}

#[test]
fn a_number_writer_that_asks_for_no_part_does_not_run_the_joiner() {
    let registry = registry();
    // The counter is numbered within a subsection by a joiner that fails the
    // test if it is ever asked for the parts; the style's writer arranges
    // nothing of the ancestors, so it never asks.
    let text = "\\twrsec{a} \\twrsub{i} \\twbare{one} \\twbare{two}";
    let run = run(&registry, text, writer_options());
    assert!(reports(&run).is_empty(), "{:?}", reports(&run));
    let counters = counters_of(&run);
    let bare = points_of(&run, "twbare");
    let segments = counters
        .format(bare[1], &FormatOptions::new())
        .expect("the point has a record")
        .expect("the number is written");
    assert_eq!(written(&segments), "#2");
    let many =
        counters.format_many(&bare, &FormatOptions::new()).expect("the numbers are written");
    assert_eq!(groups(&many), ["#1, #2"]);
}

#[test]
fn a_style_naming_a_number_writer_nothing_registers_fails_the_build() {
    let message =
        build_failure(Vec::new(), vec![plain("t.style.plain").with_number_writer("nosuch.writer")]);
    assert!(message.contains("number writer `nosuch.writer`"), "{message}");
}

#[test]
fn a_style_replaced_through_the_configuration_carries_the_writer_it_names() {
    // The style `t.style.eq` is declared without a writer; the host replaces
    // it by one that names a writer of its own, and every number shown in that
    // style is written by it.
    let replacement = equation_style("t.style.eq").with_number_writer("t.num.bare");
    let config = StdcoreConfig::new().numbering(NumberingConfig::new().style(replacement));
    let registry = registry_with(config, test_feature());
    let run = run(&registry, "\\teq{a} \\teq{b}", NumberingOptions::new());
    assert!(reports(&run).is_empty(), "{:?}", reports(&run));
    assert_eq!(numbers(&run), ["#1", "#2"]);
}

#[test]
fn two_configurations_of_one_style_with_different_writers_digest_differently() {
    let plain_style = NumberingConfig::new().style(equation_style("t.style.eq"));
    let written = NumberingConfig::new()
        .style(equation_style("t.style.eq").with_number_writer("t.num.bare"));
    assert_ne!(plain_style.digest_bytes(), written.digest_bytes());
}

// ---------------------------------------------------------------------------------------------
// alternative parents, parent sets, `#{_parent}` and `no_restart`
// ---------------------------------------------------------------------------------------------

/// A feature contributing parent sets and nothing else.
struct SetFeature {
    sets: Vec<ParentSetDecl>,
}

impl FlmFeature for SetFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.sets", "Test parent sets").docs("Parent sets for the numbering tests.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        for set in &self.sets {
            b.peer_slot::<ParentSetDecl>(PARENT_SETS_SLOT, Arc::new(set.clone()))?;
        }
        Ok(())
    }
}

/// The shared registry plus a feature contributing `sets`.
fn try_registry_with_sets(sets: Vec<ParentSetDecl>) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(test_feature());
    builder.feature(Arc::new(SetFeature { sets }));
    builder.build(&RegistryInputs::new())
}

#[test]
fn a_within_template_writes_the_parent_that_applies_as_underscore_parent() {
    // The template language takes a name beginning with an underscore, and
    // `#{_parent}` is the one parent of a single-parent counter.
    let options = NumberingOptions::new()
        .within("teq", Within::parent("tsub").template("#{_parent}:"));
    let run = run(&registry(), "\\tsec{s} \\tsub{a} \\teq{x}", options);
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), ["1", "1.1", "(1.1:1)"]);
}

#[test]
fn a_counter_with_several_parents_is_numbered_within_the_one_advanced_last() {
    // `tany` is declared within any of `tsec` and `tsub`: after a section
    // alone it is written within the section, after a subsection within the
    // subsection, and after the next section within that section again, its
    // count kept apart for each.
    let run = run(
        &registry(),
        "\\tsec{a} \\tany{x} \\tsub{b} \\tany{y} \\tany{z} \\tsec{c} \\tany{w}",
        NumberingOptions::new(),
    );
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), ["1", "1.1", "1.1", "1.1.1", "1.1.2", "2", "2.1"]);
}

#[test]
fn the_options_may_give_a_counter_several_parents() {
    let options = NumberingOptions::new().within("teq", Within::parents(["tsec", "tsub"]));
    let run = run(&registry(), "\\tsec{a} \\teq{x} \\tsub{b} \\teq{y} \\teq{z}", options);
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), ["1", "(1.1)", "1.1", "(1.1.1)", "(1.1.2)"]);
}

#[test]
fn underscore_parent_writes_whichever_parent_applies() {
    let template = Within::parents(["tsec", "tsub"]).template("#{_parent}:");
    let options = NumberingOptions::new().within("teq", template);
    let whole = run(&registry(), "\\tsec{a} \\teq{x} \\tsub{b} \\teq{y}", options);
    assert_eq!(reports(&whole), []);
    assert_eq!(numbers(&whole), ["1", "(1:1)", "1.1", "(1.1:1)"]);

    // The keys of a named counter work on it the same way.
    let bare = Within::parents(["tsec", "tsub"]).template("#[bare,Roman]{_parent}:");
    let options = NumberingOptions::new().within("teq", bare);
    let keyed = run(&registry(), "\\tsec{a} \\teq{x} \\tsub{b} \\tsub{c} \\teq{y}", options);
    assert_eq!(reports(&keyed), []);
    assert_eq!(numbers(&keyed), ["1", "(I:1)", "1.1", "1.2", "(II:1)"]);
}

#[test]
fn a_point_before_any_of_its_parents_was_advanced_is_reported_once() {
    let options = NumberingOptions::new().within("teq", Within::parents(["tsec", "tsub"]));
    let run = run(&registry(), "\\teq{x} \\teq{y} \\tsec{a} \\teq{z}", options);
    assert_eq!(reports(&run), [error("flm.numbering.parent-uninitialized")]);
    assert_eq!(numbers(&run), ["-", "-", "1", "(1.1)"]);
}

#[test]
fn options_naming_a_counter_that_is_an_ancestor_under_one_parent_only_are_refused() {
    // `#{tsub}` is outside the chain that ends in `tsec`, so the options are
    // refused before any point is walked and `teq` keeps the hierarchy it was
    // declared with, which is none.
    let options = NumberingOptions::new()
        .within("teq", Within::parents(["tsec", "tsub"]).template("#{tsub}:"));
    let run = run(&registry(), "\\tsec{a} \\teq{x} \\tsub{b} \\teq{y}", options);
    assert_eq!(reports(&run), [error("flm.numbering.not-an-ancestor")]);
    assert_eq!(numbers(&run), ["1", "(1)", "1.1", "(2)"]);
}

#[test]
fn a_declaration_naming_a_counter_that_is_an_ancestor_under_one_parent_only_fails_the_build() {
    let message = build_failure(
        vec![
            CounterDecl::new("tsec", "s"),
            CounterDecl::new("tsub", "s").within(Within::parent("tsec")),
            CounterDecl::new("teq", "s")
                .within(Within::parents(["tsec", "tsub"]).template("#{tsub}:")),
        ],
        vec![plain("s")],
    );
    assert!(message.contains("writes the counter `tsub`"), "{message}");
}

#[test]
fn an_initial_value_holds_under_every_value_of_the_ancestors() {
    // `tsub` stands at 0 in every section until a subsection advances it, so a
    // paragraph before the first subsection of each section is `n.0.1`.
    let options = NumberingOptions::new().initial("tsub", 0);
    let zero = run(
        &registry(),
        "\\tsec{a} \\tpara{p} \\tsub{b} \\tpara{q} \\tsec{c} \\tpara{r}",
        options,
    );
    assert_eq!(reports(&zero), []);
    assert_eq!(numbers(&zero), ["1", "1.0.1", "1.1", "1.1.1", "2", "2.0.1"]);

    // And a counter numbered within another starts from its initial value
    // under each value of its parent: the first subsection of every section
    // is the fourth.
    let options = NumberingOptions::new().initial("tsub", 3);
    let later = run(&registry(), "\\tsec{a} \\tsub{b} \\tsec{c} \\tsub{d}", options);
    assert_eq!(numbers(&later), ["1", "1.4", "2", "2.4"]);
}

#[test]
fn no_restart_writes_the_parent_and_continues_the_count() {
    let options = NumberingOptions::new().within("tsub", Within::parent("tsec").no_restart());
    let run = run(
        &registry(),
        "\\tsec{a} \\tsub{x} \\tsub{y} \\tsec{b} \\tsub{z}",
        options,
    );
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), ["1", "1.1", "1.2", "2", "2.3"]);
}

#[test]
fn the_options_may_number_a_counter_within_a_registered_parent_set() {
    let registry = try_registry_with_sets(vec![ParentSetDecl::new("t.levels", ["tsec", "tsub"])])
        .expect("the registry builds");
    let options = NumberingOptions::new().within("teq", Within::parent_set("t.levels"));
    let run = run(&registry, "\\tsec{a} \\teq{x} \\tsub{b} \\teq{y}", options);
    assert_eq!(reports(&run), []);
    assert_eq!(numbers(&run), ["1", "(1.1)", "1.1", "(1.1.1)"]);
}

#[test]
fn a_parent_set_the_options_name_and_nothing_registers_is_an_error() {
    let options = NumberingOptions::new().within("teq", Within::parent_set("t.nosuch"));
    let run = run(&registry(), "\\tsec{a} \\teq{x}", options);
    assert_eq!(reports(&run), [error("flm.numbering.unknown-parent-set")]);
    // The counter keeps the hierarchy it was declared with, which is none.
    assert_eq!(numbers(&run), ["1", "(1)"]);
}

#[test]
fn a_cycle_through_any_one_parent_numbers_none_of_its_counters() {
    let options = NumberingOptions::new().within("tsec", Within::parents(["teq", "tsub"]));
    let run = run(&registry(), "\\tsec{a} \\tsub{b}", options);
    assert_eq!(reports(&run), [error("flm.numbering.hierarchy-cycle")]);
    assert_eq!(numbers(&run), ["-", "-"]);
}

#[test]
fn a_counter_named_with_a_leading_underscore_fails_the_build() {
    let message = build_failure(vec![CounterDecl::new("_x", "s")], vec![plain("s")]);
    assert!(message.contains("the counter `_x`"), "{message}");
    assert!(message.contains("numbering feature's own"), "{message}");
}

#[test]
fn a_parent_set_named_with_a_leading_underscore_fails_the_build() {
    let message = match try_registry_with_sets(vec![ParentSetDecl::new("_levels", ["tsec"])]) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    };
    assert!(message.contains("the parent set `_levels`"), "{message}");
    assert!(message.contains("numbering feature's own"), "{message}");
}

#[test]
fn two_parent_sets_of_one_name_fail_the_build() {
    let set = ParentSetDecl::new("t.levels", ["tsec"]);
    let message = match try_registry_with_sets(vec![set.clone(), set]) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    };
    assert!(message.contains("two features declare the parent set `t.levels`"), "{message}");
}

#[test]
fn a_declaration_numbered_within_a_parent_set_nothing_registers_fails_the_build() {
    let message = build_failure(
        vec![CounterDecl::new("teq", "s").within(Within::parent_set("t.nosuch"))],
        vec![plain("s")],
    );
    assert!(message.contains("the parent set `t.nosuch`"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// scoped instances
// ---------------------------------------------------------------------------------------------

/// The element value of the `tlist` environment: the node a scoped instance
/// of the list counter is attached to. It answers no reference point.
#[derive(Debug)]
struct TList;

/// How a `\titem` names the instances it counts under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chain {
    /// The enclosing `tlist` nodes, outermost first: the right chain.
    Enclosing,
    /// The enclosing `tlist` nodes innermost first: the wrong order.
    Reversed,
    /// The node of the item's own argument, which the item encloses rather
    /// than stands in.
    Inside,
    /// The enclosing `tlist` nodes, outermost first, then the item's own
    /// node, which carries the point rather than encloses it.
    Own,
}

/// The element value of an item: one point counting in the list counter under
/// the `tlist` environments enclosing it, in the style of its depth — the
/// number of those environments — and labeled `t:<its text>`, so that a test
/// finds each item's point through the labels table.
#[derive(Debug)]
struct TItem {
    counter: Name,
    chain: Chain,
    /// The number template the item's point carries, if any.
    format: Option<&'static str>,
}

/// The text of a node's first argument.
fn argument_text(node: &NodeRef<'_, Flm>) -> String {
    node.argument_content_nodes(0)
        .map(|nodes| nodes.iter().filter_map(|node| node.chars()).collect())
        .unwrap_or_default()
}

impl Referenceable for TItem {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let mut lists: Vec<_> = std::iter::successors(node.parent(), |up| up.parent())
            .filter(|up| flm_core::model::element::element_value::<TList>(up).is_some())
            .map(|up| up.id())
            .collect();
        let depth = lists.len();
        lists.reverse();
        let instances = match self.chain {
            Chain::Enclosing => lists,
            Chain::Reversed => lists.into_iter().rev().collect(),
            Chain::Inside => node
                .argument_content_nodes(0)
                .and_then(|nodes| nodes.iter().next().map(|inside| inside.id()))
                .into_iter()
                .collect(),
            Chain::Own => lists.into_iter().chain([node.id()]).collect(),
        };
        let style = format!("t.style.item{}", depth.clamp(1, 3));
        vec![RefPoint {
            kind: RefKind::from("t.item"),
            labels: vec![flm_core::model::aspects::Label::new("t", argument_text(node))],
            malformed: Vec::new(),
            computed: 0,
            anchor: AnchorPolicy::Auto { prefix: "t".to_string() },
            number: Some(NumberSource {
                counter: self.counter.clone(),
                instances,
                style: Name::new(style),
                advance: vec![0],
                number_format: self.format.map(String::from),
            }),
            content: None,
            accepts: AcceptedRefTypes::Any,
        }]
    }
}

/// A feature declaring the counter `t.list`, counted through instances: the
/// environment `tlist`, and the items `\titem`, `\titemrev`, `\titemin` and
/// `\titemown`, whose points name their instances as [`Chain`] says, and
/// `\titemalph`, `\titemroman` and `\titembad`, which carry the number
/// templates `(#{alph})`, `#{Roman}` and one that does not compile. Its counter
/// is declared with `nesting` as its nesting joiner, and numbered within
/// `within`.
struct ListFeature {
    nesting: Option<flm_core::stdcore::features::numbering::WithinJoinerSpec>,
    within: Option<Within>,
}

/// The item styles, one per depth: `1`, `(a)`, `i`, each with the words
/// `Item~` and `Items~`.
fn item_styles() -> Vec<CounterStyleDecl> {
    ["#{arabic}", "(#{alph})", "#{roman}"]
        .into_iter()
        .enumerate()
        .map(|(index, format)| {
            let mut style = CounterStyleDecl::new(format!("t.style.item{}", index + 1));
            style.number_format = NumberTemplate::formats(format);
            style.prefix_words = PrefixWords::new("Item~", "Items~");
            style.join = flm_core::stdcore::features::numbering::JoinDecl::compact();
            style
        })
        .collect()
}

impl FlmFeature for ListFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t.lists", "Test lists").docs("A counter counted through instances.")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        let mut counter = CounterDecl::new("t.list", "t.style.item1");
        counter.nesting = self.nesting.clone();
        counter.within = self.within.clone();
        b.peer_slot::<CounterDecl>(COUNTERS_SLOT, Arc::new(counter))?;
        for style in item_styles() {
            b.peer_slot::<CounterStyleDecl>(STYLES_SLOT, Arc::new(style))?;
        }
        let list = Arc::new(TList);
        b.element(
            ElementDef::new(
                owner.clone(),
                ["tlist"],
                SyntaxDef::Environment {
                    args: Vec::new(),
                    body: flm_core::lang::mint::BodyDef::new(),
                },
                Arc::new(ElementSpec::new("t.lists.tlist").with_value(list)),
            )
            .docs("A test list: one scoped instance of `t.list`."),
        )?;
        for (command, chain, format) in [
            ("titem", Chain::Enclosing, None),
            ("titemrev", Chain::Reversed, None),
            ("titemin", Chain::Inside, None),
            ("titemown", Chain::Own, None),
            ("titemalph", Chain::Enclosing, Some("(#{alph})")),
            ("titemroman", Chain::Enclosing, Some("#{Roman}")),
            ("titembad", Chain::Enclosing, Some("#{nosuchformat}")),
        ] {
            let value = Arc::new(TItem { counter: Name::new("t.list"), chain, format });
            b.element(
                ElementDef::new(
                    owner.clone(),
                    [command],
                    SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                    Arc::new(
                        ElementSpec::new(format!("t.lists.{command}"))
                            .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                            .with_value(value),
                    ),
                )
                .docs("A test item."),
            )?;
        }
        Ok(())
    }
}

/// The registry of the tests of instances: the package, the shared test
/// feature (for `\tsec`) and a list feature with `nesting`.
fn list_registry(
    nesting: Option<flm_core::stdcore::features::numbering::WithinJoinerSpec>,
) -> Arc<FlmRegistry> {
    try_list_registry(nesting).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// [`list_registry`], answering the build's failure.
fn try_list_registry(
    nesting: Option<flm_core::stdcore::features::numbering::WithinJoinerSpec>,
) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    try_list_registry_within(nesting, None)
}

/// [`try_list_registry`] with the list counter declared within `within`.
fn try_list_registry_within(
    nesting: Option<flm_core::stdcore::features::numbering::WithinJoinerSpec>,
    within: Option<Within>,
) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(test_feature());
    builder.feature(Arc::new(ListFeature { nesting, within }));
    builder.build(&RegistryInputs::new())
}

/// The point labeled `t:<text>`.
fn item(run: &Run, text: &str) -> flm_core::model::aspects::PointId {
    let labels = run
        .table(&flm_core::stdcore::features::refs::LABELS)
        .expect("the table is declared")
        .expect("the labels were collected");
    *labels
        .by_label
        .get(&flm_core::model::aspects::Label::new("t", text))
        .unwrap_or_else(|| panic!("no item `{text}`"))
}

/// The number of each item named, without its word, or `-` for an item the
/// run did not number.
fn items(run: &Run, texts: &[&str]) -> Vec<String> {
    let counters = counters_of(run);
    let options = FormatOptions::new().without_word();
    texts
        .iter()
        .map(|text| match counters.format(item(run, text), &options) {
            None => "-".to_string(),
            Some(written_number) => written(&written_number.expect("the number is written")),
        })
        .collect()
}

/// The numbers of the items named, written together: one text per style.
fn items_together(run: &Run, texts: &[&str]) -> Vec<String> {
    let points: Vec<_> = texts.iter().map(|text| item(run, text)).collect();
    let many = counters_of(run)
        .format_many(&points, &FormatOptions::new())
        .expect("the numbers are written");
    groups(&many)
}

/// Two lists side by side, each holding a nested list after its second item,
/// and the second holding two nested lists side by side.
const LISTS: &str = "\\begin{tlist}\\titem{a}\\titem{b}\
                     \\begin{tlist}\\titem{ba}\\titem{bb}\\titem{bc}\\end{tlist}\
                     \\titem{c}\\end{tlist} \
                     \\begin{tlist}\\titem{d}\
                     \\begin{tlist}\\titem{da}\\titem{db}\\end{tlist}\
                     \\begin{tlist}\\titem{dx}\\titem{dy}\\end{tlist}\
                     \\titem{e}\\end{tlist}";

#[test]
fn sibling_instances_each_restart_and_an_enclosing_instance_continues_past_a_nested_one() {
    let run = run(&list_registry(None), LISTS, NumberingOptions::new());
    assert_eq!(reports(&run), []);
    // Two sibling outermost lists: each starts at one, and the first goes on
    // past the list nested in it.
    assert_eq!(items(&run, &["a", "b", "c", "d", "e"]), ["1", "2", "3", "1", "2"]);
    // The nested lists: each starts again, the two sibling nested lists of
    // the second list included, and the bare nesting joiner writes nothing of
    // the enclosing item.
    assert_eq!(
        items(&run, &["ba", "bb", "bc", "da", "db", "dx", "dy"]),
        ["(a)", "(b)", "(c)", "(a)", "(b)", "(a)", "(b)"]
    );
}

#[test]
fn a_nesting_joiner_writes_the_enclosing_items_number_before_a_nested_one() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    for nesting in [
        WithinJoinerSpec::Template("#{_parent}.".to_string()),
        WithinJoinerSpec::Template("#{t.list}.".to_string()),
        WithinJoinerSpec::Joined(".".to_string()),
    ] {
        let run = run(&list_registry(Some(nesting.clone())), LISTS, NumberingOptions::new());
        assert_eq!(reports(&run), [], "{nesting:?}");
        assert_eq!(
            items(&run, &["a", "ba", "bb", "c", "da", "dy"]),
            ["1", "2.(a)", "2.(b)", "3", "1.(a)", "1.(b)"],
            "{nesting:?}"
        );
    }
}

#[test]
fn an_instance_three_deep_writes_each_enclosing_instance_through_its_own_joiner() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    let text = "\\begin{tlist}\\titem{a}\\begin{tlist}\\titem{b}\\titem{c}\
                \\begin{tlist}\\titem{x}\\titem{y}\\end{tlist}\\end{tlist}\\end{tlist}";
    // The counter's own name stands for the innermost enclosing instance, as
    // `#{_parent}` does, and that instance brings the ones around it.
    for template in ["#{_parent}-", "#{t.list}-"] {
        let nesting = WithinJoinerSpec::Template(template.to_string());
        let run = run(&list_registry(Some(nesting)), text, NumberingOptions::new());
        assert_eq!(reports(&run), []);
        assert_eq!(
            items(&run, &["a", "b", "c", "x", "y"]),
            ["1", "1-(a)", "1-(b)", "1-(b)-i", "1-(b)-ii"],
            "{template}"
        );
    }
    // Bare, the default, writes each number alone at every depth.
    let run = bare_run(text);
    assert_eq!(items(&run, &["x", "y"]), ["i", "ii"]);
}

/// [`run`] of `text` over the list registry with the bare nesting joiner.
fn bare_run(text: &str) -> Run {
    run(&list_registry(None), text, NumberingOptions::new())
}

#[test]
fn references_to_items_of_one_instance_form_a_range_and_of_two_instances_two_groups() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    // Without the parent's part: the bare joiner.
    let bare = bare_run(LISTS);
    assert_eq!(items_together(&bare, &["ba", "bb", "bc"]), ["Items~(a)\u{2013}(c)"]);
    // Three items of two sibling lists, and the first items of two nested
    // lists: never one range across two instances.
    assert_eq!(items_together(&bare, &["a", "b", "d"]), ["Items~1,2,1"]);
    assert_eq!(items_together(&bare, &["da", "db", "dx"]), ["Items~(a),(b),(a)"]);
    assert_eq!(items_together(&bare, &["da", "dx"]), ["Items~(a),(a)"]);

    // With the parent's part: the end of a range leaves it out.
    let joined = run(
        &list_registry(Some(WithinJoinerSpec::Template("#{_parent}.".to_string()))),
        LISTS,
        NumberingOptions::new(),
    );
    assert_eq!(items_together(&joined, &["ba", "bb", "bc"]), ["Items~2.(a)\u{2013}(c)"]);
    assert_eq!(items_together(&joined, &["da", "db", "dx", "dy"]), ["Items~1.(a),1.(b),1.(a),1.(b)"]);
}

#[test]
fn the_outermost_instances_may_be_numbered_within_a_section_by_the_options() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    let text = "\\tsec{one} \\begin{tlist}\\titem{a}\\titem{b}\
                \\begin{tlist}\\titem{ba}\\end{tlist}\\end{tlist} \
                \\tsec{two} \\begin{tlist}\\titem{c}\\end{tlist}";
    let options = NumberingOptions::new().within("t.list", Within::parent("tsec"));
    let registry = list_registry(Some(WithinJoinerSpec::Template("#{_parent}.".to_string())));
    let run = run(&registry, text, options);
    assert_eq!(reports(&run), []);
    // The section's number stands before an outermost item, and the nested
    // item writes the enclosing item, which brings the section with it.
    assert_eq!(items(&run, &["a", "b", "ba", "c"]), ["1.1", "1.2", "1.2.(a)", "2.1"]);
    let record = counters_of(&run).record(item(&run, "ba")).expect("numbered");
    let ancestors: Vec<&str> = record.ancestors.iter().map(|a| a.counter.as_str()).collect();
    assert_eq!(ancestors, ["tsec", "t.list"]);
    assert!(record.is_nested());
    assert!(record.ancestors[1].instance.is_some() && record.ancestors[0].instance.is_none());
}

#[test]
fn an_initial_value_is_where_every_instance_starts() {
    let run = run(&list_registry(None), LISTS, NumberingOptions::new().initial("t.list", 4));
    assert_eq!(reports(&run), []);
    assert_eq!(items(&run, &["a", "c", "d", "ba", "dx"]), ["5", "7", "5", "(e)", "(e)"]);
}

#[test]
fn instances_that_do_not_enclose_the_point_one_within_the_next_are_an_error_at_the_point() {
    let text = "\\begin{tlist}\\titem{a}\\begin{tlist}\\titemrev{b}\\titem{c}\\end{tlist}\
                \\titemin{d}\\titem{e}\\end{tlist}";
    let run = bare_run(text);
    assert_eq!(
        reports(&run),
        [error("flm.numbering.instance-not-enclosing"), error("flm.numbering.instance-not-enclosing")]
    );
    // The two points take no number and advance nothing.
    assert_eq!(items(&run, &["a", "b", "c", "d", "e"]), ["1", "-", "(a)", "-", "2"]);
    assert!(counters_of(&run).lookup(item(&run, "b")).is_some_and(|record| record.is_none()));
}

#[test]
fn a_point_naming_its_own_node_as_an_instance_is_an_error_at_the_point() {
    // The point's own node is never one of its instances: `b` names its list
    // and then itself, `z`, outside any list, itself alone.
    let run = bare_run("\\begin{tlist}\\titem{a}\\titemown{b}\\titem{c}\\end{tlist}\\titemown{z}");
    assert_eq!(
        reports(&run),
        [error("flm.numbering.instance-not-enclosing"), error("flm.numbering.instance-not-enclosing")]
    );
    // The two points take no number and advance nothing.
    assert_eq!(items(&run, &["a", "b", "c", "z"]), ["1", "-", "2", "-"]);
    assert!(counters_of(&run).lookup(item(&run, "z")).is_some_and(|record| record.is_none()));
}

#[test]
fn a_nested_instance_before_any_item_of_the_enclosing_one_is_not_numbered() {
    let run = bare_run("\\begin{tlist}\\begin{tlist}\\titem{x}\\end{tlist}\\titem{a}\\end{tlist}");
    assert_eq!(reports(&run), [error("flm.numbering.parent-uninitialized")]);
    assert_eq!(items(&run, &["x", "a"]), ["-", "1"]);
}

#[test]
fn a_nesting_joiner_naming_a_counter_that_is_not_an_ancestor_fails_the_build() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    let failure = |nesting: WithinJoinerSpec| match try_list_registry(Some(nesting)) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    };
    let named = failure(WithinJoinerSpec::Template("#{tsec}.".to_string()));
    assert!(named.contains("`t.list`") && named.contains("`tsec`"), "{named}");
    let unknown = failure(WithinJoinerSpec::Named(Name::new("t.num.nothing")));
    assert!(unknown.contains("t.num.nothing"), "{unknown}");
}

#[test]
fn a_nesting_joiner_may_name_what_the_counter_is_numbered_within() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    // Declared within the section, the counter's nesting joiner may write the
    // section as well as the enclosing item.
    let registry = try_list_registry_within(
        Some(WithinJoinerSpec::Template("#{tsec}/#[bare]{_parent}.".to_string())),
        Some(Within::parent("tsec")),
    )
    .unwrap_or_else(|error| panic!("{}", error.render()));
    let text = "\\tsec{one} \\begin{tlist}\\titem{a}\\begin{tlist}\\titem{b}\\end{tlist}\\end{tlist}";
    let declared = run(&registry, text, NumberingOptions::new());
    assert_eq!(reports(&declared), []);
    assert_eq!(items(&declared, &["a", "b"]), ["1.1", "1/1.(a)"]);

    // Options that number the counter within nothing leave that joiner
    // naming a counter that is no longer an ancestor: the counter is refused,
    // since the options cannot change the nesting joiner, and nothing that
    // counts in it is numbered.
    let moved = run(&registry, text, NumberingOptions::new().within_nothing("t.list"));
    assert_eq!(reports(&moved), [error("flm.numbering.not-an-ancestor")]);
    assert_eq!(items(&moved, &["a", "b"]), ["-", "-"]);
}

#[test]
fn the_table_projects_the_instance_of_a_point_and_of_its_enclosing_items() {
    let run = bare_run("\\begin{tlist}\\titem{a}\\begin{tlist}\\titem{b}\\end{tlist}\\end{tlist}");
    let projection = common::stdcore::projection(&run, COUNTERS);
    let inner = common::dump::field(&projection, "flm.counters").unwrap_or(&projection).clone();
    let SerialValue::Map(entries) = inner else { panic!("the table projects a map") };
    let field = |value: &SerialValue, key: &str| common::dump::field(value, key).cloned();
    let nested = entries
        .iter()
        .map(|(_, record)| record)
        .find(|record| field(record, "within") != Some(SerialValue::List(Vec::new())))
        .expect("the nested item's record");
    assert_eq!(field(nested, "counter"), Some(SerialValue::Str("t.list".into())));
    assert!(field(nested, "instance").is_some(), "{nested:?}");
    let Some(SerialValue::List(within)) = field(nested, "within") else { panic!("{nested:?}") };
    assert!(field(&within[0], "instance").is_some(), "{within:?}");
    assert_ne!(field(&within[0], "instance"), field(nested, "instance"));

    // The table of reference points shows what the point states: the number
    // source with its instances, outermost first.
    let points =
        common::stdcore::projection(&run, flm_core::stdcore::features::refpoints::REFPOINTS);
    let written = format!("{points:?}");
    assert!(written.contains("instances"), "{written}");
}

#[test]
fn instances_are_numbered_the_same_way_however_the_run_is_executed() {
    let registry = list_registry(None);
    let sequential =
        run_with(&registry, LISTS, NumberingOptions::new(), None, &RunOptions::sequential());
    let parallel = run_with(&registry, LISTS, NumberingOptions::new(), None, &RunOptions::new());
    let all = ["a", "b", "ba", "bb", "bc", "c", "d", "da", "db", "dx", "dy", "e"];
    assert_eq!(items(&sequential, &all), items(&parallel, &all));
}

#[test]
fn instances_twenty_deep_are_numbered_without_trouble() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    let depth = 20;
    let mut text = String::new();
    for level in 0..depth {
        text.push_str(&format!("\\begin{{tlist}}\\titem{{l{level}}}"));
    }
    text.push_str(&"\\end{tlist}".repeat(depth));
    let nesting = WithinJoinerSpec::Joined(".".to_string());
    let run = run(&list_registry(Some(nesting)), &text, NumberingOptions::new());
    assert_eq!(reports(&run), []);
    let deepest = items(&run, &[&format!("l{}", depth - 1)]);
    assert_eq!(deepest, [format!("1.(a).{}i", "i.".repeat(depth - 3))]);
}

// ---------------------------------------------------------------------------------------------
// the template a point carries
// ---------------------------------------------------------------------------------------------

#[test]
fn a_point_carrying_a_number_template_is_written_through_it_everywhere() {
    let text = "\\begin{tlist}\\titem{a}\\titemalph{b}\\titemalph{c}\\titem{d}\\end{tlist}";
    let run = bare_run(text);
    assert_eq!(reports(&run), []);
    // The template writes the point's own number; the count is the list's.
    assert_eq!(items(&run, &["a", "b", "c", "d"]), ["1", "(b)", "(c)", "4"]);
    // The record holds the derived style under the declared name, and a
    // reference to the point writes the same number with the style's word.
    let counters = counters_of(&run);
    let record = counters.record(item(&run, "b")).expect("numbered");
    assert_eq!(record.style.name().as_str(), "t.style.item1");
    let with_word = counters
        .format(item(&run, "b"), &FormatOptions::new())
        .expect("a record")
        .expect("written");
    assert_eq!(written(&with_word), "Item~(b)");
    // The table of reference points shows the template the point states.
    let points =
        common::stdcore::projection(&run, flm_core::stdcore::features::refpoints::REFPOINTS);
    assert!(format!("{points:?}").contains("(#{alph})"), "{points:?}");
}

#[test]
fn points_under_one_template_group_together_and_apart_from_the_declared_style() {
    let text = "\\begin{tlist}\\titem{a}\\titemalph{b}\\titemalph{c}\\titemalph{d}\
                \\titemroman{e}\\titem{f}\\end{tlist}";
    let run = bare_run(text);
    assert_eq!(reports(&run), []);
    // One group per style value: the three points of one template collapse
    // into a range, and the declared style's points and the other template's
    // are groups of their own, in the order first met.
    assert_eq!(
        items_together(&run, &["a", "b", "c", "d", "e", "f"]),
        ["Items~1,6", "Items~(b)\u{2013}(d)", "Item~V"]
    );
    // One style value per distinct text and run: the points share it.
    let counters = counters_of(&run);
    let style_of = |text: &str| Arc::clone(&counters.record(item(&run, text)).expect("numbered").style);
    assert!(Arc::ptr_eq(&style_of("b"), &style_of("d")));
    assert!(!Arc::ptr_eq(&style_of("a"), &style_of("b")));
    assert!(Arc::ptr_eq(&style_of("a"), &style_of("f")));
}

#[test]
fn an_enclosing_item_is_written_in_the_template_it_carried() {
    use flm_core::stdcore::features::numbering::WithinJoinerSpec;
    let text = "\\begin{tlist}\\titem{a}\\titemroman{b}\\begin{tlist}\\titem{ba}\\end{tlist}\
                \\end{tlist}";
    let nesting = WithinJoinerSpec::Template("#{_parent}.".to_string());
    let run = run(&list_registry(Some(nesting)), text, NumberingOptions::new());
    assert_eq!(reports(&run), []);
    assert_eq!(items(&run, &["a", "b", "ba"]), ["1", "II", "II.(a)"]);
}

#[test]
fn a_number_template_that_does_not_compile_is_an_error_and_the_declared_style_writes_the_point() {
    let text = "\\begin{tlist}\\titem{a}\\titembad{b}\\titembad{c}\\end{tlist}";
    let run = bare_run(text);
    // Once for the text, at the first point carrying it.
    assert_eq!(reports(&run), [error("flm.numbering.bad-number-format")]);
    assert_eq!(items(&run, &["a", "b", "c"]), ["1", "2", "3"]);
    let record = counters_of(&run).record(item(&run, "b")).expect("numbered");
    assert!(Arc::ptr_eq(
        &record.style,
        &counters_of(&run).record(item(&run, "a")).expect("numbered").style
    ));
}
