//! Reading a float's body: the three commands `\caption[alt=…]{…}`,
//! `\label{…}` and `\nonumber`, the float's content, and whether the float
//! type's numbering policy numbers the float.
//!
//! A float's body is read each time it is needed, and nothing read is stored.
//! Two readers exist, for two uses:
//!
//! - [`float_fields`] reads the body and reports nothing. The float's reference
//!   point ([`FloatElement`]'s implementation of
//!   [`Referenceable`](crate::model::aspects::Referenceable)) is computed
//!   through it.
//! - [`read_float`] reads the body through [`float_fields`] and reports, at the
//!   float being lowered, every problem of the body (the conditions of
//!   [`diagnostics`](super::diagnostics)), and which of the float type's
//!   content handlers accepted the content, or the refusals where none did.
//!   Every lowering rule of a float calls
//!   it, so a document gets the same diagnostics on every backend.
//!
//! # Where the commands are read
//!
//! The three commands are the elements of the float's scoped package
//! [`FIELDS_PACKAGE`](super::FIELDS_PACKAGE). Each is inline content, so the
//! block-structure pass ([`blocks`](crate::lang::rewrites::blocks)), which
//! groups a body's inline content into paragraphs, places a command written
//! on its own line in a paragraph. The reader therefore takes a command from two places: directly
//! in the body, and directly in a paragraph that the block-structure pass
//! built in the body.
//!
//! The **content** is every other direct child of the body, except source
//! comments, whitespace (text made of whitespace alone, and the nodes that
//! are whitespace, such as `~` and `\ `), and paragraph breaks. A paragraph
//! that holds nothing but commands, whitespace and source comments is left out
//! of the content as a whole; a paragraph that holds a command beside other
//! content is one content node, and the float's rule leaves the command out
//! when it lowers that paragraph.
//!
//! On the raw tree that the recomposed LaTeX reads (the frozen tree that the
//! block-structure pass has not processed; [the raw
//! tree](mod@crate::stdcore::backends::latex_recomposed::rules#the-raw-tree)), no
//! paragraph is built: the commands and the content are the body's direct
//! children, and the paragraph breaks among them are left out.
//!
//! A command written deeper, inside a group, a list or another construct, does
//! not belong to the float: it reaches the lowering as a field node no
//! construct read, which is reported as `flm.lowering.field-not-read`
//! ([`FieldNotRead`](crate::stdcore::backends::lowering::error::FieldNotRead)).

use techy::core::node::NodeRef;
use techy::extract::parse_keyval_drop_annotations;

use crate::core::lowering::{LowerError, Lowerer};
use crate::lang::copy::{
    ValueContent, ValuePiece, WrittenPiece, copy_pieces_into_value, key_value_pieces,
};
use crate::lang::guard::{GuardPolicy, on_worker_thread};
use crate::lang::mint::field_name;
use crate::lang::{Flm, FlmGroupType};
use crate::model::aspects::ContentRef;
use crate::lang::structural::PARBREAK;
use crate::lang::text::{is_blank, is_space};
use crate::model::element::{element_of, element_value};
use crate::stdcore::features::numbering::COUNTERS;
use crate::stdcore::features::refs::{self, ANCHORS, LABEL_FIELD};

use super::config::{FloatNumbering, FloatType};
use super::content::{Handlers, Verdict};
use super::diagnostics::{
    CaptionMissingValue, CaptionRepeated, CaptionRepeatedKey, CaptionUnknownKey, ContentRefused,
    LabelWithoutRefs, NoNumbering, NonumberWithoutEffect,
};
use super::elements::FloatElement;
use super::{ALT_KEY, CAPTION_FIELD, CAPTION_OPTIONS_ARG, CAPTION_TEXT_ARG, NONUMBER_FIELD};

/// The commands and the content of a float's body, each list in document
/// order, as [`float_body`] reads them. See the [module
/// documentation](self) for which nodes go where.
#[derive(Debug, Clone, Default)]
pub struct FloatBody<'t> {
    /// The content nodes: what the content handlers are given
    /// ([`FloatContentHandler::admit`](super::FloatContentHandler::admit)) and
    /// what the float's rule lowers as the float's content.
    pub content: Vec<NodeRef<'t, Flm>>,
    /// Every `\caption` node, in order; the first is the float's caption.
    pub captions: Vec<NodeRef<'t, Flm>>,
    /// Every `\nonumber` node, in order.
    pub nonumbers: Vec<NodeRef<'t, Flm>>,
    /// Every `\label` node, in order.
    pub labels: Vec<NodeRef<'t, Flm>>,
}

/// Whether `node` is one of the float's three commands (`\caption`,
/// `\label`, `\nonumber`): what the float's rules leave out of a paragraph
/// they lower.
pub(super) fn is_float_field(node: &NodeRef<'_, Flm>) -> bool {
    field_name(node).is_some_and(|name| {
        let name = name.as_str();
        name == CAPTION_FIELD || name == NONUMBER_FIELD || name == LABEL_FIELD
    })
}

/// Whether `node` is left out of a float's content: a source comment,
/// whitespace ([`is_blank`]: text made of whitespace alone, or a node that is
/// whitespace, such as `~` and `\ `), or a paragraph break (which is found in
/// the body where the block-structure pass built no paragraph, as on the raw
/// tree the recomposed LaTeX reads).
pub(super) fn is_aside(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment() || is_blank(node) || element_of(node).name.as_str() == PARBREAK
}

/// Whether `node` is a paragraph that the block-structure pass built.
pub(super) fn is_paragraph(node: &NodeRef<'_, Flm>) -> bool {
    node.group_type() == Some(FlmGroupType::Paragraph)
}

/// Returns the commands and the content of the body of the float `node`, as
/// the [module documentation](self) describes. Returns an empty
/// [`FloatBody`] for a node with no body. Reports nothing.
// See decision rationale at [§flm-dd-dr:std-floats-fields].
pub fn float_body<'t>(node: &NodeRef<'t, Flm>) -> FloatBody<'t> {
    let mut body = FloatBody::default();
    let Some(children) = node.body() else { return body };
    let file = |field: NodeRef<'t, Flm>, body: &mut FloatBody<'t>| {
        match field_name(&field).map(|name| name.as_str()) {
            Some(CAPTION_FIELD) => body.captions.push(field),
            Some(NONUMBER_FIELD) => body.nonumbers.push(field),
            _ => body.labels.push(field),
        }
    };
    for child in children.iter() {
        if is_aside(&child) {
            continue;
        }
        if is_float_field(&child) {
            file(child, &mut body);
            continue;
        }
        if is_paragraph(&child) {
            let mut holds_content = false;
            for inner in child.children().iter() {
                if is_float_field(&inner) {
                    file(inner, &mut body);
                } else if !is_aside(&inner) {
                    holds_content = true;
                }
            }
            if !holds_content {
                continue;
            }
        }
        body.content.push(child);
    }
    body
}

/// One problem of a caption's optional argument, in the order written.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CaptionProblem {
    /// A key other than `alt`, as written, or the whole argument as written
    /// where it is not key-value pairs.
    UnknownKey(String),
    /// `alt` written with no value.
    MissingValue,
    /// `alt=` written a second time.
    RepeatedKey,
}

/// A float's caption, read from its first `\caption` command
/// ([`read_caption`]): the command's node, and the caption's alternative text
/// where its optional argument gives one.
#[derive(Debug, Clone)]
pub struct Caption<'t> {
    /// The `\caption` node.
    pub node: NodeRef<'t, Flm>,
    /// The caption's **alternative text**, which a reference to an
    /// unnumbered float shows in place of the full caption: the value of
    /// `alt=` as written in the document, as nodes of the optional argument
    /// and as text cut from a run of characters that also holds the key.
    /// `None` where the optional argument gives no `alt=` that can be read.
    pub alt: Option<Vec<WrittenPiece<'t>>>,
    /// The problems of the optional argument, in the order written.
    problems: Vec<CaptionProblem>,
}

impl<'t> Caption<'t> {
    /// Returns the caption's full content: the nodes of its mandatory
    /// argument. Returns an empty list where the argument holds nothing.
    pub fn content(&self) -> Vec<NodeRef<'t, Flm>> {
        self.node
            .argument_content_nodes_named(CAPTION_TEXT_ARG)
            .ok()
            .flatten()
            .map(|nodes| nodes.iter().collect())
            .unwrap_or_default()
    }

    /// Returns what a reference to the float shows where the float has no
    /// number, as the content of its reference point
    /// ([`RefPoint::content`](crate::model::aspects::RefPoint::content)):
    ///
    /// - where the optional argument gives an alternative text, a copy of it
    ///   in a synthesized value of its own
    ///   ([`ContentRef::Synthesized`]); `float`, the float node, gives the
    ///   position in the source of the parts of the copy that have none of
    ///   their own;
    /// - otherwise, the caption's mandatory argument where it is written as
    ///   one group or one token, referred to by that node
    ///   ([`ContentRef::Node`]);
    /// - `None` otherwise, and where the copy fails, which only content
    ///   nested deeper than the [recursion guard](crate::lang::guard) allows
    ///   can cause (the parse
    ///   refuses such content first).
    pub fn shown_content(&self, float: &NodeRef<'t, Flm>) -> Option<ContentRef> {
        if let Some(alt) = &self.alt {
            let pieces: Vec<ValuePiece<'_, '_>> = alt.iter().map(WrittenPiece::as_value_piece).collect();
            // A reference to the float shows the copy; the caption shows its
            // full content where it stands.
            return on_worker_thread(|| {
                copy_pieces_into_value(&pieces, float, ValueContent::Shown, GuardPolicy::StackBudget)
            })
            .ok()?
            .ok()
            .map(ContentRef::Synthesized);
        }
        // The argument is written as one group, or as one token: that node
        // holds the content by itself. See decision rationale at
        // [§flm-dd-dr:std-floats-point].
        let region: Vec<NodeRef<'t, Flm>> = self
            .node
            .argument_nodes_named(CAPTION_TEXT_ARG)
            .ok()
            .flatten()
            .map(|nodes| nodes.iter().filter(|node| !(node.is_comment() || is_blank(node))).collect())
            .unwrap_or_default();
        match region.as_slice() {
            [one] => Some(ContentRef::Node(one.id())),
            _ => None,
        }
    }
}

/// Reads the caption `node`, a `\caption` node: the `alt=` of its optional
/// argument and the argument's problems. Reports nothing; [`read_float`]
/// reports the problems.
///
/// The optional argument holds key-value pairs, read by techy's key-value
/// reader ([`parse_keyval_drop_annotations`]), with the only key [`ALT_KEY`]
/// (`alt`), whose value is content: `alt=Short`, `alt={Short, with a
/// comma}`. An argument holding nothing but whitespace (`~` and `\ `
/// included) and comments is read as if it were not written. These are
/// recorded as problems: any other key; an argument that is not key-value
/// pairs, or that holds no `=` at all (`\caption[Short]{…}` is read as the
/// key `Short`); `alt` with no value; and a second `alt=`, the first being
/// kept.
pub fn read_caption<'t>(node: NodeRef<'t, Flm>) -> Caption<'t> {
    let mut caption = Caption { node, alt: None, problems: Vec::new() };
    let Some(content) = node.argument_content_nodes_named(CAPTION_OPTIONS_ARG).ok().flatten() else {
        return caption;
    };
    if content.iter().all(|child| child.is_comment() || is_blank(&child)) {
        return caption;
    }
    let Ok(keyvals) = parse_keyval_drop_annotations(content) else {
        let written: String = content.iter().map(|node| node.span_content().to_string()).collect();
        caption.problems.push(CaptionProblem::UnknownKey(written.trim_matches(is_space).to_string()));
        return caption;
    };
    for entry in keyvals.iter() {
        let key = entry.key();
        if key.is_empty() {
            continue;
        }
        if key != ALT_KEY {
            caption.problems.push(CaptionProblem::UnknownKey(key.to_string()));
            continue;
        }
        let Some(value) = entry.value() else {
            caption.problems.push(CaptionProblem::MissingValue);
            continue;
        };
        if caption.alt.is_some() {
            caption.problems.push(CaptionProblem::RepeatedKey);
            continue;
        }
        caption.alt = Some(key_value_pieces(&content, value));
    }
    caption
}

/// A float's body as [`float_fields`] reads it, without reporting anything:
/// the float type, the commands and the content, the caption, and whether the
/// type's numbering policy numbers the float.
#[derive(Debug, Clone)]
pub struct FloatFields<'t> {
    /// The float's type.
    pub float_type: &'t FloatType,
    /// The body's commands and content.
    pub body: FloatBody<'t>,
    /// The float's caption, read from its first `\caption`. `None` where
    /// the body holds no `\caption`.
    pub caption: Option<Caption<'t>>,
    /// Whether the type's numbering policy numbers the float, given its
    /// commands ([`FloatNumbering::numbers`]): a float with at least one
    /// `\label` counts as labeled, and a float with at least one `\nonumber`
    /// as marked unnumbered. Whether the float is shown with a number also
    /// depends on the output: without the numbering feature, the plain-text
    /// and HTML outputs show it without a number ([`NoNumbering`]), while the
    /// recomposed LaTeX still writes it as numbered and LaTeX numbers it.
    pub numbered: bool,
}

/// Reads the body of the float `node` and reports nothing. Returns `None`
/// for a node that is not a float (whose element does not carry a
/// [`FloatElement`]).
///
/// The float's reference point is computed from what it returns;
/// [`read_float`] calls it and reports the problems.
pub fn float_fields<'t>(node: &NodeRef<'t, Flm>) -> Option<FloatFields<'t>> {
    let value = element_value::<FloatElement>(node)?;
    let body = float_body(node);
    let caption = body.captions.first().map(|first| read_caption(*first));
    let numbered = value.float_type.numbering.numbers(!body.labels.is_empty(), !body.nonumbers.is_empty());
    Some(FloatFields { float_type: &value.float_type, body, caption, numbered })
}

/// A float as [`read_float`] reads it for a lowering rule: its body, and
/// which of its type's content handlers accepted the content, or the
/// refusals where none did.
#[derive(Debug, Clone)]
pub struct FloatReading<'t> {
    /// The float's body.
    pub fields: FloatFields<'t>,
    /// The content handlers' verdict on the content.
    pub verdict: Verdict,
}

/// Reads the float `node` for a lowering rule, as every rule lowering a float
/// reads it, and reports through `lw` the problems of the float's body:
///
/// - each `\caption` after the first, an Error ([`CaptionRepeated`]) at that
///   caption; the first is the float's caption;
/// - the problems of the first caption's optional argument, each an Error at
///   the caption: a key other than `alt` ([`CaptionUnknownKey`]), `alt` with
///   no value ([`CaptionMissingValue`]), a second `alt=`
///   ([`CaptionRepeatedKey`]);
/// - each `\nonumber` that changes nothing, a Note
///   ([`NonumberWithoutEffect`]) at it: in a float of a type that is never
///   numbered, in a float without `\label` of a type numbered only when
///   labeled, and each `\nonumber` after the first;
/// - a `\label` when the references feature is not registered (the anchors
///   table is absent), a Note ([`LabelWithoutRefs`]) at the first label;
/// - a float that its policy numbers when the numbering feature is not
///   registered (the counter table is absent), an Error ([`NoNumbering`]) at
///   the float;
/// - content that none of the type's content handlers accepts, an Error
///   ([`ContentRefused`]) at the float. The message lists what each handler
///   that a registered feature contributes would accept; where no handler the
///   type lists is contributed, it says that none of the kinds of content the
///   type accepts is available.
///
/// `handlers` are the content handlers of the registry, as the feature's
/// rules hold them. The calling rule must declare optional reads of the
/// counter table [`COUNTERS`] and of the anchors table [`ANCHORS`]
/// ([`float_decl`](super::float_decl) declares both).
///
/// # Errors
///
/// [`LowerError`] for a node that is not a float; where the float has a
/// `\label` and the calling rule did not declare a read of [`ANCHORS`]; and
/// where the float's numbering policy numbers it and the calling rule did not
/// declare a read of [`COUNTERS`].
pub fn read_float<'t>(
    node: &NodeRef<'t, Flm>,
    handlers: &Handlers,
    lw: &mut dyn Lowerer,
) -> Result<FloatReading<'t>, LowerError> {
    let fields = float_fields(node)
        .ok_or_else(|| LowerError::structure("a float without its element's data"))?;
    let float_type = fields.float_type;
    let type_name = float_type.name.to_string();
    for extra in fields.body.captions.iter().skip(1) {
        lw.error(CaptionRepeated { float_type: type_name.clone() }, extra.span().clone());
    }
    if let Some(caption) = &fields.caption {
        let span = caption.node.span().clone();
        for problem in &caption.problems {
            match problem {
                CaptionProblem::UnknownKey(key) => {
                    lw.error(CaptionUnknownKey { key: key.clone() }, span.clone())
                }
                CaptionProblem::MissingValue => lw.error(CaptionMissingValue, span.clone()),
                CaptionProblem::RepeatedKey => lw.error(CaptionRepeatedKey, span.clone()),
            }
        }
    }
    let labeled = !fields.body.labels.is_empty();
    for (at, nonumber) in fields.body.nonumbers.iter().enumerate() {
        let reason = match float_type.numbering {
            FloatNumbering::Unnumbered => format!("a ‘{type_name}’ is never numbered"),
            FloatNumbering::AutoNumbered if !labeled => {
                format!("a ‘{type_name}’ without a \\label is not numbered")
            }
            _ if at > 0 => String::from("it is already written in this float"),
            _ => continue,
        };
        lw.note(NonumberWithoutEffect { reason }, nonumber.span().clone());
    }
    if let Some(first) = fields.body.labels.first() {
        if lw.get(ANCHORS)?.is_none() {
            lw.note(LabelWithoutRefs { float_type: type_name.clone() }, first.span().clone());
        }
    }
    if fields.numbered && lw.get(COUNTERS)?.is_none() {
        lw.error(NoNumbering { float_type: type_name.clone() }, node.span().clone());
    }
    let verdict = handlers.verdict(&float_type.handlers, &fields.body.content);
    if let Verdict::Refused(refusals) = &verdict {
        let admitted = if refusals.is_empty() {
            let kinds: Vec<String> = float_type.handlers.iter().map(|kind| format!("‘{kind}’")).collect();
            format!(
                "content of the kind {}, and the program processing this document handles no \
                 such content",
                english_or(&kinds)
            )
        } else {
            english_or(refusals)
        };
        lw.error(ContentRefused { float_type: type_name, admitted }, node.span().clone());
    }
    Ok(FloatReading { fields, verdict })
}

/// Returns `items` joined as an English list with `or` and no serial comma: `a`,
/// `a or b`, `a, b or c`.
fn english_or(items: &[String]) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
}

/// Returns the labels written in the float's body, read as the references
/// feature reads a run of `\label` nodes ([`refs::label_uses_in`]).
pub(super) fn label_uses(fields: &FloatFields<'_>) -> Vec<refs::LabelUse> {
    refs::label_uses_in(fields.body.labels.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::english_or;

    #[test]
    fn the_admitted_contents_join_with_or() {
        let items = |list: &[&str]| list.iter().map(|item| item.to_string()).collect::<Vec<_>>();
        assert_eq!(english_or(&items(&["one image"])), "one image");
        assert_eq!(english_or(&items(&["a", "b"])), "a or b");
        assert_eq!(english_or(&items(&["a", "b", "c"])), "a, b or c");
    }
}
