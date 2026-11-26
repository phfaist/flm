//! The kind of a row and the shared texts of a row.

use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// A text of a row (its kind, title or label), shared and never copied when
/// a row is read.
///
/// A `SharedText` holds either a `&'static str` (a kind or a title declared as
/// a constant) or an `Arc<str>` (a label, or a name the caller already holds).
/// Cloning one copies no text, so the [`RowSnapshot`](super::RowSnapshot)
/// given to the observer is built without allocating. It dereferences to
/// `str`, and it compares, hashes, orders and serializes as its text.
#[derive(Clone)]
pub struct SharedText(Repr);

#[derive(Clone)]
enum Repr {
    Static(&'static str),
    Shared(Arc<str>),
}

impl SharedText {
    /// A text that refers to `text`, a constant: nothing is allocated.
    pub const fn from_static(text: &'static str) -> SharedText {
        SharedText(Repr::Static(text))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        match &self.0 {
            Repr::Static(text) => text,
            Repr::Shared(text) => text,
        }
    }
}

impl Deref for SharedText {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for SharedText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for SharedText {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&'static str> for SharedText {
    fn from(text: &'static str) -> SharedText {
        SharedText::from_static(text)
    }
}

impl From<String> for SharedText {
    fn from(text: String) -> SharedText {
        SharedText(Repr::Shared(text.into()))
    }
}

impl From<Arc<str>> for SharedText {
    fn from(text: Arc<str>) -> SharedText {
        SharedText(Repr::Shared(text))
    }
}

impl PartialEq for SharedText {
    fn eq(&self, other: &SharedText) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SharedText {}

impl PartialEq<str> for SharedText {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SharedText {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialOrd for SharedText {
    fn partial_cmp(&self, other: &SharedText) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SharedText {
    fn cmp(&self, other: &SharedText) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for SharedText {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl fmt::Display for SharedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for SharedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl serde::Serialize for SharedText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for SharedText {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<SharedText, D::Error> {
        String::deserialize(deserializer).map(SharedText::from)
    }
}

/// The sort of work a row stands for: a dotted name and, optionally, a title.
///
/// The name is the same for every row of that sort, such as
/// `flm.graphics.convert`; the run gives the row of a stage the stage's name.
/// The feature that owns the work declares its kind as a constant where the
/// work is done, and passes it by reference to [`Scope::child`](super::Scope::child):
///
/// ```
/// use flm_core::progress::Kind;
///
/// const CONVERT: Kind = Kind::titled("flm.graphics.convert", "Converting graphics");
/// assert_eq!(CONVERT.name(), "flm.graphics.convert");
/// ```
///
/// Kinds are not registered. A display identifies a sort of row by the name,
/// so renaming a kind is a change that a host's stored data about that kind
/// (such as its measured durations) does not follow.
///
/// **The title** is the fixed text that a display shows for the rows of the
/// kind, before the label of the row: "Converting graphics figure-3.pdf". A
/// title says what is being done: it begins with a capital and a verb in
/// "-ing", has no article and no final period, and names nothing of the
/// library's internals ("Reading document", "Fetching citation data"). A kind
/// whose rows a person need not see has no title; a display then shows the
/// nearest row above that has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kind {
    name: SharedText,
    title: Option<SharedText>,
}

impl Kind {
    /// A kind named `name`, with no title.
    pub const fn new(name: &'static str) -> Kind {
        Kind { name: SharedText::from_static(name), title: None }
    }

    /// A kind named `name`, with the title `title`.
    pub const fn titled(name: &'static str, title: &'static str) -> Kind {
        Kind { name: SharedText::from_static(name), title: Some(SharedText::from_static(title)) }
    }

    /// A kind with no title whose name is a text the caller already holds,
    /// such as the name of a stage; the text is shared, not copied.
    pub fn shared(name: impl Into<Arc<str>>) -> Kind {
        Kind { name: SharedText::from(name.into()), title: None }
    }

    /// The same kind, with the title `title`, a text the caller already
    /// holds, such as a title an extension gives; the text is shared, not
    /// copied.
    pub fn with_title(self, title: impl Into<Arc<str>>) -> Kind {
        Kind { title: Some(SharedText::from(title.into())), ..self }
    }

    /// The kind's dotted name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The kind's title, if it has one.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The name, as the shared text that a row holds.
    pub(super) fn shared_name(&self) -> &SharedText {
        &self.name
    }

    /// The title, as the shared text that a row holds.
    pub(super) fn shared_title(&self) -> Option<&SharedText> {
        self.title.as_ref()
    }
}
