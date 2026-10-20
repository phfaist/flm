//! The tree-contract checker: the rules every tree satisfies after parsing,
//! and the further rules a structured tree satisfies after the
//! [block-structure pass](super::pass).
//!
//! The **tree contract** is that set of rules. The checker is two plain
//! functions over a tree, which return every violation they find as a typed
//! [`ContractViolation`], in the tree's storage order. They never panic and
//! never return a message string to match: [`ContractViolation::diagnostic`]
//! turns a violation into the Error diagnostic [`ContractViolated`]
//! (`flm.blocks.contract-violated`). The two parts are:
//!
//! - [`check_parse_contract`]: what the parser establishes and every rewrite
//!   keeps;
//! - [`check_block_contract`]: what the block-structure pass adds, assuming the
//!   parse part holds.
//!
//! A violation is a fault in the program (the parser, a rewrite pass, or an
//! element's aspect), not in the document. The check stage
//! `flm.blocks.check-contract` of the `blocks` feature
//! ([`crate::stdcore::features::blocks::check`]) runs the parse part over every
//! frozen tree, and the block-structure part over every tree the plan
//! structured, and records the violations; a test can call the functions
//! directly.
//!
//! Both functions begin with techy's [`validate_tree`], the rules every node
//! tree obeys: about the order of nodes, the child nodes each argument and
//! each node slot covers, and spans. (A node slot, techy's `ParsedSlot`, is a
//! part of a node's content outside its arguments, such as an environment's
//! body.) Both stop there when it fails, because every further check reads
//! the tree through accessors that assume those rules. Both take time linear in the
//! tree's size. The block-structure part first computes facts about each node in
//! one walk in storage order (where a parent comes before its children), each
//! from its parent's facts, and its rules read those facts.
//!
//! # The parse part
//!
//! - **Every node has the right element.** Every node carries the
//!   [element](crate::model::element::ElementSpec) that
//!   [`resolve_element`] gives for its node kind (text, comment, group of each
//!   group class, macro, environment, ...) under the [structural elements](crate::lang::structural::StructuralElements)
//!   that its parsing state carries. The test compares element values, never
//!   names; two different elements may even have the same name.
//! - **Every callable's specification was built by FLM.** A callable's
//!   specification is one of the wrapper types that FLM builds from the
//!   features' element declarations ([`crate::lang::mint`]), techy's paragraph-break specification, or a
//!   template placeholder. The two forms the parser gives a command no feature
//!   defines are checked: `flm.unknown.macro` occurs only on a node written as
//!   a macro, and `flm.unknown.environment` only on a node written as an
//!   environment whose specification FLM built.
//! - **A macro read by a fallback has nothing inside.** A macro that no
//!   declaration defines is read by a fallback: a feature's (the mathematics
//!   feature's, in mathematics mode) or the language's own. Such a node has no
//!   arguments and no children.
//! - **The text of a verbatim group was read with the markup rules off.** The
//!   text children of a verbatim group, read from the group's own source, carry
//!   a parsing state in which commands, comments, special character sequences
//!   and paragraph breaks are disabled, which shows independently that the
//!   parser read them as verbatim content (see [verbatim
//!   regions](crate::guide::concepts_overview#verbatim-regions)). Only the
//!   children from the group's own source are checked, because content that an
//!   expansion put into a template's verbatim content comes from another source
//!   and carries the state it was parsed with. Where text read with the rules
//!   off ends up is not checked: an expansion places a verbatim argument's
//!   characters wherever its template uses the argument. An empty verbatim group
//!   satisfies the rule.
//!
//! # The block-structure part
//!
//! - **Expansion is complete.** No template placeholder remains
//!   (`PlaceholderSurvived`), and no node of an element carrying the [`Expand`]
//!   aspect (a use of a user definition) remains (`UseSiteSurvived`). The two
//!   are separate kinds because a plan may be asked to keep the uses of the
//!   definitions expanded only when the plan asks for it
//!   ([`ExpandPolicy::PerPlan`](crate::lang::rewrites::userdefs::ExpandPolicy::PerPlan);
//!   the `flm.userdefs.kept` alternative). The
//!   checker, a plain function of the tree, reports them anyway; the check
//!   stage applies this rule only where the plan expanded the definitions.
//! - **Paragraphs and content regions.** No paragraph-break marker remains
//!   outside an opaque region. A paragraph has empty delimiters and holds
//!   content (something other than comments and whitespace). No block-level
//!   node is inside a paragraph. A [content region](super) (a run of sibling
//!   nodes laid out together) laid out as blocks holds nothing inline, only paragraphs, block-level nodes
//!   and comments. A content region is laid out as blocks when it holds a
//!   paragraph; when it is a body that its declaration lays out as blocks
//!   (declared so, or following the block role `Block` of its element under
//!   the default [`FromRole`](crate::lang::mint::BodyLayout::FromRole)), even
//!   when it holds no paragraph; when it is an argument declared
//!   [`Blocks`](crate::lang::mint::ArgLayout::Blocks); and when it is an
//!   argument declared
//!   [`InlineOrBlocks`](crate::lang::mint::ArgLayout::InlineOrBlocks) that holds
//!   a block-level node, a paragraph included. No paragraph is inside content
//!   laid out inline: not in a `{…}` group, not in an argument declared inline
//!   (the default), and not in a body that its declaration lays out inline.
//! - **Decorators are placed.** A
//!   [`Leads`](crate::model::aspects::Placement::Leads) decorator is preceded in
//!   its paragraph only by comments, a
//!   [`Trails`](crate::model::aspects::Placement::Trails) decorator is followed
//!   only by comments.
//! - **Text is resolved.** Text outside opaque regions holds no tab, carriage
//!   return or line feed and no two consecutive spaces; the text at the edges of
//!   a paragraph (looking through comments and edge decorators) neither starts
//!   nor ends with whitespace; and no two text nodes are adjacent siblings. The
//!   typographic substitutions are not checked, because the options that decide
//!   them are not recorded in the tree.
//! - **Source maps are consistent.** A source map is attached to a text node
//!   only, and its pieces cover exactly that node's text.
//!
//! # Opaque regions
//!
//! The pass copies an [opaque
//! region](crate::guide::concepts_overview#opaque-regions) exactly as it was
//! parsed or created: everything under a verbatim group, and every node parsed
//! in math mode (the content of a mathematics group, a math-mode environment
//! body, a mathematics snippet). The rules about content (the remaining
//! paragraph-break marker, the text rules) do not apply inside them; what a
//! blank line inside mathematics means is checked by the mathematics feature.
//!
//! An argument or a body that a construct of mathematics reads in **text** mode
//! (the argument of a text command such as `\text{…}`) is not opaque: it is
//! document content, the pass lays it out, and every rule above applies inside
//! it. A paragraph there is reported as a paragraph inside content laid out
//! inline, and its text is checked like any other text. A verbatim region is
//! opaque in every parsing mode, because reading verbatim content changes no
//! mode.
//!
//! # What the checker does not check
//!
//! - Anything the tree alone cannot show: that a block-level node in inline
//!   content came with a diagnostic, that the typographic substitutions
//!   followed the options, or the layout of the root container or of a
//!   container a block-structure hook built (a fragment's layout is not
//!   recorded in the tree). The checker does not guess: such a container is
//!   checked as content laid out as blocks only when it holds a paragraph.
//! - **The source text around an argument or a body** that is part of how the
//!   command was written rather than of its content: the whitespace before an
//!   argument, the whitespace between two arguments, the line break that techy
//!   skips after `\begin{verbatim}`. The pass copies that text unchanged and no
//!   backend writes it, so the text rules apply only to the content of
//!   arguments and bodies.
//!
//! Two rules bind an element's [block-structure hook](super::hook::BlockStructure)
//! as much as the pass:
//!
//! - A body that its declaration lays out as blocks is checked as such even
//!   when a hook laid it out. A hook must leave such a body holding only
//!   paragraphs, block-level nodes and comments, so a container it builds to
//!   hold blocks must itself be a node of an element whose block role is
//!   `Block`.
//! - An argument is checked against its declared layout in the same way: a
//!   hook that lays out an argument declared inline as blocks is reported.

use std::any::Any;
use std::fmt;

use techy::core::node::{
    CallableData, ChildRegion, NodeId, NodeRef, NodeTree, TreeViolation, validate_tree,
};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::latexlike::{Mode, ParagraphBreakSpec};

use crate::model::aspects::{BlockRole, Placement, block_role_of};
use crate::model::element::{ElementSpec, element_of};
use crate::model::names::ElementName;
use crate::lang::ext::RestrictedInterior;
use crate::lang::flm::{Flm, FlmCallableType, FlmGroupType};
use crate::lang::mint::specs::element_of_spec;
use crate::lang::mint::{FALLBACK_PACKAGE_NAME, body_def_of, resolve_element};
use crate::lang::origin::FlmDiagnostic;
use crate::lang::rewrites::userdefs::Expand;

use super::predicates::{Region, argument_holds_blocks, is_content, slot_layout};
use super::typography::is_ascii_space;

/// The rule of the [tree contract](self) that a [`ContractViolation`] breaks,
/// with the facts that describe the violation.
///
/// [`as_str`](ContractViolationKind::as_str) returns the variant's name, which
/// does not change when the message text changes; the [`Display`](fmt::Display)
/// text describes the violation for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractViolationKind {
    /// techy's [`validate_tree`] failed: the tree breaks a rule every node tree
    /// obeys. Nothing else was checked.
    Tree(TreeViolation),
    /// The node's element is not the element value that
    /// [`resolve_element`] gives for its node kind (text, comment, group of
    /// each group class, macro, environment, ...) under the structural
    /// elements of its parsing state. Element values are compared, never
    /// names, so the two names may be the same.
    ElementMismatch {
        /// The name of the element that `resolve_element` gives.
        expected: ElementName,
        /// The name of the element the node carries.
        found: ElementName,
    },
    /// A callable's specification is none of the wrapper types that FLM builds
    /// from the features' element declarations ([`crate::lang::mint`]).
    SpecNotMinted {
        /// The callable's name as written.
        name: String,
    },
    /// A node carries `flm.unknown.macro` but is not written as a macro, or
    /// carries `flm.unknown.environment` but is not written as an environment
    /// with a specification FLM built. These are the two forms the parser
    /// gives a command that no feature defines.
    UnknownCallableShape {
        /// The element the node carries.
        element: ElementName,
    },
    /// A macro read by a fallback (a macro no declaration defines) has
    /// arguments or children.
    FallbackShape {
        /// The callable's name as written.
        name: String,
    },
    /// A text child of a verbatim group, from the group's own source, was not
    /// read with the markup rules off: commands, comments, special character
    /// sequences and paragraph breaks disabled, except the rules that the
    /// parse of a template keeps on for its placeholders.
    VerbatimInteriorRulesOn,
    /// A template placeholder (a placeholder callable, or a node of the
    /// placeholder element) remains after expansion.
    PlaceholderSurvived,
    /// A node of an element carrying the [`Expand`] aspect (a use of a user
    /// definition) remains after expansion.
    UseSiteSurvived {
        /// The name of the remaining node's element.
        element: ElementName,
    },
    /// A paragraph-break marker remains outside an opaque region after the
    /// block-structure pass.
    ParbreakSurvived,
    /// A paragraph has delimiters; a paragraph's delimiters must be empty.
    ParagraphDelimiters {
        /// The opening delimiter.
        open: String,
        /// The closing delimiter.
        close: String,
    },
    /// A paragraph holds nothing but comments and whitespace.
    EmptyParagraph,
    /// A node of block role `Block` (a paragraph included) is a direct child of
    /// a paragraph.
    BlockInParagraph {
        /// The name of the node's element.
        element: ElementName,
    },
    /// A [content region](super) laid out as blocks (one holding a paragraph, or a body
    /// or an argument that its declaration lays out as blocks) also holds
    /// something inline: a text node, a node of inline or decorator role, a
    /// `{…}` group.
    InlineInBlocksRegion {
        /// The name of the node's element.
        element: ElementName,
    },
    /// A paragraph is inside content laid out inline: a `{…}` group, the
    /// content of an argument laid out inline, or the content of a body that
    /// its declaration lays out inline.
    ParagraphInInlineContainer {
        /// The name of the container's element: the group's, or that of the
        /// callable whose argument or body holds the paragraph.
        element: ElementName,
    },
    /// A [`Leads`](crate::model::aspects::Placement::Leads) decorator is
    /// preceded in its paragraph by something other than a comment.
    LeadsNotFirst {
        /// The name of the decorator's element.
        element: ElementName,
    },
    /// A [`Trails`](crate::model::aspects::Placement::Trails) decorator is
    /// followed in its paragraph by something other than a comment.
    TrailsNotLast {
        /// The name of the decorator's element.
        element: ElementName,
    },
    /// Resolved text still holds a tab, a carriage return, a line feed, or two
    /// consecutive spaces.
    WhitespaceNotCollapsed,
    /// The text at the start of a paragraph starts with whitespace, or the
    /// text at its end ends with whitespace.
    ParagraphEdgeNotTrimmed,
    /// Two text nodes outside an opaque region are adjacent siblings.
    AdjacentText,
    /// The pieces of a text node's source map do not cover its text exactly.
    SourceMapDoesNotTile {
        /// The number of bytes the pieces cover.
        mapped: u32,
        /// The text's length in bytes.
        text_len: usize,
    },
    /// A source map is attached to a node that is not a text node.
    SourceMapOnNonText,
}

impl ContractViolationKind {
    /// Returns the variant's name (`"ElementMismatch"`, …), which identifies
    /// the broken rule in the diagnostic and does not change when the message
    /// text changes.
    pub const fn as_str(&self) -> &'static str {
        match self {
            ContractViolationKind::Tree(_) => "Tree",
            ContractViolationKind::ElementMismatch { .. } => "ElementMismatch",
            ContractViolationKind::SpecNotMinted { .. } => "SpecNotMinted",
            ContractViolationKind::UnknownCallableShape { .. } => "UnknownCallableShape",
            ContractViolationKind::FallbackShape { .. } => "FallbackShape",
            ContractViolationKind::VerbatimInteriorRulesOn => "VerbatimInteriorRulesOn",
            ContractViolationKind::PlaceholderSurvived => "PlaceholderSurvived",
            ContractViolationKind::UseSiteSurvived { .. } => "UseSiteSurvived",
            ContractViolationKind::ParbreakSurvived => "ParbreakSurvived",
            ContractViolationKind::ParagraphDelimiters { .. } => "ParagraphDelimiters",
            ContractViolationKind::EmptyParagraph => "EmptyParagraph",
            ContractViolationKind::BlockInParagraph { .. } => "BlockInParagraph",
            ContractViolationKind::InlineInBlocksRegion { .. } => "InlineInBlocksRegion",
            ContractViolationKind::ParagraphInInlineContainer { .. } => {
                "ParagraphInInlineContainer"
            }
            ContractViolationKind::LeadsNotFirst { .. } => "LeadsNotFirst",
            ContractViolationKind::TrailsNotLast { .. } => "TrailsNotLast",
            ContractViolationKind::WhitespaceNotCollapsed => "WhitespaceNotCollapsed",
            ContractViolationKind::ParagraphEdgeNotTrimmed => "ParagraphEdgeNotTrimmed",
            ContractViolationKind::AdjacentText => "AdjacentText",
            ContractViolationKind::SourceMapDoesNotTile { .. } => "SourceMapDoesNotTile",
            ContractViolationKind::SourceMapOnNonText => "SourceMapOnNonText",
        }
    }
}

impl fmt::Display for ContractViolationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractViolationKind::Tree(violation) => write!(f, "{violation}"),
            ContractViolationKind::ElementMismatch { expected, found } if expected == found => {
                write!(
                    f,
                    "the node carries an element named ‘{found}’ that is not the element its \
                     parser settings give to a node of this form (text, group, macro, environment, ...)"
                )
            }
            ContractViolationKind::ElementMismatch { expected, found } => {
                write!(
                    f,
                    "the node carries the element ‘{found}’, but the parser settings it was read with \
                     give the element ‘{expected}’ to a node of this form"
                )
            }
            ContractViolationKind::SpecNotMinted { name } => {
                write!(f, "the command ‘{name}’ has a specification that FLM did not build")
            }
            ContractViolationKind::UnknownCallableShape { element } => {
                write!(
                    f,
                    "a node carries ‘{element}’ but is not written as the command form \
                     that this element requires"
                )
            }
            ContractViolationKind::FallbackShape { name } => {
                write!(
                    f,
                    "the macro ‘{name}’, which no declaration defines, must have no arguments and no content"
                )
            }
            ContractViolationKind::VerbatimInteriorRulesOn => {
                write!(
                    f,
                    "the text of a verbatim group was not read with the markup rules switched off"
                )
            }
            ContractViolationKind::PlaceholderSurvived => {
                write!(f, "a template placeholder remains after expansion")
            }
            ContractViolationKind::UseSiteSurvived { element } => {
                write!(
                    f,
                    "a use of the user definition ‘{element}’ remains after expansion"
                )
            }
            ContractViolationKind::ParbreakSurvived => {
                write!(f, "a paragraph break remains outside mathematics and verbatim content")
            }
            ContractViolationKind::ParagraphDelimiters { open, close } => {
                write!(
                    f,
                    "a paragraph has the delimiters {open:?} and {close:?}, and must have none"
                )
            }
            ContractViolationKind::EmptyParagraph => write!(f, "a paragraph holds nothing but comments and whitespace"),
            ContractViolationKind::BlockInParagraph { element } => {
                write!(f, "the block-level node ‘{element}’ is inside a paragraph")
            }
            ContractViolationKind::InlineInBlocksRegion { element } => {
                write!(f, "the inline node ‘{element}’ is placed between blocks, outside any paragraph")
            }
            ContractViolationKind::ParagraphInInlineContainer { element } => {
                write!(
                    f,
                    "a paragraph is inside ‘{element}’, whose content is laid out inline"
                )
            }
            ContractViolationKind::LeadsNotFirst { element } => {
                write!(
                    f,
                    "the decorator ‘{element}’ must come first in its paragraph"
                )
            }
            ContractViolationKind::TrailsNotLast { element } => {
                write!(
                    f,
                    "the decorator ‘{element}’ must come last in its paragraph"
                )
            }
            ContractViolationKind::WhitespaceNotCollapsed => {
                write!(f, "a text node holds a tab, a line break or two consecutive spaces")
            }
            ContractViolationKind::ParagraphEdgeNotTrimmed => {
                write!(f, "a paragraph begins or ends with whitespace")
            }
            ContractViolationKind::AdjacentText => write!(f, "two text nodes follow each other without a node between them"),
            ContractViolationKind::SourceMapDoesNotTile { mapped, text_len } => {
                write!(
                    f,
                    "the source map of a text node covers {mapped} bytes, and its text has {text_len} bytes"
                )
            }
            ContractViolationKind::SourceMapOnNonText => {
                write!(f, "a source map is attached to a node that is not text")
            }
        }
    }
}

/// One violation of the [tree contract](self): the node it concerns and the
/// rule it breaks.
///
/// [`check_parse_contract`] and [`check_block_contract`] return these;
/// [`diagnostic`](ContractViolation::diagnostic) turns one into a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractViolation {
    /// The node the violation concerns, or `None` when no single node can be
    /// given (a violation of techy's [`validate_tree`] may concern none).
    pub node: Option<NodeId>,
    /// The rule broken, with the facts that describe the violation.
    pub kind: ContractViolationKind,
}

impl ContractViolation {
    fn at(node: &NodeRef<'_, Flm>, kind: ContractViolationKind) -> ContractViolation {
        ContractViolation {
            node: Some(node.id()),
            kind,
        }
    }

    /// Returns the violation as an Error diagnostic ([`ContractViolated`]) at
    /// the span of the node in `tree` (the root's span when the violation
    /// concerns no node), for a check stage to record.
    pub fn diagnostic(&self, tree: &NodeTree<Flm>) -> FlmDiagnostic {
        let span = self
            .node
            .and_then(|id| tree.get(id))
            .map_or_else(|| tree.root().span().clone(), |node| node.span().clone());
        let condition = ContractViolated::new(self.kind.as_str(), self.kind.to_string());
        Diagnostic::error(condition, span)
    }
}

impl fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node {
            Some(node) => write!(f, "node {node:?}: {}", self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}

/// Condition `flm.blocks.contract-violated`: a tree breaks a rule of the
/// [tree contract](self). This is a fault in the program that built the tree,
/// not in the document.
///
/// [`ContractViolation::diagnostic`] builds it; the check stage of the
/// `blocks` feature records one for each violation it finds.
///
/// Recovery: none; the tree is not changed, and the diagnostic is placed at
/// the node the violation concerns.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.blocks.contract-violated",
    message = "the document tree breaks the structural rule {rule}: {detail}"
)]
pub struct ContractViolated {
    /// The name of the broken rule ([`ContractViolationKind::as_str`]).
    pub rule: String,
    /// The violation described for a person: the [`Display`](fmt::Display)
    /// text of the [`ContractViolationKind`].
    pub detail: String,
}

// --- per-node facts ---------------------------------------------------------------------

/// The facts about each node that the rules read, indexed by storage index and
/// computed in one walk in storage order: a node's facts follow from its
/// parent's, which come first (techy's tree rules place children after their
/// parent).
struct Facts {
    /// The node is inside an opaque region: under a verbatim group, or parsed
    /// in math mode.
    opaque: Vec<bool>,
    /// The node holds the content of an argument of a callable (the node is
    /// the argument's group) or of one of the callable's node slots (the node
    /// is the slot's container; a node slot, techy's `ParsedSlot`, is a part of
    /// a node's content outside its arguments), so its children are that
    /// content and are checked through the callable.
    content_parent: Vec<bool>,
}

impl Facts {
    fn of(tree: &NodeTree<Flm>) -> Facts {
        let count = tree.node_count();
        let mut facts = Facts {
            opaque: vec![false; count],
            content_parent: vec![false; count],
        };
        // Some ancestor is a verbatim group, per node: what `opaque` is
        // derived from.
        let mut under_verbatim = vec![false; count];
        for node in tree.iter_storage_order() {
            let i = node.id().index();
            let math_mode = node.parsing_state().mode() == Mode::Math;
            if let Some(parent) = node.parent() {
                let p = parent.id().index();
                let verbatim = parent.group_type() == Some(FlmGroupType::Verbatim);
                under_verbatim[i] = under_verbatim[p] || verbatim;
            }
            // Verbatim content is opaque whatever its parsing mode, since a
            // verbatim region changes no mode as it is read. Everywhere else
            // the node's own mode decides: math mode is opaque, and text mode
            // is content — which is what a text command's argument inside
            // mathematics is.
            facts.opaque[i] = under_verbatim[i] || math_mode;
            if let Some(callable) = node.callable() {
                for region in regions(callable) {
                    let parent = region.content_parent().index();
                    if let Some(flag) = facts.content_parent.get_mut(parent) {
                        *flag = true;
                    }
                }
            }
        }
        facts
    }

    fn index(node: &NodeRef<'_, Flm>) -> usize {
        node.id().index()
    }

    fn opaque(&self, node: &NodeRef<'_, Flm>) -> bool {
        self.opaque[Facts::index(node)]
    }

    fn is_content_parent(&self, node: &NodeRef<'_, Flm>) -> bool {
        self.content_parent[Facts::index(node)]
    }
}

// --- the parse-level part ---------------------------------------------------------------

/// Checks `tree` against the parse part of the [tree contract](self), and
/// returns every violation: every node has the right element, every
/// callable's specification was built by FLM, a macro read by a fallback has
/// nothing inside, and the text of a verbatim group was read with the markup
/// rules off.
///
/// The violations come in the tree's storage order; the result is empty when
/// the tree satisfies the rules. When techy's [`validate_tree`] fails, that one
/// violation is the whole result and nothing else is checked. Every tree
/// satisfies these rules after parsing and after every rewrite.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::lang::rewrites::blocks::check_parse_contract;
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmSource, LangInputs, ParseOptions,
/// };
///
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("a {b} \\emph{c}\n\nd"));
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::default(), &ParseOptions::default(),
/// ).expect("the input is shallow enough to parse");
///
/// assert!(check_parse_contract(fragment.tree()).is_empty());
/// ```
///
/// See also [`check_block_contract`], for what a structured tree adds.
pub fn check_parse_contract(tree: &NodeTree<Flm>) -> Vec<ContractViolation> {
    let mut out = Vec::new();
    if let Err(violation) = validate_tree(tree) {
        out.push(ContractViolation {
            node: violation.node,
            kind: ContractViolationKind::Tree(violation),
        });
        return out;
    }
    for node in tree.iter_storage_order() {
        let ext = node.parsing_state().ext();
        let element = element_of(&node);

        // A spec that is none of the flm-minted types carries no element at all;
        // `SpecNotMinted` below is that node's report, so no comparison is made.
        if let Ok(expected) = resolve_element(node.kind(), ext) {
            if !ElementSpec::same_value(&expected, element) {
                let kind = ContractViolationKind::ElementMismatch {
                    expected: expected.name.clone(),
                    found: element.name.clone(),
                };
                out.push(ContractViolation::at(&node, kind));
            }
        }

        if let Some(spec) = node.spec() {
            let minted = node.callable_type() == Some(FlmCallableType::Placeholder)
                || element_of_spec(&**spec).is_some()
                || (&**spec as &dyn Any)
                    .downcast_ref::<ParagraphBreakSpec>()
                    .is_some();
            if !minted {
                let name = node.name().unwrap_or_default().to_string();
                out.push(ContractViolation::at(
                    &node,
                    ContractViolationKind::SpecNotMinted { name },
                ));
            }
            let from_fallback = spec
                .provenance()
                .and_then(|provenance| provenance.provider())
                .is_some_and(|provider| provider.name() == FALLBACK_PACKAGE_NAME);
            if from_fallback && node.callable_type() == Some(FlmCallableType::Macro) {
                let has_arguments = node
                    .arguments()
                    .is_some_and(|arguments| !arguments.is_empty());
                if has_arguments || node.child_count() > 0 {
                    let name = node.name().unwrap_or_default().to_string();
                    out.push(ContractViolation::at(
                        &node,
                        ContractViolationKind::FallbackShape { name },
                    ));
                }
            }
        }

        if ElementSpec::same_value(element, &ext.structural.unknown_macro)
            && node.callable_type() != Some(FlmCallableType::Macro)
        {
            out.push(ContractViolation::at(
                &node,
                ContractViolationKind::UnknownCallableShape {
                    element: element.name.clone(),
                },
            ));
        }
        if ElementSpec::same_value(element, &ext.structural.unknown_environment) {
            let recovery = node.callable_type() == Some(FlmCallableType::Environment)
                && body_def_of(&node).is_some();
            if !recovery {
                out.push(ContractViolation::at(
                    &node,
                    ContractViolationKind::UnknownCallableShape {
                        element: element.name.clone(),
                    },
                ));
            }
        }

        if node.group_type() == Some(FlmGroupType::Verbatim) {
            for child in node.children().iter() {
                let own = child.is_chars() && child.span().same_source(node.span());
                if own && !interior_rules_off(&child) {
                    out.push(ContractViolation::at(
                        &child,
                        ContractViolationKind::VerbatimInteriorRulesOn,
                    ));
                }
            }
        }
    }
    out
}

/// Whether `node`'s parsing state is the state of verbatim content as the
/// policy for restricted interiors builds it: comments and paragraph breaks
/// disabled, and commands and special character sequences disabled too, except
/// where the state carries the [`Template`](RestrictedInterior::Template)
/// policy of a template parse and that policy keeps them (commands when it
/// lists a command syntax, the placeholder escape; special character
/// sequences when it keeps them on). A verbatim group copied from a template
/// keeps the text its template was parsed with, so its own text carries that
/// policy.
fn interior_rules_off(node: &NodeRef<'_, Flm>) -> bool {
    let state = node.parsing_state();
    let rules = state.rules();
    let (commands_listed, specials_listed) = match &state.ext().restricted_interior {
        RestrictedInterior::Template(template) => (!template.commands.is_empty(), template.specials),
        RestrictedInterior::Document => (false, false),
    };
    !rules.comments.enabled
        && !rules.paragraphs.enabled
        && (commands_listed || !rules.commands.enabled)
        && (specials_listed || !rules.specials.enabled)
}

/// The regions of a callable's arguments and node slots (techy's `ParsedSlot`,
/// the parts of its content outside the arguments), in record order.
fn regions(callable: &CallableData<Flm>) -> impl Iterator<Item = &ChildRegion> {
    callable
        .arguments
        .iter()
        .filter_map(|argument| argument.region.as_ref())
        .chain(callable.slots.iter().map(|slot| &slot.region))
}

/// Whether a node is source text around an argument or a body rather than
/// content (see the module documentation): a direct child of a callable that
/// lies in no content range anchored on the callable itself, neither an
/// argument's nor a node slot's (a node slot, techy's `ParsedSlot`, is a part
/// of the callable's content outside its arguments), or a child of the node
/// holding an argument's or slot's content that lies outside that content
/// range.
fn is_region_noise(node: &NodeRef<'_, Flm>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let Ok(index) = u32::try_from(node.id().index()) else {
        return false;
    };
    if let Some(callable) = parent.callable() {
        return !regions(callable).any(|region| {
            region.content_parent() == parent.id() && region.content_range().contains(&index)
        });
    }
    let Some(callable) = parent.parent().and_then(|grand| grand.callable()) else {
        return false;
    };
    regions(callable)
        .find(|region| region.content_parent() == parent.id())
        .is_some_and(|region| !region.content_range().contains(&index))
}

// --- the block-structure part -----------------------------------------------------------

/// Checks `tree` against the block-structure part of the [tree
/// contract](self), and returns every violation: expansion is complete,
/// paragraphs and content regions are laid out, decorators are placed, text is
/// resolved, and source maps are consistent. It assumes that the tree satisfies
/// the parse part ([`check_parse_contract`]); a tree returned by
/// [`structure`](super::structure) satisfies both.
///
/// The violations come in the tree's storage order; the result is empty when
/// the tree satisfies the rules. When techy's [`validate_tree`] fails, that one
/// violation is the whole result and nothing else is checked.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::lang::rewrites::blocks::{check_block_contract, structure, BlocksConfig};
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmDiagnostics, FlmSource, LangInputs, ParseOptions,
/// };
///
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("One paragraph.\n\nAnother one."));
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::Blocks, &ParseOptions::default(),
/// ).expect("the input is shallow enough to parse");
///
/// let mut diags = FlmDiagnostics::new();
/// let tree = structure(
///     fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags,
/// ).expect("the input is shallow enough to structure");
///
/// // What the pass produces satisfies the tree contract.
/// assert!(check_block_contract(&tree).is_empty());
/// ```
///
/// See also [`check_parse_contract`], the part this one assumes.
pub fn check_block_contract(tree: &NodeTree<Flm>) -> Vec<ContractViolation> {
    let mut out = Vec::new();
    if let Err(violation) = validate_tree(tree) {
        out.push(ContractViolation {
            node: violation.node,
            kind: ContractViolationKind::Tree(violation),
        });
        return out;
    }
    let facts = Facts::of(tree);
    for node in tree.iter_storage_order() {
        let ext = node.parsing_state().ext();
        let element = element_of(&node);

        // Expansion is complete: no placeholder, and no use site of an element
        // carrying the `Expand` aspect.
        if node.callable_type() == Some(FlmCallableType::Placeholder)
            || ElementSpec::same_value(element, &ext.structural.placeholder)
        {
            out.push(ContractViolation::at(
                &node,
                ContractViolationKind::PlaceholderSurvived,
            ));
        }
        if element.has_aspect::<dyn Expand>() {
            out.push(ContractViolation::at(
                &node,
                ContractViolationKind::UseSiteSurvived { element: element.name.clone() },
            ));
        }
        // Paragraphs and regions: markers appear nowhere outside opaque regions.
        if !facts.opaque(&node) && ElementSpec::same_value(element, &ext.structural.parbreak) {
            out.push(ContractViolation::at(
                &node,
                ContractViolationKind::ParbreakSurvived,
            ));
        }
        // Paragraphs, their containers, decorator placement, edge trimming.
        if node.is_callable() {
            check_callable_regions(&node, &facts, &mut out);
        } else if node.group_type() == Some(FlmGroupType::Paragraph) {
            check_paragraph(&node, &mut out);
        } else if node.group_type() == Some(FlmGroupType::Content) {
            if !facts.is_content_parent(&node) {
                check_inline_container(&node, element, &mut out);
            }
        } else if node.is_list() && !facts.is_content_parent(&node) {
            check_list(&node, &mut out);
        }
        // Text is final.
        if is_resolved_text(&node, &facts) {
            let text = node.chars().unwrap_or("");
            let raw = text
                .bytes()
                .any(|byte| matches!(byte, b'\t' | b'\r' | b'\n'))
                || text.contains("  ");
            if raw {
                out.push(ContractViolation::at(
                    &node,
                    ContractViolationKind::WhitespaceNotCollapsed,
                ));
            }
            let next = node
                .parent()
                .zip(node.index_in_parent())
                .and_then(|(parent, index)| parent.child(index + 1));
            if next.is_some_and(|next| is_resolved_text(&next, &facts)) {
                out.push(ContractViolation::at(
                    &node,
                    ContractViolationKind::AdjacentText,
                ));
            }
        }
        // Provenance.
        if let Some(map) = node.ext().source_map.as_ref() {
            match node.chars() {
                None => out.push(ContractViolation::at(
                    &node,
                    ContractViolationKind::SourceMapOnNonText,
                )),
                Some(text) if map.total_len() as usize != text.len() => {
                    let kind = ContractViolationKind::SourceMapDoesNotTile {
                        mapped: map.total_len(),
                        text_len: text.len(),
                    };
                    out.push(ContractViolation::at(&node, kind));
                }
                Some(_) => {}
            }
        }
    }
    out
}

/// The checks on one paragraph: delimiters, content, block roles, edges.
fn check_paragraph(paragraph: &NodeRef<'_, Flm>, out: &mut Vec<ContractViolation>) {
    if let Some((open, close)) = paragraph.group_delimiters() {
        if !open.is_empty() || !close.is_empty() {
            let kind = ContractViolationKind::ParagraphDelimiters {
                open: open.to_string(),
                close: close.to_string(),
            };
            out.push(ContractViolation::at(paragraph, kind));
        }
    }
    let children: Vec<NodeRef<'_, Flm>> = paragraph.children().iter().collect();
    if !children.iter().any(is_content) {
        out.push(ContractViolation::at(
            paragraph,
            ContractViolationKind::EmptyParagraph,
        ));
    }
    for (index, child) in children.iter().enumerate() {
        match block_role_of(child) {
            BlockRole::Block => {
                let kind = ContractViolationKind::BlockInParagraph {
                    element: element_of(child).name.clone(),
                };
                out.push(ContractViolation::at(child, kind));
            }
            BlockRole::BlockDecorator(Placement::Leads) => {
                if children[..index].iter().any(|before| !before.is_comment()) {
                    let kind = ContractViolationKind::LeadsNotFirst {
                        element: element_of(child).name.clone(),
                    };
                    out.push(ContractViolation::at(child, kind));
                }
            }
            BlockRole::BlockDecorator(Placement::Trails) => {
                if children[index + 1..]
                    .iter()
                    .any(|after| !after.is_comment())
                {
                    let kind = ContractViolationKind::TrailsNotLast {
                        element: element_of(child).name.clone(),
                    };
                    out.push(ContractViolation::at(child, kind));
                }
            }
            BlockRole::BlockDecorator(Placement::Free) | BlockRole::Inline(_) => {}
        }
    }
    let leads = |node: &NodeRef<'_, Flm>| {
        node.is_comment()
            || matches!(
                block_role_of(node),
                BlockRole::BlockDecorator(Placement::Leads)
            )
    };
    let trails = |node: &NodeRef<'_, Flm>| {
        node.is_comment()
            || matches!(
                block_role_of(node),
                BlockRole::BlockDecorator(Placement::Trails)
            )
    };
    let head = children.iter().find(|child| !leads(child));
    if let Some(text) = head.and_then(|child| child.chars()) {
        if text.bytes().next().is_some_and(is_ascii_space) {
            out.push(ContractViolation::at(
                head.expect("found"),
                ContractViolationKind::ParagraphEdgeNotTrimmed,
            ));
        }
    }
    let tail = children.iter().rev().find(|child| !trails(child));
    if let Some(text) = tail.and_then(|child| child.chars()) {
        if text.bytes().next_back().is_some_and(is_ascii_space) {
            out.push(ContractViolation::at(
                tail.expect("found"),
                ContractViolationKind::ParagraphEdgeNotTrimmed,
            ));
        }
    }
}

/// The checks on a callable's arguments and node slots (techy's `ParsedSlot`,
/// the parts of its content outside the arguments, such as an environment's
/// body). An argument's content is checked
/// as blocks exactly when [`argument_holds_blocks`] returns `true`
/// (declared blocks, or declared `InlineOrBlocks` and holding a block-level
/// node, a paragraph included), and as inline content otherwise. A slot's
/// content follows the body declaration's layout: checked as blocks, as inline
/// content, or not at all when it is opaque (a verbatim body, a body read in
/// math mode). Content inside an opaque region is skipped in every case (an
/// argument or a body that takes math mode from around it).
fn check_callable_regions(
    node: &NodeRef<'_, Flm>,
    facts: &Facts,
    out: &mut Vec<ContractViolation>,
) {
    let Some(callable) = node.callable() else {
        return;
    };
    for i in 0..callable.arguments.len() {
        let Some(content) = node.argument_content_nodes(i) else {
            continue;
        };
        let layout = if argument_holds_blocks(node, i) { Region::Blocks } else { Region::Inline };
        check_region(node, content.iter(), layout, facts, out);
    }
    let layout = slot_layout(node);
    for i in 0..callable.slots.len() {
        let Some(content) = node.slot_content_nodes(i) else {
            continue;
        };
        check_region(node, content.iter(), layout, facts, out);
    }
}

/// Checks the content of one of `node`'s arguments or node slots (techy's
/// `ParsedSlot`, the parts of its content outside the arguments) against the rule of
/// `layout`: nothing inline when laid out as blocks, no paragraph when laid
/// out inline, nothing checked when opaque. Children inside an opaque region
/// are skipped regardless of the layout.
fn check_region<'t>(
    node: &NodeRef<'t, Flm>,
    content: impl Iterator<Item = NodeRef<'t, Flm>>,
    layout: Region,
    facts: &Facts,
    out: &mut Vec<ContractViolation>,
) {
    for child in content.filter(|child| !facts.opaque(child)) {
        match layout {
            Region::Blocks if !is_block_child(&child) => {
                let kind = ContractViolationKind::InlineInBlocksRegion {
                    element: element_of(&child).name.clone(),
                };
                out.push(ContractViolation::at(&child, kind));
            }
            Region::Inline if is_paragraph(&child) => {
                let kind = ContractViolationKind::ParagraphInInlineContainer {
                    element: element_of(node).name.clone(),
                };
                out.push(ContractViolation::at(&child, kind));
            }
            Region::Blocks | Region::Inline | Region::Raw => {}
        }
    }
}

/// The check on a `{…}` group, whose content is inline: no paragraph among its
/// children.
fn check_inline_container(
    group: &NodeRef<'_, Flm>,
    element: &ElementSpec,
    out: &mut Vec<ContractViolation>,
) {
    for child in group.children().iter() {
        if is_paragraph(&child) {
            let kind = ContractViolationKind::ParagraphInInlineContainer {
                element: element.name.clone(),
            };
            out.push(ContractViolation::at(&child, kind));
        }
    }
}

/// The check on a container whose layout the tree does not record (the root, a
/// container a hook built): when it holds a paragraph, it is laid out as
/// blocks, and then it must hold nothing inline.
fn check_list(list: &NodeRef<'_, Flm>, out: &mut Vec<ContractViolation>) {
    let children: Vec<NodeRef<'_, Flm>> = list.children().iter().collect();
    if !children.iter().any(is_paragraph) {
        return;
    }
    for child in &children {
        if !is_block_child(child) {
            let kind = ContractViolationKind::InlineInBlocksRegion {
                element: element_of(child).name.clone(),
            };
            out.push(ContractViolation::at(child, kind));
        }
    }
}

fn is_paragraph(node: &NodeRef<'_, Flm>) -> bool {
    node.group_type() == Some(FlmGroupType::Paragraph)
}

/// Whether a node may be a child of a content region laid out as blocks: a
/// paragraph, a node of block role `Block`, or a comment.
fn is_block_child(node: &NodeRef<'_, Flm>) -> bool {
    is_paragraph(node) || node.is_comment() || matches!(block_role_of(node), BlockRole::Block)
}

/// Whether a node is text that text resolution has resolved: content text
/// outside every opaque region (math-mode text, and the content of mathematics
/// and verbatim groups and everything inside them, are not resolved),
/// excluding the source text around an argument or a body.
fn is_resolved_text(node: &NodeRef<'_, Flm>, facts: &Facts) -> bool {
    node.is_chars() && !facts.opaque(node) && !is_region_noise(node)
}
