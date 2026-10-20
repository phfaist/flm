//! The stage [`BuildMeta`] (`flm.meta.build`), which parses the document's metadata
//! and builds the three pieces of the [`meta`](super) feature, and the diagnostic
//! [`MetaFailed`] it reports when a part of the metadata cannot be built.

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::data::store::StageOut;
use crate::core::data::table::TableHandle;
use crate::core::piece::Piece;
use crate::core::plan::PlanCx;
use crate::core::registry::FlmRegistry;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::copy::{copy_pieces_into_value, SynthesizedPiece, ValueContent, ValuePiece};
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::{nowhere, parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use crate::model::doc::{Fragment, FragmentLayout};
use crate::stdcore::backends::pres;

use super::{DocMeta, MetaText, AUTHOR, AUTHOR_NAME, BUILD, DATE, READ, TITLE};

/// The label of the origin under which a text of the metadata without a place
/// is parsed, `metadata`.
const NO_PLACE_LABEL: &str = "metadata";

/// The stage [`BUILD`] (`flm.meta.build`), which builds the metadata's pieces: a
/// compute stage that runs once per document. It reads the option [`DocMeta`],
/// writes the tables [`TITLE`], [`AUTHOR`] and [`DATE`], and provides the
/// capability [`READ`].
///
/// Each text (the title, each author, the date) is parsed as inline FLM content
/// with the registry's language, as a source named after the file the text was
/// written in and counted from the text's line and column there, so that each
/// problem the parse finds is reported at its place in that file. Every problem
/// the parse finds is reported. The title's content is one [`TITLE`] node holding
/// the parsed content, and the date's one [`DATE`] node; the authors' content is
/// one [`AUTHOR`] node holding one [`AUTHOR_NAME`] node per author, in the order
/// given. A piece without a text holds no content, and no backend places it.
///
/// A part that cannot be parsed or built is reported as [`MetaFailed`], and its
/// piece holds no content.
#[derive(Debug, Default, Clone, Copy)]
pub struct BuildMeta;

impl Stage for BuildMeta {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD, Effect::Compute, Scope::Document)
            .writes(TableHandle::<Piece>::new(TITLE))
            .writes(TableHandle::<Piece>::new(AUTHOR))
            .writes(TableHandle::<Piece>::new(DATE))
            .provides(READ)
            .docs(
                "Parses the document's title, authors and date as inline content and builds the \
                 pieces `flm.meta.title`, `flm.meta.author` and `flm.meta.date`.",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().option::<DocMeta>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let meta = match cx.option::<DocMeta>() {
            Ok(meta) => meta,
            Err(error) => {
                failed(out, "the metadata", error.to_string());
                return;
            }
        };
        let registry = cx.registry();
        let title = meta.title.as_ref().and_then(|text| single(registry, TITLE, text, out));
        let authors = authors(registry, &meta.authors, out);
        let date = meta.date.as_ref().and_then(|text| single(registry, DATE, text, out));
        for (name, content) in [(TITLE, title), (AUTHOR, authors), (DATE, date)] {
            match out.table(&TableHandle::<Piece>::new(name)) {
                Ok(table) => *table = Piece { content },
                Err(error) => failed(out, name, error.to_string()),
            }
        }
    }
}

/// Returns `text` parsed as inline content, with every problem of the parse
/// reported into `out`; `None` when the parse fails altogether, which is reported
/// too.
fn parse(registry: &FlmRegistry, what: &str, text: &MetaText, out: &mut StageOut<'_>) -> Option<Fragment> {
    let mut source = FlmSource::new(text.text.clone());
    source = match &text.place {
        Some(place) => source
            .with_origin(FlmOrigin::new(place.file.clone()))
            .with_line_column_number_offsets(place.line as usize, place.column as usize),
        None => source.with_origin(FlmOrigin::synthetic(NO_PLACE_LABEL)),
    };
    match parse_fragment(registry.lang(), Arc::new(source), FragmentLayout::Inline, &ParseOptions::default()) {
        Ok(fragment) => {
            for diagnostic in fragment.diags().iter() {
                out.diags.push(diagnostic.clone());
            }
            Some(fragment)
        }
        Err(error) => {
            failed(out, what, error.to_string());
            None
        }
    }
}

/// Returns the synthesized specification of the element `name`, which this
/// feature registers.
fn spec(registry: &FlmRegistry, name: &str) -> Result<Arc<FlmSynthesizedSpec>, String> {
    match registry.element_value(name) {
        Ok(Some(element)) => Ok(Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))),
        Ok(None) => Err(format!("no element `{name}` is registered")),
        Err(error) => Err(error.to_string()),
    }
}

/// Returns the body of a node holding `nodes`, the content of a parsed text.
fn body<'n, 't>(nodes: &'n [NodeRef<'t, Flm>]) -> Vec<ValuePiece<'n, 't>> {
    nodes.iter().copied().map(ValuePiece::Node).collect()
}

/// Returns the content of the title piece or of the date piece: one node of the
/// element `name` holding `text`, parsed.
fn single(registry: &FlmRegistry, name: &str, text: &MetaText, out: &mut StageOut<'_>) -> Option<Arc<NodeTree<Flm>>> {
    let fragment = parse(registry, name, text, out)?;
    let root = fragment.tree().root();
    let nodes: Vec<NodeRef<'_, Flm>> = root.children().iter().collect();
    let built = spec(registry, name).and_then(|spec| {
        let node = SynthesizedPiece {
            spec,
            slot: pres::BODY_SLOT,
            span: root.span().clone(),
            state: Arc::clone(root.parsing_state()),
            body: body(&nodes),
            payload: None,
        };
        copy_pieces_into_value(&[ValuePiece::Synthesized(&node)], &root, ValueContent::Own, registry.lang().guard())
            .map_err(|error| error.to_string())
    });
    built.map_err(|detail| failed(out, name, detail)).ok()
}

/// Returns the content of the authors piece: one [`AUTHOR`] node holding one
/// [`AUTHOR_NAME`] node per author; `None` when there is no author, or when no
/// author could be parsed.
fn authors(registry: &FlmRegistry, texts: &[MetaText], out: &mut StageOut<'_>) -> Option<Arc<NodeTree<Flm>>> {
    let fragments: Vec<Fragment> = texts.iter().filter_map(|text| parse(registry, AUTHOR, text, out)).collect();
    let roots: Vec<NodeRef<'_, Flm>> = fragments.iter().map(|fragment| fragment.tree().root()).collect();
    let first = *roots.first()?;
    let contents: Vec<Vec<NodeRef<'_, Flm>>> = roots.iter().map(|root| root.children().iter().collect()).collect();
    let built = spec(registry, AUTHOR).and_then(|author_spec| {
        let name_spec = spec(registry, AUTHOR_NAME)?;
        let names: Vec<SynthesizedPiece<'_, '_>> = roots
            .iter()
            .zip(&contents)
            .map(|(root, nodes)| SynthesizedPiece {
                spec: Arc::clone(&name_spec),
                slot: pres::BODY_SLOT,
                span: root.span().clone(),
                state: Arc::clone(root.parsing_state()),
                body: body(nodes),
                payload: None,
            })
            .collect();
        let node = SynthesizedPiece {
            spec: author_spec,
            slot: pres::BODY_SLOT,
            span: first.span().clone(),
            state: Arc::clone(first.parsing_state()),
            body: names.iter().map(ValuePiece::Synthesized).collect(),
            payload: None,
        };
        copy_pieces_into_value(&[ValuePiece::Synthesized(&node)], &first, ValueContent::Own, registry.lang().guard())
            .map_err(|error| error.to_string())
    });
    built.map_err(|detail| failed(out, AUTHOR, detail)).ok()
}

/// Reports that `what` (one of the three pieces, or `the metadata`) could not be
/// built, as the Error [`MetaFailed`], with the part written as an author knows
/// it.
fn failed(out: &mut StageOut<'_>, what: &str, detail: String) {
    let what = match what {
        TITLE => "the title",
        AUTHOR => "the authors",
        DATE => "the date",
        other => other,
    };
    out.diags.push(Diagnostic::error(
        MetaFailed { what: what.to_string(), detail },
        nowhere(FlmOrigin::synthetic(NO_PLACE_LABEL)),
    ));
}

/// Condition `flm.meta.failed`, an Error: a part of the metadata could not be
/// parsed or built, because its content nests too deeply or because of a fault
/// in the program. Recovery: the piece is left out.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.meta.failed", message = "{what} could not be read: {detail}")]
pub struct MetaFailed {
    /// The part, as an author knows it: `the title`, `the authors`, `the date`, or
    /// `the metadata`.
    pub what: String,
    /// Why the part could not be read.
    pub detail: String,
}
