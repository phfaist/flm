//! The words `flmdoc` writes for each fact: one function per fact, read by every construct
//! that accepts its key. Each function stages the fact's words around the content its keys
//! hold, or nothing when the keys it reads were not given.

use flm_ext::ExtError;
use flm_ext::guest::core::lowering::{LowerCx, Lowerer, Staged};
use flm_ext::guest::lang::copy::WrittenPiece;
use flm_ext::guest::techy::node::Nodes;

use crate::flmdoc::facts::Facts;
use crate::flmdoc::{INLINE_ITEM, INLINE_LIST, code};

/// A note or a sentence of a construct: the key it is written for, and its words.
pub(crate) type Note = (&'static str, Vec<Staged>);
/// A clause of an entry: the key it is written for, the separator written before it, and
/// its words.
pub(crate) type Clause = (&'static str, &'static str, Vec<Staged>);

/// The sentence of a parameter that may be written more than once (`repeatable`).
pub(crate) const REPEATABLE: &str = "It may be written more than once.";
/// The sentence of an element value no command is written for (`unwritten`).
pub(crate) const UNWRITTEN: &str = "No command is written for this value.";

/// Staged nodes built in order: text, inline code, and the content of a key.
pub(crate) struct Words<'a> {
    cx: &'a mut LowerCx,
    lw: &'a mut Lowerer,
    staged: Vec<Staged>,
}

impl<'a> Words<'a> {
    /// Nothing yet, staged with `cx` and `lw`.
    pub(crate) fn new(cx: &'a mut LowerCx, lw: &'a mut Lowerer) -> Words<'a> {
        Words { cx, lw, staged: Vec::new() }
    }

    /// Appends the text `text`.
    pub(crate) fn text(&mut self, text: &str) -> Result<&mut Self, ExtError> {
        if !text.is_empty() {
            let node = self.lw.text(self.cx, text)?;
            self.staged.push(node);
        }
        Ok(self)
    }

    /// Appends `word` as inline code.
    pub(crate) fn code(&mut self, word: &str) -> Result<&mut Self, ExtError> {
        let node = code(self.cx, self.lw, word, "inline")?;
        self.staged.push(node);
        Ok(self)
    }

    /// Appends the content `pieces`, lowered.
    pub(crate) fn content(&mut self, pieces: &[WrittenPiece]) -> Result<&mut Self, ExtError> {
        let staged = lower_pieces(self.cx, self.lw, pieces)?;
        self.staged.extend(staged);
        Ok(self)
    }

    /// The nodes staged.
    pub(crate) fn done(&mut self) -> Vec<Staged> {
        std::mem::take(&mut self.staged)
    }
}

/// The content `pieces` lowered: each node through the rules, each run of characters as text.
pub(crate) fn lower_pieces(cx: &mut LowerCx, lw: &mut Lowerer, pieces: &[WrittenPiece]) -> Result<Vec<Staged>, ExtError> {
    let mut staged = Vec::new();
    for piece in pieces {
        match piece {
            WrittenPiece::Node(node) => staged.extend(lw.lower_nodes(cx, &Nodes::from(vec![*node]))?.to_vec()?),
            WrittenPiece::Text(text) if text.is_empty() => {}
            WrittenPiece::Text(text) => staged.push(lw.text(cx, text)?),
        }
    }
    Ok(staged)
}

/// The number of things `pieces` names: the number of items of an inline list
/// (`docinlinelist`) given alone, and one for any other content.
pub(crate) fn count(pieces: &[WrittenPiece]) -> Result<usize, ExtError> {
    match pieces {
        [WrittenPiece::Node(node)] if node.is_callable()? && node.element_name()? == INLINE_LIST => {
            let mut items = 0;
            for child in node.body()?.unwrap_or_default().to_vec()? {
                if child.is_callable()? && child.element_name()? == INLINE_ITEM {
                    items += 1;
                }
            }
            Ok(items)
        }
        _ => Ok(1),
    }
}

/// `singular` for one thing, `plural` for any other number.
fn noun(number: usize, singular: &'static str, plural: &'static str) -> &'static str {
    if number == 1 { singular } else { plural }
}

/// Who declared an item (`declared-by`, as `declarer` says, with `within`, `switched-off`
/// and `page`): “the feature X”, “the language itself (X)”, “the program, as data (X)”, “the
/// package X”, “written as data by X”, then “, in the scoped package Y”, “, which is
/// switched off” and “; see Z”.
pub(crate) fn declared_by(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let Some(who) = facts.content("declared-by") else { return Ok(Vec::new()) };
    let mut words = Words::new(cx, lw);
    match facts.word("declarer").unwrap_or("feature") {
        "language" => words.text("the language itself (")?.content(who)?.text(")")?,
        "program" => words.text("the program, as data (")?.content(who)?.text(")")?,
        "package" => words.text("the package ")?.content(who)?,
        "data" => words.text("written as data by ")?.content(who)?,
        _ => words.text("the feature ")?.content(who)?,
    };
    if let Some(within) = facts.content("within") {
        words.text(", in the scoped package ")?.content(within)?;
    }
    if facts.has("switched-off") {
        words.text(", which is switched off")?;
    }
    if let Some(page) = facts.content("page") {
        words.text("; see ")?.content(page)?;
    }
    Ok(words.done())
}

/// Where a feature was switched off (`switched-off`): “in X”.
pub(crate) fn switched_off(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let Some(place) = facts.content("switched-off") else { return Ok(Vec::new()) };
    Ok(Words::new(cx, lw).text("in ")?.content(place)?.done())
}

/// Where a command is available (`modes`, `places`): “X mode” or “X modes”, then “inside Y”,
/// separated by a comma.
pub(crate) fn available(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let mut words = Words::new(cx, lw);
    if let Some(modes) = facts.content("modes") {
        words.content(modes)?.text(&format!(" {}", noun(count(modes)?, "mode", "modes")))?;
    }
    if let Some(places) = facts.content("places") {
        if facts.has("modes") {
            words.text(", ")?;
        }
        words.text("inside ")?.content(places)?;
    }
    Ok(words.done())
}

/// Where a package comes from (`origin`, with `extension`): “compiled into the program”,
/// “from the extension X”, “from the extension X, bundled into the program”.
pub(crate) fn origin(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let mut words = Words::new(cx, lw);
    match (facts.word("origin"), facts.content("extension")) {
        (Some("compiled"), _) => words.text("compiled into the program")?,
        (Some("extension"), Some(extension)) => words.text("from the extension ")?.content(extension)?,
        (Some("bundled"), Some(extension)) => {
            words.text("from the extension ")?.content(extension)?.text(", bundled into the program")?
        }
        _ => &mut words,
    };
    Ok(words.done())
}

/// The label of a stage's status in a plan (`workflow`): “In the plan of the workflow X”, or
/// “In the plan in view”.
pub(crate) fn plan_label(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let mut words = Words::new(cx, lw);
    match facts.content("workflow") {
        Some(workflow) => words.text("In the plan of the workflow ")?.content(workflow)?,
        None => words.text("In the plan in view")?,
    };
    Ok(words.done())
}

/// A stage's status in a plan (`plan`, with `batch`, `by` and `over`): “scheduled, in batch
/// N”, “scheduled, as a rewrite stage”, “ruled out by X, which provides Y as well”, “not
/// scheduled: nothing the plan needs requires it”.
pub(crate) fn plan(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let mut words = Words::new(cx, lw);
    match facts.word("plan") {
        Some("scheduled") => match facts.word("batch") {
            Some(batch) => words.text(&format!("scheduled, in batch {batch}"))?,
            None => words.text("scheduled, as a rewrite stage")?,
        },
        Some("ruled-out") => {
            words.text("ruled out ")?;
            ruled_out_by(facts, &mut words)?
        }
        Some("not-scheduled") => words.text("not scheduled: nothing the plan needs requires it")?,
        _ => &mut words,
    };
    Ok(words.done())
}

/// The stage and the capability that rule a stage out (`by`, `over`): “by X, which provides
/// Y as well”.
fn ruled_out_by<'w, 'a>(facts: &Facts, words: &'w mut Words<'a>) -> Result<&'w mut Words<'a>, ExtError> {
    if let (Some(by), Some(over)) = (facts.content("by"), facts.content("over")) {
        words.text("by ")?.content(by)?.text(", which provides ")?.content(over)?.text(" as well")?;
    }
    Ok(words)
}

/// The notes of a parameter, each its words, in order: “laid out as X” (`layout`), “read in
/// X mode” (`mode`), “with the scoped package X available inside” (`scoped`; “packages” for
/// several), “content kept as written” (`kept`), “an argument of the field X” (`field`),
/// “default: X” (`default`), “read as X” (`reader`).
pub(crate) fn param_notes(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Note>, ExtError> {
    let mut notes = Vec::new();
    let mut words = Words::new(cx, lw);
    if let Some(layout) = facts.word("layout") {
        notes.push(("layout", words.text("laid out as ")?.code(layout)?.done()));
    }
    if let Some(mode) = facts.word("mode") {
        notes.push(("mode", words.text("read in ")?.code(mode)?.text(" mode")?.done()));
    }
    if let Some(scoped) = facts.content("scoped") {
        let noun = noun(count(scoped)?, "package", "packages");
        notes.push(("scoped", words.text(&format!("with the scoped {noun} "))?.content(scoped)?.text(" available inside")?.done()));
    }
    if facts.has("kept") {
        notes.push(("kept", words.text("content kept as written")?.done()));
    }
    if let Some(field) = facts.content("field") {
        notes.push(("field", words.text("an argument of the field ")?.content(field)?.done()));
    }
    if let Some(default) = facts.content("default") {
        notes.push(("default", words.text("default: ")?.content(default)?.done()));
    }
    if let Some(reader) = facts.content("reader") {
        notes.push(("reader", words.text("read as ")?.content(reader)?.done()));
    }
    Ok(notes)
}

/// The notes of an item of an inline list, each its words, in order: “written by X”
/// (`writer`), “may be absent” (`optional`), “not in the registry” (`missing`).
pub(crate) fn item_notes(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Note>, ExtError> {
    let mut notes = Vec::new();
    let mut words = Words::new(cx, lw);
    if let Some(writer) = facts.content("writer") {
        notes.push(("writer", words.text("written by ")?.content(writer)?.done()));
    }
    if facts.has("optional") {
        notes.push(("optional", words.text("may be absent")?.done()));
    }
    if facts.has("missing") {
        notes.push(("missing", words.text("not in the registry")?.done()));
    }
    Ok(notes)
}

/// What an item names, before the item: “the package ” (`package`); then the part of a
/// command an item of `places` names (`body`, `argument`, `field`): “the body of ”,
/// “argument N of ”, “the argument X of ”, each followed, with `field`, by “the field F of
/// ”; nothing when none of these is given.
pub(crate) fn place(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Staged>, ExtError> {
    let mut words = Words::new(cx, lw);
    if facts.has("package") {
        words.text("the package ")?;
    }
    let field = facts.content("field");
    match facts.word("argument") {
        Some(position) if position.bytes().all(|byte| byte.is_ascii_digit()) => {
            words.text(&format!("argument {position} of "))?;
        }
        Some(name) => {
            words.text("the argument ")?.code(name)?.text(" of ")?;
        }
        None if facts.has("body") => {
            words.text("the body of ")?;
        }
        None => {}
    }
    if let Some(field) = field {
        words.text("the field ")?.content(field)?.text(" of ")?;
    }
    Ok(words.done())
}

/// The facts of an entry written in parentheses after its name, each its words, in order:
/// its origin (`origin`, as [`origin`]), its declaring feature (`owner`, or “given as data”
/// for `data`), “outputs: X” (`outputs`), and where a folder was set (`from`).
pub(crate) fn entry_notes(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Note>, ExtError> {
    let mut notes = Vec::new();
    let origin = origin(facts, cx, lw)?;
    if !origin.is_empty() {
        notes.push(("origin", origin));
    }
    let mut words = Words::new(cx, lw);
    if let Some(owner) = facts.content("owner") {
        notes.push(("owner", words.content(owner)?.done()));
    }
    if facts.has("data") {
        notes.push(("data", words.text("given as data")?.done()));
    }
    if let Some(outputs) = facts.content("outputs") {
        notes.push(("outputs", words.text("outputs: ")?.content(outputs)?.done()));
    }
    if let Some(from) = facts.content("from") {
        notes.push(("from", words.content(from)?.done()));
    }
    Ok(notes)
}

/// The facts of an entry written as clauses after its name and its notes, each with the
/// separator written before it and its words, in order: “, which does not exist” (`absent`),
/// “, by X, which provides Y as well” (`by`, `over`), “ provides the profile X” (`profile`).
pub(crate) fn entry_clauses(
    facts: &Facts,
    cx: &mut LowerCx,
    lw: &mut Lowerer,
) -> Result<Vec<Clause>, ExtError> {
    let mut clauses = Vec::new();
    let mut words = Words::new(cx, lw);
    if facts.has("absent") {
        clauses.push(("absent", ", ", words.text("which does not exist")?.done()));
    }
    if facts.has("by") {
        ruled_out_by(facts, &mut words)?;
        clauses.push(("by", ", ", words.done()));
    }
    if let Some(profile) = facts.content("profile") {
        clauses.push(("profile", " ", words.text("provides the profile ")?.content(profile)?.done()));
    }
    Ok(clauses)
}

/// The sentences of an entry after its summary, each its words, in order: “Written as X.”
/// (`written`), “No command is written for this value.” (`unwritten`), “Aspects: X.”
/// (`aspects`).
pub(crate) fn entry_sentences(facts: &Facts, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Vec<Note>, ExtError> {
    let mut sentences = Vec::new();
    let mut words = Words::new(cx, lw);
    if let Some(written) = facts.content("written") {
        sentences.push(("written", words.text("Written as ")?.content(written)?.text(".")?.done()));
    }
    if facts.has("unwritten") {
        sentences.push(("unwritten", words.text(UNWRITTEN)?.done()));
    }
    if let Some(aspects) = facts.content("aspects") {
        sentences.push(("aspects", words.text("Aspects: ")?.content(aspects)?.text(".")?.done()));
    }
    Ok(sentences)
}
