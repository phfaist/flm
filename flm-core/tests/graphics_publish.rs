//! How the `graphics` feature publishes images (`flm_core::stdcore::features::graphics`):
//! the resolver, the strategies `reference`, `copy` and `data-uri`, the
//! conversion rules and the choice of a converter, and the artifacts an
//! output then holds — with a resolver, converters and a naming service of
//! the tests' own, which hold their files in memory.

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use std::collections::BTreeSet;

use flm_core::core::artifact::{
    Artifact, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, RelPath, ScopedOutput,
};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    CapabilityDef, FeatureChain, FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::graphics::{
    self, CONVERTERS_SLOT, ConversionOptions, ConversionRequest, ConversionRule,
    GraphicsConvertError, GraphicsConverter, GraphicsConfig, GraphicsRenderOptions,
    GraphicsResolveError, GraphicsResolveRequest, GraphicsResolver, INFO, ImageFile, ImageFormat,
    RESOLVER_SLOT, Strategy, published_table,
};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::error::Severity;

use common::stdcore::{plan, reports};

// ---------------------------------------------------------------------------------------------
// the tests' services
// ---------------------------------------------------------------------------------------------

/// A resolver over files held in memory, by the path as written; it counts
/// what it is asked.
#[derive(Default)]
struct MemoryFiles {
    files: BTreeMap<String, (&'static str, Vec<u8>)>,
    asked: AtomicUsize,
}

impl MemoryFiles {
    fn with(mut self, path: &str, media_type: &'static str, content: &[u8]) -> MemoryFiles {
        self.files.insert(path.to_string(), (media_type, content.to_vec()));
        self
    }
}

impl GraphicsResolver for MemoryFiles {
    fn resolve(&self, request: &GraphicsResolveRequest) -> Result<ImageFile, GraphicsResolveError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        match self.files.get(&request.path) {
            Some((media_type, _)) => {
                Ok(ImageFile::new(format!("mem:{}", request.path), *media_type, &request.path))
            }
            None => Err(GraphicsResolveError::new("no such file")),
        }
    }

    fn read(&self, file: &ImageFile) -> Result<Vec<u8>, GraphicsResolveError> {
        let path = file.identity.strip_prefix("mem:").unwrap_or(&file.identity);
        self.files
            .get(path)
            .map(|(_, content)| content.clone())
            .ok_or_else(|| GraphicsResolveError::new("gone"))
    }
}

/// A namer placing every copy of an image in `files/`, named `<name>.<extension>`, and
/// every other artifact (the recomposed LaTeX's style file) at `<name>.<extension>`, the
/// extension being the one the artifact gives, else `bin`.
struct FolderNamer;

impl ArtifactNamer for FolderNamer {
    fn url_of(&self, artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
        let extension = artifact.extension.unwrap_or("bin");
        let folder = if artifact.id.owner.as_str() == graphics::FEATURE { "files/" } else { "" };
        RelPath::new(format!("{folder}{}.{extension}", artifact.id.name))
    }
}

/// A converter offering the pairs it lists, answering the content prefixed
/// with its name.
struct Prefixing {
    name: &'static str,
    pairs: Vec<(ImageFormat, ImageFormat)>,
    used: Arc<Mutex<Vec<String>>>,
}

impl GraphicsConverter for Prefixing {
    fn name(&self) -> &str {
        self.name
    }

    fn offers(&self, from: ImageFormat, to: ImageFormat) -> bool {
        self.pairs.contains(&(from, to))
    }

    fn convert(&self, request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
        self.used.lock().expect("a lock").push(self.name.to_string());
        let mut out = format!("{}:", self.name).into_bytes();
        out.extend_from_slice(request.content);
        Ok(out)
    }
}

/// A converter offering every pair and failing every conversion.
struct Failing;

impl GraphicsConverter for Failing {
    fn name(&self) -> &str {
        "failing"
    }

    fn offers(&self, _from: ImageFormat, _to: ImageFormat) -> bool {
        true
    }

    fn convert(&self, _request: &ConversionRequest<'_>) -> Result<Vec<u8>, GraphicsConvertError> {
        Err(GraphicsConvertError::new("the tests' converter always fails"))
    }
}

/// A resolver that finds every PNG file and can read none of them.
struct Unreadable;

impl GraphicsResolver for Unreadable {
    fn resolve(&self, request: &GraphicsResolveRequest) -> Result<ImageFile, GraphicsResolveError> {
        Ok(ImageFile::new(format!("unreadable:{}", request.path), "image/png", &request.path))
    }

    fn read(&self, _file: &ImageFile) -> Result<Vec<u8>, GraphicsResolveError> {
        Err(GraphicsResolveError::new("permission denied"))
    }
}

/// The standard package, `\includegraphics` an ordinary command, with the
/// services given and the tests' naming service.
fn registry(
    files: Option<Arc<MemoryFiles>>,
    converters: Vec<Arc<dyn GraphicsConverter>>,
) -> Arc<FlmRegistry> {
    registry_with(files.map(|files| files as Arc<dyn GraphicsResolver>), converters, true)
}

/// The standard package, `\includegraphics` an ordinary command, with the
/// resolver and converters given, and the tests' naming service where
/// `namer` says so.
fn registry_with(
    resolver: Option<Arc<dyn GraphicsResolver>>,
    converters: Vec<Arc<dyn GraphicsConverter>>,
    namer: bool,
) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().graphics(GraphicsConfig::new().global(true))));
    builder.use_package(flm_core::math_passthrough::package());
    if let Some(resolver) = resolver {
        builder.contribute_to(RESOLVER_SLOT, resolver);
    }
    for converter in converters {
        builder.contribute_to(CONVERTERS_SLOT, converter);
    }
    if namer {
        builder.contribute_to(NAMER_SLOT, Arc::new(FolderNamer) as Arc<dyn ArtifactNamer>);
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// The options publishing `output` by `strategy` under `rules`.
fn options(output: &str, strategy: Strategy, rules: Vec<ConversionRule>) -> Overrides {
    let mut chosen = GraphicsRenderOptions::default();
    chosen.strategy.insert(output.to_string(), strategy);
    chosen.convert.insert(output.to_string(), rules);
    let mut values = OptionValues::new();
    values.set(chosen);
    Overrides { render_options: values, ..Overrides::none() }
}

/// Runs `workflow` over the trees `texts`, on one thread or not.
fn render(
    registry: &Arc<FlmRegistry>,
    workflow: &str,
    texts: &[&str],
    overrides: &Overrides,
    run_options: &RunOptions,
) -> Run {
    let doc = common::pipeline::doc(registry, texts);
    let plan = plan(registry, workflow, overrides);
    common::pipeline::run_with(&doc, &plan, Externals::none(), run_options)
}

/// The output's set.
fn output(run: &Run, table: flm_core::core::data::table::TableName<ArtifactSet>) -> ArtifactSet {
    run.table(table).expect("declared").expect("written").clone()
}

/// The entry's text.
fn entry_text(set: &ArtifactSet) -> String {
    String::from_utf8(set.entry().and_then(Artifact::as_bytes).expect("an entry").to_vec())
        .expect("text")
}

/// Every `src="…"` of the page, in order.
fn sources(page: &str) -> Vec<String> {
    page.split("src=\"").skip(1).map(|rest| rest[..rest.find('"').expect("closed")].to_string()).collect()
}

/// The artifacts of the set other than its entry, as (identity, media type, content).
fn copies(set: &ArtifactSet) -> Vec<(String, String, Vec<u8>)> {
    set.iter()
        .filter(|artifact| !artifact.entry)
        .map(|artifact| {
            (artifact.id.to_string(), artifact.media_type.clone(), artifact.as_bytes().expect("bytes").to_vec())
        })
        .collect()
}

/// The Errors of the run, by identifier.
fn errors(run: &Run) -> Vec<String> {
    reports(run).into_iter().filter(|(_, severity)| *severity == Severity::Error).map(|(id, _)| id).collect()
}

/// `count` Warnings `flm.graphics.unknown-size`, which an image draws where
/// no inspector measures it and no option gives its size.
fn unknown_sizes(count: usize) -> Vec<(String, Severity)> {
    vec![(String::from("flm.graphics.unknown-size"), Severity::Warning); count]
}

/// Two files of different formats.
fn two_files() -> MemoryFiles {
    MemoryFiles::default()
        .with("figures/plot.png", "image/png", b"PNG-plot")
        .with("photo.jpg", "image/jpeg", b"JPEG-photo")
}

// ---------------------------------------------------------------------------------------------
// the acceptance tests
// ---------------------------------------------------------------------------------------------

/// Acceptance 1: under `copy`, each image has the address the namer gives,
/// and the output is the page and the two copies, each holding its file.
#[test]
fn under_copy_the_page_names_each_copy_and_the_output_holds_them() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let run = render(
        &registry,
        html::WORKFLOW,
        &["A \\includegraphics{figures/plot.png} and \\includegraphics{photo.jpg}."],
        &options(html::OUTPUT, Strategy::Copy, Vec::new()),
        &RunOptions::sequential(),
    );
    assert_eq!(errors(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let set = output(&run, html::DOCUMENT);
    let found = copies(&set);
    assert_eq!(found.len(), 2, "{found:?}");
    let plot = found.iter().find(|(_, _, bytes)| bytes == b"PNG-plot").expect("the plot's copy");
    let photo = found.iter().find(|(_, _, bytes)| bytes == b"JPEG-photo").expect("the photo's copy");
    assert!(plot.0.starts_with("graphics:plot-") && plot.0.len() == "graphics:plot-".len() + 8, "{}", plot.0);
    assert!(photo.0.starts_with("graphics:photo-"), "{}", photo.0);
    assert_eq!((plot.1.as_str(), photo.1.as_str()), ("image/png", "image/jpeg"));
    // Each copy carries its format's short name as its extension, which the namer used.
    let extension = |id: &str| set.iter().find(|a| a.id.to_string() == id).and_then(|a| a.extension.clone());
    assert_eq!((extension(&plot.0), extension(&photo.0)), (Some("png".to_string()), Some("jpeg".to_string())));
    let name = |id: &str| id.trim_start_matches("graphics:").to_string();
    assert_eq!(
        sources(&entry_text(&set)),
        [format!("files/{}.png", name(&plot.0)), format!("files/{}.jpeg", name(&photo.0))]
    );
}

/// Acceptance 2: a file shown twice, in one tree or in two, is one copy, and
/// nothing is reported.
#[test]
fn a_file_shown_twice_in_one_tree_or_two_is_one_copy() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let run = render(
        &registry,
        html::WORKFLOW,
        &[
            "\\includegraphics{figures/plot.png} and \\includegraphics{figures/plot.png}",
            "Again \\includegraphics{figures/plot.png}.",
        ],
        &options(html::OUTPUT, Strategy::Copy, Vec::new()),
        &RunOptions::new(),
    );
    // No inspector measures the three images.
    assert_eq!(reports(&run), unknown_sizes(3), "{}", run.diagnostics().render_all());
    let set = output(&run, html::DOCUMENT);
    assert_eq!(copies(&set).len(), 1);
    assert!(set.violations().is_empty());
    let found = sources(&entry_text(&set));
    assert_eq!(found.len(), 3);
    assert!(found.iter().all(|src| *src == found[0]));
}

/// Acceptance 3: under `data-uri`, the page holds each file's content in its
/// address, and the output is the page alone.
#[test]
fn under_data_uri_the_page_holds_the_content_and_the_output_is_the_page_alone() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let run = render(
        &registry,
        html::WORKFLOW,
        &["\\includegraphics{figures/plot.png} \\includegraphics{photo.jpg}"],
        &options(html::OUTPUT, Strategy::DataUri, Vec::new()),
        &RunOptions::sequential(),
    );
    assert_eq!(errors(&run), Vec::<String>::new());
    let set = output(&run, html::DOCUMENT);
    assert_eq!(set.len(), 1);
    let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    assert_eq!(
        sources(&entry_text(&set)),
        [
            format!("data:image/png;base64,{}", encode(b"PNG-plot")),
            format!("data:image/jpeg;base64,{}", encode(b"JPEG-photo")),
        ]
    );
}

/// Acceptance 4: a rule's conversion gives the copy the target format; of
/// two converters offering the pair the first contributed is used, and the
/// one `via` names where it names one; a pair no converter offers is
/// reported and the file copied as it is.
#[test]
fn a_rule_converts_through_the_first_converter_offering_its_pair_or_the_one_via_names() {
    let files = || Arc::new(MemoryFiles::default().with("anim.gif", "image/gif", b"GIF").with("fig.pdf", "application/pdf", b"PDF"));
    let used = Arc::new(Mutex::new(Vec::new()));
    let converters = || -> Vec<Arc<dyn GraphicsConverter>> {
        ["first", "second"]
            .into_iter()
            .map(|name| {
                Arc::new(Prefixing {
                    name,
                    pairs: vec![(ImageFormat::Gif, ImageFormat::Png)],
                    used: Arc::clone(&used),
                }) as Arc<dyn GraphicsConverter>
            })
            .collect()
    };
    let gif_to_png = ConversionRule::new([ImageFormat::Gif, ImageFormat::Webp], ImageFormat::Png)
        .options(ConversionOptions::new().dpi(192));
    let pdf_to_png = ConversionRule::new([ImageFormat::Pdf], ImageFormat::Png);
    let document = ["\\includegraphics{anim.gif} \\includegraphics{fig.pdf}"];
    let copy_of = |run: &Run, prefix: &str| -> (String, String, Vec<u8>) {
        copies(&output(run, html::DOCUMENT))
            .into_iter()
            .find(|(_, _, bytes)| bytes.ends_with(prefix.as_bytes()))
            .expect("a copy")
    };

    let registry = registry(Some(files()), converters());
    let rules = vec![gif_to_png.clone(), pdf_to_png];
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::Copy, rules), &RunOptions::sequential());
    let (id, media_type, bytes) = copy_of(&run, "GIF");
    assert_eq!((media_type.as_str(), bytes.as_slice()), ("image/png", &b"first:GIF"[..]));
    let page = entry_text(&output(&run, html::DOCUMENT));
    assert!(page.contains(&format!("src=\"files/{}.png\"", id.trim_start_matches("graphics:"))), "{page}");
    // No converter offers PDF to PNG: reported, and the file copied as it is.
    assert_eq!(errors(&run), ["flm.graphics.no-converter"]);
    let (_, media_type, bytes) = copy_of(&run, "PDF");
    assert_eq!((media_type.as_str(), bytes.as_slice()), ("application/pdf", &b"PDF"[..]));
    assert_eq!(*used.lock().expect("a lock"), ["first"]);

    // The rule's `via` names the second.
    used.lock().expect("a lock").clear();
    let rules = vec![gif_to_png.clone().via("second")];
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::Copy, rules), &RunOptions::sequential());
    assert_eq!(copy_of(&run, "GIF").2, b"second:GIF");
    assert_eq!(errors(&run), Vec::<String>::new());

    // A `via` naming no converter is reported, and the file copied as it is.
    let rules = vec![gif_to_png.via("third")];
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::Copy, rules), &RunOptions::sequential());
    assert_eq!(copy_of(&run, "GIF").2, b"GIF");
    assert_eq!(errors(&run), ["flm.graphics.unknown-converter"]);
}

/// The standard package with the features `t.two` (the converters `two-a`
/// then `two-b`) and `t.one` (the converter `one`), added in this order, each
/// offering GIF to PNG, the host's `chains`, the resolver `files` and the
/// tests' naming service.
fn two_converting_features(files: Arc<MemoryFiles>, used: &Arc<Mutex<Vec<String>>>, chains: &[&str]) -> Arc<FlmRegistry> {
    let contributing = |feature: &'static str, names: &'static [&'static str]| {
        let used = Arc::clone(used);
        common::pipeline::feature(feature, move |b| {
            for &name in names {
                let converter = Prefixing { name, pairs: vec![(ImageFormat::Gif, ImageFormat::Png)], used: Arc::clone(&used) };
                b.contribute_to(CONVERTERS_SLOT, Arc::new(converter) as Arc<dyn GraphicsConverter>)?;
            }
            Ok(())
        })
    };
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().graphics(GraphicsConfig::new().global(true))));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(contributing("t.two", &["two-a", "two-b"]));
    builder.feature(contributing("t.one", &["one"]));
    for text in chains {
        builder.feature_order(FeatureChain::parse(text, "test").expect("a chain"));
    }
    builder.contribute_to(RESOLVER_SLOT, files as Arc<dyn GraphicsResolver>);
    builder.contribute_to(NAMER_SLOT, Arc::new(FolderNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// Of converters two features contribute, those of the feature placed last in
/// the feature order are asked first, each feature's in the order it
/// contributed them.
#[test]
fn the_converters_of_the_feature_placed_last_are_asked_first_each_in_its_features_order() {
    let files = || Arc::new(MemoryFiles::default().with("anim.gif", "image/gif", b"GIF"));
    let used = Arc::new(Mutex::new(Vec::new()));
    let rules = vec![ConversionRule::new([ImageFormat::Gif], ImageFormat::Png)];
    let overrides = options(html::OUTPUT, Strategy::Copy, rules);
    let document = ["\\includegraphics{anim.gif}"];
    let converted = |run: &Run| -> Vec<u8> {
        copies(&output(run, html::DOCUMENT)).into_iter().map(|(_, _, bytes)| bytes).next().expect("a copy")
    };

    // `t.one` is placed last: its converter converts.
    let registry = two_converting_features(files(), &used, &[]);
    let run = render(&registry, html::WORKFLOW, &document, &overrides, &RunOptions::sequential());
    assert_eq!(errors(&run), Vec::<String>::new());
    assert_eq!(converted(&run), b"one:GIF");
    assert_eq!(*used.lock().expect("a lock"), ["one"]);

    // The chain places `t.two` last: its first converter converts.
    used.lock().expect("a lock").clear();
    let registry = two_converting_features(files(), &used, &["t.one < t.two"]);
    let run = render(&registry, html::WORKFLOW, &document, &overrides, &RunOptions::sequential());
    assert_eq!(errors(&run), Vec::<String>::new());
    assert_eq!(converted(&run), b"two-a:GIF");
    assert_eq!(*used.lock().expect("a lock"), ["two-a"]);
}

/// Acceptance 5: an image inside content shown a second time — a heading's
/// title a reference shows — has the address of the original, and the output
/// holds one copy.
#[test]
fn an_image_shown_again_by_a_reference_has_the_original_address_and_one_copy() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let run = render(
        &registry,
        html::WORKFLOW,
        &["\\section*{Plot \\includegraphics{figures/plot.png}}\\label{sec:c}\n\nSee \\ref{sec:c}."],
        &options(html::OUTPUT, Strategy::Copy, Vec::new()),
        &RunOptions::sequential(),
    );
    assert_eq!(errors(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let set = output(&run, html::DOCUMENT);
    let found = sources(&entry_text(&set));
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0], found[1]);
    assert_eq!(copies(&set).len(), 1);
}

/// Acceptance 6: a path naming a scheme is published by reference under
/// every strategy, and the resolver is never asked about it.
#[test]
fn a_path_naming_a_scheme_is_published_as_written_under_every_strategy() {
    let path = "https://example.org/a.png";
    for strategy in [Strategy::Reference, Strategy::Copy, Strategy::DataUri] {
        let files = Arc::new(MemoryFiles::default());
        let registry = registry(Some(Arc::clone(&files)), Vec::new());
        let run = render(
            &registry,
            html::WORKFLOW,
            &[&format!("\\includegraphics{{{path}}}")],
            &options(html::OUTPUT, strategy, Vec::new()),
            &RunOptions::sequential(),
        );
        // A web address is never measured.
        assert_eq!(reports(&run), unknown_sizes(1), "{strategy:?}");
        let set = output(&run, html::DOCUMENT);
        assert_eq!(sources(&entry_text(&set)), [path], "{strategy:?}");
        assert_eq!(set.len(), 1);
        assert_eq!(files.asked.load(Ordering::SeqCst), 0);
    }
}

/// A stage writing two different artifacts under one identity, one per tree:
/// what two images whose copies collide would contribute.
fn colliding_feature() -> Arc<dyn FlmFeature> {
    common::pipeline::feature("t", |b| {
        b.capability(CapabilityDef::new("t.files"))?;
        b.stage(common::pipeline::stage(
            StageDecl::new("t.write-files", Effect::Compute, Scope::PerTree)
                .provides("t.files")
                .writes(html::ARTIFACTS),
            Needs::new(),
            |cx, out| {
                let content = format!("tree {:?}", cx.tree_id()).into_bytes();
                let set = out.table(html::ARTIFACTS).expect("declared");
                set.insert(Artifact::bytes(ArtifactId::new("t", "copy"), "image/png", content))
                    .expect("one artifact");
            },
        ))
    })
}

/// Acceptance 7: two different artifacts under one identity are reported.
#[test]
fn two_different_artifacts_under_one_identity_are_reported() {
    let mut features: Vec<Arc<dyn FlmFeature>> = package(StdcoreConfig::new())
        .features
        .into_iter()
        .filter(|feature| feature.info().name.as_str() != graphics::FEATURE)
        .chain([flm_core::math_passthrough::feature()])
        .collect();
    features.push(colliding_feature());
    let registry = common::pipeline::registry(features);
    let request = flm_core::core::plan::PlanRequest::new().demand(html::DOCUMENT.name()).demand("t.files");
    let plan = common::pipeline::plan_for(&registry, &request);
    let doc = common::pipeline::doc(&registry, &["One.", "Two."]);
    let run = common::pipeline::run(&doc, &plan, &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.artifacts.duplicate"]);
    let set = output(&run, html::DOCUMENT);
    assert_eq!(set.len(), 2);
    assert_eq!(set.violations().len(), 1);
}

/// Acceptance 8: with no resolver and the default options, the page names
/// the path as written and holds nothing else — and asking for a strategy
/// that needs a resolver without one is reported, the image then published
/// by reference.
#[test]
fn with_no_resolver_the_path_as_written_is_published() {
    let registry = registry(None, Vec::new());
    let document = ["\\includegraphics{figures/plot.png}"];
    let run = render(&registry, html::WORKFLOW, &document, &Overrides::none(), &RunOptions::sequential());
    assert_eq!(reports(&run), unknown_sizes(1));
    let set = output(&run, html::DOCUMENT);
    assert_eq!((set.len(), sources(&entry_text(&set))), (1, vec![String::from("figures/plot.png")]));
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::Copy, Vec::new()), &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.graphics.needs-resolver"]);
    assert_eq!(sources(&entry_text(&output(&run, html::DOCUMENT))), ["figures/plot.png"]);
}

/// With a resolver, `reference` writes the resolver's address, and a file
/// the resolver cannot find is reported once, where the record is made.
#[test]
fn under_reference_the_resolvers_address_is_written_and_a_missing_file_reported() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let run = render(
        &registry,
        html::WORKFLOW,
        &["\\includegraphics{figures/plot.png} \\includegraphics{missing.png}"],
        &options(html::OUTPUT, Strategy::DataUri, Vec::new()),
        &RunOptions::sequential(),
    );
    assert_eq!(errors(&run), ["flm.graphics.resolve-failed"]);
    let found = sources(&entry_text(&output(&run, html::DOCUMENT)));
    assert_eq!(found[1], "missing.png");
    let info = run.table(INFO).expect("declared").expect("written");
    let (_, record) = info.iter().next().expect("a record");
    assert_eq!(record.file.as_ref().map(|file| file.identity.as_str()), Some("mem:figures/plot.png"));
}

/// Acceptance 9: a run on one thread and a run in parallel write the same
/// tables.
#[test]
fn one_thread_and_several_write_the_same_tables() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let texts = [
        "\\includegraphics{figures/plot.png} \\includegraphics{photo.jpg}",
        "\\includegraphics{photo.jpg}",
        "\\includegraphics{figures/plot.png}",
    ];
    let overrides = options(html::OUTPUT, Strategy::Copy, Vec::new());
    let tables = |run_options: &RunOptions| {
        let run = render(&registry, html::WORKFLOW, &texts, &overrides, run_options);
        let set = output(&run, html::DOCUMENT);
        // Each run parses the document anew, so a node's identity differs
        // between the two: the addresses are compared in node order.
        let published: Vec<String> = run
            .table(&published_table(html::BACKEND))
            .expect("declared")
            .expect("written")
            .iter()
            .map(|(_, image)| image.src.clone())
            .collect();
        (entry_text(&set), copies(&set), published)
    };
    assert_eq!(tables(&RunOptions::sequential()), tables(&RunOptions::new()));
}

/// Acceptance 10: under `copy` on the recomposed LaTeX, the output is the
/// document, its style file and the copies, and the hook names each copy by
/// the address the namer gives.
#[test]
fn under_copy_the_recomposed_latex_names_each_copy_beside_its_style_file() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let run = render(
        &registry,
        latex_recomposed::WORKFLOW,
        &["A \\includegraphics{figures/plot.png} and \\includegraphics{photo.jpg}."],
        &options(latex_recomposed::BACKEND, Strategy::Copy, Vec::new()),
        &RunOptions::sequential(),
    );
    assert_eq!(errors(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let set = output(&run, latex_recomposed::DOCUMENT);
    let ids: Vec<String> = set.iter().map(|artifact| artifact.id.to_string()).collect();
    assert_eq!(set.len(), 4, "{ids:?}");
    let style = latex_recomposed::style_artifact(&set).expect("the style file");
    let path = style.fixed_path.as_ref().expect("a fixed path").as_str();
    assert_eq!(path, format!("{}.sty", style.id.name), "the namer's path for the style file");
    let tex = entry_text(&set);
    for (_, media_type, bytes) in copies(&set).into_iter().filter(|(id, _, _)| id.starts_with("graphics:")) {
        let id = set.iter().find(|artifact| artifact.as_bytes() == Some(&bytes[..])).expect("found").id.clone();
        let extension = ImageFormat::from_media_type(&media_type).expect("known").name();
        let hook = format!("\\flmIncludeGraphics{{files/{}.{extension}}}", id.name);
        assert!(tex.contains(&hook), "{hook} in {tex}");
    }
}

// ---------------------------------------------------------------------------------------------
// the registry without a backend the feature publishes for
// ---------------------------------------------------------------------------------------------

/// The standard package without the HTML backend: the publish stage writing
/// the page's artifact table has nowhere to write, is not registered and is
/// named in the Note, and the other outputs render as they do beside it.
#[test]
fn without_the_html_backend_its_publish_stage_is_set_aside_and_the_other_outputs_unchanged() {
    let config = || StdcoreConfig::new().graphics(GraphicsConfig::new().global(true));
    let without: Vec<Arc<dyn FlmFeature>> = package(config())
        .features
        .into_iter()
        .filter(|feature| feature.info().name.as_str() != html::FEATURE_NAME)
        .chain([flm_core::math_passthrough::feature()])
        .collect();
    let without = common::pipeline::registry(without);
    let mut with = package(config()).features;
    with.push(flm_core::math_passthrough::feature());
    let with = common::pipeline::registry(with);
    assert!(!without.build_diagnostics().has_errors());
    let publish = |registry: &FlmRegistry| -> Vec<String> {
        registry
            .stages()
            .iter()
            .map(|entry| entry.name.to_string())
            .filter(|name| name.starts_with("flm.graphics.publish."))
            .collect()
    };
    assert_eq!(
        publish(&with),
        [
            "flm.graphics.publish.text",
            "flm.graphics.publish.html",
            "flm.graphics.publish.latex-recomposed",
            "flm.graphics.publish.latex-rendered"
        ]
    );
    assert_eq!(
        publish(&without),
        ["flm.graphics.publish.text", "flm.graphics.publish.latex-recomposed", "flm.graphics.publish.latex-rendered"]
    );
    let notes: Vec<String> = without
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.registry.unresolved-peer")
        .map(|d| d.message())
        .collect();
    assert!(
        notes.iter().any(|note| note.contains("refers to the table `flm.output.html.artifacts`")
            && note.ends_with("the stages writing it are set aside: [flm.graphics.publish.html]")),
        "{notes:?}"
    );
    let document = ["\\section{Top}\n\nA \\includegraphics{figures/plot.png} figure."];
    for (workflow, table) in [
        (text::WORKFLOW, text::TEXT_DOCUMENT),
        (latex_recomposed::WORKFLOW, latex_recomposed::DOCUMENT),
    ] {
        let render_on = |registry: &Arc<FlmRegistry>| {
            let run = render(registry, workflow, &document, &Overrides::none(), &RunOptions::sequential());
            (entry_text(&output(&run, table)), reports(&run))
        };
        let (text, found) = render_on(&without);
        assert_eq!(found, unknown_sizes(1), "{workflow}");
        assert!(text.contains("figures/plot.png"), "{workflow}: {text}");
        assert_eq!((text, found), render_on(&with), "{workflow}");
    }
}

// ---------------------------------------------------------------------------------------------
// the conditions
// ---------------------------------------------------------------------------------------------

/// `data-uri` asked for an output whose image cannot show a data address —
/// the text output, the recomposed LaTeX — is reported at the image, which is
/// published by reference: no `data:` address is written.
#[test]
fn data_uri_on_an_output_other_than_the_page_is_reported_and_published_by_reference() {
    let registry = registry(Some(Arc::new(two_files())), Vec::new());
    let document = ["A \\includegraphics{figures/plot.png} figure."];
    for (workflow, backend, table) in [
        (text::WORKFLOW, text::BACKEND, text::TEXT_DOCUMENT),
        (latex_recomposed::WORKFLOW, latex_recomposed::BACKEND, latex_recomposed::DOCUMENT),
    ] {
        let run = render(&registry, workflow, &document, &options(backend, Strategy::DataUri, Vec::new()), &RunOptions::sequential());
        assert_eq!(errors(&run), ["flm.graphics.data-uri-not-shown"], "{workflow}");
        let set = output(&run, table);
        let written = entry_text(&set);
        assert!(written.contains("figures/plot.png"), "{workflow}: {written}");
        assert!(!written.contains("data:"), "{workflow}: {written}");
        assert_eq!(set.len(), if backend == text::BACKEND { 1 } else { 2 }, "{workflow}");
    }
}

/// Each condition the publish stage meets while carrying out `copy` or
/// `data-uri` is reported at the image, which is still published: by
/// reference where the strategy cannot be carried out, as the file is where
/// the conversion fails.
#[test]
fn the_publish_stages_conditions_are_reported_and_the_image_still_published() {
    let files = || Arc::new(two_files()) as Arc<dyn GraphicsResolver>;
    let document = ["A \\includegraphics{figures/plot.png} figure."];
    let page = |run: &Run| entry_text(&output(run, html::DOCUMENT));

    // `copy` on the text output, which is one file.
    let registry = registry_with(Some(files()), Vec::new(), true);
    let run = render(&registry, text::WORKFLOW, &document, &options(text::BACKEND, Strategy::Copy, Vec::new()), &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.graphics.copy-in-one-file"]);
    let written = entry_text(&output(&run, text::TEXT_DOCUMENT));
    assert!(written.contains("figures/plot.png"), "{written}");

    // `copy` with no naming service.
    let registry = registry_with(Some(files()), Vec::new(), false);
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::Copy, Vec::new()), &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.graphics.needs-namer"]);
    assert_eq!(sources(&page(&run)), ["figures/plot.png"]);
    assert_eq!(output(&run, html::DOCUMENT).len(), 1);

    // A conversion that fails: the file is copied as it is.
    let registry = registry_with(Some(files()), vec![Arc::new(Failing) as Arc<dyn GraphicsConverter>], true);
    let rules = vec![ConversionRule::new([ImageFormat::Png], ImageFormat::Webp)];
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::Copy, rules), &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.graphics.conversion-failed"]);
    let found = copies(&output(&run, html::DOCUMENT));
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].1.as_str(), found[0].2.as_slice()), ("image/png", &b"PNG-plot"[..]));

    // A file the resolver finds and then cannot read: reported by the
    // publish stage, where the content is read.
    let registry = registry_with(Some(Arc::new(Unreadable)), Vec::new(), true);
    let run = render(&registry, html::WORKFLOW, &document, &options(html::OUTPUT, Strategy::DataUri, Vec::new()), &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.graphics.resolve-failed"]);
    assert_eq!(sources(&page(&run)), ["figures/plot.png"]);
}

/// A stage writing an artifact marked as an entry into the page's artifact
/// table, beside the page's own entry.
fn second_entry_feature() -> Arc<dyn FlmFeature> {
    common::pipeline::feature("t", |b| {
        b.capability(CapabilityDef::new("t.files"))?;
        b.stage(common::pipeline::stage(
            StageDecl::new("t.write-entry", Effect::Compute, Scope::Document)
                .provides("t.files")
                .writes(html::ARTIFACTS),
            Needs::new(),
            |_, out| {
                let set = out.table(html::ARTIFACTS).expect("declared");
                let entry = Artifact::bytes(ArtifactId::new("t", "other-page"), "text/html", b"<p>other</p>".to_vec());
                set.insert(entry.as_entry()).expect("the table's only entry");
            },
        ))
    })
}

/// A table beside the entry holding a second entry is reported when the
/// output's final stage merges it into the set.
#[test]
fn a_second_entry_beside_the_page_is_reported() {
    let mut features: Vec<Arc<dyn FlmFeature>> = package(StdcoreConfig::new())
        .features
        .into_iter()
        .filter(|feature| feature.info().name.as_str() != graphics::FEATURE)
        .chain([flm_core::math_passthrough::feature()])
        .collect();
    features.push(second_entry_feature());
    let registry = common::pipeline::registry(features);
    let request = flm_core::core::plan::PlanRequest::new().demand(html::DOCUMENT.name()).demand("t.files");
    let plan = common::pipeline::plan_for(&registry, &request);
    let doc = common::pipeline::doc(&registry, &["One."]);
    let run = common::pipeline::run(&doc, &plan, &RunOptions::sequential());
    assert_eq!(errors(&run), ["flm.artifacts.second-entry"]);
    let set = output(&run, html::DOCUMENT);
    assert_eq!(set.violations().len(), 1);
    assert!(entry_text(&set).contains("One."));
}

/// The outputs of a command, as a namer receives them: the outputs whose capability is
/// demanded, each with its entry's extension, in the registry's order; none when nothing
/// is demanded.
#[test]
fn the_naming_scope_lists_the_demanded_outputs_with_their_extensions() {
    let registry = registry(None, Vec::new());
    let capability = |output: &str| registry.output(output).expect("registered").def.capability.clone();
    let demands: BTreeSet<_> = [capability(latex_recomposed::WORKFLOW), capability(html::OUTPUT)].into_iter().collect();
    let scope = NamingScope::of(&registry, &demands);
    let scoped = |name: &str, extension: &str| ScopedOutput { name: name.into(), extension: extension.to_string() };
    assert_eq!(scope.outputs, [scoped("html", "html"), scoped("latex-recomposed", "tex")]);
    assert_eq!(NamingScope::of(&registry, &BTreeSet::new()), NamingScope::default());
    // A plan's demands give its outputs.
    let plan = plan(&registry, text::WORKFLOW, &Overrides::none());
    assert_eq!(NamingScope::of(&registry, plan.demands()).outputs, [scoped("text", "txt")]);
}
