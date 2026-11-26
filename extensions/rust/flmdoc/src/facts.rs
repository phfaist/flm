//! Reading the key–values of a construct: the facts of an item, an entry, a parameter or an
//! item of an inline list, and the options of `docentries`.
//!
//! Each construct that takes key–values names the keys it reads ([`Key`]), what each accepts
//! ([`Accepts`]), and how the keys depend on one another ([`Companion`], [`Required`],
//! [`Exclusive`]). [`read`] reads the construct's bracket against them and reports, as an
//! Error at the construct, every key it does not read, every value a key does not accept,
//! and every key given without what it depends on; the facts it returns hold the keys it
//! read. A key given twice is read once, its last value in effect, as in LaTeX.

use flm_ext::ExtError;
use flm_ext::guest::lang::copy::WrittenPiece;
use flm_ext::guest::lang::text::chars_text;
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::extract::parse_keyval_drop_annotations;
use flm_ext::guest::techy::node::{Node, Nodes};
use flm_ext::guest::core::lowering::Lowerer;
use flm_ext::guest::stdlib::backends::lowering::is_blank;
use serde_json::json;

/// The Error reported for a key that a construct does not read.
pub(crate) const UNKNOWN_KEY: &str = "flmdoc.unknown-key";
/// The Error reported for a value that a key does not accept, a key given without the value
/// it needs, and a flag given a value.
pub(crate) const BAD_VALUE: &str = "flmdoc.key-value";
/// The Error reported for a key given without a key it depends on, a key missing that
/// another key's value needs, and two keys that may not be given together.
pub(crate) const KEY_DEPENDENCY: &str = "flmdoc.key-dependency";
/// The Error reported for a bracket that cannot be read as key–values.
pub(crate) const BAD_KEYVALS: &str = "flmdoc.keyvals";

/// What the value of a key may be.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Accepts {
    /// No value: the key alone is the fact (`optional`, `repeatable`).
    Nothing,
    /// Document content: a link, code, an inline list, text.
    Content,
    /// One word, without whitespace: one of the words listed, or any word when the list is
    /// empty.
    Word(&'static [&'static str]),
    /// A whole number, at most the number given, when one is.
    Number(Option<u32>),
}

/// A key a construct reads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Key {
    /// The key, as written.
    pub(crate) name: &'static str,
    /// What its value may be.
    pub(crate) accepts: Accepts,
}

/// The key `name`, whose value is as `accepts` says.
pub(crate) const fn key(name: &'static str, accepts: Accepts) -> Key {
    Key { name, accepts }
}

/// A key read only together with another: `key` is given with `with`, which has one of the
/// values `values` (any value, or none, when the list is empty).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Companion {
    /// The key that depends on the other.
    pub(crate) key: &'static str,
    /// The key it is read with.
    pub(crate) with: &'static str,
    /// The values of `with` it is read with; any when empty.
    pub(crate) values: &'static [&'static str],
}

/// Keys a value needs: when `key` has the value `value`, each of `needs` is given.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Required {
    /// The key whose value needs the others.
    pub(crate) key: &'static str,
    /// Its value that needs them.
    pub(crate) value: &'static str,
    /// The keys it needs.
    pub(crate) needs: &'static [&'static str],
}

/// Two keys that may not be given together; the first is read, the second is not.
pub(crate) type Exclusive = (&'static str, &'static str);

/// The keys a construct reads and how they depend on one another.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Vocabulary {
    /// The construct, as an author writes it: `docitem`, `\docparam`, …
    pub(crate) construct: &'static str,
    /// The keys it reads.
    pub(crate) keys: &'static [Key],
    /// The keys read only with another.
    pub(crate) companions: &'static [Companion],
    /// The keys a value needs.
    pub(crate) required: &'static [Required],
    /// The keys that may not be given together.
    pub(crate) exclusive: &'static [Exclusive],
}

/// The value read for a key.
#[derive(Debug, Clone)]
pub(crate) enum Given {
    /// The key alone.
    Flag,
    /// Document content, as pieces of the document.
    Content(Vec<WrittenPiece>),
    /// A word, or a whole number written as one.
    Word(String),
}

/// The facts a construct's bracket gives: each key it read, with its value.
#[derive(Debug, Clone, Default)]
pub(crate) struct Facts {
    given: Vec<(&'static str, Given)>,
}

impl Facts {
    /// Whether the key `key` was read.
    pub(crate) fn has(&self, key: &str) -> bool {
        self.given.iter().any(|(name, _)| *name == key)
    }

    /// The content of the key `key`, when it was read.
    pub(crate) fn content(&self, key: &str) -> Option<&[WrittenPiece]> {
        self.given.iter().find(|(name, _)| *name == key).and_then(|(_, given)| match given {
            Given::Content(pieces) => Some(pieces.as_slice()),
            _ => None,
        })
    }

    /// The word of the key `key`, when it was read.
    pub(crate) fn word(&self, key: &str) -> Option<&str> {
        self.given.iter().find(|(name, _)| *name == key).and_then(|(_, given)| match given {
            Given::Word(word) => Some(word.as_str()),
            _ => None,
        })
    }

    /// Forgets the key `key`.
    fn remove(&mut self, key: &str) {
        self.given.retain(|(name, _)| *name != key);
    }

    /// Records `given` for `key`, in place of an earlier value.
    fn set(&mut self, key: &'static str, given: Given) {
        self.remove(key);
        self.given.push((key, given));
    }
}

/// Reads the bracket `argument` of `node`, the construct `vocabulary` describes, as
/// key–values: no facts when the bracket is not given. Every problem is reported as an Error
/// at `node`, and the key concerned is not read.
pub(crate) fn read(node: &Node, argument: &str, vocabulary: &Vocabulary, lw: &mut Lowerer) -> Result<Facts, ExtError> {
    let mut facts = Facts::default();
    let Some(content) = node.argument_content_nodes_named(argument)? else { return Ok(facts) };
    let construct = vocabulary.construct;
    let list = match parse_keyval_drop_annotations(&content) {
        Ok(list) => list,
        Err(ExtError::Core { .. }) => {
            let message = format!("Invalid keys of ‘{construct}’: each key must be plain text, as in ‘key=value’");
            report(lw, node, BAD_KEYVALS, message, json!({ "construct": construct }))?;
            return Ok(facts);
        }
        Err(error) => return Err(error),
    };
    for entry in list.iter()? {
        let name = entry.key()?;
        let value = entry.value()?;
        // An empty entry, as a trailing comma leaves, says nothing.
        if name.is_empty() && value.as_ref().is_none_or(Nodes::is_empty) {
            continue;
        }
        let data = json!({ "construct": construct, "key": name });
        let Some(key) = vocabulary.keys.iter().find(|key| key.name == name) else {
            let accepted = quoted(vocabulary.keys.iter().map(|key| key.name));
            let message = format!("Unknown key ‘{name}’ of ‘{construct}’ (accepted keys: {accepted})");
            report(lw, node, UNKNOWN_KEY, message, data)?;
            continue;
        };
        let pieces = match entry.value_pieces()? {
            Some(pieces) => Some(value_pieces(pieces)?),
            None => None,
        };
        let blank = match &pieces {
            Some(pieces) => is_blank_value(pieces)?,
            None => false,
        };
        let given = match (key.accepts, pieces) {
            (Accepts::Nothing, None) => Given::Flag,
            (Accepts::Nothing, Some(_)) => {
                report(lw, node, BAD_VALUE, format!("Key ‘{name}’ of ‘{construct}’ may not have a value"), data)?;
                continue;
            }
            (_, None) => {
                report(lw, node, BAD_VALUE, format!("Missing value for key ‘{name}’ of ‘{construct}’"), data)?;
                continue;
            }
            (Accepts::Content, Some(_)) if blank => {
                report(lw, node, BAD_VALUE, format!("Missing value for key ‘{name}’ of ‘{construct}’"), data)?;
                continue;
            }
            (Accepts::Content, Some(pieces)) => Given::Content(pieces),
            (Accepts::Word(words), Some(pieces)) => match word_of(&pieces)? {
                Some(word) if word.is_empty() => {
                    report(lw, node, BAD_VALUE, format!("Missing value for key ‘{name}’ of ‘{construct}’"), data)?;
                    continue;
                }
                Some(word) if is_one_word(&word) && (words.is_empty() || words.contains(&word.as_str())) => Given::Word(word),
                word => {
                    let written = match word {
                        Some(word) => word,
                        None => source_of(&value.unwrap_or_default().to_vec()?),
                    };
                    let rule = match words.is_empty() {
                        true => String::from(": the value must be one word"),
                        false => format!(" (accepted values: {})", quoted(words.iter().copied())),
                    };
                    report_invalid(lw, node, construct, &name, &written, &rule)?;
                    continue;
                }
            },
            (Accepts::Number(max), Some(pieces)) => match word_of(&pieces)? {
                Some(word) if is_number_up_to(&word, max) => Given::Word(word),
                word => {
                    let written = match word {
                        Some(word) => word,
                        None => source_of(&value.unwrap_or_default().to_vec()?),
                    };
                    let rule = match max {
                        Some(max) => format!(": the value must be a whole number from 0 to {max}"),
                        None => String::from(": the value must be a whole number"),
                    };
                    report_invalid(lw, node, construct, &name, &written, &rule)?;
                    continue;
                }
            },
        };
        facts.set(key.name, given);
    }
    check_dependencies(&mut facts, node, vocabulary, lw)?;
    Ok(facts)
}

/// Reports, and forgets, the keys given without what they depend on: a key read only with
/// another, a value that needs keys that are missing (that key is forgotten), and the second
/// of two keys that may not be given together.
fn check_dependencies(facts: &mut Facts, node: &Node, vocabulary: &Vocabulary, lw: &mut Lowerer) -> Result<(), ExtError> {
    let construct = vocabulary.construct;
    for (first, second) in vocabulary.exclusive {
        if facts.has(first) && facts.has(second) {
            let message = format!("Keys ‘{first}’ and ‘{second}’ of ‘{construct}’ may not be given together");
            report(lw, node, KEY_DEPENDENCY, message, json!({ "construct": construct, "keys": [first, second] }))?;
            facts.remove(second);
        }
    }
    for companion in vocabulary.companions {
        if !facts.has(companion.key) {
            continue;
        }
        let present = match companion.values {
            [] => facts.has(companion.with),
            values => facts.word(companion.with).is_some_and(|word| values.contains(&word)),
        };
        if !present {
            let with = match companion.values {
                [] => format!("‘{}’", companion.with),
                values => values.iter().map(|value| format!("‘{}={value}’", companion.with)).collect::<Vec<_>>().join(" or "),
            };
            let message = format!("Key ‘{}’ of ‘{construct}’ must be given with {with}", companion.key);
            let data = json!({ "construct": construct, "key": companion.key, "with": companion.with });
            report(lw, node, KEY_DEPENDENCY, message, data)?;
            facts.remove(companion.key);
        }
    }
    for required in vocabulary.required {
        if facts.word(required.key) != Some(required.value) {
            continue;
        }
        let missing: Vec<&str> = required.needs.iter().copied().filter(|need| !facts.has(need)).collect();
        if !missing.is_empty() {
            let keys = quoted(missing.iter().copied());
            let noun = if missing.len() == 1 { "key" } else { "keys" };
            let message = format!("Missing {noun} {keys} of ‘{construct}’ for ‘{}={}’", required.key, required.value);
            report(lw, node, KEY_DEPENDENCY, message, json!({ "construct": construct, "key": required.key, "missing": missing }))?;
            facts.remove(required.key);
        }
    }
    Ok(())
}

/// The pieces `pieces` of an entry's value; a value written as one group, `{…}`, is the
/// group's content, without the blank nodes at either end.
fn value_pieces(pieces: Vec<WrittenPiece>) -> Result<Vec<WrittenPiece>, ExtError> {
    match pieces.as_slice() {
        [WrittenPiece::Node(group)] if group.is_group()? => {
            let mut children = group.children()?.to_vec()?;
            while let Some(last) = children.last() {
                if !is_blank(last)? {
                    break;
                }
                children.pop();
            }
            let mut first = 0;
            while first < children.len() && is_blank(&children[first])? {
                first += 1;
            }
            Ok(children.split_off(first).into_iter().map(WrittenPiece::Node).collect())
        }
        _ => Ok(pieces),
    }
}

/// Whether `pieces` hold nothing but whitespace, as [`is_blank`] tells it of a node.
fn is_blank_value(pieces: &[WrittenPiece]) -> Result<bool, ExtError> {
    for piece in pieces {
        let blank = match piece {
            WrittenPiece::Text(text) => text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')),
            WrittenPiece::Node(node) => is_blank(node)?,
        };
        if !blank {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The characters of `pieces`, or `None` when a piece is not characters.
pub(crate) fn word_of(pieces: &[WrittenPiece]) -> Result<Option<String>, ExtError> {
    let mut word = String::new();
    for piece in pieces {
        match piece {
            WrittenPiece::Text(text) => word.push_str(text),
            WrittenPiece::Node(node) => match chars_text(&Nodes::from(vec![*node]))? {
                Some(text) => word.push_str(&text),
                None => return Ok(None),
            },
        }
    }
    Ok(Some(word.trim().to_string()))
}

/// Whether `word` is one word: it holds no whitespace.
fn is_one_word(word: &str) -> bool {
    !word.chars().any(char::is_whitespace)
}

/// Whether `word` is a whole number, at most `max` when it is given.
fn is_number_up_to(word: &str, max: Option<u32>) -> bool {
    if word.is_empty() || !word.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match max {
        Some(max) => word.parse::<u32>().is_ok_and(|number| number <= max),
        None => true,
    }
}

/// The source text of `nodes`, trimmed.
fn source_of(nodes: &[Node]) -> String {
    let mut text = String::new();
    for node in nodes {
        text.push_str(&node.span_content().unwrap_or_default());
    }
    text.trim().to_string()
}

/// `names` each between ‘…’, separated by commas.
fn quoted<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names.map(|name| format!("‘{name}’")).collect::<Vec<_>>().join(", ")
}

/// Reports the value `written` of the key `name` of `construct`, which the key does not
/// accept, by the rule `rule` (its text after the message's first part), at `node`.
fn report_invalid(lw: &mut Lowerer, node: &Node, construct: &str, name: &str, written: &str, rule: &str) -> Result<(), ExtError> {
    let message = format!("Invalid value ‘{written}’ for key ‘{name}’ of ‘{construct}’{rule}");
    report(lw, node, BAD_VALUE, message, json!({ "construct": construct, "key": name, "value": written }))
}

/// Reports the Error `id` with `message` and `data` at `node`.
pub(crate) fn report(lw: &mut Lowerer, node: &Node, id: &str, message: String, data: serde_json::Value) -> Result<(), ExtError> {
    lw.diagnose(Diagnostic::error(id, message).with_data(data).at(node))
}
