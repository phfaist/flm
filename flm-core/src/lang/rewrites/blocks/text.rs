//! Text resolution: the merge of adjacent text nodes, the collapsing of their
//! whitespace, and the sparse source map that records where the resulting text
//! stops matching its source.
//!
//! The layout step merges a run of adjacent **text chunks** (text nodes read in
//! text mode, and the single spaces read for paragraph breaks found inside
//! inline content) by laying them on a **tape** ([`Tape`]): the concatenated
//! text, and for each text chunk the source bytes it comes from. One scan over
//! the tape ([`resolve`]) produces the run's final text and records where each
//! part of it comes from:
//!
//! - every run of ASCII whitespace (space, tab, carriage return, line feed)
//!   becomes one space; the whitespace at the start and at the end of the run
//!   is removed where the caller says so (at the edges of a paragraph, or next
//!   to an element that absorbs the whitespace beside it);
//! - the typographic substitutions that the [options](super::config::BlocksConfig)
//!   enable are applied in the same scan (see [`super::typography`]);
//! - a source-map piece ([`TextSourcePiece`]) is recorded wherever the output
//!   stops being a byte-for-byte copy of one source range: at a collapsed run of
//!   whitespace, a removed edge, a substitution (even one that keeps the
//!   length), and a boundary between text chunks from different sources or
//!   from ranges that do not follow each other. Each maximal stretch that is a
//!   byte-for-byte copy of one source range is one source-map piece.
//!
//! When the whole result is one such stretch, no map is needed: the node's own
//! span, narrowed to the bytes that remain, records the source exactly. The
//! layout step decides this from the source-map pieces
//! ([`Resolved::is_one_source_image`]), and only a text node that needs a map
//! carries a [`TextSourceMap`].

use std::ops::Range;
use std::sync::Arc;

use techy::source::SourceSpan;

use super::config::BlocksConfig;
use super::typography::{is_ascii_space, substitution_at};
use crate::lang::ext::{TextSourceMap, TextSourcePiece};
use crate::lang::origin::{FlmSource, FlmSpan};

/// One text chunk of the tape: a stretch of text with the source bytes it
/// comes from.
struct Chunk {
    /// Offset of the chunk in the tape's text.
    start: usize,
    /// Length of the chunk in the tape's text.
    len: usize,
    /// The source bytes the chunk comes from.
    span: FlmSpan,
    /// Whether the chunk's text is byte-identical to the content of `span`, so
    /// that a sub-range of the text maps linearly onto a sub-range of the span.
    /// `false` for a chunk the pass created (the space read for a paragraph
    /// break) and for text a node already holds as its own, where only the
    /// chunk as a whole has a span.
    exact: bool,
}

/// The merge tape of one text run: the concatenated text, and the source of
/// each text chunk. Adjacent text nodes are always merged, so a tape usually
/// holds several text chunks.
#[derive(Default)]
pub(crate) struct Tape {
    text: String,
    chunks: Vec<Chunk>,
}

impl Tape {
    /// Appends a chunk. `exact` says whether `text` is byte-identical to
    /// `span`'s content. An empty chunk contributes nothing.
    pub(crate) fn push(&mut self, text: &str, span: FlmSpan, exact: bool) {
        if text.is_empty() {
            return;
        }
        self.chunks.push(Chunk {
            start: self.text.len(),
            len: text.len(),
            span,
            exact,
        });
        self.text.push_str(text);
    }

    /// Appends the text of a node that was already resolved once: each piece of
    /// its source map becomes a text chunk, exact where the piece's text is a
    /// byte-for-byte copy of its span and covering the whole span otherwise.
    /// Going through the pieces keeps the node's exact source across a merge
    /// with a neighbor or a further edit, instead of reducing it to the node's
    /// span; one map may cover several sources.
    ///
    /// Returns `false` when the pieces do not cover `text` exactly, ending on
    /// character boundaries, that is, when the map does not match the text.
    /// Nothing is pushed then, and the caller pushes the text as one chunk.
    pub(crate) fn push_mapped(&mut self, text: &str, map: &TextSourceMap) -> bool {
        if map.total_len() as usize != text.len() {
            return false;
        }
        let mut at = 0usize;
        for piece in map.pieces() {
            let end = at + piece.out_len as usize;
            if !text.is_char_boundary(at) || !text.is_char_boundary(end) {
                return false;
            }
            at = end;
        }
        let mut at = 0usize;
        for piece in map.pieces() {
            let end = at + piece.out_len as usize;
            let slice = &text[at..end];
            let exact = slice == piece.span.content();
            self.push(slice, piece.span.clone(), exact);
            at = end;
        }
        true
    }

    /// The concatenated text.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Whether nothing was pushed.
    pub(crate) fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// The index of the text chunk holding the tape offset `at`.
    ///
    /// # Panics
    ///
    /// If `at` lies outside the tape's text. Every caller derives the offset
    /// from the tape's own text, so this is an internal invariant.
    fn chunk_at(&self, at: usize) -> usize {
        self.chunks
            .iter()
            .rposition(|chunk| chunk.start <= at)
            .expect("tape offsets lie inside a chunk")
    }

    /// The source span of the tape range `range`, which must lie inside one
    /// text chunk: the matching sub-range of the chunk's span when the chunk is
    /// exact, the chunk's whole span otherwise.
    fn span_of(&self, range: Range<usize>) -> FlmSpan {
        let chunk = &self.chunks[self.chunk_at(range.start)];
        if chunk.exact {
            let lo = range.start - chunk.start;
            let hi = range.end.min(chunk.start + chunk.len) - chunk.start;
            SourceSpan::new(
                chunk.span.source(),
                chunk.span.start() + lo..chunk.span.start() + hi,
            )
        } else {
            chunk.span.clone()
        }
    }
}

/// How the edges of a run are treated, and what precedes it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Edges {
    /// Drop the run's leading whitespace outright: the run opens a paragraph, or
    /// the element before it absorbs the whitespace beside it.
    pub(crate) drop_leading: bool,
    /// Drop the run's trailing whitespace outright.
    pub(crate) drop_trailing: bool,
    /// Whether a content sibling — anything but a comment — precedes the run.
    /// The position test for straight quotes reads this field, so that
    /// `\(A_j\)'s` closes rather than opens.
    pub(crate) prev_is_content: bool,
}

/// The result of resolving a tape: the final text and its source-map pieces,
/// which cover the text in order.
pub(crate) struct Resolved {
    /// The final text.
    pub(crate) text: String,
    /// The source-map pieces, covering `text` in order.
    pub(crate) pieces: Vec<TextSourcePiece>,
}

impl Resolved {
    /// Whether the result is a byte-for-byte copy of one source range, so that
    /// a span alone records its source and no map is needed.
    pub(crate) fn is_one_source_image(&self) -> bool {
        match self.pieces.as_slice() {
            [piece] => {
                piece.out_len as usize == piece.span.len() && self.text == piece.span.content()
            }
            _ => false,
        }
    }

    /// The span to put on the merged text node: the union of the pieces' spans
    /// when they all lie in one source, otherwise the first piece's span. Where
    /// the pieces come from several sources, the map, not the span, records the
    /// source exactly. `None` when there is no piece.
    pub(crate) fn merged_span(&self) -> Option<FlmSpan> {
        let first = &self.pieces.first()?.span;
        let mut start = first.start();
        let mut end = first.end();
        for piece in &self.pieces[1..] {
            if !piece.span.same_source(first) {
                return Some(first.clone());
            }
            start = start.min(piece.span.start());
            end = end.max(piece.span.end());
        }
        Some(SourceSpan::new(first.source(), start..end))
    }
}

/// A byte-for-byte stretch being built: output and source bytes still
/// identical and contiguous, waiting to be closed into one piece.
struct Open {
    source: Arc<FlmSource>,
    start: usize,
    end: usize,
    out_len: u32,
}

/// The writer of the normalization scan: writes the final text and its
/// source-map pieces.
struct Writer<'a> {
    tape: &'a Tape,
    out: String,
    pieces: Vec<TextSourcePiece>,
    open: Option<Open>,
}

impl<'a> Writer<'a> {
    fn new(tape: &'a Tape) -> Writer<'a> {
        Writer {
            tape,
            out: String::new(),
            pieces: Vec::new(),
            open: None,
        }
    }

    /// Closes the open stretch into a piece.
    fn flush(&mut self) {
        if let Some(open) = self.open.take() {
            self.pieces.push(TextSourcePiece {
                out_len: open.out_len,
                span: SourceSpan::new(&open.source, open.start..open.end),
            });
        }
    }

    /// Writes `text`, a byte-for-byte copy of `span`: extends the open stretch
    /// when `span` continues it in the same source, otherwise starts a new one.
    fn identical(&mut self, text: &str, span: FlmSpan) {
        let extend = matches!(
            &self.open,
            Some(open) if Arc::ptr_eq(&open.source, span.source()) && open.end == span.start()
        );
        if extend {
            let open = self.open.as_mut().expect("checked above");
            open.end = span.end();
            open.out_len += text.len() as u32;
        } else {
            self.flush();
            self.open = Some(Open {
                source: Arc::clone(span.source()),
                start: span.start(),
                end: span.end(),
                out_len: text.len() as u32,
            });
        }
        self.out.push_str(text);
    }

    /// Copies a tape range unchanged, split at text chunk boundaries: an exact
    /// chunk's bytes join the open byte-for-byte stretch; a chunk that is not
    /// exact is a piece of its own.
    fn copy(&mut self, range: Range<usize>) {
        let mut at = range.start;
        while at < range.end {
            let k = self.tape.chunk_at(at);
            let chunk = &self.tape.chunks[k];
            let chunk_end = (chunk.start + chunk.len).min(range.end);
            let piece = at..chunk_end;
            let text = &self.tape.text[piece.clone()];
            if chunk.exact {
                let span = self.tape.span_of(piece);
                self.identical(text, span);
            } else {
                self.flush();
                self.pieces.push(TextSourcePiece {
                    out_len: text.len() as u32,
                    span: chunk.span.clone(),
                });
                self.out.push_str(text);
            }
            at = chunk_end;
        }
    }

    /// Writes `replacement` in place of the tape range: always a piece of its
    /// own, even where the replacement is exactly as long as what it replaces,
    /// because the output stops being a byte-for-byte copy of the source there.
    fn substitute(&mut self, replacement: &str, range: Range<usize>) {
        let span = self.tape.span_of(range);
        self.flush();
        self.pieces.push(TextSourcePiece {
            out_len: replacement.len() as u32,
            span,
        });
        self.out.push_str(replacement);
    }

    /// Removes a tape range from the output: nothing is written, and the open
    /// byte-for-byte stretch ends here.
    fn drop_range(&mut self) {
        self.flush();
    }

    /// The normalization scan over the tape: runs of whitespace and the
    /// typographic substitutions, in one pass.
    fn normalize(&mut self, config: &BlocksConfig, edges: Edges) {
        let text = self.tape.text();
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut i = 0usize;
        // The start of the pending verbatim stretch.
        let mut plain = 0usize;

        while i < len {
            if is_ascii_space(bytes[i]) {
                let mut j = i;
                while j < len && is_ascii_space(bytes[j]) {
                    j += 1;
                }
                if plain < i {
                    self.copy(plain..i);
                }
                let leading = i == 0;
                let trailing = j == len;
                if (leading && edges.drop_leading) || (trailing && edges.drop_trailing) {
                    self.drop_range();
                } else if j - i == 1 && bytes[i] == b' ' {
                    self.copy(i..j);
                } else {
                    self.substitute(" ", i..j);
                }
                i = j;
                plain = j;
                continue;
            }
            if let Some((from, to)) = substitution_at(text, i, config, edges.prev_is_content) {
                if plain < i {
                    self.copy(plain..i);
                }
                self.substitute(to, i..i + from);
                i += from;
                plain = i;
                continue;
            }
            // Advance by one whole character: `i` must stay a character boundary
            // (the substitution table slices the text at it).
            i += text[i..].chars().next().map_or(1, char::len_utf8);
        }
        if plain < len {
            self.copy(plain..len);
        }
    }

    fn finish(mut self) -> Resolved {
        self.flush();
        Resolved {
            text: self.out,
            pieces: self.pieces,
        }
    }
}

/// Resolves a tape under the given options and edge treatment, and returns the
/// final text and the source-map pieces covering it (see the module
/// documentation).
pub(crate) fn resolve(tape: &Tape, config: &BlocksConfig, edges: Edges) -> Resolved {
    let mut writer = Writer::new(tape);
    writer.normalize(config, edges);
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use techy::source::Source;

    fn tape(text: &str) -> (Arc<FlmSource>, Tape) {
        let source: Arc<FlmSource> = Arc::new(Source::new(text));
        let mut tape = Tape::default();
        tape.push(text, SourceSpan::entire(&source), true);
        (source, tape)
    }

    fn pieces(resolved: &Resolved) -> Vec<(u32, String)> {
        resolved
            .pieces
            .iter()
            .map(|p| (p.out_len, p.span.content().to_string()))
            .collect()
    }

    fn keep() -> Edges {
        Edges {
            drop_leading: false,
            drop_trailing: false,
            prev_is_content: false,
        }
    }

    #[test]
    fn an_untouched_run_is_one_source_image() {
        let (_, tape) = tape("abc");
        let resolved = resolve(&tape, &BlocksConfig::no_typography(), keep());
        assert_eq!(resolved.text, "abc");
        assert!(resolved.is_one_source_image());
    }

    #[test]
    fn a_collapsed_whitespace_run_is_a_piece_between_two_identical_stretches() {
        let (_, tape) = tape("a\t\t b");
        let resolved = resolve(&tape, &BlocksConfig::no_typography(), keep());
        assert_eq!(resolved.text, "a b");
        assert_eq!(
            pieces(&resolved),
            [(1, "a".into()), (1, "\t\t ".into()), (1, "b".into())]
        );
    }

    #[test]
    fn a_length_preserving_substitution_is_its_own_piece() {
        let (_, tape) = tape("a---b");
        let resolved = resolve(&tape, &BlocksConfig::default(), keep());
        assert_eq!(resolved.text, "a\u{2014}b");
        assert_eq!(
            pieces(&resolved),
            [(1, "a".into()), (3, "---".into()), (1, "b".into())]
        );
        assert!(!resolved.is_one_source_image());
    }

    #[test]
    fn a_dropped_edge_narrows_the_single_piece_onto_the_surviving_bytes() {
        let (_, tape) = tape("  word ");
        let edges = Edges {
            drop_leading: true,
            drop_trailing: true,
            prev_is_content: false,
        };
        let resolved = resolve(&tape, &BlocksConfig::no_typography(), edges);
        assert_eq!(resolved.text, "word");
        assert!(resolved.is_one_source_image());
        assert_eq!(resolved.merged_span().unwrap().range(), 2..6);
    }

    #[test]
    fn a_source_switch_is_a_piece_boundary_even_when_the_bytes_continue() {
        let a: Arc<FlmSource> = Arc::new(Source::new("ab"));
        let b: Arc<FlmSource> = Arc::new(Source::new("cd"));
        let mut tape = Tape::default();
        tape.push("ab", SourceSpan::entire(&a), true);
        tape.push("cd", SourceSpan::entire(&b), true);
        let resolved = resolve(&tape, &BlocksConfig::no_typography(), keep());
        assert_eq!(resolved.text, "abcd");
        assert_eq!(pieces(&resolved), [(2, "ab".into()), (2, "cd".into())]);
        assert_eq!(resolved.merged_span().unwrap().range(), 0..2);
    }

    #[test]
    fn a_non_exact_chunk_is_a_piece_covering_its_whole_span() {
        let source: Arc<FlmSource> = Arc::new(Source::new("x\n\ny"));
        let mut tape = Tape::default();
        tape.push("x", SourceSpan::new(&source, 0..1), true);
        tape.push(" ", SourceSpan::new(&source, 1..3), false);
        tape.push("y", SourceSpan::new(&source, 3..4), true);
        let resolved = resolve(&tape, &BlocksConfig::no_typography(), keep());
        assert_eq!(resolved.text, "x y");
        assert_eq!(
            pieces(&resolved),
            [(1, "x".into()), (1, "\n\n".into()), (1, "y".into())]
        );
    }
}
