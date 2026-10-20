//! The typed sink ([`LatexSink`]): the only interface through which a LaTeX
//! emitter writes bytes, with a separate method for each kind of thing it
//! writes, so that document text is always escaped and unescaped bytes can come
//! only from sources that are byte-exact LaTeX.
//!
//! An emitter never holds the output string. It writes through a
//! [`LatexSink`], whose methods differ by what they write:
//!
//! - [`text`](LatexSink::text) — document text, through the escaping of
//!   [`super::escape`];
//! - [`raw`](LatexSink::raw) — bytes written exactly as they are, which the
//!   caller can pass only as a [`LatexRaw`] value; a `LatexRaw` can be made
//!   only from the two sources that are byte-exact LaTeX: the text of a raw
//!   construct node ([`LatexRaw::from_raw_payload`]) and the source text of
//!   an opaque run of nodes (mathematics, a verbatim body) as techy writes it
//!   back ([`LatexRaw::from_source`]);
//! - [`escverb`](LatexSink::escverb) — verbatim text in the encoding the
//!   `fvextra` package's `\EscVerb` command reads ([`escverb_encode`]) — and
//!   [`verbatim_block`](LatexSink::verbatim_block), the content of a verbatim
//!   environment, where `%` is an ordinary character and nothing is escaped;
//! - the emitter's own syntax — a control word, a star, `\begin{…}` and
//!   `\end{…}`, braces, brackets, a newline, a blank line, a LaTeX comment — each
//!   through a method that writes that syntax and nothing else.
//!
//! # LaTeX comments are written by one method, and always end their line
//!
//! [`comment`](LatexSink::comment) is the only way a LaTeX comment reaches the
//! recomposed LaTeX, and it writes the newline that ends the comment itself. A
//! LaTeX comment runs to the end of its line, so a comment whose line did not end
//! would swallow anything an emitter wrote next; because the sink writes the
//! newline, no caller can forget it. A caller that holds the whitespace which
//! followed the source comment writes it after the call, minus the one
//! newline the sink has already written.
//!
//! The same rule applies to [`raw`](LatexSink::raw): raw LaTeX text that a
//! lowering rule assembled (the payload of a raw construct node) must not hold
//! a `%` that starts a LaTeX comment, and the sink refuses such a text instead of
//! writing it ([`RawCommentRefused`]). Every `%` the library means literally
//! is written by the escaping as `\%`, and every `%` an author wrote starts a
//! source comment, a comment node that the emitter writes through
//! [`comment`](LatexSink::comment); a
//! `%` in rule-built raw text is therefore a mistake in the rule that built
//! it.
//!
//! # Nothing reads the output back
//!
//! The sink offers no access to the text written before
//! [`finish`](LatexSink::finish) (only to its length,
//! [`position`](LatexSink::position)), so no decision an emitter takes can
//! depend on the bytes already produced, and no escaping can run over output.
//! Double escaping, and escaping inside a [verbatim
//! region](crate::guide::concepts_overview#verbatim-regions), therefore cannot
//! be expressed at all. What the sink learned while writing (the characters
//! and packages the style file must declare, the characters no spelling is
//! known for) comes back with the text in the [`SinkOutput`] that `finish`
//! returns.
//!
//! The sink keeps no notion of its position in a line; the emitter preserves
//! the author's whitespace and writes a newline where it means one.
//!
//! The free functions of this module and of [`super::escape`]
//! ([`escverb_encode`], [`super::escape::escape_into`]) exist so that a test
//! or a host can inspect what the sink writes; an emitter uses the sink.

use std::fmt;

use techy::core::node::NodeRef;

use crate::lang::Flm;
use crate::stdcore::backends::pres;

use super::construct::LatexRawPayload;
use super::escape::{Declared, EscapeConfig, escape_into};

/// LaTeX to be written byte for byte by [`LatexSink::raw`]. Only two things
/// can be turned into one, the text of a raw construct node and the source
/// text of an opaque run of nodes, so that a `LatexRaw` value always shows
/// where unescaped bytes came from. The guarantee is precisely this: bytes
/// that reach the output unescaped come only from a raw construct node's
/// payload or from techy's recomposition of the document's source, never from
/// a string an emitter assembled itself.
///
/// ```compile_fail
/// // There is no constructor from a string.
/// use flm_core::stdcore::backends::latex::sink::LatexRaw;
/// let raw = LatexRaw(String::from("\\danger"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatexRaw {
    text: String,
    copied: bool,
}

impl LatexRaw {
    /// The text of a `flm.pres.latex.raw` node's payload: LaTeX a lowering
    /// rule assembled. [`LatexSink::raw`] refuses one holding a `%` that
    /// starts a comment.
    pub fn from_raw_payload(payload: &LatexRawPayload) -> LatexRaw {
        LatexRaw { text: payload.text.clone(), copied: false }
    }

    /// The source spelling of `nodes`, byte for byte, as
    /// [`pres::recompose_source`] gives it: the content of a mathematics
    /// region or of a verbatim body. Call it on a worker thread, as
    /// `recompose_source` asks.
    ///
    /// These bytes are the author's own, copied rather than assembled, so a
    /// `%` among them is the author's comment with the author's newline
    /// after it: [`LatexSink::raw`] writes them unchanged.
    pub fn from_source(nodes: &[NodeRef<'_, Flm>]) -> LatexRaw {
        LatexRaw { text: pres::recompose_source(nodes), copied: true }
    }

    /// The bytes as text.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// The error [`LatexSink::raw`] returns for raw LaTeX text, assembled by a
/// lowering rule, that holds a `%` starting a LaTeX comment, which would
/// comment out anything the emitter wrote after it. See [the module
/// documentation](self).
///
/// This is an internal structure error, a mistake in the lowering rule that
/// built the text and never anything an author can write. The
/// `latex-recomposed` emitter reports it as the Error
/// `flm.latex-recomposed.stage-failed` and writes nothing for the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCommentRefused {
    /// The text, as the rule built it.
    pub text: String,
    /// The byte offset of the offending `%` in [`text`](Self::text).
    pub at: usize,
}

impl fmt::Display for RawCommentRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "raw LaTeX text built by a lowering rule must not contain a LaTeX comment, but \
             the `%` at byte {} of {:?} starts one",
            self.at, self.text
        )
    }
}

/// The byte offset of the first `%` of `text` that starts a LaTeX comment, if
/// any: a `%` whose run of immediately preceding backslashes has even length.
///
/// An odd run means the last backslash escapes the `%`, which is then the
/// character itself (`\%`); an even run means every backslash is itself
/// escaped (`\\%` is a line break followed by a comment), so the `%` starts a
/// comment.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::sink::comment_percent;
///
/// assert_eq!(comment_percent("100\\% sure"), None);   // an escaped per cent
/// assert_eq!(comment_percent("a %% note"), Some(2));  // a comment
/// assert_eq!(comment_percent("\\\\% note"), Some(2));  // a line break, then a comment
/// assert_eq!(comment_percent("\\\\\\% sure"), None);   // a line break, then an escaped per cent
/// ```
pub fn comment_percent(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'%' {
            continue;
        }
        let mut backslashes = 0usize;
        let mut before = at;
        while before > 0 && bytes[before - 1] == b'\\' {
            backslashes += 1;
            before -= 1;
        }
        if backslashes % 2 == 0 {
            return Some(at);
        }
    }
    None
}

/// The only printable ASCII character that is not a letter or a digit and is
/// nonetheless written unescaped, `@`: `fvextra` rejects `\@` (see
/// [`escverb_encode`]).
pub const ESCVERB_UNESCAPED: char = '@';

/// Appends `text` to `out` in the encoding `\EscVerb` reads: every printable
/// ASCII character that is not a letter or a digit — the space included,
/// [`ESCVERB_UNESCAPED`] excepted — is preceded by a backslash, and every
/// other character (letters, digits, control characters, non-ASCII
/// characters) is written as it is. See [`escverb_encode`].
pub fn escverb_encode_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        if ch.is_ascii()
            && !ch.is_ascii_alphanumeric()
            && !ch.is_ascii_control()
            && ch != ESCVERB_UNESCAPED
        {
            out.push('\\');
        }
        out.push(ch);
    }
}

/// The encoding `\EscVerb` reads, as a fresh string.
///
/// `\EscVerb{…}` (the `fvextra` package) typesets its braced argument
/// verbatim while reading it as an ordinary argument, so that it works
/// inside a moving argument such as a section title; inside it, a backslash
/// escapes the character after it. Every printable ASCII character that is
/// not a letter or a digit is escaped here, the space included, **except
/// `@`** ([`ESCVERB_UNESCAPED`]): that covers the characters that must be
/// escaped for the argument to be read (`\`, `{`, `}`, `%`) and leaves
/// nothing to a case-by-case rule. Non-ASCII characters are written as they
/// are: a backslash before one would not be a single escape. Newlines and
/// tabs are written as they are.
///
/// `@` is the exception because `fvextra` refuses `\@` and stops the
/// compilation with "Invalid backslash escape"; written plain, `@` is
/// ordinary verbatim content. Every other escape in the range was compiled
/// one by one against `fvextra` 1.6.1 and accepted, and `%`, `\`, `{` and
/// `}` were the only ones refused unescaped. The Python implementation of FLM
/// escapes `@` too, and its output does not compile.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::sink::escverb_encode;
///
/// assert_eq!(escverb_encode("a{b}\\c d_e"), "a\\{b\\}\\\\c\\ d\\_e");
/// assert_eq!(escverb_encode("user@example.com"), "user@example\\.com");
/// assert_eq!(escverb_encode("café"), "café");
/// ```
pub fn escverb_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    escverb_encode_into(&mut out, text);
    out
}

/// What a sink produced: the text and what it learned while writing it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SinkOutput {
    /// The LaTeX written.
    pub text: String,
    /// What the style file must declare because of the text written through
    /// [`LatexSink::text`].
    pub declared: Declared,
    /// The characters no spelling is known for, each once, in order of first
    /// occurrence; the `latex-recomposed` emitter reports each once per
    /// lowered tree as a Warning
    /// ([`UnknownCharacter`](crate::stdcore::backends::latex_recomposed::UnknownCharacter)).
    pub unknown: Vec<char>,
}

/// The typed sink: it accumulates the LaTeX an emitter writes, escapes
/// document text under one [`EscapeConfig`], and returns the text together
/// with what it learned in a [`SinkOutput`] ([`finish`](Self::finish)). See
/// [the module documentation](self) for the kinds of writing it offers and the
/// guarantees they give.
#[derive(Debug, Clone)]
pub struct LatexSink {
    out: String,
    config: EscapeConfig,
    declared: Declared,
    unknown: Vec<char>,
}

impl LatexSink {
    /// An empty sink escaping text under `config`.
    pub fn new(config: EscapeConfig) -> LatexSink {
        LatexSink { out: String::new(), config, declared: Declared::new(), unknown: Vec::new() }
    }

    /// The escaping configuration the sink writes text under.
    pub fn config(&self) -> &EscapeConfig {
        &self.config
    }

    /// How many bytes of LaTeX the sink holds so far: the position at which
    /// the next thing written starts.
    pub fn position(&self) -> usize {
        self.out.len()
    }

    /// Writes document text, escaped ([`super::escape::escape_into`]).
    pub fn text(&mut self, text: &str) {
        let unknown = escape_into(&mut self.out, text, &self.config, &mut self.declared);
        for ch in unknown {
            if !self.unknown.contains(&ch) {
                self.unknown.push(ch);
            }
        }
    }

    /// Writes bytes exactly as they are.
    ///
    /// # Errors
    ///
    /// [`RawCommentRefused`] when `raw` is text a lowering rule assembled
    /// ([`LatexRaw::from_raw_payload`]) and it holds a `%` that starts a
    /// LaTeX comment ([`comment_percent`]); nothing is written then. Bytes copied
    /// from the document's own source ([`LatexRaw::from_source`]) are written
    /// unchanged — see [the module documentation](self).
    pub fn raw(&mut self, raw: &LatexRaw) -> Result<(), RawCommentRefused> {
        if !raw.copied {
            if let Some(at) = comment_percent(raw.as_str()) {
                return Err(RawCommentRefused { text: raw.as_str().to_string(), at });
            }
        }
        self.out.push_str(raw.as_str());
        Ok(())
    }

    /// Writes the content of a verbatim environment, byte for byte.
    ///
    /// This is verbatim content, not LaTeX: inside `\begin{Verbatim}` …
    /// `\end{Verbatim}` every character stands for itself, `%` included, so
    /// nothing here is escaped and nothing is refused. Inline verbatim text
    /// goes through [`escverb`](LatexSink::escverb) instead.
    pub fn verbatim_block(&mut self, text: &str) {
        self.out.push_str(text);
    }

    /// Writes verbatim text in the encoding `\EscVerb` reads
    /// ([`escverb_encode`]).
    pub fn escverb(&mut self, text: &str) {
        escverb_encode_into(&mut self.out, text);
    }

    /// Writes `\` followed by `name`: a control word. The name is written as
    /// given; a construct's name is letters, and nothing follows it here —
    /// the caller writes the arguments, or a space where the author's text
    /// needs separating from the name.
    pub fn control_word(&mut self, name: &str) {
        self.out.push('\\');
        self.out.push_str(name);
    }

    /// Writes `*`, the star of a starred command or environment.
    pub fn star(&mut self) {
        self.out.push('*');
    }

    /// Writes `\begin{name}`.
    pub fn begin(&mut self, name: &str) {
        self.out.push_str("\\begin{");
        self.out.push_str(name);
        self.out.push('}');
    }

    /// Writes `\end{name}`.
    pub fn end(&mut self, name: &str) {
        self.out.push_str("\\end{");
        self.out.push_str(name);
        self.out.push('}');
    }

    /// Writes `{`.
    pub fn open_brace(&mut self) {
        self.out.push('{');
    }

    /// Writes `}`.
    pub fn close_brace(&mut self) {
        self.out.push('}');
    }

    /// Writes `[`.
    pub fn open_bracket(&mut self) {
        self.out.push('[');
    }

    /// Writes `]`.
    pub fn close_bracket(&mut self) {
        self.out.push(']');
    }

    /// Writes one newline.
    pub fn newline(&mut self) {
        self.out.push('\n');
    }

    /// Writes a source comment as a LaTeX comment, and the newline that ends
    /// it: `prefix` — what starts a source comment under the document's
    /// parsing configuration, `%%` by default — followed by `text`, the
    /// comment's content as written, then
    /// one newline. Where `prefix` does not itself begin with `%`, a `%` is
    /// written first, so that what follows is a LaTeX comment regardless of
    /// the document's comment syntax: the prefix is a convention of the
    /// document's source, and the output must compile.
    ///
    /// This is the only way a LaTeX comment reaches a recomposed output, and the newline
    /// is written here rather than by the caller so that a comment can never
    /// run on into what follows it (see [the module documentation](self)).
    /// A caller holding the whitespace that followed the comment in the
    /// source writes it after this call, minus the one newline already
    /// written — a comment runs to the end of its line, so that whitespace
    /// begins with a newline except at the end of the input, where there is
    /// none to leave out.
    ///
    /// The text is written as it is (a comment is not escaped) and must hold
    /// no newline, which would end the comment; a comment's text as the
    /// parser produces it never does.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::latex::escape::EscapeConfig;
    /// use flm_core::stdcore::backends::latex::sink::LatexSink;
    ///
    /// let mut sink = LatexSink::new(EscapeConfig::default());
    /// sink.comment("%%", " a note");
    /// sink.comment("//", " another");
    /// assert_eq!(sink.finish().text, "%% a note\n%// another\n");
    /// ```
    pub fn comment(&mut self, prefix: &str, text: &str) {
        if !prefix.starts_with('%') {
            self.out.push('%');
        }
        self.out.push_str(prefix);
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// The text written and what the sink learned while writing it.
    pub fn finish(self) -> SinkOutput {
        SinkOutput { text: self.out, declared: self.declared, unknown: self.unknown }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::escape::UnicodeMode;

    #[test]
    fn every_entry_point_writes_its_own_syntax_and_nothing_else() {
        let mut sink = LatexSink::new(EscapeConfig::new(UnicodeMode::Replace));
        sink.control_word("section");
        sink.star();
        sink.open_bracket();
        sink.text("short");
        sink.close_bracket();
        sink.open_brace();
        sink.text("100% & more");
        sink.close_brace();
        sink.newline();
        sink.begin("center");
        sink.raw(&LatexRaw::from_raw_payload(&LatexRawPayload::new("\\\\[1ex]"))).expect("no comment");
        sink.end("center");
        sink.newline();
        sink.escverb("a b");
        let output = sink.finish();
        assert_eq!(
            output.text,
            "\\section*[short]{100\\% \\& more}\n\\begin{center}\\\\[1ex]\\end{center}\na\\ b"
        );
        assert!(output.unknown.is_empty());
    }

    #[test]
    fn the_comment_gains_a_leading_percent_only_when_the_prefix_lacks_one() {
        let mut sink = LatexSink::new(EscapeConfig::default());
        sink.comment("%%", " a note");
        sink.comment("//", " another");
        sink.comment("%", " third");
        assert_eq!(sink.finish().text, "%% a note\n%// another\n% third\n");
    }

    #[test]
    fn a_comment_always_ends_its_own_line_whatever_follows_it() {
        let mut sink = LatexSink::new(EscapeConfig::default());
        sink.comment("%%", " a note");
        sink.text("next");
        assert_eq!(sink.finish().text, "%% a note\nnext");
    }

    #[test]
    fn a_raw_piece_a_rule_assembled_may_not_hold_a_comment() {
        let mut sink = LatexSink::new(EscapeConfig::default());
        // An escaped per cent is the character, and is written.
        let ok = LatexRaw::from_raw_payload(&LatexRawPayload::new("100\\% sure"));
        assert_eq!(sink.raw(&ok), Ok(()));
        // A bare one starts a comment, and is refused; nothing is written.
        let bad = LatexRaw::from_raw_payload(&LatexRawPayload::new("a %% note"));
        assert_eq!(
            sink.raw(&bad),
            Err(RawCommentRefused { text: String::from("a %% note"), at: 2 })
        );
        // `\\` is a line break, so the `%` after it starts a comment.
        let linebreak = LatexRaw::from_raw_payload(&LatexRawPayload::new("\\\\% note"));
        assert!(sink.raw(&linebreak).is_err());
        assert_eq!(sink.finish().text, "100\\% sure");
    }

    #[test]
    fn source_copied_from_the_document_is_written_as_it_stands() {
        // `from_source` bytes are the author's own, comment and newline
        // together; only assembled pieces are policed.
        let mut sink = LatexSink::new(EscapeConfig::default());
        sink.verbatim_block("100% of the time\n");
        assert_eq!(sink.finish().text, "100% of the time\n");
    }

    #[test]
    fn the_unknown_characters_and_the_declarations_come_back_with_the_text() {
        let mut sink = LatexSink::new(EscapeConfig::new(UnicodeMode::Declare));
        sink.text("é ธ");
        sink.text("ธ α");
        let output = sink.finish();
        assert_eq!(output.text, "é ธธ α");
        assert_eq!(output.unknown, ['ธ']);
        assert_eq!(output.declared.chars().len(), 2);
    }

    #[test]
    fn the_escverb_encoding_escapes_printable_ascii_punctuation_only() {
        assert_eq!(escverb_encode("x = {1, 2}; 100% #a\\b"), "x\\ \\=\\ \\{1\\,\\ 2\\}\\;\\ 100\\%\\ \\#a\\\\b");
        assert_eq!(escverb_encode("Zé9\n\t"), "Zé9\n\t");
        assert_eq!(escverb_encode("~^_$&"), "\\~\\^\\_\\$\\&");
    }

    #[test]
    fn the_escverb_encoding_leaves_the_one_character_fvextra_refuses_escaped() {
        // `\@` stops `fvextra` with "Invalid backslash escape"; `@` plain is
        // ordinary verbatim content.
        assert_eq!(escverb_encode("a@b"), "a@b");
        assert_eq!(escverb_encode("@decorator(x)"), "@decorator\\(x\\)");
        // Every other printable ASCII non-alphanumeric is escaped.
        for code in 0x20u8..=0x7E {
            let ch = char::from(code);
            let escaped = escverb_encode(&ch.to_string());
            if ch.is_ascii_alphanumeric() || ch == ESCVERB_UNESCAPED {
                assert_eq!(escaped, ch.to_string(), "{ch:?}");
            } else {
                assert_eq!(escaped, format!("\\{ch}"), "{ch:?}");
            }
        }
    }

    #[test]
    fn a_raw_value_carries_its_bytes_unchanged() {
        let raw = LatexRaw::from_raw_payload(&LatexRawPayload::new("\n\n"));
        assert_eq!(raw.as_str(), "\n\n");
        let mut sink = LatexSink::new(EscapeConfig::default());
        sink.raw(&raw).expect("no comment");
        assert_eq!(sink.finish().text, "\n\n");
    }
}
