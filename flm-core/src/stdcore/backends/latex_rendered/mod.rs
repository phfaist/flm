//! The `latex-rendered` backend: the document written as fully resolved LaTeX
//! for LuaLaTeX, through one command or environment per presentation element.
//!
//! FLM resolves everything it can itself (numbering, references, the table of
//! contents, notes) and the backend writes the result through a **contract**
//! of fine-grained commands, all named `\flmx…`, one per element of the
//! [presentation vocabulary](crate::stdcore::backends::pres): `flmxBlock`,
//! `\flmxSpan`, `\flmxHeading`, `\flmxLink`, and so on
//! ([`contract`]). Every fact the element carries is a key of the command's
//! key-value argument (`\flmxSpan[style=emph, role=…]{…}`), so that a template
//! decides the look of each element, role and kind; LuaLaTeX only typesets.
//! The backend is a product of its own, independent of the `latex-recomposed`
//! backend: they share the LaTeX family's library mechanisms
//! ([`crate::stdcore::backends::latex`]: the escaping, the typed sink, the
//! construct elements, the package and definition slots), nothing else.
//!
//! # What the backend produces
//!
//! An artifact set ([`ArtifactSet`](crate::core::artifact::ArtifactSet)): the
//! document as the entry artifact `latex-rendered:document`, in the backend's
//! fixed skeleton (a complete `article` document), and a **style file**, the
//! `.sty` file the document loads, which defines every `\flmx…` command the
//! document uses ([`style`]), named `<stem>.<hash>.sty` through the program's
//! naming of files. A host's template assembles the backend's regions
//! (`content`, one per piece, `style-package`) instead of the fixed skeleton.
//!
//! # The modules of the backend
//!
//! - [`mod@feature`]: the [`LatexRendered`] feature, the native element set
//!   ([`native_elements`]), the render options
//!   ([`LatexRenderedRenderOptions`]) and the chunks of the contract
//!   ([`chunks`]).
//! - [`contract`]: the table of what is written for each element.
//! - [`mod@emit`]: the emitter ([`emit_tree`]) and the conditions
//!   `flm.latex-rendered.too-deep` and `flm.latex-rendered.worker-thread`;
//!   mathematics is written by the emitter's code in `math.rs`.
//! - [`length`] and [`verbatim`]: lengths and verbatim text spelled for LaTeX.
//! - [`style`]: the style file.
//! - [`mod@stages`]: the emit, pieces, regions and final stages, their tables,
//!   the resolution of requirements ([`resolve`]) and the conditions
//!   `flm.latex-rendered.stage-failed`, `unknown-requirement`,
//!   `conflicting-package-options`, `style-path`, `missing-subtree`,
//!   `subtree-cycle` and `piece-placement`.

pub mod contract;
pub mod emit;
pub mod feature;
pub mod length;
mod math;
pub mod stages;
pub mod style;
pub mod verbatim;

pub use emit::{DEPTH_LIMIT, Emitted, LINE_WIDTH, NestingTooDeep, NoWorkerThread, StagedDefinition, emit_tree};
pub use feature::{
    BACKEND, DEFAULT_STYLE_STEM, FEATURE_NAME, FLOW_THROUGH, FONTSPEC, LatexRendered,
    LatexRenderedRenderOptions, OUTPUT, WORKFLOW, chunks, feature, native_elements,
};
pub use stages::{
    ARTIFACT_OWNER, ARTIFACTS, ConflictingPackageOptions, DOCUMENT, DOCUMENT_ARTIFACT, EMIT_PIECES_STAGE,
    EMIT_STAGE, EXTENSION, FINAL_STAGE, LatexRenderedEmitPiecesStage, LatexRenderedEmitStage,
    LatexRenderedFinalStage, LatexRenderedRegionsStage, LatexSegment, MEDIA_TYPE, MissingSubtree,
    OUTPUT_TABLE, PIECES_TABLE, PiecePlacement, REGIONS, REGIONS_STAGE, RenderedFragment,
    RenderedFragments, RenderedPiece, RenderedPieces, Resolved, SKELETON_HEADER, STYLE_EXTENSION,
    STYLE_FILE, StageFailed, StylePathRefused, SubtreeCycle, UnknownRequirement, resolve, stages,
    style_artifact,
};
