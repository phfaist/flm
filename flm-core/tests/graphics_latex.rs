//! The `graphics` feature on the recomposed LaTeX
//! (`flm_core::stdcore::features::graphics::latex`): the hook
//! `\flmIncludeGraphics[<keys>]{<path>}` with its lengths through the family's
//! length writer, its definition piece and the two packages it requests
//! (test 8); a path's characters escaped, and a user definition whose
//! template holds a placeholder in a path expanded at its use sites (the
//! LaTeX half of test 3). The golden `graphics.tex` is compiled under both
//! engines by `tests/latex_compile.rs`, with an image written beside it.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::backends::pres::Length;
use flm_core::stdcore::features::graphics::{
    Dimensions, GraphicsConfig, GraphicsInspectError, GraphicsInspectRequest,
    GraphicsInspectedInformation, GraphicsInspector, INSPECTOR_SLOT,
};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{StdcoreConfig, package};

use common::stdcore::identifiers;

/// The configuration under which `\includegraphics` is an ordinary command,
/// with `definitions` as the user definitions.
fn config(definitions: Vec<Definition>) -> StdcoreConfig {
    StdcoreConfig::new()
        .graphics(GraphicsConfig::new().global(true))
        .userdefs(UserdefsConfig::new().definitions(definitions))
}

/// A vector picture of the size its path names, `<w>x<h>` in points, for
/// every path of that form; any other path is no file.
struct SizedByName;

impl GraphicsInspector for SizedByName {
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
        let stem = request.path.trim_end_matches(".pdf");
        let (width, height) =
            stem.split_once('x').ok_or_else(|| GraphicsInspectError::new("no such file"))?;
        let points = |amount: &str| {
            Length::parse(&format!("{amount}pt")).map_err(|error| GraphicsInspectError::new(error.to_string()))
        };
        Ok(GraphicsInspectedInformation::Vector {
            physical_dimensions: Dimensions { width: points(width)?, height: points(height)? },
        })
    }
}

/// The registry under `config`, with the inspector of `SizedByName` where
/// `inspector` says so.
fn registry(config: StdcoreConfig, inspector: bool) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    if inspector {
        builder.contribute_to(INSPECTOR_SLOT, Arc::new(SizedByName) as Arc<dyn GraphicsInspector>);
    }
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// The run of the recomposed LaTeX over `document`.
fn run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    common::stdcore::run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &Overrides::none())
}

/// The recomposed LaTeX and the style file of `document`; the run may
/// report only the Warnings every `width=` and `height=` draws, and the
/// Warning of an image whose size is not known. The recomposed LaTeX is given
/// without the backend's closing line, `%%`.
fn written(registry: &Arc<FlmRegistry>, document: &str) -> (String, String, Run) {
    let run = run(registry, document);
    let unexpected: Vec<String> = identifiers(&run)
        .into_iter()
        .filter(|id| id != "flm.graphics.resized" && id != "flm.graphics.unknown-size")
        .collect();
    assert_eq!(unexpected, Vec::<String>::new(), "{}", run.diagnostics().render_all());
    // The output ends with the backend's closing comment line.
    let output = common::latex::output(&run);
    let output = output.strip_suffix("%%\n").expect("the closing line").to_string();
    let style = common::latex::style(&run).0;
    (output, style, run)
}

/// The document of the golden: the three forms test 8 names, and a
/// dimension scaled from an inspected size.
const DOCUMENT: &str = "\\includegraphics[width=5cm]{figures/plot.png}\n\n\
                        \\includegraphics[width=10pt]{figures/plot.png}\n\n\
                        \\includegraphics{figures/plot.png}\n\n\
                        \\includegraphics[height=1in]{figures/plot.png}\n";

#[test]
fn an_image_is_written_as_its_hook_with_its_lengths_through_the_family_writer() {
    let (output, style, run) = written(&registry(config(Vec::new()), false), DOCUMENT);
    assert_eq!(
        output,
        "\\flmIncludeGraphics[width=5cm]{figures/plot.png}\n\n\
         \\flmIncludeGraphics[width=10bp]{figures/plot.png}\n\n\
         \\flmIncludeGraphics{figures/plot.png}\n\n\
         \\flmIncludeGraphics[height=1in]{figures/plot.png}\n"
    );
    assert!(style.contains("\\RequirePackage{graphicx}\n"), "{style}");
    assert!(style.contains("\\RequirePackage[export]{adjustbox}\n"), "{style}");
    assert!(
        style.contains(
            "\\ProvideDocumentCommand\\flmIncludeGraphics{O{} m}{\\includegraphics[max \
             width=\\linewidth,#1]{#2}}"
        ),
        "{style}"
    );
    common::latex::assert_golden("graphics.tex", &run);
}

#[test]
fn a_scaled_dimension_and_an_inspected_size_are_written_too() {
    let registry = registry(config(Vec::new()), true);
    let (output, _, _) = written(
        &registry,
        "\\includegraphics[width=5cm]{340x170.pdf}\n\n\\includegraphics{72x36.pdf}",
    );
    assert_eq!(
        output,
        "\\flmIncludeGraphics[width=5cm,height=2.5cm]{340x170.pdf}\n\n\
         \\flmIncludeGraphics[width=72bp,height=36bp]{72x36.pdf}"
    );
}

#[test]
fn a_path_is_written_through_the_address_escaping() {
    let (output, _, _) =
        written(&registry(config(Vec::new()), false), "\\includegraphics{a%b#c~d_e{f}.png}");
    assert_eq!(output, "\\flmIncludeGraphics{a\\%b\\#c~d_e{f}.png}");
}

#[test]
fn a_placeholder_in_a_path_expands_the_definitions_use_sites() {
    let fig = Definition::macro_("fig", vec![ArgDef::mandatory("name")])
        .text("\\includegraphics[width=2cm]{figs/#1.png}");
    let (output, _, _) = written(&registry(config(vec![fig]), false), "\\fig{plot} and \\fig{map}");
    assert_eq!(
        output,
        "\\flmIncludeGraphics[width=2cm]{figs/plot.png} and \\flmIncludeGraphics[width=2cm]{figs/map.png}"
    );
    assert!(!output.contains("\\flmUD"), "{output}");
}

#[test]
fn a_placeholder_scaled_from_an_inspected_size_in_an_expanded_use_site() {
    // The use site is expanded into a value; the inspect and publish stages,
    // which run once per tree, record and publish the image there too.
    let fig = Definition::macro_("fig", vec![ArgDef::mandatory("name")])
        .text("\\includegraphics[width=5cm]{#1.pdf}");
    let registry = registry(config(vec![fig]), true);
    let (output, _, _) = written(&registry, "\\fig{340x170}");
    assert_eq!(output, "\\flmIncludeGraphics[width=5cm,height=2.5cm]{340x170.pdf}");
}

#[test]
fn an_empty_path_is_an_error_and_writes_no_hook() {
    let run = run(&registry(config(Vec::new()), true), "A\n\n\\includegraphics{}\n\nB");
    assert_eq!(identifiers(&run), ["flm.graphics.empty-path"]);
    let output = common::latex::output(&run);
    assert!(!output.contains("\\flmIncludeGraphics"), "{output}");
}
