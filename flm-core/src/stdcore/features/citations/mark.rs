//! The citation mark of a citation chunk: [`stage_mark`], the only function
//! that stages one, the feature's settings as the lowering rules read them
//! ([`MarkSettings`]), and the general lowering rules of `\cite` and `\R`
//! ([`CiteRule`], [`RefCiteRule`]).
//!
//! To stage a node is to build it into the lowered tree through the
//! [`Lowerer`] a lowering rule is handed. The general rules are the default
//! rules of `\cite` and `\R` on every backend but the recomposed LaTeX,
//! which has rules of its own ([`latex`](super::latex)). The general rules
//! require the feature's style identifier [`STYLE`] (`flm.citations.style`) at every
//! citation chunk, which each backend resolves as it resolves requirements:
//! the stylesheet on the `html` backend ([`html`](super::html)), the
//! definition pieces delivering it on the `latex-rendered` backend, nothing
//! on the `text` backend.
//!
//! The citation mark is a span whose role and kind tokens say what it is (a
//! role is a feature-owned name that says what a presentation node is, and
//! kind tokens classify it further; see
//! [roles and kinds](crate::stdcore::backends::pres::payload)): role
//! [`MARK_ROLE`], kind token [`ATTACHED`] or [`STANDALONE`], and [`IN_PLACE`]
//! under the in-place placement. The space before a `\cite`'s citation mark
//! has the role [`SPACE_ROLE`], and the word before an `\R`'s citation mark
//! the role [`WORD_ROLE`].

use std::sync::{Arc, OnceLock};

use techy::core::node::{BuildId, NodeRef, NodeTree};

use crate::core::lowering::{
    chars_at_lowering, DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl,
    LoweringRule,
};
use crate::core::registry::{LateInit, LateInitError};
use crate::lang::rewrites::userdefs::template::{PlaceholderVocabulary, TemplateLanguage};
use crate::lang::{BuiltLang, Flm, FlmDiagnostics, FlmSpan};
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::stage_trimmed;
use crate::stdcore::backends::pres::{
    self, DocRole, DroppedPayload, LinkPayload, SpanPayload, SpanStyle, spelling_identity,
};
use crate::stdcore::features::numbering::{
    compile_text, format_many, instantiate_fixed, CounterStyle, CounterValue, FormatOptions,
    JoinText, NumberedValue,
};

use super::config::{CitationPlacement, CitationsConfig, MarkForm, CAPITAL_VARIANT};
use super::diagnostics::{NoCitation, NoKey, Uncollected, UnknownFlag};
use super::html::STYLE;
use super::elements::{
    leading_call, read_chunk, Chunk, ChunkMistake, Reading, CITE, FLAGS_ARG,
    REFCITE,
};
use super::entries::{CitationEntries, ENTRIES};
use super::numbers::{CitedWorks, CITATIONS};

/// The role of a citation mark's span, `flm.citations.mark`.
pub const MARK_ROLE: &str = "flm.citations.mark";

/// The role of the span that holds the word `\R` writes before its citation
/// mark, `flm.citations.word`.
pub const WORD_ROLE: &str = "flm.citations.word";

/// The role of the span that holds the space written before the citation
/// mark of a `\cite` ([`space_before`]), `flm.citations.space`.
///
/// The space is a character of the text, in a span of its own, so that a
/// stylesheet can select it (to hide it or change its width) while the text a
/// reader copies keeps it.
pub const SPACE_ROLE: &str = "flm.citations.space";

/// The kind token of the citation mark of a `\cite`, `attached`: the citation
/// mark belongs to the content before it, and the space that its form calls
/// for is written before it.
pub const ATTACHED: &str = "attached";

/// The kind token of the citation mark of an `\R`, `standalone`: the citation
/// mark is a part of its sentence, shown inline, with nothing written before
/// it.
pub const STANDALONE: &str = "standalone";

/// The kind token of a citation mark under the in-place placement,
/// `in-place`, which the span carries after [`ATTACHED`] or [`STANDALONE`]:
/// the citation mark shows the citation entries.
pub const IN_PLACE: &str = "in-place";

/// The kind of every link from a citation mark to a cited work, `cite`.
pub const LINK_KIND: &str = "cite";

/// The name of the general lowering rule of `\cite`,
/// `flm.citations.cite.lower.default` ([`CiteRule`]), the default on every
/// backend.
pub const CITE_RULE: &str = "flm.citations.cite.lower.default";

/// The name of the general lowering rule of `\R`,
/// `flm.citations.refcite.lower.default` ([`RefCiteRule`]), the default on
/// every backend.
pub const REFCITE_RULE: &str = "flm.citations.refcite.lower.default";

/// The flag of `\R` that selects the capitalized word (the citation counter style's
/// variant [`CAPITAL_VARIANT`]), `S`.
pub const CAPITAL_FLAG: char = 'S';

/// The flag of `\R` that writes no word, only the citation mark, `I`.
pub const INLINE_FLAG: char = 'I';

/// The no-break space (U+00A0) written before an attached citation mark,
/// under the bracketed form and under the in-place placement.
pub const NO_BREAK_SPACE: &str = "\u{a0}";

/// How a citation mark is placed in its sentence: attached to the content
/// before it (`\cite`) or standing alone (`\R`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The citation mark of a `\cite`: attached to the content before it,
    /// with the space of its form written before it ([`space_before`]).
    Attached,
    /// The citation mark of an `\R`: a part of its sentence, shown inline and
    /// never raised, with nothing written before it.
    Standalone,
}

impl Standing {
    /// Returns the kind token that the citation mark's span carries:
    /// [`ATTACHED`] or [`STANDALONE`].
    pub fn kind_token(self) -> &'static str {
        match self {
            Standing::Attached => ATTACHED,
            Standing::Standalone => STANDALONE,
        }
    }
}

/// The feature's settings as the lowering rules read them: the
/// configuration, and its texts (the citation counter style, the note
/// separator and the title of the list of cited works) compiled when the
/// registry is built.
///
/// The feature creates one value per registration and shares it between its
/// rules; a build hook ([`LateInit`]) compiles the texts once the language is
/// built. Before that, the accessors that need the compiled texts return a
/// structure error, which a built registry never returns.
#[derive(Debug)]
pub struct MarkSettings {
    config: CitationsConfig,
    compiled: OnceLock<Compiled>,
}

/// The compiled texts of the configuration.
struct Compiled {
    style: CounterStyle,
    separator: Arc<NodeTree<Flm>>,
    title: Option<Arc<NodeTree<Flm>>>,
}

impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiled").field("style", &self.style).finish_non_exhaustive()
    }
}

impl MarkSettings {
    /// Creates the settings of `config`, its texts not yet compiled; the
    /// feature registers [`CompileSettings`] to compile them.
    pub(super) fn new(config: CitationsConfig) -> MarkSettings {
        MarkSettings { config, compiled: OnceLock::new() }
    }

    /// Returns the configuration.
    pub fn config(&self) -> &CitationsConfig {
        &self.config
    }

    /// Returns the compiled texts, or a structure error before the registry
    /// build compiled them, which never happens with a built registry.
    fn compiled(&self) -> Result<&Compiled, LowerError> {
        self.compiled
            .get()
            .ok_or_else(|| LowerError::structure("the citation settings were not compiled"))
    }

    /// Returns the compiled citation counter style.
    ///
    /// # Errors
    ///
    /// A structure error ([`LowerError::structure`]) before the registry build
    /// compiled the citation counter style.
    pub fn style(&self) -> Result<&CounterStyle, LowerError> {
        Ok(&self.compiled()?.style)
    }

    /// Returns the compiled title of the heading above the list of cited works
    /// ([`heading_title`](CitationsConfig::heading_title)), or `None` for a
    /// list with no heading.
    ///
    /// # Errors
    ///
    /// A structure error ([`LowerError::structure`]) before the registry build
    /// compiled the title.
    pub fn heading_title(&self) -> Result<Option<&Arc<NodeTree<Flm>>>, LowerError> {
        Ok(self.compiled()?.title.as_ref())
    }
}

/// The build hook ([`LateInit`]) that compiles the citation counter style,
/// the note separator and the title of the list of cited works. A text
/// that is not valid FLM fails the registry build, with a message that
/// identifies the citation counter style or the text.
pub(super) struct CompileSettings {
    pub(super) settings: Arc<MarkSettings>,
}

/// Compiles `text`, the configuration's text described by `what`, as FLM
/// text with no placeholder, in the template language that counter styles
/// use.
fn compile_fixed(lang: &BuiltLang, text: &str, what: &str) -> Result<Arc<NodeTree<Flm>>, LateInitError> {
    let failed = |error: &dyn std::fmt::Display| LateInitError::new(format!("{what} is set to `{text}`, which is not valid FLM text: {error}"));
    let language = TemplateLanguage::new(lang).map_err(|errors| {
        failed(&errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
    })?;
    let template = compile_text(text, what, &PlaceholderVocabulary::new(0), lang, &language)
        .map_err(|error| failed(&error))?;
    instantiate_fixed(&template).map_err(|error| failed(&error))
}

impl LateInit for CompileSettings {
    fn late_init(&self, lang: &BuiltLang, _diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        let config = &self.settings.config;
        let style = config.style.compile(lang).map_err(|error| {
            LateInitError::new(format!("the option `style` of the feature `citations` is not a valid counter style: {error}"))
        })?;
        let separator = compile_fixed(lang, &config.note_separator, "the option `note_separator` of the feature `citations`")?;
        let title = config
            .heading_title
            .as_deref()
            .map(|title| compile_fixed(lang, title, "the option `heading_title` of the feature `citations`"))
            .transpose()?;
        // One hook per registration of the feature, run once.
        let _ = self.settings.compiled.set(Compiled { style, separator, title });
        Ok(())
    }
}

/// Returns the text written before the citation mark of a `\cite` under
/// `config`: a no-break space ([`NO_BREAK_SPACE`]) under the bracketed form,
/// and under the in-place placement regardless of the form; `None` under the
/// superscript form of the listed placement. Nothing is ever written before
/// the citation mark of an `\R`.
///
/// The general rule of `\cite` ([`CiteRule`]) stages it, on every backend
/// that runs that rule, as a span of role [`SPACE_ROLE`] holding the
/// character, directly before the citation mark's span. The HTML page writes
/// the character inside an element that a stylesheet can select, and the
/// character is part of the text a reader copies.
pub fn space_before(config: &CitationsConfig) -> Option<&'static str> {
    match (config.placement, config.mark) {
        (CitationPlacement::InPlace, _) | (CitationPlacement::Listed, MarkForm::Bracketed) => {
            Some(NO_BREAK_SPACE)
        }
        (CitationPlacement::Listed, MarkForm::Superscript) => None,
    }
}

/// What [`stage_mark`] staged, and how many numbers the citation mark
/// holds.
#[derive(Debug, Clone)]
pub struct StagedMark {
    /// The citation mark's span, or, for a citation chunk with nothing to
    /// show, a dropped mark (`flm.pres.dropped`).
    pub node: BuildId,
    /// How many numbers the citation mark holds, counting each cited work once
    /// per group; 0 for a dropped mark. Under the in-place placement the count
    /// is computed the same way, although the citation mark shows citation
    /// entries. The rule of `\R` chooses the singular or the plural word by
    /// it.
    pub shown: usize,
}

/// One group of a citation mark: the numbers of the items without a note, or
/// of the items of one note.
struct Group<'t> {
    /// The note's spelling with whitespace collapsed, `None` for the group of
    /// the items without a note.
    identity: Option<String>,
    /// The note as written at its first appearance.
    note: Option<Vec<NodeRef<'t, Flm>>>,
    /// The group's numbers with their identifiers, in item order.
    works: Vec<(usize, String)>,
}

/// Reports each mistake that [`read_chunk`] found in `chunk` as an Error at
/// the span of its call ([`ChunkMistake::NoKey`] as [`NoKey`]).
pub fn report_mistakes(lw: &mut dyn Lowerer, chunk: &Chunk<'_>) {
    for mistake in &chunk.mistakes {
        match mistake {
            ChunkMistake::NoKey { span } => lw.error(NoKey, span.clone()),
        }
    }
}

/// Stages the citation mark of `chunk` (what is shown in the text for the
/// citation chunk) and returns it with the count of numbers it holds. This is
/// the only function that stages a citation mark; the rules of `\cite` and
/// of `\R` call it.
///
/// Each item's number and identifier are looked up by its citation key in the
/// table [`CITATIONS`], never by the node, so a citation mark shows the same
/// numbers wherever its citation chunk is lowered, a copy shown again
/// included. A citation key that the table does not hold is reported
/// ([`Uncollected`]) at `span` and left out.
///
/// The citation mark is one span of role [`MARK_ROLE`] whose kind token is
/// the [`Standing`]'s, holding, in order: one group of every item without a
/// note, then one group per distinct note in order of the note's first
/// appearance in the citation chunk. Two notes are the same note when their
/// content has the same spelling with whitespace collapsed
/// ([`spelling_identity`]). A group holds the citation counter style's opening
/// delimiter, its numbers sorted and compressed through [`format_many`], then,
/// for a note, the note separator and the note's content lowered with its
/// edges trimmed, then the closing delimiter. With `a`, `b`, `c`, `d`, `e`
/// cited in that order earlier, `\cite{c,b}\cite[Thm 4]{a}\cite{d}\cite[Thm
/// 4]{e}` reads `[2–4][1,5; Thm 4]`. Each work appears once in a group: two
/// citation keys of one work, or a citation key written twice, show one
/// number, and [`StagedMark::shown`] counts the numbers shown. Every number is
/// a link of kind [`LINK_KIND`] to its work's identifier, with the document
/// role [`DocRole::BiblioRef`] (written `role="doc-biblioref"` on the HTML
/// page); the delimiters and the join texts are outside the links.
///
/// The configuration's form decides the delimiters. Under
/// [`MarkForm::Superscript`] a group has none, two groups are joined by the
/// citation counter style's `list_mid` join text, and an
/// attached citation mark's span carries the superscript style; a standalone
/// citation mark is never raised.
///
/// A citation chunk with no item to show (every call has a mistake, every
/// citation key is reported as [`Uncollected`], or an `\R` has no citation
/// after it) is staged as a dropped mark ([`pres::DROPPED`]; see
/// [recovery](crate::stdcore::backends::lowering::error)) instead.
///
/// Under the in-place placement ([`CitationPlacement::InPlace`]) the citation
/// mark shows the citation entries instead: for each item whose citation key
/// has a number, in item order, one group between the citation counter style's
/// delimiters holding the item's citation entry (read by its citation key from
/// the table [`ENTRIES`] and spliced with [`Lowerer::splice`]), then the note
/// separator and the note where the item has one:
/// `[Knuth, 1984][Lamport, 1994; Thm 4]`. The form, the sorting, the ranges
/// and the join texts do not apply, nothing is linked, the span carries the
/// kind token [`IN_PLACE`] after the [`Standing`]'s, and an item whose
/// citation key has no citation entry shows a dropped mark inside its
/// delimiters.
///
/// A caller's rule must declare that it reads [`CITATIONS`] and [`ENTRIES`]
/// and produces [`pres::SPAN`], [`pres::LINK`] and [`pres::DROPPED`].
///
/// # Errors
///
/// [`LowerError`] from staging, and a structure error before the registry
/// build compiled the settings.
pub fn stage_mark(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    chunk: &Chunk<'_>,
    settings: &MarkSettings,
    standing: Standing,
    span: &FlmSpan,
) -> Result<StagedMark, LowerError> {
    let compiled = settings.compiled()?;
    let form = settings.config.mark;
    let found: Vec<Option<(usize, String)>> = {
        let citations: Option<&CitedWorks> = lw.get(CITATIONS)?;
        chunk
            .items
            .iter()
            .map(|item| {
                let work = citations?.work(&item.key)?;
                Some((work.number, work.identifier.clone()))
            })
            .collect()
    };
    let mut groups: Vec<Group<'_>> = vec![Group { identity: None, note: None, works: Vec::new() }];
    let mut uncollected = false;
    for (item, work) in chunk.items.iter().zip(found) {
        let Some(work) = work else {
            lw.error(Uncollected { key: item.key.clone() }, span.clone());
            uncollected = true;
            continue;
        };
        let identity = item.note.as_ref().map(|note| spelling_identity(note.iter().copied()));
        let at = match groups.iter().position(|group| group.identity == identity) {
            Some(at) => at,
            None => {
                groups.push(Group { identity, note: item.note.clone(), works: Vec::new() });
                groups.len() - 1
            }
        };
        // The numbers of a group are a set: two keys of one work, or one key
        // written twice, show one number.
        if !groups[at].works.iter().any(|(number, _)| *number == work.0) {
            groups[at].works.push(work);
        }
    }
    groups.retain(|group| !group.works.is_empty());
    let shown: usize = groups.iter().map(|group| group.works.len()).sum();
    if shown == 0 {
        let node = stage_dropped(cx, lw, chunk, uncollected)?;
        return Ok(StagedMark { node, shown: 0 });
    }
    let style = &compiled.style;
    if settings.config.placement == CitationPlacement::InPlace {
        return stage_in_place(cx, lw, chunk, compiled, standing, shown);
    }
    let delimited = form == MarkForm::Bracketed;
    let mut children = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        // The join between two groups under the superscript form: see decision
        // rationale at [§flm-dd-dr:std-citations-grammar].
        if index > 0 && !delimited {
            children.extend(lw.splice(cx, style.join().text(JoinText::ListMid))?);
        }
        if delimited {
            children.extend(lw.splice(cx, style.open())?);
        }
        children.extend(stage_numbers(cx, lw, style, &group.works)?);
        if let Some(note) = &group.note {
            children.extend(lw.splice(cx, &compiled.separator)?);
            children.extend(stage_trimmed(cx, lw, note, |_| false)?);
        }
        if delimited {
            children.extend(lw.splice(cx, style.close())?);
        }
    }
    let raised = form == MarkForm::Superscript && standing == Standing::Attached;
    let payload = SpanPayload {
        style: raised.then_some(SpanStyle::Superscript),
        role: Some(Name::from(MARK_ROLE)),
        kinds: vec![Name::from(standing.kind_token())],
        ..SpanPayload::default()
    };
    let node = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, children)?;
    Ok(StagedMark { node, shown })
}

/// Stages the dropped mark ([`pres::DROPPED`]) shown in place of a citation
/// chunk with no item to show. Its condition gives the reason: every citation
/// key uncollected (`uncollected`), else the citation chunk's first mistake,
/// else no citation at all.
pub(super) fn stage_dropped(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    chunk: &Chunk<'_>,
    uncollected: bool,
) -> Result<BuildId, LowerError> {
    let condition = if uncollected {
        "flm.citations.uncollected"
    } else {
        match chunk.mistakes.first() {
            Some(ChunkMistake::NoKey { .. }) => "flm.citations.no-key",
            None => "flm.citations.no-citation",
        }
    };
    let mark = DroppedPayload { condition: Name::from(condition), detail: "citation".to_string() };
    lw.pres_with(cx, &ElementName::from(pres::DROPPED), mark, Vec::new())
}

/// The note of an item, as [`read_chunk`] returns it.
type Note<'t> = Option<Vec<NodeRef<'t, Flm>>>;

/// Stages the citation mark of `chunk` under the in-place placement, as
/// [`stage_mark`] describes: one group per item whose citation key has a
/// number, holding its citation entry or a dropped mark.
fn stage_in_place(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    chunk: &Chunk<'_>,
    compiled: &Compiled,
    standing: Standing,
    shown: usize,
) -> Result<StagedMark, LowerError> {
    // Each shown item's entry, looked up by its key before anything is staged.
    let items: Vec<(Option<Arc<NodeTree<Flm>>>, &Note<'_>)> = {
        let citations: Option<&CitedWorks> = lw.get(CITATIONS)?;
        let entries: Option<&CitationEntries> = lw.get(ENTRIES)?;
        chunk
            .items
            .iter()
            .filter(|item| citations.is_some_and(|citations| citations.work(&item.key).is_some()))
            .map(|item| {
                let entry = entries.and_then(|entries| entries.entry(&item.key));
                (entry.map(|entry| Arc::clone(&entry.content)), &item.note)
            })
            .collect()
    };
    let style = &compiled.style;
    let mut children = Vec::new();
    for (entry, note) in items {
        children.extend(lw.splice(cx, style.open())?);
        match entry {
            Some(content) => children.extend(lw.splice(cx, &content)?),
            None => {
                let mark = DroppedPayload {
                    condition: Name::from("flm.citations.no-entry"),
                    detail: "bibliography entry".to_string(),
                };
                children.push(lw.pres_with(cx, &ElementName::from(pres::DROPPED), mark, Vec::new())?);
            }
        }
        if let Some(note) = note {
            children.extend(lw.splice(cx, &compiled.separator)?);
            children.extend(stage_trimmed(cx, lw, note, |_| false)?);
        }
        children.extend(lw.splice(cx, style.close())?);
    }
    let payload = SpanPayload {
        role: Some(Name::from(MARK_ROLE)),
        kinds: vec![Name::from(standing.kind_token()), Name::from(IN_PLACE)],
        ..SpanPayload::default()
    };
    let node = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, children)?;
    Ok(StagedMark { node, shown })
}

/// Stages the numbers of one group, sorted and compressed by the citation
/// counter style, each number a link to its work.
fn stage_numbers(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    style: &CounterStyle,
    works: &[(usize, String)],
) -> Result<Vec<BuildId>, LowerError> {
    let values: Vec<CounterValue> =
        works.iter().map(|(number, _)| CounterValue::new(*number as i64)).collect();
    let numbered: Vec<NumberedValue<'_>> = values.iter().map(NumberedValue::new).collect();
    let options = FormatOptions::new().without_word().without_delimiters();
    let segments = format_many(style, &numbered, &options)
        .map_err(|error| LowerError::structure(error.to_string()))?;
    let mut staged = Vec::new();
    for segment in &segments {
        let body = lw.splice(cx, segment.content())?;
        match segment.link().and_then(|position| works.get(position)) {
            Some((_, identifier)) => {
                let payload = LinkPayload {
                    href: format!("#{identifier}"),
                    kind: Name::from(LINK_KIND),
                    doc_role: Some(DocRole::BiblioRef),
                    label: None,
                    shows_target: false,
                };
                staged.push(lw.pres_with(cx, &ElementName::from(pres::LINK), payload, body)?);
            }
            None => staged.extend(body),
        }
    }
    Ok(staged)
}

/// The flags of an `\R` node as read: whether the capitalized word is
/// chosen, and whether a word is written.
pub(super) struct Flags {
    pub(super) capital: bool,
    pub(super) word: bool,
}

/// Reads the flags of the `\R` node `node`: each character of its flags
/// argument is a flag, and whitespace is skipped. An unknown flag is reported
/// ([`UnknownFlag`]) and ignored; a flags argument that is not plain
/// characters is reported once ([`UnknownFlag`] with its plain text) and
/// ignored as a whole.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] for a flags argument that holds a
/// placeholder of a user definition's template being lowered without
/// expansion ([`chars_at_lowering`]).
pub(super) fn read_flags(node: &NodeRef<'_, Flm>, lw: &mut dyn Lowerer) -> Result<Flags, LowerError> {
    let mut flags = Flags { capital: false, word: true };
    let Ok(Some(content)) = node.argument_content_nodes_named(FLAGS_ARG) else {
        return Ok(flags);
    };
    let text = match chars_at_lowering(content.iter())? {
        Ok(text) => text,
        Err(_) => {
            let flag = crate::lang::text::plain_text_of(content.iter());
            lw.error(UnknownFlag { flag }, node.span().clone());
            return Ok(flags);
        }
    };
    for flag in text.chars().filter(|flag| !flag.is_whitespace()) {
        match flag {
            CAPITAL_FLAG => flags.capital = true,
            INLINE_FLAG => flags.word = false,
            other => lw.error(UnknownFlag { flag: other.to_string() }, node.span().clone()),
        }
    }
    Ok(flags)
}

/// The body of the general rule of `\cite`, [`CITE_RULE`]: it stages the
/// space that the form calls for ([`space_before`]) in a span of role
/// [`SPACE_ROLE`], then the citation chunk's attached citation mark
/// ([`stage_mark`]). It reports the citation chunk's writing mistakes
/// ([`report_mistakes`]), and requires the feature's style identifier
/// [`STYLE`].
///
/// # Errors
///
/// As [`read_chunk`](super::read_chunk) and [`stage_mark`].
#[derive(Debug)]
pub struct CiteRule {
    settings: Arc<MarkSettings>,
}

impl CiteRule {
    /// Creates the rule under the feature's settings.
    pub(super) fn new(settings: &Arc<MarkSettings>) -> CiteRule {
        CiteRule { settings: Arc::clone(settings) }
    }
}

impl Lower for CiteRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let chunk = read_chunk(&node, Reading::AtLowering)?;
        report_mistakes(lw, &chunk);
        let mut out = Vec::with_capacity(2);
        if let Some(space) = space_before(&self.settings.config) {
            let body = vec![lw.text(cx, space)?];
            let payload = SpanPayload { role: Some(Name::from(SPACE_ROLE)), ..SpanPayload::default() };
            out.push(lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?);
        }
        let mark = stage_mark(cx, lw, &chunk, &self.settings, Standing::Attached, node.span())?;
        out.push(mark.node);
        Ok(Lowering::Emit(out))
    }
}

/// The body of the general rule of `\R`, [`REFCITE_RULE`]: it stages the
/// citation counter style's word in the variant the flags select, singular or
/// plural by [`StagedMark::shown`], in a span of role [`WORD_ROLE`], then the
/// citation chunk's standalone citation mark ([`stage_mark`]). It writes no
/// word under the flag `I` or for a dropped mark, and nothing before the word.
/// It reports an `\R` with no `\cite` directly after it ([`NoCitation`]), the
/// flags it does not define ([`UnknownFlag`]) and the citation chunk's writing
/// mistakes ([`report_mistakes`]). It requires the feature's style
/// identifier [`STYLE`].
///
/// # Errors
///
/// As [`read_chunk`](super::read_chunk) and [`stage_mark`].
#[derive(Debug)]
pub struct RefCiteRule {
    settings: Arc<MarkSettings>,
}

impl RefCiteRule {
    /// Creates the rule under the feature's settings.
    pub(super) fn new(settings: &Arc<MarkSettings>) -> RefCiteRule {
        RefCiteRule { settings: Arc::clone(settings) }
    }
}

impl Lower for RefCiteRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let flags = read_flags(&node, lw)?;
        if leading_call(&node).is_none() {
            lw.error(NoCitation, node.span().clone());
        }
        let chunk = read_chunk(&node, Reading::AtLowering)?;
        report_mistakes(lw, &chunk);
        let mark = stage_mark(cx, lw, &chunk, &self.settings, Standing::Standalone, node.span())?;
        let mut out = Vec::with_capacity(2);
        if flags.word && mark.shown > 0 {
            let variant = flags.capital.then_some(CAPITAL_VARIANT);
            let style = self.settings.style()?;
            if let Some(word) = style.word_text(mark.shown, variant) {
                let body = lw.splice(cx, word)?;
                let payload = SpanPayload { role: Some(Name::from(WORD_ROLE)), ..SpanPayload::default() };
                out.push(lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?);
            }
        }
        out.push(mark.node);
        Ok(Lowering::Emit(out))
    }
}

/// The declaration of what the general rules of `\cite` and `\R` read and
/// produce.
pub(super) fn decl() -> LoweringDecl {
    LoweringDecl::new()
        .reads(CITATIONS)
        .reads(ENTRIES)
        .produces(pres::SPAN)
        .produces(pres::LINK)
        .produces(pres::DROPPED)
}

/// The general rules of `\cite` and `\R`, marked as the defaults on every
/// backend. The `latex-recomposed` backend has rules of its own, marked as
/// the defaults there.
pub(super) fn rules(settings: &Arc<MarkSettings>) -> Vec<LoweringRule> {
    vec![
        LoweringRule::new(
            CITE_RULE,
            CITE,
            decl(),
            Arc::new(CiteRule::new(settings)),
            DefaultMark::Everywhere,
        )
        .docs(
            "a citation chunk lowers to the space its form calls for and a `flm.pres.span` of \
             role `flm.citations.mark`, kind token `attached`, holding its numbers as links of kind \
             `cite`",
        ),
        LoweringRule::new(
            REFCITE_RULE,
            REFCITE,
            decl(),
            Arc::new(RefCiteRule::new(settings)),
            DefaultMark::Everywhere,
        )
        .docs(
            "a citation chunk written with `\\R` lowers to the citation counter style's word in a \
             `flm.pres.span` of role `flm.citations.word`, then the citation mark, kind token \
             `standalone`, shown inline",
        ),
    ]
}
