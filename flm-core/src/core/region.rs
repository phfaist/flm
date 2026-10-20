//! Output regions: the named parts an output's entry artifact is made of, the
//! table in which a backend writes them, and a backend's statement that its
//! output is made of them.
//!
//! An output region is one named part of an output's entry artifact (the
//! document's content, each [piece](crate::core::piece) placed beside it, and
//! on HTML the stylesheets, the scripts, the title and the language), which
//! the backend writes once into the table `flm.output.<backend>.regions`
//! ([`Regions`]). The entry artifact is the artifact that represents the
//! output as a whole: the HTML page, the printed text, the recomposed LaTeX
//! document ([`ArtifactSet`]).
//!
//! The backend's final stage assembles the entry artifact from those regions,
//! in a layout of the backend's own, and publishes the result as the output's
//! artifact set. A host may instead assemble the same regions through a
//! template of its own, with a stage that the plan schedules in place of the
//! backend's final stage ([output regions and the final
//! stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
//! Either way the regions are computed once. The [`Regions`] table also states
//! what the entry artifact is (its identity, its media type and the artifacts
//! it refers to), so that every assembler makes the same artifact from one
//! statement.
//!
//! This module holds the region names (below), one region ([`Region`]), the
//! regions of one output ([`Regions`]), the function that computes a piece's
//! plain words ([`piece_words`]), and a backend's statement that an output is
//! made of regions ([`RegionsOutput`]).
//!
//! # The region names
//!
//! | Name | What the region holds | Backends |
//! |---|---|---|
//! | [`CONTENT`] | the document's content: every tree in document order; a tree placed inside another tree is written where it is placed | every backend (the text backend writes its pieces inside this region too) |
//! | a piece's name | that piece, as the backend writes it | HTML, recomposed LaTeX |
//! | [`STYLES`] | the stylesheets the document requires, one part per HTML element (`<style>` or `<link>`) | HTML |
//! | [`SCRIPTS`] | the scripts the document requires, one part per `<script>` element | HTML |
//! | [`TITLE`] | the words of the page's title: the document's title, else the title the HTML render options ([`HtmlRenderOptions`](crate::stdcore::backends::html::HtmlRenderOptions)) give | HTML |
//! | [`LANG`] | the page's language tag, when the HTML render options give a language; absent otherwise | HTML |
//! | [`STYLE_PACKAGE`] | the package name of the style file, when the output has a style file | recomposed LaTeX |
//!
//! # Which outputs are made of regions
//!
//! A backend whose output is made of regions states it by contributing a
//! [`RegionsOutput`] to the service slot
//! [`flm.output.regions`](crate::stdcore::backends::lowering::REGIONS_SLOT):
//! the output, its backend, the table of its regions, the tables of the files
//! written beside its entry artifact, and the capability its final stage
//! provides. A stage that assembles the regions another way, such as a
//! host's template stage, reads that service slot to learn which outputs it can
//! serve. It
//! therefore serves exactly the outputs of the backends the registry holds.
//!
//! # Parts
//!
//! A region is a list of **parts**, each a text the backend wrote in its
//! output format (HTML, LaTeX or plain text). The HTML backend's final stage places each part of a
//! region on a line of its own, at the indentation of the place where the
//! region goes. A template places the region's [`text`](Region::text): its
//! parts joined by line breaks. A region that is one text has one part.

use techy::core::node::NodeTree;
use techy::serialize::SerialValue;

use crate::core::artifact::{ArtifactId, ArtifactSet};
use crate::core::data::table::{PieceClass, ProjectCx, Table, TableHandle};
use crate::core::piece::PiecePlace;
use crate::lang::Flm;
use crate::model::names::{BackendName, CapName, Name, OutputName};

/// The name of the output region holding the document's content, `content`.
pub const CONTENT: &str = "content";
/// The name of the output region holding the stylesheets an HTML page loads,
/// `styles`.
pub const STYLES: &str = "styles";
/// The name of the output region holding the scripts an HTML page loads,
/// `scripts`.
pub const SCRIPTS: &str = "scripts";
/// The name of the output region holding the words of an HTML page's title,
/// `title`.
///
/// The words are those of the document's title piece when the render places
/// that piece and its words are not empty, else the title the HTML render
/// options give
/// ([`HtmlRenderOptions`](crate::stdcore::backends::html::HtmlRenderOptions)).
/// The region's [`words`](Region::words) are the plain text, and its one part
/// is that text escaped as HTML text.
pub const TITLE: &str = "title";
/// The name of the output region holding an HTML page's language tag, `lang`.
///
/// The region is present only when the HTML render options
/// ([`HtmlRenderOptions`](crate::stdcore::backends::html::HtmlRenderOptions))
/// give a language. Its [`words`](Region::words) are the tag, and its one
/// part is the tag escaped as the value of an HTML attribute.
pub const LANG: &str = "lang";
/// The name of the output region holding the package name of the recomposed
/// LaTeX's style file, `style-package`: the name with which a LaTeX document
/// loads the style file through `\usepackage`.
pub const STYLE_PACKAGE: &str = "style-package";

/// One output region: its name, whether it is a piece, its parts, and the
/// plain words of a piece or of the HTML title and language.
///
/// Build one with [`Region::new`], [`Region::with_words`] or
/// [`Region::piece`]. A backend's regions stage pushes the regions of its
/// output into a [`Regions`] table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// The region's name: one of the names in the [module
    /// documentation](self#the-region-names), or a piece's name.
    pub name: Name,
    /// Where the piece is placed, when the region is a piece; `None` for every
    /// other region.
    pub piece: Option<PiecePlace>,
    /// The region's parts, in order, each written by the backend in its
    /// output format (see the [module documentation](self#parts)).
    pub parts: Vec<String>,
    /// The region's **words**: its displayed characters as plain text, for a
    /// place in the entry artifact where no markup is allowed, such as the
    /// title of an HTML page or the value of an attribute. Set for a piece and
    /// for the HTML regions [`TITLE`] and [`LANG`]; `None` for every other
    /// region.
    pub words: Option<String>,
}

impl Region {
    /// Returns a region named `name` holding `parts`, which is not a piece
    /// and has no words.
    pub fn new(name: impl Into<Name>, parts: Vec<String>) -> Region {
        Region { name: name.into(), piece: None, parts, words: None }
    }

    /// Returns a region named `name`, not a piece, holding the one part
    /// `part`, with the words `words`: the form of the HTML regions [`TITLE`]
    /// and [`LANG`].
    pub fn with_words(name: impl Into<Name>, part: String, words: String) -> Region {
        Region { name: name.into(), piece: None, parts: vec![part], words: Some(words) }
    }

    /// Returns the region of the piece `name`, placed at `place`, holding
    /// `parts`, with the words `words` (computed by [`piece_words`]).
    pub fn piece(
        name: impl Into<Name>,
        place: PiecePlace,
        parts: Vec<String>,
        words: String,
    ) -> Region {
        Region { name: name.into(), piece: Some(place), parts, words: Some(words) }
    }

    /// Returns the region as one text: its parts joined by line breaks.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::region::Region;
    ///
    /// let region = Region::new("styles", vec![String::from("<style>a</style>"), String::from("<style>b</style>")]);
    /// assert_eq!(region.text(), "<style>a</style>\n<style>b</style>");
    /// ```
    pub fn text(&self) -> String {
        self.parts.join("\n")
    }
}

/// Returns the words of a piece whose lowered tree is `tree`, as its region
/// carries them ([`Region::words`]): the tree's plain text
/// ([`plain_text`](crate::lang::text::plain_text)), with every run of white
/// space replaced by one space and none at either end.
///
/// The HTML and recomposed LaTeX backends compute a piece's words with this
/// function, so that both give the same words for the same piece.
pub fn piece_words(tree: &NodeTree<Flm>) -> String {
    crate::lang::text::plain_text(tree).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The output regions of one output, and what the entry artifact assembled
/// from them is: the value of the table `flm.output.<backend>.regions`, which
/// a backend's regions stage writes once per run.
///
/// The backend's final stage, or a host's template stage, reads it to make
/// the entry artifact; see the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regions {
    /// The identity of the entry artifact.
    pub entry: ArtifactId,
    /// The entry artifact's media type. Every assembler, the backend's final
    /// stage and a host's template stage alike, takes it from here.
    pub media_type: String,
    /// The artifacts the entry artifact refers to, among those the backend
    /// writes beside it, such as the style file of the recomposed LaTeX. The
    /// final stage copies them into the entry artifact's
    /// [`refs`](crate::core::artifact::Artifact::refs).
    pub refs: Vec<ArtifactId>,
    /// The regions, in the order the backend wrote them; no name occurs
    /// twice.
    pub regions: Vec<Region>,
}

impl Regions {
    /// Returns a table with no region yet, for an entry artifact of identity
    /// `entry` and media type `media_type`.
    pub fn new(entry: ArtifactId, media_type: impl Into<String>) -> Regions {
        Regions { entry, media_type: media_type.into(), refs: Vec::new(), regions: Vec::new() }
    }

    /// Returns the region named `name`, or `None` when there is none.
    pub fn get(&self, name: &str) -> Option<&Region> {
        self.regions.iter().find(|region| region.name.as_str() == name)
    }

    /// Returns the regions that are pieces placed at `place`, in order.
    pub fn pieces_at(&self, place: PiecePlace) -> impl Iterator<Item = &Region> + '_ {
        self.regions.iter().filter(move |region| region.piece == Some(place))
    }

    /// Returns the regions that are pieces, in order.
    pub fn pieces(&self) -> impl Iterator<Item = &Region> + '_ {
        self.regions.iter().filter(|region| region.piece.is_some())
    }
}

impl Default for Regions {
    /// Returns an empty table: no region, and an entry artifact whose identity
    /// and media type are empty, which the backend's regions stage replaces.
    fn default() -> Regions {
        Regions::new(ArtifactId::new("", ""), "")
    }
}

impl Table for Regions {
    /// Replaces the table's value: one stage per run writes the table, the
    /// backend's regions stage, so a second table piece replaces the first.
    fn absorb(&mut self, piece: Regions) {
        *self = piece;
    }

    /// `{entry, media_type, refs: [id], regions: [{name, piece?, parts: [text], words?}]}`.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        let regions = self
            .regions
            .iter()
            .map(|region| {
                let mut entries =
                    vec![(String::from("name"), SerialValue::Str(region.name.to_string()))];
                if let Some(place) = region.piece {
                    entries.push((String::from("piece"), SerialValue::Str(place.as_str().into())));
                }
                entries.push((
                    String::from("parts"),
                    SerialValue::List(
                        region.parts.iter().map(|part| SerialValue::Str(part.clone())).collect(),
                    ),
                ));
                if let Some(words) = &region.words {
                    entries.push((String::from("words"), SerialValue::Str(words.clone())));
                }
                SerialValue::Map(entries)
            })
            .collect();
        SerialValue::Map(vec![
            (String::from("entry"), SerialValue::Str(self.entry.to_string())),
            (String::from("media_type"), SerialValue::Str(self.media_type.clone())),
            (
                String::from("refs"),
                SerialValue::List(
                    self.refs.iter().map(|id| SerialValue::Str(id.to_string())).collect(),
                ),
            ),
            (String::from("regions"), SerialValue::List(regions)),
        ])
    }

    fn piece_class(&self) -> PieceClass {
        PieceClass::DocumentFact
    }
}

/// A regions output: a backend's statement that its output is made of output
/// regions (which table holds them and which capability its final stage
/// provides), contributed to the service slot `flm.output.regions` so that a
/// host's own assembler can read it.
///
/// A backend contributes one per such output to the service slot
/// [`flm.output.regions`](crate::stdcore::backends::lowering::REGIONS_SLOT);
/// see the [module documentation](self#which-outputs-are-made-of-regions).
#[derive(Debug, Clone)]
pub struct RegionsOutput {
    /// The output's name, with which a caller asks for it.
    pub output: OutputName,
    /// The backend that writes the output's regions.
    pub backend: BackendName,
    /// The name of the output's table that holds its artifact set, which its
    /// final stage writes.
    pub document: CapName,
    /// The capability a request for the output demands: the capability of the
    /// [`document`](RegionsOutput::document) table, or a refinement of it that
    /// the final stage provides. A host's template stage provides this
    /// capability to take the final stage's place.
    pub capability: CapName,
    /// The table of the output's regions, `flm.output.<backend>.regions`.
    pub regions: TableHandle<Regions>,
    /// The tables of the files that the output's stages write beside its
    /// entry artifact, in the order in which they are added to the output's
    /// artifact set ([`ArtifactSet::insert_entry_with`]): the backend's own
    /// files (the recomposed LaTeX's style file), then the output's artifact
    /// table `flm.output.<backend>.artifacts`, which only the `graphics`
    /// feature's publish stage writes (its copies of images). Empty for an
    /// output of one file. An assembler reads each table when a stage of the
    /// plan writes it.
    pub artifacts: Vec<TableHandle<ArtifactSet>>,
}
