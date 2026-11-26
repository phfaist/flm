//! The stage [`BuildMetadata`] (`flm.metadata.build`), which parses the document's
//! metadata and builds the three pieces of the [`metadata`](super) feature, and the
//! diagnostic [`MetadataFailed`] it reports when a part of the metadata cannot be built.

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::source::SourceSpan;

use crate::core::data::store::StageOut;
use crate::core::data::table::TableHandle;
use crate::core::piece::Piece;
use crate::core::plan::PlanCx;
use crate::core::registry::FlmRegistry;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::copy::{copy_pieces_into_value, SynthesizedPiece, ValueContent, ValuePiece};
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::{nowhere, parse_fragment, Flm, FlmOrigin, FlmSource, FlmSpan, ParseOptions, PresPayload};
use crate::model::doc::{Fragment, FragmentLayout};
use crate::stdlib::backends::pres;

use super::{
    AffiliationPayload, DocMeta, EmailPayload, MetaAuthor, MetaText, AUTHOR, AUTHOR_AFFILIATION, AUTHOR_EMAIL,
    AUTHOR_ENTRY, AUTHOR_NAME, AUTHOR_NOTE, BUILD, DATE, READ, TITLE,
};

/// The label of the origin under which a text of the metadata without a place
/// is parsed, `metadata`.
const NO_PLACE_LABEL: &str = "metadata";

/// The stage [`BUILD`] (`flm.metadata.build`), which builds the metadata's pieces:
/// a compute stage that runs once per document. It reads the option [`DocMeta`],
/// writes the tables [`TITLE`], [`AUTHOR`] and [`DATE`], and provides the
/// capability [`READ`].
///
/// Each text (the title, each author's name, affiliations and notes, the date) is
/// parsed as inline FLM content with the registry's language, as a source named
/// after the file the text was written in and counted from the text's line and
/// column there, so that each problem the parse finds is reported at its place in
/// that file. Every problem the parse finds is reported. An author's email address
/// is not parsed.
///
/// The title's content is one [`TITLE`] node holding the parsed content, and the
/// date's one [`DATE`] node. The authors' content is one [`AUTHOR`] node holding
/// one [`AUTHOR_ENTRY`] node per author, in the order given; an author's node
/// holds, in this order, an [`AUTHOR_NAME`] node, one [`AUTHOR_AFFILIATION`] node
/// per affiliation, carrying the affiliation's number ([`AffiliationPayload`]), an
/// [`AUTHOR_EMAIL`] node carrying the address ([`EmailPayload`]) when the author
/// has one, and one [`AUTHOR_NOTE`] node per note. The stage numbers the distinct
/// affiliations of the document from 1, in the order in which they first appear;
/// two affiliations whose texts are the same once the whitespace at both ends is
/// trimmed are the same affiliation. A piece without a text holds no content, and
/// no backend places it.
///
/// A part that cannot be parsed or built is reported as [`MetadataFailed`]. The
/// title or the date is then left out; in the authors, the affiliation or the
/// note is left out, and an author whose name cannot be parsed is left out
/// whole.
#[derive(Debug, Default, Clone, Copy)]
pub struct BuildMetadata;

impl Stage for BuildMetadata {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD, Effect::Compute, Scope::Document)
            .writes(TableHandle::<Piece>::new(TITLE))
            .writes(TableHandle::<Piece>::new(AUTHOR))
            .writes(TableHandle::<Piece>::new(DATE))
            .provides(READ)
            .docs(
                "Parses the document's title, authors and date as inline content and builds the \
                 pieces `flm.metadata.title`, `flm.metadata.author` and `flm.metadata.date`.",
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

/// Returns the source of `text`: named after the file the text was written in and
/// counted from the text's line and column there, or under the label
/// [`NO_PLACE_LABEL`] for a text without a place.
fn source(text: &MetaText) -> Arc<FlmSource> {
    let source = FlmSource::new(text.text.clone());
    Arc::new(match &text.place {
        Some(place) => source
            .with_origin(FlmOrigin::new(place.file.clone()))
            .with_line_column_number_offsets(place.line as usize, place.column as usize),
        None => source.with_origin(FlmOrigin::synthetic(NO_PLACE_LABEL)),
    })
}

/// Returns `text` parsed as inline content, with every problem of the parse
/// reported into `out`; `None` when the parse fails altogether, which is reported
/// too.
fn parse(registry: &FlmRegistry, what: &str, text: &MetaText, out: &mut StageOut<'_>) -> Option<Fragment> {
    match parse_fragment(registry.lang(), source(text), FragmentLayout::Inline, &ParseOptions::default()) {
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

/// One author, parsed: the name, each affiliation with its text, the email
/// address with its span, and each note. An affiliation or a note that could not
/// be parsed is left out.
struct ParsedAuthor<'m> {
    name: Fragment,
    affiliations: Vec<(Fragment, &'m str)>,
    email: Option<(&'m str, FlmSpan)>,
    notes: Vec<Fragment>,
}

/// The specifications of the elements of the authors piece.
struct AuthorSpecs {
    root: Arc<FlmSynthesizedSpec>,
    entry: Arc<FlmSynthesizedSpec>,
    name: Arc<FlmSynthesizedSpec>,
    affiliation: Arc<FlmSynthesizedSpec>,
    email: Arc<FlmSynthesizedSpec>,
    note: Arc<FlmSynthesizedSpec>,
}

impl AuthorSpecs {
    fn of(registry: &FlmRegistry) -> Result<AuthorSpecs, String> {
        Ok(AuthorSpecs {
            root: spec(registry, AUTHOR)?,
            entry: spec(registry, AUTHOR_ENTRY)?,
            name: spec(registry, AUTHOR_NAME)?,
            affiliation: spec(registry, AUTHOR_AFFILIATION)?,
            email: spec(registry, AUTHOR_EMAIL)?,
            note: spec(registry, AUTHOR_NOTE)?,
        })
    }
}

/// Returns the content nodes of a parsed text: the children of its root.
fn content(fragment: &Fragment) -> Vec<NodeRef<'_, Flm>> {
    fragment.tree().root().children().iter().collect()
}

/// Returns a node of the element of `spec` holding `nodes`, the content of the
/// parsed text `fragment`, at the text's span and carrying `payload`.
fn part<'n, 't>(
    spec: &Arc<FlmSynthesizedSpec>,
    fragment: &'t Fragment,
    nodes: &'n [NodeRef<'t, Flm>],
    payload: Option<Arc<dyn PresPayload>>,
) -> SynthesizedPiece<'n, 't> {
    let root = fragment.tree().root();
    SynthesizedPiece {
        spec: Arc::clone(spec),
        slot: pres::BODY_SLOT,
        span: root.span().clone(),
        state: Arc::clone(root.parsing_state()),
        body: body(nodes),
        payload,
    }
}

/// Returns the content of the authors piece: one [`AUTHOR`] node holding one
/// [`AUTHOR_ENTRY`] node per author, in the order given; `None` when there is no
/// author, or when no author's name could be parsed.
///
/// Each text but the email address is parsed as inline content. An author whose
/// name cannot be parsed is left out, and so is an affiliation or a note that
/// cannot be parsed. The distinct affiliations are numbered from 1 in the order
/// in which they first appear, two affiliations being the same when their texts
/// are the same once the whitespace at both ends is trimmed
/// ([`AffiliationPayload`]).
fn authors(registry: &FlmRegistry, authors: &[MetaAuthor], out: &mut StageOut<'_>) -> Option<Arc<NodeTree<Flm>>> {
    let mut parsed: Vec<ParsedAuthor<'_>> = Vec::new();
    for author in authors {
        let name = parse(registry, AUTHOR, &author.name, out);
        let affiliations = (author.affiliations.iter())
            .filter_map(|text| Some((parse(registry, AUTHOR, text, out)?, text.text.as_str())))
            .collect();
        let notes = author.notes.iter().filter_map(|text| parse(registry, AUTHOR, text, out)).collect();
        let email = author.email.as_ref().map(|text| {
            (text.text.as_str(), SourceSpan::new(&source(text), 0..text.text.len()))
        });
        if let Some(name) = name {
            parsed.push(ParsedAuthor { name, affiliations, email, notes });
        }
    }
    let first = parsed.first()?.name.tree().root();
    let mut distinct: Vec<&str> = Vec::new();
    let mut numbers: Vec<Vec<usize>> = Vec::new();
    for author in &parsed {
        let mut these = Vec::new();
        for (_, text) in &author.affiliations {
            let text = text.trim();
            let at = distinct.iter().position(|known| *known == text).unwrap_or_else(|| {
                distinct.push(text);
                distinct.len() - 1
            });
            these.push(at + 1);
        }
        numbers.push(these);
    }
    let built = AuthorSpecs::of(registry).and_then(|specs| {
        // The content nodes of every text, which the parts' nodes borrow.
        let names: Vec<Vec<NodeRef<'_, Flm>>> = parsed.iter().map(|author| content(&author.name)).collect();
        let affiliations: Vec<Vec<Vec<NodeRef<'_, Flm>>>> = (parsed.iter())
            .map(|author| author.affiliations.iter().map(|(fragment, _)| content(fragment)).collect())
            .collect();
        let notes: Vec<Vec<Vec<NodeRef<'_, Flm>>>> =
            parsed.iter().map(|author| author.notes.iter().map(content).collect()).collect();
        let parts: Vec<Vec<SynthesizedPiece<'_, '_>>> = (parsed.iter().enumerate())
            .map(|(at, author)| {
                let root = author.name.tree().root();
                let mut parts = vec![part(&specs.name, &author.name, &names[at], None)];
                for (((fragment, _), nodes), number) in
                    author.affiliations.iter().zip(&affiliations[at]).zip(&numbers[at])
                {
                    let payload = AffiliationPayload { number: *number };
                    parts.push(part(&specs.affiliation, fragment, nodes, Some(Arc::new(payload))));
                }
                if let Some((address, span)) = &author.email {
                    parts.push(SynthesizedPiece {
                        spec: Arc::clone(&specs.email),
                        slot: pres::BODY_SLOT,
                        span: span.clone(),
                        state: Arc::clone(root.parsing_state()),
                        body: Vec::new(),
                        payload: Some(Arc::new(EmailPayload { address: address.to_string() })),
                    });
                }
                for (fragment, nodes) in author.notes.iter().zip(&notes[at]) {
                    parts.push(part(&specs.note, fragment, nodes, None));
                }
                parts
            })
            .collect();
        let entries: Vec<SynthesizedPiece<'_, '_>> = (parsed.iter().zip(&parts))
            .map(|(author, parts)| {
                let root = author.name.tree().root();
                SynthesizedPiece {
                    spec: Arc::clone(&specs.entry),
                    slot: pres::BODY_SLOT,
                    span: root.span().clone(),
                    state: Arc::clone(root.parsing_state()),
                    body: parts.iter().map(ValuePiece::Synthesized).collect(),
                    payload: None,
                }
            })
            .collect();
        let node = SynthesizedPiece {
            spec: Arc::clone(&specs.root),
            slot: pres::BODY_SLOT,
            span: first.span().clone(),
            state: Arc::clone(first.parsing_state()),
            body: entries.iter().map(ValuePiece::Synthesized).collect(),
            payload: None,
        };
        copy_pieces_into_value(&[ValuePiece::Synthesized(&node)], &first, ValueContent::Own, registry.lang().guard())
            .map_err(|error| error.to_string())
    });
    built.map_err(|detail| failed(out, AUTHOR, detail)).ok()
}

/// Reports that `what` (one of the three pieces, or `the metadata`) could not be
/// built, as the Error [`MetadataFailed`], with the part written as an author
/// knows it.
fn failed(out: &mut StageOut<'_>, what: &str, detail: String) {
    let what = match what {
        TITLE => "the title",
        AUTHOR => "the authors",
        DATE => "the date",
        other => other,
    };
    out.diags.push(Diagnostic::error(
        MetadataFailed { what: what.to_string(), detail },
        nowhere(FlmOrigin::synthetic(NO_PLACE_LABEL)),
    ));
}

/// Condition `flm.metadata.failed`, an Error: a part of the metadata could not be
/// parsed or built, because its content nests too deeply or because of a fault
/// in the program. Recovery: the title or the date is left out; in the authors,
/// the affiliation or the note is left out, and an author whose name cannot be
/// parsed is left out whole.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.metadata.failed", message = "{what} could not be read: {detail}")]
pub struct MetadataFailed {
    /// The part, as an author knows it: `the title`, `the authors`, `the date`, or
    /// `the metadata`.
    pub what: String,
    /// Why the part could not be read.
    pub detail: String,
}
