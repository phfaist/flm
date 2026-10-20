//! Assembling the content of a document from its primary file and the files
//! it includes with `\input`.
//!
//! A document may be made of several files: the **primary file**, the one a
//! host is asked to process, and the files it includes with the command
//! `\input`. An included file is a content input or a heading input ([content
//! inputs and heading inputs](crate::lang::input#content-inputs-and-heading-inputs)).
//! The library reads no file itself: the
//! program that embeds it supplies an input resolver
//! ([`InputResolver`](crate::lang::InputResolver)), which
//! finds and reads the file that an `\input` refers to.
//!
//! [`assemble`] parses the primary file into a document's content
//! ([`DocContent`]), bringing in the included files as it goes:
//!
//! - The content of an included file (a **content input**) is parsed at the
//!   place of its `\input` command, inside the tree of the file that includes
//!   it, and the rewrite stage of the `input` feature
//!   ([`input`](crate::stdcore::features::input)) later inserts it in place of
//!   the command, so every later stage sees one continuous text.
//! - An included file whose front matter (the block of settings at the head of
//!   the file, read by the input resolver) declares that it opens with a
//!   heading (a **heading input**,
//!   [`IncludedFile::heading`](crate::lang::IncludedFile::heading)) becomes a
//!   tree of its own instead. It is parsed under the parsing state that applies
//!   at its `\input` command, with the heading its front matter declares at
//!   the start, and the tree is placed at the command's node
//!   ([`DocContent::resolve`]), so that the walk visits it there.
//!
//! A heading input may include files in turn, of either kind. Later stages see
//! trees and the walk only; whether a tree came from an included file plays no
//! part in them.

use std::sync::Arc;

use techy::core::node::NodeId;
use techy::core::Language;
use techy::error::Diagnostic;

use crate::lang::input::{heading_source, is_placing, InputPlacementLost, PlacedRoot, Placements};
use crate::lang::parse::{language_for_assembly, parse_with};
use crate::lang::{Flm, FlmSource, FragmentError, ParseOptions};
use crate::lang::BuiltLang;
use crate::model::addr::NodeAddr;
use crate::model::doc::{DocContent, Fragment, FragmentLayout};

/// Parses `source`, the primary file of a document, with the language `lang`,
/// and returns the document's content: the tree of the primary file, with each
/// content input parsed at its `\input` command, and one more tree for each
/// heading input, placed at its `\input` command (see the [module
/// documentation](self)).
///
/// - `options.input_resolver` is the input resolver for this call; when it is
///   `None`, the language's own input resolver is used. Without either, each
///   `\input` is reported and brings nothing in.
/// - `layout` is the layout of the top level of the primary file's tree; the
///   tree of a heading input always holds blocks
///   ([`FragmentLayout::Blocks`]).
///
/// Every problem found while parsing is a diagnostic of the fragment of the
/// tree it was found in ([`Fragment::diags`](crate::model::doc::Fragment::diags)).
/// A heading input whose file yields no tree, or whose command was not kept by
/// the parse ([`InputPlacementLost`]), is reported in the tree of the file that
/// includes it, and adds no tree.
///
/// Each file is parsed on a worker thread (a thread with a large enough stack;
/// see [`on_worker_thread`](crate::lang::guard::on_worker_thread)), created
/// when the calling thread is not one.
///
/// Use [`Doc::new`](crate::model::doc::Doc::new) or
/// [`Doc::with_options`](crate::model::doc::Doc::with_options) to make a
/// document of the result.
///
/// # Errors
///
/// Returns the [`FragmentError`] of the parse when the primary file yields no
/// tree, for the same reasons as
/// [`parse_fragment`](crate::lang::parse_fragment): the content nests too
/// deeply, the parse was aborted, or no worker thread could be created.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::lang::{
///     build_language, FlmOrigin, FlmParsing, FlmSource, IncludedFile, InputError,
///     InputResolver, LangInputs, ParseOptions,
/// };
/// use flm_core::model::assemble::assemble;
/// use flm_core::model::doc::FragmentLayout;
///
/// // A host that knows one file, held in memory.
/// struct OneFile;
/// impl InputResolver for OneFile {
///     fn resolve(&self, reference: &str, _including: Option<&str>, _parsing: &FlmParsing) -> Result<IncludedFile, InputError> {
///         match reference {
///             "part.flm" => Ok(IncludedFile {
///                 name: String::from("part.flm"),
///                 text: String::from("world"),
///                 first_line: 1,
///                 ..IncludedFile::default()
///             }),
///             _ => Err(InputError::new(format!("there is no file `{reference}`"))),
///         }
///     }
/// }
///
/// // The source holds no `\input`, so the document is the primary file's tree
/// // alone. (The bare language below would not recognize `\input` anyway: the
/// // `input` feature of the standard package declares it.)
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("Hello").with_origin(FlmOrigin::new("main.flm")));
/// let options = ParseOptions { input_resolver: Some(Arc::new(OneFile)), ..ParseOptions::default() };
/// let content = assemble(&lang, source, FragmentLayout::default(), &options)
///     .expect("the input is shallow enough to parse");
/// assert_eq!(content.len(), 1);
/// ```
pub fn assemble(
    lang: &BuiltLang,
    source: Arc<FlmSource>,
    layout: FragmentLayout,
    options: &ParseOptions,
) -> Result<DocContent, FragmentError> {
    let placements = Arc::new(Placements::default());
    let language = language_for_assembly(lang, options, Arc::clone(&placements));
    let primary = parse_with(&language, source, layout, Arc::clone(lang.initial_state()), None)?;
    let assembled = assemble_tree(&language, &placements, primary, 0);
    let mut content = DocContent::new();
    insert(&mut content, assembled, true);
    Ok(content)
}

/// A parsed tree and the trees of the heading inputs placed in it, each with
/// the node that places it.
struct Assembled {
    fragment: Fragment,
    placed: Vec<(NodeId, Assembled)>,
}

/// Parses, as trees of their own, the heading inputs found by the parse of
/// `fragment` (the records of `placements` from index `from` on), and in turn
/// the heading inputs found by those parses.
fn assemble_tree(
    language: &Language<Flm>,
    placements: &Placements,
    fragment: Fragment,
    from: usize,
) -> Assembled {
    let to = placements.len();
    let mut diags = fragment.diags().clone();
    let mut placed = Vec::new();
    for record in placements.range(from, to) {
        // The node the record came from: an `\input` placing a tree, at the
        // command's span. The parser stages it right after the record, so
        // every record of this parse has one; were it missing, the file would
        // have no place, which is reported at the command.
        let site = fragment
            .tree()
            .root()
            .descendants()
            .find(|node| is_placing(node) && *node.span() == record.site)
            .map(|node| node.id());
        let Some(site) = site else {
            diags.push(Diagnostic::error(InputPlacementLost { file: record.name.clone() }, record.site.clone()));
            continue;
        };
        let start = placements.len();
        let mut root = PlacedRoot { heading: heading_source(&record) };
        match parse_with(language, record.source, FragmentLayout::Blocks, record.state, Some(&mut root)) {
            Ok(tree) => placed.push((site, assemble_tree(language, placements, tree, start))),
            Err(error) => diags.push(error.diagnostic()),
        }
    }
    let fragment = Fragment::new(
        Arc::clone(fragment.source()),
        Arc::clone(fragment.tree()),
        diags,
        fragment.layout(),
    );
    Assembled { fragment, placed }
}

/// Adds the tree of `assembled` to `content` (appended to the traversal list
/// when `listed`, otherwise reached through the node that places it), then
/// each tree placed in it, placed at its node; returns the tree's identifier.
fn insert(content: &mut DocContent, assembled: Assembled, listed: bool) -> crate::model::addr::TreeId {
    let Assembled { fragment, placed } = assembled;
    let fragment = Arc::new(fragment);
    let id = if listed { content.place(fragment) } else { content.add_tree(fragment) };
    for (site, child) in placed {
        let target = insert(content, child, false);
        // Both trees are new to `content`, each site is one node and each
        // file is placed once (the parse refuses a second placing), so the
        // placement cannot be refused.
        let _ = content.resolve(NodeAddr::new(id, site), target);
    }
    id
}
