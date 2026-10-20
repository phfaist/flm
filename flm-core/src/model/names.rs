//! Dotted names, which identify every entity registered with the library, and
//! one Rust type per kind of name.
//!
//! A **name** is a string, usually dotted and by convention lowercase, whose
//! first segment says who owns it: `flm` for the standard features, a prefix of
//! its own for anything a third party writes. Capabilities, tables, stages,
//! lowering rules, features, backends, workflows and outputs are identified by
//! their names, never by a Rust type. An element also has a name, its type name
//! ([`ElementName`]), but it is identified by its element value (see
//! [`ElementSpec`](crate::model::element::ElementSpec)). The reference kind of
//! a reference point ([`RefKind`]) is a name too. See [names and
//! identity](crate::guide::concepts_overview#names-and-identity).
//!
//! [`Name`] is the shared representation: a shared string (`Arc<str>`) that
//! compares, hashes and orders by its text. Each kind of name has a type of its
//! own that wraps a [`Name`] ([`ElementName`], [`CapName`], [`StageName`],
//! [`RuleName`], [`FeatureName`], [`BackendName`], [`WorkflowName`],
//! [`OutputName`], [`RefKind`]), so that a function that asks for a capability
//! name cannot be given a stage name by mistake. Every one of these types
//! converts from a string literal, so a declaration can write
//! `"flm.refs.resolved".into()`.
//!
//! A name owns its text rather than borrowing a `&'static str`, because a name
//! may be computed while the registry is built: the name of a theorem
//! environment declared in a configuration, a table created once per backend
//! (`flm.lowered.<backend>`), a stage created once per kind of note
//! (`flm.xnotes.build-list.<kind>`).
//!
//! Every name type is written to and read from data (serde) as its plain text,
//! so that [settings](crate::core::settings) given as data can hold names.
//!
//! # Examples
//!
//! ```
//! use flm_core::model::names::{CapName, Name, StageName};
//!
//! let stage: StageName = "flm.blocks.structure".into();
//! let provided: CapName = "flm.blocks.structured".into();
//! assert_eq!(stage.owner_prefix(), "flm");
//! assert_eq!(provided.as_str(), "flm.blocks.structured");
//! // Every name type dereferences to the `Name` it wraps.
//! assert_eq!(&*stage, &Name::from("flm.blocks.structure"));
//! ```

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// A name: a string, usually dotted, whose first segment says who owns
/// it, such as `flm.heading`, `flm.refs.resolved` or `thm.numbered`.
///
/// `Name` compares, hashes and orders by its text, like a `String`, and
/// cloning one only increments a reference count. Every name type of this
/// module ([`ElementName`], [`CapName`], …) wraps a `Name` and dereferences to
/// it. See the [module documentation](self) for what names identify.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(Arc<str>);

impl Name {
    /// Creates a name holding `text`. No check is made on the text.
    pub fn new(text: impl Into<Arc<str>>) -> Name {
        Name(text.into())
    }

    /// The name's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The owner prefix: the text before the first dot (`flm` in
    /// `flm.heading`, `thm` in `thm.numbered`), or the whole name when it holds
    /// no dot.
    ///
    /// By convention, the prefix says who owns the name: `flm` for the
    /// standard features, another prefix for a third party.
    pub fn owner_prefix(&self) -> &str {
        self.0.split('.').next().unwrap_or(&self.0)
    }
}

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Name {
    fn from(text: &str) -> Name {
        Name(Arc::from(text))
    }
}

impl From<String> for Name {
    fn from(text: String) -> Name {
        Name(Arc::from(text))
    }
}

impl From<Arc<str>> for Name {
    fn from(text: Arc<str>) -> Name {
        Name(text)
    }
}

impl From<&Name> for Name {
    fn from(name: &Name) -> Name {
        name.clone()
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Reads a name from data as a string, so that settings given as data
/// (see [`crate::core::settings`]) can hold names.
impl<'de> serde::Deserialize<'de> for Name {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Name, D::Error> {
        String::deserialize(deserializer).map(Name::from)
    }
}

/// Writes a name to data as a string.
impl serde::Serialize for Name {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Describes a name in a JSON schema as a string.
impl schemars::JsonSchema for Name {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "String".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &*self.0)
    }
}

/// Declares the name type of one kind of name: a type wrapping a [`Name`].
///
/// Every type declared this way converts from `&str`, `String`, `Arc<str>` and
/// [`Name`], and into [`Name`]; dereferences to [`Name`] (and therefore to
/// `str`); displays as its text; compares, hashes and orders by its text; and
/// is written to and read from data as a string.
macro_rules! name_newtype {
    ($(#[$meta:meta])* $newtype:ident) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $newtype(pub Name);

        impl $newtype {
            /// Creates a name holding `text`. No check is made on the text.
            pub fn new(text: impl Into<Arc<str>>) -> $newtype {
                $newtype(Name::new(text))
            }

            /// The [`Name`] this name wraps.
            pub fn name(&self) -> &Name {
                &self.0
            }

            /// The name's text.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl Deref for $newtype {
            type Target = Name;

            fn deref(&self) -> &Name {
                &self.0
            }
        }

        impl Borrow<str> for $newtype {
            fn borrow(&self) -> &str {
                self.0.as_str()
            }
        }

        impl AsRef<str> for $newtype {
            fn as_ref(&self) -> &str {
                self.0.as_str()
            }
        }

        impl From<&str> for $newtype {
            fn from(text: &str) -> $newtype {
                $newtype(Name::from(text))
            }
        }

        impl From<String> for $newtype {
            fn from(text: String) -> $newtype {
                $newtype(Name::from(text))
            }
        }

        impl From<Arc<str>> for $newtype {
            fn from(text: Arc<str>) -> $newtype {
                $newtype(Name::from(text))
            }
        }

        impl From<Name> for $newtype {
            fn from(name: Name) -> $newtype {
                $newtype(name)
            }
        }

        impl From<$newtype> for Name {
            fn from(newtype: $newtype) -> Name {
                newtype.0
            }
        }

        impl PartialEq<str> for $newtype {
            fn eq(&self, other: &str) -> bool {
                self.0.as_str() == other
            }
        }

        impl PartialEq<&str> for $newtype {
            fn eq(&self, other: &&str) -> bool {
                self.0.as_str() == *other
            }
        }

        impl<'de> serde::Deserialize<'de> for $newtype {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<$newtype, D::Error> {
                String::deserialize(deserializer).map($newtype::from)
            }
        }

        impl serde::Serialize for $newtype {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl schemars::JsonSchema for $newtype {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                "String".into()
            }

            fn inline_schema() -> bool {
                true
            }

            fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                String::json_schema(generator)
            }
        }

        impl fmt::Display for $newtype {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for $newtype {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($newtype), self.0.as_str())
            }
        }
    };
}

name_newtype! {
    /// The type name of an element, which says what kind of thing a node is:
    /// `flm.heading`, `flm.pres.block`, `thm.exercise`.
    ///
    /// Several element values may share one type name and differ in their data
    /// (every heading level is an element value of type `flm.heading`); an
    /// element is identified by its value, not by this name. See
    /// [`ElementSpec`](crate::model::element::ElementSpec).
    ElementName
}

name_newtype! {
    /// The reference kind of a reference point, which says what sort of thing
    /// the point is: `section`, `equation`, `theorem`
    /// ([`RefPoint::kind`](crate::model::aspects::RefPoint::kind)).
    ///
    /// The set of kinds is open: a feature uses the kinds it needs. A reference
    /// kind is not the label type that an author writes before the colon of a
    /// label such as `sec:intro`; the point declares separately which label
    /// types it accepts (see [`Label`](crate::model::aspects::Label) and
    /// [`AcceptedRefTypes`](crate::model::aspects::AcceptedRefTypes)).
    RefKind
}

name_newtype! {
    /// The name of a capability, such as `flm.refs.resolved`.
    ///
    /// A capability is a registered dotted name for a state of processing that
    /// a stage brings about: a stage declares the capabilities it provides and
    /// the capabilities it requires, and a caller demands the capabilities it
    /// wants ([`CapabilityDef`](crate::core::registry::CapabilityDef)).
    /// Capabilities and computed tables share one set of names: a table's name
    /// is also the capability "this table has been computed".
    CapName
}

name_newtype! {
    /// The name of a [stage](crate::core::stage::Stage).
    ///
    /// The standard features write it `flm.<feature>.<verb>`: the stage
    /// `flm.blocks.structure` provides the capability `flm.blocks.structured`.
    StageName
}

name_newtype! {
    /// The name of a [lowering rule](crate::core::lowering::LoweringRule).
    ///
    /// The standard features write it `<element>.lower.<variant>`, the type
    /// name of the element the rule lowers followed by `lower` and the
    /// variant: `flm.cells.cell.lower.default`.
    RuleName
}

name_newtype! {
    /// The registered name of a [feature](crate::core::registry::FlmFeature):
    /// `refs`, `theorems`, `htmlplus`.
    ///
    /// The standard features use it as the second segment of the names of
    /// their stages and their diagnostics (`flm.refs.resolve`).
    FeatureName
}

name_newtype! {
    /// The name of a [backend](crate::core::registry::BackendDef), the
    /// registration bundle for one output format, such as `text` or `html`.
    ///
    /// Registering a backend derives from this name the names of what belongs
    /// to it, such as its table of lowered trees `flm.lowered.<backend>`.
    BackendName
}

name_newtype! {
    /// The name of a workflow, such as `text` or `check`.
    ///
    /// A workflow is a named set of demands together with the choices that go
    /// with them, which a caller selects by name instead of listing
    /// capabilities ([`WorkflowDef`](crate::core::registry::WorkflowDef)).
    WorkflowName
}

name_newtype! {
    /// The name of an output, such as `text` or `html`: a result a caller can
    /// ask for, registered with what producing it means
    /// ([`OutputDef`](crate::core::registry::OutputDef)).
    ///
    /// The `flm` command-line program accepts these names as the values of its
    /// `--format` option.
    OutputName
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn names_compare_by_text() {
        let a = Name::from("flm.heading");
        let b = Name::new(String::from("flm.heading"));
        assert_eq!(a, b);
        assert_eq!(a, "flm.heading");
        assert_eq!(a.owner_prefix(), "flm");
        assert_eq!(Name::from("plain").owner_prefix(), "plain");
    }

    #[test]
    fn newtypes_convert_and_order() {
        let cap: CapName = "flm.refs.resolved".into();
        let cap2 = CapName::from(Name::from("flm.refs.resolved"));
        assert_eq!(cap, cap2);
        assert_eq!(cap.as_str(), "flm.refs.resolved");
        assert_eq!(&*cap, &Name::from("flm.refs.resolved"));
        let set: BTreeSet<StageName> =
            ["b.x", "a.y"].into_iter().map(StageName::from).collect();
        let ordered: Vec<&str> = set.iter().map(|n| n.as_str()).collect();
        assert_eq!(ordered, ["a.y", "b.x"]);
        assert_eq!(format!("{}", cap), "flm.refs.resolved");
        assert_eq!(format!("{:?}", cap), "CapName(\"flm.refs.resolved\")");
    }
}
