//! Storing an FLM parse in techy's serialized form and reading it back: FLM's
//! part of techy's serialization.
//!
//! techy can write a whole parse (the node tree, the parsing states and sources
//! behind it, the specifications and providers it used, the diagnostics) into a
//! format-independent serialized value, and rebuild it elsewhere with shared
//! values and identities preserved. A language takes part by implementing
//! techy's `SerializableLang`, which requires a way to write and read every type
//! the language adds to a parse. This module provides that for FLM.
//!
//! An embedding program does not use this module directly: the functions of
//! [`crate::dump::session`] ([`writer`](crate::dump::session::writer),
//! [`reader`](crate::dump::session::reader)) prepare a session with everything
//! described here, and that module re-exports the public types of this one.
//!
//! Most of the conversions are derived on the types themselves, because their
//! serialized form is their Rust form: the group classes and callable forms,
//! the source origin, the text source map, the restricted-interior policy, the
//! parsing settings. Two are written by hand here, because their serialized
//! form differs from their Rust form:
//!
//! - the **node extension** ([`FlmNodeExt`]): the element is written by its
//!   stable name and looked up again when read;
//! - the **state extension** ([`FlmStateExt`]): the structural elements are
//!   written by their names and read back as the *reading* language's own
//!   shared set.
//!
//! # Specifications and providers
//!
//! A specification a node records is stored in one of two ways.
//!
//! - **By identity**, for a specification a package holds: a package records
//!   on every specification it hands out a *provenance stamp*
//!   ([`SpecProvenance`](techy::core::specs::SpecProvenance)), which identifies the
//!   package that holds the specification and the name the package holds it
//!   under. An [`FlmCallable`] and an environment's specification are written
//!   as that stamp, and read back as the same value that the reading language's
//!   package of that name holds. The fallback provider
//!   ([`FlmFallbackProvider`]) is written by its name and read back as the
//!   reading language's own fallback provider in the same way.
//! - **Self-contained**, for the two specifications that belong to no package
//!   and are never parsed: [`FlmSynthesizedSpec`] and [`FlmPresentationSpec`]
//!   are written as their element's name, and rebuilt around the element of
//!   that name when read.
//!
//! [`register`] installs the readers of these forms on a reading session.
//!
//! # The environment a session needs
//!
//! Reading needs a value for every name in the serialized data: the reading
//! language's packages, given as techy's `KnownProviders`, and FLM's
//! [`ReadEnvironment`], which holds the structural elements, an
//! [`ElementResolver`] for every other element name, and the
//! [`PayloadReaders`] that rebuild the typed payloads of nodes. Both are set as
//! the session's user data.
//!
//! Writing needs one value, and only to write elements precisely: a
//! [`WriteEnvironment`], which records, for every element that a registered
//! specification carries, which specification that is.
//!
//! These types are declared here because the conversions read them from the
//! session, but their public path is [`crate::dump::session`]
//! ([`crate::dump::session::ReadEnvironment`], and so on).
//! That module also provides the functions that prepare a session with them,
//! which an embedding program should use.
//!
//! # Why a node extension records its element twice
//!
//! Two element *values* may share one stable name (`\section` and
//! `\subsection` are two values of one heading element), so a name alone
//! cannot say which value a node carried. A node extension is therefore written
//! with the name *and*, when the write environment records which registered
//! specification carries that exact value, a reference to that specification.
//! When reading, the element is taken from the specification, by identity; the
//! element resolver is used only for elements that no specification carries
//! (the structural elements, and the elements of the two self-contained
//! specifications). Without a write environment, only the name is written and
//! the resolver decides.
//!
//! For the same reason, a name shared by several values is refused, not
//! guessed, where only a name is available: the resolver returns
//! [`AmbiguousElement`], and the read fails with it as the cause. The element
//! of a parsed callable is always read through its specification and never
//! depends on the resolver.
//!
//! # Serialized names
//!
//! FLM's own serialized identifiers are [`SYNTHESIZED_IDENTIFIER`],
//! [`PRESENTATION_IDENTIFIER`] and [`FALLBACK_GATE_IDENTIFIER`]. Its keys are
//! `element`, `spec`, `source_map` and `payload` for a node extension, and
//! `structural`, `restricted_interior` and `parsing` for a state extension.
//! They are as stable as FLM's element names.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use techy::core::ClosedVocabulary;
use techy::core::specs::{CallableSpec, SpecsProvider};
use techy::latexlike::Mode;
use techy::serialize::{
    DeserializableObject, DeserializableValue, DeserializeContext, DeserializeError,
    KnownProviders, RegistrationError, SerdeSession, SerialEntry, SerialIndex, SerialValue,
    SerializableLang, SerializableObject, SerializableValue, SerializeContext, SerializeError,
    SpecIndex, StandardTableInterning, StandardTableReading, StandardTables,
};

use crate::model::element::ElementSpec;
use crate::model::names::ElementName;
use crate::lang::ext::{FlmNodeExt, FlmStateExt, PresPayload, TextSourceMap, RestrictedInterior};
use crate::lang::flm::{Flm, FlmCallableType};
use crate::lang::mint::specs::element_of_spec;
use crate::lang::mint::{
    FlmCallable, FlmFallbackProvider, FlmPresentationSpec, FlmSynthesizedSpec, FlmUnknownMacroSpec,
};
use crate::lang::rules::FlmParsing;
use crate::lang::structural::StructuralElements;

// --- the language opts in ------------------------------------------------------------

/// FLM parses can be serialized and read back: every type the language adds to
/// a parse has its conversions, derived on the plain types and written by hand
/// in this module for the node and state extensions.
impl SerializableLang for Flm {}

// --- stored identifiers -------------------------------------------------------------------

/// The serialized identifier of an [`FlmSynthesizedSpec`]: `flm.synthesized`.
/// Its serialized form is `{element}`.
pub const SYNTHESIZED_IDENTIFIER: &str = "flm.synthesized";

/// The serialized identifier of an [`FlmPresentationSpec`]: `flm.presentation`.
/// Its serialized form is `{element}`.
pub const PRESENTATION_IDENTIFIER: &str = "flm.presentation";

/// The serialized identifier of an [`FlmFallbackProvider`]:
/// `flm.fallback-gate`. Its serialized form is `{name}`.
pub const FALLBACK_GATE_IDENTIFIER: &str = "flm.fallback-gate";

// --- the reading environment: elements and payloads ----------------------------------------

pub use crate::model::element::AmbiguousElement;

/// An element resolver: the lookup with which a reading session turns a
/// serialized element name back into an element value.
///
/// On the wire, an element is written by its stable type name, so this is the
/// lookup from that name back to the element value the registry holds under
/// it. The registry implements it; a test can use
/// [`crate::dump::session::ElementTable`], a set of structural elements and a
/// map.
///
/// It need not return the structural elements: the reading environment looks
/// names up in the reading language's own set first
/// ([`ReadEnvironment::resolve_element`]).
///
/// Its public path is [`crate::dump::session::ElementResolver`].
pub trait ElementResolver: Send + Sync {
    /// Returns the element value registered under `name`, or `Ok(None)` (not
    /// an error) when no value is registered under it.
    ///
    /// # Errors
    ///
    /// [`AmbiguousElement`], which identifies the owner of each value, when
    /// several values share the name: a name is never resolved by guessing.
    fn resolve_element(&self, name: &str) -> Result<Option<Arc<ElementSpec>>, AmbiguousElement>;
}

impl<T: ElementResolver + ?Sized> ElementResolver for Arc<T> {
    fn resolve_element(&self, name: &str) -> Result<Option<Arc<ElementSpec>>, AmbiguousElement> {
        (**self).resolve_element(name)
    }
}

/// A payload reader: a function that rebuilds the typed payload of a node of one
/// element from its serialized form.
///
/// A reader receives **exactly the value [`PresPayload::project`] returned** for
/// a payload of that element, and the reading context. A projected payload is
/// self-contained data, with nothing that only makes sense next to the tree it
/// came from, so a reader is usually the derived conversion of the payload
/// type.
pub type PayloadReader = Arc<
    dyn Fn(
            &SerialValue,
            &mut DeserializeContext<'_, Flm>,
        ) -> Result<Arc<dyn PresPayload>, DeserializeError>
        + Send
        + Sync,
>;

/// The payload readers of a reading session, one per element, keyed by the
/// element's stable name.
///
/// A backend registers one reader for each of its presentation elements whose
/// nodes carry a payload, and a feature registers one for an element whose
/// rebuilt nodes carry a payload (see [`PresPayload`]). Reading a payload of an
/// element with no reader registered fails with an error that identifies the
/// element.
///
/// Its public path is [`crate::dump::session::PayloadReaders`].
#[derive(Clone, Default)]
pub struct PayloadReaders {
    readers: BTreeMap<ElementName, PayloadReader>,
}

impl PayloadReaders {
    /// Returns a set with no reader in it.
    pub fn new() -> PayloadReaders {
        PayloadReaders::default()
    }

    /// Registers `reader` for the payloads of the element named `element`, and
    /// returns the reader it replaced, if there was one.
    pub fn register<F>(
        &mut self,
        element: impl Into<ElementName>,
        reader: F,
    ) -> Option<PayloadReader>
    where
        F: Fn(
                &SerialValue,
                &mut DeserializeContext<'_, Flm>,
            ) -> Result<Arc<dyn PresPayload>, DeserializeError>
            + Send
            + Sync
            + 'static,
    {
        self.readers.insert(element.into(), Arc::new(reader))
    }

    /// Returns the reader registered for the element named `element`, if there
    /// is one.
    pub fn get(&self, element: &str) -> Option<&PayloadReader> {
        self.readers.get(element)
    }

    /// Returns the names of the elements that have a reader, in name order.
    pub fn elements(&self) -> impl Iterator<Item = &ElementName> + '_ {
        self.readers.keys()
    }

    /// Returns how many readers are registered.
    pub fn len(&self) -> usize {
        self.readers.len()
    }

    /// Returns whether no reader is registered at all.
    pub fn is_empty(&self) -> bool {
        self.readers.is_empty()
    }
}

impl fmt::Debug for PayloadReaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.readers.keys()).finish()
    }
}

/// The read environment: FLM's part of what a reading session needs. The other
/// part is techy's `KnownProviders`, the reading language's packages.
///
/// It holds three things: the reading language's structural elements, which
/// every state extension read in the session shares; the [`ElementResolver`]
/// for every other element name in the serialized data; and the
/// [`PayloadReaders`].
///
/// It must be set as the session's user data;
/// [`crate::dump::session::reader`] does that, along with everything else a
/// reading session needs. Its public path is
/// [`crate::dump::session::ReadEnvironment`].
#[derive(Clone)]
pub struct ReadEnvironment {
    structural: Arc<StructuralElements>,
    elements: Arc<dyn ElementResolver>,
    payloads: Arc<PayloadReaders>,
}

impl ReadEnvironment {
    /// Returns an environment with the given structural elements and element
    /// resolver, and no payload reader. Add readers with
    /// [`ReadEnvironment::with_payload_readers`].
    pub fn new(
        structural: Arc<StructuralElements>,
        elements: Arc<dyn ElementResolver>,
    ) -> ReadEnvironment {
        ReadEnvironment { structural, elements, payloads: Arc::new(PayloadReaders::new()) }
    }

    /// Builder: sets the payload readers, replacing any already set.
    pub fn with_payload_readers(mut self, readers: PayloadReaders) -> ReadEnvironment {
        self.payloads = Arc::new(readers);
        self
    }

    /// Returns the structural elements that every state extension read in this
    /// session shares.
    pub fn structural(&self) -> &Arc<StructuralElements> {
        &self.structural
    }

    /// Returns the element named `name`: a structural element of this
    /// environment's set when the name is one, and otherwise what the element
    /// resolver returns.
    ///
    /// # Errors
    ///
    /// [`AmbiguousElement`] when several element values share the name.
    pub fn resolve_element(
        &self,
        name: &str,
    ) -> Result<Option<Arc<ElementSpec>>, AmbiguousElement> {
        match self.structural.by_name(name) {
            Some(element) => Ok(Some(Arc::clone(element))),
            None => self.elements.resolve_element(name),
        }
    }

    /// Returns the payload reader of the element named `element`, if one is
    /// registered.
    pub fn payload_reader(&self, element: &str) -> Option<&PayloadReader> {
        self.payloads.get(element)
    }
}

impl fmt::Debug for ReadEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadEnvironment")
            .field("structural", &self.structural)
            .field("payloads", &self.payloads)
            .finish_non_exhaustive()
    }
}

/// Returns the read environment from the session's user data, or the error
/// that says how to prepare the session.
fn read_environment(cx: &DeserializeContext<'_, Flm>) -> Result<ReadEnvironment, DeserializeError> {
    cx.user_data::<ReadEnvironment>().cloned().ok_or_else(|| {
        DeserializeError::failed(
            "reading FLM data requires a read environment (the structural elements, the \
             element resolver and the payload readers): prepare the session with \
             flm_core::dump::session::reader",
        )
    })
}

/// Returns the element from a resolver's result, or the read error: no element
/// of that name is known, or several values share it, in which case
/// [`AmbiguousElement`] is the error's cause.
fn resolved_element(
    name: &str,
    resolution: Result<Option<Arc<ElementSpec>>, AmbiguousElement>,
) -> Result<Arc<ElementSpec>, DeserializeError> {
    match resolution {
        Ok(Some(element)) => Ok(element),
        Ok(None) => Err(DeserializeError::failed(format!(
            "the element resolver of the read environment finds no element `{name}`"
        ))),
        Err(ambiguous) => {
            Err(DeserializeError::failed(ambiguous.to_string()).with_cause(ambiguous))
        }
    }
}

/// Returns the element named `name` in the session's read environment, or the
/// read error.
fn resolve_element(
    cx: &DeserializeContext<'_, Flm>,
    name: &str,
) -> Result<Arc<ElementSpec>, DeserializeError> {
    resolved_element(name, read_environment(cx)?.resolve_element(name))
}

// --- the writing environment: which spec carries an element ---------------------------------

/// The write environment: FLM's part of what a writing session needs. It
/// records, for every element value that a registered specification carries,
/// which specification that is.
///
/// A node extension writes a reference to that specification beside the
/// element's name, so the element reads back as the exact value even when
/// several values share one name (see the [module
/// documentation](self#why-a-node-extension-records-its-element-twice)).
/// Without this environment, a node extension writes the name alone, and the
/// reading side's element resolver decides.
///
/// Build one from a language's providers with
/// [`WriteEnvironment::for_providers`], and set it as the session's user data;
/// [`crate::dump::session::writer`] does both. Its public path is
/// [`crate::dump::session::WriteEnvironment`].
#[derive(Default)]
pub struct WriteEnvironment {
    /// Keyed by the element value's identity (its `Arc` pointer); the
    /// specification keeps the element alive, so a key never dangles.
    element_specs: BTreeMap<usize, Arc<dyn CallableSpec<Flm>>>,
}

impl WriteEnvironment {
    /// Returns an environment that records no element yet.
    pub fn new() -> WriteEnvironment {
        WriteEnvironment::default()
    }

    /// Returns the environment built by asking each of `providers` for the
    /// names it registers. A provider that cannot list its names, such as the
    /// fallback provider, adds nothing.
    ///
    /// The result depends only on the input: the providers are taken from the
    /// last to the first, and within each, callable forms and parsing modes in
    /// their fixed order and names in sorted order. Where specifications of
    /// several providers carry one element, the one of the provider latest in
    /// the list is recorded: the features' packages are listed in registration
    /// order, so between features, the specification of the feature placed
    /// last. Within one provider, the first one found is recorded.
    ///
    /// Pass [`BuiltLang::providers`](crate::lang::BuiltLang::providers).
    pub fn for_providers<'a>(
        providers: impl IntoIterator<Item = &'a Arc<dyn SpecsProvider<Flm>>>,
    ) -> WriteEnvironment {
        // See decision rationale at [§flm-dd-dr:feat-order].
        let providers: Vec<&Arc<dyn SpecsProvider<Flm>>> = providers.into_iter().collect();
        let mut environment = WriteEnvironment::new();
        for provider in providers.into_iter().rev() {
            environment.index_provider(&**provider);
        }
        environment
    }

    /// Records, for every element that a specification of `provider` carries
    /// and that is not recorded yet, that specification.
    pub fn index_provider(&mut self, provider: &dyn SpecsProvider<Flm>) {
        for &callable_type in FlmCallableType::ALL {
            for &mode in Mode::ALL {
                let Some(entries) = provider.iter_symbols(callable_type, mode) else {
                    continue;
                };
                let mut symbols: Vec<(String, Arc<dyn CallableSpec<Flm>>)> =
                    entries.map(|entry| (entry.name.to_string(), Arc::clone(entry.spec))).collect();
                symbols.sort_by(|a, b| a.0.cmp(&b.0));
                for (_, spec) in symbols {
                    self.index_spec(spec);
                }
            }
        }
    }

    /// Records `spec` for the element it carries, unless that element is
    /// recorded already. A specification carrying no element is ignored.
    pub fn index_spec(&mut self, spec: Arc<dyn CallableSpec<Flm>>) {
        if let Some(element) = element_of_spec(&*spec) {
            self.element_specs.entry(Arc::as_ptr(element) as usize).or_insert(spec);
        }
    }

    /// Returns the specification recorded for `element`, if there is one.
    pub fn spec_of(&self, element: &Arc<ElementSpec>) -> Option<&Arc<dyn CallableSpec<Flm>>> {
        self.element_specs.get(&(Arc::as_ptr(element) as usize))
    }

    /// Returns how many elements have a recorded specification.
    pub fn len(&self) -> usize {
        self.element_specs.len()
    }

    /// Returns whether no element has a recorded specification.
    pub fn is_empty(&self) -> bool {
        self.element_specs.is_empty()
    }
}

impl fmt::Debug for WriteEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteEnvironment").field("elements", &self.element_specs.len()).finish()
    }
}

// --- stored table positions -----------------------------------------------------------

/// Returns a position in one of the session's tables, in its serialized form.
fn index_value<I: SerialIndex>(position: I) -> SerialValue {
    SerialValue::Index { table: position.table(), index: position.index() }
}

/// Returns a serialized table reference as a typed position. The session has
/// already translated it into its own numbering before any conversion reads it.
fn index_from<I: SerialIndex>(
    value: &SerialValue,
    what: &'static str,
) -> Result<I, DeserializeError> {
    match value {
        SerialValue::Index { table, index } => Ok(I::from_parts(*table, *index)),
        other => Err(DeserializeError::failed(format!(
            "{what}: expected a table reference, found {other:?}"
        ))),
    }
}

// --- the state extension ---------------------------------------------------------------------

/// The serialized form of a state extension.
#[derive(SerializableValue, DeserializableValue)]
struct WireStateExt {
    #[serial(name = "structural")]
    structural: Vec<String>,
    #[serial(name = "restricted_interior")]
    restricted_interior: RestrictedInterior,
    #[serial(name = "parsing")]
    parsing: FlmParsing,
}

/// A state extension is written as `{structural, restricted_interior,
/// parsing}`, where `structural` is the list of structural element names in
/// their fixed order and `parsing` the parsing settings under their field
/// names. The elements are written by name only, because a state refers to
/// elements and does not own them.
impl SerializableValue<Flm> for FlmStateExt {
    fn serialize_value(
        &self,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialValue, SerializeError> {
        let wire = WireStateExt {
            structural: self
                .structural
                .all()
                .iter()
                .map(|element| element.name.to_string())
                .collect(),
            restricted_interior: self.restricted_interior.clone(),
            parsing: FlmParsing::clone(&self.parsing),
        };
        wire.serialize_value(cx)
    }
}

/// The structural elements are read back as the reading language's own shared
/// set ([`ReadEnvironment::structural`]), so every state read in one session
/// shares it. The serialized names are first checked against that set, and a
/// mismatch is a read error, never a silent substitution.
impl DeserializableValue<Flm> for FlmStateExt {
    fn deserialize_value(
        value: &SerialValue,
        cx: &mut DeserializeContext<'_, Flm>,
    ) -> Result<Self, DeserializeError> {
        let wire = WireStateExt::deserialize_value(value, cx)?;
        let environment = read_environment(cx)?;
        let structural = Arc::clone(environment.structural());
        let names: Vec<&str> =
            structural.all().iter().map(|element| element.name.as_str()).collect();
        if wire.structural != names {
            return Err(DeserializeError::failed(format!(
                "the serialized parsing state lists the structural elements {:?}, but the \
                 structural elements of the reading language are {names:?}",
                wire.structural
            )));
        }
        Ok(FlmStateExt {
            structural,
            restricted_interior: wire.restricted_interior,
            parsing: Arc::new(wire.parsing),
        })
    }
}

// --- the node extension -------------------------------------------------------------------

/// The serialized form of a node extension.
#[derive(SerializableValue, DeserializableValue)]
#[serial(lang = Flm)]
struct WireNodeExt {
    #[serial(name = "element")]
    element: String,
    #[serial(name = "spec")]
    spec: Option<SerialValue>,
    #[serial(name = "source_map")]
    source_map: Option<TextSourceMap>,
    #[serial(name = "payload")]
    payload: Option<SerialValue>,
}

/// A node extension is written as `{element, spec?, source_map?, payload?}`:
/// the element's stable name; a reference to the specification carrying that
/// exact element value, when the [`WriteEnvironment`] records one; the source
/// map, when the node has one; and the payload's projection
/// ([`PresPayload::project`]), when the node has a payload.
impl SerializableValue<Flm> for FlmNodeExt {
    fn serialize_value(
        &self,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialValue, SerializeError> {
        let carrier = cx
            .user_data::<WriteEnvironment>()
            .and_then(|environment| environment.spec_of(&self.element).cloned());
        let spec = match carrier {
            Some(spec) => Some(index_value(cx.intern_spec(&spec)?)),
            None => None,
        };
        let wire = WireNodeExt {
            element: self.element.name.to_string(),
            spec,
            source_map: self.source_map.as_deref().cloned(),
            payload: self.payload.as_ref().map(|payload| payload.project()),
        };
        wire.serialize_value(cx)
    }
}

/// When a reference to a specification is written, the element is the one that
/// specification carries, by identity, and its name must agree with the written
/// name. Otherwise it is the read environment's element of that name. A payload
/// is rebuilt by the reader registered for the element ([`PayloadReaders`]).
impl DeserializableValue<Flm> for FlmNodeExt {
    fn deserialize_value(
        value: &SerialValue,
        cx: &mut DeserializeContext<'_, Flm>,
    ) -> Result<Self, DeserializeError> {
        let wire = WireNodeExt::deserialize_value(value, cx)?;
        let environment = read_environment(cx)?;
        let element = match &wire.spec {
            Some(reference) => {
                let position: SpecIndex =
                    index_from(reference, "the node extension's specification")?;
                let spec = cx.spec(position)?;
                let element = element_of_spec(&*spec).cloned().ok_or_else(|| {
                    DeserializeError::failed(format!(
                        "the node extension of the element `{}` refers to a specification that \
                         carries no element",
                        wire.element
                    ))
                })?;
                if element.name.as_str() != wire.element {
                    return Err(DeserializeError::failed(format!(
                        "the node extension gives the element `{}`, but its specification \
                         carries the element `{}`",
                        wire.element, element.name
                    )));
                }
                element
            }
            None => resolved_element(&wire.element, environment.resolve_element(&wire.element))?,
        };
        let payload = match &wire.payload {
            Some(value) => {
                let reader =
                    environment.payload_reader(&wire.element).cloned().ok_or_else(|| {
                        DeserializeError::failed(format!(
                            "no payload reader is registered for the element `{}`",
                            wire.element
                        ))
                    })?;
                Some(reader(value, cx)?)
            }
            None => None,
        };
        Ok(FlmNodeExt {
            element,
            source_map: wire.source_map.map(Arc::new),
            payload,
        })
    }
}

// --- the specs -----------------------------------------------------------------------------

/// Written by identity, through the provenance stamp of the package that holds
/// this specification (the record of that package and of the name it holds the
/// specification under; see [`crate::lang::serialize`]): the reading side finds
/// the value that the package of that name holds. A wrapper built outside a package has no stamp and cannot
/// be written (`SerializeError::MissingProvenance`).
impl SerializableObject<Flm> for FlmCallable {
    fn serialize_object(
        &self,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialEntry, SerializeError> {
        match self.provenance() {
            Some(provenance) => provenance.serialize_object(cx),
            None => Err(SerializeError::MissingProvenance { spec: "FlmCallable" }),
        }
    }
}

/// Never written in practice: the language's specification for unknown macros
/// is the *inner* specification of an [`FlmCallable`] held by the fallback
/// package, and a node records that wrapper, which is written by identity
/// through the package's stamp. This implementation exists because
/// `CallableSpec` requires one; it returns the same missing-stamp error as a
/// wrapper without a stamp would.
impl SerializableObject<Flm> for FlmUnknownMacroSpec {
    fn serialize_object(
        &self,
        _cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialEntry, SerializeError> {
        Err(SerializeError::MissingProvenance { spec: "FlmUnknownMacroSpec" })
    }
}

/// The serialized form of the two specifications that belong to no package:
/// the element's name.
#[derive(SerializableValue, DeserializableValue)]
struct WireElementSpec {
    #[serial(name = "element")]
    element: String,
}

/// Written as `{element}` under [`SYNTHESIZED_IDENTIFIER`].
impl SerializableObject<Flm> for FlmSynthesizedSpec {
    fn serialize_object(
        &self,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialEntry, SerializeError> {
        let wire = WireElementSpec { element: self.element().name.to_string() };
        Ok(SerialEntry {
            identifier: SYNTHESIZED_IDENTIFIER.into(),
            data: wire.serialize_value(cx)?,
        })
    }
}

/// Rebuilt around the read environment's element of the written name.
impl DeserializableObject<Flm> for FlmSynthesizedSpec {
    type Output = FlmSynthesizedSpec;

    fn deserialize_object(
        value: &SerialValue,
        cx: &mut DeserializeContext<'_, Flm>,
    ) -> Result<FlmSynthesizedSpec, DeserializeError> {
        let wire = WireElementSpec::deserialize_value(value, cx)?;
        Ok(FlmSynthesizedSpec::new(resolve_element(cx, &wire.element)?))
    }
}

/// Written as `{element}` under [`PRESENTATION_IDENTIFIER`].
impl SerializableObject<Flm> for FlmPresentationSpec {
    fn serialize_object(
        &self,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialEntry, SerializeError> {
        let wire = WireElementSpec { element: self.element().name.to_string() };
        Ok(SerialEntry {
            identifier: PRESENTATION_IDENTIFIER.into(),
            data: wire.serialize_value(cx)?,
        })
    }
}

/// Rebuilt around the read environment's element of the written name.
impl DeserializableObject<Flm> for FlmPresentationSpec {
    type Output = FlmPresentationSpec;

    fn deserialize_object(
        value: &SerialValue,
        cx: &mut DeserializeContext<'_, Flm>,
    ) -> Result<FlmPresentationSpec, DeserializeError> {
        let wire = WireElementSpec::deserialize_value(value, cx)?;
        Ok(FlmPresentationSpec::new(resolve_element(cx, &wire.element)?))
    }
}

// --- the fallback gate ---------------------------------------------------------------------

/// The serialized form of the fallback provider: its name.
#[derive(SerializableValue, DeserializableValue)]
struct WireFallbackGate {
    #[serial(name = "name")]
    name: String,
}

/// Written by identity as `{name}` under [`FALLBACK_GATE_IDENTIFIER`]: the gate
/// belongs to the reading language, as a package does. The specifications it
/// returns are held in the fallback package, which is written by identity
/// separately.
impl SerializableObject<Flm> for FlmFallbackProvider {
    fn serialize_object(
        &self,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialEntry, SerializeError> {
        let wire = WireFallbackGate { name: String::from(self.name()) };
        Ok(SerialEntry {
            identifier: FALLBACK_GATE_IDENTIFIER.into(),
            data: wire.serialize_value(cx)?,
        })
    }
}

/// Looked up among the reading session's providers (techy's `KnownProviders`
/// in its user data): the gate registered under the written name.
impl DeserializableObject<Flm> for FlmFallbackProvider {
    type Output = Arc<dyn SpecsProvider<Flm>>;

    fn deserialize_object(
        value: &SerialValue,
        cx: &mut DeserializeContext<'_, Flm>,
    ) -> Result<Self::Output, DeserializeError> {
        let wire = WireFallbackGate::deserialize_value(value, cx)?;
        let missing = || DeserializeError::MissingProvider { name: wire.name.clone() };
        let known = cx.user_data::<KnownProviders<Flm>>().ok_or_else(missing)?;
        known.resolve(&wire.name)?.ok_or_else(missing)
    }
}

// --- registration --------------------------------------------------------------------------

/// Prepares a reading session to read FLM data: registers the readers of
/// techy's own serialized forms and of its LaTeX-like preset, then FLM's (the
/// two self-contained specifications, [`SYNTHESIZED_IDENTIFIER`] and
/// [`PRESENTATION_IDENTIFIER`], and the fallback provider,
/// [`FALLBACK_GATE_IDENTIFIER`]).
///
/// Call it once per reading session; a writing session needs nothing
/// registered. Registering the readers is only part of the preparation: the
/// session also needs techy's `KnownProviders` and a [`ReadEnvironment`] as its
/// user data. [`crate::dump::session::reader`] does all of it at once, and an
/// embedding program should call that function instead.
///
/// # Errors
///
/// `RegistrationError::UnknownTableName`, which identifies the specifications
/// table, when the session does not have techy's standard tables;
/// `RegistrationError::DuplicateIdentifier` when the readers are registered
/// already.
pub fn register(session: &mut SerdeSession<Flm>) -> Result<(), RegistrationError> {
    techy::latexlike::serialize::register(session)?;
    // techy's `standard_tables()` answers `None` without saying which table is
    // missing; the specs table is named, as the first this function needs.
    let StandardTables { specs, providers, .. } = session
        .standard_tables()
        .ok_or_else(|| RegistrationError::UnknownTableName { name: String::from("specs") })?;
    specs.register_type::<FlmSynthesizedSpec>(session, SYNTHESIZED_IDENTIFIER, |spec| {
        Arc::new(spec) as Arc<dyn CallableSpec<Flm>>
    })?;
    specs.register_type::<FlmPresentationSpec>(session, PRESENTATION_IDENTIFIER, |spec| {
        Arc::new(spec) as Arc<dyn CallableSpec<Flm>>
    })?;
    providers.register_type::<FlmFallbackProvider>(session, FALLBACK_GATE_IDENTIFIER, |gate| gate)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use techy::core::specs::Scope;
    use techy::latexlike::{MacroSpec, ParagraphBreakSpec};

    use crate::lang::build::{build_language, LangInputs};
    use crate::lang::mint::{ElementDef, SyntaxDef};

    #[test]
    fn a_provider_that_is_not_a_package_is_indexed_through_its_enumeration() {
        // A parse-time scope (a definition made mid-parse) is a provider that is
        // not a package; its definitions reach the environment through techy's
        // own enumeration, `SpecsProvider::iter_symbols`.
        let element = Arc::new(ElementSpec::new("t.emph"));
        let spec: Arc<dyn CallableSpec<Flm>> = Arc::new(FlmCallable::new(
            MacroSpec::<Flm>::new(Vec::new()),
            Arc::clone(&element),
        ));
        let mut scope = Scope::<Flm>::new("parse-time");
        scope.insert(FlmCallableType::Macro, "emph", spec);
        let provider: Arc<dyn SpecsProvider<Flm>> = Arc::new(scope);
        let environment = WriteEnvironment::for_providers([&provider]);
        let indexed = environment.spec_of(&element).expect("the scope's definition is indexed");
        assert!(Arc::ptr_eq(element_of_spec(&**indexed).unwrap(), &element));
    }

    #[test]
    fn of_two_providers_carrying_one_element_the_later_ones_spec_is_recorded() {
        let element = Arc::new(ElementSpec::new("t.emph"));
        let provider = |name: &str, macro_name: &str| -> (Arc<dyn SpecsProvider<Flm>>, Arc<dyn CallableSpec<Flm>>) {
            let spec: Arc<dyn CallableSpec<Flm>> = Arc::new(FlmCallable::new(
                MacroSpec::<Flm>::new(Vec::new()),
                Arc::clone(&element),
            ));
            let mut scope = Scope::<Flm>::new(name);
            scope.insert(FlmCallableType::Macro, macro_name, Arc::clone(&spec));
            (Arc::new(scope), spec)
        };
        let (earlier, _) = provider("earlier", "emph");
        let (later, later_spec) = provider("later", "textit");
        let environment = WriteEnvironment::for_providers([&earlier, &later]);
        let indexed = environment.spec_of(&element).expect("the element is indexed");
        assert!(Arc::ptr_eq(indexed, &later_spec));
    }

    #[test]
    fn the_write_environment_indexes_the_spec_carrying_a_declared_element() {
        let element = Arc::new(ElementSpec::new("t.emph"));
        let def = ElementDef::new(
            "t",
            ["emph"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::clone(&element),
        );
        let mut inputs = LangInputs::new();
        inputs.elements.push(def);
        let lang = build_language(inputs);
        let environment = WriteEnvironment::for_providers(lang.providers());
        let spec = environment.spec_of(&element).expect("the element's spec is indexed");
        assert!(Arc::ptr_eq(element_of_spec(&**spec).unwrap(), &element));
        assert!(environment.spec_of(&lang.structural().text).is_none());
    }

    #[test]
    fn a_spec_carrying_no_element_is_not_indexed() {
        let mut environment = WriteEnvironment::new();
        environment.index_spec(Arc::new(ParagraphBreakSpec) as Arc<dyn CallableSpec<Flm>>);
        assert!(environment.is_empty());
    }
}
