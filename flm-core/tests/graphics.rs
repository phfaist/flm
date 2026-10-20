//! The `graphics` feature (`flm_core::stdcore::features::graphics`) on the
//! text and HTML backends: the image staged from `\includegraphics` (test 1),
//! its options and their refusals (test 2), a path read as characters (test
//! 3), the record and the inspector (test 4), the size an image is shown at
//! (test 5), the text and HTML outputs (tests 6 and 7), where the command may
//! stand and whether it is a block or inline (test 9), documents that must
//! neither panic nor loop (test 21, the
//! graphics half), and a heading holding an image identified from its words
//! (test 23).

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flm_core::core::lowering::{DefaultMark, LowerError, Lowering};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::mint::{ArgDef, BodyDef};
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::addr::TreeId;
use flm_core::model::names::ElementName;
use flm_core::stdcore::backends::lowering::lower_run;
use flm_core::stdcore::backends::pres::{
    self, BlockPayload, I48F16, ImageDisplay, ImagePayload, Length, LengthUnit, payload_of,
};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::floats::{FloatType, FloatsConfig};
use flm_core::stdcore::features::graphics::{
    self, Dimensions, GraphicsConfig, GraphicsInspectError, GraphicsInspectRequest,
    Factor, GraphicsInspectedInformation, GraphicsInspector, GraphicsRecord, GraphicsRenderOptions,
    INCLUDEGRAPHICS_PACKAGE, INFO, INSPECTOR_SLOT, Magnification, PixelSize,
};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{StdcoreConfig, package};
use techy::error::Severity;

use common::lowering::{lowered, text as printed};
use common::stdcore::{identifiers, reports, reports_but_unknown_size, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries, runs and what a run staged
// ---------------------------------------------------------------------------------------------

/// The configuration under which `\includegraphics` is an ordinary command.
fn global() -> StdcoreConfig {
    StdcoreConfig::new().graphics(GraphicsConfig::new().global(true))
}

/// A test inspector: an answer, or an error, per path; a path it does not
/// know is an error.
struct TestInspector(BTreeMap<String, Result<GraphicsInspectedInformation, String>>);

impl GraphicsInspector for TestInspector {
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
        match self.0.get(&request.path) {
            Some(Ok(answer)) => Ok(*answer),
            Some(Err(detail)) => Err(GraphicsInspectError::new(detail.clone())),
            None => Err(GraphicsInspectError::new("no such file")),
        }
    }
}

/// The registry of the standard package under `config`, with `features`
/// added after it and `inspector` contributed to the slot where given.
fn registry_with(
    config: StdcoreConfig,
    features: Vec<Arc<dyn FlmFeature>>,
    inspector: Option<TestInspector>,
) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    for feature in features {
        builder.feature(feature);
    }
    if let Some(inspector) = inspector {
        builder.contribute_to(INSPECTOR_SLOT, Arc::new(inspector) as Arc<dyn GraphicsInspector>);
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The standard package under `global`, with no inspector.
fn std_global() -> Arc<FlmRegistry> {
    registry_with(global(), Vec::new(), None)
}

/// The standard package under `global`, with `inspector`.
fn inspected(inspector: TestInspector) -> Arc<FlmRegistry> {
    registry_with(global(), Vec::new(), Some(inspector))
}

/// Runs `workflow` of `registry` over `document`.
fn render(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> Run {
    run_workflow(registry, workflow, &[document], &Overrides::none())
}

/// The payload of every image the text backend staged for `document`, in
/// document order.
fn images(run: &Run) -> Vec<ImagePayload> {
    let tree = &lowered(run, TreeId::tree(0)).tree;
    tree.root()
        .descendants()
        .filter_map(|node| payload_of::<ImagePayload>(&node).cloned())
        .collect()
}

/// The one image the text backend staged for `document` under `registry`,
/// and the run's reports.
fn one_image(registry: &Arc<FlmRegistry>, document: &str) -> (ImagePayload, Vec<(String, Severity)>) {
    let run = render(registry, text::WORKFLOW, document);
    let mut found = images(&run);
    assert_eq!(found.len(), 1, "{document}: {found:?}\n{}", run.diagnostics().render_all());
    (found.remove(0), reports(&run))
}

/// A length.
fn length(text: &str) -> Length {
    Length::parse(text).expect("a length")
}

/// Two lengths as dimensions.
fn dims(width: &str, height: &str) -> Dimensions {
    Dimensions { width: length(width), height: length(height) }
}

/// A report of the feature: `flm.graphics.<condition>` at `severity`.
fn report(condition: &str, severity: Severity) -> (String, Severity) {
    (format!("flm.graphics.{condition}"), severity)
}

/// The Warning every accepted `width=` or `height=` draws.
fn resized() -> (String, Severity) {
    report("resized", Severity::Warning)
}

/// The Warning every image of no known size draws: one given neither
/// `width=` nor `height=` whose file no inspector measured.
fn unknown_size() -> (String, Severity) {
    report("unknown-size", Severity::Warning)
}

// ---------------------------------------------------------------------------------------------
// test 1: one image
// ---------------------------------------------------------------------------------------------

#[test]
fn an_image_stages_one_image_with_its_path_and_nothing_else() {
    let (image, reports) = one_image(&std_global(), "\\includegraphics{figures/plot.png}");
    assert_eq!(
        image,
        ImagePayload { src: String::from("figures/plot.png"), ..ImagePayload::default() }
    );
    // No inspector measures the file, and no option gives a size.
    assert_eq!(reports, [unknown_size()]);
}

// ---------------------------------------------------------------------------------------------
// test 2: the options
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_options_fill_the_payload_each_dimension_with_a_warning() {
    let (image, reports) =
        one_image(&std_global(), "\\includegraphics[alt=A plot, width=5cm, height=2in]{plot.png}");
    assert_eq!(image.alt.as_deref(), Some("A plot"));
    assert_eq!(image.width, Some(length("5cm")));
    assert_eq!(image.height, Some(length("2in")));
    assert_eq!(reports, [resized(), resized()]);
    // A value in braces is read without them; `alt=` may hold commas there.
    let (image, _) = one_image(&std_global(), "\\includegraphics[alt={a, b}]{plot.png}");
    assert_eq!(image.alt.as_deref(), Some("a, b"));
}

#[test]
fn a_refused_option_is_an_error_and_the_image_is_still_shown() {
    let error = |condition: &str| report(condition, Severity::Error);
    for (options, expected, alt, width) in [
        ("foo=1", vec![error("unknown-key")], None, None),
        ("alt=a, alt=b", vec![error("repeated-key")], Some("a"), None),
        ("width=1cm, width=2cm", vec![resized(), error("repeated-key")], None, Some("1cm")),
        ("width", vec![error("missing-value")], None, None),
        ("=5cm", vec![error("missing-key")], None, None),
        ("alt=a, =, width=1cm", vec![error("missing-key"), resized()], Some("a"), Some("1cm")),
        // An empty entry says nothing.
        (",alt=a,,", vec![], Some("a"), None),
        ("alt=see \\(x\\)", vec![error("alt-not-text")], None, None),
        ("width=300px", vec![error("not-a-length")], None, None),
        // `\\linewidth` is a command no feature declares, reported by the parse.
        (
            "width=0.5\\linewidth",
            vec![(String::from("core.specs.unresolvable-command"), Severity::Error), error("not-a-length")],
            None,
            None,
        ),
        // A `%` outside a verbatim argument is refused by the parse.
        (
            "width=50%",
            vec![(String::from("flm.lang.percent-forbidden"), Severity::Error), error("not-a-length")],
            None,
            None,
        ),
        ("width=10bp", vec![error("not-a-length")], None, None),
        ("width=", vec![error("not-a-length")], None, None),
        ("width=0cm", vec![error("not-positive")], None, None),
        ("height=-1in", vec![error("not-positive")], None, None),
        ("width=1.5em", vec![error("relative-unit")], None, None),
        ("height=2tab", vec![error("relative-unit")], None, None),
        ("\\emph{x}=1", vec![error("unreadable-options")], None, None),
    ] {
        let document = format!("\\includegraphics[{options}]{{plot.png}}");
        let (image, reports) = one_image(&std_global(), &document);
        // With no dimension taken, the image's size is not known.
        let mut expected = expected;
        if width.is_none() {
            expected.push(unknown_size());
        }
        assert_eq!(reports, expected, "{options}");
        assert_eq!(image.src, "plot.png", "{options}");
        assert_eq!(image.alt.as_deref(), alt, "{options}");
        assert_eq!(image.width, width.map(length), "{options}");
        assert_eq!(image.height, None, "{options}");
    }
}

#[test]
fn a_dimension_in_em_or_tab_is_refused_whatever_the_inspector_reports() {
    let inspector = TestInspector(BTreeMap::from([(
        String::from("plot.png"),
        Ok(GraphicsInspectedInformation::Vector { physical_dimensions: dims("340pt", "170pt") }),
    )]));
    let registry = inspected(inspector);
    for options in ["width=1.5em", "height=2tab"] {
        let document = format!("\\includegraphics[{options}]{{plot.png}}");
        let (image, reports) = one_image(&registry, &document);
        assert_eq!(reports, [report("relative-unit", Severity::Error)], "{options}");
        // The key ignored: the inspected size, as without it.
        assert_eq!((image.width, image.height), (Some(length("340pt")), Some(length("170pt"))));
    }
}

#[test]
fn each_message_states_the_condition() {
    let run = render(
        &std_global(),
        text::WORKFLOW,
        "\\includegraphics[foo=1, width=1.5em, height=300px, alt=a, alt=b, width=2cm]{p.png}",
    );
    let messages: Vec<String> = run.diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert_eq!(
        messages,
        [
            "\\includegraphics accepts only the options `alt`, `width` and `height`, not `foo`",
            "the `width` of an image must be given in pt, mm, cm or in, not in `em`",
            "the `height` of an image must be a number followed directly by one of the units pt, \
             mm, cm or in, such as `5cm`, not `300px`",
            "the option `alt` of \\includegraphics may be given only once",
            "the option `width` of \\includegraphics may be given only once",
            "the size of the image `p.png` must be known from its file or given with `width` or \
             `height`",
        ]
    );
    let run = render(&std_global(), text::WORKFLOW, "\\includegraphics[alt, =1cm]{p.png}");
    let messages: Vec<String> = run.diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert_eq!(
        messages,
        [
            "the option `alt` of \\includegraphics must be given a value",
            "each option of \\includegraphics must give its key before `=`",
            "the size of the image `p.png` must be known from its file or given with `width` or \
             `height`",
        ]
    );
    let run = render(&std_global(), text::WORKFLOW, "\\includegraphics[width=2cm]{p.png}");
    let messages: Vec<String> = run.diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert_eq!(
        messages,
        ["an image file should be prepared at the size it is shown at, rather than resized with \
          `width=`"]
    );
}

// ---------------------------------------------------------------------------------------------
// test 3: the path is raw text
// ---------------------------------------------------------------------------------------------

/// A path holding every character a verbatim argument keeps.
const HARD_PATH: &str = "a%b#c~d_e{f}.png";

#[test]
fn a_path_is_read_as_characters_and_reaches_every_output_unchanged() {
    let document = format!("\\includegraphics{{{HARD_PATH}}}");
    let registry = std_global();
    let (image, reports) = one_image(&registry, &document);
    assert_eq!(image.src, HARD_PATH);
    assert_eq!(reports, [unknown_size()]);
    assert_eq!(printed(&render(&registry, text::WORKFLOW, &document)), format!("[image: {HARD_PATH}]\n"));
    let page = common::html::fragment(&render(&registry, html::WORKFLOW, &document));
    assert!(page.contains(&format!("src=\"{HARD_PATH}\"")), "{page}");
}

#[test]
fn two_images_in_one_paragraph_flow_and_two_in_a_float_stack_as_blocks() {
    // Two images written in one paragraph are two words of it: one line.
    let run = render(&std_global(), text::WORKFLOW, "\\includegraphics{a.png} \\includegraphics{b.png}");
    assert_eq!(printed(&run), "[image: a.png] [image: b.png]\n");
    assert!(images(&run).iter().all(|image| image.display == ImageDisplay::Inline));
    // Standing directly in the body of a float admitting any content, each is
    // a block: a line of its own, and a block of the page.
    let any = global().floats(FloatsConfig::new().float_type(FloatType::figure().handlers(["any"])));
    let registry = registry_with(any, Vec::new(), None);
    let document = "\\begin{figure}\\includegraphics{a.png}\n\\includegraphics{b.png}\\end{figure}";
    let run = render(&registry, text::WORKFLOW, document);
    assert_eq!(reports_of(&run), [unknown_size(), unknown_size()], "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "[image: a.png]\n\n[image: b.png]\n");
    assert!(images(&run).iter().all(|image| image.display == ImageDisplay::Block));
    let page = common::html::fragment(&render(&registry, html::WORKFLOW, document));
    assert!(
        page.contains("<img class=\"flm-pres-block flm-pres-image\" src=\"a.png\">")
            && page.contains("<img class=\"flm-pres-block flm-pres-image\" src=\"b.png\">"),
        "{page}"
    );
    assert!(!page.contains("<p"), "{page}");
}

/// The standard package under `global` with `\fig{name}` defined as
/// `\includegraphics{figs/#1.png}`.
fn with_fig() -> Arc<FlmRegistry> {
    let fig = Definition::macro_("fig", vec![ArgDef::mandatory("name")])
        .text("\\includegraphics{figs/#1.png}");
    registry_with(global().userdefs(UserdefsConfig::new().definitions(vec![fig])), Vec::new(), None)
}

#[test]
fn a_placeholder_in_a_path_takes_the_use_sites_characters() {
    let registry = with_fig();
    let (image, reports) = one_image(&registry, "\\fig{plot}");
    assert_eq!(image.src, "figs/plot.png");
    assert_eq!(reports, [unknown_size()]);
    let page = common::html::fragment(&render(&registry, html::WORKFLOW, "\\fig{plot}"));
    assert!(page.contains("src=\"figs/plot.png\""), "{page}");
    // A command passed into the path is the feature's error, and no image.
    let run = render(&registry, text::WORKFLOW, "\\fig{\\emph{x}}");
    assert_eq!(reports_of(&run), [report("path-not-characters", Severity::Error)]);
    assert!(images(&run).is_empty());
}

/// The reports of a run.
fn reports_of(run: &Run) -> Vec<(String, Severity)> {
    reports(run)
}

// ---------------------------------------------------------------------------------------------
// test 4: the record and the inspector
// ---------------------------------------------------------------------------------------------

/// The records of the run, in document order of their node addresses.
fn records(run: &Run) -> Vec<GraphicsRecord> {
    let info = run.table(INFO).expect("declared").expect("written");
    info.iter().map(|(_, record)| record.clone()).collect()
}

#[test]
fn without_an_inspector_a_record_holds_the_path_the_origin_and_no_answer() {
    let registry = std_global();
    let source = Arc::new(FlmSource::new("\\includegraphics{a.png}").with_origin(FlmOrigin::new("chapter.flm")));
    let parsed = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the parse yields a tree");
    let mut content = DocContent::new();
    content.place(Arc::new(parsed));
    let doc = Arc::new(Doc::new(content));
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let run = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    assert_eq!(identifiers(&run), ["flm.graphics.unknown-size"]);
    let found = records(&run);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "a.png");
    assert_eq!(found[0].origin.canonical_path(), Some("chapter.flm"));
    assert_eq!(found[0].inspected, None);
}

/// The inspector of test 4.
fn test_four_inspector() -> TestInspector {
    let raster = |dpi: Option<i32>, physical: Option<Dimensions>| GraphicsInspectedInformation::Raster {
        dpi: dpi.map(I48F16::from_num),
        size: PixelSize { width: 300, height: 150 },
        physical_dimensions: physical,
    };
    TestInspector(BTreeMap::from([
        (String::from("dpi.png"), Ok(raster(Some(150), None))),
        (String::from("bare.png"), Ok(raster(None, None))),
        (String::from("stated.png"), Ok(raster(Some(300), Some(dims("3cm", "1.5cm"))))),
        (
            String::from("drawing.svg"),
            Ok(GraphicsInspectedInformation::Vector { physical_dimensions: dims("4in", "3in") }),
        ),
        (String::from("broken.png"), Err(String::from("not an image file"))),
        (
            String::from("empty.png"),
            Ok(GraphicsInspectedInformation::Raster {
                dpi: None,
                size: PixelSize { width: 0, height: 0 },
                physical_dimensions: None,
            }),
        ),
        (
            String::from("ems.svg"),
            Ok(GraphicsInspectedInformation::Vector { physical_dimensions: dims("2em", "1em") }),
        ),
    ]))
}

#[test]
fn a_raster_answer_is_recorded_as_retrieved_and_completed() {
    let registry = inspected(test_four_inspector());
    let run = render(
        &registry,
        text::WORKFLOW,
        "\\includegraphics{dpi.png}\n\n\\includegraphics{stated.png}\n\n\
         \\includegraphics{drawing.svg}",
    );
    assert_eq!(reports_of(&run), []);
    let answers: Vec<Option<GraphicsInspectedInformation>> =
        records(&run).into_iter().map(|record| record.inspected).collect();
    assert_eq!(
        answers,
        [
            // 300 × 150 pixels at 150 dots per inch: 144pt × 72pt.
            Some(GraphicsInspectedInformation::Raster {
                dpi: Some(I48F16::from_num(150)),
                size: PixelSize { width: 300, height: 150 },
                physical_dimensions: Some(dims("144pt", "72pt")),
            }),
            // A stated size kept in its own unit.
            Some(GraphicsInspectedInformation::Raster {
                dpi: Some(I48F16::from_num(300)),
                size: PixelSize { width: 300, height: 150 },
                physical_dimensions: Some(dims("3cm", "1.5cm")),
            }),
            Some(GraphicsInspectedInformation::Vector { physical_dimensions: dims("4in", "3in") }),
        ]
    );
}

#[test]
fn no_resolution_is_taken_at_96_with_one_warning_per_image() {
    let registry = inspected(test_four_inspector());
    let run = render(&registry, text::WORKFLOW, "\\includegraphics{bare.png}\n\n\\includegraphics{bare.png}");
    assert_eq!(
        reports_of(&run),
        [report("assumed-resolution", Severity::Warning), report("assumed-resolution", Severity::Warning)]
    );
    for record in records(&run) {
        assert_eq!(
            record.inspected,
            Some(GraphicsInspectedInformation::Raster {
                dpi: Some(I48F16::from_num(96)),
                size: PixelSize { width: 300, height: 150 },
                physical_dimensions: Some(dims("225pt", "112.5pt")),
            })
        );
    }
    assert!(
        run.diagnostics().iter().all(|d| d.message() == "the image file `bare.png` should state its \
                                                          resolution in dots per inch"),
        "{}",
        run.diagnostics().render_all()
    );
}

/// An inspector that counts the requests it answers, answering each as
/// `inner` does.
struct Counting(TestInspector, Arc<AtomicUsize>);

impl GraphicsInspector for Counting {
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0.inspect(request)
    }
}

#[test]
fn an_image_shown_again_by_a_reference_reports_at_each_showing() {
    // A reference to an unnumbered heading shows its title again; the image
    // the title holds reports where it is written and again at each
    // reference, and the inspector is asked once per tree holding it.
    let document = "\\section*{Plot \\includegraphics[width=5cm]{bare.png}}\\label{sec:c}\n\n\
                    See \\ref{sec:c} and \\ref{sec:c}.";
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let asked = Arc::new(AtomicUsize::new(0));
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(global()));
        builder.use_package(flm_core::math_passthrough::package());
        let inspector = Counting(test_four_inspector(), Arc::clone(&asked));
        builder.contribute_to(INSPECTOR_SLOT, Arc::new(inspector) as Arc<dyn GraphicsInspector>);
        let registry = builder.build(&RegistryInputs::new()).expect("a registry");
        let run = render(&registry, workflow, document);
        let mut found = reports_of(&run);
        found.sort();
        let assumed = report("assumed-resolution", Severity::Warning);
        assert_eq!(
            found,
            [assumed.clone(), assumed.clone(), assumed, resized(), resized(), resized()],
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
        assert_eq!(asked.load(Ordering::SeqCst), 3, "{workflow}");
        // The report where the image is written has no frame; two of each
        // are raised inside the copy one `\ref{sec:c}` shows, and name it.
        let first = document.find("\\ref{sec:c}").expect("a first reference");
        let second = document.rfind("\\ref{sec:c}").expect("a second reference");
        for condition in ["assumed-resolution", "resized"] {
            let mut shown: Vec<Vec<(String, usize)>> = reports_of(&run)
                .into_iter()
                .zip(common::stdcore::frames(&run))
                .filter(|(found, _)| found.0.ends_with(condition))
                .map(|(_, frames)| frames)
                .collect();
            shown.sort();
            let at = |offset| vec![(String::from(common::stdcore::SHOWN_HERE), offset)];
            assert_eq!(shown, [vec![], at(first), at(second)], "{workflow}: {condition}");
        }
    }
}

#[test]
fn an_answer_that_cannot_be_recorded_is_one_error_naming_the_path() {
    let registry = inspected(test_four_inspector());
    for (path, condition, message) in [
        ("broken.png", "inspect-failed", "the image file `broken.png` could not be examined: not an image file"),
        ("missing.png", "inspect-failed", "the image file `missing.png` could not be examined: no such file"),
        ("empty.png", "no-pixel-size", "the size in pixels reported for the image file `empty.png` must be greater than zero"),
        (
            "ems.svg",
            "inspected-size-not-physical",
            "the size reported for the image file `ems.svg` must be in pt, mm, cm or in, not in `em`",
        ),
    ] {
        let run = render(&registry, text::WORKFLOW, &format!("\\includegraphics{{{path}}}"));
        // Nothing recorded, the image's size is not known.
        assert_eq!(reports_of(&run), [report(condition, Severity::Error), unknown_size()], "{path}");
        assert_eq!(run.diagnostics().iter().next().map(|d| d.message().to_string()).as_deref(), Some(message));
        let found = records(&run);
        assert_eq!((found[0].path.as_str(), found[0].inspected), (path, None));
        // The image is still shown.
        assert_eq!(images(&run).len(), 1);
    }
}

// ---------------------------------------------------------------------------------------------
// test 5: the size an image is shown at
// ---------------------------------------------------------------------------------------------

#[test]
fn one_dimension_given_scales_the_other_from_the_inspected_size() {
    let answer = GraphicsInspectedInformation::Vector { physical_dimensions: dims("340pt", "170pt") };
    let registry = inspected(TestInspector(BTreeMap::from([(String::from("p.svg"), Ok(answer))])));
    let shown = |options: &str| {
        let (image, _) = one_image(&registry, &format!("\\includegraphics[{options}]{{p.svg}}"));
        (image.width.map(|l| l.to_string()), image.height.map(|l| l.to_string()))
    };
    let pair = |w: &str, h: &str| (Some(String::from(w)), Some(String::from(h)));
    assert_eq!(shown("width=5cm"), pair("5cm", "2.5cm"));
    assert_eq!(shown("height=2.5cm"), pair("5cm", "2.5cm"));
    assert_eq!(shown("width=1in,height=3in"), pair("1in", "3in"));
    assert_eq!(shown(""), pair("340pt", "170pt"));
    // The record is unchanged by the options.
    let run = render(&registry, text::WORKFLOW, "\\includegraphics[width=5cm]{p.svg}");
    assert_eq!(records(&run)[0].inspected, Some(answer));
    // With no inspected size, the given dimension alone.
    let (image, _) = one_image(&std_global(), "\\includegraphics[width=5cm]{p.svg}");
    assert_eq!((image.width, image.height), (Some(length("5cm")), None));
    assert_eq!(length("5cm").unit, LengthUnit::Centimeters);
}

/// The render options magnifying the images of `output` by `raster` and
/// `vector`.
fn magnified(output: &str, raster: f64, vector: f64) -> Overrides {
    let mut chosen = GraphicsRenderOptions::default();
    let factor = |value: f64| Factor::try_from(value).expect("a factor");
    chosen
        .magnification
        .insert(output.to_string(), Magnification { raster: factor(raster), vector: factor(vector) });
    let mut values = OptionValues::new();
    values.set(chosen);
    Overrides { render_options: values, ..Overrides::none() }
}

#[test]
fn each_output_magnifies_raster_and_vector_images_by_its_own_factors() {
    let registry = inspected(test_four_inspector());
    // dpi.png is a raster image of 144pt × 72pt, drawing.svg a vector image of
    // 4in × 3in.
    let document = "\\includegraphics{dpi.png}\n\n\\includegraphics{drawing.svg}\n\n\
                    \\includegraphics[width=2cm]{dpi.png}";
    let overrides = magnified(html::BACKEND, 1.5, 0.5);
    let page = |overrides: &Overrides| {
        let run = run_workflow(&registry, html::WORKFLOW, &[document], overrides);
        assert_eq!(reports_of(&run), [resized()], "{}", run.diagnostics().render_all());
        common::html::fragment(&run)
    };
    let shown = page(&overrides);
    assert!(shown.contains("--flm-element-width: 216pt; --flm-element-height: 108pt"), "{shown}");
    assert!(shown.contains("--flm-element-width: 2in; --flm-element-height: 1.5in"), "{shown}");
    // The width the author gave is magnified too, and the height follows.
    assert!(shown.contains("--flm-element-width: 3cm; --flm-element-height: 1.5cm"), "{shown}");
    // Another output keeps its own factors, 1 by default.
    let text_run = run_workflow(&registry, text::WORKFLOW, &[document], &overrides);
    let sizes: Vec<_> = images(&text_run).into_iter().map(|image| (image.width, image.height)).collect();
    assert_eq!(
        sizes,
        [
            (Some(length("144pt")), Some(length("72pt"))),
            (Some(length("4in")), Some(length("3in"))),
            (Some(length("2cm")), Some(length("1cm"))),
        ]
    );
    let unmagnified = page(&Overrides::none());
    assert!(unmagnified.contains("--flm-element-width: 144pt; --flm-element-height: 72pt"), "{unmagnified}");
}

#[test]
fn an_image_of_unknown_type_takes_the_vector_factor() {
    let overrides = magnified(text::BACKEND, 3.0, 2.0);
    let run = run_workflow(&std_global(), text::WORKFLOW, &["\\includegraphics[height=1in]{a.png}"], &overrides);
    let found = images(&run);
    assert_eq!((found[0].width, found[0].height), (None, Some(length("2in"))));
    // With no size at all, nothing to magnify, and the Warning.
    let run = run_workflow(&std_global(), text::WORKFLOW, &["\\includegraphics{a.png}"], &overrides);
    assert_eq!((images(&run)[0].width, images(&run)[0].height), (None, None));
    assert_eq!(reports_of(&run), [unknown_size()]);
}

#[test]
fn an_image_of_unknown_size_is_reported_on_every_output() {
    // A web address is never inspected, even with an inspector registered.
    let registry = inspected(test_four_inspector());
    for document in ["\\includegraphics{https://example.org/a.png}", "\\includegraphics{missing.png}"] {
        for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
            let run = render(&registry, workflow, document);
            let found: Vec<_> = reports_of(&run).into_iter().filter(|found| *found == unknown_size()).collect();
            assert_eq!(found, [unknown_size()], "{document} under {workflow}");
        }
    }
    let run = render(&registry, text::WORKFLOW, "\\includegraphics{https://example.org/a.png}");
    assert_eq!(
        run.diagnostics().iter().map(|d| d.message().to_string()).collect::<Vec<_>>(),
        ["the size of the image `https://example.org/a.png` must be known from its file or given \
          with `width` or `height`"]
    );
    // A dimension given, or a size inspected, is a known size.
    for document in ["\\includegraphics[width=1cm]{https://example.org/a.png}", "\\includegraphics{drawing.svg}"] {
        let run = render(&registry, text::WORKFLOW, document);
        assert!(!reports_of(&run).contains(&unknown_size()), "{document}");
    }
}

// ---------------------------------------------------------------------------------------------
// tests 6 and 7: the text and the HTML output
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_output_shows_the_alternative_text_or_the_path() {
    let registry = std_global();
    let text_of = |document: &str| printed(&render(&registry, text::WORKFLOW, document));
    assert_eq!(
        text_of("\\includegraphics[alt=A plot, width=5cm, height=2in]{figures/plot.png}"),
        "[image: A plot]\n"
    );
    assert_eq!(text_of("\\includegraphics{figures/plot.png}"), "[image: figures/plot.png]\n");
}

#[test]
fn the_page_writes_the_two_dimensions_as_custom_properties() {
    let registry = std_global();
    let run = render(
        &registry,
        html::WORKFLOW,
        "\\includegraphics[alt=A plot, width=5cm, height=2in]{figures/plot.png}\n\n\
         \\includegraphics[width=5cm]{figures/wide.png}\n\n\
         \\includegraphics[height=3cm]{figures/tall.png}\n\n\
         \\includegraphics{figures/natural.png}",
    );
    let page = common::html::fragment(&run);
    assert!(
        page.contains(
            "<img class=\"flm-pres-image\" src=\"figures/plot.png\" alt=\"A plot\" \
             style=\"--flm-element-width: 5cm; --flm-element-height: 2in; \
             --flm-element-aspect-ratio: 141.73228 / 144\">"
        ),
        "{page}"
    );
    assert!(page.contains("src=\"figures/wide.png\" style=\"--flm-element-width: 5cm\">"), "{page}");
    assert!(page.contains("src=\"figures/tall.png\" style=\"--flm-element-height: 3cm\">"), "{page}");
    assert!(page.contains("<img class=\"flm-pres-image\" src=\"figures/natural.png\">"), "{page}");
    common::html::assert_golden("graphics-images.html", &page);
    // The core chunk reads the properties.
    let page = common::html::page(&render(&registry, html::WORKFLOW, "\\includegraphics{a.png}"));
    for property in ["--flm-element-width", "--flm-element-height", "--flm-element-aspect-ratio"] {
        assert!(page.contains(&format!("var({property}")), "{property}");
    }
}

#[test]
fn a_refused_address_scheme_is_reported_and_writes_no_src() {
    let run = render(&std_global(), html::WORKFLOW, "\\includegraphics{javascript:alert(1)}");
    assert_eq!(
        reports_but_unknown_size(&run),
        [(String::from("flm.html.refused-address"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    let page = common::html::fragment(&run);
    assert!(page.contains("<img class=\"flm-pres-image\">"), "{page}");
}

// ---------------------------------------------------------------------------------------------
// test 9: where the command may stand
// ---------------------------------------------------------------------------------------------

#[test]
fn under_global_an_image_is_inline_content_wherever_text_stands() {
    let registry = std_global();
    let document = "Before.\n\n\\includegraphics{a.png}\n\nText \\includegraphics{b.png} inside.\n\n\
                    \\begin{itemize}\\item \\includegraphics{c.png}\\end{itemize}\n\n\
                    A\\footnote{First.\n\n\\includegraphics{d.png}}.\n\n\
                    \\begin{quote}\\block{\\includegraphics{e.png}}\\end{quote}\n\n\
                    \\begin{blockquote}\n\\includegraphics{f.png}\n\\end{blockquote}\n\n\
                    B\\footnote{See \\includegraphics{g.png} here.}.\n";
    let run = render(&registry, text::WORKFLOW, document);
    assert_eq!(reports_but_unknown_size(&run), [], "{}", run.diagnostics().render_all());
    // An image written between paragraphs is a paragraph holding it; in a
    // sentence it is a word of the sentence.
    assert_eq!(
        printed(&run),
        "Before.\n\n[image: a.png]\n\nText [image: b.png] inside.\n\n• [image: c.png]\n\nA¹.\n\n\
         [image: e.png]\n\n[image: f.png]\n\nB².\n\nFootnotes\n---------\n\n¹ First.\n\n  \
         [image: d.png]\n\n² See [image: g.png] here.\n"
    );
    let found: Vec<(String, ImageDisplay)> =
        images(&run).into_iter().map(|image| (image.src, image.display)).collect();
    let inline = |src: &str| (String::from(src), ImageDisplay::Inline);
    assert_eq!(found, ["a.png", "b.png", "c.png", "e.png", "f.png"].map(inline));
    // The same sentence on the three backends.
    let sentence = "See \\includegraphics[alt=A plot]{plot.png} here.\n\nNext.";
    assert_eq!(printed(&render(&registry, text::WORKFLOW, sentence)), "See [image: A plot] here.\n\nNext.\n");
    let page = common::html::fragment(&render(&registry, html::WORKFLOW, sentence));
    assert!(
        page.contains("<p class=\"flm-pres-par\">See <img class=\"flm-pres-image\" src=\"plot.png\" alt=\"A plot\"> here.</p>"),
        "{page}"
    );
    let latex = common::latex::body(&render(&registry, latex_recomposed::WORKFLOW, sentence));
    assert!(latex.contains("See \\flmIncludeGraphics{plot.png} here."), "{latex}");
    // A footnote's sentence on the three backends.
    let note = "B\\footnote{See \\includegraphics{g.png} here.}.";
    let run = render(&registry, text::WORKFLOW, note);
    assert_eq!(reports_but_unknown_size(&run), [], "{}", run.diagnostics().render_all());
    assert!(printed(&run).contains("¹ See [image: g.png] here.\n"), "{}", printed(&run));
    let page = common::html::page(&render(&registry, html::WORKFLOW, note));
    assert!(page.contains("See <img class=\"flm-pres-image\" src=\"g.png\"> here."), "{page}");
    let latex = common::latex::body(&render(&registry, latex_recomposed::WORKFLOW, note));
    assert!(latex.contains("See \\flmIncludeGraphics{g.png} here."), "{latex}");
}

#[test]
fn an_image_in_a_caption_is_inline_on_the_three_backends() {
    let registry = std_global();
    let document = "\\begin{figure}\\includegraphics{a.png}\\caption{A plot \\includegraphics{i.png}}\\end{figure}";
    let run = render(&registry, text::WORKFLOW, document);
    assert_eq!(reports_but_unknown_size(&run), [], "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "[image: a.png]\n\nA plot [image: i.png]\n");
    let found: Vec<(String, ImageDisplay)> =
        images(&run).into_iter().map(|image| (image.src, image.display)).collect();
    assert_eq!(
        found,
        [(String::from("a.png"), ImageDisplay::Block), (String::from("i.png"), ImageDisplay::Inline)]
    );
    let run = render(&registry, html::WORKFLOW, document);
    assert_eq!(reports_but_unknown_size(&run), []);
    let page = common::html::fragment(&run);
    assert!(page.contains("<img class=\"flm-pres-block flm-pres-image\" src=\"a.png\">"), "{page}");
    assert!(page.contains("A plot <img class=\"flm-pres-image\" src=\"i.png\">"), "{page}");
    let run = render(&registry, latex_recomposed::WORKFLOW, document);
    assert_eq!(reports_but_unknown_size(&run), []);
    let latex = common::latex::body(&run);
    assert!(latex.contains("A plot \\flmIncludeGraphics{i.png}"), "{latex}");
}

/// A test feature: an environment `picturebox` whose body names the scoped
/// package of `\includegraphics`, lowered as a block around its body.
fn picturebox() -> Arc<dyn FlmFeature> {
    common::pipeline::feature("t", |b| {
        b.peer_scoped_package(INCLUDEGRAPHICS_PACKAGE, "the images a picture box holds")?;
        b.element(common::env_def(
            "t",
            "picturebox",
            BodyDef::new().blocks().scoped(INCLUDEGRAPHICS_PACKAGE),
        ))?;
        b.lowering_rule(common::lowering::rule(
            "t.picturebox.lower",
            "t.picturebox",
            common::lowering::produces(&[pres::BLOCK]),
            DefaultMark::Everywhere,
            |node, cx, lw| {
                let body: Vec<_> = node.body().map(|b| b.iter().collect()).unwrap_or_default();
                let content = lower_run(cx, lw, &body)?;
                let block = BlockPayload { role: Some("t.picturebox".into()), ..BlockPayload::default() };
                let staged = lw.pres_with(cx, &ElementName::from(pres::BLOCK), block, content)?;
                Ok::<Lowering, LowerError>(Lowering::Emit(vec![staged]))
            },
        ))
    })
}

#[test]
fn under_the_default_the_command_exists_only_in_a_body_naming_its_package() {
    let registry = registry_with(StdcoreConfig::new(), vec![picturebox()], None);
    let run = render(&registry, text::WORKFLOW, "\\begin{picturebox}\\includegraphics{a.png}\\end{picturebox}");
    assert_eq!(reports_but_unknown_size(&run), [], "{}", run.diagnostics().render_all());
    assert_eq!(images(&run).len(), 1);
    let run = render(&registry, text::WORKFLOW, "\\includegraphics{a.png}");
    assert_eq!(identifiers(&run), ["core.specs.unresolvable-command"]);
    assert!(images(&run).is_empty());
}

#[test]
fn one_element_declared_globally_and_in_its_package_is_admitted_and_answers_in_both() {
    // The language admits the command declared as an ordinary command and in
    // the scoped package at once: both declarations carry one element.
    let registry = registry_with(global(), vec![picturebox()], None);
    let run = render(
        &registry,
        text::WORKFLOW,
        "\\includegraphics{a.png}\n\n\\begin{picturebox}\\includegraphics{b.png}\\end{picturebox}",
    );
    assert_eq!(reports_but_unknown_size(&run), [], "{}", run.diagnostics().render_all());
    let found: Vec<String> = images(&run).into_iter().map(|image| image.src).collect();
    assert_eq!(found, ["a.png", "b.png"]);
    let tree = &lowered(&run, TreeId::tree(0)).tree;
    let names: Vec<String> = tree.root().descendants().map(|node| {
        flm_core::model::element::element_of(&node).name.to_string()
    }).filter(|name| name.starts_with("flm.graphics")).collect();
    assert!(names.is_empty(), "every image was lowered: {names:?}");
    let _ = graphics::IMAGE_ELEMENT;
}

#[test]
fn an_empty_path_is_an_error_and_shows_no_image() {
    let registry = inspected(test_four_inspector());
    for document in ["\\includegraphics{}", "\\includegraphics[width=5cm]{}", "A\n\n\\includegraphics{}\n\nB"] {
        let run = render(&registry, text::WORKFLOW, document);
        assert_eq!(reports_of(&run), [report("empty-path", Severity::Error)], "{document}");
        assert_eq!(
            run.diagnostics().iter().next().map(|d| d.message().to_string()).as_deref(),
            Some("the path of an image must not be empty")
        );
        assert!(images(&run).is_empty(), "{document}");
        assert!(records(&run).is_empty(), "{document}");
        assert!(!printed(&run).contains("[image"), "{document}");
        let run = render(&registry, html::WORKFLOW, document);
        assert_eq!(reports_of(&run), [report("empty-path", Severity::Error)], "{document}");
        assert!(!common::html::fragment(&run).contains("<img"), "{document}");
    }
}

#[test]
fn a_registry_without_a_backend_builds_and_shows_images_on_the_others() {
    for left_out in ["latex-recomposed", "html"] {
        let registry = common::stdcore::registry_over(common::stdcore::package_features_but(global(), left_out));
        let notes: Vec<String> = registry
            .build_diagnostics()
            .iter()
            .filter(|d| d.severity() == Severity::Note)
            .map(|d| d.to_string())
            .collect();
        assert!(
            notes.iter().any(|note| note.contains("`graphics`") && note.contains(left_out)),
            "{left_out}: {notes:#?}"
        );
        let (image, reports) = one_image(&registry, "\\includegraphics{a.png}");
        assert_eq!((image.src.as_str(), reports), ("a.png", vec![unknown_size()]), "{left_out}");
    }
}

// ---------------------------------------------------------------------------------------------
// test 21: documents that must neither panic nor loop; the three execution settings
// ---------------------------------------------------------------------------------------------

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let registry = inspected(test_four_inspector());
    let mut nested = String::from("\\includegraphics{deep.png}");
    for depth in 0..10 {
        nested = if depth % 2 == 0 {
            format!("\\begin{{itemize}}\\item {nested}\\end{{itemize}}")
        } else {
            format!("\\begin{{quote}}\\block{{{nested}}}\\end{{quote}}")
        };
    }
    for document in [
        "\\includegraphics{}",
        "\\includegraphics",
        "\\includegraphics[",
        "\\includegraphics[width=5cm",
        "\\includegraphics[]{}",
        "\\includegraphics[=]{x}",
        "\\includegraphics[,,]{x}",
        "\\includegraphics[width={}]{x}",
        "\\includegraphics[alt=]{x}",
        "\\includegraphics{{{{}}}}",
        "\\includegraphics[width=99999999999999999999999cm]{bare.png}",
        "\\includegraphics[height=0.00001pt]{dpi.png}",
        "\\section{\\includegraphics{x.png}}",
        "A\\footnote{\\includegraphics{x.png}}",
        nested.as_str(),
    ] {
        for workflow in [text::WORKFLOW, html::WORKFLOW] {
            let run = render(&registry, workflow, document);
            let _ = run.diagnostics().render_all();
        }
    }
    let run = render(&registry, text::WORKFLOW, &nested);
    assert_eq!(images(&run).len(), 1);
}

#[test]
fn the_three_execution_settings_give_one_output() {
    let registry = inspected(test_four_inspector());
    let texts = [
        "\\includegraphics[width=2cm]{dpi.png}",
        "\\includegraphics{bare.png}\n\n\\includegraphics{drawing.svg}",
        "\\includegraphics[alt=x]{broken.png}",
    ];
    let doc = common::pipeline::doc(&registry, &texts);
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let plan = common::stdcore::plan(&registry, workflow, &Overrides::none());
        let outputs: Vec<(String, Vec<String>)> = [RunOptions::sequential(), RunOptions::new()]
        .iter()
        .map(|options| {
            let run = common::pipeline::run_with(&doc, &plan, Externals::none(), options);
            let out = if workflow == text::WORKFLOW {
                printed(&run)
            } else {
                common::html::page(&run)
            };
            (out, identifiers(&run))
        })
        .collect();
        assert_eq!(outputs[0], outputs[1], "{workflow}");
    }
}

// ---------------------------------------------------------------------------------------------
// test 23: a heading holding an image is identified from its words
// ---------------------------------------------------------------------------------------------

#[test]
fn a_heading_holding_an_image_is_identified_from_its_words_alone() {
    let registry = std_global();
    let id = |page: &str| {
        let at = page.find(" id=\"").expect("an identifier");
        page[at + 5..at + 5 + page[at + 5..].find('"').expect("a quote")].to_string()
    };
    let alone = id(&common::html::fragment(&render(&registry, html::WORKFLOW, "\\section{Results }\n\nText.")));
    assert!(alone.starts_with("sec--Results~"), "{alone}");
    for (image, expected) in [
        ("\\includegraphics{x.png}", vec![unknown_size()]),
        ("\\includegraphics[alt=A plot, width=5cm]{x.png}", vec![resized()]),
        ("\\includegraphics[alt=A plot]{x.png}", vec![unknown_size()]),
    ] {
        let run = render(&registry, html::WORKFLOW, &format!("\\section{{Results {image}}}\n\nText."));
        // An image is inline content in a heading's title: legitimate.
        assert_eq!(reports_of(&run), expected, "{image}");
        // Neither the path nor the options, the alternative text among them,
        // are words of the title.
        let page = common::html::fragment(&run);
        assert_eq!(id(&page), alone, "{image}: {page}");
    }
    // The image a word of the title on the three backends.
    let heading = "\\section{Results \\includegraphics[alt=A plot]{x.png}}\n\nText.";
    let run = render(&registry, text::WORKFLOW, heading);
    assert_eq!(reports_of(&run), [unknown_size()], "{}", run.diagnostics().render_all());
    assert!(printed(&run).contains("Results [image: A plot]\n"), "{}", printed(&run));
    let page = common::html::fragment(&render(&registry, html::WORKFLOW, heading));
    assert!(page.contains("Results <img class=\"flm-pres-image\" src=\"x.png\" alt=\"A plot\">"), "{page}");
    let latex = common::latex::body(&render(&registry, latex_recomposed::WORKFLOW, heading));
    assert!(latex.contains("{Results \\flmIncludeGraphics{x.png}}"), "{latex}");
}
