//! The output regions of the HTML output, and the page the backend's final
//! stage assembles from them.
//!
//! The entry artifact of the `html` output is a **page**: an HTML document a
//! person can open in a browser and a validator can check. A host that wants
//! the HTML of each document tree without a page around it reads the table
//! `flm.output.html` ([`HtmlFragments`]) instead; that table and the page's
//! table `flm.output.html.document` are written in the same run.
//!
//! The page is made in two steps:
//!
//! 1. [`regions`] makes the page's output regions. An output region is one
//!    named part of an output's entry artifact, which the backend writes once
//!    into the table `flm.output.<backend>.regions` ([`Regions`]). On this
//!    backend the regions are the document's content (with every tree placed in
//!    another tree written where it was placed), each piece the render places (a
//!    [piece](crate::core::piece::Piece) is a document-level value a backend
//!    places beside the content, such as the title), the stylesheets, the
//!    scripts, the words of the page's title and its language. The stage `flm.html.regions`
//!    ([`HtmlRegionsStage`](super::stages::HtmlRegionsStage)) calls it and
//!    writes the result into the table `flm.output.html.regions`.
//! 2. [`assemble`] writes the page from those regions; the backend's final
//!    stage ([`HtmlFinalStage`](super::stages::HtmlFinalStage)) calls it. A
//!    host may assemble the same regions through a template of its own instead
//!    (see [output regions and the final
//!    stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
//!
//! # The page's layout
//!
//! The final stage reads the regions alone and takes no template. The page
//! holds, in order: the document type line; `<html>`, with the `lang`
//! attribute when there is a [`LANG`] region; `<head>`, holding the character
//! encoding, the viewport line, the `<title>` holding the words of the
//! [`TITLE`] region, and the stylesheets; then `<body>`, holding
//! `<main id="flm-content" class="flm-content">` and, after it, the scripts.
//! Inside `<main>` come the pieces placed before the content, the content, and
//! the pieces placed after the content.
//!
//! The parts of a region are placed as block-level content
//! ([`HtmlSink::markup_block`](super::HtmlSink::markup_block)), so that the
//! line break this backend writes between two block-level tags is present
//! where a tree begins, between two trees, and before the closing tag of the
//! element they are written into. The bytes of each part are written
//! unchanged, indentation included: re-indenting finished HTML would require
//! reading output back, which this backend's sink does not allow.
//!
//! # Stylesheets and scripts
//!
//! The stylesheets and the scripts are the style and script definitions the
//! document requires, read from the definitions table
//! [`flm.html.definitions`](super::defs::DEFINITIONS_TABLE), in the order that
//! table gives. The [stage that writes that table](super::defs) has already
//! decided each definition's delivery (written into the page, loaded from an
//! address, or left to the host), and the regions stage reads nothing else
//! about definitions:
//!
//! - a text part becomes a `<style>` or a `<script>` holding it;
//! - an address part becomes a `<link rel="stylesheet">` or a
//!   `<script src defer>`;
//! - a file part, which this library does not read, and a text part the page
//!   cannot hold are left out and reported
//!   ([`DefinitionNotInlinable`]);
//! - a definition the host provides itself writes nothing.
//!
//! Every element written for a definition carries the attribute
//! [`DEF_ATTRIBUTE`], so that a reader of the page can tell which definition it
//! came from.
//!
//! # Why this module writes unescaped bytes
//!
//! This module writes three kinds of bytes without escaping them, each through
//! an entry point of the [sink](super::HtmlSink) made for it:
//!
//! - **The HTML of each tree and each piece.** The [emitter](super::emit)
//!   wrote those bytes through the same sink, with all document text escaped
//!   where it was written. The page writes them unchanged as a
//!   [`Markup`] value created by this library; escaping them again would show
//!   the markup instead of rendering it.
//! - **The document type line** ([`DOCTYPE`]), written the same way, because it
//!   is not a tag and the tag builder cannot write it.
//! - **The text of a style or script definition**, written into a `<style>` or
//!   a `<script>` through
//!   [`raw_text_element`](super::HtmlSink::raw_text_element). A browser reads
//!   the content of those two elements character by character, so nothing in
//!   it can be escaped. That text comes from a feature's author or from the
//!   host's settings, never from a document. The only sequence it must not hold
//!   is the element's own closing tag, which that entry point refuses; the
//!   part is then left out and reported.
//!
//! An address a definition carries is written unchanged as an attribute value
//! (escaped like every attribute value). It comes from a feature's author or a
//! host, not from a document, so the [address policy](super::check_address), which
//! decides which addresses a link or an image may carry, does not apply to it,
//! and this library never fetches it.

use std::collections::BTreeSet;

use crate::core::artifact::ArtifactId;
use crate::core::piece::PiecePlace;
use crate::core::region::{CONTENT, LANG, Region, Regions, SCRIPTS, STYLES, TITLE};
use crate::core::registry::DefContent;
use crate::model::addr::TreeId;
use crate::stdcore::features::meta::TITLE as TITLE_PIECE;

use super::conventions::CONTENT_CLASS;
use super::defs::{
    DefinitionKind, DefinitionNotInlinable, NotInlinablePart, ResolvedDefinition,
    ResolvedDefinitions,
};
use super::escape::{escape_attr_into, escape_text_into};
use super::feature::{DefinitionOverride, HtmlRenderOptions};
use super::markup::Markup;
use super::sink::HtmlSink;
use super::stages::{
    ARTIFACT_NAME, ARTIFACT_OWNER, HtmlFragments, HtmlPieces, HtmlSegment, MEDIA_TYPE,
};

/// The first line of every page, `<!DOCTYPE html>`: the document type
/// declaration that puts a browser in its standards mode.
///
/// It is not a tag, so the tag builder cannot write it; [`assemble`] writes it
/// as a [`Markup`] value created by this library. The line break after it is
/// the one the sink writes before the `<html>` element that follows.
pub const DOCTYPE: &str = "<!DOCTYPE html>";

/// The `id` of the `<main>` element holding the document's content,
/// `flm-content`: the same string as its class
/// ([`CONTENT_CLASS`]), so that a host can
/// select the content either way.
pub const CONTENT_ID: &str = "flm-content";

/// The value of the viewport line (`<meta name="viewport">`) every page
/// carries, which tells a small screen to lay the page out at its own width
/// rather than at a desktop's.
pub const VIEWPORT: &str = "width=device-width, initial-scale=1";

/// The attribute every element written for a style or script definition
/// carries, holding that definition's identifier: `data-flm-def`.
///
/// It is the page's own attribute, not one derived from a presentation
/// element. It lets a reader of the page (a person, or a tool that removes a
/// definition again) tell which definition each `<style>`, `<script>` or
/// `<link>` came from.
pub const DEF_ATTRIBUTE: &str = "data-flm-def";

/// The event that a script updating a page in place dispatches on a region's
/// element once that element's content has been replaced:
/// `flm:region-updated`.
///
/// It is one of the two conventions for a page updated in place, which the
/// rendering guide describes ([a page updated in
/// place](crate::guide::rendering#a-page-updated-in-place)); the other is
/// [`ORIGINAL_HTML_ATTRIBUTE`]. The event bubbles, so it reaches a listener on
/// the region's element and one on `document`. Its `detail` holds `region`,
/// the `id` of the region's element, and `artifacts`, the paths of the files
/// beside the page that changed with it. A script of the page that transforms
/// elements in the browser listens to it to transform the region's new
/// elements, as the formula renderer's configuration ([`super::mathjax`])
/// does.
pub const REGION_UPDATED_EVENT: &str = "flm:region-updated";

/// The attribute in which a script that transformed an element in the browser
/// records the HTML the element held before: `data-flm-original-html`.
///
/// It is the second convention for a page updated in place
/// ([`REGION_UPDATED_EVENT`] is the first). The script that updates the page
/// leaves an element carrying this attribute unchanged when the new HTML for
/// that element equals the recorded HTML, and otherwise replaces the element's
/// content, which removes the record; the script that transformed the element
/// then transforms it again when it receives the event. The formula
/// renderer's configuration writes this attribute on each formula just before
/// typesetting it.
pub const ORIGINAL_HTML_ATTRIBUTE: &str = "data-flm-original-html";

/// The result of [`regions`]: the output regions of the HTML output, and what
/// could not be written into them, for the regions stage to report.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HtmlRegions {
    /// The output regions ([`crate::core::region`]): the content, each piece,
    /// the stylesheets, the scripts, the title and, when the options give one,
    /// the language.
    pub regions: Regions,
    /// The trees that a `flm.pres.subtree` node placed and that have no HTML,
    /// in the order encountered; the regions stage reports each as
    /// [`flm.html.missing-subtree`](super::stages::MissingSubtree).
    pub missing: Vec<TreeId>,
    /// The trees that a `flm.pres.subtree` node placed while they were already
    /// being written (a placement cycle), in the order encountered; the regions
    /// stage reports each as
    /// [`flm.html.subtree-cycle`](super::stages::SubtreeCycle).
    pub cycles: Vec<TreeId>,
    /// The parts of required definitions that could not be written, in the
    /// order encountered; the regions stage reports each as
    /// [`flm.html.definition-not-inlinable`](super::defs::DefinitionNotInlinable).
    pub not_inlinable: Vec<DefinitionNotInlinable>,
}

/// Makes the output regions of an HTML page from the HTML of the trees, the
/// HTML of the pieces, the definitions table, the tree order and the render
/// options. The regions stage
/// ([`HtmlRegionsStage`](super::stages::HtmlRegionsStage)) calls it.
///
/// It makes these regions, in this order:
///
/// - [`CONTENT`], from `fragments`: the trees listed in `order`, in that order.
///   Each part of the region is one segment of HTML a tree's emission wrote,
///   so that the final stage places each on a line of its own. A tree that a
///   `flm.pres.subtree` node places in another tree is written where it was
///   placed, and not again at the top level, even when `order` lists it.
///   Trees that are placed only by one another, in a cycle, and by no other
///   tree, are written at the top level after the other trees, so that their
///   content still reaches the page.
/// - One region per piece of `pieces`, with the piece's place and words.
/// - [`STYLES`] and [`SCRIPTS`], from `definitions`: one part per element
///   written for a part of a definition (a `<style>`, a `<link>` or a
///   `<script>`).
/// - [`TITLE`]: the words of the title piece
///   ([`flm.meta.title`](crate::stdcore::features::meta::TITLE)) when `pieces`
///   holds it and its words are not empty, otherwise the title of `options`
///   ([`HtmlRenderOptions::title`]).
/// - [`LANG`], when `options` gives a language ([`HtmlRenderOptions::lang`]).
///
/// The placement that closes a cycle, a placed tree with no HTML, and a
/// definition part the page cannot hold write nothing and are recorded in the
/// result ([`HtmlRegions`]; a cycle in [`HtmlRegions::cycles`]). A tree with no HTML at the top level writes nothing and
/// is not recorded, because the failure that left it without HTML was reported
/// where it happened.
pub fn regions(
    fragments: &HtmlFragments,
    pieces: &HtmlPieces,
    definitions: &ResolvedDefinitions,
    order: &[TreeId],
    options: &HtmlRenderOptions,
) -> HtmlRegions {
    let mut content = Content {
        parts: Vec::new(),
        fragments,
        writing: Vec::new(),
        written: BTreeSet::new(),
        missing: Vec::new(),
        cycles: Vec::new(),
    };
    let placed = placed_trees(fragments);
    for id in order {
        if content.written.contains(id) || placed.contains(id) {
            continue;
        }
        content.top_level(*id);
    }
    // A tree every other tree's placement left unwritten — trees placing each
    // other in a cycle — is written at the top level, so that its content is
    // in the page and the cycle is reported rather than swallowed.
    for id in order {
        if !content.written.contains(id) {
            content.top_level(*id);
        }
    }
    let mut not_inlinable = Vec::new();
    let styles = definition_parts(definitions, DefinitionKind::Style, &mut not_inlinable);
    let scripts = definition_parts(definitions, DefinitionKind::Script, &mut not_inlinable);
    let mut regions =
        Regions::new(ArtifactId::new(ARTIFACT_OWNER, ARTIFACT_NAME), MEDIA_TYPE);
    regions.regions.push(Region::new(CONTENT, content.parts));
    for piece in &pieces.pieces {
        let parts = piece
            .fragment
            .segments
            .iter()
            .filter_map(|segment| match segment {
                HtmlSegment::Html(html) => Some(html.clone()),
                HtmlSegment::Tree(_) => None,
            })
            .collect();
        regions.regions.push(Region::piece(
            piece.name.clone(),
            piece.place,
            parts,
            piece.words.clone(),
        ));
    }
    regions.regions.push(Region::new(STYLES, styles));
    regions.regions.push(Region::new(SCRIPTS, scripts));
    let title = pieces
        .pieces
        .iter()
        .find(|piece| piece.name.as_str() == TITLE_PIECE && !piece.words.is_empty())
        .map_or_else(|| options.title.clone(), |piece| piece.words.clone());
    let mut part = String::new();
    escape_text_into(&mut part, &title);
    regions.regions.push(Region::with_words(TITLE, part, title));
    if let Some(lang) = &options.lang {
        let mut part = String::new();
        escape_attr_into(&mut part, lang);
        regions.regions.push(Region::with_words(LANG, part, lang.clone()));
    }
    let Content { missing, cycles, .. } = content;
    HtmlRegions { regions, missing, cycles, not_inlinable }
}

/// Returns the page the backend's final stage writes, assembled from
/// `regions`: the stylesheets in the head; in `<main>`, the pieces placed
/// before the content, the content and the pieces placed after it; the
/// scripts at the end of the body; the language and the title's words from
/// the regions [`LANG`] and [`TITLE`]. The [module
/// documentation](self#the-pages-layout) gives the full layout.
///
/// A region missing from `regions` contributes nothing. The page ends with a
/// line break.
pub fn assemble(regions: &Regions) -> String {
    let words = |name: &str| regions.get(name).and_then(|region| region.words.as_deref());
    let mut sink = HtmlSink::new();
    sink.markup(&Markup::from_library(DOCTYPE));
    let mut html = sink.open_block("html");
    if let Some(lang) = words(LANG) {
        html = html.attr("lang", lang);
    }
    html.finish();
    sink.open_block("head").finish();
    sink.void_block("meta").attr("charset", "utf-8").finish();
    sink.void_block("meta").attr("name", "viewport").attr("content", VIEWPORT).finish();
    sink.open_block("title").finish();
    sink.text(words(TITLE).unwrap_or_default());
    sink.close();
    place(&mut sink, regions.get(STYLES));
    sink.close();
    sink.open_block("body").finish();
    sink.open_block("main").class([CONTENT_CLASS]).attr("id", CONTENT_ID).finish();
    for piece in regions.pieces_at(PiecePlace::BeforeContent) {
        place(&mut sink, Some(piece));
    }
    place(&mut sink, regions.get(CONTENT));
    for piece in regions.pieces_at(PiecePlace::AfterContent) {
        place(&mut sink, Some(piece));
    }
    sink.close();
    place(&mut sink, regions.get(SCRIPTS));
    sink.close();
    sink.close();
    let mut text = sink.finish();
    text.push('\n');
    text
}

/// Writes every part of `region` as block-level content, each on a line of
/// its own; writes nothing when `region` is `None`.
fn place(sink: &mut HtmlSink, region: Option<&Region>) {
    let Some(region) = region else { return };
    for part in &region.parts {
        // The bytes this backend wrote, through the same sink, with every
        // piece of document text already escaped. They are placed as
        // block-level content, so that the line break between two
        // block-level tags is there at the seam as it is inside a tree; the
        // bytes themselves keep the indentation they were written with, since
        // re-indenting them where they land would be a pass over finished
        // output.
        // See decision rationale at [§flm-dd-dr:rend-html-format].
        sink.markup_block(&Markup::from_library(part.clone()));
    }
}

/// Returns the parts of every definition of kind `kind` in `definitions`, in
/// the table's order: one element per part, each written by a sink of its
/// own. A part that cannot be written is pushed onto `not_inlinable` instead.
fn definition_parts(
    definitions: &ResolvedDefinitions,
    kind: DefinitionKind,
    not_inlinable: &mut Vec<DefinitionNotInlinable>,
) -> Vec<String> {
    let mut parts = Vec::new();
    for definition in definitions.of_kind(kind) {
        // A definition the host supplies itself writes nothing.
        let DefinitionOverride::Parts(contents) = &definition.delivery else { continue };
        for content in contents {
            let mut sink = HtmlSink::new();
            let written = match content {
                DefContent::Inline(text) => definition_text(&mut sink, definition, text),
                DefContent::Url(address) => {
                    definition_address(&mut sink, definition, address);
                    Ok(())
                }
                // This library opens no file. The host reads it and delivers
                // its content itself, through the `definitions` option. One
                // condition serves this and the refused text below, since the
                // recovery is the same.
                // See decision rationale at [§flm-dd-dr:rend-html-definition-delivery].
                DefContent::File(path) => Err(DefinitionNotInlinable {
                    definition: definition.id.to_string(),
                    part: NotInlinablePart::File { path: path.clone() },
                }),
                // The resolve stage puts the table's text in the part's
                // place; a part still naming its table came from a table built
                // by other means, and is refused as a file is.
                DefContent::Table(table) => Err(DefinitionNotInlinable {
                    definition: definition.id.to_string(),
                    part: NotInlinablePart::Table { table: table.to_string() },
                }),
            };
            match written {
                Ok(()) => parts.push(sink.finish()),
                Err(refused) => not_inlinable.push(refused),
            }
        }
    }
    parts
}

/// Writes a text part as a `<style>` or a `<script>` holding it.
///
/// The content of those two elements cannot be escaped, because a browser
/// reads it character by character, so the sink refuses text holding the
/// element's own closing tag; the refusal is returned and nothing is written.
fn definition_text(
    sink: &mut HtmlSink,
    definition: &ResolvedDefinition,
    text: &str,
) -> Result<(), DefinitionNotInlinable> {
    let tag = match definition.kind {
        DefinitionKind::Style => "style",
        DefinitionKind::Script => "script",
    };
    // The one place this backend writes bytes that were never escaped and are
    // not its own: a definition's content, written by a feature's author or
    // put here by the host's settings.
    sink.raw_text_element(tag, &[(DEF_ATTRIBUTE, definition.id.as_str())], text).map_err(
        |refused| DefinitionNotInlinable {
            definition: definition.id.to_string(),
            part: NotInlinablePart::ClosingTag { tag: refused.tag, at: refused.at },
        },
    )
}

/// Writes an address part as a `<link rel="stylesheet">` or a
/// `<script src defer>`.
///
/// The address comes from a feature's author or a host, so it is written
/// unchanged (escaped as an attribute value); this library never fetches it.
/// A script is written with `defer`, which lets a browser finish reading the
/// page before the script runs and runs deferred scripts in the order they
/// were written.
fn definition_address(sink: &mut HtmlSink, definition: &ResolvedDefinition, address: &str) {
    match definition.kind {
        DefinitionKind::Style => {
            sink.void_block("link")
                .attr("rel", "stylesheet")
                .attr("href", address)
                .attr(DEF_ATTRIBUTE, definition.id.as_str())
                .finish();
        }
        DefinitionKind::Script => {
            sink.open_block("script")
                .attr("src", address)
                .flag("defer")
                .attr(DEF_ATTRIBUTE, definition.id.as_str())
                .finish();
            sink.close();
        }
    }
}

/// Returns the trees that another tree's HTML places (a tree placing itself is
/// a cycle: it is written at the top level and reported).
fn placed_trees(fragments: &HtmlFragments) -> BTreeSet<TreeId> {
    let mut placed = BTreeSet::new();
    for (id, fragment) in &fragments.trees {
        for segment in &fragment.segments {
            if let HtmlSegment::Tree(tree) = segment {
                if tree != id {
                    placed.insert(*tree);
                }
            }
        }
    }
    placed
}

/// The content region while it is being made.
struct Content<'f> {
    parts: Vec<String>,
    fragments: &'f HtmlFragments,
    /// The trees being written, outermost first (the placement cycle guard).
    writing: Vec<TreeId>,
    /// The trees written so far, at the top level or where they were placed.
    written: BTreeSet<TreeId>,
    missing: Vec<TreeId>,
    cycles: Vec<TreeId>,
}

impl Content<'_> {
    /// One of the document's own trees, at the top level of the content.
    ///
    /// A tree with no HTML — one whose lowering or emission failed — writes
    /// nothing and is not reported here: the failure was reported where it
    /// happened.
    fn top_level(&mut self, id: TreeId) {
        self.written.insert(id);
        if self.fragments.trees.contains_key(&id) {
            self.tree(id);
        }
    }

    /// A tree another tree places, written where it was placed.
    ///
    /// Unlike a tree of the document's own, one that is placed and has no HTML
    /// is reported: the node that placed it asked for something the page could
    /// not give.
    fn placed(&mut self, id: TreeId) {
        if self.writing.contains(&id) {
            self.cycles.push(id);
            return;
        }
        if !self.fragments.trees.contains_key(&id) {
            self.missing.push(id);
            return;
        }
        self.tree(id);
    }

    /// One tree's HTML, with the trees it places written where they are
    /// placed.
    fn tree(&mut self, id: TreeId) {
        let Some(fragment) = self.fragments.trees.get(&id) else { return };
        self.writing.push(id);
        self.written.insert(id);
        for segment in &fragment.segments {
            match segment {
                HtmlSegment::Html(html) => self.parts.push(html.clone()),
                HtmlSegment::Tree(placed) => self.placed(*placed),
            }
        }
        self.writing.pop();
    }
}
