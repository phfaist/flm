//! The within-joiner, which writes a counter's ancestors' numbers before the
//! counter's own number, and the parts ([`Part`]) in which it writes them.
//!
//! A counter numbered within another shows something of that other counter
//! before its own number: a subsection in section 1 is written `1.2`. The
//! **within-joiner** decides what is shown. Four forms are built in
//! ([`WithinJoinerSpec`](super::WithinJoinerSpec)): a joining text, a within
//! template, nothing at all, and a within-joiner that a feature registered by
//! name. The code that writes a number treats
//! all four alike: given a chain of ancestors, a within-joiner returns a list
//! of [`Part`]s.
//!
//! A within-joiner exists in two forms, as a declaration and as a compiled
//! value. [`WithinJoiner`] is the declaration of a registered within-joiner: a
//! name, and a method that compiles it against the built language.
//! [`CompiledWithinJoiner`] is the compiled form of any of the four: it
//! returns the parts, and it says which counters of a chain it writes, with
//! which the registry build and the start of every run check the hierarchy.
//!
//! **The parts always come first**: they are written in order before the
//! point's own number, which is never inside a part. Each part lists the
//! ancestors it shows, so that a caller writing several numbers at once can
//! find which parts two numbers share. A joining text gives one part per
//! ancestor (`1.`, then `2.`, then the counter's own number `3`); a within
//! template gives one part per `#{…}`, holding the text after it; the form
//! that writes nothing of the ancestors gives no part; a registered
//! within-joiner gives the parts its code gives, possibly a single part for
//! everything.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeTree;

use crate::lang::rewrites::userdefs::template::{CompiledTemplate, TemplateLanguage};
use crate::lang::{BuiltLang, Flm};
use crate::model::names::Name;

use super::decl::{Parents, Within, WithinJoinerSpec, PARENT_PLACEHOLDER};
use super::formats::NumberFormat;
use super::style::{
    assemble_pieces, number_tree, parse_text, Ancestor, CounterStyle, FormatError, Piece, StyleError,
};
use super::template::{
    compile_text, instantiate_template, occurrences_of, parse_keys, placeholder_starts,
    within_vocabulary, AncestorKeys, Binding, Bindings,
};
use super::value::CounterValue;

// --- what a joiner writes ---------------------------------------------------------------------

/// A part: one unit of the text that a within-joiner writes before a point's
/// own number, with the ancestors it shows and its content.
///
/// The parts of one number are written in order, and no part holds anything
/// of the point's own number. A part that shows no ancestor is text only; a
/// part that shows several ancestors is one in which the ancestors cannot be
/// separated, such as a within template's `#{flm.headings.subsection}`, which
/// writes the section's number and the subsection's together.
#[derive(Clone, Debug)]
pub struct Part {
    /// The ancestors this part shows, outermost first, each with its value at
    /// the point.
    pub stands_for: Vec<(Name, CounterValue)>,
    /// The part's content.
    pub content: Arc<NodeTree<Flm>>,
}

impl Part {
    /// Returns a part that shows the ancestors `stands_for`, with the content
    /// `content`.
    pub fn new(
        stands_for: Vec<(Name, CounterValue)>,
        content: Arc<NodeTree<Flm>>,
    ) -> Part {
        Part { stands_for, content }
    }
}

// --- the two traits ---------------------------------------------------------------------------

/// A within-joiner registered by name, as a feature declares it: its name,
/// and how it compiles itself once the language is built.
///
/// A feature contributes one, wrapped in a [`WithinJoinerDecl`], to the
/// service slot [`WITHIN_JOINERS_SLOT`](super::WITHIN_JOINERS_SLOT), and a
/// counter asks for it by name with
/// [`Within::joiner_named`](super::Within::joiner_named). Two within-joiners of
/// one name fail the registry build.
pub trait WithinJoiner: Send + Sync {
    /// Returns the name by which counters ask for this within-joiner. It is
    /// dotted and owned by the feature that declares it, such as
    /// `example.numbering.compact`, as a counter style's name is.
    fn name(&self) -> &Name;

    /// Returns this within-joiner compiled with `cx`, which holds the built
    /// language; every text it holds is parsed here.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a text of its own that is not valid FLM, or of a
    /// template it could not compile.
    fn compile(&self, cx: &WithinCx<'_>) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError>;
}

/// A within-joiner as it is contributed to the service slot
/// [`WITHIN_JOINERS_SLOT`](super::WITHIN_JOINERS_SLOT): a value that holds the
/// within-joiner.
///
/// A service slot hands out its values as borrowed references, and the
/// numbering feature uses a within-joiner long after the registry build has
/// released them, so the service slot holds this value, of which the
/// numbering feature keeps a copy.
#[derive(Clone)]
pub struct WithinJoinerDecl {
    joiner: Arc<dyn WithinJoiner>,
}

impl WithinJoinerDecl {
    /// Returns the declaration of `joiner`.
    pub fn new(joiner: Arc<dyn WithinJoiner>) -> WithinJoinerDecl {
        WithinJoinerDecl { joiner }
    }

    /// Returns the within-joiner it holds.
    pub fn joiner(&self) -> &Arc<dyn WithinJoiner> {
        &self.joiner
    }
}

impl fmt::Debug for WithinJoinerDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WithinJoinerDecl")
            .field("name", &self.joiner.name().as_str())
            .finish()
    }
}

/// A compiled within-joiner: what writes the ancestors' numbers.
///
/// Its two methods are called at different times.
/// [`counters_written`](CompiledWithinJoiner::counters_written) is called when
/// the registry is built and at the start of every run, once per chain of
/// counters the counter may be numbered within, to check that every counter a
/// number would show is in the chain it is shown with.
/// [`parts`](CompiledWithinJoiner::parts) is called wherever a number is
/// shown.
pub trait CompiledWithinJoiner: Send + Sync + fmt::Debug {
    /// Returns the counters this within-joiner writes, given `chain`: the
    /// counters the point is numbered within, outermost first, the last of
    /// them being the parent that applies.
    ///
    /// The method is called once per chain the counter may be numbered within,
    /// before anything is counted, and every counter it returns must be in the
    /// chain it was given. A declared within-joiner that fails this check
    /// fails the registry build. At the start of a run, a within-joiner given
    /// by the document's numbering options that fails it is refused, and the
    /// counter keeps its declared place in the hierarchy; a declared
    /// within-joiner that fails it because the options moved the counter's
    /// ancestors makes the counter unnumbered for that run. `#{_parent}` and
    /// the joining text return the last counter of the chain, so they pass for
    /// every chain; a within template that refers to a counter by name passes
    /// where that counter is an ancestor under every chain.
    ///
    /// Only the counters this within-joiner writes *itself* are returned, not
    /// the ones an ancestor's own within-joiner writes: each counter of the
    /// chain is checked with its own within-joiner in turn.
    fn counters_written(&self, chain: &[Name]) -> Vec<Name>;

    /// Returns the parts written before the point's own number.
    ///
    /// `ancestors` is the whole chain, outermost first, so the ancestors of
    /// `ancestors[i]` are `ancestors[..i]`; `cx` builds the content.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownAncestor`] when the within-joiner refers to a
    /// counter that is not in the chain, and [`FormatError::NestingTooDeep`]
    /// or [`FormatError::Failed`] when the content could not be built.
    fn parts(&self, ancestors: &[Ancestor], cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError>;
}

// --- compiling ---------------------------------------------------------------------------------

/// The context against which a within-joiner is compiled: the built language,
/// the template language shared by every template of the numbering feature,
/// the counters a within template may refer to, and the within-joiners
/// registered by name.
pub struct WithinCx<'a> {
    lang: &'a BuiltLang,
    language: &'a TemplateLanguage,
    counters: &'a [Name],
    joiners: &'a BTreeMap<Name, Arc<dyn WithinJoiner>>,
}

impl<'a> WithinCx<'a> {
    /// Returns the context over `lang`, the shared template language, the
    /// counters a within template may refer to and the registered
    /// within-joiners.
    pub(super) fn new(
        lang: &'a BuiltLang,
        language: &'a TemplateLanguage,
        counters: &'a [Name],
        joiners: &'a BTreeMap<Name, Arc<dyn WithinJoiner>>,
    ) -> WithinCx<'a> {
        WithinCx { lang, language, counters, joiners }
    }

    /// Returns the built language.
    pub fn lang(&self) -> &BuiltLang {
        self.lang
    }

    /// Returns `text` parsed as FLM, as every text of a counter style is
    /// parsed. Warnings of the parse are dropped.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a text that is not valid FLM.
    pub fn parse(&self, text: &str) -> Result<Arc<NodeTree<Flm>>, StyleError> {
        let mut warnings = Vec::new();
        parse_text(text, self.lang, &mut warnings)
    }

    /// Returns `text` compiled as a within template: `#{counter}` writes that
    /// counter's number and `#{_parent}` the parent that applies, with the
    /// keys that [`Within::template`](super::Within::template) describes.
    /// `what` describes the owner of the text in a diagnostic.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a text that is not valid FLM, of a `#{…}` that is
    /// not a declared counter, and of an unknown key in brackets.
    pub fn compile_template(
        &self,
        text: &str,
        what: &str,
    ) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError> {
        let vocabulary = within_vocabulary(self.counters.iter());
        let whole = compile_text(text, what, &vocabulary, self.lang, self.language)?;
        let mut occurrences = Vec::new();
        for (name, keys) in occurrences_of(&whole) {
            let parsed = parse_keys(&keys).map_err(|key| StyleError::UnknownKey {
                text: text.to_string(),
                key,
            })?;
            occurrences.push(Occurrence {
                counter: Name::new(name.as_str()),
                keys: parsed,
                written: keys,
            });
        }
        let shape = self.split(text, &whole, &vocabulary, what, &occurrences);
        Ok(Arc::new(Templated { text: text.to_string(), whole, occurrences, shape }))
    }

    /// Returns the template cut into one part per `#{…}`, or `None` when it
    /// cannot be cut: when a piece of it between two substitutions is not
    /// valid FLM on its own, as happens when markup surrounds a substitution.
    /// The whole template is then one part.
    fn split(
        &self,
        text: &str,
        whole: &CompiledTemplate,
        vocabulary: &crate::lang::rewrites::userdefs::template::PlaceholderVocabulary,
        what: &str,
        occurrences: &[Occurrence],
    ) -> Option<Vec<CompiledTemplate>> {
        let starts = placeholder_starts(whole);
        if starts.len() != occurrences.len() || starts.is_empty() {
            return None;
        }
        let mut cuts: Vec<usize> = Vec::with_capacity(starts.len() + 1);
        cuts.push(0);
        cuts.extend(starts.iter().skip(1).copied());
        cuts.push(text.len());
        let mut parts = Vec::with_capacity(starts.len());
        for window in cuts.windows(2) {
            let piece = text.get(window[0]..window[1])?;
            let compiled = compile_text(piece, what, vocabulary, self.lang, self.language).ok()?;
            if occurrences_of(&compiled).len() != 1 {
                return None;
            }
            parts.push(compiled);
        }
        Some(parts)
    }
}

impl Within {
    /// Returns this declaration's within-joiner compiled against the built
    /// language, ready to write parts.
    ///
    /// `counters` are the counters a within template may refer to, and
    /// `joiners` the within-joiners registered by name. The numbering feature
    /// compiles every declaration itself when the registry is built; a caller
    /// that writes numbers of its own, outside the feature, compiles one with
    /// this method. Each call builds a template language of its own when the
    /// form needs one, whereas the feature builds one for all the
    /// declarations it compiles.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a text that is not valid FLM, of a template that
    /// does not compile, and [`StyleError::UnknownJoiner`] for a name that no
    /// within-joiner in `joiners` has.
    pub fn compile(
        &self,
        lang: &BuiltLang,
        counters: &[Name],
        joiners: &[Arc<dyn WithinJoiner>],
    ) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError> {
        // The forms that hold no template are compiled without building a
        // template language for them.
        match &self.joiner {
            WithinJoinerSpec::Joined(text) => {
                let mut warnings = Vec::new();
                return Ok(Arc::new(Joined {
                    described: described(&self.parents, text),
                    joiner: parse_text(text, lang, &mut warnings)?,
                }));
            }
            WithinJoinerSpec::Bare => return Ok(Arc::new(Bare)),
            _ => {}
        }
        let language = TemplateLanguage::new(lang).map_err(|errors| StyleError::ParseFailed {
            text: joiner_text(self),
            detail: errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        })?;
        let registered: BTreeMap<Name, Arc<dyn WithinJoiner>> =
            joiners.iter().map(|joiner| (joiner.name().clone(), Arc::clone(joiner))).collect();
        let cx = WithinCx::new(lang, &language, counters, &registered);
        compile_within(self, &format!("the counter written within {}", self.parents.describe()), &cx)
    }
}

/// Returns the within-joiner of `within`, compiled with `cx` and ready to
/// write parts.
///
/// `what` describes the counter in a diagnostic.
///
/// # Errors
///
/// The [`StyleError`] of a text that is not valid FLM, of a template that does
/// not compile, and [`StyleError::UnknownJoiner`] for a name that no feature
/// registered.
pub(super) fn compile_within(
    within: &Within,
    what: &str,
    cx: &WithinCx<'_>,
) -> Result<Arc<dyn CompiledWithinJoiner>, StyleError> {
    match &within.joiner {
        WithinJoinerSpec::Joined(text) => Ok(Arc::new(Joined {
            described: described(&within.parents, text),
            joiner: cx.parse(text)?,
        })),
        WithinJoinerSpec::Bare => Ok(Arc::new(Bare)),
        WithinJoinerSpec::Template(text) => cx.compile_template(text, what),
        WithinJoinerSpec::Named(name) => {
            let Some(joiner) = cx.joiners.get(name) else {
                return Err(StyleError::UnknownJoiner { name: name.clone() });
            };
            joiner.compile(cx)
        }
    }
}

// --- what content is made with -----------------------------------------------------------------

/// The context with which a within-joiner builds content: an ancestor's
/// number, the parts written before an ancestor, and text of its own.
///
/// Everything a reader sees is content, not characters, and a within-joiner
/// builds content only through this context.
pub struct JoinCx<'a> {
    style: &'a CounterStyle,
}

impl<'a> JoinCx<'a> {
    /// Returns the context over `style`, the counter style in which the whole
    /// number is written. A within-joiner's own texts are attributed to this
    /// counter style; an ancestor's number is written in the ancestor's own
    /// counter style regardless.
    pub(super) fn new(style: &'a CounterStyle) -> JoinCx<'a> {
        JoinCx { style }
    }

    /// Returns `ancestor`'s own number, written in its own counter style: the
    /// number alone, with no word, no delimiters and nothing of its own
    /// ancestors.
    ///
    /// # Errors
    ///
    /// [`FormatError`] when the content could not be built, and
    /// [`FormatError::UndeclaredSubLevel`] when the value carries a sub-number
    /// for which the counter style declares no format.
    pub fn number_of(&self, ancestor: &Ancestor) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        number_tree(&ancestor.style, &ancestor.value, None)
    }

    /// Returns `ancestor`'s own number written in `format` instead of the
    /// format of its own counter style, its sub-numbers in that format too.
    ///
    /// # Errors
    ///
    /// The same as [`number_of`](JoinCx::number_of).
    pub fn number_of_in(
        &self,
        ancestor: &Ancestor,
        format: NumberFormat,
    ) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        number_tree(&ancestor.style, &ancestor.value, Some(format))
    }

    /// Returns the parts written before `ancestors[index]`: what that
    /// ancestor's own within-joiner writes of the ancestors before it. An
    /// ancestor numbered within nothing has no part.
    ///
    /// Through this recursion, a within-joiner that refers to the counter
    /// immediately above writes the whole chain: each step has strictly fewer
    /// ancestors before it than the step before, so the writing always ends.
    ///
    /// # Errors
    ///
    /// The error the ancestor's own within-joiner returns.
    pub fn parts_before(
        &self,
        ancestors: &[Ancestor],
        index: usize,
    ) -> Result<Vec<Part>, FormatError> {
        let Some(ancestor) = ancestors.get(index) else { return Ok(Vec::new()) };
        let Some(joiner) = &ancestor.joiner else { return Ok(Vec::new()) };
        joiner.parts(&ancestors[..index], self)
    }

    /// Returns `text` as a small tree of content, built as the characters of
    /// a number are: one text node attributed to the counter style's source.
    ///
    /// # Errors
    ///
    /// [`FormatError`] when the content could not be built.
    pub fn literal(&self, text: &str) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        assemble_pieces(self.style, &[Piece::Digits(text.to_string())])
    }

    /// Returns one tree of content that holds the nodes of `contents`, in
    /// order.
    ///
    /// # Errors
    ///
    /// [`FormatError`] when the content could not be built.
    pub fn joined(
        &self,
        contents: &[Arc<NodeTree<Flm>>],
    ) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        let pieces: Vec<Piece<'_>> = contents.iter().map(Piece::Flm).collect();
        assemble_pieces(self.style, &pieces)
    }

    /// Returns the counter style to which a within-joiner's content is
    /// attributed.
    pub(super) fn style(&self) -> &CounterStyle {
        self.style
    }
}

/// Returns the index of `counter` among `ancestors`, or the error that gives
/// it. [`PARENT_PLACEHOLDER`] is the last ancestor, the parent that applies.
///
/// A counter appears more than once in a chain only as scoped instances of
/// one counter nested in each other, and its name then refers to the
/// innermost of them, the one immediately above: the enclosing instance of a
/// nested point, whose own joiner writes the instances around it.
fn place_of(
    ancestors: &[Ancestor],
    counter: &Name,
    joiner: &str,
) -> Result<usize, FormatError> {
    let place = if counter.as_str() == PARENT_PLACEHOLDER {
        ancestors.len().checked_sub(1)
    } else {
        ancestors.iter().rposition(|ancestor| &ancestor.counter == counter)
    };
    place.ok_or_else(|| FormatError::UnknownAncestor {
        joiner: joiner.to_string(),
        counter: counter.clone(),
    })
}

/// Returns the parent that applies in `chain`, its last counter, as a list
/// that holds it, or an empty list for an empty chain.
fn parent_in(chain: &[Name]) -> Vec<Name> {
    chain.last().cloned().into_iter().collect()
}

// --- the joining-text form ----------------------------------------------------------------------

/// Returns how a [`Within`] writes the ancestors' numbers, as a text a
/// diagnostic can give: the joining text, the within template, the
/// within-joiner's name, or the empty text for the form that writes nothing of
/// the ancestors.
pub(super) fn joiner_text(within: &Within) -> String {
    match &within.joiner {
        WithinJoinerSpec::Joined(text) | WithinJoinerSpec::Template(text) => text.clone(),
        WithinJoinerSpec::Named(name) => name.to_string(),
        WithinJoinerSpec::Bare => String::new(),
    }
}

/// Returns how a diagnostic describes a joining-text form: the parents it
/// writes and the text after them, since the text alone does not identify
/// the form.
fn described(parents: &Parents, text: &str) -> String {
    format!("{} joined by {text}", parents.describe())
}

/// The joining-text form: the number of the parent that applies (the only
/// parent, or among several the one advanced most recently, which is the last
/// counter of the chain in both cases), then a text.
#[derive(Debug)]
struct Joined {
    described: String,
    joiner: Arc<NodeTree<Flm>>,
}

impl CompiledWithinJoiner for Joined {
    fn counters_written(&self, chain: &[Name]) -> Vec<Name> {
        parent_in(chain)
    }

    fn parts(&self, ancestors: &[Ancestor], cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError> {
        let place = place_of(ancestors, &Name::new(PARENT_PLACEHOLDER), &self.described)?;
        let ancestor = &ancestors[place];
        let mut parts = cx.parts_before(ancestors, place)?;
        let content = cx.joined(&[cx.number_of(ancestor)?, Arc::clone(&self.joiner)])?;
        parts.push(Part::new(
            vec![(ancestor.counter.clone(), ancestor.value.clone())],
            content,
        ));
        Ok(parts)
    }
}

// --- the form that writes nothing of the ancestors ---------------------------------------------

/// The bare form: the counter restarts under its parent, and nothing of the
/// parent, or of any counter above it, is written before its own number.
#[derive(Debug)]
struct Bare;

impl CompiledWithinJoiner for Bare {
    fn counters_written(&self, _chain: &[Name]) -> Vec<Name> {
        Vec::new()
    }

    fn parts(&self, _ancestors: &[Ancestor], _cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError> {
        Ok(Vec::new())
    }
}

// --- the template form -----------------------------------------------------------------------

/// One `#{…}` of a within template: the counter it refers to, the keys it
/// carries, and the text of those keys as written in the template.
struct Occurrence {
    counter: Name,
    keys: AncestorKeys,
    written: String,
}

impl fmt::Debug for Occurrence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Occurrence")
            .field("counter", &self.counter.as_str())
            .field("keys", &self.keys)
            .finish()
    }
}

/// The within-template form.
struct Templated {
    text: String,
    /// The whole template, which writes all parts as one where the template
    /// cannot be cut.
    whole: CompiledTemplate,
    /// Every `#{…}` of the template, in order.
    occurrences: Vec<Occurrence>,
    /// The template cut into one piece per `#{…}`, or `None` when it cannot
    /// be cut.
    shape: Option<Vec<CompiledTemplate>>,
}

impl fmt::Debug for Templated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Templated").field("text", &self.text).finish_non_exhaustive()
    }
}

impl Templated {
    /// Returns what one `#{…}` is replaced by: the content, and the ancestors
    /// that content shows. The result is a [`Part`] because a part has exactly
    /// these two fields; the content is then substituted into the template,
    /// and the result is the part the within-joiner returns.
    fn written(
        &self,
        occurrence: &Occurrence,
        ancestors: &[Ancestor],
        cx: &JoinCx<'_>,
    ) -> Result<Part, FormatError> {
        let place = place_of(ancestors, &occurrence.counter, &self.text)?;
        written_with_keys(ancestors, place, occurrence.keys, cx)
    }
}

/// Returns what a placeholder carrying `keys` writes for `ancestors[place]`:
/// that counter's number, in the format the keys give or else in its own
/// counter style, preceded by the parts its own within-joiner writes of the
/// ancestors before it unless the keys say `bare`; together with the
/// counters that content shows.
///
/// This is the only function that applies the keys of `#[…]{counter}`: a
/// within template writes each of its placeholders through it, and
/// [`Counters::format_with_keys`](super::Counters::format_with_keys) writes a
/// point's own number through it by placing the point at the end of its own
/// chain.
///
/// # Errors
///
/// The error the ancestor's own within-joiner returns, and [`FormatError`]
/// when the content could not be built.
pub(super) fn written_with_keys(
    ancestors: &[Ancestor],
    place: usize,
    keys: AncestorKeys,
    cx: &JoinCx<'_>,
) -> Result<Part, FormatError> {
    let ancestor = &ancestors[place];
    let mut stands_for = Vec::new();
    let mut contents = Vec::new();
    if !keys.bare {
        for part in cx.parts_before(ancestors, place)? {
            stands_for.extend(part.stands_for);
            contents.push(part.content);
        }
    }
    contents.push(match keys.format {
        Some(format) => cx.number_of_in(ancestor, format)?,
        None => cx.number_of(ancestor)?,
    });
    stands_for.push((ancestor.counter.clone(), ancestor.value.clone()));
    Ok(Part::new(stands_for, cx.joined(&contents)?))
}

impl CompiledWithinJoiner for Templated {
    fn counters_written(&self, chain: &[Name]) -> Vec<Name> {
        let mut named: Vec<Name> = Vec::new();
        for occurrence in &self.occurrences {
            let written = if occurrence.counter.as_str() == PARENT_PLACEHOLDER {
                parent_in(chain)
            } else {
                vec![occurrence.counter.clone()]
            };
            for counter in written {
                if !named.contains(&counter) {
                    named.push(counter);
                }
            }
        }
        named
    }

    fn parts(&self, ancestors: &[Ancestor], cx: &JoinCx<'_>) -> Result<Vec<Part>, FormatError> {
        let site = cx.style().site();
        match &self.shape {
            Some(pieces) => {
                let mut parts = Vec::with_capacity(pieces.len());
                for (piece, occurrence) in pieces.iter().zip(&self.occurrences) {
                    let written = self.written(occurrence, ancestors, cx)?;
                    let bindings = Bindings::new(vec![Binding {
                        name: occurrence.counter.as_str().to_string(),
                        keys: occurrence.written.clone(),
                        content: written.content,
                    }]);
                    parts.push(Part::new(
                        written.stands_for,
                        instantiate_template(&site, piece, &bindings)?,
                    ));
                }
                Ok(parts)
            }
            // A template with no `#{…}` at all writes one part standing for
            // no ancestor, and an empty template writes no part.
            None if self.text.is_empty() => Ok(Vec::new()),
            None => {
                let mut entries = Vec::with_capacity(self.occurrences.len());
                let mut stands_for = Vec::new();
                for occurrence in &self.occurrences {
                    let written = self.written(occurrence, ancestors, cx)?;
                    stands_for.extend(written.stands_for);
                    entries.push(Binding {
                        name: occurrence.counter.as_str().to_string(),
                        keys: occurrence.written.clone(),
                        content: written.content,
                    });
                }
                let bindings = Bindings::new(entries);
                let content = instantiate_template(&site, &self.whole, &bindings)?;
                Ok(vec![Part::new(stands_for, content)])
            }
        }
    }
}
