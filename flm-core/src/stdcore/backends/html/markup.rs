//! The markup value: a string of HTML that the HTML sink writes without
//! escaping it.
//!
//! Everything else the HTML backend writes is escaped where it is written
//! ([`super::escape`]), so a document cannot put a `<` into the output. A
//! [`Markup`] value is one of two exceptions; the other is the content of a
//! `<style>` or a `<script>`, which
//! [`HtmlSink::raw_text_element`](super::HtmlSink::raw_text_element) writes.
//! `Markup` is a type of its own so that the exception is easy to find: a
//! reader looking for every place where markup enters an HTML page unescaped
//! looks for the two constructors of [`Markup`].

use std::fmt;

/// A string of HTML that the [sink](super::HtmlSink) writes unchanged, without
/// escaping.
///
/// **What it is for.** The HTML this library writes is escaped at the point of
/// writing: document text goes through [`HtmlSink::text`](super::HtmlSink::text)
/// and an attribute value through
/// [`TagBuilder::attr`](super::TagBuilder::attr), each of which rewrites the
/// characters that would otherwise be read as markup. A `Markup` value carries
/// bytes that are *already* HTML and must not be rewritten — the output of a
/// tool that produces markup, such as a formula typeset as inline
/// `<svg>` — and [`HtmlSink::markup`](super::HtmlSink::markup) writes them
/// through unchanged.
///
/// **What its creator must ensure.** Whoever calls [`trusted`](Self::trusted)
/// accepts the requirement that the string is well-formed HTML that is safe to
/// place in the page: it closes every element it opens, and it holds no
/// script, no event handler attribute and no address a browser would execute,
/// unless the caller put them there intentionally and is responsible for them.
/// The library checks nothing about the string. If the bytes come from a
/// document, from a network resource or from any other input the caller does
/// not control, the caller sanitizes them first; this library's own features
/// never pass document text to this constructor.
///
/// Outside this crate, unescaped bytes reach an HTML page in two ways only:
/// through `trusted`, and through
/// [`HtmlSink::raw_text_element`](super::HtmlSink::raw_text_element), which
/// writes the content of a `<style>` or a `<script>` and refuses content that
/// holds that element's closing tag. A review of what an extension can put into
/// a page is therefore a review of its calls to these two.
///
/// There is no other constructor from a string (the crate-private
/// `from_library` serves the library's own code), and the text inside is
/// private:
///
/// ```compile_fail
/// use flm_core::stdcore::backends::html::Markup;
///
/// let markup = Markup(String::from("<b>bold</b>"));
/// ```
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::{HtmlSink, Markup};
///
/// let mut sink = HtmlSink::new();
/// sink.text("2 < 3");
/// sink.markup(&Markup::trusted("<br>"));
/// assert_eq!(sink.finish(), "2 &lt; 3<br>");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Markup(String);

impl Markup {
    /// Returns a markup value holding `html`, on the caller's assurance that
    /// the bytes are HTML that is safe to place in a page ([the type's
    /// documentation](Markup) states what that assurance covers).
    ///
    /// It is the only way, outside this crate, to put unescaped bytes anywhere
    /// in an HTML page other than inside a `<style>` or a `<script>` (see
    /// [`HtmlSink::raw_text_element`](super::HtmlSink::raw_text_element)).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::html::Markup;
    ///
    /// let markup = Markup::trusted("<svg viewBox=\"0 0 1 1\"></svg>");
    /// assert_eq!(markup.as_str(), "<svg viewBox=\"0 0 1 1\"></svg>");
    /// ```
    pub fn trusted(html: impl Into<String>) -> Markup {
        Markup(html.into())
    }

    /// Returns a markup value holding HTML this library itself wrote or
    /// received from its own configuration: the HTML of a tree or a piece and
    /// the document type line as the page assembly writes them, the content of
    /// a raw text element as
    /// [`HtmlSink::raw_text_element`](super::HtmlSink::raw_text_element) writes
    /// it once its check has passed, and the markup body of a construct node
    /// read back from a stored lowered tree.
    ///
    /// It is crate-private because those bytes are the library's own, a
    /// feature author's, or the host's configuration, never a document's: code
    /// outside this crate uses [`trusted`](Self::trusted) and accepts the
    /// requirement stated there.
    pub(crate) fn from_library(html: impl Into<String>) -> Markup {
        Markup(html.into())
    }

    /// Returns the HTML as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Markup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_markup_value_carries_its_bytes_unchanged() {
        let markup = Markup::trusted("<em>a & b</em>");
        assert_eq!(markup.as_str(), "<em>a & b</em>");
        assert_eq!(markup.to_string(), "<em>a & b</em>");
        assert_eq!(Markup::from_library(String::from("x")).as_str(), "x");
    }
}
