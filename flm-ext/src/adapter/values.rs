//! The JSON shapes of the values that cross the boundary: one
//! [`JsonForm`] implementation — a `to_value` and `from_value` pair — per type.
//!
//! A value crosses as a [`serde_json::Value`]. Every crossing type whose JSON
//! shape holds no handle (pure data) has its [`JsonForm`] pair here, both
//! directions in one place, since the host writes what a WebAssembly guest
//! reads and reads what it writes (the guest client, `flm_ext::guest`, uses
//! these same pairs): the scalars and the containers (`Option`, `Vec`, pairs),
//! the enums with one string per variant, the declarations, and the types
//! with a serde form of their own in both directions (through
//! `serde_form!`). Where the host had one direction as a function (a
//! `*_from_value` or a `*_to_value`), the function is one half of the pair or
//! its thin wrapper, its texts unchanged; the readers of the slot values
//! (`adapter/slots.rs`), of the reference points' parts (`adapter/aspects.rs`)
//! and the projections of the LaTeX payloads (`flm-core`) stay where they are,
//! and the pairs here call them.
//!
//! The declarations whose shape can hold a handle (an argument list, a
//! syntax, an element, an element declaration, a fallback, a user
//! definition) are read by reader functions (`*_from_value`) rather than a
//! [`JsonForm`] pair; an `Expand` aspect inside one is a handle, which the
//! reader's caller resolves. Beside the readers sit
//! the conversions of the extension API's declarations into `flm-core`'s, which
//! the registration makes (`element_def_to_core`, `fallback_def_to_core`),
//! the rules every data value obeys ([`check_data`]), and the conversion of a
//! handler's diagnostic into `flm-core`'s ([`diagnostic_to_core`]).
//!
//! The values that hold handles are read and written with the running call's
//! [`CallTable`] at hand: a handle is `{"@handle": {"kind", "call", "index"}}`
//! ([`handle_to_value`]); an object a value carries is lent into the table
//! when it is written (a node of a walk, a table read by name, a staged node),
//! and looked up in it when it is read (the node a diagnostic stands at, the
//! hook of an `Expand` aspect, a declaration a helper made, the staged nodes
//! of an expansion).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use flm_core::lang::mint::{
    ArgCodeError, ArgData, ArgDef, ArgLayout, ArgListData, BodyDef, BodyLayout, CallableKind, ElementDef as CoreElementDef,
    FallbackDef as CoreFallbackDef, FallbackKind, FlmBodyParser, SyntaxDef as CoreSyntaxDef,
};
use flm_core::lang::origin::{FlmDiagnostic, FlmSpan};
use flm_core::lang::verbatim::FlmVerbatimBehavior;
use flm_core::lang::rewrites::userdefs::{
    Definition, DefinitionKind, DefinitionModes, Expand as CoreExpand, ExpandPolicy,
    UserDefinition as CoreUserDefinition,
};
use flm_core::model::aspects::{
    BlockRole, PointId, BlockRoleAspect, InlineSpacing, Placement, Referenceable as CoreReferenceable, Sectioning as CoreSectioning,
};
use flm_core::model::element::ElementSpec as CoreElementSpec;
use flm_core::model::addr::NodeAddr;
use flm_core::core::artifact::RelPath;
use flm_core::core::registry::DefContent;
use flm_core::model::names::{CapName, FeatureName, Name};
use flm_core::techy::error::DiagnosticValue;

use crate::adapter::aspects::{ExtReferenceable, ExtSectioning};
use crate::adapter::diag::ExtCondition;
use crate::adapter::durable::{DurableTemplateFacade, DurableToken};
use crate::adapter::expand::ExtExpand;
use crate::adapter::handles::{CallTable, Entry, Handle, HandleKind, TableEntry};
use crate::api::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload, LatexEnvironmentPayload, LatexMacroPayload, LatexPackageDef,
    LatexPreambleDef, LatexRawPayload,
};
use crate::api::stdcore::features::numbering::{
    CounterDecl, CounterStyleDecl, Delimiters as NumberDelimiters, NumberTemplate, ParentSetDecl, Parents, PluralWords,
    PrefixWords, Within, WithinJoinerSpec,
};
use crate::adapter::slots::{self, SlotValue};
use flm_core::dump::json::{from_json, to_json};
use flm_core::lang::PresPayload;
use flm_core::stdcore::backends::latex::preamble::ID_PREFIX as PREAMBLE_ID_PREFIX;
use crate::api::stdcore::features::numbering::{CounterRecord, FormatOptions, NumberSegment};
use crate::api::core::data::table::TableValue;
use crate::api::core::artifact::{Artifact, ArtifactData, ArtifactId};
use flm_core::core::artifact::ArtifactSet;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use crate::api::core::registry::{
    BackendDef, BackendStage, Contributed, LibraryStage, OutputDef, ScriptDef, StandardTable, StyleDef,
};
use crate::api::core::services::Arity;
use crate::api::host::{Files, FilesScope, HostHandle, HostService};
use crate::api::stdcore::features::citations::{
    CitationAnswer, CitationEntry, CitationProviderError, CitationRequest, CitationResult, EntryPiece,
};
use crate::api::stdcore::features::compose::{Accepts, PieceValue, TemplateContext, TemplateDescription, TemplateFile};
use crate::adapter::citations::CitationEntryFacade;
use crate::api::core::lowering::{DefaultMark, Lower, LoweringDecl, Lowering, PresTree, Staged};
use crate::api::core::run::WalkEntry;
use crate::api::stdcore::features::math::{equation_lines, EquationLine, LineFrame, LineShows, NormalizedLine};
use crate::api::core::stage::{Effect, Needs, RewriteDecl, RewriteStage, Scope, Stage, StageDecl};
use crate::api::techy::transform::{Restage, RestagedArgument, RestagedSlot};
use crate::api::lang::mint::{ElementDef, ElementDefForm, FallbackDef, Mode, ScopedPackageDef, SyntaxDef};
use crate::api::core::lowering::LoweringRule;
use crate::api::lang::rewrites::userdefs::{
    CompiledTemplate, Expand, Expansion, ModeVariant, PlaceholderBindings, PlaceholderVocabulary, TemplateLanguage,
    TemplateSpec, UserDefinition, ValueBindings,
};
use crate::api::techy::extract::{KeyValEntry, KeyVals};
use crate::api::lang::text::{CharsContent, NotCharacters};
use crate::api::model::aspects::{AcceptedRefTypes, FixedBlockRole, Label, MalformedLabel, Referenceable, Sectioning};
use crate::api::core::registry::{Finalizer, LateInit};
use crate::api::stdcore::features::refs::LabelUse;
use crate::api::model::element::{Aspect, ElementSpec};
use crate::api::techy::error::{Diagnostic, Severity};
use crate::api::techy::node::{Node, NodeKindName, Span};
use crate::sealed::Token;
use crate::adapter::names::Owner;
use crate::runtime::ExtError;

/// A type's JSON shape: how a value of it is written as JSON, and read back.
pub trait JsonForm: Sized {
    /// The value written as JSON.
    fn to_value(&self) -> Value;

    /// The value read from JSON.
    ///
    /// # Errors
    ///
    /// `value` is not of the type's shape ([`ExtError::Value`]).
    fn from_value(value: &Value) -> Result<Self, ExtError>;
}

/// The error for a value that is none of the strings `expected`.
fn not_one_of(what: &str, expected: &[&str], value: &Value) -> ExtError {
    let list: Vec<String> = expected.iter().map(|text| format!("`{text}`")).collect();
    let choices = match list.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    };
    // A string is written without its JSON quotes; any other value as JSON.
    let given = match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    ExtError::Value { detail: format!("{what} must be one of {choices}, not `{given}`") }
}

/// The error for a value that does not read as `what`.
fn not_read(what: &str, error: &serde_json::Error) -> ExtError {
    ExtError::Value { detail: format!("{what}: {error}") }
}

/// Reads `value` as `T` through `T`'s own serde form.
fn read<T: serde::de::DeserializeOwned>(what: &str, value: &Value) -> Result<T, ExtError> {
    serde_json::from_value(value.clone()).map_err(|error| not_read(what, &error))
}

/// The error for a value that is not `expected` (a scalar's or a
/// container's shape).
fn not_a(expected: &str, value: &Value) -> ExtError {
    ExtError::Value { detail: format!("the value must be {expected}, not `{value}`") }
}

// --- the scalars and the containers --------------------------------------------------------

/// A string.
impl JsonForm for String {
    fn to_value(&self) -> Value {
        Value::String(self.clone())
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        value.as_str().map(str::to_string).ok_or_else(|| not_a("a text", value))
    }
}

/// `true` or `false`.
impl JsonForm for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        value.as_bool().ok_or_else(|| not_a("`true` or `false`", value))
    }
}

/// A number from 0 up.
impl JsonForm for usize {
    fn to_value(&self) -> Value {
        json!(self)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        value.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| not_a("a number from 0 up", value))
    }
}

/// A number from 0 up, below 2³².
impl JsonForm for u32 {
    fn to_value(&self) -> Value {
        json!(self)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        value.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| not_a("a number from 0 up, below 2³²", value))
    }
}

/// Data, as it is.
impl JsonForm for Value {
    fn to_value(&self) -> Value {
        self.clone()
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        Ok(value.clone())
    }
}

/// `null` for `None`, else the value's own form.
impl<T: JsonForm> JsonForm for Option<T> {
    fn to_value(&self) -> Value {
        self.as_ref().map_or(Value::Null, JsonForm::to_value)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match value {
            Value::Null => Ok(None),
            value => T::from_value(value).map(Some),
        }
    }
}

/// A list, each item in its own form.
impl<T: JsonForm> JsonForm for Vec<T> {
    fn to_value(&self) -> Value {
        Value::Array(self.iter().map(JsonForm::to_value).collect())
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let Value::Array(items) = value else { return Err(not_a("a list", value)) };
        items.iter().map(T::from_value).collect()
    }
}

/// A list of the two items, each in its own form.
impl<A: JsonForm, B: JsonForm> JsonForm for (A, B) {
    fn to_value(&self) -> Value {
        Value::Array(vec![self.0.to_value(), self.1.to_value()])
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match value.as_array().map(Vec::as_slice) {
            Some([first, second]) => Ok((A::from_value(first)?, B::from_value(second)?)),
            _ => Err(not_a("a list of two items", value)),
        }
    }
}

/// A type with a serde form of its own in both directions, written and read
/// through it (`what` names the type in the errors).
macro_rules! serde_form {
    ($($type:ty => $what:literal),+ $(,)?) => {
        $(
            impl JsonForm for $type {
                fn to_value(&self) -> Value {
                    serde_json::to_value(self).unwrap_or(Value::Null)
                }

                fn from_value(value: &Value) -> Result<Self, ExtError> {
                    read($what, value)
                }
            }
        )+
    };
}

serde_form!(
    Within => "a place within counters (`Within`)",
    Parents => "the parents of a counter (`Parents`)",
    WithinJoinerSpec => "a within-joiner (`WithinJoinerSpec`)",
    PrefixWords => "prefix words (`PrefixWords`)",
    PluralWords => "plural words (`PluralWords`)",
    NumberDelimiters => "delimiters (`Delimiters`)",
    NumberTemplate => "a number template (`NumberTemplate`)",
);

/// A unit enum written as one string per variant.
macro_rules! string_form {
    ($type:ty, $what:literal, { $($variant:path => $text:literal),+ $(,)? }) => {
        impl JsonForm for $type {
            fn to_value(&self) -> Value {
                Value::String(match self { $($variant => $text),+ }.to_string())
            }

            fn from_value(value: &Value) -> Result<Self, ExtError> {
                match value.as_str() {
                    $(Some($text) => Ok($variant),)+
                    _ => Err(not_one_of($what, &[$($text),+], value)),
                }
            }
        }
    };
}

string_form!(Effect, "an effect", { Effect::Compute => "compute", Effect::Check => "check", Effect::Io => "io" });
string_form!(Scope, "a scope", { Scope::PerTree => "per-tree", Scope::Document => "document" });
string_form!(CallableKind, "a callable kind", {
    CallableKind::Macro => "macro",
    CallableKind::Environment => "environment",
    CallableKind::Specials => "specials",
});
string_form!(FallbackKind, "a fallback kind", { FallbackKind::Macro => "macro", FallbackKind::Environment => "environment" });
string_form!(DefinitionKind, "a definition kind", {
    DefinitionKind::Macro => "macro",
    DefinitionKind::Environment => "environment",
    DefinitionKind::Specials => "specials",
});
string_form!(DefinitionModes, "a definition's modes", {
    DefinitionModes::Both => "both",
    DefinitionModes::Text => "text",
    DefinitionModes::Math => "math",
});
string_form!(ArgLayout, "an argument layout", {
    ArgLayout::Inline => "inline",
    ArgLayout::Blocks => "blocks",
    ArgLayout::InlineOrBlocks => "inline-or-blocks",
});
string_form!(BodyLayout, "a body layout", {
    BodyLayout::Inline => "inline",
    BodyLayout::Blocks => "blocks",
    BodyLayout::FromRole => "from-role",
});
string_form!(ExpandPolicy, "an expansion policy", { ExpandPolicy::Always => "always", ExpandPolicy::PerPlan => "per-plan" });
string_form!(Placement, "a placement", { Placement::Leads => "leads", Placement::Trails => "trails", Placement::Free => "free" });
string_form!(NodeKindName, "a node kind", {
    NodeKindName::Chars => "chars",
    NodeKindName::Group => "group",
    NodeKindName::Callable => "callable",
    NodeKindName::Comment => "comment",
    NodeKindName::List => "list",
});
string_form!(Severity, "a severity", { Severity::Error => "error", Severity::Warning => "warning", Severity::Note => "note" });
string_form!(StandardTable, "a standard table type", {
    StandardTable::LoweredTrees => "lowered-trees",
    StandardTable::LoweredPieces => "lowered-pieces",
    StandardTable::Regions => "regions",
    StandardTable::ArtifactSet => "artifact-set",
});
string_form!(Arity, "an arity", { Arity::Single => "single", Arity::OrderedMany => "ordered-many" });
string_form!(FilesScope, "the scope of a file read", { FilesScope::Extension => "extension", FilesScope::Document => "document" });

/// `"text"` or `"math"`: the mode's own serde form.
impl JsonForm for Mode {
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        serde_json::from_value(value.clone()).map_err(|_| not_one_of("a mode", &["text", "math"], value))
    }
}

/// `{"absorb_before": false, "absorb_after": false}`: the serde form
/// `InlineSpacing` reads.
impl JsonForm for InlineSpacing {
    fn to_value(&self) -> Value {
        json!({ "absorb_before": self.absorb_before, "absorb_after": self.absorb_after })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        read("an inline spacing", value)
    }
}

/// `"block"`, `{"inline": <InlineSpacing>}` or `{"block-decorator":
/// <Placement>}`: the serde form `BlockRole` reads.
impl JsonForm for BlockRole {
    fn to_value(&self) -> Value {
        match self {
            BlockRole::Block => Value::String("block".to_string()),
            BlockRole::Inline(spacing) => json!({ "inline": spacing.to_value() }),
            BlockRole::BlockDecorator(placement) => json!({ "block-decorator": placement.to_value() }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        read("a block role", value)
    }
}

/// `{"name", "effect", "scope", "provides": [], "writes": [], "docs"}`; the
/// three lists and the docs may be left out.
impl JsonForm for StageDecl {
    fn to_value(&self) -> Value {
        json!({
            "name": self.name,
            "effect": self.effect.to_value(),
            "scope": self.scope.to_value(),
            "provides": self.provides,
            "writes": self.writes,
            "docs": self.docs,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Form {
            name: String,
            effect: Value,
            scope: Value,
            #[serde(default)]
            provides: Vec<String>,
            #[serde(default)]
            writes: Vec<String>,
            #[serde(default)]
            docs: String,
        }
        let form: Form = read("a stage declaration", value)?;
        Ok(StageDecl {
            name: form.name,
            effect: Effect::from_value(&form.effect)?,
            scope: Scope::from_value(&form.scope)?,
            provides: form.provides,
            writes: form.writes,
            docs: form.docs,
        })
    }
}

/// `{"requires": [], "reads": [], "reads_opt": [], "after": [], "before": [],
/// "options": []}`; any list may be left out.
impl JsonForm for Needs {
    fn to_value(&self) -> Value {
        json!({
            "requires": self.requires,
            "reads": self.reads,
            "reads_opt": self.reads_opt,
            "after": self.after,
            "before": self.before,
            "options": self.options,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Form {
            #[serde(default)]
            requires: Vec<String>,
            #[serde(default)]
            reads: Vec<String>,
            #[serde(default)]
            reads_opt: Vec<String>,
            #[serde(default)]
            after: Vec<String>,
            #[serde(default)]
            before: Vec<String>,
            #[serde(default)]
            options: Vec<String>,
        }
        let form: Form = read("a stage's needs", value)?;
        Ok(Needs {
            requires: form.requires,
            reads: form.reads,
            reads_opt: form.reads_opt,
            after: form.after,
            before: form.before,
            options: form.options,
        })
    }
}

/// `{"name", "publishes", "refines", "default", "docs"}`; `refines` is a
/// name or `null`, and `refines`, `default` and `docs` may be left out.
impl JsonForm for RewriteDecl {
    fn to_value(&self) -> Value {
        json!({
            "name": self.name,
            "publishes": self.publishes,
            "refines": self.refines,
            "default": self.default,
            "docs": self.docs,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Form {
            name: String,
            publishes: String,
            #[serde(default)]
            refines: Option<String>,
            #[serde(default)]
            default: bool,
            #[serde(default)]
            docs: String,
        }
        let form: Form = read("a rewrite stage's declaration", value)?;
        Ok(RewriteDecl {
            name: form.name,
            publishes: form.publishes,
            refines: form.refines,
            default: form.default,
            docs: form.docs,
        })
    }
}

// --- the rules every data value obeys ------------------------------------------------------

/// The map key that marks a handle in JSON; a data map may not hold it.
pub const HANDLE_KEY: &str = "@handle";

/// The map key that marks a durable token in JSON
/// ([`DurableToken`]); a data map may
/// not hold it.
pub const DURABLE_KEY: &str = "@durable";

/// Checks that `value` can cross the boundary as data: every number an
/// integer between `i64::MIN` and `i64::MAX`, no map key beginning with `$`
/// (techy's value model reserves the prefix), no map key [`HANDLE_KEY`].
///
/// # Errors
///
/// [`ExtError::Value`] stating the first value that breaks a rule.
pub fn check_data(value: &Value) -> Result<(), ExtError> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) => {
            if number.is_i64() {
                Ok(())
            } else if number.is_u64() {
                Err(ExtError::Value {
                    detail: format!("a number must lie between {} and {}, not `{number}`", i64::MIN, i64::MAX),
                })
            } else {
                Err(ExtError::Value { detail: format!("a number must be an integer, not `{number}`") })
            }
        }
        Value::Array(items) => items.iter().try_for_each(check_data),
        Value::Object(map) => {
            for (key, item) in map {
                check_key(key)?;
                check_data(item)?;
            }
            Ok(())
        }
    }
}

/// Checks a map key of data: it does not begin with `$` and is not
/// [`HANDLE_KEY`].
fn check_key(key: &str) -> Result<(), ExtError> {
    if key.starts_with('$') {
        return Err(ExtError::Value { detail: format!("a map key must not begin with `$`, as `{key}` does") });
    }
    if key == HANDLE_KEY {
        return Err(ExtError::Value { detail: format!("the map key `{HANDLE_KEY}` is reserved") });
    }
    if key == DURABLE_KEY {
        return Err(ExtError::Value { detail: format!("the map key `{DURABLE_KEY}` is reserved") });
    }
    Ok(())
}

/// `value` as a diagnostic's data, after [`check_data`].
///
/// # Errors
///
/// As [`check_data`].
pub fn diagnostic_value(value: &Value) -> Result<DiagnosticValue, ExtError> {
    check_data(value)?;
    Ok(to_diagnostic_value(value))
}

/// The conversion behind [`diagnostic_value`], on a value already checked.
fn to_diagnostic_value(value: &Value) -> DiagnosticValue {
    match value {
        Value::Null => DiagnosticValue::Null,
        Value::Bool(flag) => DiagnosticValue::Bool(*flag),
        // Checked: an `i64`.
        Value::Number(number) => number.as_i64().map_or(DiagnosticValue::Null, DiagnosticValue::Int),
        Value::String(text) => DiagnosticValue::Str(text.clone()),
        Value::Array(items) => DiagnosticValue::List(items.iter().map(to_diagnostic_value).collect()),
        Value::Object(map) => {
            DiagnosticValue::Map(map.iter().map(|(key, item)| (key.clone(), to_diagnostic_value(item))).collect())
        }
    }
}

/// A diagnostic's data as JSON: the inverse of [`diagnostic_value`], for the
/// data of an `flm-core` condition that crosses inside an
/// [`ExtError::Core`].
pub fn diagnostic_value_to_json(value: &DiagnosticValue) -> Value {
    match value {
        DiagnosticValue::Null => Value::Null,
        DiagnosticValue::Bool(flag) => Value::Bool(*flag),
        DiagnosticValue::Int(number) => Value::from(*number),
        DiagnosticValue::Str(text) => Value::String(text.clone()),
        DiagnosticValue::List(items) => Value::Array(items.iter().map(diagnostic_value_to_json).collect()),
        DiagnosticValue::Map(entries) => {
            Value::Object(entries.iter().map(|(key, item)| (key.clone(), diagnostic_value_to_json(item))).collect())
        }
    }
}

/// A handler's diagnostic as `flm-core`'s: the condition an [`ExtCondition`]
/// of the diagnostic's identifier, message and data, at the severity given,
/// placed at the diagnostic's own span if it has one, else at `default_span`.
/// The name rule is the caller's, which knows the package.
///
/// # Errors
///
/// The diagnostic's data cannot cross ([`ExtError::Value`]).
pub fn diagnostic_to_core(diagnostic: Diagnostic, default_span: FlmSpan) -> Result<FlmDiagnostic, ExtError> {
    let condition = ExtCondition::new(diagnostic.id, diagnostic.message, diagnostic.data)?;
    Ok(FlmDiagnostic::new(diagnostic.severity, condition, diagnostic.at.unwrap_or(default_span)))
}

// --- the declarations, read from JSON --------------------------------------------------------

/// An object a declaration holds as a handle, as the running call's table
/// lent it: an element's hook or handler aspect, or an argument a helper made
/// ([`label_fields_arg`](crate::api::stdcore::features::refs::label_fields_arg)).
pub enum Held {
    /// An expansion hook (the kind `expand`).
    Expand(Arc<dyn Expand>),
    /// A reference-points aspect (the kind `referenceable`).
    Referenceable(Arc<dyn Referenceable>),
    /// A sectioning aspect (the kind `sectioning`).
    Sectioning(Arc<dyn Sectioning>),
    /// An argument a helper made (the kind `arg_def`).
    ArgDef(ArgDef),
}

impl Held {
    /// The kind of handle that stood for the object.
    pub fn kind(&self) -> HandleKind {
        match self {
            Held::Expand(_) => HandleKind::Expand,
            Held::Referenceable(_) => HandleKind::Referenceable,
            Held::Sectioning(_) => HandleKind::Sectioning,
            Held::ArgDef(_) => HandleKind::ArgDef,
        }
    }
}

/// What resolves a handle written in a declaration (an element's aspect, an
/// argument a helper made): the dispatcher's lookup in the running call's
/// table ([`held_from_table`]).
pub type Resolver<'r> = dyn FnMut(&Value) -> Result<Held, ExtError> + 'r;

/// The resolver of a declaration read without a call: every handle is
/// refused.
///
/// # Errors
///
/// Always ([`ExtError::Argument`]).
pub fn no_handles(value: &Value) -> Result<Held, ExtError> {
    Err(ExtError::Argument {
        method: String::new(),
        detail: format!("a declaration read outside a call cannot hold an object, not `{value}`"),
    })
}

/// A map's fields, each a JSON value, refusing a field not in `known` and
/// reading a missing one as `null`.
pub(crate) fn fields<'v>(what: &str, value: &'v Value, known: &[&str]) -> Result<BTreeMap<String, &'v Value>, ExtError> {
    let Value::Object(map) = value else {
        return Err(ExtError::Value { detail: format!("{what} must be a map, not `{value}`") });
    };
    let mut out = BTreeMap::new();
    for (key, item) in map {
        if !known.contains(&key.as_str()) {
            let list: Vec<String> = known.iter().map(|k| format!("`{k}`")).collect();
            return Err(ExtError::Value {
                detail: format!("{what} must hold only the fields {}, not `{key}`", list.join(", ")),
            });
        }
        out.insert(key.clone(), item);
    }
    Ok(out)
}

/// The field `key` of `map`, `null` when absent.
pub(crate) fn field<'v>(map: &BTreeMap<String, &'v Value>, key: &str) -> &'v Value {
    map.get(key).copied().unwrap_or(&Value::Null)
}

/// A text field: a string, or `default` when absent or `null`.
fn text_field(what: &str, map: &BTreeMap<String, &Value>, key: &str, default: &str) -> Result<String, ExtError> {
    match field(map, key) {
        Value::Null => Ok(default.to_string()),
        Value::String(text) => Ok(text.clone()),
        other => Err(ExtError::Value { detail: format!("the field `{key}` of {what} must be a string, not `{other}`") }),
    }
}

/// A required text field.
pub(crate) fn required_text(what: &str, map: &BTreeMap<String, &Value>, key: &str) -> Result<String, ExtError> {
    match field(map, key) {
        Value::String(text) => Ok(text.clone()),
        Value::Null => Err(ExtError::Value { detail: format!("{what} must have the field `{key}`") }),
        other => Err(ExtError::Value { detail: format!("the field `{key}` of {what} must be a string, not `{other}`") }),
    }
}

/// A field read through a [`JsonForm`], absent being `null`; an error of its
/// form names the field (`what` names the map).
fn required_form<T: JsonForm>(what: &str, map: &BTreeMap<String, &Value>, key: &str) -> Result<T, ExtError> {
    T::from_value(field(map, key)).map_err(|error| match error {
        ExtError::Value { detail } => ExtError::Value { detail: format!("in the field `{key}` of {what}, {detail}") },
        other => other,
    })
}

/// A field read through a [`JsonForm`], or `None` when absent or `null`.
fn form_field<T: JsonForm>(map: &BTreeMap<String, &Value>, key: &str) -> Result<Option<T>, ExtError> {
    match field(map, key) {
        Value::Null => Ok(None),
        other => T::from_value(other).map(Some),
    }
}

/// A list of modes, or `None` when absent or `null`: the caller decides what
/// an absent list means (an element declaration keeps its default, text mode;
/// a fallback requires the field).
fn modes_field(what: &str, map: &BTreeMap<String, &Value>, key: &str) -> Result<Option<Vec<Mode>>, ExtError> {
    match field(map, key) {
        Value::Null => Ok(None),
        Value::Array(items) => items.iter().map(Mode::from_value).collect::<Result<_, _>>().map(Some),
        other => Err(ExtError::Value { detail: format!("the field `{key}` of {what} must be a list of modes, not `{other}`") }),
    }
}

/// A list of names, empty when absent or `null`.
fn names_field(what: &str, map: &BTreeMap<String, &Value>, key: &str) -> Result<Vec<Name>, ExtError> {
    match field(map, key) {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str().map(Name::from).ok_or_else(|| ExtError::Value {
                    detail: format!("the field `{key}` of {what} must be a list of strings, not `{}`", field(map, key)),
                })
            })
            .collect(),
        other => Err(ExtError::Value { detail: format!("the field `{key}` of {what} must be a list of strings, not `{other}`") }),
    }
}

/// An argument list in the `ArgListData` shape: a string of argument codes
/// (`"[{"`), or a list of codes, maps `{"code", "name", "layout", "docs",
/// "mode", "scoped", "role"}` and handles of kind `arg_def` (an argument a
/// helper made, resolved by `resolve`); `null` is the empty list.
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]; a helper's argument
/// written with fields beside its handle, as a guest writes one its setters
/// changed, is refused naming those fields), a code is not one
/// ([`ExtError::Core`] of kind `arg-code`), a handle stands for no argument,
/// or what `resolve` reports.
pub fn arg_defs_from_value(value: &Value, resolve: &mut Resolver<'_>) -> Result<Vec<ArgDef>, ExtError> {
    let code_error =
        |error: ArgCodeError| ExtError::Core { kind: "arg-code".to_string(), detail: error.to_string(), data: Value::Null };
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                if let Value::Object(map) = item
                    && map.len() > 1
                    && let Some(handle) = map.get(HANDLE_KEY)
                {
                    let fields: Vec<String> = map.keys().filter(|key| *key != HANDLE_KEY).map(|key| format!("`{key}`")).collect();
                    let arg = match resolve(&serde_json::json!({ HANDLE_KEY: handle }))? {
                        Held::ArgDef(arg) => arg,
                        other => return Err(mismatch(HandleKind::ArgDef, other.kind())),
                    };
                    let what = match &arg.name {
                        Some(name) => format!("the argument `{name}` that a helper made"),
                        None => "an argument a helper made".to_string(),
                    };
                    return Err(ExtError::Value {
                        detail: format!(
                            "{what} must be used as the helper made it, without the fields {} set on it",
                            fields.join(", ")
                        ),
                    });
                }
                if is_handle(item) {
                    return match resolve(item)? {
                        Held::ArgDef(arg) => Ok(arg),
                        other => Err(mismatch(HandleKind::ArgDef, other.kind())),
                    };
                }
                let data: ArgData = read("an argument", item)?;
                ArgDef::try_from(data).map_err(code_error)
            })
            .collect(),
        _ => {
            let data: ArgListData = read("an argument list", value)?;
            data.arg_defs().map_err(code_error)
        }
    }
}

/// A body `{"layout": "from-role", "verbatim": false, "mode": null,
/// "scoped": []}`, every field optional; `scoped` names the scoped packages
/// whose commands are legal in the body.
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]).
pub fn body_def_from_value(value: &Value) -> Result<BodyDef, ExtError> {
    const WHAT: &str = "a body";
    if value.is_null() {
        return Ok(BodyDef::new());
    }
    let map = fields(WHAT, value, &["layout", "verbatim", "mode", "scoped"])?;
    let mut body = BodyDef::new();
    if let Some(layout) = form_field::<BodyLayout>(&map, "layout")? {
        body = body.layout(layout);
    }
    match field(&map, "verbatim") {
        Value::Null | Value::Bool(false) => {}
        Value::Bool(true) => body = body.verbatim(),
        other => {
            return Err(ExtError::Value { detail: format!("the field `verbatim` of {WHAT} must be a boolean, not `{other}`") })
        }
    }
    if let Some(mode) = form_field::<Mode>(&map, "mode")? {
        body = body.mode(mode);
    }
    for package in names_field(WHAT, &map, "scoped")? {
        body = body.scoped(package);
    }
    Ok(body)
}

/// `{"layout", "verbatim", "mode", "scoped": [name, …]}`; read by
/// [`body_def_from_value`]. The one parser the shape names is the verbatim
/// one ([`BodyDef::verbatim`]), written `"verbatim": true`. A body with a
/// parser of its own is written with the parser's description under the
/// field `parser`, which the reader refuses: the write drops nothing, so such
/// a body never reads back as another one.
impl JsonForm for BodyDef {
    fn to_value(&self) -> Value {
        let scoped: Vec<&str> = self.scoped.iter().map(Name::as_str).collect();
        let verbatim = FlmVerbatimBehavior::new().identity();
        let mut value = json!({
            "layout": self.layout.to_value(),
            "verbatim": self.parser.as_ref().is_some_and(|parser| parser.identity() == verbatim),
            "mode": self.mode.to_value(),
            "scoped": scoped,
        });
        if let Some(parser) = self.parser.as_ref().filter(|parser| parser.identity() != verbatim) {
            value["parser"] = Value::String(parser.description());
        }
        value
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        body_def_from_value(value)
    }
}

/// A syntax: `{"macro": {"args"}}`, `{"environment": {"args", "body"}}`,
/// `{"specials": {"trigger", "args"}}` or `"synthesized"`.
///
/// # Errors
///
/// As [`arg_defs_from_value`] and [`body_def_from_value`].
pub fn syntax_def_from_value(value: &Value, resolve: &mut Resolver<'_>) -> Result<SyntaxDef, ExtError> {
    const WHAT: &str = "a syntax";
    if value.as_str() == Some("synthesized") {
        return Ok(SyntaxDef::Synthesized);
    }
    let shape = || ExtError::Value {
        detail: format!(
            "{WHAT} must be `\"synthesized\"` or a map of one of the keys `macro`, `environment`, `specials`, not `{value}`"
        ),
    };
    let Value::Object(map) = value else { return Err(shape()) };
    let mut entries = map.iter();
    let (Some((kind, inner)), None) = (entries.next(), entries.next()) else { return Err(shape()) };
    match kind.as_str() {
        "macro" => {
            let inner = fields("a macro's syntax", inner, &["args"])?;
            Ok(SyntaxDef::Macro { args: arg_defs_from_value(field(&inner, "args"), resolve)? })
        }
        "environment" => {
            let inner = fields("an environment's syntax", inner, &["args", "body"])?;
            Ok(SyntaxDef::Environment {
                args: arg_defs_from_value(field(&inner, "args"), resolve)?,
                body: body_def_from_value(field(&inner, "body"))?,
            })
        }
        "specials" => {
            const INNER: &str = "a specials' syntax";
            let inner = fields(INNER, inner, &["trigger", "args"])?;
            Ok(SyntaxDef::Specials {
                trigger: required_text(INNER, &inner, "trigger")?,
                args: arg_defs_from_value(field(&inner, "args"), resolve)?,
            })
        }
        _ => Err(shape()),
    }
}

/// A user definition: `{"kind", "name", "args", "content", "modes",
/// "defaults", "argument_number_offset", "block_role", "expand", "overrides",
/// "docs", "extra_placeholders"}`; `kind` and `name` are required, every
/// other field optional, with [`Definition::new`]'s defaults. Each extra
/// placeholder is `{"name", "substitution"}`, applied through
/// [`Definition::extra_placeholder`]. The arguments are read as
/// [`arg_defs_from_value`] reads them, `resolve` resolving their handles.
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); an argument code is
/// not one ([`ExtError::Core`]).
pub fn definition_from_value(value: &Value, resolve: &mut Resolver<'_>) -> Result<Definition, ExtError> {
    const WHAT: &str = "a definition";
    let map = fields(
        WHAT,
        value,
        &[
            "kind",
            "name",
            "args",
            "content",
            "modes",
            "defaults",
            "argument_number_offset",
            "block_role",
            "expand",
            "overrides",
            "docs",
            "extra_placeholders",
        ],
    )?;
    let kind = form_field::<DefinitionKind>(&map, "kind")?
        .ok_or_else(|| ExtError::Value { detail: format!("{WHAT} must have the field `kind`") })?;
    let name = required_text(WHAT, &map, "name")?;
    let mut definition = Definition::new(kind, name, arg_defs_from_value(field(&map, "args"), resolve)?);
    match field(&map, "content") {
        Value::Null => {}
        Value::String(text) => definition = definition.content(text.clone()),
        other => {
            return Err(ExtError::Value { detail: format!("the field `content` of {WHAT} must be a string, not `{other}`") })
        }
    }
    if let Some(modes) = form_field::<DefinitionModes>(&map, "modes")? {
        definition = definition.modes(modes);
    }
    match field(&map, "defaults") {
        Value::Null => {}
        Value::Object(defaults) => {
            for (key, template) in defaults {
                let Some(template) = template.as_str() else {
                    return Err(ExtError::Value {
                        detail: format!("the default `{key}` of {WHAT} must be a string, not `{template}`"),
                    });
                };
                definition = definition.default(key.clone(), template);
            }
        }
        other => {
            return Err(ExtError::Value { detail: format!("the field `defaults` of {WHAT} must be a map, not `{other}`") })
        }
    }
    match field(&map, "argument_number_offset") {
        Value::Null => {}
        other => match other.as_u64().and_then(|offset| usize::try_from(offset).ok()) {
            Some(offset) => definition = definition.argument_number_offset(offset),
            None => {
                return Err(ExtError::Value {
                    detail: format!(
                        "the field `argument_number_offset` of {WHAT} must be a number from 0 up, not `{other}`"
                    ),
                })
            }
        },
    }
    if let Some(role) = form_field::<BlockRole>(&map, "block_role")? {
        definition = definition.block_role(role);
    }
    if let Some(policy) = form_field::<ExpandPolicy>(&map, "expand")? {
        definition = definition.expand(policy);
    }
    match field(&map, "overrides") {
        Value::Null | Value::Bool(false) => {}
        Value::Bool(true) => definition = definition.overriding(),
        other => {
            return Err(ExtError::Value { detail: format!("the field `overrides` of {WHAT} must be a boolean, not `{other}`") })
        }
    }
    let docs = text_field(WHAT, &map, "docs", "")?;
    if !docs.is_empty() {
        definition = definition.docs(docs);
    }
    let extras: &[Value] = match field(&map, "extra_placeholders") {
        Value::Null => &[],
        Value::Array(items) => items,
        other => {
            return Err(ExtError::Value {
                detail: format!("the field `extra_placeholders` of {WHAT} must be a list, not `{other}`"),
            })
        }
    };
    for extra in extras {
        const EXTRA: &str = "an extra placeholder";
        let extra = fields(EXTRA, extra, &["name", "substitution"])?;
        let substitution = match field(&extra, "substitution") {
            Value::Null => false,
            Value::Bool(flag) => *flag,
            other => {
                return Err(ExtError::Value {
                    detail: format!("the field `substitution` of {EXTRA} must be a boolean, not `{other}`"),
                })
            }
        };
        definition = definition.extra_placeholder(required_text(EXTRA, &extra, "name")?, substitution);
    }
    Ok(definition)
}

/// An element: `{"name", "docs", "aspects": [{"BlockRoleAspect": <BlockRole>},
/// {"Expand": <handle>}, {"Referenceable": <handle>}, {"Sectioning":
/// <handle>}], "value"}`; `name` is required. `resolve` resolves a handler
/// aspect's handle (of the kind `expand`, `referenceable` or `sectioning`).
///
/// # Errors
///
/// The value is not of the shape, its data cannot cross
/// ([`ExtError::Value`]); an aspect kind is attached twice
/// ([`ExtError::Argument`]); a handle of another kind than its key names;
/// what `resolve` reports.
pub fn element_spec_from_value(value: &Value, resolve: &mut Resolver<'_>) -> Result<ElementSpec, ExtError> {
    const WHAT: &str = "an element";
    let map = fields(WHAT, value, &["name", "docs", "aspects", "value"])?;
    let mut spec = ElementSpec::new(required_text(WHAT, &map, "name")?).docs(text_field(WHAT, &map, "docs", "")?);
    let aspects: &[Value] = match field(&map, "aspects") {
        Value::Null => &[],
        Value::Array(items) => items,
        other => {
            return Err(ExtError::Value { detail: format!("the field `aspects` of {WHAT} must be a list, not `{other}`") })
        }
    };
    for aspect in aspects {
        let aspect = aspect_from_value(aspect, resolve)?;
        spec = spec.with(aspect)?;
    }
    match field(&map, "value") {
        Value::Null => {}
        data => {
            check_data(data)?;
            spec = spec.with_value(data.clone());
        }
    }
    Ok(spec)
}

/// An aspect: `{"BlockRoleAspect": <BlockRole>}`, `{"Expand": <handle>}`,
/// `{"Referenceable": <handle>}` or `{"Sectioning": <handle>}`, each handle
/// resolved by `resolve` and of the kind its key names (`expand`,
/// `referenceable`, `sectioning`).
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); a handle of another
/// kind; what `resolve` reports.
pub fn aspect_from_value(aspect: &Value, resolve: &mut Resolver<'_>) -> Result<Aspect, ExtError> {
    let shape = || ExtError::Value {
        detail: format!(
            "an aspect must be a map of one of the keys `BlockRoleAspect`, `Expand`, `Referenceable`, `Sectioning`, not \
             `{aspect}`"
        ),
    };
    let Value::Object(entry) = aspect else { return Err(shape()) };
    let mut entries = entry.iter();
    let (Some((kind, inner)), None) = (entries.next(), entries.next()) else { return Err(shape()) };
    Ok(match kind.as_str() {
        "BlockRoleAspect" => Aspect::BlockRoleAspect(FixedBlockRole(BlockRole::from_value(inner)?)),
        "Expand" => match resolve(inner)? {
            Held::Expand(hook) => Aspect::Expand(hook),
            other => return Err(mismatch(HandleKind::Expand, other.kind())),
        },
        "Referenceable" => match resolve(inner)? {
            Held::Referenceable(aspect) => Aspect::Referenceable(aspect),
            other => return Err(mismatch(HandleKind::Referenceable, other.kind())),
        },
        "Sectioning" => match resolve(inner)? {
            Held::Sectioning(aspect) => Aspect::Sectioning(aspect),
            other => return Err(mismatch(HandleKind::Sectioning, other.kind())),
        },
        _ => return Err(shape()),
    })
}

/// A scoped package: `{"name", "elements": [element declaration, …],
/// "docs"}`; `name` is required. Each element is read as
/// [`element_def_from_table`] reads one: its data form, or a handle of a
/// declaration a helper made.
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]), or an element does
/// not read.
pub fn scoped_package_def_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<ScopedPackageDef, ExtError> {
    const WHAT: &str = "a scoped package";
    let map = fields(WHAT, value, &["name", "elements", "docs"])?;
    let elements = match field(&map, "elements") {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().map(|item| element_def_from_table(table, item)).collect::<Result<_, _>>()?,
        other => {
            return Err(ExtError::Value { detail: format!("the field `elements` of {WHAT} must be a list, not `{other}`") })
        }
    };
    Ok(ScopedPackageDef::new(required_text(WHAT, &map, "name")?, elements, text_field(WHAT, &map, "docs", "")?))
}

/// An element declaration in its data form: `{"owner", "names", "syntax",
/// "element", "docs", "modes"}`; `syntax` and `element` are required. An
/// absent or `null` `modes` leaves the declaration at its default, text mode
/// ([`ElementDef::new`]). A
/// declaration a helper made crosses as a handle, which the dispatcher
/// resolves itself before it reaches this reader.
///
/// # Errors
///
/// As [`syntax_def_from_value`] and [`element_spec_from_value`].
pub fn element_def_from_value(value: &Value, resolve: &mut Resolver<'_>) -> Result<ElementDef, ExtError> {
    const WHAT: &str = "an element declaration";
    let map = fields(WHAT, value, &["owner", "names", "syntax", "element", "docs", "modes"])?;
    let names: Vec<String> = match field(&map, "names") {
        Value::Null => Vec::new(),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| ExtError::Value {
                    detail: format!("a name of {WHAT} must be a string, not `{item}`"),
                })
            })
            .collect::<Result<_, _>>()?,
        other => {
            return Err(ExtError::Value { detail: format!("the field `names` of {WHAT} must be a list, not `{other}`") })
        }
    };
    let syntax = match field(&map, "syntax") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `syntax`") }),
        syntax => syntax_def_from_value(syntax, resolve)?,
    };
    let element = match field(&map, "element") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `element`") }),
        element => element_spec_from_value(element, resolve)?,
    };
    let mut def = ElementDef::new(text_field(WHAT, &map, "owner", "")?, names, syntax, element)
        .docs(text_field(WHAT, &map, "docs", "")?);
    if let Some(modes) = modes_field(WHAT, &map, "modes")? {
        def = def.modes(modes);
    }
    Ok(def)
}

/// A fallback: `{"modes": ["text"], "kind": "macro", "element": <element>}`,
/// every field required.
///
/// # Errors
///
/// As [`element_spec_from_value`].
pub fn fallback_def_from_value(value: &Value, resolve: &mut Resolver<'_>) -> Result<FallbackDef, ExtError> {
    const WHAT: &str = "a fallback";
    let map = fields(WHAT, value, &["modes", "kind", "element"])?;
    let missing = |key: &str| ExtError::Value { detail: format!("{WHAT} must have the field `{key}`") };
    let modes = modes_field(WHAT, &map, "modes")?.ok_or_else(|| missing("modes"))?;
    let kind = form_field::<FallbackKind>(&map, "kind")?.ok_or_else(|| missing("kind"))?;
    let element = match field(&map, "element") {
        Value::Null => return Err(missing("element")),
        element => element_spec_from_value(element, resolve)?,
    };
    Ok(FallbackDef::new(modes, kind, element))
}

// --- the declarations, converted for flm-core --------------------------------------------------

/// The `flm-core` element of `spec`: its block role as a
/// [`FixedBlockRole`], its hook wrapped as an [`ExtExpand`] for `owner`, its
/// handler aspects as an [`ExtReferenceable`] and an [`ExtSectioning`], its
/// data as an `Arc<serde_json::Value>`.
///
/// # Errors
///
/// The element's data cannot cross ([`ExtError::Value`]).
pub(crate) fn element_spec_to_core(spec: &ElementSpec, owner: &Owner) -> Result<CoreElementSpec, ExtError> {
    let mut core = CoreElementSpec::new(spec.name.as_str()).docs(spec.docs.as_str());
    for aspect in spec.aspects() {
        let inserted = match aspect {
            Aspect::BlockRoleAspect(role) => core.aspects.insert::<dyn BlockRoleAspect>(Arc::new(*role)),
            Aspect::Expand(hook) => core.aspects.insert::<dyn CoreExpand>(Arc::new(ExtExpand {
                hook: Arc::clone(hook),
                owner: owner.clone(),
                definition: None,
            })),
            Aspect::Referenceable(aspect) => core.aspects.insert::<dyn CoreReferenceable>(Arc::new(ExtReferenceable {
                aspect: Arc::clone(aspect),
                owner: owner.clone(),
            })),
            Aspect::Sectioning(aspect) => core.aspects.insert::<dyn CoreSectioning>(Arc::new(ExtSectioning {
                aspect: Arc::clone(aspect),
                owner: owner.clone(),
            })),
        };
        // `ElementSpec::with` attaches each kind once, so this does not fail;
        // it is checked rather than assumed.
        inserted.map_err(|_| ExtError::Argument {
            method: "ElementSpec::with".to_string(),
            detail: format!("the aspect `{}` is attached twice to the element `{}`", aspect.kind(), spec.name),
        })?;
    }
    if let Some(value) = &spec.value {
        check_data(value)?;
        core = core.with_value(Arc::new(value.clone()));
    }
    Ok(core)
}

/// The `flm-core` syntax of `syntax`.
pub(crate) fn syntax_def_to_core(syntax: &SyntaxDef) -> CoreSyntaxDef {
    match syntax {
        SyntaxDef::Macro { args } => CoreSyntaxDef::Macro { args: args.clone() },
        SyntaxDef::Environment { args, body } => CoreSyntaxDef::Environment { args: args.clone(), body: body.clone() },
        SyntaxDef::Specials { trigger, args } => CoreSyntaxDef::Specials { trigger: trigger.clone(), args: args.clone() },
        SyntaxDef::Synthesized => CoreSyntaxDef::Synthesized,
    }
}

/// The name of the element `def` declares, which the name rule checks.
pub(crate) fn element_name_of(def: &ElementDef) -> &str {
    match &def.form {
        ElementDefForm::Data { element, .. } => &element.name,
        ElementDefForm::Helper { element_name, .. } => element_name,
    }
}

/// The `flm-core` declaration of `def`, and the user definition behind it
/// when a helper made it (plan A, A10): a data declaration converted field by
/// field; a helper-made one built now by the definition's own `element_def`,
/// or its `element_def_with` with the hook wrapped as an [`ExtExpand`] for
/// `declarer`, then given the docs and modes the value carries.
///
/// # Errors
///
/// As [`element_spec_to_core`].
pub(crate) fn element_def_to_core(
    def: ElementDef,
    declarer: &Owner,
) -> Result<(CoreElementDef, Option<Arc<CoreUserDefinition>>), ExtError> {
    match def.form {
        ElementDefForm::Data { owner, names, syntax, element, docs, modes } => {
            let spec = element_spec_to_core(&element, declarer)?;
            let core =
                CoreElementDef::new(owner, names, syntax_def_to_core(&syntax), Arc::new(spec)).docs(docs).modes(modes);
            Ok((core, None))
        }
        ElementDefForm::Helper { definition, owner, element_name, hook, docs, modes } => {
            let mut core = match hook {
                None => definition.element_def(owner, element_name),
                Some(hook) => {
                    let hook: Arc<dyn CoreExpand> = Arc::new(ExtExpand {
                        hook,
                        owner: declarer.clone(),
                        definition: Some(UserDefinition { definition: Arc::clone(&definition) }),
                    });
                    definition.element_def_with(owner, element_name, hook)
                }
            };
            if let Some(docs) = docs {
                core = core.docs(docs);
            }
            if let Some(modes) = modes {
                core = core.modes(modes);
            }
            Ok((core, Some(definition)))
        }
    }
}

/// The `flm-core` fallback of `def`, owned by `owner`.
///
/// # Errors
///
/// As [`element_spec_to_core`].
pub(crate) fn fallback_def_to_core(def: &FallbackDef, owner: FeatureName, declarer: &Owner) -> Result<CoreFallbackDef, ExtError> {
    Ok(CoreFallbackDef {
        owner,
        modes: def.modes.clone(),
        kind: def.kind,
        element: Arc::new(element_spec_to_core(&def.element, declarer)?),
    })
}

// --- handles, and the values that hold them -------------------------------------------------

/// A handle in its JSON shape: `{"@handle": {"kind": "node", "call": 17,
/// "index": 3}}`, the kind written as its serde name.
pub fn handle_to_value(handle: Handle) -> Value {
    json!({ HANDLE_KEY: { "kind": handle.kind.name(), "call": handle.call, "index": handle.index } })
}

/// Whether `value` is written as a handle: a map whose one key is
/// [`HANDLE_KEY`].
fn is_handle(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.len() == 1 && map.contains_key(HANDLE_KEY))
}

/// The handle `value` writes, in the shape of [`handle_to_value`].
///
/// # Errors
///
/// `value` is not a handle ([`ExtError::Argument`]; the dispatcher names the
/// method).
pub fn handle_from_value(value: &Value) -> Result<Handle, ExtError> {
    let not_one = || ExtError::Argument {
        method: String::new(),
        detail: format!(
            "an object must be written `{{\"{HANDLE_KEY}\": {{\"kind\", \"call\", \"index\"}}}}`, not `{value}`"
        ),
    };
    match value {
        Value::Object(map) if is_handle(value) => {
            let token = map.get(HANDLE_KEY).ok_or_else(not_one)?;
            serde_json::from_value(token.clone()).map_err(|_| not_one())
        }
        _ => Err(not_one()),
    }
}

/// A durable token in its JSON shape: `{"@durable": {"kind":
/// "compiled_template", "table": 3, "index": 0}}`, the kind written as its
/// serde name.
pub fn durable_to_value(token: DurableToken) -> Value {
    json!({ DURABLE_KEY: { "kind": token.kind.name(), "table": token.table, "index": token.index } })
}

/// Whether `value` is written as a durable token: a map whose one key is
/// [`DURABLE_KEY`].
pub fn is_durable(value: &Value) -> bool {
    matches!(value, Value::Object(map) if map.len() == 1 && map.contains_key(DURABLE_KEY))
}

/// The durable token `value` writes, in the shape of [`durable_to_value`].
///
/// # Errors
///
/// `value` is not a durable token ([`ExtError::Argument`]; the dispatcher
/// names the method).
pub fn durable_from_value(value: &Value) -> Result<DurableToken, ExtError> {
    let not_one = || ExtError::Argument {
        method: String::new(),
        detail: format!(
            "a durable token must be written `{{\"{DURABLE_KEY}\": {{\"kind\", \"table\", \"index\"}}}}`, not `{value}`"
        ),
    };
    match value {
        Value::Object(map) if is_durable(value) => {
            let token = map.get(DURABLE_KEY).ok_or_else(not_one)?;
            serde_json::from_value(token.clone()).map_err(|_| not_one())
        }
        _ => Err(not_one()),
    }
}

/// Checks that `value` can cross the boundary as the arguments of a call: as
/// [`check_data`], except that a map written as a handle
/// (`{"@handle": …}`, its one key) is a handle, not data.
///
/// # Errors
///
/// As [`check_data`], for the first value outside a handle that breaks a
/// rule.
pub fn check_arguments(value: &Value) -> Result<(), ExtError> {
    match value {
        Value::Array(items) => items.iter().try_for_each(check_arguments),
        // The token is read by `handle_from_value`, which refuses any other
        // shape; a durable token by `durable_from_value`.
        Value::Object(_) if is_handle(value) || is_durable(value) => Ok(()),
        Value::Object(map) => {
            for (key, item) in map {
                check_key(key)?;
                check_arguments(item)?;
            }
            Ok(())
        }
        other => check_data(other),
    }
}

/// The error of an object of the kind `actual` given where one of the kind
/// `expected` is taken.
pub(crate) fn mismatch(expected: HandleKind, actual: HandleKind) -> ExtError {
    ExtError::Argument {
        method: String::new(),
        detail: format!(
            "an object of the kind `{}` is expected here, not one of the kind `{}`",
            expected.name(),
            actual.name()
        ),
    }
}

/// `nodes` lent into `table`, as a JSON list of handles.
pub fn lend_nodes<'c>(table: &mut CallTable<'c>, nodes: Vec<Box<dyn Node<'c> + 'c>>) -> Value {
    Value::Array(nodes.into_iter().map(|node| handle_to_value(table.lend(Entry::Node(node)))).collect())
}

/// `node` lent into `table`, as a handle, or `null`.
pub fn lend_node<'c>(table: &mut CallTable<'c>, node: Option<Box<dyn Node<'c> + 'c>>) -> Value {
    node.map_or(Value::Null, |node| handle_to_value(table.lend(Entry::Node(node))))
}

/// An owned copy of the node `handle` stands for in `table` (plan A, A16).
///
/// # Errors
///
/// The handle is stale, or stands for no node.
pub fn node_from_table<'c>(table: &mut CallTable<'c>, handle: Handle) -> Result<Box<dyn Node<'c> + 'c>, ExtError> {
    match table.entry(handle)? {
        Entry::Node(node) => Ok(node.clone_box(Token::new())),
        other => Err(mismatch(HandleKind::Node, other.kind())),
    }
}

/// The nodes a JSON list of node handles stands for in `table`, each an owned
/// copy, as the readers of a node list take them.
///
/// # Errors
///
/// `value` is not a list of handles, or a handle is stale or stands for no
/// node.
pub fn nodes_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<Vec<Box<dyn Node<'c> + 'c>>, ExtError> {
    let Value::Array(items) = value else {
        return Err(ExtError::Argument {
            method: String::new(),
            detail: format!("a list of nodes is expected here, not `{value}`"),
        });
    };
    items.iter().map(|item| node_from_table(table, handle_from_value(item)?)).collect()
}

/// The hook an `Expand` handle stands for in `table`.
///
/// # Errors
///
/// `value` is not a handle, or stands for no hook.
pub fn expand_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<Arc<dyn Expand>, ExtError> {
    match table.entry(handle_from_value(value)?)? {
        Entry::Expand(hook) => Ok(Arc::clone(hook)),
        other => Err(mismatch(HandleKind::Expand, other.kind())),
    }
}

/// The object a handle written in a declaration stands for in `table`: the
/// resolver of the declaration readers ([`Resolver`]).
///
/// # Errors
///
/// `value` is not a handle, or stands for no object a declaration may hold
/// (a hook, a handler aspect, an argument a helper made).
pub fn held_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<Held, ExtError> {
    match table.entry(handle_from_value(value)?)? {
        Entry::Expand(hook) => Ok(Held::Expand(Arc::clone(hook))),
        Entry::Referenceable(aspect) => Ok(Held::Referenceable(Arc::clone(aspect))),
        Entry::Sectioning(aspect) => Ok(Held::Sectioning(Arc::clone(aspect))),
        Entry::ArgDef(arg) => Ok(Held::ArgDef(arg.clone())),
        other => Err(ExtError::Argument {
            method: String::new(),
            detail: format!(
                "a declaration may hold objects of the kinds `expand`, `referenceable`, `sectioning` and `arg_def`, not \
                 one of the kind `{}`",
                other.kind().name()
            ),
        }),
    }
}

/// An element declaration read with the table at hand: a handle of kind
/// `element_def` (a declaration a helper made), or the data form of
/// [`element_def_from_value`] whose handles ([`held_from_table`]) are
/// resolved in `table`.
///
/// # Errors
///
/// As [`element_def_from_value`]; a handle of another kind.
pub fn element_def_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<ElementDef, ExtError> {
    if is_handle(value) {
        return match table.entry(handle_from_value(value)?)? {
            Entry::ElementDef(def) => Ok(def.clone()),
            other => Err(mismatch(HandleKind::ElementDef, other.kind())),
        };
    }
    element_def_from_value(value, &mut |held| held_from_table(table, held))
}

/// A fallback read with the table at hand, its handles
/// ([`held_from_table`]) resolved in `table`.
///
/// # Errors
///
/// As [`fallback_def_from_value`].
pub fn fallback_def_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<FallbackDef, ExtError> {
    fallback_def_from_value(value, &mut |held| held_from_table(table, held))
}

/// A handler's diagnostic: `{"id", "severity", "message", "data", "at"}`;
/// `data` is any data (`{}` when absent), `at` a node handle (optional),
/// whose node's span the diagnostic carries
/// ([`Diagnostic::at`]).
///
/// # Errors
///
/// The value is not of the shape, its data cannot cross
/// ([`ExtError::Value`]); `at` is not a node of the call.
pub fn diagnostic_from_value(table: &mut CallTable<'_>, value: &Value) -> Result<Diagnostic, ExtError> {
    const WHAT: &str = "a diagnostic";
    let map = fields(WHAT, value, &["id", "severity", "message", "data", "at"])?;
    let severity = form_field::<Severity>(&map, "severity")?
        .ok_or_else(|| ExtError::Value { detail: format!("{WHAT} must have the field `severity`") })?;
    let mut diagnostic = Diagnostic::new(severity, required_text(WHAT, &map, "id")?, required_text(WHAT, &map, "message")?);
    match field(&map, "data") {
        Value::Null => {}
        data => {
            check_data(data)?;
            diagnostic = diagnostic.with_data(data.clone());
        }
    }
    match field(&map, "at") {
        Value::Null => {}
        at => {
            let node = node_from_table(table, handle_from_value(at)?)?;
            diagnostic = diagnostic.at(&*node);
        }
    }
    Ok(diagnostic)
}

/// What a hook answers: `"default"`, or `{"emit": [<staged handles>]}`, each
/// handle resolved in `table`.
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); a handle is stale or
/// stands for no staged node.
pub fn expansion_from_value(table: &mut CallTable<'_>, value: &Value) -> Result<Expansion, ExtError> {
    if value.as_str() == Some("default") {
        return Ok(Expansion::Default);
    }
    let shape = || ExtError::Value {
        detail: format!("an expansion must be `\"default\"` or `{{\"emit\": [...]}}`, not `{value}`"),
    };
    let Value::Object(map) = value else { return Err(shape()) };
    let (Some(Value::Array(items)), 1) = (map.get("emit"), map.len()) else { return Err(shape()) };
    let mut staged = Vec::with_capacity(items.len());
    for item in items {
        match table.entry(handle_from_value(item)?)? {
            Entry::Staged(node) => staged.push(*node),
            other => return Err(mismatch(HandleKind::Staged, other.kind())),
        }
    }
    Ok(Expansion::Emit(staged))
}

/// What a hook answers, written: `"default"`, or `{"emit": [...]}` with each
/// staged node lent into `table`.
pub fn expansion_to_value(table: &mut CallTable<'_>, expansion: &Expansion) -> Value {
    match expansion {
        Expansion::Default => Value::String("default".to_string()),
        Expansion::Emit(staged) => json!({
            "emit": staged.iter().map(|node| handle_to_value(table.lend(Entry::Staged(*node)))).collect::<Vec<_>>(),
        }),
    }
}

/// A node of a walk: `{"addr": <NodeAddr>, "node": <handle>}`, the node lent
/// into `table`.
pub fn walk_entry_to_value<'c>(table: &mut CallTable<'c>, entry: WalkEntry<'c>) -> Value {
    json!({ "addr": node_addr_to_value(&entry.addr), "node": handle_to_value(table.lend(Entry::Node(entry.node))) })
}

/// A table read by name: `{"ext": <handle>}`, `{"trees": <handle>}`,
/// `{"lowered": <handle>}` or `{"projection": <value>}`, a table lent into
/// `table`.
pub fn table_value_to_value<'c>(table: &mut CallTable<'c>, value: TableValue<'c>) -> Value {
    match value {
        TableValue::Ext(read) => json!({ "ext": handle_to_value(table.lend(Entry::ExtTableRead(read))) }),
        TableValue::Trees(trees) => json!({ "trees": handle_to_value(table.lend(Entry::TreesTable(trees))) }),
        TableValue::Lowered(lowered) => json!({ "lowered": handle_to_value(table.lend(Entry::LoweredTrees(lowered))) }),
        TableValue::Projection(projection) => json!({ "projection": projection }),
        TableValue::Counters(counters) => {
            json!({ "counters": handle_to_value(table.lend(Entry::Counters(TableEntry::Lent(counters)))) })
        }
        TableValue::Anchors(anchors) => {
            json!({ "anchors": handle_to_value(table.lend(Entry::Anchors(TableEntry::Lent(anchors)))) })
        }
    }
}

// --- the artifacts of an output ------------------------------------------------------------------

/// The shape [`artifact_from_value`] reads, as its refusal states it.
const ARTIFACT_SHAPE: &str = "`{ id: { owner, name }, media_type, data: { bytes: CONTENT } | { files: [[path, CONTENT], …] }, \
                              entry, extension, fixed_path, refs }` (`CONTENT`: a text, `{ text: text }` or `{ base64: text }`)";

/// An artifact from its data form (the [module documentation of
/// `api::core::artifact`](crate::api::core::artifact)): `{ id: { owner, name
/// }, media_type, entry, data: { bytes: CONTENT } | { files: [[path,
/// CONTENT], …] }, fixed_path, refs: [{ owner, name }, …], extension }`, where
/// `id`, `media_type` and `data` are required and the other fields may be
/// left out or `null`. A `CONTENT`, the bytes of the artifact or of one file
/// of a folder artifact, is a text or `{ text: text }` (the UTF-8 bytes of the
/// text), or `{ base64: text }` (the bytes the text encodes in base64, the
/// standard alphabet with padding).
///
/// The field `path`, which a stage's read of an artifact set adds to each
/// artifact ([`artifact_set_to_value`]), is ignored, whatever it holds, so
/// that an artifact read can be pushed back unchanged; where the host writes
/// the artifact is decided when it is written, by its `fixed_path` or by the
/// host's naming of files.
///
/// A refusal names the field or the shape at fault and the kind of value
/// found there, never the value itself, so that an artifact's text is not
/// copied into a diagnostic.
///
/// # Errors
///
/// [`ExtError::Value`]: the value is not of that shape (a field unknown,
/// missing or of another type), or a content is not of the form `CONTENT` or
/// holds a text that is not base64 (the refusal names the artifact, and the
/// file of a folder artifact).
pub fn artifact_from_value(value: &Value) -> Result<Artifact, ExtError> {
    let wrong = |fault: String| ExtError::Value { detail: format!("an artifact must be written {ARTIFACT_SHAPE}; {fault}") };
    let Value::Object(map) = value else { return Err(wrong(format!("this one is {}", kind_of_value(value)))) };
    const KNOWN: [&str; 8] = ["id", "media_type", "entry", "data", "fixed_path", "refs", "extension", "path"];
    if let Some(key) = map.keys().find(|key| !KNOWN.contains(&key.as_str())) {
        return Err(wrong(format!("`{key}` is not one of its fields")));
    }
    let present = |key: &str| map.get(key).filter(|value| !value.is_null());
    let id_of = |value: &Value| ArtifactId::from_value(value).ok();
    let id = map.get("id").and_then(id_of).ok_or_else(|| wrong("its field `id` must be `{ owner, name }`, two texts".to_string()))?;
    let media_type = map
        .get("media_type")
        .and_then(Value::as_str)
        .ok_or_else(|| wrong("its field `media_type` must be a text".to_string()))?
        .to_string();
    let text_field = |key: &str| -> Result<Option<String>, ExtError> {
        match present(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(other) => Err(wrong(format!("its field `{key}` must be a text or `null`, not {}", kind_of_value(other)))),
        }
    };
    let entry = match present("entry") {
        None => false,
        Some(Value::Bool(entry)) => *entry,
        Some(other) => return Err(wrong(format!("its field `entry` must be `true`, `false` or `null`, not {}", kind_of_value(other)))),
    };
    let fixed_path = text_field("fixed_path")?.map(RelPath::new);
    let extension = text_field("extension")?;
    let refs = match present("refs") {
        None => Vec::new(),
        Some(Value::Array(refs)) => refs
            .iter()
            .map(|id| id_of(id).ok_or_else(|| wrong("each item of its field `refs` must be `{ owner, name }`, two texts".to_string())))
            .collect::<Result<_, _>>()?,
        Some(other) => return Err(wrong(format!("its field `refs` must be a list or `null`, not {}", kind_of_value(other)))),
    };
    let data = data_from_value(map.get("data").unwrap_or(&Value::Null), Some(&id))?;
    Ok(Artifact { id, entry, media_type, data, fixed_path, refs, extension })
}

/// The shape [`artifact_from_value`] reads, which [`Artifact`]'s
/// [`JsonForm::to_value`] writes, each content in the form a read gives (see
/// [`ArtifactData`]'s).
impl JsonForm for Artifact {
    fn to_value(&self) -> Value {
        json!({
            "id": self.id.to_value(),
            "media_type": self.media_type,
            "entry": self.entry,
            "data": self.data.to_value(),
            "fixed_path": self.fixed_path.as_ref().map(RelPath::as_str),
            "refs": self.refs.to_value(),
            "extension": self.extension,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        artifact_from_value(value)
    }
}

/// An artifact's identifier `{"owner", "name"}`, two texts.
impl JsonForm for ArtifactId {
    fn to_value(&self) -> Value {
        json!({ "owner": self.owner.as_str(), "name": self.name })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let id = match value {
            Value::Object(id) if id.len() == 2 => {
                id.get("owner").and_then(Value::as_str).zip(id.get("name").and_then(Value::as_str))
            }
            _ => None,
        };
        let (owner, name) =
            id.ok_or_else(|| ExtError::Value { detail: format!("an artifact's identifier must be `{{ owner, name }}`, two texts, not `{value}`") })?;
        Ok(ArtifactId::new(owner, name))
    }
}

/// The field `data` of an artifact: `{"bytes": CONTENT}` or `{"files":
/// [[path, CONTENT], …]}`, each `CONTENT` written `{"text": text}` when the
/// bytes are UTF-8 and `{"base64": text}` otherwise, the convention of the
/// host service `programs`; read also as a bare text, the short form of `{
/// text }`. A reference to a file holding the bytes
/// ([`ArtifactData::FileRef`]) is written `null`, which the reader refuses,
/// as it refuses an artifact holding one.
impl JsonForm for ArtifactData {
    fn to_value(&self) -> Value {
        match self {
            ArtifactData::Bytes(data) => json!({ "bytes": content_to_value(data) }),
            ArtifactData::Files(files) => {
                let files: Vec<Value> = files.iter().map(|(path, data)| json!([path.as_str(), content_to_value(data)])).collect();
                json!({ "files": files })
            }
            ArtifactData::FileRef(_) => Value::Null,
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        data_from_value(value, None)
    }
}

/// The content `bytes` in its data form: `{"text": text}` when the bytes are
/// UTF-8, `{"base64": text}` otherwise (the standard alphabet with padding).
fn content_to_value(bytes: &[u8]) -> Value {
    // See decision rationale at [§flm-dd-dr:ext-artifact-content].
    match std::str::from_utf8(bytes) {
        Ok(text) => json!({ "text": text }),
        Err(_) => json!({ "base64": BASE64.encode(bytes) }),
    }
}

/// The field `data` of the artifact `id` (`None` when the reader does not
/// know it) from its data form ([`ArtifactData`]'s), the refusal of a content
/// naming the artifact.
fn data_from_value(value: &Value, id: Option<&ArtifactId>) -> Result<ArtifactData, ExtError> {
    let wrong = |fault: String| ExtError::Value { detail: format!("an artifact must be written {ARTIFACT_SHAPE}; {fault}") };
    let artifact = || match id {
        Some(id) => format!("the artifact `{id}`"),
        None => String::from("an artifact"),
    };
    let data_shape = || wrong("its field `data` must be `{ bytes: CONTENT }` or `{ files: [[path, CONTENT], …] }`".to_string());
    match value {
        Value::Object(data) if data.len() == 1 => match data.iter().next() {
            Some((key, content)) if key == "bytes" => Ok(ArtifactData::Bytes(content_from_value(content, &artifact)?)),
            Some((key, Value::Array(files))) if key == "files" => {
                let mut read = Vec::with_capacity(files.len());
                for file in files {
                    match file.as_array().map(Vec::as_slice) {
                        Some([Value::String(path), content]) => {
                            let file = || format!("the file `{path}` of {}", artifact());
                            read.push((RelPath::new(path.clone()), content_from_value(content, &file)?));
                        }
                        _ => return Err(wrong("each file of its field `data` must be `[path, CONTENT]`".to_string())),
                    }
                }
                Ok(ArtifactData::Files(read))
            }
            _ => Err(data_shape()),
        },
        _ => Err(data_shape()),
    }
}

/// The bytes of a `CONTENT`, `value`: a text or `{"text": text}` (its UTF-8
/// bytes), or `{"base64": text}` (the bytes the text encodes, the standard
/// alphabet with padding). `subject` gives the refusal's words for whose
/// content it is ("the artifact `x:page`", "the file `a.png` of the artifact
/// `x:page`").
fn content_from_value(value: &Value, subject: &dyn Fn() -> String) -> Result<Vec<u8>, ExtError> {
    let refused = |detail: String| ExtError::Value { detail };
    match value {
        Value::String(text) => Ok(text.as_bytes().to_vec()),
        Value::Object(map) => match (map.len(), map.get("text"), map.get("base64")) {
            (1, Some(Value::String(text)), None) => Ok(text.as_bytes().to_vec()),
            (1, None, Some(Value::String(encoded))) => BASE64.decode(encoded).map_err(|_| {
                refused(format!("the content of {} must be base64 in the standard alphabet, with padding", subject()))
            }),
            _ => Err(refused(format!(
                "the content of {}, a map, must be `{{ text: text }}` or `{{ base64: text }}`",
                subject()
            ))),
        },
        other => Err(refused(format!(
            "the content of {} must be a text, `{{ text: text }}` or `{{ base64: text }}`, not {}",
            subject(),
            kind_of_value(other)
        ))),
    }
}

/// An artifact set as a stage of an extension reads it ([`StageCx::get`] on a
/// table declared with
/// [`StandardTable::ArtifactSet`]):
/// `{"artifacts": [ARTIFACT, …], "violations": [text, …]}`. Each artifact of
/// the set, in the set's order ([`ArtifactSet::iter`]), is written in its data
/// form ([`Artifact`]'s [`JsonForm::to_value`]) with one more field, `path`:
/// the text of the path `path_of` answers for it, or `null` when it answers
/// none. Each violation is its text ([`ArtifactSet::violations`]).
///
/// [`StageCx::get`]: crate::api::core::run::StageCx::get
///
/// # Errors
///
/// [`ExtError::Value`] when an artifact of the set refers to a file holding
/// its bytes ([`ArtifactData::FileRef`]) instead of holding them: the
/// refusal names the artifact.
pub fn artifact_set_to_value(set: &ArtifactSet, path_of: &dyn Fn(&Artifact) -> Option<RelPath>) -> Result<Value, ExtError> {
    let mut artifacts = Vec::with_capacity(set.len());
    for artifact in set.iter() {
        if matches!(artifact.data, ArtifactData::FileRef(_)) {
            return Err(ExtError::Value {
                detail: format!(
                    "the artifact `{}` must hold its bytes to be read by an extension, not refer to a file holding them",
                    artifact.id
                ),
            });
        }
        let mut value = artifact.to_value();
        value["path"] = path_of(artifact).map_or(Value::Null, |path| Value::String(path.as_str().to_string()));
        artifacts.push(value);
    }
    let violations: Vec<Value> = set.violations().iter().map(|violation| Value::String(violation.to_string())).collect();
    Ok(json!({ "artifacts": artifacts, "violations": violations }))
}

/// The kind of a JSON value, as a refusal names it without quoting it: "a
/// text", "a number", "a list", "a map", "`true`", "`false`", "`null`".
fn kind_of_value(value: &Value) -> &'static str {
    match value {
        Value::Null => "`null`",
        Value::Bool(true) => "`true`",
        Value::Bool(false) => "`false`",
        Value::Number(_) => "a number",
        Value::String(_) => "a text",
        Value::Array(_) => "a list",
        Value::Object(_) => "a map",
    }
}

// --- the backends and the outputs ----------------------------------------------------------------

/// `{"name", "capability", "table", "media_type", "extension", "made_from":
/// [name, …], "docs"}`; `made_from` and `docs` may be left out.
impl JsonForm for OutputDef {
    fn to_value(&self) -> Value {
        json!({
            "name": self.name.as_str(),
            "capability": self.capability.as_str(),
            "table": self.table.as_str(),
            "media_type": self.entry.media_type,
            "extension": self.entry.extension,
            "made_from": self.made_from.iter().map(|output| output.as_str()).collect::<Vec<_>>(),
            "docs": self.docs,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Form {
            name: String,
            capability: String,
            table: String,
            media_type: String,
            extension: String,
            #[serde(default)]
            made_from: Vec<String>,
            #[serde(default)]
            docs: String,
        }
        let form: Form = read("an output", value)?;
        let def = OutputDef::new(form.name, form.capability, form.table, form.media_type, form.extension).docs(form.docs);
        Ok(form.made_from.into_iter().fold(def, OutputDef::made_from))
    }
}

/// A backend from its data form with the table at hand: `{"name", "native":
/// [name, …], "lowering", "emit", "final_stage", "docs"}`, each stage a handle
/// of kind `library_stage` (a stage of the library, the entry cloned) or
/// `stage` (a stage object the binding lent in, its `Arc` cloned); `docs` may
/// be left out or `null`.
///
/// # Errors
///
/// [`ExtError::Value`] for another shape; a stage that is not a handle, or a
/// handle that is stale or of another kind ([`ExtError::Argument`], with the
/// kind mismatch's text naming `library_stage`).
pub fn backend_def_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<BackendDef, ExtError> {
    const WHAT: &str = "a backend";
    let map = fields(WHAT, value, &["name", "native", "lowering", "emit", "final_stage", "docs"])?;
    let name = required_text(WHAT, &map, "name")?;
    let docs = text_field(WHAT, &map, "docs", "")?;
    let native = match field(&map, "native") {
        Value::Array(items) if items.iter().all(Value::is_string) => {
            items.iter().filter_map(Value::as_str).map(str::to_string).collect()
        }
        other => {
            return Err(ExtError::Value { detail: format!("the field `native` of {WHAT} must be a list of names, not `{other}`") });
        }
    };
    let mut stage = |key: &str| -> Result<BackendStage, ExtError> {
        match table.entry(handle_from_value(field(&map, key))?)? {
            Entry::LibraryStage(stage) => Ok(BackendStage::Library(stage.clone())),
            Entry::Stage(stage) => Ok(BackendStage::Own(Arc::clone(stage))),
            other => Err(ExtError::Argument {
                method: String::new(),
                detail: format!(
                    "an object of the kind `library_stage` or `stage` is expected here, not one of the kind `{}`",
                    other.kind().name()
                ),
            }),
        }
    };
    let lowering = stage("lowering")?;
    let emit = stage("emit")?;
    let final_stage = stage("final_stage")?;
    Ok(BackendDef::new(name, native, lowering, emit, final_stage).docs(docs))
}

// --- the values a node and the readers answer ------------------------------------------------

/// `{"start": 12, "end": 20}`: byte offsets into the node's own source.
impl JsonForm for Span {
    fn to_value(&self) -> Value {
        json!({ "start": self.start, "end": self.end })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        read("a span", value)
    }
}

/// `{"text": "…" | null, "has_placeholder": false}`.
impl JsonForm for CharsContent {
    fn to_value(&self) -> Value {
        json!({ "text": self.text, "has_placeholder": self.has_placeholder })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        read("the characters of a content", value)
    }
}

/// `{"text": "…", "trailing": "…"}`.
impl JsonForm for NormalizedLine {
    fn to_value(&self) -> Value {
        json!({ "text": self.text, "trailing": self.trailing })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let field = |name: &str| match value.get(name) {
            Some(Value::String(text)) => Ok(text.clone()),
            _ => Err(ExtError::Value { detail: format!("a line's text after the newline rule must have the text field `{name}`") }),
        };
        Ok(NormalizedLine { text: field("text")?, trailing: field("trailing")? })
    }
}

/// A node's address: `{"tree": "<the tree id's display text>", "node": <the
/// node's index>}`. Opaque: a handler may store and compare it; nothing takes
/// it back yet.
pub fn node_addr_to_value(addr: &NodeAddr) -> Value {
    json!({ "tree": addr.tree.to_string(), "node": addr.node.index() })
}

/// A [`NotCharacters`] as the error that crosses: [`ExtError::Core`] of kind
/// `not-characters`, its message, and the index of the node that is not
/// characters as `{"node": <index>}` in its data.
pub fn not_characters(error: NotCharacters) -> ExtError {
    ExtError::Core {
        kind: "not-characters".to_string(),
        detail: error.to_string(),
        data: json!({ "node": error.node.index() }),
    }
}

/// An argument written in the `ArgData` map shape `{"code", "name",
/// "layout", "docs", "mode", "scoped", "role"}`, which
/// [`arg_defs_from_value`] reads back: what `arg_def_from_code` answers, and
/// each argument of [`definition_to_value`]. `code` is the code the argument
/// was read from ([`ArgDef::code`]), `null` for an argument built from a
/// parser (which then does not read back); `mode` is `null` when the argument
/// keeps the mode of its surroundings; `role` is `null` for an argument that
/// states no role.
pub fn arg_def_to_value(arg: &ArgDef) -> Value {
    let scoped: Vec<&str> = arg.scoped.iter().map(Name::as_str).collect();
    json!({
        "code": arg.code(),
        "name": arg.name,
        "layout": arg.layout.to_value(),
        "docs": arg.docs,
        "mode": arg.mode.as_ref().map_or(Value::Null, JsonForm::to_value),
        "scoped": scoped,
        "role": arg.role.as_ref().map(Name::as_str),
    })
}

/// A user definition's declaration written as data, in the shape
/// [`definition_from_value`] reads: each argument as [`arg_def_to_value`]
/// writes it, with its code where it was read from one, and the extra
/// placeholders as `[{"name", "substitution"}]`. A definition whose
/// arguments were all read from codes reads back as written.
pub fn definition_to_value(definition: &Definition) -> Value {
    let args: Vec<Value> = definition.args.iter().map(arg_def_to_value).collect();
    let extras: Vec<Value> = definition
        .extra_placeholders
        .iter()
        .map(|extra| json!({ "name": extra.name, "substitution": extra.substitution }))
        .collect();
    json!({
        "kind": definition.kind.to_value(),
        "name": definition.name.as_str(),
        "args": args,
        "content": definition.content,
        "modes": definition.modes.to_value(),
        "defaults": definition.defaults,
        "argument_number_offset": definition.argument_number_offset,
        "block_role": definition.block_role.to_value(),
        "expand": definition.expand.to_value(),
        "overrides": definition.overrides,
        "docs": definition.docs,
        "extra_placeholders": extras,
    })
}

/// A point's identifier as data: `{"addr": {"tree", "node"}, "index"}`, the
/// address as [`node_addr_to_value`] writes it.
pub fn point_id_to_value(point: &PointId) -> Value {
    json!({ "addr": node_addr_to_value(&point.addr), "index": point.index })
}

// --- lowering rules ----------------------------------------------------------------------------

/// `{"requires": [], "reads": [], "reads_opt": [], "options": [],
/// "produces": [], "selection_requires": []}`; any list may be left out.
impl JsonForm for LoweringDecl {
    fn to_value(&self) -> Value {
        json!({
            "requires": self.requires,
            "reads": self.reads,
            "reads_opt": self.reads_opt,
            "options": self.options,
            "produces": self.produces,
            "selection_requires": self.selection_requires,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Form {
            #[serde(default)]
            requires: Vec<String>,
            #[serde(default)]
            reads: Vec<String>,
            #[serde(default)]
            reads_opt: Vec<String>,
            #[serde(default)]
            options: Vec<String>,
            #[serde(default)]
            produces: Vec<String>,
            #[serde(default)]
            selection_requires: Vec<String>,
        }
        let form: Form = read("a lowering rule's declaration", value)?;
        Ok(LoweringDecl {
            requires: form.requires,
            reads: form.reads,
            reads_opt: form.reads_opt,
            options: form.options,
            produces: form.produces,
            selection_requires: form.selection_requires,
        })
    }
}

/// `"everywhere"`, `"never"`, or `{"on_backends": ["html", ...]}`.
impl JsonForm for DefaultMark {
    fn to_value(&self) -> Value {
        match self {
            DefaultMark::Everywhere => Value::String("everywhere".to_string()),
            DefaultMark::Never => Value::String("never".to_string()),
            DefaultMark::OnBackends(backends) => {
                json!({ "on_backends": backends.iter().map(|backend| backend.to_string()).collect::<Vec<_>>() })
            }
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match value {
            Value::String(text) if text == "everywhere" => return Ok(DefaultMark::Everywhere),
            Value::String(text) if text == "never" => return Ok(DefaultMark::Never),
            Value::Object(map) if map.len() == 1 => {
                if let Some(Value::Array(backends)) = map.get("on_backends") {
                    let names: Option<Vec<_>> =
                        backends.iter().map(|backend| backend.as_str().map(Into::into)).collect();
                    if let Some(names) = names {
                        return Ok(DefaultMark::OnBackends(names));
                    }
                }
            }
            _ => {}
        }
        Err(ExtError::Value {
            detail: format!(
                "a default mark must be `\"everywhere\"`, `\"never\"` or `{{\"on_backends\": [...]}}` with backend names, not `{value}`"
            ),
        })
    }
}

/// A rule's body the binding lent in: a handle of kind `lower`.
///
/// # Errors
///
/// `value` is not a handle, or stands for no body.
pub fn lower_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<Arc<dyn Lower>, ExtError> {
    match table.entry(handle_from_value(value)?)? {
        Entry::Lower(body) => Ok(Arc::clone(body)),
        other => Err(mismatch(HandleKind::Lower, other.kind())),
    }
}

/// A staged node: a handle of kind `staged`.
///
/// # Errors
///
/// `value` is not a handle of that kind in the running call.
pub fn staged_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<Staged, ExtError> {
    match table.entry(handle_from_value(value)?)? {
        Entry::Staged(node) => Ok(*node),
        other => Err(mismatch(HandleKind::Staged, other.kind())),
    }
}

/// A list of staged nodes, each a handle of kind `staged`.
///
/// # Errors
///
/// `value` is not a list, or an item is not a staged node of the call.
pub fn staged_list_from_table(table: &mut CallTable<'_>, value: &Value) -> Result<Vec<Staged>, ExtError> {
    let Value::Array(items) = value else {
        return Err(ExtError::Value { detail: format!("a list of staged nodes must be a list, not `{value}`") });
    };
    items.iter().map(|item| staged_from_table(table, item)).collect()
}

/// What a rule's body answers: `"default"`, or `{"emit": [...]}` with staged
/// nodes, as an expansion is written ([`expansion_from_value`]).
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); an item is not a
/// staged node of the call.
pub fn lowering_from_value(table: &mut CallTable<'_>, value: &Value) -> Result<Lowering, ExtError> {
    if value.as_str() == Some("default") {
        return Ok(Lowering::Default);
    }
    let shape = || ExtError::Value {
        detail: format!("a lowering must be `\"default\"` or `{{\"emit\": [...]}}`, not `{value}`"),
    };
    let Value::Object(map) = value else { return Err(shape()) };
    let (Some(items), 1) = (map.get("emit"), map.len()) else { return Err(shape()) };
    if !items.is_array() {
        return Err(shape());
    }
    Ok(Lowering::Emit(staged_list_from_table(table, items)?))
}

/// What a rule's body answered, written: `"default"`, or `{"emit": [...]}`
/// with each staged node lent into `table`.
pub fn lowering_to_value(table: &mut CallTable<'_>, lowering: &Lowering) -> Value {
    match lowering {
        Lowering::Default => Value::String("default".to_string()),
        Lowering::Emit(staged) => json!({
            "emit": staged.iter().map(|node| handle_to_value(table.lend(Entry::Staged(*node)))).collect::<Vec<_>>(),
        }),
    }
}

/// What a rewrite stage's handler answers: `"descend"`, or `{"emit": [...]}`
/// with staged nodes, as a lowering is written ([`lowering_from_value`]).
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); an item is not a
/// staged node of the call.
pub fn restage_from_value(table: &mut CallTable<'_>, value: &Value) -> Result<Restage, ExtError> {
    if value.as_str() == Some("descend") {
        return Ok(Restage::Descend);
    }
    let shape = || ExtError::Value {
        detail: format!("a rewrite handler's answer must be `\"descend\"` or `{{\"emit\": [...]}}`, not `{value}`"),
    };
    let Value::Object(map) = value else { return Err(shape()) };
    let (Some(items), 1) = (map.get("emit"), map.len()) else { return Err(shape()) };
    if !items.is_array() {
        return Err(shape());
    }
    Ok(Restage::Emit(staged_list_from_table(table, items)?))
}

/// What a rewrite stage's handler answered, written: `"descend"`, or
/// `{"emit": [...]}` with each staged node lent into `table`.
pub fn restage_to_value(table: &mut CallTable<'_>, restage: &Restage) -> Value {
    match restage {
        Restage::Descend => Value::String("descend".to_string()),
        Restage::Emit(staged) => json!({
            "emit": staged.iter().map(|node| handle_to_value(table.lend(Entry::Staged(*node)))).collect::<Vec<_>>(),
        }),
    }
}

/// The bundles of a call to
/// [`Restager::restage_invocation`](crate::api::techy::transform::Restager::restage_invocation),
/// each a handle of the kind `kind`, taken out of the call's table: `take`
/// takes the bundle out of its entry (`None` when it was taken already) and
/// answers it in a new box. A bundle taken already is refused (each bundle
/// is given once).
///
/// # Errors
///
/// `value` is not a list ([`ExtError::Value`]); an item is not a handle of
/// the kind in the running call; a bundle was given already
/// ([`ExtError::Argument`]).
fn bundles_from_table<'c, T>(
    table: &mut CallTable<'c>,
    value: &Value,
    kind: HandleKind,
    mut take: impl FnMut(&mut Entry<'c>) -> Option<Option<T>>,
) -> Result<Vec<T>, ExtError> {
    let Value::Array(items) = value else {
        return Err(ExtError::Value { detail: format!("a list of `{}` objects must be a list, not `{value}`", kind.name()) });
    };
    let mut bundles = Vec::with_capacity(items.len());
    for item in items {
        let entry = table.entry(handle_from_value(item)?)?;
        let actual = entry.kind();
        match take(entry) {
            None => return Err(mismatch(kind, actual)),
            Some(None) => return Err(crate::adapter::rewrite::spent()),
            Some(Some(bundle)) => bundles.push(bundle),
        }
    }
    Ok(bundles)
}

/// A tree of presentation nodes written as data ([`PresTree`]):
/// `{"element", "payload", "children": [...]}` (the payload `{}` and the
/// children `[]` when left out), `{"build": <staged node>}`, or
/// `{"text": "..."}`.
///
/// # Errors
///
/// The value is not of the shape ([`ExtError::Value`]); a `build` is not a
/// staged node of the call.
pub fn pres_tree_from_value(table: &mut CallTable<'_>, value: &Value) -> Result<PresTree, ExtError> {
    const WHAT: &str = "a presentation tree";
    let Value::Object(map) = value else {
        return Err(ExtError::Value { detail: format!("{WHAT} must be a map, not `{value}`") });
    };
    if let Some(build) = map.get("build")
        && map.len() == 1
    {
        return Ok(PresTree::Build(staged_from_table(table, build)?));
    }
    if let Some(Value::String(text)) = map.get("text")
        && map.len() == 1
    {
        return Ok(PresTree::Text(text.clone()));
    }
    let map = fields(WHAT, value, &["element", "payload", "children"])?;
    let element = required_text(WHAT, &map, "element")?;
    let payload = match field(&map, "payload") {
        Value::Null => Value::Object(serde_json::Map::new()),
        other => other.clone(),
    };
    let children = match field(&map, "children") {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().map(|item| pres_tree_from_value(table, item)).collect::<Result<_, _>>()?,
        other => return Err(ExtError::Value { detail: format!("the children of {WHAT} must be a list, not `{other}`") }),
    };
    Ok(PresTree::Node { element, payload, children })
}

// --- the `math` library ------------------------------------------------------------------------

/// An equation line as data: `{"content": [node, …], "labels": [{"text",
/// "span": {"start", "end"}, "label": {"ref_type", "ref_label"} | null}, …], "tag": {"starred",
/// "content": [node, …]} | null, "nonumber", "linebreak": {"spacing"} |
/// null, "span": {"start", "end"} | null}`, the nodes lent into `table`. The
/// span runs from the first content node's start to the last one's end;
/// for a line with no content, it is the span of the `\\` that ends it, and
/// `null` for an empty last line.
pub fn equation_line_to_value<'c>(table: &mut CallTable<'c>, line: EquationLine<'c>) -> Value {
    let labels: Vec<Value> = line.labels.iter().map(JsonForm::to_value).collect();
    let tag = match line.tag {
        Some(tag) => json!({ "starred": tag.starred, "content": lend_nodes(table, tag.content) }),
        None => Value::Null,
    };
    let span = match (line.content.first(), line.content.last(), &line.linebreak) {
        (Some(first), Some(last), _) => Some(Span { start: first.span().start, end: last.span().end }),
        (_, _, Some(brk)) => Some(brk.span),
        _ => None,
    };
    let linebreak = line.linebreak.map_or(Value::Null, |brk| json!({ "spacing": brk.spacing }));
    json!({
        "span": span.as_ref().map_or(Value::Null, JsonForm::to_value),
        "content": lend_nodes(table, line.content),
        "labels": labels,
        "tag": tag,
        "nonumber": line.nonumber,
        "linebreak": linebreak,
    })
}

/// A line's frame as data: `{"environment": node, "index", "line": <the
/// line, as [`equation_line_to_value`]>, "anchor": text | null, "shows":
/// {"kind": "nothing"} | {"kind": "number", "segments": [node, …], "text"} |
/// {"kind": "tag", "starred", "content": [node, …]}}`, the nodes lent into
/// `table`.
pub fn line_frame_to_value<'c>(table: &mut CallTable<'c>, frame: LineFrame<'c>) -> Value {
    let environment = lend_node(table, Some(frame.environment));
    let line = equation_line_to_value(table, frame.line);
    let shows = match frame.shows {
        LineShows::Nothing => json!({ "kind": "nothing" }),
        LineShows::Number { segments, text } => {
            json!({ "kind": "number", "segments": lend_nodes(table, segments), "text": text })
        }
        LineShows::Tag { starred, content } => {
            json!({ "kind": "tag", "starred": starred, "content": lend_nodes(table, content) })
        }
    };
    json!({ "environment": environment, "index": frame.index, "line": line, "anchor": frame.anchor, "shows": shows })
}

/// A line's frame read from the data [`line_frame_to_value`] writes: its
/// `environment`, `index` and `shows` are read, its `line` read again from
/// the environment at the index (an empty line where none is there), its
/// `anchor` taken as written.
///
/// # Errors
///
/// [`ExtError::Value`] for another shape; a stale or mismatched handle.
pub fn line_frame_from_value<'c>(table: &mut CallTable<'c>, value: &Value) -> Result<LineFrame<'c>, ExtError> {
    const WHAT: &str = "a line's frame";
    let map = fields(WHAT, value, &["environment", "index", "line", "anchor", "shows"])?;
    let environment = node_from_table(table, handle_from_value(field(&map, "environment"))?)?;
    let index = field(&map, "index").as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| ExtError::Value {
        detail: format!("the `index` of {WHAT} must be a number from 0 up, not `{}`", field(&map, "index")),
    })?;
    let anchor = match field(&map, "anchor") {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => return Err(ExtError::Value { detail: format!("the `anchor` of {WHAT} must be a text or null, not `{other}`") }),
    };
    let shows_value = field(&map, "shows");
    let refuse = || ExtError::Value {
        detail: format!(
            "the `shows` of {WHAT} must be `{{\"kind\": \"nothing\"}}`, `{{\"kind\": \"number\", \"segments\", \"text\"}}` \
             or `{{\"kind\": \"tag\", \"starred\", \"content\"}}`, not `{shows_value}`"
        ),
    };
    let shows = match shows_value.get("kind").and_then(Value::as_str) {
        Some("nothing") => LineShows::Nothing,
        Some("number") => LineShows::Number {
            segments: nodes_from_value(table, shows_value.get("segments").unwrap_or(&Value::Null))?,
            text: shows_value.get("text").and_then(Value::as_str).ok_or_else(refuse)?.to_string(),
        },
        Some("tag") => LineShows::Tag {
            starred: shows_value.get("starred").and_then(Value::as_bool).ok_or_else(refuse)?,
            content: nodes_from_value(table, shows_value.get("content").unwrap_or(&Value::Null))?,
        },
        _ => return Err(refuse()),
    };
    let line = equation_lines(&*environment).into_iter().nth(index).unwrap_or_else(|| EquationLine {
        content: Vec::new(),
        labels: Vec::new(),
        tag: None,
        nonumber: false,
        linebreak: None,
    });
    Ok(LineFrame { environment, index, line, anchor, shows })
}

// --- style and script definitions ------------------------------------------------------------

/// A style or script definition's identifier, parts and documentation, read
/// from `{"id", "content": [parts], "docs"}` (`what` names the definition in
/// the messages): each part `{"inline": "…"}`, `{"file": "…"}`, `{"url": "…"}`
/// or `{"table": "name"}`; `docs` may be absent.
///
/// # Errors
///
/// [`ExtError::Value`] for another shape.
pub fn definition_parts_from_value(what: &str, value: &Value) -> Result<(Name, Vec<DefContent>, String), ExtError> {
    let map = fields(what, value, &["id", "content", "docs"])?;
    let id = required_text(what, &map, "id")?;
    let docs = text_field(what, &map, "docs", "")?;
    let Value::Array(parts) = field(&map, "content") else {
        return Err(ExtError::Value {
            detail: format!("the content of {what} must be a list of parts, not `{}`", field(&map, "content")),
        });
    };
    let content = parts.iter().map(definition_part_from_value).collect::<Result<_, _>>()?;
    Ok((Name::from(id.as_str()), content, docs))
}

/// One part of a definition: `{"inline"}`, `{"file"}`, `{"url"}` or
/// `{"table"}`, with a string.
fn definition_part_from_value(value: &Value) -> Result<DefContent, ExtError> {
    let refuse = || ExtError::Value {
        detail: format!(
            "a part of a definition must be `{{\"inline\": text}}`, `{{\"file\": path}}`, `{{\"url\": address}}` or \
             `{{\"table\": name}}`, not `{value}`"
        ),
    };
    let Value::Object(map) = value else { return Err(refuse()) };
    let mut entries = map.iter();
    let (Some((key, Value::String(text))), None) = (entries.next(), entries.next()) else { return Err(refuse()) };
    match key.as_str() {
        "inline" => Ok(DefContent::Inline(text.clone())),
        "file" => Ok(DefContent::File(text.clone())),
        "url" => Ok(DefContent::Url(text.clone())),
        "table" => Ok(DefContent::Table(CapName::from(text.as_str()))),
        _ => Err(refuse()),
    }
}

/// `{"inline": text}`, `{"file": path}`, `{"url": address}` or `{"table":
/// name}`; read by `definition_part_from_value`.
impl JsonForm for DefContent {
    fn to_value(&self) -> Value {
        match self {
            DefContent::Inline(text) => json!({ "inline": text }),
            DefContent::File(path) => json!({ "file": path }),
            DefContent::Url(address) => json!({ "url": address }),
            DefContent::Table(name) => json!({ "table": name.as_str() }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        definition_part_from_value(value)
    }
}

/// `{"id", "content": [parts], "docs"}`; read by
/// [`definition_parts_from_value`], named "a style definition".
impl JsonForm for StyleDef {
    fn to_value(&self) -> Value {
        json!({ "id": self.id.as_str(), "content": self.content.to_value(), "docs": self.docs })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let (id, content, docs) = definition_parts_from_value("a style definition", value)?;
        Ok(StyleDef { id, content, docs })
    }
}

/// `{"id", "content": [parts], "docs"}`; read by
/// [`definition_parts_from_value`], named "a script definition".
impl JsonForm for ScriptDef {
    fn to_value(&self) -> Value {
        json!({ "id": self.id.as_str(), "content": self.content.to_value(), "docs": self.docs })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let (id, content, docs) = definition_parts_from_value("a script definition", value)?;
        Ok(ScriptDef { id, content, docs })
    }
}

// --- the arguments of a call, and the two conversion traits ------------------------------------

/// A call's arguments: a JSON list, read by position.
pub(crate) struct Args<'a> {
    list: &'a [Value],
}

impl<'a> Args<'a> {
    /// The arguments `args`, which must be a list.
    pub(crate) fn new(args: &'a Value) -> Result<Args<'a>, ExtError> {
        match args {
            Value::Array(list) => Ok(Args { list }),
            other => Err(argument(format!("the arguments must be a list, not `{other}`"))),
        }
    }

    /// Checks that there are `count` arguments.
    pub(crate) fn arity(&self, count: usize) -> Result<(), ExtError> {
        if self.list.len() == count {
            Ok(())
        } else {
            let plural = if count == 1 { "" } else { "s" };
            Err(argument(format!("the method takes {count} argument{plural}, not {}", self.list.len())))
        }
    }

    /// The argument at `i`; `null` past the end (counts are checked first).
    pub(crate) fn get(&self, i: usize) -> &'a Value {
        self.list.get(i).unwrap_or(&Value::Null)
    }

    /// The argument at `i`, a string.
    pub(crate) fn text(&self, i: usize) -> Result<&'a str, ExtError> {
        self.get(i).as_str().ok_or_else(|| self.bad(i, "a string"))
    }

    /// The argument at `i`, a number from 0 up.
    pub(crate) fn index(&self, i: usize) -> Result<usize, ExtError> {
        self.get(i).as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| self.bad(i, "a number from 0 up"))
    }

    /// The argument at `i`, a boolean.
    pub(crate) fn flag(&self, i: usize) -> Result<bool, ExtError> {
        self.get(i).as_bool().ok_or_else(|| self.bad(i, "`true` or `false`"))
    }

    /// The argument at `i`, a handle.
    pub(crate) fn handle(&self, i: usize) -> Result<Handle, ExtError> {
        handle_from_value(self.get(i))
    }

    /// The error of an argument at `i` that is not `what`.
    pub(crate) fn bad(&self, i: usize, what: &str) -> ExtError {
        argument(format!("argument {} must be {what}, not `{}`", i + 1, self.get(i)))
    }
}

/// An argument error; the dispatcher's `call` names the method.
pub(crate) fn argument(detail: String) -> ExtError {
    ExtError::Argument { method: String::new(), detail }
}

/// How a crossing type is read from the argument at position `i` of a call,
/// with the running call's table at hand (a handle argument is resolved in
/// it).
///
/// One implementation per type a façade method, a constructor or a free
/// function of the extension API takes, each over the reader this module had
/// before; the generated dispatcher calls them. A borrowed parameter (`&str`,
/// `&[u8]`, `&[Box<dyn Node>]`, `&Definition`) is read as its owned form
/// (`String`, `Vec<u8>`, `Vec<Box<dyn Node>>`, `Definition`) and passed by
/// reference; a borrowed façade (`&mut dyn LowerCx`, `&dyn CompiledTemplate`)
/// is not read here: the arm takes it from the table with the receiver.
pub(crate) trait FromArg<'c>: Sized {
    /// The argument at `i` of `args`, read as this type.
    ///
    /// # Errors
    ///
    /// The argument is not of the type's shape ([`ExtError::Argument`],
    /// [`ExtError::Value`]), or a handle in it is stale or of another kind.
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError>;
}

/// How a crossing type is written as a call's reply, lending what it holds
/// (a node, a staged node, a table read) into the running call's table.
///
/// One implementation per type a façade method, a constructor or a free
/// function answers, each over the writer this module had before, and one
/// per shape that wraps another: `Option<T>` (`null`, or `T`), `Vec<T>` (a
/// list), `(A, B)` (a list of the two), `()` (`null`), `Result<T, ExtError>`
/// (the error, or `T`), `Result<T, NotCharacters>` (the error as
/// [`not_characters`] writes it, or `T`).
pub(crate) trait IntoReply<'c> {
    /// The value written as JSON, its objects lent into `table`.
    ///
    /// # Errors
    ///
    /// The value cannot cross ([`ExtError::Value`]: a floating-point number,
    /// a reserved key), or a `flm-core` error it carries ([`ExtError::Core`]).
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError>;
}

// The shapes that wrap another type.

impl<'c, T: IntoReply<'c>> IntoReply<'c> for Option<T> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        self.map_or(Ok(Value::Null), |value| value.into_reply(table))
    }
}

impl<'c, T: IntoReply<'c>> IntoReply<'c> for Vec<T> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        self.into_iter().map(|item| item.into_reply(table)).collect::<Result<_, _>>().map(Value::Array)
    }
}

impl<'c, A: IntoReply<'c>, B: IntoReply<'c>> IntoReply<'c> for (A, B) {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(Value::Array(vec![self.0.into_reply(table)?, self.1.into_reply(table)?]))
    }
}

impl<'c> IntoReply<'c> for () {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(Value::Null)
    }
}

impl<'c, T: IntoReply<'c>> IntoReply<'c> for Result<T, ExtError> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        self?.into_reply(table)
    }
}

impl<'c, T: IntoReply<'c>> IntoReply<'c> for Result<T, NotCharacters> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        self.map_err(not_characters)?.into_reply(table)
    }
}

// Scalars and data.

impl<'c> FromArg<'c> for String {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        Ok(args.text(i)?.to_string())
    }
}

impl<'c> IntoReply<'c> for String {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(Value::String(self))
    }
}

/// A string, or `null` (a script's argument left out) for none.
impl<'c> FromArg<'c> for Option<String> {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        match args.get(i) {
            Value::Null => Ok(None),
            _ => Ok(Some(args.text(i)?.to_string())),
        }
    }
}

impl<'c> FromArg<'c> for usize {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        args.index(i)
    }
}

impl<'c> IntoReply<'c> for usize {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(json!(self))
    }
}

impl<'c> IntoReply<'c> for u32 {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(json!(self))
    }
}

impl<'c> FromArg<'c> for bool {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        args.flag(i)
    }
}

impl<'c> IntoReply<'c> for bool {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(Value::Bool(self))
    }
}

/// `&[u8]`: a string whose UTF-8 bytes are taken (plan A, A3).
impl<'c> FromArg<'c> for Vec<u8> {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        Ok(args.text(i)?.as_bytes().to_vec())
    }
}

/// Data: taken as it is (the arguments were checked on entry,
/// [`check_arguments`]), checked on its way out ([`check_data`]).
impl<'c> FromArg<'c> for Value {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        Ok(args.get(i).clone())
    }
}

impl<'c> IntoReply<'c> for Value {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        check_data(&self)?;
        Ok(self)
    }
}

/// The types that cross as their [`JsonForm`] in both directions: the enums
/// with one string per variant.
macro_rules! crosses_as_its_form {
    ($($type:ty),+ $(,)?) => {
        $(
            impl<'c> FromArg<'c> for $type {
                fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
                    <$type as JsonForm>::from_value(args.get(i))
                }
            }

            impl<'c> IntoReply<'c> for $type {
                fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
                    Ok(self.to_value())
                }
            }
        )+
    };
}

crosses_as_its_form!(
    Effect,
    Scope,
    CallableKind,
    FallbackKind,
    DefinitionKind,
    DefinitionModes,
    ArgLayout,
    BodyLayout,
    ExpandPolicy,
    Placement,
    Severity,
    NodeKindName,
    Mode,
    StandardTable,
    Arity,
);

/// The types read as their [`JsonForm`] and never answered.
macro_rules! read_as_its_form {
    ($($type:ty),+ $(,)?) => {
        $(
            impl<'c> FromArg<'c> for $type {
                fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
                    <$type as JsonForm>::from_value(args.get(i))
                }
            }
        )+
    };
}

read_as_its_form!(StageDecl, Needs, LoweringDecl, DefaultMark, OutputDef, RewriteDecl);

/// The types answered as their [`JsonForm`] and never read.
macro_rules! answered_as_its_form {
    ($($type:ty),+ $(,)?) => {
        $(
            impl<'c> IntoReply<'c> for $type {
                fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
                    Ok(self.to_value())
                }
            }
        )+
    };
}

answered_as_its_form!(Span, CharsContent, NormalizedLine);

/// `ElementDef::modes`'s list: each item a mode.
impl<'c> FromArg<'c> for Vec<Mode> {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let Value::Array(list) = args.get(i) else { return Err(args.bad(i, "a list of modes")) };
        list.iter().map(Mode::from_value).collect()
    }
}

/// A handle lent into `table` for `entry`, as JSON.
fn lent<'c>(table: &mut CallTable<'c>, entry: Entry<'c>) -> Result<Value, ExtError> {
    Ok(handle_to_value(table.lend(entry)))
}

// The declarations.

/// [`element_def_from_table`]: a handle of kind `element_def`, or the data
/// form.
impl<'c> FromArg<'c> for ElementDef {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        element_def_from_table(table, args.get(i))
    }
}

impl<'c> IntoReply<'c> for ElementDef {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::ElementDef(self))
    }
}

/// [`fallback_def_from_table`].
impl<'c> FromArg<'c> for FallbackDef {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        fallback_def_from_table(table, args.get(i))
    }
}

/// [`definition_parts_from_value`], named "a style definition".
impl<'c> FromArg<'c> for StyleDef {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        StyleDef::from_value(args.get(i))
    }
}

/// [`definition_parts_from_value`], named "a script definition".
impl<'c> FromArg<'c> for ScriptDef {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        ScriptDef::from_value(args.get(i))
    }
}

/// [`definition_from_value`].
impl<'c> FromArg<'c> for Definition {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        definition_from_value(args.get(i), &mut |held| held_from_table(table, held))
    }
}

/// [`definition_to_value`], which does not read back.
impl<'c> IntoReply<'c> for &Definition {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(definition_to_value(self))
    }
}

/// [`diagnostic_from_value`].
impl<'c> FromArg<'c> for Diagnostic {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        diagnostic_from_value(table, args.get(i))
    }
}

// What a hook or a rule's body answers, and what a rule stages.

/// [`expansion_from_value`].
impl<'c> FromArg<'c> for Expansion {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        expansion_from_value(table, args.get(i))
    }
}

/// [`expansion_to_value`].
impl<'c> IntoReply<'c> for Expansion {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(expansion_to_value(table, &self))
    }
}

/// [`lowering_from_value`].
impl<'c> FromArg<'c> for Lowering {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        lowering_from_value(table, args.get(i))
    }
}

/// [`lowering_to_value`].
impl<'c> IntoReply<'c> for Lowering {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(lowering_to_value(table, &self))
    }
}

/// [`pres_tree_from_value`].
impl<'c> FromArg<'c> for PresTree {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        pres_tree_from_value(table, args.get(i))
    }
}

/// [`staged_from_table`].
impl<'c> FromArg<'c> for Staged {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        staged_from_table(table, args.get(i))
    }
}

impl<'c> IntoReply<'c> for Staged {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::Staged(self))
    }
}

/// [`staged_list_from_table`].
impl<'c> FromArg<'c> for Vec<Staged> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        staged_list_from_table(table, args.get(i))
    }
}

/// The slots of [`Lowerer::pres_slots`](crate::api::core::lowering::Lowerer::pres_slots):
/// `[[name, [staged, …]], …]`.
impl<'c> FromArg<'c> for Vec<(String, Vec<Staged>)> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let Value::Array(items) = args.get(i) else { return Err(args.bad(i, "a list of slots")) };
        let mut slots = Vec::with_capacity(items.len());
        for item in items {
            let (Some(Value::String(name)), Some(nodes), 2) = (item.get(0), item.get(1), item.as_array().map_or(0, Vec::len))
            else {
                return Err(args.bad(i, "a list of slots, each `[name, [staged nodes]]`"));
            };
            slots.push((name.clone(), staged_list_from_table(table, nodes)?));
        }
        Ok(slots)
    }
}

// What a stage reads.

/// [`walk_entry_to_value`].
impl<'c> IntoReply<'c> for WalkEntry<'c> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(walk_entry_to_value(table, self))
    }
}

/// [`node_addr_to_value`].
impl<'c> IntoReply<'c> for NodeAddr {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(node_addr_to_value(&self))
    }
}

/// [`table_value_to_value`].
impl<'c> IntoReply<'c> for TableValue<'c> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(table_value_to_value(table, self))
    }
}

// The `math` library.

/// [`equation_line_to_value`].
impl<'c> IntoReply<'c> for EquationLine<'c> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(equation_line_to_value(table, self))
    }
}

/// [`line_frame_from_value`].
impl<'c> FromArg<'c> for LineFrame<'c> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        line_frame_from_value(table, args.get(i))
    }
}

/// [`line_frame_to_value`].
impl<'c> IntoReply<'c> for LineFrame<'c> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(line_frame_to_value(table, self))
    }
}

/// A list of lines' frames, each read by [`line_frame_from_value`].
impl<'c> FromArg<'c> for Vec<LineFrame<'c>> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let Value::Array(items) = args.get(i) else { return Err(args.bad(i, "a list of line frames")) };
        items.iter().map(|item| line_frame_from_value(table, item)).collect()
    }
}

// The objects that cross as handles.

/// A node: an owned copy out of the table ([`node_from_table`], plan A,
/// A16); lent when answered.
impl<'c> FromArg<'c> for Box<dyn Node<'c> + 'c> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        node_from_table(table, args.handle(i)?)
    }
}

impl<'c> IntoReply<'c> for Box<dyn Node<'c> + 'c> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::Node(self))
    }
}

/// A list of nodes ([`nodes_from_value`]), each an owned copy.
impl<'c> FromArg<'c> for Vec<Box<dyn Node<'c> + 'c>> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        nodes_from_value(table, args.get(i))
    }
}

/// The entry of kind `$kind` at the handle `$handle` of `$table`, cloned.
macro_rules! cloned_entry {
    ($table:expr, $handle:expr, $variant:ident) => {
        match $table.entry($handle)? {
            Entry::$variant(inner) => Ok(inner.clone()),
            other => Err(mismatch(HandleKind::$variant, other.kind())),
        }
    };
}

/// A user definition: the entry of its handle, cloned.
impl<'c> FromArg<'c> for UserDefinition {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        cloned_entry!(table, args.handle(i)?, UserDefinition)
    }
}

impl<'c> IntoReply<'c> for UserDefinition {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::UserDefinition(self))
    }
}

/// A lowering rule: the entry of its handle, cloned.
impl<'c> FromArg<'c> for LoweringRule {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        cloned_entry!(table, args.handle(i)?, LoweringRule)
    }
}

impl<'c> IntoReply<'c> for LoweringRule {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::LoweringRule(self))
    }
}

/// A stage object the binding lent in: the entry's `Arc`, cloned.
impl<'c> FromArg<'c> for Arc<dyn Stage> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        match table.entry(args.handle(i)?)? {
            Entry::Stage(stage) => Ok(Arc::clone(stage)),
            other => Err(mismatch(HandleKind::Stage, other.kind())),
        }
    }
}

/// [`backend_def_from_table`].
impl<'c> FromArg<'c> for BackendDef {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        backend_def_from_table(table, args.get(i))
    }
}

/// A stage of the library, lent as its entry.
impl<'c> IntoReply<'c> for LibraryStage {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::LibraryStage(self))
    }
}

/// [`expand_from_table`].
impl<'c> FromArg<'c> for Arc<dyn Expand> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        expand_from_table(table, args.get(i))
    }
}

/// [`lower_from_table`].
impl<'c> FromArg<'c> for Arc<dyn Lower> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        lower_from_table(table, args.get(i))
    }
}

/// [`restage_from_value`].
impl<'c> FromArg<'c> for Restage {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        restage_from_value(table, args.get(i))
    }
}

/// [`restage_to_value`].
impl<'c> IntoReply<'c> for Restage {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(restage_to_value(table, &self))
    }
}

/// A rewrite stage's handler the binding lent in: the entry's `Arc`, cloned.
impl<'c> FromArg<'c> for Arc<dyn RewriteStage> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        match table.entry(args.handle(i)?)? {
            Entry::RewriteStage(stage) => Ok(Arc::clone(stage)),
            other => Err(mismatch(HandleKind::RewriteStage, other.kind())),
        }
    }
}

/// A restaged argument, lent as its entry.
impl<'c> IntoReply<'c> for Box<dyn RestagedArgument> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::RestagedArgument(self))
    }
}

/// A restaged slot, lent as its entry.
impl<'c> IntoReply<'c> for Box<dyn RestagedSlot> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::RestagedSlot(self))
    }
}

/// The restaged arguments given to `restage_invocation`, each taken out of
/// the table once ([`bundles_from_table`]).
impl<'c> FromArg<'c> for Vec<Box<dyn RestagedArgument>> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        bundles_from_table(table, args.get(i), HandleKind::RestagedArgument, |entry| match entry {
            Entry::RestagedArgument(bundle) => Some(bundle.take_core(Token::new()).map(|core| {
                Box::new(crate::adapter::rewrite::RestagedArgumentFacade::new(core)) as Box<dyn RestagedArgument>
            })),
            _ => None,
        })
    }
}

/// The restaged slots given to `restage_invocation`, each taken out of the
/// table once ([`bundles_from_table`]).
impl<'c> FromArg<'c> for Vec<Box<dyn RestagedSlot>> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        bundles_from_table(table, args.get(i), HandleKind::RestagedSlot, |entry| match entry {
            Entry::RestagedSlot(bundle) => Some(
                bundle
                    .take_core(Token::new())
                    .map(|core| Box::new(crate::adapter::rewrite::RestagedSlotFacade::new(core)) as Box<dyn RestagedSlot>),
            ),
            _ => None,
        })
    }
}

/// The boxed façades a method answers, lent as their entry.
macro_rules! answered_lent {
    ($($variant:ident: $type:ty),+ $(,)?) => {
        $(
            impl<'c> IntoReply<'c> for $type {
                fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
                    lent(table, Entry::$variant(self))
                }
            }
        )+
    };
}

answered_lent!(KeyVals: Box<dyn KeyVals<'c> + 'c>, KeyValEntry: Box<dyn KeyValEntry<'c> + 'c>);

// The service slots and the host services.

/// A contribution to a service slot: a handle of kind `citation_provider`
/// or `template_module` (a handler object the binding lent in), or data, as
/// it is.
impl<'c> FromArg<'c> for Contributed {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let value = args.get(i);
        if !is_handle(value) {
            return Ok(Contributed::Data(value.clone()));
        }
        match table.entry(handle_from_value(value)?)? {
            Entry::CitationProvider(provider) => Ok(Contributed::CitationProvider(Arc::clone(provider))),
            Entry::TemplateModule(module) => Ok(Contributed::TemplateModule(Arc::clone(module))),
            other => Err(ExtError::Argument {
                method: String::new(),
                detail: format!(
                    "the value contributed to a service slot must be data, a citation provider or a template module, \
                     not an object of the kind `{}`",
                    other.kind().name()
                ),
            }),
        }
    }
}

/// The scope of a file read: `"extension"` or `"document"`; any other value
/// is refused by name.
impl<'c> FromArg<'c> for FilesScope {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let value = args.get(i);
        FilesScope::from_value(value).map_err(|_| {
            let given = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            ExtError::Argument {
                method: String::new(),
                detail: format!("the scope of a file read must be `extension` or `document`, not `{given}`"),
            }
        })
    }
}

impl<'c> IntoReply<'c> for FilesScope {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(self.to_value())
    }
}

/// A host service lent by `StageCx::host`: a handle of kind `files` or
/// `host_service`.
impl<'c> IntoReply<'c> for HostHandle {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        match self {
            HostHandle::Files(files) => lent(table, Entry::Files(files)),
            HostHandle::Service(service) => lent(table, Entry::HostService(service)),
        }
    }
}

impl<'c> IntoReply<'c> for Box<dyn Files> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::Files(self))
    }
}

impl<'c> IntoReply<'c> for Box<dyn HostService> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::HostService(self))
    }
}

/// The boxed façades with no lifetime of a tree, lent as their entry: a box
/// that outlives the call (`template` answers a `'static` one) is lent as
/// one of the call.
macro_rules! answered_lent_outliving {
    ($($variant:ident: $trait:path),+ $(,)?) => {
        $(
            impl<'c, 'a: 'c> IntoReply<'c> for Box<dyn $trait + 'a> {
                fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
                    lent(table, Entry::$variant(self))
                }
            }
        )+
    };
}

answered_lent_outliving!(
    ModeVariant: ModeVariant,
    CompiledTemplate: CompiledTemplate,
    PlaceholderBindings: PlaceholderBindings,
);

// The citation providers.

/// A citation entry a provider's context made, lent as its entry.
impl<'c> IntoReply<'c> for Box<dyn CitationEntry> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::CitationEntry(self))
    }
}

/// The parts of `ProviderCx::build_entry`: a list, each part in the serde
/// form of [`EntryPiece`].
impl<'c> FromArg<'c> for Vec<EntryPiece> {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let value = args.get(i);
        let Value::Array(items) = value else { return Err(args.bad(i, "a list of the parts of a citation entry")) };
        items.iter().map(EntryPiece::from_value).collect()
    }
}

/// A part of a citation entry in its serde form: `{"text"}`, `{"source"}`,
/// `{"emphasized": [parts]}`, `{"strong": [parts]}` or `{"link": {"address",
/// "content": [parts]}}`.
impl JsonForm for EntryPiece {
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        serde_json::from_value(value.clone()).map_err(|_| ExtError::Value {
            detail: format!(
                "a part of a citation entry must be `{{ text }}`, `{{ source }}`, `{{ emphasized: [parts] }}`, \
                 `{{ strong: [parts] }}` or `{{ link: {{ address, content: [parts] }} }}`, not `{value}`"
            ),
        })
    }
}

/// `{"error": text}`: a provider's failure for one request, an item of its
/// answer ([`citation_results_from_value`]).
impl JsonForm for CitationProviderError {
    fn to_value(&self) -> Value {
        json!({ "error": self.detail })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a citation provider's failure";
        let map = fields(WHAT, value, &["error"])?;
        Ok(CitationProviderError::new(required_text(WHAT, &map, "error")?))
    }
}

/// The words that name a handler object in a message: "the citation
/// provider `bib.file` of the feature `bib` of the package `_` of the
/// extension `bib`", for `what` "citation provider".
pub fn handler_named(what: &str, name: &str, feature: &str, package: &str, stem: &str) -> String {
    format!("the {what} `{name}` of the feature `{feature}` of the package `{package}` of the extension `{stem}`")
}

/// The requests of a call of a citation provider as data: a list of `{
/// "key": … }`, the shape a binding hands the provider.
pub fn citation_requests_to_value(requests: &[CitationRequest]) -> Value {
    Value::Array(requests.iter().map(|request| json!({ "key": request.key })).collect())
}

/// A citation provider's answer read from data, its entries resolved in
/// `table`: a list with one item per request (the count is not checked
/// here), each `null`, `{ "error": text }`, or `{ "entry": <handle of kind
/// citation_entry>, "identity": text | null, "warnings": [text] }` (the last
/// two optional). The adapter reports a failure here inside the
/// provider's failure, which names the provider, so the text says "the
/// answer".
///
/// # Errors
///
/// An answer of another shape ([`ExtError::Value`]); a stale entry handle,
/// or a handle of another kind.
pub fn citation_results_from_value(table: &mut CallTable<'_>, value: &Value) -> Result<Vec<CitationResult>, ExtError> {
    let shape = || ExtError::Value {
        detail: "the answer must be a list with one item per request, each `null`, `{ error }` or `{ entry, identity, \
                 warnings }`"
            .to_string(),
    };
    let Value::Array(items) = value else { return Err(shape()) };
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let map = match item {
            Value::Null => {
                results.push(Ok(None));
                continue;
            }
            Value::Object(map) if !is_handle(item) => map,
            _ => return Err(shape()),
        };
        if map.contains_key("error") {
            results.push(Err(CitationProviderError::from_value(item).map_err(|_| shape())?));
            continue;
        }
        if map.keys().any(|key| !["entry", "identity", "warnings"].contains(&key.as_str())) {
            return Err(shape());
        }
        let entry = map.get("entry").ok_or_else(shape)?;
        if !is_handle(entry) {
            return Err(shape());
        }
        let entry: Box<dyn CitationEntry> = match table.entry(handle_from_value(entry)?)? {
            Entry::CitationEntry(entry) => Box::new(CitationEntryFacade(entry.core_entry(Token::new()))),
            other => return Err(mismatch(HandleKind::CitationEntry, other.kind())),
        };
        let mut answer = CitationAnswer::new(entry);
        match map.get("identity") {
            None | Some(Value::Null) => {}
            Some(Value::String(identity)) => answer = answer.with_identity(identity.clone()),
            Some(_) => return Err(shape()),
        }
        match map.get("warnings") {
            None => {}
            Some(Value::Array(warnings)) => {
                for warning in warnings {
                    let Value::String(warning) = warning else { return Err(shape()) };
                    answer = answer.with_warning(warning.clone());
                }
            }
            Some(_) => return Err(shape()),
        }
        results.push(Ok(Some(answer)));
    }
    Ok(results)
}

// The template modules.

/// What a template module's `files` and `render` receive as `data`: `{
/// "html", "regions": [[name, text], …], "pieces": [{ "name", "place",
/// "text", "words" }, …], "meta", "config", "artifacts": [[name, path], …]
/// }` (a pair is a list of its two items).
///
/// # Errors
///
/// The document's metadata or the template's settings hold a value that
/// cannot cross ([`ExtError::Value`]: a number that is not an integer, named
/// by its key path inside the data, `meta.version`; a reserved key).
pub fn template_context_to_value(context: &TemplateContext) -> Result<Value, ExtError> {
    let value = context.to_value();
    if let Some((path, number)) = number_not_crossing(&value, "") {
        return Err(ExtError::Value {
            detail: format!("`{path}` of the data of a compose template must be an integer, not `{number}`"),
        });
    }
    check_data(&value)?;
    Ok(value)
}

/// The data of a template module's `files` and `render` as JSON (see
/// [`template_context_to_value`], which also checks that it can cross).
impl JsonForm for TemplateContext {
    fn to_value(&self) -> Value {
        json!({
            "html": self.html,
            "regions": self.regions.to_value(),
            "pieces": self.pieces.to_value(),
            "meta": self.meta,
            "config": Value::Object(self.config.clone()),
            "artifacts": self.artifacts.to_value(),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "the data of a compose template";
        let map = fields(WHAT, value, &["html", "regions", "pieces", "meta", "config", "artifacts"])?;
        let config = match field(&map, "config") {
            Value::Object(config) => config.clone(),
            other => return Err(ExtError::Value { detail: format!("the field `config` of {WHAT} must be a map, not `{other}`") }),
        };
        Ok(TemplateContext {
            html: required_form(WHAT, &map, "html")?,
            regions: required_form(WHAT, &map, "regions")?,
            pieces: required_form(WHAT, &map, "pieces")?,
            meta: field(&map, "meta").clone(),
            config,
            artifacts: required_form(WHAT, &map, "artifacts")?,
        })
    }
}

/// `{"name", "place", "text", "words"}`: a piece of a document, inside the
/// data of a template module ([`TemplateContext`]).
impl JsonForm for PieceValue {
    fn to_value(&self) -> Value {
        json!({ "name": self.name, "place": self.place, "text": self.text, "words": self.words })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a piece of a document";
        let map = fields(WHAT, value, &["name", "place", "text", "words"])?;
        Ok(PieceValue {
            name: required_text(WHAT, &map, "name")?,
            place: required_text(WHAT, &map, "place")?,
            text: required_text(WHAT, &map, "text")?,
            words: required_text(WHAT, &map, "words")?,
        })
    }
}

/// The key path inside `value` (from `path`) of its first number that cannot
/// cross ([`check_data`]: not an integer, or out of the range of `i64`), with
/// the number's text; `None` when there is none.
fn number_not_crossing(value: &Value, path: &str) -> Option<(String, String)> {
    match value {
        Value::Number(number) if !number.is_i64() => Some((path.to_string(), number.to_string())),
        Value::Array(items) => {
            items.iter().enumerate().find_map(|(index, item)| number_not_crossing(item, &format!("{path}[{index}]")))
        }
        Value::Object(map) => map.iter().find_map(|(key, item)| {
            let path = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
            number_not_crossing(item, &path)
        }),
        _ => None,
    }
}

/// A template module's description, as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptionData {
    output: String,
    accepts: Value,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    config: serde_json::Map<String, Value>,
}

/// `"all"`, or a list of piece names: the pieces a template module accepts.
impl JsonForm for Accepts {
    fn to_value(&self) -> Value {
        match self {
            Accepts::All => Value::String("all".to_string()),
            Accepts::Only(names) => Value::Array(names.iter().map(|name| Value::String(name.to_string())).collect()),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let shape = || ExtError::Value {
            detail: format!("the pieces a template module accepts must be `all` or a list of piece names, not `{value}`"),
        };
        match value {
            Value::String(all) if all == "all" => Ok(Accepts::All),
            Value::Array(names) => names
                .iter()
                .map(|name| name.as_str().map(Name::from))
                .collect::<Option<Vec<Name>>>()
                .map(Accepts::Only)
                .ok_or_else(shape),
            _ => Err(shape()),
        }
    }
}

/// `{"output", "accepts", "requires", "config"}`, `accepts` as [`Accepts`]
/// writes it; [`template_description_from_value`] reads it, naming the
/// module in its error.
impl JsonForm for TemplateDescription {
    fn to_value(&self) -> Value {
        json!({
            "output": self.output,
            "accepts": self.accepts.to_value(),
            "requires": self.requires,
            "config": Value::Object(self.config.clone()),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let data: DescriptionData = serde_json::from_value(value.clone()).map_err(|_| ExtError::Value {
            detail: "a template module's description must be `{ output, accepts, requires, config }`".to_string(),
        })?;
        Ok(TemplateDescription {
            output: data.output,
            accepts: Accepts::from_value(&data.accepts)?,
            requires: data.requires,
            config: data.config,
        })
    }
}

/// A template module's description read from data: `{ "output", "accepts",
/// "requires", "config" }`, `accepts` `"all"` or a list of piece names,
/// `requires` (a list of piece names) and `config` (a map) optional, no
/// other key. `who` names the module ([`handler_named`]).
///
/// # Errors
///
/// A description of another shape ([`ExtError::Value`], naming `who`).
pub fn template_description_from_value(value: &Value, who: &str) -> Result<TemplateDescription, ExtError> {
    let shape = || ExtError::Value {
        detail: format!(
            "the description {who} answers must be `{{ output, accepts, requires, config }}`, with `accepts` `all` or \
             a list of piece names"
        ),
    };
    TemplateDescription::from_value(value).map_err(|_| shape())
}

/// One file a template module ships, as written.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum FileData {
    Content(ContentData),
    File(FileRefData),
}

/// A file whose text the module computed, as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentData {
    name: String,
    extension: String,
    media_type: String,
    text: String,
}

/// A file of one of the two roots, as written.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRefData {
    name: String,
    file: String,
    scope: Value,
    #[serde(default)]
    media_type: Option<String>,
}

/// The files a template module ships, read from data: a list, each `{
/// "name", "extension", "media_type", "text" }` or `{ "name", "file",
/// "scope", "media_type" }` (`scope` `"extension"` or `"document"`,
/// `media_type` optional).
///
/// # Errors
///
/// A list of another shape, or an unknown scope ([`ExtError::Value`]).
pub fn template_files_from_value(value: &Value) -> Result<Vec<TemplateFile>, ExtError> {
    let items: Vec<FileData> = serde_json::from_value(value.clone()).map_err(|_| files_shape())?;
    items.into_iter().map(file_from_data).collect()
}

/// The error of a file of a template module of another shape.
fn files_shape() -> ExtError {
    ExtError::Value {
        detail: String::from(
            "each file it ships must be `{ name, extension, media_type, text }` or `{ name, file, scope, media_type }`",
        ),
    }
}

/// A file a template module ships, from its form as written.
fn file_from_data(item: FileData) -> Result<TemplateFile, ExtError> {
    match item {
        FileData::Content(ContentData { name, extension, media_type, text }) => {
            Ok(TemplateFile::Content { name, extension, media_type, text })
        }
        FileData::File(FileRefData { name, file, scope, media_type }) => {
            let scope = FilesScope::from_value(&scope).map_err(|_| {
                let given = match &scope {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                ExtError::Value { detail: format!("the scope of a file read must be `extension` or `document`, not `{given}`") }
            })?;
            Ok(TemplateFile::File { name, path: file, scope, media_type })
        }
    }
}

/// One file a template module ships: `{"name", "extension", "media_type",
/// "text"}` or `{"name", "file", "scope", "media_type"}`; a list of them is
/// read by [`template_files_from_value`].
impl JsonForm for TemplateFile {
    fn to_value(&self) -> Value {
        match self {
            TemplateFile::Content { name, extension, media_type, text } => {
                json!({ "name": name, "extension": extension, "media_type": media_type, "text": text })
            }
            TemplateFile::File { name, path, scope, media_type } => {
                json!({ "name": name, "file": path, "scope": scope.to_value(), "media_type": media_type })
            }
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let item: FileData = serde_json::from_value(value.clone()).map_err(|_| files_shape())?;
        file_from_data(item)
    }
}

// The crossing types of E4's mirror (WP-2a).

/// [`aspect_from_value`], its handles resolved in the table.
impl<'c> FromArg<'c> for Aspect {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        aspect_from_value(args.get(i), &mut |held| held_from_table(table, held))
    }
}

/// [`scoped_package_def_from_table`].
impl<'c> FromArg<'c> for ScopedPackageDef {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        scoped_package_def_from_table(table, args.get(i))
    }
}

/// The entry's `Arc`, cloned (`Entry::LateInit`).
impl<'c> FromArg<'c> for Arc<dyn LateInit> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        match table.entry(args.handle(i)?)? {
            Entry::LateInit(hook) => Ok(Arc::clone(hook)),
            other => Err(mismatch(HandleKind::LateInit, other.kind())),
        }
    }
}

/// The entry's `Arc`, cloned (`Entry::Finalizer`).
impl<'c> FromArg<'c> for Arc<dyn Finalizer> {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        match table.entry(args.handle(i)?)? {
            Entry::Finalizer(hook) => Ok(Arc::clone(hook)),
            other => Err(mismatch(HandleKind::Finalizer, other.kind())),
        }
    }
}

/// An argument a helper made, lent as `Entry::ArgDef`: an argument list
/// takes the handle in place of a code ([`arg_defs_from_value`]).
impl<'c> IntoReply<'c> for ArgDef {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        lent(table, Entry::ArgDef(self))
    }
}

/// A label `{"ref_type", "ref_label"}`.
fn label_to_value(label: &Label) -> Value {
    json!({ "ref_type": label.ref_type, "ref_label": label.ref_label })
}

/// A label read from `{"ref_type", "ref_label"}`.
pub(crate) fn label_from_value(value: &Value) -> Result<Label, ExtError> {
    const WHAT: &str = "a label";
    let map = fields(WHAT, value, &["ref_type", "ref_label"])?;
    Ok(Label::new(required_text(WHAT, &map, "ref_type")?, required_text(WHAT, &map, "ref_label")?))
}

/// `{"ref_type", "ref_label"}`.
impl<'c> IntoReply<'c> for Label {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(label_to_value(&self))
    }
}

/// A label `{"ref_type", "ref_label"}`.
impl<'c> FromArg<'c> for Label {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        label_from_value(args.get(i))
    }
}

/// A label `{"ref_type", "ref_label"}`, or `null` for none.
impl<'c> FromArg<'c> for Option<Label> {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        match args.get(i) {
            Value::Null => Ok(None),
            value => label_from_value(value).map(Some),
        }
    }
}

/// `{"ref_type", "ref_label"}` (`label_to_value`, `label_from_value`).
impl JsonForm for Label {
    fn to_value(&self) -> Value {
        label_to_value(self)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        label_from_value(value)
    }
}

/// `{"text", "span": {"start", "end"}, "position"}`; read by the reader of a
/// reference point's malformed labels (`adapter/aspects.rs`).
impl JsonForm for MalformedLabel {
    fn to_value(&self) -> Value {
        json!({ "text": self.text, "span": self.span.to_value(), "position": self.position })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        crate::adapter::aspects::malformed_label_from_value(value)
    }
}

/// `"any"` or `{"these": [type, …]}`; read by the reader of the label types
/// a reference point accepts (`adapter/aspects.rs`).
impl JsonForm for AcceptedRefTypes {
    fn to_value(&self) -> Value {
        match self {
            AcceptedRefTypes::Any => Value::String("any".to_string()),
            AcceptedRefTypes::These(types) => json!({ "these": types }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        crate::adapter::aspects::accepted_from_value(value)
    }
}

/// `{"text", "span": {"start", "end"}, "label": {"ref_type", "ref_label"} |
/// null}`.
impl JsonForm for LabelUse {
    fn to_value(&self) -> Value {
        json!({ "text": self.text, "span": self.span.to_value(), "label": self.label.to_value() })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a label use";
        let map = fields(WHAT, value, &["text", "span", "label"])?;
        let span = Span::from_value(field(&map, "span"))?;
        let label = match field(&map, "label") {
            Value::Null => None,
            label => Some(label_from_value(label)?),
        };
        Ok(LabelUse { text: required_text(WHAT, &map, "text")?, span, label })
    }
}

/// [`MalformedLabel`]'s form.
impl<'c> IntoReply<'c> for MalformedLabel {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(self.to_value())
    }
}

/// [`LabelUse`]'s form.
impl<'c> IntoReply<'c> for LabelUse {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(self.to_value())
    }
}

/// A list of label uses, each `{"text", "span": {"start", "end"}, "label":
/// {"ref_type", "ref_label"} | null}`, as [`label_uses`](crate::api::stdcore::features::refs::label_uses)
/// answers them.
impl<'c> FromArg<'c> for Vec<LabelUse> {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        let Value::Array(items) = args.get(i) else { return Err(args.bad(i, "a list of label uses")) };
        items.iter().map(LabelUse::from_value).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `value`, checks the JSON is `expected`, reads it back.
    fn round_trip<T: JsonForm + PartialEq + std::fmt::Debug>(value: T, expected: Value) {
        let written = value.to_value();
        assert_eq!(written, expected);
        assert_eq!(T::from_value(&written), Ok(value));
    }

    #[test]
    fn an_output_crosses_as_its_fields() {
        let def = OutputDef::new("tb", "tb.document", "tb.files", "application/x-typst", "typ").docs("Typst.");
        round_trip(
            def,
            json!({
                "name": "tb", "capability": "tb.document", "table": "tb.files", "media_type": "application/x-typst",
                "extension": "typ", "made_from": [], "docs": "Typst.",
            }),
        );
        let pdf = OutputDef::new("tp", "tp.document", "tp.files", "application/pdf", "pdf").made_from("latex-rendered");
        round_trip(
            pdf,
            json!({
                "name": "tp", "capability": "tp.document", "table": "tp.files", "media_type": "application/pdf",
                "extension": "pdf", "made_from": ["latex-rendered"], "docs": "",
            }),
        );
        let bare = json!({ "name": "tb", "capability": "c", "table": "t", "media_type": "", "extension": "x" });
        assert_eq!(OutputDef::from_value(&bare).map(|def| (def.made_from, def.docs)), Ok((Vec::new(), String::new())));
        let error = OutputDef::from_value(&json!({ "name": "tb" })).expect_err("fields missing");
        assert_eq!(error.kind(), "value");
    }

    #[test]
    fn an_effect_crosses_as_its_kebab_case_name() {
        round_trip(Effect::Compute, json!("compute"));
        round_trip(Effect::Check, json!("check"));
        round_trip(Effect::Io, json!("io"));
        assert_eq!(
            Effect::from_value(&json!("Compute")),
            Err(ExtError::Value { detail: "an effect must be one of `compute`, `check` or `io`, not `Compute`".to_string() })
        );
    }

    #[test]
    fn a_scope_crosses_as_its_kebab_case_name() {
        round_trip(Scope::PerTree, json!("per-tree"));
        round_trip(Scope::Document, json!("document"));
    }

    #[test]
    fn a_body_with_a_parser_of_its_own_is_written_whole_and_refused_by_its_reader() {
        let verbatim = BodyDef::new().verbatim().to_value();
        assert_eq!(verbatim["verbatim"], json!(true));
        assert_eq!(verbatim.get("parser"), None);
        let own = flm_core::lang::mint::FlmUnknownEnvironmentBehavior::new();
        let written = BodyDef::new().parser(flm_core::lang::mint::FlmUnknownEnvironmentBehavior::new()).to_value();
        assert_eq!(written["verbatim"], json!(false));
        assert_eq!(written["parser"], json!(own.description()));
        assert_eq!(
            body_def_from_value(&written).err(),
            Some(ExtError::Value {
                detail: "a body must hold only the fields `layout`, `verbatim`, `mode`, `scoped`, not `parser`".to_string()
            })
        );
    }

    #[test]
    fn a_preamble_piece_its_shape_cannot_hold_is_written_whole_and_refused_by_its_reader() {
        let refusal = |field: &str| ExtError::Value {
            detail: format!("a preamble piece must hold only the fields `name`, `content`, `docs`, `requires`, not `{field}`"),
        };
        let piece = LatexPreambleDef::new("x.p", "\\def\\x{X}", "D.");
        let mut delivering = piece.clone();
        delivering.delivers = vec![Name::from("x.style")];
        let written = delivering.to_value();
        assert_eq!(written["delivers"], json!(["x.style"]));
        assert_eq!(LatexPreambleDef::from_value(&written).err(), Some(refusal("delivers")));
        let mut unprefixed = piece;
        unprefixed.id = Name::from("x.p");
        let written = unprefixed.to_value();
        assert_eq!(written["id"], json!("x.p"));
        assert_eq!(written.get("name"), None);
        assert_eq!(LatexPreambleDef::from_value(&written).err(), Some(refusal("id")));
    }

    #[test]
    fn a_mode_crosses_as_its_serde_name() {
        round_trip(Mode::Text, json!("text"));
        round_trip(Mode::Math, json!("math"));
        assert!(Mode::from_value(&json!(3)).is_err());
    }

    #[test]
    fn the_callable_and_fallback_kinds_cross_as_their_names() {
        round_trip(CallableKind::Macro, json!("macro"));
        round_trip(CallableKind::Environment, json!("environment"));
        round_trip(CallableKind::Specials, json!("specials"));
        round_trip(FallbackKind::Macro, json!("macro"));
        round_trip(FallbackKind::Environment, json!("environment"));
    }

    #[test]
    fn a_definition_s_kind_and_modes_cross_as_their_tokens() {
        round_trip(DefinitionKind::Macro, json!("macro"));
        round_trip(DefinitionKind::Environment, json!("environment"));
        round_trip(DefinitionKind::Specials, json!("specials"));
        for modes in [DefinitionModes::Both, DefinitionModes::Text, DefinitionModes::Math] {
            round_trip(modes, json!(modes.token()));
        }
    }

    #[test]
    fn the_layouts_cross_as_their_kebab_case_names() {
        round_trip(ArgLayout::Inline, json!("inline"));
        round_trip(ArgLayout::InlineOrBlocks, json!("inline-or-blocks"));
        for layout in [ArgLayout::Inline, ArgLayout::Blocks, ArgLayout::InlineOrBlocks] {
            let own: Result<ArgLayout, _> = serde_json::from_value(layout.to_value());
            assert_eq!(own.ok(), Some(layout));
        }
        round_trip(BodyLayout::Inline, json!("inline"));
        round_trip(BodyLayout::Blocks, json!("blocks"));
        round_trip(BodyLayout::FromRole, json!("from-role"));
    }

    #[test]
    fn an_expansion_policy_crosses_in_the_form_it_reads() {
        round_trip(ExpandPolicy::Always, json!("always"));
        round_trip(ExpandPolicy::PerPlan, json!("per-plan"));
        let own: Result<ExpandPolicy, _> = serde_json::from_value(json!("per-plan"));
        assert_eq!(own.ok(), Some(ExpandPolicy::PerPlan));
    }

    #[test]
    fn a_block_role_crosses_in_the_form_it_reads() {
        round_trip(BlockRole::Block, json!("block"));
        round_trip(BlockRole::DEFAULT, json!({ "inline": { "absorb_before": false, "absorb_after": false } }));
        round_trip(
            BlockRole::Inline(InlineSpacing { absorb_before: true, absorb_after: false }),
            json!({ "inline": { "absorb_before": true, "absorb_after": false } }),
        );
        round_trip(BlockRole::BlockDecorator(Placement::Leads), json!({ "block-decorator": "leads" }));
        round_trip(Placement::Trails, json!("trails"));
        round_trip(Placement::Free, json!("free"));
        round_trip(InlineSpacing::CONTENT, json!({ "absorb_before": false, "absorb_after": false }));
    }

    #[test]
    fn a_node_kind_and_a_severity_cross_as_their_names() {
        round_trip(NodeKindName::Chars, json!("chars"));
        round_trip(NodeKindName::List, json!("list"));
        assert_eq!(serde_json::to_value(NodeKindName::Callable).ok(), Some(NodeKindName::Callable.to_value()));
        round_trip(Severity::Error, json!("error"));
        round_trip(Severity::Warning, json!("warning"));
        round_trip(Severity::Note, json!("note"));
    }

    #[test]
    fn a_stage_declaration_crosses_as_its_six_fields() {
        let decl = StageDecl::new("wc.count", Effect::Compute, Scope::PerTree)
            .provides("wc.counted")
            .writes("wc.words")
            .docs("Counts words.");
        round_trip(
            decl,
            json!({
                "name": "wc.count", "effect": "compute", "scope": "per-tree",
                "provides": ["wc.counted"], "writes": ["wc.words"], "docs": "Counts words.",
            }),
        );
        let short = StageDecl::from_value(&json!({ "name": "wc.check", "effect": "check", "scope": "document" }));
        assert_eq!(short, Ok(StageDecl::new("wc.check", Effect::Check, Scope::Document)));
        assert!(StageDecl::from_value(&json!({ "name": "x", "effect": "compute", "scope": "per-tree", "extra": 1 })).is_err());
        assert!(StageDecl::from_value(&json!({ "name": "x", "effect": "fast", "scope": "per-tree" })).is_err());
    }

    #[test]
    fn needs_cross_as_their_six_lists() {
        let needs = Needs::new().requires("a").reads("b").reads_opt("c").after("d").before("e").option("f.g");
        round_trip(
            needs,
            json!({
                "requires": ["a"], "reads": ["b"], "reads_opt": ["c"], "after": ["d"], "before": ["e"],
                "options": ["f.g"],
            }),
        );
        assert_eq!(Needs::from_value(&json!({})), Ok(Needs::new()));
    }

    #[test]
    fn data_refuses_floats_large_numbers_and_reserved_keys() {
        assert_eq!(check_data(&json!({ "a": [1, -2, "x", null, true] })), Ok(()));
        let refused = |value: Value| check_data(&value).expect_err("refused").to_string();
        assert_eq!(refused(json!([0.5])), "a number must be an integer, not `0.5`");
        assert_eq!(
            refused(json!(u64::MAX)),
            "a number must lie between -9223372036854775808 and 9223372036854775807, not `18446744073709551615`"
        );
        assert_eq!(refused(json!({ "b": { "$x": 1 } })), "a map key must not begin with `$`, as `$x` does");
        assert_eq!(refused(json!({ "@handle": 1 })), "the map key `@handle` is reserved");
    }

    #[test]
    fn an_argument_list_reads_codes_and_maps() {
        let args = arg_defs_from_value(&json!("[{"), &mut no_handles).expect("two codes");
        assert_eq!(args.len(), 2);
        let args = arg_defs_from_value(&json!(["{", { "code": "[", "name": "options" }]), &mut no_handles).expect("a list");
        assert_eq!(args[1].name.as_deref(), Some("options"));
        assert!(arg_defs_from_value(&Value::Null, &mut no_handles).expect("none").is_empty());
        let error = arg_defs_from_value(&json!("q"), &mut no_handles).expect_err("not a code");
        assert_eq!(error.kind(), "core");
    }

    #[test]
    fn a_syntax_reads_each_of_its_four_shapes() {
        assert!(matches!(syntax_def_from_value(&json!("synthesized"), &mut no_handles), Ok(SyntaxDef::Synthesized)));
        assert!(matches!(
            syntax_def_from_value(&json!({ "macro": { "args": "{" } }), &mut no_handles),
            Ok(SyntaxDef::Macro { args }) if args.len() == 1
        ));
        let environment = syntax_def_from_value(&json!({
            "environment": { "args": [], "body": { "layout": "blocks", "mode": "math" } }
        }), &mut no_handles);
        assert!(matches!(
            environment,
            Ok(SyntaxDef::Environment { body, .. }) if body.layout == BodyLayout::Blocks && body.mode == Some(Mode::Math)
        ));
        assert!(matches!(
            syntax_def_from_value(&json!({ "specials": { "trigger": "~" } }), &mut no_handles),
            Ok(SyntaxDef::Specials { trigger, .. }) if trigger == "~"
        ));
        assert!(syntax_def_from_value(&json!({ "macro": {}, "specials": {} }), &mut no_handles).is_err());
        assert!(syntax_def_from_value(&json!({ "inclusion": {} }), &mut no_handles).is_err());
    }

    #[test]
    fn a_definition_reads_its_fields_with_the_library_s_defaults() {
        let definition = definition_from_value(&json!({
            "kind": "macro", "name": "shout", "args": "{", "content": "#1!", "modes": "text",
            "defaults": { "1": "x" }, "block_role": "block", "expand": "always", "overrides": true,
            "docs": "Shouts.",
        }), &mut no_handles)
        .expect("a definition");
        assert_eq!(definition.name.as_str(), "shout");
        assert_eq!(definition.kind, DefinitionKind::Macro);
        assert_eq!(definition.args.len(), 1);
        assert_eq!(definition.content.as_deref(), Some("#1!"));
        assert_eq!(definition.modes, DefinitionModes::Text);
        assert_eq!(definition.defaults.get("1").map(String::as_str), Some("x"));
        assert_eq!(definition.block_role, BlockRole::Block);
        assert_eq!(definition.expand, ExpandPolicy::Always);
        assert!(definition.overrides);
        assert_eq!(definition.docs, "Shouts.");
        let short = definition_from_value(&json!({ "kind": "environment", "name": "box" }), &mut no_handles).expect("a short one");
        assert_eq!(short.content, None);
        assert_eq!(short.modes, DefinitionModes::Both);
        let error = definition_from_value(&json!({ "kind": "macro", "name": "x", "body": 1 }), &mut no_handles).expect_err("unknown");
        assert!(error.to_string().contains("not `body`"), "{error}");
        assert!(definition_from_value(&json!({ "name": "x" }), &mut no_handles).is_err());
    }

    #[test]
    fn an_element_resolves_its_hook_through_the_caller() {
        struct Keep;
        impl Expand for Keep {
            fn expand<'c>(
                &self,
                _: &'c dyn crate::api::techy::node::Node<'c>,
                _: &'c mut dyn crate::api::lang::rewrites::userdefs::ExpandCx,
                _: &'c mut dyn crate::api::lang::rewrites::userdefs::Expander,
            ) -> Result<crate::api::lang::rewrites::userdefs::Expansion, ExtError> {
                Ok(crate::api::lang::rewrites::userdefs::Expansion::Default)
            }
        }
        let asked = std::cell::Cell::new(0);
        let mut resolve = |_: &Value| -> Result<Held, ExtError> {
            asked.set(asked.get() + 1);
            Ok(Held::Expand(Arc::new(Keep)))
        };
        let value = json!({
            "name": "wc.box",
            "aspects": [{ "BlockRoleAspect": "block" }, { "Expand": { "@handle": { "kind": "expand", "call": 1, "index": 0 } } }],
            "value": { "n": 1 },
        });
        let spec = element_spec_from_value(&value, &mut resolve).expect("an element");
        assert_eq!(spec.aspects().iter().map(|a| a.kind()).collect::<Vec<_>>(), ["BlockRoleAspect", "Expand"]);
        assert_eq!(spec.value, Some(json!({ "n": 1 })));
        assert_eq!(asked.get(), 1);
        let twice = json!({ "name": "wc.box", "aspects": [{ "BlockRoleAspect": "block" }, { "BlockRoleAspect": "block" }] });
        assert_eq!(element_spec_from_value(&twice, &mut resolve).expect_err("twice").kind(), "argument");
        let def = element_def_from_value(
            &json!({ "names": ["box"], "syntax": { "macro": {} }, "element": { "name": "wc.box" }, "modes": ["text"] }),
            &mut resolve,
        )
        .expect("a declaration");
        assert_eq!(def.names(), Some(&["box".to_string()][..]));
        let fallback = fallback_def_from_value(
            &json!({ "modes": ["text"], "kind": "macro", "element": { "name": "wc.unknown" } }),
            &mut resolve,
        )
        .expect("a fallback");
        assert_eq!(fallback.kind, FallbackKind::Macro);
        let core = element_spec_to_core(&spec, &Owner::new("wc", "_", "f")).expect("converted");
        assert!(core.has_aspect::<dyn CoreExpand>());
        assert!(core.has_aspect::<dyn BlockRoleAspect>());
        assert_eq!(core.value_as::<Value>(), Some(&json!({ "n": 1 })));
    }


    #[test]
    fn a_handle_crosses_as_its_marked_map() {
        let handle = Handle { kind: HandleKind::KeyValEntry, call: 17, index: 3 };
        let written = handle_to_value(handle);
        assert_eq!(written, json!({ "@handle": { "kind": "key_val_entry", "call": 17, "index": 3 } }));
        assert_eq!(handle_from_value(&written), Ok(handle));
        for not_one in [json!({ "@handle": { "kind": "nothing", "call": 1, "index": 0 } }), json!({ "kind": "node" }), json!(3)] {
            assert_eq!(handle_from_value(&not_one).map_err(|error| error.kind()), Err("argument"), "{not_one}");
        }
    }

    #[test]
    fn arguments_may_hold_handles_but_no_float_and_no_reserved_key() {
        let handle = handle_to_value(Handle { kind: HandleKind::Node, call: 1, index: 0 });
        assert_eq!(check_arguments(&json!([handle, [handle], { "nodes": [handle] }, 3, "x"])), Ok(()));
        let refused = |value: Value| check_arguments(&value).map_err(|error| error.to_string());
        assert_eq!(refused(json!([1.5])), Err("a number must be an integer, not `1.5`".to_string()));
        assert_eq!(refused(json!([{ "$a": 1 }])), Err("a map key must not begin with `$`, as `$a` does".to_string()));
        assert_eq!(refused(json!([{ "@handle": 1, "b": 2 }])), Err("the map key `@handle` is reserved".to_string()));
        // A handle is not data.
        assert!(check_data(&handle).is_err());
    }

    #[test]
    fn a_span_and_the_characters_of_a_content_cross_as_their_maps() {
        round_trip(Span { start: 12, end: 20 }, json!({ "start": 12, "end": 20 }));
        round_trip(
            CharsContent { text: Some("ab".to_string()), has_placeholder: false },
            json!({ "text": "ab", "has_placeholder": false }),
        );
        round_trip(CharsContent { text: None, has_placeholder: true }, json!({ "text": null, "has_placeholder": true }));
    }

    #[test]
    fn a_condition_s_data_converts_back_to_json() {
        let value = json!({ "a": [1, "x", null, true], "b": {} });
        let converted = diagnostic_value(&value).expect("data");
        assert_eq!(diagnostic_value_to_json(&converted), value);
    }

    #[test]
    fn an_argument_is_written_with_the_code_it_was_read_from() {
        let arg = crate::api::lang::mint::arg_def_from_code("[").expect("a code");
        assert_eq!(
            arg_def_to_value(&arg),
            json!({ "code": "[", "name": null, "layout": "inline", "docs": "", "mode": null, "scoped": [], "role": null })
        );
    }

    #[test]
    fn an_argument_s_role_is_read_and_written_back() {
        let written = json!({ "code": "{", "name": "key", "layout": "inline", "docs": "", "mode": null, "scoped": [], "role": "wc.key" });
        let data: ArgData = serde_json::from_value(written.clone()).expect("the argument map");
        let arg = ArgDef::try_from(data).expect("a code");
        assert_eq!(arg.role.as_ref().map(Name::as_str), Some("wc.key"));
        assert_eq!(arg_def_to_value(&arg), written);
    }

    #[test]
    fn an_expansion_crosses_with_its_staged_nodes_as_handles() {
        let mut table = CallTable::new(3);
        assert_eq!(expansion_from_value(&mut table, &json!("default")), Ok(Expansion::Default));
        assert_eq!(expansion_to_value(&mut table, &Expansion::Default), json!("default"));
        assert_eq!(expansion_to_value(&mut table, &Expansion::Emit(Vec::new())), json!({ "emit": [] }));
        assert_eq!(expansion_from_value(&mut table, &json!({ "emit": [] })), Ok(Expansion::Emit(Vec::new())));
        for refused in [json!("emit"), json!({ "emit": [], "other": 1 }), json!({ "emit": 3 })] {
            assert_eq!(expansion_from_value(&mut table, &refused).map_err(|error| error.kind()), Err("value"), "{refused}");
        }
        let not_staged = Handle { kind: HandleKind::Staged, call: 3, index: 0 };
        let definition = crate::api::lang::rewrites::userdefs::UserDefinition::new(Definition::macro_("x", Vec::new()));
        table.lend(Entry::UserDefinition(definition));
        let error = expansion_from_value(&mut table, &json!({ "emit": [handle_to_value(not_staged)] })).expect_err("a definition");
        assert_eq!(error.to_string(), "an object of the kind `staged` is expected here, not one of the kind `user_definition`");
    }

    #[test]
    fn a_diagnostic_needs_its_identifier_severity_and_message() {
        let mut table = CallTable::new(4);
        let diagnostic = diagnostic_from_value(&mut table, &json!({ "id": "wc.x", "severity": "warning", "message": "m" }))
            .expect("a diagnostic");
        assert_eq!((diagnostic.id.as_str(), diagnostic.severity, diagnostic.data.clone()), ("wc.x", Severity::Warning, json!({})));
        assert!(diagnostic.at.is_none());
        let missing = diagnostic_from_value(&mut table, &json!({ "id": "wc.x", "message": "m" })).expect_err("no severity");
        assert_eq!(missing.to_string(), "a diagnostic must have the field `severity`");
        let float = diagnostic_from_value(&mut table, &json!({ "id": "wc.x", "severity": "note", "message": "m", "data": 0.5 }));
        assert_eq!(float.map_err(|error| error.kind()).map(|_| ()), Err("value"));
    }

    // --- the conversion traits: one case per implementation ----------------------------------

    /// Reads `value` as the argument 0 of a call, as `T`.
    fn arg<'c, T: FromArg<'c>>(table: &mut CallTable<'c>, value: Value) -> Result<T, ExtError> {
        let list = json!([value]);
        T::from_arg(table, &Args::new(&list)?, 0)
    }

    /// The text of the error `result` holds.
    fn error_of<T>(result: Result<T, ExtError>) -> String {
        match result {
            Ok(_) => panic!("an error was expected"),
            Err(error) => error.to_string(),
        }
    }

    /// The root of a parsed fragment, as the binding lends nodes.
    fn root() -> Box<dyn Node<'static>> {
        use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
        use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
        use flm_core::model::doc::FragmentLayout;
        let mut builder = RegistryBuilder::new();
        builder.use_package(flm_core::stdcore::package(flm_core::stdcore::StdcoreConfig::new()));
        let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
        let source = Arc::new(FlmSource::new("ab \\emph{cd}").with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the fragment parses");
        let tree = Arc::clone(fragment.tree());
        let id = tree.root().id();
        Box::new(crate::adapter::node::NodeFacade::shared(tree, id))
    }

    /// A rule's body that answers the default.
    struct Body;
    impl Lower for Body {
        fn lower<'c>(
            &self,
            _: &'c dyn Node<'c>,
            _: &'c mut dyn crate::api::core::lowering::LowerCx,
            _: &'c mut dyn crate::api::core::lowering::Lowerer,
        ) -> Result<Lowering, ExtError> {
            Ok(Lowering::Default)
        }
    }

    /// A hook that answers the default.
    struct Hook;
    impl Expand for Hook {
        fn expand<'c>(
            &self,
            _: &'c dyn Node<'c>,
            _: &'c mut dyn crate::api::lang::rewrites::userdefs::ExpandCx,
            _: &'c mut dyn crate::api::lang::rewrites::userdefs::Expander,
        ) -> Result<Expansion, ExtError> {
            Ok(Expansion::Default)
        }
    }

    /// A stage that does nothing.
    struct Quiet;
    impl Stage for Quiet {
        fn decl(&self) -> StageDecl {
            StageDecl::new("x.quiet", Effect::Compute, Scope::Document)
        }
        fn needs(&self) -> Needs {
            Needs::new()
        }
        fn run<'c>(
            &self,
            _: &'c dyn crate::api::core::run::StageCx,
            _: &'c mut dyn crate::api::core::data::store::StageOut,
        ) -> Result<(), ExtError> {
            Ok(())
        }
    }

    #[test]
    fn the_scalars_and_data_cross_as_the_argument_readers_read_them() {
        let mut table = CallTable::new(1);
        let wrong = json!([{ "a": 1 }]);
        let wrong = Args::new(&wrong).expect("a list");
        // String
        assert_eq!(arg::<String>(&mut table, json!("ab")), Ok("ab".to_string()));
        assert_eq!("ab".to_string().into_reply(&mut table), Ok(json!("ab")));
        assert_eq!(error_of(arg::<String>(&mut table, json!({ "a": 1 }))), error_of(wrong.text(0)));
        // usize
        assert_eq!(arg::<usize>(&mut table, json!(3)), Ok(3));
        assert_eq!(3usize.into_reply(&mut table), Ok(json!(3)));
        assert_eq!(error_of(arg::<usize>(&mut table, json!({ "a": 1 }))), error_of(wrong.index(0)));
        // bool
        assert_eq!(arg::<bool>(&mut table, json!(true)), Ok(true));
        assert_eq!(false.into_reply(&mut table), Ok(json!(false)));
        assert_eq!(error_of(arg::<bool>(&mut table, json!({ "a": 1 }))), error_of(wrong.flag(0)));
        // &[u8]
        assert_eq!(arg::<Vec<u8>>(&mut table, json!("é")), Ok("é".as_bytes().to_vec()));
        assert_eq!(error_of(arg::<Vec<u8>>(&mut table, json!({ "a": 1 }))), error_of(wrong.text(0)));
        // data
        assert_eq!(arg::<Value>(&mut table, json!({ "a": [1] })), Ok(json!({ "a": [1] })));
        assert_eq!(json!({ "a": [1] }).into_reply(&mut table), Ok(json!({ "a": [1] })));
        assert_eq!(error_of(json!([0.5]).into_reply(&mut table)), error_of(check_data(&json!([0.5]))));
    }

    #[test]
    fn the_enums_cross_as_their_tokens() {
        let mut table = CallTable::new(1);
        macro_rules! crosses {
            ($($type:ty: $value:expr => $token:literal),+ $(,)?) => {
                $(
                    assert_eq!($value.into_reply(&mut table), Ok(json!($token)));
                    assert_eq!(arg::<$type>(&mut table, json!($token)), Ok($value));
                    assert_eq!(
                        error_of(arg::<$type>(&mut table, json!("nothing"))),
                        error_of(<$type as JsonForm>::from_value(&json!("nothing")))
                    );
                )+
            };
        }
        crosses!(
            Effect: Effect::Io => "io",
            Scope: Scope::PerTree => "per-tree",
            CallableKind: CallableKind::Specials => "specials",
            FallbackKind: FallbackKind::Environment => "environment",
            DefinitionKind: DefinitionKind::Macro => "macro",
            DefinitionModes: DefinitionModes::Math => "math",
            ArgLayout: ArgLayout::InlineOrBlocks => "inline-or-blocks",
            BodyLayout: BodyLayout::FromRole => "from-role",
            ExpandPolicy: ExpandPolicy::PerPlan => "per-plan",
            Placement: Placement::Trails => "trails",
            Severity: Severity::Note => "note",
            NodeKindName: NodeKindName::Callable => "callable",
            Mode: Mode::Math => "math",
        );
        assert_eq!(arg::<Vec<Mode>>(&mut table, json!(["text", "math"])), Ok(vec![Mode::Text, Mode::Math]));
        assert_eq!(error_of(arg::<Vec<Mode>>(&mut table, json!("text"))), "argument 1 must be a list of modes, not `\"text\"`");
        assert_eq!(error_of(arg::<Vec<Mode>>(&mut table, json!(["prose"]))), error_of(Mode::from_value(&json!("prose"))));
    }

    #[test]
    fn the_declarations_are_read_as_their_forms_read_them() {
        let mut table = CallTable::new(1);
        let decl = json!({ "name": "x.s", "effect": "compute", "scope": "document" });
        assert_eq!(arg::<StageDecl>(&mut table, decl.clone()), StageDecl::from_value(&decl));
        assert_eq!(error_of(arg::<StageDecl>(&mut table, json!(1))), error_of(StageDecl::from_value(&json!(1))));
        let needs = json!({ "reads": ["x.t"] });
        assert_eq!(arg::<Needs>(&mut table, needs.clone()), Needs::from_value(&needs));
        assert_eq!(error_of(arg::<Needs>(&mut table, json!(1))), error_of(Needs::from_value(&json!(1))));
        let lowering = json!({ "produces": ["flm.pres.span"] });
        assert_eq!(arg::<LoweringDecl>(&mut table, lowering.clone()), LoweringDecl::from_value(&lowering));
        assert_eq!(error_of(arg::<LoweringDecl>(&mut table, json!(1))), error_of(LoweringDecl::from_value(&json!(1))));
        let mark = json!({ "on_backends": ["html"] });
        assert_eq!(arg::<DefaultMark>(&mut table, mark.clone()), DefaultMark::from_value(&mark));
        assert_eq!(error_of(arg::<DefaultMark>(&mut table, json!(1))), error_of(DefaultMark::from_value(&json!(1))));
        // An element declaration, read as data, answered as a handle, read back.
        let def = json!({ "owner": "x", "names": ["box"], "syntax": { "macro": {} }, "element": { "name": "x.box" } });
        let read: ElementDef = arg(&mut table, def.clone()).expect("a declaration");
        assert_eq!(read.names(), element_def_from_table(&mut table, &def).expect("a declaration").names());
        let handle = read.into_reply(&mut table).expect("lent");
        let back: ElementDef = arg(&mut table, handle).expect("the same declaration");
        assert_eq!(back.names(), Some(&["box".to_string()][..]));
        assert_eq!(error_of(arg::<ElementDef>(&mut table, json!(1))), error_of(element_def_from_table(&mut table, &json!(1))));
        // A fallback.
        let fallback = json!({ "modes": ["text"], "kind": "macro", "element": { "name": "x.unknown" } });
        assert_eq!(arg::<FallbackDef>(&mut table, fallback).expect("a fallback").kind, FallbackKind::Macro);
        assert_eq!(error_of(arg::<FallbackDef>(&mut table, json!(1))), error_of(fallback_def_from_table(&mut table, &json!(1))));
        // The style and script definitions.
        let parts = json!({ "id": "x.css", "content": [{ "inline": "p {}" }] });
        let style: StyleDef = arg(&mut table, parts.clone()).expect("a style definition");
        let (id, content, docs) = definition_parts_from_value("a style definition", &parts).expect("parts");
        assert_eq!((style.id, style.content, style.docs), (id, content, docs));
        assert_eq!(
            error_of(arg::<StyleDef>(&mut table, json!(1))),
            error_of(definition_parts_from_value("a style definition", &json!(1)))
        );
        let script: ScriptDef = arg(&mut table, parts.clone()).expect("a script definition");
        assert_eq!(script.id.as_str(), "x.css");
        assert_eq!(
            error_of(arg::<ScriptDef>(&mut table, json!(1))),
            error_of(definition_parts_from_value("a script definition", &json!(1)))
        );
        // A user definition's declaration: read, and written as `definition_to_value` writes it.
        let definition: Definition = arg(&mut table, json!({ "kind": "macro", "name": "x" })).expect("a definition");
        assert_eq!((&definition).into_reply(&mut table), Ok(definition_to_value(&definition)));
        assert_eq!(error_of(arg::<Definition>(&mut table, json!(1))), error_of(definition_from_value(&json!(1), &mut no_handles)));
        // A diagnostic.
        let diagnostic = json!({ "id": "x.d", "severity": "error", "message": "m" });
        assert_eq!(arg::<Diagnostic>(&mut table, diagnostic).expect("a diagnostic").severity, Severity::Error);
        assert_eq!(error_of(arg::<Diagnostic>(&mut table, json!(1))), error_of(diagnostic_from_value(&mut table, &json!(1))));
    }

    #[test]
    fn what_a_hook_or_a_body_answers_crosses_both_ways() {
        let mut table = CallTable::new(1);
        for (expansion, written) in [(Expansion::Default, json!("default")), (Expansion::Emit(Vec::new()), json!({ "emit": [] }))] {
            assert_eq!(expansion.clone().into_reply(&mut table), Ok(written.clone()));
            assert_eq!(arg::<Expansion>(&mut table, written), Ok(expansion));
        }
        assert_eq!(error_of(arg::<Expansion>(&mut table, json!(1))), error_of(expansion_from_value(&mut table, &json!(1))));
        for (lowering, written) in [(Lowering::Default, json!("default")), (Lowering::Emit(Vec::new()), json!({ "emit": [] }))] {
            assert_eq!(lowering.clone().into_reply(&mut table), Ok(written.clone()));
            assert_eq!(arg::<Lowering>(&mut table, written), Ok(lowering));
        }
        assert_eq!(error_of(arg::<Lowering>(&mut table, json!(1))), error_of(lowering_from_value(&mut table, &json!(1))));
        assert_eq!(arg::<PresTree>(&mut table, json!({ "text": "x" })), Ok(PresTree::Text("x".to_string())));
        assert_eq!(error_of(arg::<PresTree>(&mut table, json!(1))), error_of(pres_tree_from_value(&mut table, &json!(1))));
    }

    #[test]
    fn staged_nodes_cross_as_handles() {
        // A staged node is minted by a tree builder only; the cases here are
        // the readers' refusals and the empty lists.
        let mut table = CallTable::new(1);
        let definition = UserDefinition::new(Definition::macro_("x", Vec::new()));
        let not_staged = handle_to_value(table.lend(Entry::UserDefinition(definition)));
        assert_eq!(error_of(arg::<Staged>(&mut table, not_staged.clone())), error_of(staged_from_table(&mut table, &not_staged)));
        assert_eq!(arg::<Vec<Staged>>(&mut table, json!([])), Ok(Vec::new()));
        assert_eq!(Vec::<Staged>::new().into_reply(&mut table), Ok(json!([])));
        assert_eq!(
            error_of(arg::<Vec<Staged>>(&mut table, json!([not_staged]))),
            error_of(staged_list_from_table(&mut table, &json!([not_staged])))
        );
        assert_eq!(arg::<Vec<(String, Vec<Staged>)>>(&mut table, json!([["body", []]])), Ok(vec![("body".to_string(), Vec::new())]));
        assert_eq!(error_of(arg::<Vec<(String, Vec<Staged>)>>(&mut table, json!(1))), "argument 1 must be a list of slots, not `1`");
        assert_eq!(
            error_of(arg::<Vec<(String, Vec<Staged>)>>(&mut table, json!([["body"]]))),
            "argument 1 must be a list of slots, each `[name, [staged nodes]]`, not `[[\"body\"]]`"
        );
    }

    #[test]
    fn nodes_and_what_holds_them_cross_as_handles() {
        let mut table: CallTable<'static> = CallTable::new(5);
        let node = root();
        let handle = node.clone_box(Token::new()).into_reply(&mut table).expect("lent");
        let back: Box<dyn Node<'static>> = arg(&mut table, handle.clone()).expect("the node");
        assert_eq!(back.span(), node.span());
        let stale = json!({ "@handle": { "kind": "node", "call": 4, "index": 0 } });
        let stale_handle = handle_from_value(&stale).expect("a handle");
        assert_eq!(error_of(arg::<Box<dyn Node<'static>>>(&mut table, stale.clone())), error_of(node_from_table(&mut table, stale_handle)));
        let nodes: Vec<Box<dyn Node<'static>>> = arg(&mut table, json!([handle, handle])).expect("two nodes");
        assert_eq!(nodes.len(), 2);
        assert_eq!(error_of(arg::<Vec<Box<dyn Node<'static>>>>(&mut table, json!(1))), error_of(nodes_from_value(&mut table, &json!(1))));
        // A node's address, a walk's entry and a table's projection.
        let addr = NodeAddr::new(flm_core::model::addr::TreeId::tree(0), node.core_node_ref(Token::new()).id());
        assert_eq!(addr.into_reply(&mut table), Ok(node_addr_to_value(&addr)));
        let walked = WalkEntry { addr, node: node.clone_box(Token::new()) }.into_reply(&mut table).expect("written");
        assert_eq!(walked["addr"], node_addr_to_value(&addr));
        assert_eq!(walked["node"]["@handle"]["kind"], json!("node"));
        assert_eq!(TableValue::Projection(json!([1])).into_reply(&mut table), Ok(json!({ "projection": [1] })));
        // A span and the characters of a content.
        assert_eq!(node.span().into_reply(&mut table), Ok(node.span().to_value()));
        let chars = CharsContent { text: Some("ab".to_string()), has_placeholder: false };
        assert_eq!(chars.clone().into_reply(&mut table), Ok(chars.to_value()));
        // An equation line and a line's frame.
        let line = || EquationLine { content: Vec::new(), labels: Vec::new(), tag: None, nonumber: true, linebreak: None };
        assert_eq!(line().into_reply(&mut table), Ok(equation_line_to_value(&mut table, line())));
        let frame = LineFrame { environment: node.clone_box(Token::new()), index: 0, line: line(), anchor: None, shows: LineShows::Nothing };
        let written = frame.into_reply(&mut table).expect("written");
        let read: LineFrame<'static> = arg(&mut table, written.clone()).expect("read back");
        assert_eq!((read.index, read.anchor.clone(), read.environment.span()), (0, None, node.span()));
        let again = read.into_reply(&mut table).expect("written again");
        assert_eq!((&again["index"], &again["shows"], &again["anchor"]), (&written["index"], &written["shows"], &written["anchor"]));
        assert_eq!(error_of(arg::<LineFrame<'static>>(&mut table, json!(1))), error_of(line_frame_from_value(&mut table, &json!(1))));
        // The key-value lists.
        let list = crate::api::techy::extract::parse_keyval_drop_annotations(&[]).expect("the empty list");
        let lent_list = list.into_reply(&mut table).expect("lent");
        assert_eq!(lent_list["@handle"]["kind"], json!("key_vals"));
        let entries: Vec<Box<dyn KeyValEntry<'static>>> = Vec::new();
        assert_eq!(entries.into_reply(&mut table), Ok(json!([])));
    }

    #[test]
    fn the_opaque_values_and_the_lent_objects_cross_as_handles() {
        let mut table: CallTable<'static> = CallTable::new(6);
        let definition = UserDefinition::new(Definition::macro_("x", Vec::new()));
        let handle = definition.clone().into_reply(&mut table).expect("lent");
        assert_eq!(arg::<UserDefinition>(&mut table, handle.clone()).expect("read back").definition().name, "x");
        let rule = LoweringRule::new("x.r", "x.e", LoweringDecl::new(), Arc::new(Body), DefaultMark::Everywhere);
        let rule_handle = rule.into_reply(&mut table).expect("lent");
        assert_eq!(arg::<LoweringRule>(&mut table, rule_handle.clone()).expect("read back").name(), "x.r");
        // Each refuses a handle of another kind, as the dispatcher's arms do.
        assert_eq!(
            error_of(arg::<LoweringRule>(&mut table, handle.clone())),
            error_of(Err::<(), _>(mismatch(HandleKind::LoweringRule, HandleKind::UserDefinition)))
        );
        assert_eq!(
            error_of(arg::<UserDefinition>(&mut table, rule_handle.clone())),
            error_of(Err::<(), _>(mismatch(HandleKind::UserDefinition, HandleKind::LoweringRule)))
        );
        // The objects the binding lends in.
        let stage = handle_to_value(table.lend(Entry::Stage(Arc::new(Quiet))));
        assert_eq!(arg::<Arc<dyn Stage>>(&mut table, stage).expect("the stage").decl().name, "x.quiet");
        assert_eq!(
            error_of(arg::<Arc<dyn Stage>>(&mut table, handle.clone())),
            error_of(Err::<(), _>(mismatch(HandleKind::Stage, HandleKind::UserDefinition)))
        );
        let hook = handle_to_value(table.lend(Entry::Expand(Arc::new(Hook))));
        assert_eq!(arg::<Arc<dyn Expand>>(&mut table, hook).expect("the hook").policy(), ExpandPolicy::PerPlan);
        assert_eq!(error_of(arg::<Arc<dyn Expand>>(&mut table, handle.clone())), error_of(expand_from_table(&mut table, &handle)));
        let body = handle_to_value(table.lend(Entry::Lower(Arc::new(Body))));
        assert!(arg::<Arc<dyn Lower>>(&mut table, body).is_ok());
        assert_eq!(error_of(arg::<Arc<dyn Lower>>(&mut table, handle.clone())), error_of(lower_from_table(&mut table, &handle)));
        // The boxed façades a method answers (`ModeVariant`,
        // `CompiledTemplate`, `PlaceholderBindings`) need a definition the
        // registry compiled, which a unit test cannot make; the suites reach
        // them through the dispatcher.
    }

    #[test]
    fn the_wrapping_shapes_write_what_they_wrap() {
        let mut table = CallTable::new(1);
        assert_eq!(None::<String>.into_reply(&mut table), Ok(Value::Null));
        assert_eq!(Some("a".to_string()).into_reply(&mut table), Ok(json!("a")));
        assert_eq!(vec![true, false].into_reply(&mut table), Ok(json!([true, false])));
        assert_eq!(Some(("(".to_string(), ")".to_string())).into_reply(&mut table), Ok(json!(["(", ")"])));
        assert_eq!(().into_reply(&mut table), Ok(Value::Null));
        assert_eq!(Ok::<usize, ExtError>(2).into_reply(&mut table), Ok(json!(2)));
        let failed = ExtError::Argument { method: String::new(), detail: "d".to_string() };
        assert_eq!(Err::<usize, ExtError>(failed.clone()).into_reply(&mut table), Err(failed));
        let not_chars = crate::api::lang::text::read_chars(&[root()]).expect_err("a paragraph is not characters");
        let expected = not_characters(not_chars);
        assert_eq!(Err::<CharsContent, NotCharacters>(not_chars).into_reply(&mut table), Err(expected));
    }
}

// The `numbering` library and the LaTeX family (WP-2b).

string_form!(LatexArgShape, "a LaTeX argument shape", {
    LatexArgShape::Star => "star",
    LatexArgShape::Optional => "optional",
    LatexArgShape::Mandatory => "mandatory",
    LatexArgShape::EscVerb => "escverb",
});

/// `"macro"`, `"environment"` or `"declaration"`: `flm-core`'s own names of
/// the kinds ([`LatexDefinitionKind::as_str`], [`LatexDefinitionKind::parse`]).
impl JsonForm for LatexDefinitionKind {
    fn to_value(&self) -> Value {
        Value::String(self.as_str().to_string())
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        value
            .as_str()
            .and_then(LatexDefinitionKind::parse)
            .ok_or_else(|| not_one_of("a LaTeX definition kind", &["macro", "environment", "declaration"], value))
    }
}

/// The LaTeX family's payloads: written and read as `flm-core`'s own
/// projection of the payload ([`PresPayload::project`] and the type's
/// `from_projection`), in its JSON form, which is how a rule gives one as
/// data ([`payload_from_value`](crate::adapter::payloads::payload_from_value)).
macro_rules! projection_form {
    ($($type:ty => $what:literal),+ $(,)?) => {
        $(
            impl JsonForm for $type {
                fn to_value(&self) -> Value {
                    to_json(&self.project()).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Value::Null)
                }

                fn from_value(value: &Value) -> Result<Self, ExtError> {
                    let wrong = |detail: String| ExtError::Value { detail: format!("{}: {detail}", $what) };
                    let projection = from_json(&value.to_string()).map_err(|error| wrong(error.to_string()))?;
                    <$type>::from_projection(&projection).map_err(|error| wrong(error.to_string()))
                }
            }
        )+
    };
}

projection_form!(
    LatexMacroPayload => "the payload of a LaTeX macro",
    LatexEnvironmentPayload => "the payload of a LaTeX environment",
    LatexRawPayload => "the payload of raw LaTeX",
    LatexDefinitionPayload => "the payload of a LaTeX definition",
);

/// The value of the data-valued slot `slot` read from `value` by the slot's
/// own reader ([`slots::read`]), the one reader of its shape.
fn slot_value(slot: &str, value: &Value) -> Result<SlotValue, ExtError> {
    match slots::read(slot, value) {
        Some(read) => read.map(|(_, _, value)| value),
        None => Err(ExtError::Value { detail: format!("`{slot}` must be a data-valued slot") }),
    }
}

/// The error of a slot's reader that answered a value of another slot; its
/// table makes this unreachable.
fn other_slot(slot: &str) -> ExtError {
    ExtError::Value { detail: format!("the reader of `{slot}` must answer a value of that slot") }
}

/// `{"name", "default_style", "docs", "within", "nesting"}`, `within` and
/// `nesting` in their serde forms; read by the reader of
/// `flm.numbering.counters`.
impl JsonForm for CounterDecl {
    fn to_value(&self) -> Value {
        json!({
            "name": self.name.as_str(),
            "default_style": self.default_style.as_str(),
            "docs": self.docs,
            "within": self.within.to_value(),
            "nesting": self.nesting.to_value(),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match slot_value(slots::COUNTERS, value)? {
            SlotValue::Counter(decl) => Ok(decl),
            _ => Err(other_slot(slots::COUNTERS)),
        }
    }
}

/// The serde form of a counter style declaration; read by the reader of
/// `flm.numbering.styles`, each field through its serde form.
impl JsonForm for CounterStyleDecl {
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match slot_value(slots::STYLES, value)? {
            SlotValue::Style(decl) => Ok(*decl),
            _ => Err(other_slot(slots::STYLES)),
        }
    }
}

/// `{"name", "counters": [name, …]}`; read by the reader of
/// `flm.numbering.parent-sets`.
impl JsonForm for ParentSetDecl {
    fn to_value(&self) -> Value {
        let counters: Vec<&str> = self.counters.iter().map(Name::as_str).collect();
        json!({ "name": self.name.as_str(), "counters": counters })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match slot_value(slots::PARENT_SETS, value)? {
            SlotValue::ParentSet(decl) => Ok(decl),
            _ => Err(other_slot(slots::PARENT_SETS)),
        }
    }
}

/// `{"name", "content", "docs", "requires": [id, …]}`, `name` the piece's
/// identifier without the prefix `flm.latex.def.` that [`LatexPreambleDef::new`]
/// adds; read by the reader of `flm.latex.preamble`, through `new`. The shape
/// has no place for the identifiers the piece delivers, nor for an identifier
/// without the prefix: the write drops nothing, so a piece with `delivers` is
/// written with the field `delivers`, and one whose `id` lacks the prefix
/// with the field `id` in place of `name`, both fields the reader refuses.
impl JsonForm for LatexPreambleDef {
    fn to_value(&self) -> Value {
        let requires: Vec<&str> = self.requires.iter().map(Name::as_str).collect();
        let mut value = json!({ "content": self.content, "docs": self.docs, "requires": requires });
        match self.id.as_str().strip_prefix(PREAMBLE_ID_PREFIX) {
            Some(name) => value["name"] = json!(name),
            None => value["id"] = json!(self.id.as_str()),
        }
        if !self.delivers.is_empty() {
            let delivers: Vec<&str> = self.delivers.iter().map(Name::as_str).collect();
            value["delivers"] = json!(delivers);
        }
        value
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match slot_value(slots::PREAMBLE, value)? {
            SlotValue::Preamble(def) => Ok(def),
            _ => Err(other_slot(slots::PREAMBLE)),
        }
    }
}

/// `{"id", "package", "options", "docs"}`; read by the reader of
/// `flm.latex.packages`, through the type's serde form.
impl JsonForm for LatexPackageDef {
    fn to_value(&self) -> Value {
        json!({ "id": self.id.as_str(), "package": self.package, "options": self.options, "docs": self.docs })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match slot_value(slots::PACKAGES, value)? {
            SlotValue::Package(def) => Ok(def),
            _ => Err(other_slot(slots::PACKAGES)),
        }
    }
}

/// [`LatexArgShape`]'s string.
impl<'c> FromArg<'c> for LatexArgShape {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        LatexArgShape::from_value(args.get(i))
    }
}

/// [`LatexArgShape`]'s string.
impl<'c> IntoReply<'c> for LatexArgShape {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(self.to_value())
    }
}

/// The options of a number's formatting, `{"variant"?, "with_word"?,
/// "with_delimiters"?}`, read through [`FormatOptions::new`] and its builder
/// methods: no variant, the word and the delimiters written, unless the
/// fields say otherwise.
///
/// # Errors
///
/// A field is not of its type ([`ExtError::Value`]).
pub fn format_options_from_value(value: &Value) -> Result<FormatOptions, ExtError> {
    const WHAT: &str = "format options";
    let map = fields(WHAT, value, &["variant", "with_word", "with_delimiters"])?;
    let flag = |key: &str| match field(&map, key) {
        Value::Null => Ok(true),
        Value::Bool(flag) => Ok(*flag),
        other => Err(ExtError::Value { detail: format!("the field `{key}` of {WHAT} must be a boolean, not `{other}`") }),
    };
    let mut options = FormatOptions::new();
    match field(&map, "variant") {
        Value::Null => {}
        Value::String(variant) => options = options.variant(variant.as_str()),
        other => {
            return Err(ExtError::Value { detail: format!("the field `variant` of {WHAT} must be a text, not `{other}`") });
        }
    }
    if !flag("with_word")? {
        options = options.without_word();
    }
    if !flag("with_delimiters")? {
        options = options.without_delimiters();
    }
    Ok(options)
}

/// `{"variant": text | null, "with_word", "with_delimiters"}`; read by
/// [`format_options_from_value`].
impl JsonForm for FormatOptions {
    fn to_value(&self) -> Value {
        json!({ "variant": self.variant, "with_word": self.with_word, "with_delimiters": self.with_delimiters })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        format_options_from_value(value)
    }
}

/// `{"text", "mode", "vocabulary", "definition", "label", "origin_label",
/// "user_definition", "offset"}`; read by [`template_spec_from_value`].
impl JsonForm for TemplateSpec {
    fn to_value(&self) -> Value {
        json!({
            "text": self.text,
            "mode": self.mode.to_value(),
            "vocabulary": self.vocabulary.to_value(),
            "definition": self.definition,
            "label": self.label,
            "origin_label": self.origin_label,
            "user_definition": self.user_definition,
            "offset": self.offset,
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        template_spec_from_value(value)
    }
}

/// `{"arity", "names": [text | null, …], "body", "extras": [{"name",
/// "substitution"}, …]}`; read as [`template_spec_from_value`] reads a
/// specification's vocabulary.
impl JsonForm for PlaceholderVocabulary {
    fn to_value(&self) -> Value {
        let extras: Vec<Value> =
            self.extras.iter().map(|extra| json!({ "name": extra.name, "substitution": extra.substitution })).collect();
        json!({ "arity": self.arity, "names": self.names, "body": self.body, "extras": extras })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        placeholder_vocabulary_from_value(value)
    }
}

/// A template specification in its data form ([`TemplateSpec`]): `{"text",
/// "mode", "vocabulary", "definition", "label", "origin_label",
/// "user_definition"?, "offset"?}`, the vocabulary `{"arity", "names"?,
/// "body"?, "extras"?: [{"name", "substitution"}]}` read through
/// [`PlaceholderVocabulary::new`] and its builder methods.
///
/// # Errors
///
/// A field missing or of another shape ([`ExtError::Value`]).
pub fn template_spec_from_value(value: &Value) -> Result<TemplateSpec, ExtError> {
    const WHAT: &str = "a template specification";
    let map = fields(
        WHAT,
        value,
        &["text", "mode", "vocabulary", "definition", "label", "origin_label", "user_definition", "offset"],
    )?;
    let mode = match field(&map, "mode") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `mode`") }),
        other => Mode::from_value(other)?,
    };
    let user_definition = match field(&map, "user_definition") {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        other => {
            return Err(ExtError::Value { detail: format!("the field `user_definition` of {WHAT} must be a boolean, not `{other}`") });
        }
    };
    let offset = match field(&map, "offset") {
        Value::Null => 0,
        other => other.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| ExtError::Value {
            detail: format!("the field `offset` of {WHAT} must be a number from 0 up, not `{other}`"),
        })?,
    };
    let vocabulary = match field(&map, "vocabulary") {
        Value::Null => return Err(ExtError::Value { detail: format!("{WHAT} must have the field `vocabulary`") }),
        other => placeholder_vocabulary_from_value(other)?,
    };
    Ok(TemplateSpec {
        text: required_text(WHAT, &map, "text")?,
        mode,
        vocabulary,
        definition: required_text(WHAT, &map, "definition")?,
        label: required_text(WHAT, &map, "label")?,
        origin_label: required_text(WHAT, &map, "origin_label")?,
        user_definition,
        offset,
    })
}

/// A placeholder vocabulary in its data form, as [`template_spec_from_value`]
/// reads it.
fn placeholder_vocabulary_from_value(value: &Value) -> Result<PlaceholderVocabulary, ExtError> {
    const WHAT: &str = "a placeholder vocabulary";
    let map = fields(WHAT, value, &["arity", "names", "body", "extras"])?;
    let arity = field(&map, "arity").as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| ExtError::Value {
        detail: format!("the field `arity` of {WHAT} must be a number from 0 up, not `{}`", field(&map, "arity")),
    })?;
    let mut vocabulary = PlaceholderVocabulary::new(arity);
    match field(&map, "names") {
        Value::Null => {}
        Value::Array(items) => {
            let names = items
                .iter()
                .map(|item| match item {
                    Value::Null => Ok(None),
                    Value::String(name) => Ok(Some(name.clone())),
                    other => Err(ExtError::Value {
                        detail: format!("the field `names` of {WHAT} must be a list of strings or `null`, not one holding `{other}`"),
                    }),
                })
                .collect::<Result<Vec<_>, _>>()?;
            vocabulary = vocabulary.names(names);
        }
        other => {
            return Err(ExtError::Value { detail: format!("the field `names` of {WHAT} must be a list, not `{other}`") });
        }
    }
    match field(&map, "body") {
        Value::Null | Value::Bool(false) => {}
        Value::Bool(true) => vocabulary = vocabulary.with_body(),
        other => {
            return Err(ExtError::Value { detail: format!("the field `body` of {WHAT} must be a boolean, not `{other}`") });
        }
    }
    match field(&map, "extras") {
        Value::Null => {}
        Value::Array(items) => {
            for item in items {
                const EXTRA: &str = "an extra placeholder";
                let extra = fields(EXTRA, item, &["name", "substitution"])?;
                let substitution = match field(&extra, "substitution") {
                    Value::Null => false,
                    Value::Bool(flag) => *flag,
                    other => {
                        return Err(ExtError::Value {
                            detail: format!("the field `substitution` of {EXTRA} must be a boolean, not `{other}`"),
                        });
                    }
                };
                vocabulary = vocabulary.extra(required_text(EXTRA, &extra, "name")?, substitution);
            }
        }
        other => {
            return Err(ExtError::Value { detail: format!("the field `extras` of {WHAT} must be a list, not `{other}`") });
        }
    }
    Ok(vocabulary)
}

/// [`template_spec_from_value`].
impl<'c> FromArg<'c> for TemplateSpec {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        template_spec_from_value(args.get(i))
    }
}

/// A template language: the entry cloned (a durable token passed as an
/// argument is lent as one by the dispatcher).
impl<'c> FromArg<'c> for TemplateLanguage {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        cloned_entry!(table, args.handle(i)?, TemplateLanguage)
    }
}

/// Value bindings: the entry cloned.
impl<'c> FromArg<'c> for ValueBindings {
    fn from_arg(table: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        cloned_entry!(table, args.handle(i)?, ValueBindings)
    }
}

/// A template language answered as its durable token.
impl<'c> IntoReply<'c> for TemplateLanguage {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(durable_to_value(self.token()))
    }
}

/// A durable template answered as its durable token.
impl<'c> IntoReply<'c> for DurableTemplateFacade {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(durable_to_value(self.token()))
    }
}

/// Value bindings, lent into the table.
impl<'c> IntoReply<'c> for ValueBindings {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(handle_to_value(table.lend(Entry::ValueBindings(self))))
    }
}

/// [`format_options_from_value`].
impl<'c> FromArg<'c> for FormatOptions {
    fn from_arg(_: &mut CallTable<'c>, args: &Args<'_>, i: usize) -> Result<Self, ExtError> {
        format_options_from_value(args.get(i))
    }
}

/// A counter record `{"counter", "value": [n, …], "style", "instance":
/// address | null}`, the address as [`node_addr_to_value`] writes it.
pub fn counter_record_to_value(record: &CounterRecord) -> Value {
    json!({
        "counter": record.counter,
        "value": record.value,
        "style": record.style,
        "instance": record.instance.as_ref().map(node_addr_to_value),
    })
}

/// [`counter_record_to_value`].
impl<'c> IntoReply<'c> for CounterRecord {
    fn into_reply(self, _: &mut CallTable<'c>) -> Result<Value, ExtError> {
        Ok(counter_record_to_value(&self))
    }
}

/// A number segment `{"content": node, "link": point | null}`, the content
/// lent as a node, the point as [`point_id_to_value`] writes it.
impl<'c> IntoReply<'c> for NumberSegment<'c> {
    fn into_reply(self, table: &mut CallTable<'c>) -> Result<Value, ExtError> {
        let content = handle_to_value(table.lend(Entry::Node(self.content)));
        Ok(json!({ "content": content, "link": self.link.as_ref().map(point_id_to_value) }))
    }
}
