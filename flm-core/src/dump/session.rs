//! Serialization sessions for FLM: writing parsed fragments and trees in
//! techy's serialized form (the *wire form*) and reading them back into
//! in-memory objects.
//!
//! The wire form is techy's lossless serialization of a parse, which
//! [`crate::lang::serialize`] implements for FLM. Unlike the one-way
//! [projection](crate::dump::project), it reads back into in-memory objects with
//! their sharing and identity intact. The `flm dump --view wire` command
//! prints it, the library's round-trip tests use it, and a program can use it
//! to store parses (for example in a cache) and read them back.
//!
//! A techy serialization session holds one table per kind of object (sources,
//! parsing states, trees, diagnostics, and so on) and writes each object once.
//! An FLM session holds techy's standard tables plus one table of this
//! library's own, `flm.fragments` ([`FragmentSerdeDriver`]), whose entry for a
//! fragment ties together its source, its tree, its diagnostics and its region
//! layout ([`FragmentLayout`]).
//!
//! # Creating a session
//!
//! - [`writer`] creates a writing session over a built language
//!   ([`BuiltLang`], available as
//!   [`FlmRegistry::lang`](crate::core::registry::FlmRegistry::lang)). The
//!   language supplies the writing environment ([`WriteEnvironment`]), which
//!   records which specification carries which element.
//! - [`reader`] and [`reader_with`] create a reading session over a built
//!   language. Reading needs the reading environment: the language's
//!   specification providers ([`known_providers`]), its [structural
//!   elements](crate::lang::structural::StructuralElements), an
//!   [`ElementResolver`] for every other element name found in the data (an
//!   [`ElementTable`] when no registry is at hand), and the [`PayloadReaders`]
//!   that rebuild the payloads of presentation nodes.
//!
//! Every session is created with the recursion guard of the language
//! ([`BuiltLang::guard`]), so that writing or
//! reading a deeply nested tree is refused instead of overflowing the stack.
//! Like every guarded run, writing and reading must therefore execute on a
//! [worker thread](crate::lang::guard::on_worker_thread), a thread created with
//! enough stack; the functions of this module do not create one.
//!
//! Once a session is created, [`write_fragment`] and [`write_tree`] write objects
//! into a writing session, and [`read_fragment`] and [`read_tree`] read them from a reading
//! session. [`json::segment_line`](crate::dump::json::segment_line) writes the
//! segment a session emits as one line of JSON.
//!
//! A techy *profile* is a string that a writing session records in its stream
//! and that a reading session compares with its own: a stream with a different
//! profile is refused. No session of this module sets one, because only the
//! program can declare which configurations are compatible: a program that
//! caches serialized data calls techy's
//! [`set_profile`](crate::techy::serialize::SerdeSession::set_profile) on both its writer and its reader with the same
//! string, such as its registry's
//! [`digest`](crate::core::registry::FlmRegistry::digest), while a debugging
//! dump sets none, so that any stream can be inspected.
//!
//! # What a round trip preserves
//!
//! Writing a fragment and reading it back preserves: the tree's structure;
//! every span, into the same sources, shared; each node's parsing state,
//! shared as before and rebuilt once per state; the specifications by identity
//! (the instances the language's packages hold); the elements by identity,
//! through those specifications, otherwise through the element resolver; the
//! structural elements (the reading language's own); the diagnostics'
//! identifiers, data and spans; the diagnostics collection's retention limit and
//! its counts (the number of suppressed diagnostics and the number of errors,
//! so that a fragment reports the same thing about whether it holds errors on
//! both sides); and the fragment's region layout. The collection is rebuilt
//! from those parts, and counts that contradict each other are refused
//! ([`DeserializeError::InconsistentDiagnosticCounts`]) rather than corrected.
//!
//! A round trip does not preserve node ids and tree tags, which are new on
//! reading, nor the Rust types of diagnostic conditions, which come back as
//! techy's deserialized condition carrying the same identifier and data.
//!
//! # Identity and positions
//!
//! A fragment is an *object* of its table: writing the same `Arc<Fragment>`
//! twice yields one entry, because a fragment's identity is that `Arc`. Its tree
//! is written by its own `Arc` as well, so no tree is copied on writing.
//!
//! Every position these functions return or take ([`FragmentIndex`], techy's
//! [`TreeIndex`]) belongs to the session that assigned it and must never be
//! passed to another session. A position is carried between sessions as its
//! table's name together with its numeric index, and the receiving session
//! rebuilds it against its own table.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::error::Diagnostics;
use techy::serialize::{
    DeserializableValue, DeserializeContext, DeserializeError, DiagnosticIndex, KnownProviders,
    ObjectSerdeDriver, RegistrationError, SerdeSession, SerialEntry, SerialIndex, SerialValue,
    SerializableValue, SerializeContext, SerializeError, SourceIndex, StandardTableInterning,
    StandardTableReading, StandardTables, TableHandle, TreeIndex,
};

use crate::model::doc::{Fragment, FragmentLayout};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, FeatureName};
use crate::core::registry::HOST_FEATURE;
use crate::lang::build::BuiltLang;
use crate::lang::flm::Flm;
use crate::lang::guard;
use crate::lang::structural::StructuralElements;

/// The environment types and the registration function of a reading session,
/// re-exported here as their canonical public path. They are defined in
/// [`crate::lang::serialize`], beside the conversions that read them from the
/// session.
pub use crate::lang::serialize::{
    AmbiguousElement, ElementResolver, FALLBACK_GATE_IDENTIFIER, PRESENTATION_IDENTIFIER,
    PayloadReader, PayloadReaders, ReadEnvironment, SYNTHESIZED_IDENTIFIER, WriteEnvironment,
    register,
};

// --- a map-based element resolver ---------------------------------------------------------

/// An [`ElementResolver`] that looks element values up by name in a table the
/// caller fills: what a test, or a program without a registry, passes to
/// [`reader`].
///
/// Fill it with the structural elements of a language
/// ([`with_structural`](ElementTable::with_structural)) and the elements its
/// declarations carry ([`with`](ElementTable::with),
/// [`from_elements`](ElementTable::from_elements)). Every element value
/// inserted under a name is kept. A name held by several values resolves to
/// the error [`AmbiguousElement`] rather than to one of them, because a reader
/// that has only a name cannot tell which value was written.
#[derive(Clone, Default)]
pub struct ElementTable {
    elements: BTreeMap<ElementName, Vec<Arc<ElementSpec>>>,
}

impl ElementTable {
    /// Creates an empty table.
    pub fn new() -> ElementTable {
        ElementTable::default()
    }

    /// Creates a table holding the given element values.
    pub fn from_elements(elements: impl IntoIterator<Item = Arc<ElementSpec>>) -> ElementTable {
        let mut table = ElementTable::new();
        for element in elements {
            table.insert(element);
        }
        table
    }

    /// Builder: adds one element value.
    pub fn with(mut self, element: Arc<ElementSpec>) -> ElementTable {
        self.insert(element);
        self
    }

    /// Builder: adds every element of a set of structural elements, such as a
    /// language's ([`BuiltLang::structural`]).
    pub fn with_structural(mut self, structural: &StructuralElements) -> ElementTable {
        for element in structural.all() {
            self.insert(Arc::clone(element));
        }
        self
    }

    /// Adds one element value under its name and returns how many values the
    /// name now holds. The same `Arc` is never added twice.
    pub fn insert(&mut self, element: Arc<ElementSpec>) -> usize {
        let values = self.elements.entry(element.name.clone()).or_default();
        if !values.iter().any(|held| Arc::ptr_eq(held, &element)) {
            values.push(element);
        }
        values.len()
    }

    /// Returns the element values held under `name`, in insertion order (empty
    /// when the name is not held).
    pub fn values(&self, name: &str) -> &[Arc<ElementSpec>] {
        self.elements.get(name).map_or(&[], Vec::as_slice)
    }

    /// Returns the names held, in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &ElementName> + '_ {
        self.elements.keys()
    }
}

impl ElementResolver for ElementTable {
    /// Returns the one value held under `name`, `None` when there is none, and
    /// [`AmbiguousElement`] when there are several. A table does not record which
    /// feature declared a value, so the error lists the host
    /// ([`HOST_FEATURE`]) as the owner, once per value.
    fn resolve_element(&self, name: &str) -> Result<Option<Arc<ElementSpec>>, AmbiguousElement> {
        match self.values(name) {
            [] => Ok(None),
            [one] => Ok(Some(Arc::clone(one))),
            several => Err(AmbiguousElement {
                name: ElementName::from(name),
                owners: vec![FeatureName::from(HOST_FEATURE); several.len()],
            }),
        }
    }
}

impl fmt::Debug for ElementTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.elements.iter().map(|(name, values)| (name, values.len())))
            .finish()
    }
}

// --- the fragments table -----------------------------------------------------------------

/// The name of the fragments table that every FLM session holds beside techy's
/// standard tables ([`FragmentSerdeDriver`]).
pub const FRAGMENTS_TABLE: &str = "flm.fragments";

/// The identifier of a fragment's entry in the fragments table.
pub const FRAGMENT_IDENTIFIER: &str = "flm.fragment";

/// The serialized name of techy's trees table, given in the `UnknownTableName`
/// error when a session lacks the standard tables. techy's `standard_tables()`
/// returns `None` without saying which table is missing; the trees table is the
/// one every function here needs.
const TREES_TABLE: &str = "trees";

techy::serial_index! {
    /// A position in the fragments table, the serialized reference to a
    /// [`Fragment`] (the `Index` type of [`FragmentSerdeDriver`]). It belongs to
    /// the session that assigned it; see the [module documentation](self).
    pub struct FragmentIndex;
}

/// Writes and reads the entries of the fragments table ([`FRAGMENTS_TABLE`]),
/// which holds one kind of object, fragments ([`FRAGMENT_IDENTIFIER`]).
///
/// [`writer`] and [`reader`] register it on every FLM session. A fragment's
/// entry is `{source, tree, diagnostics: {items, limit, suppressed,
/// error_count}, layout}`:
///
/// - `source`: the position of the fragment's source in the sources table;
/// - `tree`: the position of its tree in the trees table (the tree written by
///   its `Arc`, as a tree with techy's unit annotation, that is, with no
///   per-node annotation value);
/// - `diagnostics`: the positions of its retained diagnostics in the
///   diagnostics table, in the order they were recorded, with the collection's
///   retention limit, its number of suppressed diagnostics and its number of
///   errors;
/// - `layout`: the fragment's region layout ([`FragmentLayout`]), `blocks`,
///   `inline` or `inline-or-blocks`.
///
/// Reading rebuilds the fragment from the shared source, the rebuilt tree and
/// a diagnostics collection rebuilt from the retained items, the limit and the
/// counts after techy checks that the counts are consistent. An entry whose
/// tree was parsed from another source than the entry's source is refused. The
/// [module documentation](self) says what a round trip preserves.
#[derive(Debug, Default)]
pub struct FragmentSerdeDriver;

/// The fields of a fragment's entry in the fragments table.
#[derive(SerializableValue, DeserializableValue)]
struct WireFragment {
    #[serial(name = "source")]
    source: SerialValue,
    #[serial(name = "tree")]
    tree: SerialValue,
    #[serial(name = "diagnostics")]
    diagnostics: WireDiagnostics,
    #[serial(name = "layout")]
    layout: String,
}

/// The fields of a fragment's diagnostics in its entry: the positions of the
/// retained diagnostics, the collection's retention limit and its counts.
#[derive(SerializableValue, DeserializableValue)]
struct WireDiagnostics {
    #[serial(name = "items")]
    items: Vec<SerialValue>,
    #[serial(name = "limit")]
    limit: usize,
    #[serial(name = "suppressed")]
    suppressed: usize,
    #[serial(name = "error_count")]
    error_count: usize,
}

fn index_value<I: SerialIndex>(position: I) -> SerialValue {
    SerialValue::Index { table: position.table(), index: position.index() }
}

fn index_from<I: SerialIndex>(value: &SerialValue, what: &str) -> Result<I, DeserializeError> {
    match value {
        SerialValue::Index { table, index } => Ok(I::from_parts(*table, *index)),
        other => Err(DeserializeError::failed(format!(
            "{what}: expected a table reference, found {other:?}"
        ))),
    }
}

fn layout_name(layout: FragmentLayout) -> &'static str {
    match layout {
        FragmentLayout::Blocks => "blocks",
        FragmentLayout::Inline => "inline",
        FragmentLayout::InlineOrBlocks => "inline-or-blocks",
    }
}

fn layout_from(name: &str) -> Result<FragmentLayout, DeserializeError> {
    match name {
        "blocks" => Ok(FragmentLayout::Blocks),
        "inline" => Ok(FragmentLayout::Inline),
        "inline-or-blocks" => Ok(FragmentLayout::InlineOrBlocks),
        other => Err(DeserializeError::failed(format!(
            "the fragment layout `{other}` must be one of `blocks`, `inline` and `inline-or-blocks`"
        ))),
    }
}

fn no_table_to_write(name: &str) -> SerializeError {
    SerializeError::UnknownTableName { name: String::from(name) }
}

fn no_table_to_read(name: &str) -> DeserializeError {
    DeserializeError::UnknownTableName { name: String::from(name) }
}

impl ObjectSerdeDriver<Flm> for FragmentSerdeDriver {
    type Object = Fragment;
    type Index = FragmentIndex;

    fn table_name(&self) -> &'static str {
        FRAGMENTS_TABLE
    }

    fn homogeneous_identifier(&self) -> Option<&'static str> {
        Some(FRAGMENT_IDENTIFIER)
    }

    fn serialize_object(
        &self,
        fragment: &Arc<Fragment>,
        cx: &mut SerializeContext<'_, Flm>,
    ) -> Result<SerialEntry, SerializeError> {
        let StandardTables { trees, diagnostics, .. } =
            cx.standard_tables().ok_or_else(|| no_table_to_write(TREES_TABLE))?;
        let source = cx.intern_source(fragment.source())?;
        let tree_arc: Arc<NodeTree<Flm>> = Arc::clone(fragment.tree());
        let tree_object: Arc<dyn Any + Send + Sync> = tree_arc;
        let tree = cx.intern(trees, &tree_object)?;
        let items = fragment
            .diags()
            .iter()
            .map(|diagnostic| {
                cx.intern(diagnostics, &Arc::new(diagnostic.clone())).map(index_value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let wire = WireFragment {
            source: index_value(source),
            tree: index_value(tree),
            diagnostics: WireDiagnostics {
                items,
                limit: fragment.diags().limit(),
                suppressed: fragment.diags().suppressed(),
                error_count: fragment.diags().error_count(),
            },
            layout: String::from(layout_name(fragment.layout())),
        };
        Ok(SerialEntry { identifier: FRAGMENT_IDENTIFIER.into(), data: wire.serialize_value(cx)? })
    }

    fn deserialize_object(
        &self,
        entry: &SerialEntry,
        cx: &mut DeserializeContext<'_, Flm>,
    ) -> Result<Arc<Fragment>, DeserializeError> {
        let wire = WireFragment::deserialize_value(&entry.data, cx)?;
        let StandardTables { trees, diagnostics, .. } =
            cx.standard_tables().ok_or_else(|| no_table_to_read(TREES_TABLE))?;
        let source =
            cx.source(index_from::<SourceIndex>(&wire.source, "the fragment's source")?)?;
        let tree_object =
            cx.object(trees, index_from::<TreeIndex>(&wire.tree, "the fragment's tree")?)?;
        let tree = tree_object.downcast::<NodeTree<Flm>>().map_err(|_| {
            DeserializeError::failed(
                "the entry of the fragment's tree is not a tree of FLM nodes (`NodeTree<Flm>`)",
            )
        })?;
        if !Arc::ptr_eq(tree.root().span().source(), &source) {
            return Err(DeserializeError::failed(
                "the fragment's source is not the source its tree was parsed from",
            ));
        }
        let mut items = Vec::with_capacity(wire.diagnostics.items.len());
        for item in &wire.diagnostics.items {
            let position = index_from::<DiagnosticIndex>(item, "a fragment diagnostic")?;
            items.push((*cx.object(diagnostics, position)?).clone());
        }
        let WireDiagnostics { limit, suppressed, error_count, .. } = wire.diagnostics;
        let diags = Diagnostics::from_parts(items, limit, suppressed, error_count)
            .map_err(DeserializeError::InconsistentDiagnosticCounts)?;
        let layout = layout_from(&wire.layout)?;
        Ok(Arc::new(Fragment::new(source, tree, diags, layout)))
    }
}

/// Returns the handle of the fragments table of `session`, or `None` when the
/// session has none (a session not created by [`writer`] or [`reader`]).
pub fn fragments_table(session: &SerdeSession<Flm>) -> Option<TableHandle<FragmentSerdeDriver>> {
    session.table_handle::<FragmentSerdeDriver>(FRAGMENTS_TABLE)
}

// --- the sessions ----------------------------------------------------------------------------

/// Returns the specification providers (techy's packages, and FLM's fallback
/// gate) against which a reading session resolves the serialized references to
/// specifications: every provider of the language's initial parsing state
/// (techy's built-in `_builtin` package included) and every provider the
/// language built from the features' declarations ([`BuiltLang::providers`]).
///
/// These are the very instances the language's parses use, so data read back
/// shares them with freshly parsed data. techy's recipe for its `_builtin`
/// package (the description from which techy rebuilds that package) is added
/// as well, used only for a name that no provider above
/// holds. [`reader_with`] sets this value on the session.
pub fn known_providers(lang: &BuiltLang) -> KnownProviders<Flm> {
    let mut known = KnownProviders::<Flm>::new();
    for provider in lang.initial_state().scopes().providers() {
        known.insert(Arc::clone(provider));
    }
    for provider in lang.providers() {
        known.insert(Arc::clone(provider));
    }
    techy::latexlike::serialize::register_package_recipes(&mut known);
    known
}

/// Creates a session with the standard tables and the fragments table, under the
/// language's recursion-guard policy.
fn session(lang: &BuiltLang) -> Result<SerdeSession<Flm>, RegistrationError> {
    let mut session = guard::serde_session(SerdeSession::<Flm>::new(), lang.guard());
    session.register_table(FragmentSerdeDriver)?;
    Ok(session)
}

/// Creates a writing session over the built language `lang`.
///
/// The session holds techy's standard tables and the fragments table, runs
/// under the language's recursion guard, and carries the writing environment
/// built from the language's packages, so that each node records the
/// specification that carries its element. Writing needs no reading
/// environment. No profile is set (see the [module documentation](self)): a
/// program that caches serialized data calls techy's
/// [`set_profile`](SerdeSession::set_profile) on the returned session.
///
/// Write objects with [`write_fragment`] and [`write_tree`], then take the
/// session's output with techy's
/// [`take_segment`](SerdeSession::take_segment), or with
/// [`take_segment_with_main`](SerdeSession::take_segment_with_main) to record
/// one position as the segment's *main entry* (the object a reader should
/// start from), and write it with
/// [`segment_line`](crate::dump::json::segment_line).
///
/// # Panics
///
/// Only if a new session refused the fragments table, which cannot happen: a
/// session holding just the standard tables accepts one more table with a
/// distinct name and a non-empty identifier. The
/// `expect` states an invariant of this module, and no requirement on the
/// caller is involved. This is why `writer` returns the session directly while
/// [`reader`] returns a `Result`: the registrations of a reader (techy's and
/// FLM's readers, [`register`]) can fail.
pub fn writer(lang: &BuiltLang) -> SerdeSession<Flm> {
    let mut session = session(lang).expect("a fresh session accepts the fragments table");
    let providers = lang.initial_state().scopes().providers().iter().chain(lang.providers());
    session.set_user_data(WriteEnvironment::for_providers(providers));
    session
}

/// Creates a reading session over the built language `lang`, in which
/// `elements` resolves every element name other than the structural elements.
///
/// This is [`reader_with`] with no payload readers; use [`reader_with`] to read
/// trees that hold presentation nodes.
///
/// # Errors
///
/// The registration errors of [`register`].
pub fn reader(
    lang: &BuiltLang,
    elements: Arc<dyn ElementResolver>,
) -> Result<SerdeSession<Flm>, RegistrationError> {
    reader_with(lang, elements, PayloadReaders::new())
}

/// Creates a reading session over the built language `lang`, with payload
/// readers.
///
/// The session holds the same tables as a [`writer`]'s, runs under the
/// language's recursion guard, and carries the reading environment: the
/// language's specification providers ([`known_providers`]), its structural
/// elements, `elements` to resolve every other element name found in the data,
/// and `payloads` to rebuild the payloads of presentation nodes. The readers of
/// FLM's stored forms are registered on it ([`register`]). No profile is set
/// (see the [module documentation](self)): a program that caches serialized
/// data calls techy's [`set_profile`](SerdeSession::set_profile) with the same
/// string as on its writer.
///
/// Give the session a segment with techy's
/// [`push_segment`](SerdeSession::push_segment), which returns the position of
/// the segment's main entry (the object the writer recorded as the one to
/// start from), if it has one; then read objects with [`read_fragment`] and
/// [`read_tree`].
///
/// # Errors
///
/// The registration errors of [`register`].
pub fn reader_with(
    lang: &BuiltLang,
    elements: Arc<dyn ElementResolver>,
    payloads: PayloadReaders,
) -> Result<SerdeSession<Flm>, RegistrationError> {
    let mut session = session(lang)?;
    session.set_user_data(known_providers(lang));
    session.set_user_data(
        ReadEnvironment::new(Arc::clone(lang.structural()), elements)
            .with_payload_readers(payloads),
    );
    register(&mut session)?;
    Ok(session)
}

/// Writes `fragment` into the fragments table of `session` and returns its
/// position, which belongs to `session`. Writing the same `Arc` again returns
/// the existing position.
///
/// Call it on a worker thread (see the [module documentation](self)).
///
/// # Errors
///
/// [`SerializeError::UnknownTableName`] if the session has no fragments table
/// (it was not created by [`writer`]); [`SerializeError::MissingProvenance`] if a
/// specification belongs to no package; the other errors of techy's writing
/// (interning) of an object.
pub fn write_fragment(
    session: &mut SerdeSession<Flm>,
    fragment: &Arc<Fragment>,
) -> Result<FragmentIndex, SerializeError> {
    let table = fragments_table(session).ok_or_else(|| no_table_to_write(FRAGMENTS_TABLE))?;
    session.intern(table, fragment)
}

/// Reads the fragment at `position` of the fragments table of `session`, and
/// returns the `Arc` the session holds: the same one for every call with that
/// position.
///
/// `position` must belong to `session`, never be taken from another session.
/// For a segment pushed into `session`, rebuild it from the index that
/// [`push_segment`](SerdeSession::push_segment) returned for the segment's main
/// entry, with techy's [`TableHandle::position`] on the handle from
/// [`fragments_table`].
/// Call this function on a worker thread (see the [module
/// documentation](self)).
///
/// # Errors
///
/// [`DeserializeError::UnknownTableName`] if the session has no fragments table;
/// the other errors of techy's reading of an object.
pub fn read_fragment(
    session: &mut SerdeSession<Flm>,
    position: FragmentIndex,
) -> Result<Arc<Fragment>, DeserializeError> {
    let table = fragments_table(session).ok_or_else(|| no_table_to_read(FRAGMENTS_TABLE))?;
    session.object(table, position)
}

/// Writes `tree` into the trees table of `session` and returns its position,
/// which belongs to `session`.
///
/// The tree is written by its `Arc`, without a copy; writing the same `Arc`
/// again returns the existing position. Call this function on a worker thread
/// (see the [module documentation](self)).
///
/// # Errors
///
/// [`SerializeError::UnknownTableName`] if the session has no trees table; the
/// other errors of techy's writing (interning) of an object.
pub fn write_tree(
    session: &mut SerdeSession<Flm>,
    tree: &Arc<NodeTree<Flm>>,
) -> Result<TreeIndex, SerializeError> {
    let table =
        session.standard_tables().ok_or_else(|| no_table_to_write(TREES_TABLE))?.trees;
    let tree_arc: Arc<NodeTree<Flm>> = Arc::clone(tree);
    let object: Arc<dyn Any + Send + Sync> = tree_arc;
    session.intern(table, &object)
}

/// Reads the tree at `position` of the trees table of `session`, and returns
/// the shared `Arc` the session holds.
///
/// `position` must belong to `session`, never be taken from another session.
/// For a segment pushed into `session`, rebuild it from the index that
/// [`push_segment`](SerdeSession::push_segment) returned for the segment's main
/// entry, with techy's [`TableHandle::position`] on the trees table's handle
/// (the `trees` field of
/// [`SerdeSession::standard_tables`](SerdeSession::standard_tables)). Call this
/// function on a worker thread (see the [module documentation](self)).
///
/// # Errors
///
/// [`DeserializeError::UnknownTableName`] if the session has no trees table; a
/// failure if the entry is not a `NodeTree<Flm>` (a tree of FLM nodes with
/// techy's unit annotation); the other errors of
/// techy's reading of an object.
pub fn read_tree(
    session: &mut SerdeSession<Flm>,
    position: TreeIndex,
) -> Result<Arc<NodeTree<Flm>>, DeserializeError> {
    let table = session.standard_tables().ok_or_else(|| no_table_to_read(TREES_TABLE))?.trees;
    let object = session.object(table, position)?;
    object
        .downcast::<NodeTree<Flm>>()
        .map_err(|_| {
            DeserializeError::failed("the entry is not a tree of FLM nodes (`NodeTree<Flm>`)")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_element_table_reports_a_name_several_values_share() {
        let first = Arc::new(ElementSpec::new("t.heading"));
        let second = Arc::new(ElementSpec::new("t.heading"));
        let table = ElementTable::new().with(Arc::clone(&first)).with(Arc::clone(&second));
        let error = table.resolve_element("t.heading").unwrap_err();
        let host = FeatureName::from(HOST_FEATURE);
        let name = ElementName::from("t.heading");
        let expected = AmbiguousElement { name, owners: vec![host; 2] };
        assert_eq!(error, expected);
        assert!(table.resolve_element("t.other").unwrap().is_none());
    }

    #[test]
    fn an_element_table_resolves_a_name_with_one_value() {
        let element = Arc::new(ElementSpec::new("t.emph"));
        let mut table = ElementTable::new();
        assert_eq!(table.insert(Arc::clone(&element)), 1);
        assert_eq!(table.insert(Arc::clone(&element)), 1, "the same value is held once");
        let resolved = table.resolve_element("t.emph").unwrap().expect("the name resolves");
        assert!(Arc::ptr_eq(&resolved, &element));
    }

    #[test]
    fn a_reading_environment_over_a_language_holds_every_minted_provider() {
        let lang = crate::lang::build::build_language(crate::lang::build::LangInputs::new());
        let known = known_providers(&lang);
        for provider in lang.providers() {
            assert!(known.get(provider.name()).is_some(), "`{}` is held", provider.name());
        }
        assert!(known.get("_builtin").is_some(), "the seed's dispatch package is held");
    }
}
