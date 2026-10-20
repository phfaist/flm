//! The rewrite passes: the functions that turn one document tree into a new
//! tree before the trees are frozen.
//!
//! A **rewrite pass** is a plain function that takes one node tree of the FLM
//! language and returns a new one. It reads the tree, the
//! [element](crate::model::element::ElementSpec) each node carries, and the
//! options passed to the function, and nothing else. It can therefore be called on
//! any tree (a fragment's tree, a snippet's tree, a tree an earlier pass
//! produced) with no pipeline around it.
//!
//! Rewriting is the only operation that changes a tree. Once the last rewrite
//! of a plan has run, the trees are frozen, and everything computed afterwards
//! is stored in tables beside them (see [frozen trees and computed
//! data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
//!
//! The three passes, in the order in which a plan that uses all three runs them:
//!
//! - [`input`]: [`splice`](input::splice) replaces every `\input` command by
//!   the content of the file it includes, when the parse read that file, in the
//!   run of sibling nodes that holds the command.
//! - [`userdefs`]: [`expand`](userdefs::expand) replaces every use of a
//!   [user definition](userdefs::Definition) (a macro, environment or special
//!   character sequence defined by substitution) by the definition's template,
//!   filled in with the arguments written where it is used.
//! - [`blocks`]: [`structure`](blocks::structure) divides the content into
//!   paragraphs and blocks and resolves the text once (whitespace collapsed,
//!   adjacent text merged, typographic substitutions applied). The module also
//!   holds the checker of the [tree contract](blocks::check), the rules a tree
//!   satisfies after parsing and, in addition, after block structuring.
//!
//! A feature places each pass in a plan as a [rewrite
//! stage](crate::core::stage::RewriteStage): the `input` feature
//! ([`crate::stdcore::features::input`]), the `userdefs` feature
//! ([`crate::stdcore::features::userdefs`]) and the `blocks` feature
//! ([`crate::stdcore::features::blocks`]). The stage declares where the pass
//! runs in the plan; the pass does the work.

pub mod blocks;
pub mod input;
pub mod userdefs;
