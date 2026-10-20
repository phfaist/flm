//! Rendering: the backends of the `stdcore` package, and what every backend
//! shares — the presentation vocabulary, the lowering stage and the pieces
//! stage.
//!
//! Rendering turns a frozen document into output bytes in ordinary pipeline
//! stages. The terms below are used throughout this module and its
//! submodules:
//!
//! - A **backend** is the registration bundle for one output format: the
//!   elements its emitter writes natively, a lowering stage, an emit stage and
//!   a final stage ([`BackendDef`](crate::core::registry::BackendDef)).
//! - **Lowering** rewrites each frozen tree into a presentation tree for one
//!   backend, by applying the lowering rule selected for each element, in
//!   rounds, until only elements the backend writes natively remain; the
//!   result is computed data in the table `flm.lowered.<backend>` (see
//!   [backends, lowering, and
//!   outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! - A **lowering rule** is a named registry entity written for one element:
//!   it declares what it requires, reads and produces, and implements `Lower`,
//!   which stages (builds into the lowered tree) the element's presentation
//!   for one backend ([`LoweringRule`](crate::core::lowering::LoweringRule)).
//! - A **presentation node** is a node of a **presentation element**, an
//!   element that never occurs in a source document: lowering rules stage
//!   such nodes, and every backend's emitter reads them. The presentation
//!   elements every backend shares form the presentation vocabulary
//!   ([`pres`]).
//! - A **decoration** is a node whose element carries the
//!   [`Decoration`](pres::Decoration) aspect (a run-in heading, a margin note,
//!   a closing mark): the lowering driver moves it out of the content of the
//!   presentation node that contains it into an attached slot of that node
//!   (see [decorations, roles and
//!   kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)).
//! - A **piece** is a document-level synthesized value that a backend places
//!   before or after the document's content (the title, the list of
//!   footnotes, the bibliography) ([`Piece`](crate::core::piece::Piece)).
//! - A **node region** is one of a node's arguments or one of its slots
//!   ([`NodeRegion`](crate::core::lowering::NodeRegion)); a lowering rule can
//!   lower a node region of another node where its own node is placed. An
//!   **output region** is a different thing: one named part of an output's
//!   entry artifact, such as the document's content or one piece
//!   ([`Regions`](crate::core::region::Regions)).
//!
//! After lowering, a backend's **emitter** (its emit stage) walks the lowered
//! trees and writes the output's bytes, and its final stage (see [output
//! regions and the final
//! stage](crate::guide::concepts_overview#output-regions-and-the-final-stage))
//! makes the artifacts from them.
//!
//! # Module map
//!
//! - [`pres`] — the presentation vocabulary: the presentation elements
//!   (`flm.pres.*`), their typed payloads, and the functions an emitter reads
//!   a presentation node with.
//! - [`lowering`] — the lowering stage every backend registers, the driver
//!   that runs the rounds over one tree and implements the
//!   [`Lowerer`](crate::core::lowering::Lowerer) every rule works through, the
//!   default rules of the structural elements, and the `lowering` feature.
//! - [`pieces`] — the `pieces` feature and the pieces stage every backend that
//!   places pieces registers.
//! - [`text`] — the `text` backend: its native element set, its emitter over
//!   the [layout engine](crate::layout), its emit and final stages, and the
//!   `text` output and workflow.
//! - [`latex`] — the LaTeX family: what every LaTeX backend shares — the
//!   construct elements, escaping, the typed sink, the package requests and
//!   definition pieces, and the style file.
//! - [`latex_recomposed`] — the `latex-recomposed` backend: the document
//!   written back as LaTeX from the trees without block structure, with a
//!   style file holding the definitions and packages the recomposed LaTeX
//!   needs.
//! - [`latex_rendered`] — the `latex-rendered` backend: the document written
//!   as fully resolved LaTeX for LuaLaTeX, one command or environment per
//!   presentation element, with a style file defining them.
//! - [`html`] — the `html` backend: the document written as an HTML page for
//!   a browser.
//!
//! See also [the rendering guide](crate::guide::rendering).
//!
//! # Dependencies between backends and features
//!
//! This module is the only one in the crate that uses both
//! [`crate::core::data`] and [`crate::layout`]; computed data and the layout
//! engine do not depend on each other. No backend depends on a content
//! feature (a feature that declares elements an author writes, such as
//! headings or lists). A content feature may depend on a backend: it may
//! write a lowering rule for that backend, mark that rule as the default on
//! that backend ([`DefaultMark`](crate::core::lowering::DefaultMark)) after
//! declaring the backend as a peer ([`FeatureBuilder::peer_backend`]), and
//! read that backend's render options. Each standard feature's rule for the
//! `latex-recomposed` backend does this.
//!
//! [`FeatureBuilder::peer_backend`]: crate::core::registry::FeatureBuilder::peer_backend

pub mod html;
pub mod latex;
pub mod latex_recomposed;
pub mod latex_rendered;
pub mod lowering;
pub mod pieces;
pub mod pres;
pub mod text;
