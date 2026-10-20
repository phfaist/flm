//! The data-valued slots: the service slots of the standard package whose
//! value type is plain data, each with the reader that builds the value from
//! its data form.
//!
//! [`FeatureBuilder::contribute_to`](crate::api::core::registry::FeatureBuilder::contribute_to),
//! `contribute_first_to` and `peer_slot` take the value as a
//! [`Contributed`](crate::api::core::registry::Contributed), routed by the
//! slot's name ([`services`](crate::adapter::services)); data given to one of
//! the slots below reaches `contribute` through the route's table of
//! standard data-valued slots. `flm-core` types a contribution when it is
//! made and refuses the methods in the build's second round, so the value
//! is typed here, at the call, by the slot's name: the table below gives
//! each slot its value type and its reader. The stem rule is checked on the
//! value's own name (`name`, or `id` for a package definition).
//!
//! | Slot | Value type | Data form |
//! |---|---|---|
//! | `flm.numbering.counters` | `CounterDecl` | `{ name, default_style, docs?, within?, nesting? }` |
//! | `flm.numbering.styles` | `CounterStyleDecl` | `{ name, number_format?, sub_levels?, prefix_words?, delimiters?, join?, name_in_link?, repeat_parents_in_range?, ranges?, number_writer? }` |
//! | `flm.numbering.parent-sets` | `ParentSetDecl` | `{ name, counters: [name] }` |
//! | `flm.latex.preamble` | `LatexPreambleDef` | `{ name, content, docs?, requires?: [id] }` |
//! | `flm.latex.packages` | `LatexPackageDef` | `{ id, package, options: [text], docs }` |
//!
//! A field marked `?` may be left out or `null`. `CounterDecl`,
//! `ParentSetDecl` and `LatexPreambleDef` are built through their
//! constructors and builder methods; a `CounterStyleDecl` through `new(name)`
//! and then each field given, read through that field's own serde form (the
//! defaults are the constructor's); a `LatexPackageDef` through its serde
//! form, every field required, then through its constructor and builder
//! methods. `within`, `nesting` and the fields of a
//! counter style are their `flm-core` types' serde forms.

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder as CoreFeatureBuilder};
use flm_core::model::names::Name;
use flm_core::stdcore::backends::latex::{packages, preamble, LatexPackageDef, LatexPreambleDef};
use flm_core::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, ParentSetDecl, Within, WithinJoinerSpec, COUNTERS_SLOT, PARENT_SETS_SLOT, STYLES_SLOT,
};
use serde_json::Value;

use crate::adapter::names::Owner;
use crate::adapter::services::How;
use crate::adapter::values::{field, fields, required_text};
use crate::runtime::ExtError;

/// `flm.numbering.counters`.
pub(crate) const COUNTERS: &str = COUNTERS_SLOT;
/// `flm.numbering.styles`.
pub(crate) const STYLES: &str = STYLES_SLOT;
/// `flm.numbering.parent-sets`.
pub(crate) const PARENT_SETS: &str = PARENT_SETS_SLOT;
/// `flm.latex.packages`.
pub(crate) const PACKAGES: &str = packages::SLOT;
/// `flm.latex.preamble`.
pub(crate) const PREAMBLE: &str = preamble::SLOT;

/// A value read for one of the slots of [`SLOTS`].
#[derive(Debug, Clone)]
pub(crate) enum SlotValue {
    /// A counter (`flm.numbering.counters`).
    Counter(CounterDecl),
    /// A counter style (`flm.numbering.styles`).
    Style(Box<CounterStyleDecl>),
    /// A parent set (`flm.numbering.parent-sets`).
    ParentSet(ParentSetDecl),
    /// A definition piece of the recomposed LaTeX (`flm.latex.preamble`).
    Preamble(LatexPreambleDef),
    /// A package definition of the recomposed LaTeX (`flm.latex.packages`).
    Package(LatexPackageDef),
}

/// A reader: the value and its own name (the name the stem rule checks).
type Reader = fn(&Value) -> Result<(String, SlotValue), ExtError>;

/// One data-valued slot: its name, what its value's name is called in the
/// stem rule's message, and its reader.
struct SlotRow {
    slot: &'static str,
    what: &'static str,
    read: Reader,
}

/// The table of the data-valued slots.
const SLOTS: [SlotRow; 5] = [
    SlotRow { slot: COUNTERS, what: "counter", read: counter_decl },
    SlotRow { slot: STYLES, what: "counter style", read: counter_style_decl },
    SlotRow { slot: PARENT_SETS, what: "parent set", read: parent_set_decl },
    SlotRow { slot: PACKAGES, what: "package definition", read: package_def },
    SlotRow { slot: PREAMBLE, what: "preamble piece", read: preamble_def },
];

/// The value given for `slot`, read from `value`, with the kind of name the
/// stem rule names and the value's own name; `None` when `slot` is not a
/// data-valued slot.
pub(crate) fn read(slot: &str, value: &Value) -> Option<Result<(&'static str, String, SlotValue), ExtError>> {
    let row = SLOTS.iter().find(|row| row.slot == slot)?;
    Some((row.read)(value).map(|(name, value)| (row.what, name, value)))
}

/// Contributes the data `value` to the data-valued slot `slot` for `owner`,
/// as `how` says: read by the slot's reader, its own name checked under the
/// extension's stem, then given to `flm-core` as the slot's type. The row of
/// the route's table of standard data-valued slots
/// ([`services`](crate::adapter::services)) for each slot of [`SLOTS`].
///
/// # Errors
///
/// The value does not read ([`ExtError::Value`]); its name is outside the
/// extension ([`ExtError::NameOutsideExtension`]); `flm-core`'s refusal
/// ([`ExtError::Core`]), such as `ContributionAfterAssembly`.
pub(crate) fn contribute(
    b: &mut CoreFeatureBuilder<'_>,
    owner: &Owner,
    slot: &str,
    value: Value,
    how: How,
) -> Result<(), ExtError> {
    let Some(read) = read(slot, &value) else {
        unreachable!("the route's table names only the slots of `SLOTS`")
    };
    let (what, name, value) = read?;
    owner.check(what, &name)?;
    value.contribute(b, slot, how).map_err(|error| ExtError::Core {
        kind: "build".to_string(),
        detail: error.to_string(),
        data: Value::Null,
    })
}

impl SlotValue {
    /// Contributes the value to `slot` through `b`, as `how` says
    /// (`contribute_to`, `contribute_first_to` or `peer_slot`).
    pub(crate) fn contribute(self, b: &mut CoreFeatureBuilder<'_>, slot: &str, how: How) -> Result<(), BuildError> {
        fn give<S: std::any::Any + Send + Sync>(
            b: &mut CoreFeatureBuilder<'_>,
            slot: &str,
            value: S,
            how: How,
        ) -> Result<(), BuildError> {
            match how {
                How::Append => b.contribute_to(slot, Arc::new(value)),
                How::First => b.contribute_first_to(slot, Arc::new(value)),
                How::Peer => b.peer_slot(slot, Arc::new(value)),
            }
        }
        match self {
            SlotValue::Counter(value) => give(b, slot, value, how),
            SlotValue::Style(value) => give(b, slot, *value, how),
            SlotValue::ParentSet(value) => give(b, slot, value, how),
            SlotValue::Preamble(value) => give(b, slot, value, how),
            SlotValue::Package(value) => give(b, slot, value, how),
        }
    }
}

// --- the readers -------------------------------------------------------------------------------

/// The error of a field of `what` that is not `expected`.
fn bad_field(what: &str, key: &str, expected: &str, value: &Value) -> ExtError {
    ExtError::Value { detail: format!("the field `{key}` of {what} must be {expected}, not `{value}`") }
}

/// An optional text field: `None` when absent or `null`.
fn optional_text(what: &str, map: &BTreeMap<String, &Value>, key: &str) -> Result<Option<String>, ExtError> {
    match field(map, key) {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        other => Err(bad_field(what, key, "a text", other)),
    }
}

/// A list of texts; `None` when absent or `null`.
fn optional_texts(what: &str, map: &BTreeMap<String, &Value>, key: &str) -> Result<Option<Vec<String>>, ExtError> {
    match field(map, key) {
        Value::Null => Ok(None),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().map(str::to_string).ok_or_else(|| bad_field(what, key, "a list of texts", field(map, key))))
            .collect::<Result<_, _>>()
            .map(Some),
        other => Err(bad_field(what, key, "a list of texts", other)),
    }
}

/// A field read through its `flm-core` type's serde form (`expected` says
/// what that form is); `None` when absent or `null`.
fn optional_serde<T: serde::de::DeserializeOwned>(
    what: &str,
    map: &BTreeMap<String, &Value>,
    key: &str,
    expected: &str,
) -> Result<Option<T>, ExtError> {
    match field(map, key) {
        Value::Null => Ok(None),
        other => serde_json::from_value(other.clone()).map(Some).map_err(|error| {
            ExtError::Value { detail: format!("the field `{key}` of {what} must be {expected}, not `{other}`: {error}") }
        }),
    }
}

/// `{ name, default_style, docs?, within?, nesting? }`, through
/// [`CounterDecl::new`] and the builder methods.
fn counter_decl(value: &Value) -> Result<(String, SlotValue), ExtError> {
    const WHAT: &str = "a counter declaration";
    let map = fields(WHAT, value, &["name", "default_style", "docs", "within", "nesting"])?;
    let name = required_text(WHAT, &map, "name")?;
    let mut decl = CounterDecl::new(name.as_str(), required_text(WHAT, &map, "default_style")?);
    if let Some(docs) = optional_text(WHAT, &map, "docs")? {
        decl = decl.docs(docs);
    }
    if let Some(within) = optional_serde::<Within>(WHAT, &map, "within", "a place within counters (`Within`)")? {
        decl = decl.within(within);
    }
    if let Some(nesting) = optional_serde::<WithinJoinerSpec>(WHAT, &map, "nesting", "a within-joiner (`WithinJoinerSpec`)")? {
        decl = decl.nesting(nesting);
    }
    Ok((name, SlotValue::Counter(decl)))
}

/// `{ name, … }`, through [`CounterStyleDecl::new`], then each field given
/// read through its own serde form.
fn counter_style_decl(value: &Value) -> Result<(String, SlotValue), ExtError> {
    const WHAT: &str = "a counter style declaration";
    let map = fields(
        WHAT,
        value,
        &[
            "name",
            "number_format",
            "sub_levels",
            "prefix_words",
            "delimiters",
            "join",
            "name_in_link",
            "repeat_parents_in_range",
            "ranges",
            "number_writer",
        ],
    )?;
    let name = required_text(WHAT, &map, "name")?;
    let mut decl = CounterStyleDecl::new(name.as_str());
    if let Some(format) = optional_serde(WHAT, &map, "number_format", "a number template (`NumberTemplate`)")? {
        decl.number_format = format;
    }
    if let Some(levels) = optional_serde(WHAT, &map, "sub_levels", "a list of sub-level declarations (`SubLevelDecl`)")? {
        decl.sub_levels = levels;
    }
    if let Some(words) = optional_serde(WHAT, &map, "prefix_words", "prefix words (`PrefixWords`)")? {
        decl.prefix_words = words;
    }
    if let Some(delimiters) = optional_serde(WHAT, &map, "delimiters", "delimiters (`Delimiters`)")? {
        decl.delimiters = delimiters;
    }
    if let Some(join) = optional_serde(WHAT, &map, "join", "a join declaration (`JoinDecl`)")? {
        decl.join = join;
    }
    if let Some(flag) = optional_serde(WHAT, &map, "name_in_link", "a boolean")? {
        decl.name_in_link = flag;
    }
    if let Some(flag) = optional_serde(WHAT, &map, "repeat_parents_in_range", "a boolean")? {
        decl.repeat_parents_in_range = flag;
    }
    if let Some(flag) = optional_serde(WHAT, &map, "ranges", "a boolean")? {
        decl.ranges = flag;
    }
    if let Some(writer) = optional_text(WHAT, &map, "number_writer")? {
        decl = decl.with_number_writer(writer);
    }
    Ok((name, SlotValue::Style(Box::new(decl))))
}

/// `{ name, counters: [name] }`, through [`ParentSetDecl::new`].
fn parent_set_decl(value: &Value) -> Result<(String, SlotValue), ExtError> {
    const WHAT: &str = "a parent set declaration";
    let map = fields(WHAT, value, &["name", "counters"])?;
    let name = required_text(WHAT, &map, "name")?;
    let Some(counters) = optional_texts(WHAT, &map, "counters")? else {
        return Err(ExtError::Value { detail: format!("{WHAT} must have the field `counters`") });
    };
    Ok((name.clone(), SlotValue::ParentSet(ParentSetDecl::new(name, counters))))
}

/// `{ name, content, docs?, requires? }`, through [`LatexPreambleDef::new`]
/// and [`LatexPreambleDef::requires`].
fn preamble_def(value: &Value) -> Result<(String, SlotValue), ExtError> {
    const WHAT: &str = "a preamble piece";
    let map = fields(WHAT, value, &["name", "content", "docs", "requires"])?;
    let name = required_text(WHAT, &map, "name")?;
    let docs = optional_text(WHAT, &map, "docs")?.unwrap_or_default();
    let mut def = LatexPreambleDef::new(&name, required_text(WHAT, &map, "content")?, docs);
    if let Some(ids) = optional_texts(WHAT, &map, "requires")? {
        def = def.requires(ids.into_iter().map(Name::from));
    }
    Ok((name, SlotValue::Preamble(def)))
}

/// `{ id, package, options, docs }`, through [`LatexPackageDef`]'s serde
/// form, then [`LatexPackageDef::new`] and the builder methods, so that the
/// text is dropped when `flm-core` is built without its cargo feature `docs`.
fn package_def(value: &Value) -> Result<(String, SlotValue), ExtError> {
    let read: LatexPackageDef = serde_json::from_value(value.clone()).map_err(|error| ExtError::Value {
        detail: format!("a package definition must be a map `{{\"id\", \"package\", \"options\", \"docs\"}}`, not `{value}`: {error}"),
    })?;
    let def = LatexPackageDef::new(read.package, read.docs).id(read.id).options(read.options).engines(read.engines);
    Ok((def.id.to_string(), SlotValue::Package(def)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_counter_reads_through_its_builder_methods() {
        let (what, name, value) = read(
            COUNTERS_SLOT,
            &json!({ "name": "x.c", "default_style": "x.s", "docs": "d", "within": { "parents": { "one": "x.p" }, "joiner": { "joined": "." }, "no_restart": false } }),
        )
        .expect("a data-valued slot")
        .expect("reads");
        assert_eq!((what, name.as_str()), ("counter", "x.c"));
        let SlotValue::Counter(decl) = value else { panic!("a counter") };
        assert_eq!(decl, CounterDecl::new("x.c", "x.s").docs("d").within(Within::parent("x.p")));
    }

    #[test]
    fn a_slot_outside_the_table_is_none() {
        assert!(read("flm.numbering.within-joiners", &json!({})).is_none());
    }

    #[test]
    fn a_bad_field_is_named() {
        let error = read(PARENT_SETS_SLOT, &json!({ "name": "x.p", "counters": "x.a" })).expect("a slot").unwrap_err();
        assert_eq!(error.to_string(), "the field `counters` of a parent set declaration must be a list of texts, not `\"x.a\"`");
    }
}
