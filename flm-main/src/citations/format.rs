//! Formatting a record as its citation entry: the citation style ([`CitationStyle`]), the
//! record converted for the citation formatter `hayagriva`, the formatter's structured
//! result converted into the parts of a citation entry
//! ([`EntryPiece`](flm_core::stdcore::features::citations::EntryPiece)), and the links
//! appended after it ([`UrlLink`]).
//!
//! The dollar-sign formulas of a record's text ([`text_parts`]) are read the same way here
//! and in the BibTeX export ([`super::export`]).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use autocitefetch::CslValue;
use flm_core::lang::{FlmOrigin, FlmSource};
use flm_core::stdcore::features::citations::EntryPiece;
use hayagriva::citationberg::json::{FixedDateRange, Item, Value};
use hayagriva::citationberg::{FontStyle, FontWeight, IndependentStyle, Locale, Style};
use hayagriva::{BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, ElemChild, ElemMeta};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;

/// The citation style that citation entries are set in by default: a copy of the style
/// "American Physical Society - et al. (if more than 3 authors)" as patched for FLM, under
/// the license that the file states (Creative Commons Attribution-ShareAlike 3.0).
const SHIPPED_STYLE: &str = include_str!("styles/american-physical-society-et-al--patched.csl");

/// The citation style that citation entries are set in, as the setting
/// [`style`](super::ProviderConfig::style) gives it: a style file of the Citation Style
/// Language, or a style that the citation formatter `hayagriva` ships, by its name. A
/// citation style says in which order and with which formatting (italic, bold) the authors,
/// the title, the journal, the volume and the year are written. When the setting is not
/// written, citation entries are set in the style that the program ships: "American
/// Physical Society - et al.", with the volume in bold and the year last, in parentheses.
///
/// The citation style is unrelated to the library's citation counter style, which writes
/// the numbers of the cited works
/// ([the setting `style` of the feature `citations`](flm_core::stdcore::features::citations::CitationsConfig::style)).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum CitationStyle {
    /// The path of a style file (`.csl`), relative to the folder of the file in which it
    /// is written, or to the current folder for a value given on the command line, or
    /// absolute; written `{file: my-style.csl}`. It is read again at every
    /// build, so a change to the file changes the citation entries of the next build. A
    /// file that cannot be read or is not a usable citation style leaves every citation key
    /// that needs it without a citation entry, with an Error at its first citation.
    File(String),
    /// The name of a style that the citation formatter `hayagriva` ships, such as `ieee`
    /// or `american-physics-society`; written `{name: ieee}`. A name that is not the name
    /// of a style that `hayagriva` ships makes the registry build fail.
    Name(String),
}

/// When a link to the record's address (its field `URL`), showing `URL`, is appended to a
/// citation entry, as the setting [`url_link`](super::ProviderConfig::url_link) gives it;
/// written `always`,
/// `never` or `only-if-no-other-link`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UrlLink {
    /// Whenever the record has an address.
    Always,
    /// Never.
    Never,
    /// Only when the record has neither a DOI nor an arXiv identifier, whether or not the
    /// links to those are appended: the default.
    #[default]
    OnlyIfNoOtherLink,
}

/// The links appended after a citation entry, as the settings `doi_link`, `arxiv_link` and
/// `url_link` choose them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Links {
    pub(super) doi: bool,
    pub(super) arxiv: bool,
    pub(super) url: UrlLink,
}

/// Returns the locales that the citation formatter ships: the words of each language that a
/// style writes (`and`, `et al.`). They are the same for every style and every call, so
/// they are loaded once.
fn locales() -> &'static [Locale] {
    static LOCALES: OnceLock<Vec<Locale>> = OnceLock::new();
    LOCALES.get_or_init(hayagriva::archive::locales)
}

/// A citation style prepared for formatting: an independent style (one that holds its own
/// rules) with a `<bibliography>` element.
#[derive(Debug, Clone)]
pub(super) struct Formatter {
    style: Arc<IndependentStyle>,
}

impl Formatter {
    /// Returns the formatter of the style that the program ships ([`SHIPPED_STYLE`]).
    ///
    /// # Errors
    ///
    /// The text of [`from_xml`](Formatter::from_xml); the shipped style is valid, so it
    /// does not fail in practice.
    pub(super) fn shipped() -> Result<Formatter, String> {
        Formatter::from_xml(SHIPPED_STYLE)
    }

    /// Returns the formatter of the style called `name` among the styles that the citation
    /// formatter ships.
    ///
    /// # Errors
    ///
    /// A text beginning with "the citation style `name`" when no style has that name, or
    /// when the style cannot be used ([`from_style`](Formatter::from_style)).
    pub(super) fn archived(name: &str) -> Result<Formatter, String> {
        match hayagriva::archive::ArchivedStyle::by_name(name) {
            Some(archived) => {
                Formatter::from_style(archived.get()).map_err(|detail| format!("the citation style `{name}` {detail}"))
            }
            None => Err(format!(
                "the citation style `{name}` must be the name of a style that the citation formatter hayagriva ships, \
                 such as `ieee` or `american-physics-society`"
            )),
        }
    }

    /// Returns the formatter of the style written in `xml`, the content of a style file. A
    /// dependent style (one that takes its rules from another style, as a journal's style
    /// takes its publisher's) gets the rules of that other style, which must be one that the
    /// citation formatter ships.
    ///
    /// # Errors
    ///
    /// A text that follows the style's name: `xml` is not a style of the Citation Style
    /// Language, or the style cannot be used ([`from_style`](Formatter::from_style)).
    pub(super) fn from_xml(xml: &str) -> Result<Formatter, String> {
        let style = Style::from_xml(xml).map_err(|error| format!("must be a style of the Citation Style Language: {error}"))?;
        Formatter::from_style(style)
    }

    /// Returns the formatter of `style`, a style that the citation formatter has read.
    ///
    /// # Errors
    ///
    /// A text that follows the style's name: the style takes its rules from a style that
    /// the citation formatter does not ship, or it has no `<bibliography>` element, the part
    /// of a style that says how an entry of a list of references is set, so it can set no
    /// citation entry.
    fn from_style(style: Style) -> Result<Formatter, String> {
        let style = match style {
            Style::Independent(style) => style,
            Style::Dependent(dependent) => {
                let parent = &dependent.parent_link.href;
                match hayagriva::archive::ArchivedStyle::by_id(parent).map(|archived| archived.get()) {
                    Some(Style::Independent(style)) => style,
                    _ => {
                        return Err(format!(
                            "takes its rules from the style `{parent}`, which must be a style that the citation \
                             formatter hayagriva ships"
                        ));
                    }
                }
            }
        };
        if style.bibliography.is_none() {
            return Err(String::from(
                "must have a `<bibliography>` element, which says how an entry of a list of references is set",
            ));
        }
        Ok(Formatter { style: Arc::new(style) })
    }

    /// Returns the parts of the citation entry of `record`: the record set in the style,
    /// without the number that a numeric style writes at its head, then the links that
    /// `links` chooses ([`link_pieces`]).
    ///
    /// # Errors
    ///
    /// The text of [`item`] when a date of the record is a range.
    // See decision rationale at [§flm-dd-dr:host-citations-format]
    pub(super) fn entry(&self, record: &CslValue, links: Links) -> Result<Vec<EntryPiece>, String> {
        let item = item(record)?;
        let mut driver = BibliographyDriver::new();
        driver.citation(CitationRequest::from_items(vec![CitationItem::with_entry(&item)], &self.style, locales()));
        let rendered = driver.finish(BibliographyRequest::new(&self.style, None, locales()));
        let mut pieces = Vec::new();
        if let Some(entry) = rendered.bibliography.and_then(|bibliography| bibliography.items.into_iter().next()) {
            // A style that aligns the second field puts the first one apart; the entry is
            // the two, in order.
            for child in entry.first_field.iter().chain(&entry.content.0) {
                push_child(child, &mut pieces);
            }
        }
        trim_start(&mut pieces);
        pieces.extend(link_pieces(record, links));
        Ok(pieces)
    }
}

/// Returns the record converted into the citation formatter's form: each field whose value
/// has a shape the Citation Style Language defines, and a person written with `name` alone
/// read as having that family name ([`with_family_names`]). A field of another shape (a
/// list of ISSNs, a map of the record's own) is left out, since no style reads it.
///
/// # Errors
///
/// A text saying what is wrong when a date of the record is a range, which the formatter
/// cannot set.
fn item(record: &CslValue) -> Result<Item, String> {
    let record = with_family_names(record);
    let mut fields = BTreeMap::new();
    for (name, value) in record.as_object().into_iter().flatten() {
        let Ok(value) = Value::deserialize(value) else { continue };
        if let Value::Date(date) = &value
            && FixedDateRange::try_from(date.clone()).is_ok_and(|range| range.end.is_some())
        {
            return Err(format!("the date field `{name}` of the record must be a single date, not a range"));
        }
        fields.insert(name.clone(), value);
    }
    Ok(Item(fields))
}

/// Returns `record` with every person of its lists of persons (`author`, `editor`, …) that is
/// written with `name` alone rewritten with that text as its family name (`family`). A
/// record from a bibliography file may write an institution, or a name that is not split
/// into its parts, that way.
pub(super) fn with_family_names(record: &CslValue) -> CslValue {
    let mut record = record.clone();
    for value in record.as_object_mut().into_iter().flat_map(|fields| fields.values_mut()) {
        for person in value.as_array_mut().into_iter().flatten() {
            let Some(person) = person.as_object_mut() else { continue };
            if !person.contains_key("family")
                && !person.contains_key("given")
                && let Some(name) = person.remove("name")
            {
                person.insert(String::from("family"), name);
            }
        }
    }
    record
}

/// Adds the citation entry parts of `child`, a part of the citation formatter's result, to
/// `pieces`: a text run with its italic or bold type, a link, plain text; the number of a
/// numeric style and the parts with no text are left out.
fn push_child(child: &ElemChild, pieces: &mut Vec<EntryPiece>) {
    match child {
        ElemChild::Text(formatted) => {
            let run = text_pieces(&formatted.text);
            let run = match formatted.formatting.font_weight {
                FontWeight::Bold => vec![EntryPiece::Strong(run)],
                FontWeight::Normal | FontWeight::Light => run,
            };
            match formatted.formatting.font_style {
                FontStyle::Italic => pieces.push(EntryPiece::Emphasized(run)),
                FontStyle::Normal => pieces.extend(run),
            }
        }
        // The number a numeric style writes is the list's label, which is the library's.
        ElemChild::Elem(elem) if elem.meta == Some(ElemMeta::CitationNumber) => {}
        ElemChild::Elem(elem) => elem.children.0.iter().for_each(|child| push_child(child, pieces)),
        ElemChild::Link { text, url } => {
            pieces.push(EntryPiece::Link { address: url.clone(), content: text_pieces(&text.text) })
        }
        ElemChild::Markup(text) => pieces.push(EntryPiece::text(text.as_str())),
        ElemChild::Transparent { .. } => {}
    }
}

/// A part of a text of a record ([`text_parts`]): characters shown as they are, or a
/// formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TextPart<'a> {
    /// Characters, shown as they are.
    Literal(&'a str),
    /// The text between two dollar signs, without them, as it is written: an inline
    /// formula, `x^2` of `$x^2$`, as records write a formula in a title.
    Formula(&'a str),
}

/// Returns `text`, a text of a record, split into its literal parts and its formulas: each
/// text between a dollar sign and the next, empty included, is a formula; a last dollar
/// sign with no partner is a character. The rule is the same for every record, and does
/// not depend on whether the document's settings read dollar signs as formula delimiters
/// in its own text.
// See decision rationale at [§flm-dd-dr:host-citations-format]
pub(super) fn text_parts(text: &str) -> Vec<TextPart<'_>> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('$') {
        let Some(length) = rest[open + 1..].find('$') else { break };
        if open > 0 {
            parts.push(TextPart::Literal(&rest[..open]));
        }
        parts.push(TextPart::Formula(&rest[open + 1..open + 1 + length]));
        rest = &rest[open + length + 2..];
    }
    if !rest.is_empty() {
        parts.push(TextPart::Literal(rest));
    }
    parts
}

/// Returns the citation entry parts of `text`, a text run of the citation formatter's
/// result ([`text_parts`]): each literal part its characters as they are, each formula an
/// inline formula of FLM, `\(x^2\)` for `$x^2$`. A formula that does not parse as FLM
/// makes [`ProviderCx::build_entry`](flm_core::stdcore::features::citations::ProviderCx::build_entry)
/// fail, and the citation key then has no citation entry.
pub(super) fn text_pieces(text: &str) -> Vec<EntryPiece> {
    text_parts(text)
        .into_iter()
        .map(|part| match part {
            TextPart::Literal(characters) => EntryPiece::text(characters),
            TextPart::Formula(formula) => {
                let source =
                    FlmSource::new(format!("\\({formula}\\)")).with_origin(FlmOrigin::synthetic("a formula of a citation record"));
                EntryPiece::Source(Arc::new(source))
            }
        })
        .collect()
}

/// Removes the whitespace at the start of `pieces`, which a numeric style leaves where it
/// wrote the number that [`push_child`] leaves out.
fn trim_start(pieces: &mut Vec<EntryPiece>) {
    while let Some(EntryPiece::Text(text)) = pieces.first_mut() {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            pieces.remove(0);
            continue;
        }
        *text = trimmed.to_string();
        break;
    }
}

/// The characters of a DOI written unchanged in the address of its link: the characters
/// that need no percent-encoding in a path, the slash among them; every other character is
/// percent-encoded.
const DOI_KEPT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'/').remove(b'-').remove(b'_').remove(b'.').remove(b'~');

/// Returns the links appended after the citation entry of `record`, each after a space, in
/// this order: a link to `https://doi.org/<DOI>` showing `DOI` (the field `DOI`, or
/// `doi`), a link to `https://arxiv.org/abs/<identifier>` showing the arXiv identifier (the
/// field `arxivid`), and a link to the record's address showing `URL` (the field `URL`);
/// each only when the record has the field, non-empty, and `links` chooses it.
pub(super) fn link_pieces(record: &CslValue, links: Links) -> Vec<EntryPiece> {
    let field = |name: &str| record.get(name).and_then(CslValue::as_str).filter(|value| !value.is_empty());
    let doi = field("DOI").or_else(|| field("doi"));
    let arxiv = field("arxivid");
    let mut pieces = Vec::new();
    let mut link = |address: String, shown: &str| {
        pieces.push(EntryPiece::text(" "));
        pieces.push(EntryPiece::Link { address, content: vec![EntryPiece::text(shown)] });
    };
    if let (Some(doi), true) = (doi, links.doi) {
        link(format!("https://doi.org/{}", utf8_percent_encode(doi, DOI_KEPT)), "DOI");
    }
    if let (Some(arxiv), true) = (arxiv, links.arxiv) {
        link(format!("https://arxiv.org/abs/{arxiv}"), arxiv);
    }
    if let Some(url) = field("URL") {
        let wanted = match links.url {
            UrlLink::Always => true,
            UrlLink::Never => false,
            UrlLink::OnlyIfNoOtherLink => doi.is_none() && arxiv.is_none(),
        };
        if wanted {
            link(url.to_string(), "URL");
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_style_is_read() {
        assert!(Formatter::shipped().is_ok());
    }

    #[test]
    fn a_doi_is_percent_encoded_but_for_its_slashes() {
        let record = serde_json::json!({"DOI": "10.1000/a<b>c d"});
        let links = Links { doi: true, arxiv: true, url: UrlLink::OnlyIfNoOtherLink };
        let pieces = link_pieces(&record, links);
        let Some(EntryPiece::Link { address, .. }) = pieces.get(1) else { panic!("{pieces:?}") };
        assert_eq!(address, "https://doi.org/10.1000/a%3Cb%3Ec%20d");
    }

    #[test]
    fn every_pair_of_dollar_signs_holds_a_formula() {
        use TextPart::{Formula, Literal};
        assert_eq!(text_parts("On $x^{2$ and $$ 5$"), [Literal("On "), Formula("x^{2"), Literal(" and "), Formula(""), Literal(" 5$")]);
        assert_eq!(text_parts("$a$"), [Formula("a")]);
    }

    #[test]
    fn a_date_range_is_refused_and_a_name_alone_is_a_family_name() {
        let range = serde_json::json!({"id": "x", "issued": {"date-parts": [[1997], [2020]]}});
        assert!(item(&range).unwrap_err().contains("`issued`"));
        let named = serde_json::json!({"author": [{"name": "The Collaboration"}]});
        assert_eq!(with_family_names(&named)["author"][0]["family"], "The Collaboration");
    }
}
