//! Pieces: document-level synthesized values that a backend places before or
//! after the document's content. This module holds the declaration of a piece
//! ([`PieceDecl`], [`PiecePlace`]) and the type of its content ([`Piece`]).
//!
//! A piece is a document-level synthesized value that a backend places before
//! or after the document's content (the title, the list of footnotes, the
//! bibliography), built once by its builder stage into a table named for the
//! piece and rendered by each backend with its own rules
//! ([pieces](crate::guide::concepts_overview#pieces)). A synthesized value is
//! a small tree that a stage builds while a run is under way, held outside the
//! document's trees ([synthesized
//! values](crate::guide::concepts_overview#synthesized-values)).
//!
//! Three things distinguish a piece from the document's content: the author
//! does not write it where it appears; a stage *builds* it from what the
//! document contains, once, over the whole document; and the backend *places*
//! it at the place its declaration gives (before the content or after it),
//! not at a position in a tree.
//!
//! A piece has three parts. A feature that offers a piece writes the first two
//! with the items of this module; the backends supply the third.
//!
//! 1. **Its content** is a [`Piece`]: a small tree built from document
//!    elements (the elements the feature declares, text, references,
//!    mathematics), held in a table named for the piece. The content is a
//!    synthesized value: a consumer brings it up to date before reading it as
//!    document content ([value
//!    catch-up](crate::guide::concepts_overview#value-catch-up)). The stage
//!    that writes the table is the piece's **builder**. Because the content is
//!    a table, another stage that writes the same table replaces the builder
//!    when a plan request prefers that stage.
//! 2. **Its declaration** is a [`PieceDecl`]: the piece's name, the table
//!    holding its content, where it is placed, and whether a render places it
//!    by default. The feature contributes it to the service slot `flm.pieces`
//!    that the `pieces` feature owns
//!    ([`stdcore::backends::pieces::SLOT`](crate::stdcore::backends::pieces::SLOT)).
//! 3. **Its rendering** belongs to each backend: a per-backend stage lowers
//!    the content with the lowering rules the plan selected for that backend,
//!    and the backend writes the result into its output, as an [output
//!    region](crate::core::region) of its own on HTML and on the recomposed
//!    LaTeX, inside the printed text on the text backend. That machinery is in
//!    [`crate::stdcore::backends::pieces`].
//!
//! Because the content is built from document elements, and never a string or
//! a tree of one backend's own elements, one builder serves every backend, and
//! a stage that replaces the builder changes the piece on every output at
//! once.
//!
//! A piece in this sense is unrelated to a table piece, the part of one table
//! that one stage invocation produced ([`Table::absorb`], [`PieceClass`]).
//!
//! This module depends on no backend and no feature: it holds only the values
//! a feature declares a piece with.

use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::serialize::SerialValue;

use crate::core::data::table::{PieceClass, ProjectCx, Table, TableHandle};
use crate::lang::Flm;
use crate::model::names::Name;

/// Where a backend places a piece: before or after the document's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PiecePlace {
    /// Before the document's first tree.
    BeforeContent,
    /// After the document's last tree.
    AfterContent,
}

impl PiecePlace {
    /// Returns the place's name as a serialized table or a message writes
    /// it: `before-content` or `after-content`.
    pub fn as_str(self) -> &'static str {
        match self {
            PiecePlace::BeforeContent => "before-content",
            PiecePlace::AfterContent => "after-content",
        }
    }
}

/// The declaration of one piece: its name, the table that holds its content,
/// where a backend places it, and whether a render places it by default.
///
/// A feature that offers a piece does three things:
///
/// - it contributes this declaration to the service slot
///   [`flm.pieces`](crate::stdcore::backends::pieces::SLOT) as a peer
///   contribution
///   ([`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot)),
///   so that the feature also builds in a registry without the `pieces`
///   feature;
/// - it declares the table [`PieceDecl::table`], of value type [`Piece`]
///   ([`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table));
/// - it registers the builder stage that writes that table.
///
/// When two pieces have the same place, a backend places them in the order of
/// their contributions to the slot. Which pieces a render places is decided
/// by [`selected_pieces`](crate::stdcore::backends::pieces::selected_pieces).
///
/// # Examples
///
/// ```
/// use flm_core::core::piece::{PieceDecl, PiecePlace};
///
/// let decl = PieceDecl::new("my.notes", PiecePlace::AfterContent, "the notes");
/// assert_eq!(decl.table.name().as_str(), "my.notes");
/// assert!(decl.default_on);
/// assert!(!decl.default_on(false).default_on);
/// ```
#[derive(Debug, Clone)]
pub struct PieceDecl {
    /// The piece's name: a dotted name the contributing feature owns, such as
    /// `mypkg.notes`. The render options
    /// ([`PiecesRenderOptions`](crate::stdcore::backends::pieces::PiecesRenderOptions))
    /// refer to the piece by this name.
    pub name: Name,
    /// The table holding the piece's content, whose value type is always
    /// [`Piece`]. [`PieceDecl::new`] gives it the piece's name; the builder
    /// stage declares it as a table it writes
    /// ([`StageDecl::writes`](crate::core::stage::StageDecl::writes)).
    pub table: TableHandle<Piece>,
    /// Where a backend places the piece.
    pub place: PiecePlace,
    /// Whether every render places the piece unless the render options leave
    /// it out. A piece declared `false` is placed only when the render options
    /// include it.
    pub default_on: bool,
    /// A description of the piece, written by the feature that offers it.
    pub docs: String,
}

impl PieceDecl {
    /// Returns the declaration of the piece `name`, placed at `place` and
    /// placed by default, whose content is held in the table of the same name
    /// as the piece, with the description `docs`.
    pub fn new(name: impl Into<Name>, place: PiecePlace, docs: impl Into<String>) -> PieceDecl {
        let name = name.into();
        let table = TableHandle::new(name.clone());
        PieceDecl { name, table, place, default_on: true, docs: crate::kept_docs(docs) }
    }

    /// Sets whether every render places the piece unless the render options
    /// leave it out (see [`default_on`](PieceDecl#structfield.default_on)),
    /// and returns the declaration.
    pub fn default_on(mut self, on: bool) -> PieceDecl {
        self.default_on = on;
        self
    }
}

/// The content of one piece: a synthesized value built from document
/// elements, or nothing to place.
///
/// This is the value type of every table that holds a piece's content, so
/// that a backend's pieces stage (the stage that lowers the pieces for that
/// backend, [`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage))
/// reads any piece by its name alone. A content of `None` places nothing and
/// reports nothing: a list of notes in a document without notes is not an
/// error.
///
/// The content is a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values), which a backend
/// brings up to date before lowering it
/// ([`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value)). A
/// synthesized value must not state a fact about the document as a whole: a
/// labeled reference point inside it is refused when it is brought up to date,
/// the backend's pieces stage reports `flm.pieces.lowering-failed`
/// ([`PieceLoweringFailed`](crate::stdcore::backends::pieces::PieceLoweringFailed)),
/// and the piece is left out of that backend's output.
///
/// The piece's builder stage (the stage that builds its content) writes the
/// table once; a second value combined into the table replaces the first
/// ([`Table::absorb`]). The serialized form of the table
/// ([`Table::project`]) is a map whose key `content` holds the tree
/// ([`ProjectCx::tree`]), or an empty map when the piece holds nothing.
#[derive(Debug, Clone, Default)]
pub struct Piece {
    /// The piece's content, or `None` when there is nothing to place.
    pub content: Option<Arc<NodeTree<Flm>>>,
}

impl Piece {
    /// Returns a piece holding `content`.
    pub fn new(content: Arc<NodeTree<Flm>>) -> Piece {
        Piece { content: Some(content) }
    }
}

impl Table for Piece {
    fn absorb(&mut self, piece: Piece) {
        *self = piece;
    }

    /// `{content: <the content's tree>}` when the piece holds content
    /// ([`ProjectCx::tree`]), an empty map otherwise.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        match &self.content {
            Some(content) => SerialValue::Map(vec![(String::from("content"), cx.tree(content))]),
            None => SerialValue::Map(Vec::new()),
        }
    }

    fn piece_class(&self) -> PieceClass {
        if self.content.is_some() { PieceClass::DocumentFact } else { PieceClass::Empty }
    }
}
