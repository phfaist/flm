//! The standard features of the [`stdcore`](crate::stdcore) package, one feature per
//! submodule; the backend features of the package are in
//! [`stdcore::backends`](crate::stdcore::backends).
//!
//! Each submodule documents one feature ([`FlmFeature`](crate::core::registry::FlmFeature)):
//! the commands and environments it provides to authors and what they produce, and the
//! elements, tables, stages, lowering rules and settings it registers.
//!
//! [`stdcore::package`](crate::stdcore::package) registers all of them but
//! [`compose`], which a host registers (see [below](self#a-feature-a-host-registers)). A program may
//! register a feature without the rest of the package: a feature declares what it uses
//! of a feature it can do without as a peer declaration
//! ([`PeerName`](crate::core::registry::PeerName)), so the registry still builds without
//! that feature. Some features need others, however (for example, the stages of `refs`
//! read the tables of `refpoints` and `index`), and a plan that runs such a feature's
//! stages fails when a feature it needs is absent
//! ([`PlanError::NeedsUndeclared`](crate::core::plan::PlanError::NeedsUndeclared)). See
//! [features and packages](crate::guide::concepts_overview#features-and-packages), and
//! [the extension guide](crate::guide::extending) for writing a feature.
//!
//! # The features an author writes with
//!
//! These features declare the commands and environments an author writes:
//!
//! - [`textstyles`]: the inline formatting commands `\emph`, `\textbf`, `\textit` and
//!   `\texttt`.
//! - [`literals`]: the commands that stand for one character of text (`\%`, `\#`,
//!   `\&`, `\$`, `\ `, `\{`, `\}`, `\textbackslash`), the character `~` (a non-breaking
//!   space), and `\@`, a spacing hint that produces nothing.
//! - [`headings`]: the heading commands `\section` to `\subparagraph`, and the heading
//!   kinds and heading commands a program declares.
//! - [`math`]: inline and display mathematics: the display-math environments and their
//!   equation lines, the text commands of mathematics, `\eqref`, and equation numbering.
//!   No feature of this package processes mathematics: a registry adds one, such as the
//!   feature of the package [`math_passthrough`](crate::math_passthrough).
//! - [`refs`]: labels and references: the `\label` field (a macro a construct reads as
//!   data; see [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)),
//!   `\ref`, `\hyperref`, and the
//!   stages that compute anchors and resolve references.
//! - [`verbatim`]: content taken exactly as written: `\verbtext`, `\verbcode`,
//!   `\verba`, and the environments `verbatimtext` and `verbatimcode`.
//! - [`lists`]: the list environments `itemize`, `enumerate` and `description`, their
//!   `\item`s, and the label beside each item.
//! - [`xnotes`]: footnotes and the other kinds of text set apart from the running
//!   text (xnotes), `\footnote{…}` by default, and the lists in which they are shown.
//! - [`href`]: hyperlinks to an address outside the document: `\href`, `\url` and
//!   `\email`.
//! - [`defterm`]: defined terms: the `defterm` environment, which defines a term where
//!   it is explained, and `\term`, a link to that definition.
//! - [`theorems`]: the theorem-like environments `theorem`, `lemma`, `definition`,
//!   `proof` and the theorem kinds a program configures, numbered, with a heading and
//!   a proof's back-reference.
//! - [`quote`]: quotations: `quote`, `blockquote`, `address` and the quote
//!   environments a program configures, with their quotation sections (`\text`,
//!   `\lines`, `\attributed`, `\block`).
//! - [`annotations`]: a reviewer's highlights and comments, one command per reviewer
//!   a program configures; none by default.
//! - [`graphics`]: `\includegraphics[options]{path}`, an image, in a float's body or,
//!   when the configuration allows it, anywhere in the text.
//! - [`floats`]: `figure`, `table` and the float types a program configures: a block
//!   of content shown where it is written, with a caption and labels, numbered under
//!   its type's policy.
//! - [`cells`]: tables of cells, the `cells` environment filled with `\cell` and
//!   `\celldata`, in the body of a `table` float or, when the configuration allows it,
//!   anywhere a block can be written.
//! - [`citations`]: `\cite[note]{keys}`, each cited work numbered by first appearance
//!   and shown as a citation mark, and the list of cited works.
//! - [`input`]: `\input{file}`, the content of another file, brought in where the
//!   command is written.
//! - [`userdefs`]: the macros, environments and special character sequences an author
//!   or a program defines by substitution (user definitions), and the stages that
//!   expand them.
//! - [`meta`]: the document's metadata (title, authors and date), which a program sets
//!   on the document and the feature shows before the content.
//! - [`toc`]: the table of contents, the table of the listed headings and the piece
//!   that shows them as nested lists before the content.
//!
//! # The features other features build on
//!
//! These features declare no command. They contribute rewrites, tables and stages
//! that other features and the backends read:
//!
//! - [`blocks`]: the block-structure rewrite stage, which lays content out in
//!   paragraphs and blocks, and the check of the tree contract.
//! - [`index`]: the element index, the table of the nodes whose elements carry each
//!   aspect, which the stages that look for such nodes read.
//! - [`refpoints`]: the table of every reference point of a run, as the elements
//!   declared them, which the [`refs`] and [`numbering`] features read.
//! - [`numbering`]: counters, counter styles, and the number of every numbered
//!   reference point: the [service slots](crate::core::services) in which features
//!   declare their counters, counter styles and parent sets, the numbering options
//!   ([`NumberingOptions`](numbering::NumberingOptions), set on a document or on a
//!   render request) that say what a counter is numbered within, the stage that counts,
//!   and the counter table `flm.counters`.
//!
//! # A feature a host registers
//!
//! This feature is not registered by [`stdcore::package`](crate::stdcore::package); a host
//! that wants it registers it in a package of its own:
//!
//! - [`compose`]: the service slots of the template engines and of the compose templates,
//!   the template engine `content-only`, and one stage per output made of output regions,
//!   which assembles the output's entry artifact through the chosen compose template. It
//!   provides the same capability as each backend's final stage, so a registry holding it
//!   needs a plan request that prefers one of the two.

pub mod annotations;
pub mod blocks;
pub mod cells;
pub mod citations;
pub mod compose;
pub mod defterm;
pub mod floats;
pub mod graphics;
pub mod headings;
pub mod href;
pub mod index;
pub mod input;
pub mod lists;
pub mod literals;
pub mod math;
pub mod meta;
pub mod numbering;
pub mod quote;
pub mod refpoints;
pub mod refs;
pub mod textstyles;
pub mod theorems;
pub mod toc;
pub mod userdefs;
pub mod verbatim;
pub mod xnotes;
