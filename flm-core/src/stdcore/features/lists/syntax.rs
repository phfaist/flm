//! Reading the two optional arguments of a list: the optional argument of a
//! list environment, which may give the list's own label template and its
//! own reference template, and the optional argument of an `\item`, which
//! may give the item's tag.
//!
//! Both are key-value arguments, read as the heading commands read theirs
//! ([`parse_keyval_drop_annotations`]); each also has a short form, a bracket
//! holding no `=` outside braces. The readers ([`list_options`],
//! [`item_options`]) report nothing: they return what they could read and,
//! separately, what they could not, which the list's lowering rule reports
//! ([`UnknownKey`](super::UnknownKey), [`MissingValue`](super::MissingValue),
//! [`BadLabelForm`](super::BadLabelForm)).

use techy::core::node::{NodeRef, NodeSlice};
use techy::extract::parse_keyval_drop_annotations;

use crate::lang::Flm;
use crate::lang::copy::{WrittenPiece, key_value_pieces, trim_pieces};
use crate::lang::text::{is_blank, is_space};

use super::{LABEL_KEY, OPTIONS_ARG, REF_KEY, TAG_KEY};

/// The only characters a short label form may hold: the brackets, the
/// parentheses, the full stop, the colon, the hyphen, the plus and star
/// signs, and the five characters that stand for the number.
const SHORT_FORM_CHARACTERS: &str = "[]().:-+*aAiI1";

/// The characters a short label form's reference template drops from the end
/// of the written form: the full stop, the colon and the hyphen.
const SHORT_FORM_TRAILING: [char; 3] = ['.', ':', '-'];

/// The five characters that stand for the number in a short label form, each
/// with the number format it stands for, written as that format is named in a
/// template.
const TOKENS: [(char, &str); 5] =
    [('a', "alph"), ('A', "Alph"), ('i', "roman"), ('I', "Roman"), ('1', "arabic")];

/// The label template a list writes in its optional argument, as
/// [`list_options`] reads it.
#[derive(Debug, Clone)]
pub struct OwnLabel<'t> {
    /// The template in the language of a counter style's number template,
    /// where every `#{name}` is the number in the number format `name`,
    /// whether the author wrote it so (`label=(#{alph})`) or as a short form
    /// (`(a)`, translated). Not compiled: this module does not depend on the
    /// built language.
    pub template: String,
    /// The reference template the label gives the list where it writes no
    /// `ref=`: the same text as [`template`](Self::template) for `label=`,
    /// and for a short form that writes a number, the short form with every
    /// full stop, colon and hyphen at its end dropped before it is
    /// translated ([`short_form_ref`]) — `[1.:]` refers to its items as
    /// `#{arabic}`, `[(i)]` as `(#{roman})`, `[a-]` as `#{alph}`.
    pub ref_template: String,
    /// What the author wrote: the short form, or the value of `label=`.
    pub written: String,
    /// The label's content, in order, as the bracket writes it: the nodes of
    /// a short form (`[-]`), or the value of `label=` (braced or not, since
    /// braces only delimit a value) as nodes of the document and text split
    /// off a run of characters that also holds the key (`label=*`), with the
    /// whitespace at either end dropped. The reference point of an untagged
    /// item of an unnumbered list copies it into the content the point
    /// carries, so that a reference to the item shows the label.
    pub pieces: Vec<WrittenPiece<'t>>,
}

/// One thing the reader of an optional argument could not read, in the
/// order written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OptionProblem {
    /// A key the construct does not declare, or a bracket that is neither a
    /// short form nor key-values, as written.
    UnknownKey(String),
    /// A known key written without a value.
    MissingValue(String),
    /// A short label form holding more than one of the five number
    /// characters: what was written, and its readings with the first and the
    /// second of them as the number.
    BadLabelForm { written: String, first: String, second: String },
}

/// What a list's optional argument gives: the list's own label template and
/// its own reference template, where it writes them in a way that can be
/// read. Returned by [`list_options`].
#[derive(Debug, Clone, Default)]
pub struct ListOptions<'t> {
    /// The list's own label template, `None` where the argument is absent,
    /// holds nothing but whitespace (`~` and `\ ` included) and comments, or
    /// writes none that can be read.
    pub label: Option<OwnLabel<'t>>,
    /// The reference template the argument writes with `ref=`, as written,
    /// in the language of a counter style's number template (one enclosing
    /// pair of braces taken off, the text trimmed); not compiled. `None`
    /// where no `ref=` is written.
    pub ref_label: Option<String>,
    /// What the reader could not take, in the order written.
    pub(super) problems: Vec<OptionProblem>,
}

impl ListOptions<'_> {
    /// The list's own reference template: its `ref=` where written, else the
    /// one its label gives ([`OwnLabel::ref_template`]), else `None`. A
    /// reference to an untagged item of a numbered list writes the item's
    /// number through it.
    pub fn ref_template(&self) -> Option<&str> {
        self.ref_label
            .as_deref()
            .or_else(|| self.label.as_ref().map(|label| label.ref_template.as_str()))
    }
}

/// An item's tag, as [`item_options`] reads it: the content the author wrote
/// for the item's label, from `\item[My tag]`, `\item[tag=My tag]` or
/// `\item[tag={My tag}]`.
///
/// The item's reference point copies the pieces into the content it
/// carries, so that a reference to the item shows the tag, whichever way it
/// is written.
#[derive(Debug, Clone)]
pub struct ItemTag<'t> {
    /// The tag's content, in order: nodes of the document, and text split off
    /// a run of characters that also holds the key (`tag=Custom`).
    pub pieces: Vec<WrittenPiece<'t>>,
}

/// What an `\item`'s optional argument gives: the item's tag, where it
/// writes one. Returned by [`item_options`].
#[derive(Debug, Clone, Default)]
pub struct ItemOptions<'t> {
    /// The item's tag, `None` where the argument is absent, holds nothing but
    /// whitespace (`~` and `\ ` included) and comments, or writes no tag.
    pub tag: Option<ItemTag<'t>>,
    /// What the reader could not take, in the order written.
    pub(super) problems: Vec<OptionProblem>,
}

/// The content nodes of `node`'s bracket [`OPTIONS_ARG`], or `None` where it
/// is absent or holds nothing but whitespace ([`is_blank`]: `~` and `\ `
/// included) and comments — which is as if it were absent.
fn bracket<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeSlice<'t, Flm>> {
    let content = node.argument_content_nodes_named(OPTIONS_ARG).ok()??;
    let blank = content.iter().all(|child| child.is_comment() || is_blank(&child));
    (!blank).then_some(content)
}

/// Whether a run of sibling nodes holds a `=` outside every group and
/// command: such a bracket is read as key-values rather than as a short
/// form.
fn holds_equals(nodes: &NodeSlice<'_, Flm>) -> bool {
    nodes.iter().any(|node| node.chars().is_some_and(|text| text.contains('=')))
}

/// The source text of a run of whole nodes of the document, in order. A
/// short form is read from it, because the block-structure pass may have
/// changed the characters the tree holds (`--` into a dash, which is not a
/// short-form character); a report quotes a bracket that cannot be read with
/// it.
fn written_text<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> String {
    nodes.into_iter().map(|node| node.span_content().to_string()).collect()
}

/// The text of the value of `label=`, from the nodes the key-value reader
/// returns for it: a run of characters as the tree holds it, a group or a
/// command as its source text, a comment as nothing.
///
/// The characters are taken from the tree because the reader splits a run
/// holding the key and the value, and a piece of a run the block-structure
/// pass changed keeps the span of the whole run, key included; a group or a
/// command is always a whole node of the document, whose source text is what
/// the template compiler reads back as FLM. The compiler makes the same
/// typographic substitutions the block-structure pass makes, so `label=--` and
/// `label={--}` both write a dash.
// See decision rationale at [§flm-dd-dr:std-list-reading].
fn template_text<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> String {
    nodes
        .into_iter()
        .map(|node| match node.chars() {
            Some(text) => text.to_string(),
            None if node.is_comment() => String::new(),
            None => node.span_content().to_string(),
        })
        .collect()
}

/// The label template that the short label form `text` stands for:
/// `Some(Ok(template))` for a text made only of the short form's characters
/// and holding at most one of the five number characters `a`, `A`, `i`, `I`,
/// `1`; `Some(Err((first, second)))` for one holding more (its readings with
/// the first and with the second as the number); and `None` for a text that
/// is not a short form.
///
/// With one number character, the template is the text with that character
/// replaced by the `#{…}` of its number format (`(i)` is `(#{roman})`, `1.`
/// is `#{arabic}.`), as LaTeX's `enumerate` package reads it; with none, the
/// text is a fixed label, such as `-` or `*`.
///
/// ```
/// use flm_core::stdcore::features::lists::short_form;
///
/// assert_eq!(short_form("(i)"), Some(Ok(String::from("(#{roman})"))));
/// assert_eq!(short_form("-"), Some(Ok(String::from("-"))));
/// assert!(matches!(short_form("i.a"), Some(Err(_))));
/// assert_eq!(short_form("Step"), None);
/// ```
pub fn short_form(text: &str) -> Option<Result<String, (String, String)>> {
    if text.is_empty() || !text.chars().all(|ch| SHORT_FORM_CHARACTERS.contains(ch)) {
        return None;
    }
    let reading = |at: usize, ch: char| {
        let name = TOKENS.iter().find(|(token, _)| *token == ch).map(|(_, name)| *name)?;
        Some(format!("{}#{{{name}}}{}", &text[..at], &text[at + ch.len_utf8()..]))
    };
    let mut tokens = text.char_indices().filter_map(|(at, ch)| reading(at, ch));
    Some(match (tokens.next(), tokens.next()) {
        (None, _) => Ok(text.to_string()),
        (Some(only), None) => Ok(only),
        (Some(first), Some(second)) => Err((first, second)),
    })
}

/// The reference template that the short label form `text` gives its list.
/// Where [`short_form`] reads `text` as a template that writes a number, it
/// is `text` with every full stop, colon and hyphen at its end dropped, then
/// translated: `1.:` gives `#{arabic}`, `a-` gives `#{alph}`, `(i)` gives
/// `(#{roman})`. A short form that writes no number is a fixed label, and its
/// reference template is the label itself: `-` gives `-`. `None` where
/// [`short_form`] does not return a template.
///
/// The characters dropped are punctuation that suits a label beside an item
/// but not a reference inside a sentence: a sentence ending on a reference
/// written `2.` would end `Item~2..`. A short form is made of a few known
/// characters, so the rule is exact for it.
/// A template written with `label=` is not changed: its list refers to its
/// items through the same text unless `ref=` says otherwise.
pub fn short_form_ref(text: &str) -> Option<String> {
    let label = short_form(text)?.ok()?;
    if !text.chars().any(|ch| TOKENS.iter().any(|(token, _)| *token == ch)) {
        return Some(label);
    }
    short_form(text.trim_end_matches(SHORT_FORM_TRAILING))?.ok()
}

/// Reads the optional argument of the list environment `node`: the list's
/// own label template, its own reference template, and what could not be
/// read. This function reports nothing; the list's lowering rule reports
/// what could not be read.
///
/// A bracket holding nothing but whitespace (`~` and `\ ` included) and
/// comments is read as if it were not written. A bracket holding no `=`
/// outside braces and consisting only of the characters `[]().:-+*aAiI1`
/// (after one enclosing pair of braces is taken off, which is how a `]` is
/// written inside the bracket) is a **short form** of `label=` — see
/// [`short_form`] — which also gives the list a reference template
/// ([`short_form_ref`]). Any other bracket is read as key-values, whose keys
/// are `label` and `ref`, each value a template as written (one enclosing
/// pair of braces taken off, the text trimmed; its characters as the tree
/// holds them, so that a dash the block-structure pass wrote for `--` stays a
/// dash): `label=` alone sets the label template and, the same text, the
/// reference template; `ref=` sets the reference template. An unknown key, a
/// known key with no value, and a short form holding more than one number
/// character are recorded as problems and give no template; a later `label=`
/// or `ref=` replaces an earlier one.
pub fn list_options<'t>(node: &NodeRef<'t, Flm>) -> ListOptions<'t> {
    let mut options = ListOptions::default();
    let Some(content) = bracket(node) else { return options };
    if !holds_equals(&content) {
        // A lone group protects what it holds, `[{[-]}]`.
        let unwrapped = match (content.len(), content.first()) {
            (1, Some(group)) if group.is_group() => written_text(group.children().iter()),
            _ => written_text(content.iter()),
        };
        let written = unwrapped.trim_matches(is_space).to_string();
        let pieces = trim_pieces(content.iter().map(WrittenPiece::Node).collect());
        match (short_form(&written), short_form_ref(&written)) {
            (Some(Ok(template)), Some(ref_template)) => {
                options.label = Some(OwnLabel { template, ref_template, written, pieces })
            }
            (Some(Err((first, second))), _) => {
                options.problems.push(OptionProblem::BadLabelForm { written, first, second })
            }
            _ => options.problems.push(OptionProblem::UnknownKey(written)),
        }
        return options;
    }
    let Ok(keyvals) = parse_keyval_drop_annotations(content) else {
        let written = written_text(content.iter()).trim_matches(is_space).to_string();
        options.problems.push(OptionProblem::UnknownKey(written));
        return options;
    };
    for entry in keyvals.iter() {
        let key = entry.key();
        if key.is_empty() {
            continue;
        }
        if key != LABEL_KEY && key != REF_KEY {
            options.problems.push(OptionProblem::UnknownKey(key.to_string()));
            continue;
        }
        match (entry.value(), entry.value_content()) {
            (Some(_), Some(unwrapped)) if key == REF_KEY => {
                options.ref_label = Some(template_text(unwrapped.iter()).trim_matches(is_space).to_string());
            }
            (Some(value), Some(unwrapped)) => {
                let written = template_text(unwrapped.iter()).trim_matches(is_space).to_string();
                let pieces = key_value_pieces(&content, value);
                options.label = Some(OwnLabel {
                    template: written.clone(),
                    ref_template: written.clone(),
                    written,
                    pieces,
                });
            }
            _ => options.problems.push(OptionProblem::MissingValue(key.to_string())),
        }
    }
    options
}

/// Reads the optional argument of the `\item` node `node`: the item's tag,
/// and what could not be read. This function reports nothing; the list's
/// lowering rule reports what could not be read.
///
/// A bracket holding nothing but whitespace (`~` and `\ ` included) and
/// comments is read as if it were not written: it gives no tag. A bracket
/// holding no `=` outside braces is the tag whole — `\item[My tag]` is
/// `\item[tag={My tag}]`. Any other bracket is read as key-values, whose one
/// key is `tag`, its value the tag's content, FLM; `tag={a=b}` protects a `=`
/// in braces. An unknown key and a `tag` with no value are recorded as
/// problems; a later `tag=` replaces an earlier one.
pub fn item_options<'t>(node: &NodeRef<'t, Flm>) -> ItemOptions<'t> {
    let mut options = ItemOptions::default();
    let Some(content) = bracket(node) else { return options };
    if !holds_equals(&content) {
        options.tag = Some(ItemTag { pieces: content.iter().map(WrittenPiece::Node).collect() });
        return options;
    }
    let Ok(keyvals) = parse_keyval_drop_annotations(content) else {
        let written = written_text(content.iter()).trim_matches(is_space).to_string();
        options.problems.push(OptionProblem::UnknownKey(written));
        return options;
    };
    for entry in keyvals.iter() {
        let key = entry.key();
        if key.is_empty() {
            continue;
        }
        if key != TAG_KEY {
            options.problems.push(OptionProblem::UnknownKey(key.to_string()));
            continue;
        }
        let Some(value) = entry.value() else {
            options.problems.push(OptionProblem::MissingValue(key.to_string()));
            continue;
        };
        options.tag = Some(ItemTag { pieces: key_value_pieces(&content, value) });
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_form_with_one_number_character_names_its_format() {
        assert_eq!(short_form("(i)"), Some(Ok("(#{roman})".to_string())));
        assert_eq!(short_form("1."), Some(Ok("#{arabic}.".to_string())));
        assert_eq!(short_form("a-"), Some(Ok("#{alph}-".to_string())));
        assert_eq!(short_form("A)"), Some(Ok("#{Alph})".to_string())));
        assert_eq!(short_form("[I]"), Some(Ok("[#{Roman}]".to_string())));
    }

    #[test]
    fn a_short_form_with_no_number_character_is_a_fixed_label() {
        for fixed in ["-", "*", "+", "--", "()", ":"] {
            assert_eq!(short_form(fixed), Some(Ok(fixed.to_string())));
            assert_eq!(short_form_ref(fixed), Some(fixed.to_string()));
        }
    }

    #[test]
    fn a_short_form_s_reference_template_drops_its_trailing_punctuation() {
        assert_eq!(short_form("1.:"), Some(Ok("#{arabic}.:".to_string())));
        assert_eq!(short_form_ref("1.:"), Some("#{arabic}".to_string()));
        assert_eq!(short_form_ref("1."), Some("#{arabic}".to_string()));
        assert_eq!(short_form_ref("a-"), Some("#{alph}".to_string()));
        assert_eq!(short_form_ref("(i)"), Some("(#{roman})".to_string()));
        assert_eq!(short_form_ref("(i)."), Some("(#{roman})".to_string()));
        assert_eq!(short_form_ref("-1-"), Some("-#{arabic}".to_string()));
        assert_eq!(short_form_ref("i.a"), None);
        assert_eq!(short_form_ref("x"), None);
    }

    #[test]
    fn a_short_form_with_two_number_characters_names_both_readings() {
        assert_eq!(
            short_form("i.a"),
            Some(Err(("#{roman}.a".to_string(), "i.#{alph}".to_string())))
        );
    }

    #[test]
    fn any_other_character_makes_no_short_form() {
        for other in ["\u{2022}", "(${alph})", "(b)", "x", " (i)", ""] {
            assert_eq!(short_form(other), None, "{other:?}");
        }
    }
}
