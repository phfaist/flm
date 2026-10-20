//! Shared helpers for flm-core's integration tests. The sections below are
//! delimited by comment banners and grouped by the suites they serve.
#![allow(dead_code)]

// ============================================================================
// layout: helpers for `tests/layout_*.rs`
// ============================================================================

/// Helpers for the layout engine's tests: the invariants every rendering must
/// satisfy, and a byte-exact golden check over `tests/goldens/layout/`.
pub mod layout {
    use std::path::PathBuf;

    use flm_core::layout::display_width;

    /// The engine must never produce a line with trailing whitespace, and
    /// blank lines must be completely empty.
    pub fn assert_no_trailing_ws(out: &str) {
        for (i, line) in out.split('\n').enumerate() {
            assert_eq!(
                line.trim_end(),
                line,
                "line {} has trailing whitespace: {:?}\n--- full output ---\n{}",
                i + 1,
                line,
                out
            );
        }
    }

    /// No line of `out` exceeds `width` display columns (measured the way the
    /// engine measures).
    pub fn assert_max_width(out: &str, width: u32) {
        for (i, line) in out.split('\n').enumerate() {
            let w = display_width(line);
            assert!(
                w <= width,
                "line {} is {} columns > {}: {:?}\n--- full output ---\n{}",
                i + 1,
                w,
                width,
                line,
                out
            );
        }
    }

    /// Structural invariants that hold for every rendering: no trailing
    /// whitespace, and a non-empty output ends with exactly one newline.
    pub fn assert_well_formed(out: &str) {
        assert_no_trailing_ws(out);
        if !out.is_empty() {
            assert!(out.ends_with('\n'), "output must end with a newline: {:?}", out);
            assert!(
                !out.ends_with("\n\n"),
                "output must end with exactly one newline: {:?}",
                out
            );
        }
    }

    /// The path of the golden `tests/goldens/layout/<name>`, for
    /// `flm_core::dump::golden::assert_text_golden` (`UPDATE_GOLDEN=1 cargo test`,
    /// exactly `"1"`, rewrites a golden instead of comparing).
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("layout");
        p.push(name);
        p
    }
}

// ============================================================================
// end layout
// ============================================================================

// ---------------------------------------------------------------------------------
// the language layer: build a language from a few declarations, parse a
// string into a fragment, and look at the parsed nodes.
// ---------------------------------------------------------------------------------

use std::sync::Arc;

use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::model::element::ElementSpec;
use flm_core::model::names::ElementName;
use flm_core::lang::mint::{ArgDef, ArgStdKind, BodyDef, ElementDef, SyntaxDef};
use flm_core::lang::{
    build_language, parse_fragment, BuiltLang, Flm, FlmOrigin, FlmSource, LangInputs,
    ParseOptions,
};
use techy::core::node::NodeRef;
use techy::source::Source;

/// An element named `name`, with no aspects and no value.
pub fn element(name: &str) -> Arc<ElementSpec> {
    Arc::new(ElementSpec::new(ElementName::from(name)))
}

/// A macro `\name` declared by `owner`, taking `mandatory` mandatory arguments;
/// its element is named `owner.name`.
pub fn macro_def(owner: &str, name: &str, mandatory: usize) -> ElementDef {
    let args = (0..mandatory).map(|_| ArgDef::new(ArgStdKind::Mandatory)).collect();
    ElementDef::new(owner, [name], SyntaxDef::Macro { args }, element(&format!("{owner}.{name}")))
}

/// A macro `\name` declared by `owner` with the given argument list; its element
/// is named `owner.name`.
pub fn macro_with(owner: &str, name: &str, args: Vec<ArgDef>) -> ElementDef {
    ElementDef::new(owner, [name], SyntaxDef::Macro { args }, element(&format!("{owner}.{name}")))
}

/// An environment `\begin{name}…\end{name}` declared by `owner`, without
/// arguments, with the given body; its element is named `owner.name`.
pub fn env_def(owner: &str, name: &str, body: BodyDef) -> ElementDef {
    ElementDef::new(
        owner,
        [name],
        SyntaxDef::Environment { args: Vec::new(), body },
        element(&format!("{owner}.{name}")),
    )
}

/// The language built from `inputs`; a build error is a test failure.
pub fn build(inputs: LangInputs) -> BuiltLang {
    let lang = build_language(inputs);
    assert!(lang.is_sound(), "the language build reported errors: {:?}", lang.errors());
    lang
}

/// The language over the given element declarations, everything else default.
pub fn lang(elements: Vec<ElementDef>) -> BuiltLang {
    let mut inputs = LangInputs::new();
    inputs.elements = elements;
    build(inputs)
}

/// A source over `text` with no canonical path.
pub fn source(text: &str) -> Arc<FlmSource> {
    Arc::new(Source::new(text).with_origin(FlmOrigin::none()))
}

/// Parses `text` under `lang` with the default layout and options; a parse
/// that yields no tree is a test failure.
pub fn parse(lang: &BuiltLang, text: &str) -> Fragment {
    parse_fragment(lang, source(text), FragmentLayout::default(), &ParseOptions::default())
        .unwrap_or_else(|error| panic!("the parse of {text:?} yielded no tree: {error}"))
}

/// The callable nodes of a fragment, in document order.
pub fn callables(fragment: &Fragment) -> Vec<NodeRef<'_, Flm>> {
    fragment.tree().root().descendants().filter(|node| node.is_callable()).collect()
}

/// The names of the callable nodes of a fragment, in document order.
pub fn callable_names(fragment: &Fragment) -> Vec<String> {
    callables(fragment).iter().filter_map(|node| node.name()).map(String::from).collect()
}

// ============================================================================
// dump: helpers for `tests/dump_*.rs` — golden paths, element
// resolvers, round trips through the serialization sessions, hand-staged
// trees, and a carrier spec for checking a value's serialized form.
// ============================================================================

/// Helpers for the serialization and projection tests.
pub mod dump {
    use std::fmt;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use flm_core::model::doc::Fragment;
    use flm_core::model::element::ElementSpec;
    use flm_core::dump::session::{self, ElementResolver, ElementTable, PayloadReaders};
    use flm_core::lang::mint::ElementDef;
    use flm_core::lang::{BuiltLang, Flm, FlmNodeExt, FlmOrigin};
    use techy::core::node::{BuildId, NodeBuildError, NodeKind, NodeTree, NodeTreeBuilder};
    use techy::core::specs::CallableSpec;
    use techy::core::{Lang, ParsingState};
    use techy::serialize::{
        DeserializableObject, DeserializableValue, DeserializeContext, DeserializeError,
        SerdeSession, Segment, SerialEntry, SerialIndex, SerialValue, SerializableObject,
        SerializableValue, SerializeContext, SerializeError, StandardTableInterning,
        StandardTableReading, TreeIndex,
    };
    use techy::source::SourceSpan;

    /// `tests/goldens/dump/<name>`.
    pub fn golden(name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("goldens");
        path.push("dump");
        path.push(name);
        path
    }

    /// The elements the declarations carry, for a resolver.
    pub fn elements_of(defs: &[ElementDef]) -> Vec<Arc<ElementSpec>> {
        defs.iter().map(|def| Arc::clone(&def.element)).collect()
    }

    /// A resolver over the given elements (the structural set needs no entry:
    /// the reading environment resolves it from the language).
    pub fn resolver(elements: Vec<Arc<ElementSpec>>) -> Arc<dyn ElementResolver> {
        Arc::new(ElementTable::from_elements(elements))
    }

    /// Writes `fragment` with a writer over `lang` and reads it back into a
    /// fresh reader over `lang` whose resolver knows `elements`.
    pub fn round_trip(
        lang: &BuiltLang,
        fragment: &Arc<Fragment>,
        elements: Vec<Arc<ElementSpec>>,
    ) -> Arc<Fragment> {
        round_trip_with(lang, fragment, elements, PayloadReaders::new())
    }

    /// [`round_trip`] with payload readers on the reading side.
    pub fn round_trip_with(
        lang: &BuiltLang,
        fragment: &Arc<Fragment>,
        elements: Vec<Arc<ElementSpec>>,
        payloads: PayloadReaders,
    ) -> Arc<Fragment> {
        let (_, segment) = write(lang, fragment);
        read_fragment_from(lang, segment, elements, payloads).expect("the fragment reads back")
    }

    /// Reads the fragment a segment's main entry names through a fresh reader
    /// over `lang` whose resolver knows `elements` and whose payload readers are
    /// `payloads`: the fragment, or the error the reader reports (a segment or an
    /// entry it refuses).
    pub fn read_fragment_from(
        lang: &BuiltLang,
        segment: Segment,
        elements: Vec<Arc<ElementSpec>>,
        payloads: PayloadReaders,
    ) -> Result<Arc<Fragment>, DeserializeError> {
        let mut reader =
            session::reader_with(lang, resolver(elements), payloads).expect("the reader prepares");
        let (table, index) = reader.push_segment(segment)?.expect("a main entry");
        let fragments = session::fragments_table(&reader).expect("the fragments table");
        assert_eq!(table, fragments.id());
        session::read_fragment(&mut reader, fragments.position(index))
    }

    /// Writes `fragment` with a writer over `lang`: its position in the
    /// writer's numbering and the segment holding it (with `fragment` as the
    /// main entry).
    pub fn write(lang: &BuiltLang, fragment: &Arc<Fragment>) -> (session::FragmentIndex, Segment) {
        let mut writer = session::writer(lang);
        let position = session::write_fragment(&mut writer, fragment).expect("the fragment writes");
        let segment = writer.take_segment_with_main(position).expect("the main entry is known");
        (position, segment)
    }

    /// Writes `tree` with a writer over `lang`: its position in the writer's
    /// numbering and the segment holding it.
    pub fn write_tree(lang: &BuiltLang, tree: &Arc<NodeTree<Flm>>) -> (TreeIndex, Segment) {
        let mut writer = session::writer(lang);
        let position = session::write_tree(&mut writer, tree).expect("the tree writes");
        let segment = writer.take_segment();
        (position, segment)
    }

    /// Reads the tree written at `position` (the writer's numbering) back
    /// through a fresh reader over `lang`.
    pub fn read_tree(
        lang: &BuiltLang,
        segment: Segment,
        position: TreeIndex,
        elements: Vec<Arc<ElementSpec>>,
        payloads: PayloadReaders,
    ) -> Arc<NodeTree<Flm>> {
        try_read_tree(lang, segment, position, elements, payloads).expect("the tree reads")
    }

    /// [`read_tree`], returning the reader's error instead of failing the test.
    pub fn try_read_tree(
        lang: &BuiltLang,
        segment: Segment,
        position: TreeIndex,
        elements: Vec<Arc<ElementSpec>>,
        payloads: PayloadReaders,
    ) -> Result<Arc<NodeTree<Flm>>, DeserializeError> {
        let mut reader =
            session::reader_with(lang, resolver(elements), payloads).expect("the reader prepares");
        reader.push_segment(segment)?;
        let trees = reader.standard_tables().expect("standard tables").trees;
        session::read_tree(&mut reader, trees.position(position.index()))
    }

    // --- hostile wire input: editing a segment by hand -----------------------------

    /// `segment` with the entries of its table named `table` edited: the
    /// segment's serialized form is taken apart, `edit` runs over that table's
    /// entry list, and the segment is rebuilt from the result — how a test stages
    /// wire input no writer produces. An entry of a table holding one kind of
    /// object (the fragments table) is its bare data; an entry of any other
    /// table (the specs table) is `{identifier, data}`. Positions inside an entry
    /// are the writing session's numbering, as on the wire.
    pub fn edit_entries(
        segment: &Segment,
        table: &str,
        edit: impl FnOnce(&mut Vec<SerialValue>),
    ) -> Segment {
        let mut value = segment.to_serial_value();
        let tables = match field_mut(&mut value, "tables") {
            SerialValue::List(tables) => tables,
            other => panic!("a segment's tables are a list, not {other:?}"),
        };
        let named = |entry: &SerialValue| {
            matches!(field(entry, "name"), Some(SerialValue::Str(name)) if name == table)
        };
        let entry = tables
            .iter_mut()
            .find(|entry| named(entry))
            .unwrap_or_else(|| panic!("the segment holds no table named `{table}`"));
        match field_mut(entry, "entries") {
            SerialValue::List(entries) => edit(entries),
            other => panic!("a table's entries are a list, not {other:?}"),
        }
        Segment::from_serial_value(&value).expect("the edited segment keeps a segment's shape")
    }

    /// The value under `key` of a map value, if the map has it.
    pub fn field<'v>(value: &'v SerialValue, key: &str) -> Option<&'v SerialValue> {
        match value {
            SerialValue::Map(entries) => {
                entries.iter().find(|(name, _)| name == key).map(|(_, value)| value)
            }
            _ => None,
        }
    }

    /// The value under `key` of a map value, to edit; a missing key or a value
    /// that is not a map is a test failure.
    pub fn field_mut<'v>(value: &'v mut SerialValue, key: &str) -> &'v mut SerialValue {
        match value {
            SerialValue::Map(entries) => entries
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("the map has no field `{key}`")),
            other => panic!("a map with a field `{key}`, not {other:?}"),
        }
    }

    /// The wire's reference to a table position.
    pub fn index_value<I: SerialIndex>(position: I) -> SerialValue {
        SerialValue::Index { table: position.table(), index: position.index() }
    }

    // --- staging nodes by hand -------------------------------------------------------

    /// A staging site: a builder, the state every node is staged under, and the
    /// one (empty) span every node carries — what a rewrite or the lowering
    /// stage does when it synthesizes nodes.
    pub struct Site {
        pub builder: NodeTreeBuilder<Flm>,
        pub state: Arc<ParsingState<Flm>>,
        pub span: SourceSpan<FlmOrigin>,
    }

    impl Site {
        /// A site over the language's initial state and an empty source.
        pub fn new(lang: &BuiltLang) -> Site {
            let source = super::source("");
            Site {
                builder: NodeTreeBuilder::new(),
                state: Arc::clone(lang.initial_state()),
                span: SourceSpan::new(&source, 0..0),
            }
        }

        /// Stages a node with the extension the language mints.
        pub fn stage(
            &mut self,
            kind: NodeKind<Flm>,
            children: Vec<BuildId>,
        ) -> Result<BuildId, NodeBuildError> {
            let staged = self.builder.staged_children(&children);
            let ext = Flm::make_node_ext(&kind, &self.span, &self.state, staged)?;
            self.stage_with_ext(kind, children, ext)
        }

        /// Stages a node with the given extension (a payload, a source map).
        pub fn stage_with_ext(
            &mut self,
            kind: NodeKind<Flm>,
            children: Vec<BuildId>,
            ext: FlmNodeExt,
        ) -> Result<BuildId, NodeBuildError> {
            self.builder.add(kind, self.span.clone(), Arc::clone(&self.state), children, ext, ())
        }

        /// Finishes the tree under a list root over `children`.
        pub fn finish(mut self, children: Vec<BuildId>) -> Arc<NodeTree<Flm>> {
            let root = self.stage(NodeKind::list(), children).expect("the root stages");
            Arc::new(self.builder.finish(root).expect("the tree finishes"))
        }
    }

    // --- the carrier: a value's serialized form -------------------------------------

    /// The identifier of the carrier spec.
    pub const CARRIER: &str = "test.carrier";

    /// A spec carrying one value, so that the value's own conversions run with a
    /// real context (the pattern of techy's derive tests); it records what it
    /// wrote.
    #[derive(Debug)]
    pub struct Carrier<T> {
        value: T,
        written: Mutex<Option<SerialValue>>,
    }

    impl<T> Carrier<T> {
        pub fn new(value: T) -> Carrier<T> {
            Carrier { value, written: Mutex::new(None) }
        }
    }

    impl<T: SerializableValue<Flm> + Send + Sync + 'static + fmt::Debug> SerializableObject<Flm>
        for Carrier<T>
    {
        fn serialize_object(
            &self,
            cx: &mut SerializeContext<'_, Flm>,
        ) -> Result<SerialEntry, SerializeError> {
            let data = self.value.serialize_value(cx)?;
            *self.written.lock().unwrap() = Some(data.clone());
            Ok(SerialEntry { identifier: CARRIER.into(), data })
        }
    }

    impl<T: DeserializableValue<Flm> + Send + Sync + 'static + fmt::Debug> DeserializableObject<Flm>
        for Carrier<T>
    {
        type Output = T;

        fn deserialize_object(
            value: &SerialValue,
            cx: &mut DeserializeContext<'_, Flm>,
        ) -> Result<T, DeserializeError> {
            T::deserialize_value(value, cx)
        }
    }

    impl<T: SerializableValue<Flm> + Send + Sync + 'static + fmt::Debug> CallableSpec<Flm>
        for Carrier<T>
    {
    }

    /// The stand-in spec the reader holds once the value was handed out.
    #[derive(Debug)]
    struct Placeholder;
    impl SerializableObject<Flm> for Placeholder {}
    impl CallableSpec<Flm> for Placeholder {}

    /// Writes `value` inside a carrier with a writer over `lang`, reads the
    /// segment with a reader over `lang` whose resolver knows `elements` and
    /// whose payload readers are `payloads`: the written form and the read-back
    /// value (or the read error).
    pub fn round_trip_value<T>(
        lang: &BuiltLang,
        elements: Vec<Arc<ElementSpec>>,
        payloads: PayloadReaders,
        value: T,
    ) -> (SerialValue, Result<T, DeserializeError>)
    where
        T: SerializableValue<Flm> + DeserializableValue<Flm> + Send + Sync + 'static + fmt::Debug,
    {
        round_trip_value_with(lang, elements, payloads, value, |_| {}, |_| {})
    }

    /// [`round_trip_value`] with two hooks for staging what no writer produces:
    /// `prepare` runs on the writer before the carrier is written (to intern
    /// other objects the edited form may refer to), and `edit` runs on the
    /// carrier's written form inside the segment before it is read (the written
    /// form returned is the one before the edit).
    pub fn round_trip_value_with<T>(
        lang: &BuiltLang,
        elements: Vec<Arc<ElementSpec>>,
        payloads: PayloadReaders,
        value: T,
        prepare: impl FnOnce(&mut SerdeSession<Flm>),
        edit: impl FnOnce(&mut SerialValue),
    ) -> (SerialValue, Result<T, DeserializeError>)
    where
        T: SerializableValue<Flm> + DeserializableValue<Flm> + Send + Sync + 'static + fmt::Debug,
    {
        let carrier = Arc::new(Carrier::new(value));
        let spec: Arc<dyn CallableSpec<Flm>> = Arc::clone(&carrier) as Arc<dyn CallableSpec<Flm>>;
        let mut writer = session::writer(lang);
        prepare(&mut writer);
        let position = writer.intern_spec(&spec).expect("the carrier writes");
        let written = carrier.written.lock().unwrap().clone().expect("the carrier recorded it");
        let segment = writer.take_segment();
        let segment = edit_entries(&segment, "specs", |entries| {
            let is_carrier = |entry: &SerialValue| {
                matches!(field(entry, "identifier"), Some(SerialValue::Str(id)) if id == CARRIER)
            };
            let entry = entries
                .iter_mut()
                .find(|entry| is_carrier(entry))
                .expect("the segment holds the carrier's entry");
            edit(field_mut(entry, "data"));
        });

        let received: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
        let mut reader =
            session::reader_with(lang, resolver(elements), payloads).expect("the reader prepares");
        let specs = reader.standard_tables().expect("standard tables").specs;
        let sink = Arc::clone(&received);
        specs
            .register_type::<Carrier<T>>(&mut reader, CARRIER, move |value| {
                *sink.lock().unwrap() = Some(value);
                Arc::new(Placeholder) as Arc<dyn CallableSpec<Flm>>
            })
            .expect("the carrier reader registers");
        let read = match reader.push_segment(segment) {
            Err(error) => Err(error),
            Ok(_) => reader
                .spec(specs.position(position.index()))
                .map(|_| received.lock().unwrap().take().expect("the reader handed the value out")),
        };
        (written, read)
    }
}

// blocks: helpers for `tests/blocks_*.rs`
// ============================================================================

/// Helpers for the block-structure tests: a toy language declaring one element
/// per block role, the pass run over a parsed string, and probes over the
/// structured tree.
pub mod blocks {
    use std::ops::Range;
    use std::sync::Arc;

    use flm_core::model::aspects::{
        BlockRole, BlockRoleAspect, FixedBlockRole, InlineSpacing, Placement,
    };
    use flm_core::model::doc::FragmentLayout;
    use flm_core::model::element::ElementSpec;
    use flm_core::model::names::ElementName;
    use flm_core::lang::ext::{TextSourceMap, TextSourcePiece};
    use flm_core::lang::mint::{ArgDef, ArgStdKind, BodyDef, BodyLayout, ElementDef, SyntaxDef};
    use flm_core::lang::rewrites::blocks::{structure, BlocksConfig};
    use flm_core::lang::{
        BuiltLang, Flm, FlmDiagnostics, FlmGroupType, FlmNodeExt, FlmOrigin, FlmSource,
        LangInputs,
    };
    use techy::core::node::{
        display_tree, validate_tree, BuildId, GroupData, NodeKind, NodeRef, NodeTree,
        NodeTreeBuilder,
    };
    use techy::core::{Lang, ParsingState};
    use techy::latexlike::Mode;
    use techy::source::SourceSpan;

    /// An element named `name` carrying the given fixed block role.
    pub fn element_with_role(name: &str, role: BlockRole) -> Arc<ElementSpec> {
        Arc::new(
            ElementSpec::new(ElementName::from(name))
                .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(role))),
        )
    }

    /// A macro `\name` of the toy owner `t` with `mandatory` mandatory arguments
    /// and the given element.
    pub fn macro_of(name: &str, mandatory: usize, element: Arc<ElementSpec>) -> ElementDef {
        let args = (0..mandatory).map(|_| ArgDef::new(ArgStdKind::Mandatory)).collect();
        ElementDef::new("t", [name], SyntaxDef::Macro { args }, element)
    }

    /// An environment `\begin{name}…\end{name}` of the toy owner `t` with the
    /// given body declaration and element.
    pub fn env_of(name: &str, body: BodyDef, element: Arc<ElementSpec>) -> ElementDef {
        ElementDef::new("t", [name], SyntaxDef::Environment { args: Vec::new(), body }, element)
    }

    /// The toy declarations: one element per block role and body shape.
    ///
    /// - `quotation`: a block environment with a blocks body;
    /// - `section`: a block macro with one argument;
    /// - `runin`, `qed`, `marginnote`: a `Leads`, a `Trails`, a `Free` decorator;
    /// - `snug`: inline, absorbing the whitespace before it; `tight`: inline,
    ///   absorbing the whitespace after it;
    /// - `emph`: ordinary inline content with one argument;
    /// - `verb`: a verbatim argument; `verbatimcode`: a verbatim environment;
    /// - `inlineenv`: an inline environment with the default body (its role's:
    ///   inline); `blockenv`: a block environment with the default body (its
    ///   role's: blocks); `blockinline`: a block environment with an explicitly
    ///   inline body; `blocksbody`: an inline environment with a blocks body;
    /// - `equation`: a math-mode body; `textinmath`: a text-mode body (to nest
    ///   inside math), declared in math mode.
    ///
    /// Every declaration is available in text mode, the default; `emph` also in
    /// math mode, `textinmath` and `textarg` in math mode only.
    pub fn toy_elements() -> Vec<ElementDef> {
        let inline = |name: &str| super::element(name);
        let snug = BlockRole::Inline(InlineSpacing { absorb_before: true, absorb_after: false });
        let tight = BlockRole::Inline(InlineSpacing { absorb_before: false, absorb_after: true });
        vec![
            env_of(
                "quotation",
                BodyDef::new().blocks(),
                element_with_role("t.quotation", BlockRole::Block),
            ),
            macro_of("section", 1, element_with_role("t.section", BlockRole::Block)),
            macro_of(
                "runin",
                1,
                element_with_role("t.runin", BlockRole::BlockDecorator(Placement::Leads)),
            ),
            macro_of(
                "qed",
                0,
                element_with_role("t.qed", BlockRole::BlockDecorator(Placement::Trails)),
            ),
            macro_of(
                "marginnote",
                1,
                element_with_role("t.marginnote", BlockRole::BlockDecorator(Placement::Free)),
            ),
            macro_of("snug", 1, element_with_role("t.snug", snug)),
            macro_of("tight", 0, element_with_role("t.tight", tight)),
            macro_of("emph", 1, inline("t.emph")).modes([Mode::Text, Mode::Math]),
            ElementDef::new(
                "t",
                ["verb"],
                SyntaxDef::Macro {
                    args: vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })],
                },
                inline("t.verb"),
            ),
            env_of("verbatimcode", BodyDef::new().verbatim(), inline("t.verbatimcode")),
            env_of("inlineenv", BodyDef::new(), inline("t.inlineenv")),
            env_of(
                "blockenv",
                BodyDef::new(),
                element_with_role("t.blockenv", BlockRole::Block),
            ),
            env_of(
                "blockinline",
                BodyDef::new().layout(BodyLayout::Inline),
                element_with_role("t.blockinline", BlockRole::Block),
            ),
            env_of("blocksbody", BodyDef::new().blocks(), inline("t.blocksbody")),
            env_of("equation", BodyDef::new().mode(Mode::Math), inline("t.equation")),
            env_of("textinmath", BodyDef::new().mode(Mode::Text), inline("t.textinmath"))
                .modes([Mode::Math]),
            // A text command: one mandatory argument read in text mode, the
            // shape of `\text{…}` inside mathematics.
            ElementDef::new(
                "t",
                ["textarg"],
                SyntaxDef::Macro {
                    args: vec![ArgDef::new(ArgStdKind::Mandatory).mode(Mode::Text)],
                },
                inline("t.textarg"),
            )
            .modes([Mode::Math]),
        ]
    }

    /// The toy language.
    pub fn toy_lang() -> BuiltLang {
        super::lang(toy_elements())
    }

    /// The toy language plus further declarations.
    pub fn toy_lang_with(extra: Vec<ElementDef>) -> BuiltLang {
        let mut inputs = LangInputs::new();
        inputs.elements = toy_elements();
        inputs.elements.extend(extra);
        super::build(inputs)
    }

    /// A structured tree with the pass's diagnostics.
    pub struct Laid {
        pub tree: Arc<NodeTree<Flm>>,
        pub diags: FlmDiagnostics,
    }

    impl Laid {
        pub fn root(&self) -> NodeRef<'_, Flm> {
            self.tree.root()
        }

        pub fn children(&self) -> Vec<NodeRef<'_, Flm>> {
            self.tree.root().children().iter().collect()
        }

        pub fn dump(&self) -> String {
            display_tree(self.tree.root())
        }

        /// The identifiers of the pass's diagnostics, in emission order.
        pub fn identifiers(&self) -> Vec<&str> {
            self.diags.iter().map(|d| d.identifier()).collect()
        }
    }

    /// Parses `text` under `lang` (a parse with errors is a test failure) and
    /// runs the pass with the given layout and options; the result satisfies
    /// techy's all-trees law.
    pub fn lay_out(
        lang: &BuiltLang,
        text: &str,
        layout: FragmentLayout,
        config: &BlocksConfig,
    ) -> Laid {
        let fragment = super::parse(lang, text);
        assert!(!fragment.diags().has_errors(), "{}", fragment.diags().render_all());
        let mut diags = FlmDiagnostics::new();
        let tree = structure(fragment.tree(), layout, config, &mut diags)
            .unwrap_or_else(|error| panic!("the pass yielded no tree for {text:?}: {error}"));
        validate_tree(&tree).expect("the structured tree is well formed");
        Laid { tree, diags }
    }

    /// [`lay_out`] with the default options.
    pub fn run(lang: &BuiltLang, text: &str, layout: FragmentLayout) -> Laid {
        lay_out(lang, text, layout, &BlocksConfig::default())
    }

    /// Parses `text` under `lang`, tolerating parse errors (an unknown
    /// environment, say); only a parse yielding no tree is a test failure.
    pub fn parse_lenient(lang: &BuiltLang, text: &str) -> flm_core::model::doc::Fragment {
        super::parse(lang, text)
    }

    /// The flattened text of a node and its descendants.
    pub fn text_of(node: NodeRef<'_, Flm>) -> String {
        let mut out = String::new();
        if let Some(text) = node.chars() {
            out.push_str(text);
        }
        for child in node.descendants() {
            if let Some(text) = child.chars() {
                out.push_str(text);
            }
        }
        out
    }

    pub fn is_paragraph(node: &NodeRef<'_, Flm>) -> bool {
        node.group_type() == Some(FlmGroupType::Paragraph)
    }

    /// The paragraph groups among a node's children.
    pub fn paragraphs<'t>(node: NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
        node.children().iter().filter(is_paragraph).collect()
    }

    /// Every text node below a node, in document order.
    pub fn chars_nodes<'t>(node: NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
        node.descendants().filter(|n| n.is_chars()).collect()
    }

    /// Whether a text node's content is owned (normalized) rather than
    /// span-backed.
    pub fn is_owned(node: &NodeRef<'_, Flm>) -> bool {
        match node.kind() {
            NodeKind::Chars { content } => content.is_owned(),
            _ => false,
        }
    }

    /// A text node's source-map pieces as `(output length, source text)`; empty
    /// without a map.
    pub fn pieces(node: &NodeRef<'_, Flm>) -> Vec<(u32, String)> {
        node.ext()
            .source_map
            .as_ref()
            .map(|map| {
                map.pieces()
                    .iter()
                    .map(|piece| (piece.out_len, piece.span.content().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A text node's source map, when present, tiles its text.
    pub fn assert_map_tiles(node: &NodeRef<'_, Flm>) {
        let Some(map) = node.ext().source_map.as_ref() else { return };
        assert_eq!(
            map.total_len() as usize,
            node.chars().expect("a text node").len(),
            "the pieces must tile the node's text: {:?}",
            map.pieces()
        );
    }

    /// A staging site for hand-built trees over one source: nodes are staged
    /// under the language's initial state (or a chosen one) with spans into
    /// the source, their extensions minted at the single minting point unless
    /// given — the way to build a deliberately broken or already-resolved tree.
    pub struct Site {
        pub builder: NodeTreeBuilder<Flm>,
        pub state: Arc<ParsingState<Flm>>,
        pub source: Arc<FlmSource>,
    }

    impl Site {
        pub fn new(lang: &BuiltLang, text: &str) -> Site {
            Site {
                builder: NodeTreeBuilder::new(),
                state: Arc::clone(lang.initial_state()),
                source: super::source(text),
            }
        }

        pub fn span(&self, range: Range<usize>) -> SourceSpan<FlmOrigin> {
            SourceSpan::new(&self.source, range)
        }

        /// Stages a node with its extension minted at the single minting point.
        pub fn stage(
            &mut self,
            kind: NodeKind<Flm>,
            range: Range<usize>,
            children: Vec<BuildId>,
        ) -> BuildId {
            let span = self.span(range);
            let state = Arc::clone(&self.state);
            let staged = self.builder.staged_children(&children);
            let ext = Flm::make_node_ext(&kind, &span, &state, staged).unwrap();
            self.builder
                .add(kind, span, state, children, ext, ())
                .unwrap()
        }

        /// Stages a node under an explicit state and extension.
        pub fn stage_with(
            &mut self,
            kind: NodeKind<Flm>,
            range: Range<usize>,
            state: Arc<ParsingState<Flm>>,
            ext: FlmNodeExt,
            children: Vec<BuildId>,
        ) -> BuildId {
            let span = self.span(range);
            self.builder
                .add(kind, span, state, children, ext, ())
                .unwrap()
        }

        /// A span-backed text node over `range`.
        pub fn text(&mut self, range: Range<usize>) -> BuildId {
            let span = self.span(range.clone());
            self.stage(NodeKind::chars(span.span()), range, Vec::new())
        }

        /// An already-resolved text node: owned `text` over `range`, carrying a
        /// source map whose pieces are `(output length, source range)`.
        pub fn mapped_text(
            &mut self,
            text: &str,
            range: Range<usize>,
            pieces: Vec<(u32, Range<usize>)>,
        ) -> BuildId {
            let pieces = pieces
                .into_iter()
                .map(|(out_len, range)| TextSourcePiece {
                    out_len,
                    span: self.span(range),
                })
                .collect();
            let map = Arc::new(TextSourceMap::new(pieces));
            let element = Arc::clone(&self.state.ext().structural.text);
            let ext = FlmNodeExt::new(element).with_source_map(map);
            let state = Arc::clone(&self.state);
            self.stage_with(NodeKind::chars(text), range, state, ext, Vec::new())
        }

        /// A paragraph group over `range` with the given children.
        pub fn paragraph(&mut self, range: Range<usize>, children: Vec<BuildId>) -> BuildId {
            let kind = NodeKind::group(GroupData::new(FlmGroupType::Paragraph, "", ""));
            self.stage(kind, range, children)
        }

        pub fn finish(self, root: BuildId) -> NodeTree<Flm> {
            self.builder.finish(root).unwrap()
        }
    }
}

// ============================================================================
// end blocks
// ============================================================================

// ---------------------------------------------------------------------------------
// user definitions: build a language over a few definitions, compile their
// templates, parse a document, run the expand pass, and read the result back.
// ---------------------------------------------------------------------------------

/// Helpers for the user-definition tests (`tests/userdefs_*.rs`).
pub mod userdefs {
    use std::sync::Arc;

    use flm_core::model::doc::Fragment;
    use flm_core::model::element::element_of;
    use flm_core::lang::guard::{self, GuardPolicy};
    use flm_core::lang::mint::ElementDef;
    use flm_core::lang::rewrites::userdefs::{
        expand, Definition, ExpandOptions, ExpandSet, TemplateLanguage, UserDefinition,
    };
    use flm_core::lang::{build_language, BuiltLang, Flm, FlmDiagnostic, FlmDiagnostics, LangInputs};
    use techy::core::node::NodeTree;
    use techy::latexlike::source_recomposer;

    /// The feature name the test definitions are declared under.
    pub const OWNER: &str = "u";

    /// A language built over user definitions (and any other elements), the
    /// definitions' element values with their templates compiled, and the
    /// diagnostics the compilations reported.
    pub struct Setup {
        /// The language.
        pub lang: BuiltLang,
        /// The definitions' element values, in declaration order.
        pub definitions: Vec<Arc<UserDefinition>>,
        /// Every diagnostic the compilations reported: the errors that refuse a
        /// definition and the warnings that ride along a compiled one.
        pub build_diagnostics: Vec<FlmDiagnostic>,
    }

    impl Setup {
        /// The element value of the definition named `name`, or a test failure.
        pub fn definition(&self, name: &str) -> &Arc<UserDefinition> {
            self.definitions
                .iter()
                .find(|definition| definition.definition().name == name)
                .unwrap_or_else(|| panic!("no definition named {name:?}"))
        }
    }

    /// Builds the language over `definitions` (declared by [`OWNER`], each element
    /// named `u.<name>`) beside `others`, and compiles every definition's
    /// templates over one template language, as a registry does. A language-build
    /// error is a test failure; a compilation diagnostic is not — it is kept in
    /// `build_diagnostics` for the test to inspect.
    pub fn setup(definitions: Vec<Definition>, others: Vec<ElementDef>) -> Setup {
        let mut inputs = LangInputs::new();
        inputs.elements = others;
        let mut values = Vec::with_capacity(definitions.len());
        for definition in definitions {
            let element_name = format!("{OWNER}.{}", definition.name);
            let value = UserDefinition::new(definition);
            if let Some(override_def) = value.override_def(OWNER) {
                inputs.overrides.push(override_def);
            }
            inputs.elements.push(value.element_def(OWNER, element_name));
            values.push(value);
        }
        let lang = build_language(inputs);
        assert!(lang.is_sound(), "the language build reported errors: {:?}", lang.errors());
        let language = TemplateLanguage::new(&lang)
            .unwrap_or_else(|errors| panic!("the template language did not mint: {errors:?}"));
        let mut build_diagnostics = Vec::new();
        for value in &values {
            match value.compile_with(&lang, &language) {
                Ok(warnings) => build_diagnostics.extend(warnings),
                Err(diagnostics) => build_diagnostics.extend(diagnostics),
            }
        }
        Setup { lang, definitions: values, build_diagnostics }
    }

    /// [`setup`] over definitions only.
    pub fn setup_definitions(definitions: Vec<Definition>) -> Setup {
        setup(definitions, Vec::new())
    }

    /// The options of a test run: the given depth limit, the language's guard.
    pub fn options(setup: &Setup, recursion_limit: usize) -> ExpandOptions {
        ExpandOptions { recursion_limit, guard: setup.lang.guard() }
    }

    /// Parses `text` and expands every definition with the given depth limit.
    /// Answers the expanded tree, the pass's diagnostics, and the parsed
    /// fragment (for its source and its parse diagnostics). A parse without a
    /// tree or a pass without a tree is a test failure.
    pub fn expanded(
        setup: &Setup,
        text: &str,
        recursion_limit: usize,
    ) -> (Arc<NodeTree<Flm>>, FlmDiagnostics, Fragment) {
        expanded_set(setup, text, recursion_limit, ExpandSet::Both)
    }

    /// [`expanded`] over the given expansion set.
    pub fn expanded_set(
        setup: &Setup,
        text: &str,
        recursion_limit: usize,
        set: ExpandSet,
    ) -> (Arc<NodeTree<Flm>>, FlmDiagnostics, Fragment) {
        let fragment = super::parse(&setup.lang, text);
        let mut diagnostics = FlmDiagnostics::new();
        let tree =
            expand(fragment.tree(), &setup.lang, &options(setup, recursion_limit), set, &mut diagnostics)
                .unwrap_or_else(|failure| {
                    panic!("the expansion of {text:?} yielded no tree: {failure}")
                });
        (tree, diagnostics, fragment)
    }

    /// The source text a tree re-emits (the preset's recomposition over the
    /// nodes' recorded payloads).
    ///
    /// The recomposition is a guarded descent under the stack budget, so it
    /// runs on a worker thread: in place when the caller is
    /// already on one, else on a thread spawned for it. Without that, a caller
    /// on a thread with a smaller stack than the budget — a test thread, or a
    /// thread a test spawned itself — overflows the stack instead of getting
    /// the guard's refusal.
    pub fn recomposed(tree: &NodeTree<Flm>) -> String {
        guard::on_worker_thread(|| {
            let mut recomposer = source_recomposer::<Flm>();
            guard::recomposer(&mut recomposer, GuardPolicy::StackBudget)
                .recompose(tree, ())
                .expect("recomposition of the expanded tree")
        })
        .expect("a worker thread for the recomposition")
    }

    /// The text of the root's children, `<non-chars>` for a child that is not a
    /// text run.
    pub fn chars_of(tree: &NodeTree<Flm>) -> Vec<String> {
        tree.root()
            .children()
            .iter()
            .map(|node| node.chars().unwrap_or("<non-chars>").to_string())
            .collect()
    }

    /// The element names of the root's children.
    pub fn element_names(tree: &NodeTree<Flm>) -> Vec<String> {
        tree.root().children().iter().map(|node| element_of(&node).name.to_string()).collect()
    }
}

// ============================================================================
// pipeline: helpers for `tests/pipeline_*.rs`
// ============================================================================

/// Helpers for the pipeline tests: toy tables, toy stages and rewrites built
/// from closures, toy features, registries and documents built in one call,
/// runs, a store snapshot for bit-identity checks, and a byte-exact golden
/// check over `tests/goldens/pipeline/`.
pub mod pipeline {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use techy::core::node::{NodeRef, NodeTree};
    use techy::error::DiagnosticInfo;
    use techy::serialize::SerialValue;
    use techy::source::SourceSpan;

    use flm_core::model::addr::{NodeAddr, TreeId};
    use flm_core::model::doc::{Doc, DocContent};
    use flm_core::core::lowering::{
        DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    };
    use flm_core::model::options::OptionValues;
    use flm_core::core::plan::{Plan, PlanCx, PlanRequest};
    use flm_core::core::registry::{
        BackendDef, BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature,
        FlmRegistry, RegistryBuildError, build_registry,
    };
    use flm_core::core::run::{Externals, RewriteCx, Run, RunOptions, StageCx};
    use flm_core::core::stage::{
        Effect, Needs, RewriteDecl, RewriteError, RewriteStage, Scope, Stage, StageDecl,
    };
    use flm_core::core::data::store::StageOut;
    use flm_core::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
    use flm_core::lang::{Flm, FlmDiagnostics, FlmSpan, ParseOptions, parse_fragment};

    // --- toy tables ---------------------------------------------------------------------

    /// A value-shaped table: a sum. A table need not be node-keyed.
    #[derive(Debug, Default)]
    pub struct Count(pub u32);

    impl Table for Count {
        fn absorb(&mut self, piece: Count) {
            self.0 += piece.0;
        }

        fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
            SerialValue::Int(i64::from(self.0))
        }
    }

    impl SingletonTable for Count {
        const NAME: &'static str = "t.count";
    }

    /// The count table's name.
    pub const COUNT: TableName<Count> = TableName::new("t.count");

    /// An append-only log: absorption order is entry order, so the log shows
    /// the order pieces were folded in.
    #[derive(Debug, Default)]
    pub struct Log(pub Vec<String>);

    impl Table for Log {
        fn absorb(&mut self, piece: Log) {
            self.0.extend(piece.0);
        }

        fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
            SerialValue::List(self.0.iter().cloned().map(SerialValue::Str).collect())
        }
    }

    /// The log table's name.
    pub const LOG: TableName<Log> = TableName::new("t.log");

    /// A node-keyed table: node address → the node's text.
    #[derive(Debug, Default)]
    pub struct Marks(pub BTreeMap<NodeAddr, String>);

    impl Table for Marks {
        fn absorb(&mut self, piece: Marks) {
            self.0.extend(piece.0);
        }

        fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
            SerialValue::List(
                self.0
                    .iter()
                    .map(|(addr, text)| {
                        SerialValue::List(vec![cx.node_ref(*addr), SerialValue::Str(text.clone())])
                    })
                    .collect(),
            )
        }
    }

    /// The marks table's name.
    pub const MARKS: TableName<Marks> = TableName::new("t.marks");

    /// A table whose non-empty pieces state a document fact — what value
    /// catch-up has to reproduce for a synthesized tree.
    #[derive(Debug, Default)]
    pub struct Fact(pub u32);

    impl Table for Fact {
        fn absorb(&mut self, piece: Fact) {
            self.0 += piece.0;
        }

        fn piece_class(&self) -> PieceClass {
            if self.0 == 0 { PieceClass::Empty } else { PieceClass::DocumentFact }
        }
    }

    /// The fact table's name.
    pub const FACT: TableName<Fact> = TableName::new("t.fact");

    // --- diagnostics ----------------------------------------------------------------------

    /// The toy stages' one condition.
    #[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
    #[diagnostic(id = "t.note", message = "{detail}")]
    pub struct Note {
        /// Free text.
        pub detail: String,
    }

    /// A condition with its own identifier, for tests telling two apart.
    #[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
    #[diagnostic(id = "t.other", message = "{detail}")]
    pub struct Other {
        /// Free text.
        pub detail: String,
    }

    /// An empty span over an empty source.
    pub fn span() -> FlmSpan {
        SourceSpan::new(&super::source(""), 0..0)
    }

    // --- observation ------------------------------------------------------------------------

    /// What a toy stage observed, shared with the test.
    pub type Probe = Arc<Mutex<Vec<String>>>;

    /// A fresh probe.
    pub fn probe() -> Probe {
        Arc::new(Mutex::new(Vec::new()))
    }

    /// Records one observation.
    pub fn record(probe: &Probe, text: impl Into<String>) {
        probe.lock().unwrap().push(text.into());
    }

    /// Everything recorded so far.
    pub fn recorded(probe: &Probe) -> Vec<String> {
        probe.lock().unwrap().clone()
    }

    // --- toy stages and rewrites ------------------------------------------------------------

    type RunFn = dyn Fn(&StageCx<'_>, &mut StageOut<'_>) + Send + Sync;

    /// A stage whose declaration, needs and body the test supplies.
    pub struct ToyStage {
        decl: StageDecl,
        needs: Needs,
        run: Box<RunFn>,
    }

    impl Stage for ToyStage {
        fn decl(&self) -> StageDecl {
            self.decl.clone()
        }

        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            self.needs.clone()
        }

        fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
            (self.run)(cx, out)
        }
    }

    /// A stage from its parts.
    pub fn stage(
        decl: StageDecl,
        needs: Needs,
        run: impl Fn(&StageCx<'_>, &mut StageOut<'_>) + Send + Sync + 'static,
    ) -> Arc<dyn Stage> {
        Arc::new(ToyStage { decl, needs, run: Box::new(run) })
    }

    /// A stage that does nothing when run.
    pub fn noop(decl: StageDecl, needs: Needs) -> Arc<dyn Stage> {
        stage(decl, needs, |_, _| {})
    }

    /// A `Compute` `Document` stage `name` providing `provides` and requiring
    /// `requires`, doing nothing: the selection tests' shape.
    pub fn provider(name: &str, provides: &[&str], requires: &[&str]) -> Arc<dyn Stage> {
        let mut decl = StageDecl::new(name, Effect::Compute, Scope::Document);
        for capability in provides {
            decl = decl.provides(*capability);
        }
        let mut needs = Needs::new();
        for capability in requires {
            needs = needs.requires(*capability);
        }
        noop(decl, needs)
    }

    type RewriteFn = dyn Fn(
            TreeId,
            &Arc<NodeTree<Flm>>,
            &RewriteCx<'_>,
            &mut FlmDiagnostics,
        ) -> Result<Arc<NodeTree<Flm>>, RewriteError>
        + Send
        + Sync;

    /// A rewrite whose declaration, needs and body the test supplies.
    pub struct ToyRewrite {
        decl: RewriteDecl,
        needs: Needs,
        rewrite: Box<RewriteFn>,
    }

    impl RewriteStage for ToyRewrite {
        fn decl(&self) -> RewriteDecl {
            self.decl.clone()
        }

        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            self.needs.clone()
        }

        fn rewrite(
            &self,
            tree_id: TreeId,
            tree: &Arc<NodeTree<Flm>>,
            cx: &RewriteCx<'_>,
            diags: &mut FlmDiagnostics,
        ) -> Result<Arc<NodeTree<Flm>>, RewriteError> {
            (self.rewrite)(tree_id, tree, cx, diags)
        }
    }

    /// A rewrite from its parts.
    pub fn rewrite(
        decl: RewriteDecl,
        needs: Needs,
        rewrite: impl Fn(
            TreeId,
            &Arc<NodeTree<Flm>>,
            &RewriteCx<'_>,
            &mut FlmDiagnostics,
        ) -> Result<Arc<NodeTree<Flm>>, RewriteError>
        + Send
        + Sync
        + 'static,
    ) -> Arc<dyn RewriteStage> {
        Arc::new(ToyRewrite { decl, needs, rewrite: Box::new(rewrite) })
    }

    /// A rewrite `name` publishing `publishes` that re-parses each tree's text
    /// with `suffix` appended — a visibly different tree.
    pub fn appending(name: &str, publishes: &str, suffix: &'static str) -> Arc<dyn RewriteStage> {
        rewrite(RewriteDecl::new(name, publishes), Needs::new(), move |_, tree, cx, _| {
            Ok(reparse(cx.registry(), &format!("{}{suffix}", text_of(tree)), cx))
        })
    }

    /// The text of a tree: its chars nodes, concatenated.
    pub fn text_of(tree: &NodeTree<Flm>) -> String {
        tree.root().descendants().filter_map(|node| node.chars()).collect()
    }

    /// A tree parsed from `text` under the registry's language, with the
    /// rewrite context's layout.
    pub fn reparse(registry: &FlmRegistry, text: &str, cx: &RewriteCx<'_>) -> Arc<NodeTree<Flm>> {
        let fragment = parse_fragment(
            registry.lang(),
            super::source(text),
            cx.layout(),
            &ParseOptions::default(),
        )
        .unwrap_or_else(|error| panic!("the re-parse of {text:?} yielded no tree: {error}"));
        Arc::clone(fragment.tree())
    }

    /// A tree parsed from `text` under the registry's language and the default
    /// layout.
    pub fn tree(registry: &FlmRegistry, text: &str) -> Arc<NodeTree<Flm>> {
        Arc::clone(super::parse(registry.lang(), text).tree())
    }

    // --- lowering rules and backends ---------------------------------------------------------

    /// A lowering rule implementation that always answers `Default` — enough
    /// for the registry and plan tests, which never lower a tree.
    pub struct NoLower;

    impl Lower for NoLower {
        fn lower(
            &self,
            _node: NodeRef<'_, Flm>,
            _cx: &mut LowerCx<'_>,
            _lw: &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError> {
            Ok(Lowering::Default)
        }
    }

    /// A rule `name` lowering `element` with the given declaration and default
    /// mark, over [`NoLower`].
    pub fn rule(
        name: &str,
        element: &str,
        decl: LoweringDecl,
        default: DefaultMark,
    ) -> LoweringRule {
        LoweringRule::new(name, element, decl, Arc::new(NoLower), default)
    }

    /// The capability a toy backend's lowering stage provides.
    pub fn lowered_cap(backend: &str) -> String {
        format!("t.lowered.{backend}")
    }

    /// The capability a toy backend's emit stage provides.
    pub fn output_cap(backend: &str) -> String {
        format!("t.output.{backend}")
    }

    /// A toy lowering stage: `t.<backend>.lower`, `PerTree`, providing
    /// `t.lowered.<backend>`, its needs the union of the selected rules'
    /// `requires` — the shape the real lowering stage takes, its manifest
    /// assembled at plan build over the selected rules.
    pub struct ToyLowering {
        backend: String,
    }

    impl Stage for ToyLowering {
        fn decl(&self) -> StageDecl {
            StageDecl::new(format!("t.{}.lower", self.backend), Effect::Compute, Scope::PerTree)
                .provides(lowered_cap(&self.backend))
        }

        fn needs(&self, cx: &PlanCx<'_>) -> Needs {
            let mut needs = Needs::new();
            if let Some(selected) = cx.lowering_selection(&self.backend) {
                for idx in selected.rules() {
                    for capability in &cx.registry().rule(idx).rule.decl.requires {
                        needs = needs.requires(capability.clone());
                    }
                }
            }
            needs
        }

        fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
    }

    /// A backend `name` whose native set is the structural elements plus
    /// `extra`: the toy lowering stage, then a no-op `Document` emit stage
    /// `t.<name>.emit` (requires the lowered capability, provides
    /// `t.output.<name>`) and final stage `t.<name>.final` (requires the
    /// output capability) — the real backend's chain, with capabilities where
    /// the real one has tables. The registering feature declares the two
    /// capabilities itself — see [`register_backend`].
    pub fn backend(name: &str, extra: &[&str]) -> BackendDef {
        let native: Vec<String> = flm_core::lang::structural::NAMES
            .iter()
            .chain(extra.iter())
            .map(|element| element.to_string())
            .collect();
        let emit = noop(
            StageDecl::new(format!("t.{name}.emit"), Effect::Compute, Scope::Document)
                .provides(output_cap(name)),
            Needs::new().requires(lowered_cap(name)),
        );
        let final_stage = noop(
            StageDecl::new(format!("t.{name}.final"), Effect::Compute, Scope::Document),
            Needs::new().requires(output_cap(name)),
        );
        BackendDef::new(
            name,
            native,
            Arc::new(ToyLowering { backend: name.to_string() }),
            emit,
            final_stage,
        )
    }

    /// Declares the lowered and output capabilities of a toy backend and
    /// registers it.
    pub fn register_backend(
        b: &mut FeatureBuilder<'_>,
        name: &str,
        extra: &[&str],
    ) -> Result<(), BuildError> {
        b.capability(CapabilityDef::new(lowered_cap(name)))?;
        b.capability(CapabilityDef::new(output_cap(name)))?;
        b.backend(backend(name, extra))
    }

    // --- toy features and registries ---------------------------------------------------------

    type RegisterFn = dyn Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync;

    /// A feature whose registration the test supplies.
    pub struct ToyFeature {
        info: FeatureInfo,
        register: Box<RegisterFn>,
    }

    impl FlmFeature for ToyFeature {
        fn info(&self) -> FeatureInfo {
            self.info.clone()
        }

        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            (self.register)(b)
        }
    }

    /// A feature `name` registering through `register`.
    pub fn feature<R>(name: &str, register: R) -> Arc<dyn FlmFeature>
    where
        R: Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync + 'static,
    {
        Arc::new(ToyFeature { info: FeatureInfo::new(name, name), register: Box::new(register) })
    }

    /// The registry over `features`; a build error is a test failure.
    pub fn registry(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
        build_registry(features).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    /// The failure of the registry build over `features`; a successful build
    /// is a test failure.
    pub fn registry_error(features: Vec<Arc<dyn FlmFeature>>) -> RegistryBuildError {
        match build_registry(features) {
            Ok(_) => panic!("the registry build was expected to fail"),
            Err(error) => error,
        }
    }

    /// One capability declaration for [`toy_registry`]: name, parent, default.
    pub type ToyCap = (&'static str, Option<&'static str>, bool);

    /// One stage declaration for [`toy_registry`]: name, provides, requires.
    pub type ToyProvider = (&'static str, Vec<&'static str>, Vec<&'static str>);

    /// A registry of one feature `t` declaring plain capabilities and no-op
    /// `Document` providers — the selection tests' fixture shape.
    pub fn toy_registry(caps: &[ToyCap], stages: &[ToyProvider]) -> Arc<FlmRegistry> {
        let caps: Vec<ToyCap> = caps.to_vec();
        let stages: Vec<ToyProvider> = stages.to_vec();
        registry(vec![feature("t", move |b| {
            for (name, parent, default) in &caps {
                let mut def = CapabilityDef::new(*name);
                if let Some(parent) = parent {
                    def = def.refines(*parent);
                }
                if *default {
                    def = def.default();
                }
                b.capability(def)?;
            }
            for (name, provides, requires) in &stages {
                b.stage(provider(name, provides, requires))?;
            }
            Ok(())
        })])
    }

    // --- documents, plans, runs -----------------------------------------------------------------

    /// A document of one tree per text, in order, parsed under the registry's
    /// language.
    pub fn doc(registry: &FlmRegistry, texts: &[&str]) -> Arc<Doc> {
        doc_with_options(registry, texts, OptionValues::new())
    }

    /// [`doc`] with document-bound option values.
    pub fn doc_with_options(
        registry: &FlmRegistry,
        texts: &[&str],
        options: OptionValues,
    ) -> Arc<Doc> {
        let mut content = DocContent::new();
        for text in texts {
            content.place(Arc::new(super::parse(registry.lang(), text)));
        }
        Arc::new(Doc::with_options(content, options))
    }

    /// The plan demanding `demands`; a plan error is a test failure.
    pub fn plan(registry: &Arc<FlmRegistry>, demands: &[&str]) -> Arc<Plan> {
        let mut request = PlanRequest::new();
        for demand in demands {
            request = request.demand(*demand);
        }
        plan_for(registry, &request)
    }

    /// The plan of `request`; a plan error is a test failure.
    pub fn plan_for(registry: &Arc<FlmRegistry>, request: &PlanRequest) -> Arc<Plan> {
        Arc::new(
            Plan::build(registry, request)
                .unwrap_or_else(|error| panic!("the plan failed: {error}")),
        )
    }

    /// Runs `plan` over `doc` to the end with no externals; a run error is a
    /// test failure.
    pub fn run(doc: &Arc<Doc>, plan: &Arc<Plan>, options: &RunOptions) -> Run {
        run_with(doc, plan, Externals::none(), options)
    }

    /// [`run`] with externals installed.
    pub fn run_with(
        doc: &Arc<Doc>,
        plan: &Arc<Plan>,
        externals: Externals,
        options: &RunOptions,
    ) -> Run {
        let mut run = Run::new(Arc::clone(doc), Arc::clone(plan), externals)
            .unwrap_or_else(|error| panic!("the run could not be constructed: {error}"));
        run.execute(options).unwrap_or_else(|error| panic!("the run failed: {error}"));
        run
    }

    struct Refs;

    impl ProjectCx for Refs {
        fn node_ref(&self, addr: NodeAddr) -> SerialValue {
            SerialValue::Str(format!("{addr:?}"))
        }

        fn tree_ref(&self, id: TreeId) -> SerialValue {
            SerialValue::Str(id.to_string())
        }
    }

    /// A stable rendering of everything a run produced: every table of the
    /// store in `TableId` order through its projection, then every diagnostic
    /// of the chained view by severity, identifier and span.
    pub fn snapshot(run: &Run) -> String {
        let registry = run.plan().registry();
        let mut out = String::new();
        for id in run.computed().ids() {
            let entry = registry.table(id);
            let value = run.computed().get_erased(id).expect("a listed table is present");
            let projected = entry.def.ty.project(value.as_ref(), &Refs);
            out.push_str(&format!("{} = {projected:?}\n", entry.def.name));
        }
        for diagnostic in run.diagnostics().iter() {
            out.push_str(&format!(
                "{:?} {} {:?}\n",
                diagnostic.severity(),
                diagnostic.identifier(),
                diagnostic.span().range()
            ));
        }
        out
    }

    // --- goldens --------------------------------------------------------------------------------

    /// The path of the golden `tests/goldens/pipeline/<name>`, for
    /// `flm_core::dump::golden::assert_text_golden` (`UPDATE_GOLDEN=1 cargo test`,
    /// exactly `"1"`, rewrites a golden instead of comparing).
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("pipeline");
        p.push(name);
        p
    }
}

// ============================================================================
// end pipeline
// ============================================================================

// ============================================================================
// wiring: helpers for `tests/wiring_*.rs`
// ============================================================================

/// Helpers for the wiring tests: a registry over the `blocks` and `userdefs`
/// features, documents parsed under it with a chosen root layout, runs, and
/// probes over a run's frozen trees.
pub mod wiring {
    use std::sync::Arc;

    use flm_core::model::addr::TreeId;
    use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
    use flm_core::model::element::element_of;
    use flm_core::core::registry::{FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
    use flm_core::core::run::{Run, RunOptions};
    use flm_core::lang::rewrites::userdefs::Definition;
    use flm_core::lang::{Flm, GuardPolicy, ParseOptions, parse_fragment};
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};
    use techy::core::node::{NodeRef, NodeTree};

    /// The registry over the `blocks` feature (the default text options) and
    /// the `userdefs` feature over `definitions` (the default depth limit).
    pub fn registry(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
        registry_with(Blocks::default(), UserdefsConfig::new().definitions(definitions))
    }

    /// The registry over the given `blocks` feature and `userdefs`
    /// configuration, the `blocks` feature registered first.
    pub fn registry_with(blocks: Blocks, config: UserdefsConfig) -> Arc<FlmRegistry> {
        let features: Vec<Arc<dyn FlmFeature>> =
            vec![Arc::new(blocks), Arc::new(Userdefs::new(config))];
        super::pipeline::registry(features)
    }

    /// The registry over `features` built with the given guard policy for
    /// every parse and traversal of its language — the policy the wired
    /// stages take from the built language.
    pub fn registry_with_guard(
        features: Vec<Arc<dyn FlmFeature>>,
        guard: GuardPolicy,
    ) -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        for feature in features {
            builder.feature(feature);
        }
        let inputs = RegistryInputs { guard, ..RegistryInputs::new() };
        builder.build(&inputs).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    /// A document of one tree per text, in order, each parsed under the
    /// registry's language with the given root layout.
    pub fn doc(registry: &FlmRegistry, layout: FragmentLayout, texts: &[&str]) -> Arc<Doc> {
        let mut content = DocContent::new();
        for text in texts {
            let fragment = parse_fragment(
                registry.lang(),
                super::source(text),
                layout,
                &ParseOptions::default(),
            )
            .unwrap_or_else(|error| panic!("the parse of {text:?} yielded no tree: {error}"));
            content.place(Arc::new(fragment));
        }
        Arc::new(Doc::new(content))
    }

    /// Runs the plan demanding `demands` over a one-tree document of `text`
    /// laid out as blocks, to the end, under the default run options.
    pub fn run_blocks(registry: &Arc<FlmRegistry>, text: &str, demands: &[&str]) -> Run {
        let doc = doc(registry, FragmentLayout::Blocks, &[text]);
        super::pipeline::run(&doc, &super::pipeline::plan(registry, demands), &RunOptions::new())
    }

    /// The frozen tree of the first document tree; a test failure when absent.
    pub fn frozen(run: &Run) -> &Arc<NodeTree<Flm>> {
        run.frozen_tree(TreeId::tree(0)).expect("the first tree is frozen")
    }

    /// The element names of a node's children, in order.
    pub fn child_elements(node: &NodeRef<'_, Flm>) -> Vec<String> {
        node.children().iter().map(|child| element_of(&child).name.to_string()).collect()
    }

    /// The text at and under a node: its own characters, then every text
    /// descendant's, concatenated in document order.
    pub fn text_under(node: &NodeRef<'_, Flm>) -> String {
        let own = node.chars().unwrap_or("");
        let rest: String = node.descendants().filter_map(|child| child.chars()).collect();
        format!("{own}{rest}")
    }

    /// The text of each paragraph at a tree's root, in order; a root child
    /// that is not a paragraph is a test failure.
    pub fn paragraphs(tree: &NodeTree<Flm>) -> Vec<String> {
        tree.root()
            .children()
            .iter()
            .map(|child| {
                let element = &element_of(&child).name;
                assert_eq!(element, "flm.paragraph", "a root child is not a paragraph");
                text_under(&child)
            })
            .collect()
    }

    /// How many nodes of a tree carry the element named `name`.
    pub fn count_element(tree: &NodeTree<Flm>, name: &str) -> usize {
        let root = tree.root();
        let at_root = usize::from(element_of(&root).name == name);
        at_root + root.descendants().filter(|node| element_of(node).name == name).count()
    }
}

// ============================================================================
// end wiring
// ============================================================================

// ============================================================================
// lowering and the text backend: helpers for `tests/lowering_*.rs`
// and `tests/text_*.rs`
// ============================================================================

/// Helpers for the lowering and text-backend tests: registries holding the
/// `lowering` and `text` features, the wired `blocks`, `userdefs` (no
/// definitions) and `math` features (the real math fallback and its rule,
/// which replaced this section's stand-in at integration), and a toy
/// feature, plans and runs of the `text` workflow, readers over the lowered
/// trees and the text output, toy lowering rules from closures, hand-built
/// presentation trees, the emitter run over them, and a golden path under
/// `tests/goldens/text/`.
pub mod lowering {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use std::sync::Arc;

    use flm_core::model::addr::TreeId;
    use flm_core::core::lowering::{
        DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
    };
    use flm_core::model::names::{BackendName, ElementName};
    use flm_core::core::plan::{Overrides, Plan};
    use flm_core::core::registry::{BuildError, FeatureBuilder, FlmFeature, FlmRegistry};
    use flm_core::core::run::{Run, RunOptions};
    use flm_core::dump::project::{ProjectOptions, project_tree};
    use flm_core::lang::mint::{ElementDef, FlmPresentationSpec};
    use flm_core::lang::{Flm, FlmDiagnostic, FlmGroupType, FlmOrigin, FlmSource, PresPayload};
    use flm_core::stdcore::backends::lowering::{self, LoweredTree, LoweredTrees, lowered_table};
    use flm_core::stdcore::backends::pres::{self, CellsPayload, ListPayload};
    use flm_core::stdcore::backends::pieces::{self, LoweredPieces};
    use flm_core::stdcore::backends::text::{
        self, Emitted, TEXT_OUTPUT, TextOutput, TextRenderOptions, emit,
    };
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::math;
    use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};
    use techy::core::node::{
        BodySlotExt, BuildId, ChildRegion, ContentNodes, GroupData, NodeKind, NodeRef, NodeTree,
        NodeTreeBuilder, ParsedArguments, ParsedSlot, ParsedSlots, SlotRole,
    };
    use techy::core::{Lang, ParsingState};
    use techy::latexlike::BodyMarker;
    use techy::serialize::SerialValue;
    use techy::source::SourceSpan;

    use super::pipeline;

    /// The toy feature's name.
    pub const OWNER: &str = "t";

    // --- registration pieces -----------------------------------------------------------

    /// A registry over the `lowering` and `text` features, the wired `blocks`
    /// feature (the default text options), `userdefs` feature (no
    /// definitions), `math` feature (the display-math environments) and
    /// `math-passthrough` feature (the math-mode fallback
    /// `flm.math-passthrough.opaque` with its rule, the rules of the formulas) — so
    /// the paragraph rule's requirement schedules the real structure stage
    /// behind the real expansion stage, and math lowers through the real
    /// feature — and a toy feature `t` declaring `elements` and `rules`.
    pub fn registry(elements: Vec<ElementDef>, rules: Vec<LoweringRule>) -> Arc<FlmRegistry> {
        registry_with(elements, rules, |_| Ok(()))
    }

    /// [`registry`] with further registrations on the toy feature.
    pub fn registry_with(
        elements: Vec<ElementDef>,
        rules: Vec<LoweringRule>,
        extra: impl Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync + 'static,
    ) -> Arc<FlmRegistry> {
        let features: Vec<Arc<dyn FlmFeature>> = vec![
            lowering::feature(),
            pieces::feature(),
            text::feature(),
            flm_core::stdcore::check_feature(),
            Arc::new(Blocks::default()),
            Arc::new(Userdefs::new(UserdefsConfig::new())),
            math::feature(),
            flm_core::math_passthrough::feature(),
            pipeline::feature(OWNER, move |b| {
                for element in &elements {
                    b.element(element.clone())?;
                }
                for rule in &rules {
                    b.lowering_rule(rule.clone())?;
                }
                extra(b)
            }),
        ];
        pipeline::registry(features)
    }

    // --- plans and runs ----------------------------------------------------------------

    /// The plan of the `text` workflow; a plan error is a test failure.
    pub fn text_plan(registry: &Arc<FlmRegistry>) -> Arc<Plan> {
        text_plan_with(registry, &Overrides::none())
    }

    /// The plan of the `text` workflow under `overrides`.
    pub fn text_plan_with(registry: &Arc<FlmRegistry>, overrides: &Overrides) -> Arc<Plan> {
        Arc::new(
            Plan::for_workflow(registry, text::WORKFLOW, overrides)
                .unwrap_or_else(|error| panic!("the text plan failed: {error}")),
        )
    }

    /// Renders one document of one tree per text through the `text` workflow,
    /// sequentially.
    pub fn render(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
        render_with(registry, texts, &Overrides::none())
    }

    /// [`render`] under overrides.
    pub fn render_with(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
        let doc = pipeline::doc(registry, texts);
        let plan = text_plan_with(registry, overrides);
        pipeline::run(&doc, &plan, &RunOptions::sequential())
    }

    /// The text backend's name.
    pub fn backend() -> BackendName {
        BackendName::from(text::BACKEND)
    }

    /// The lowered form of a document tree, if the run lowered it.
    pub fn try_lowered(run: &Run, id: TreeId) -> Option<&LoweredTree> {
        run.table(lowered_table(&backend())).expect("the lowered table is declared")?.get(id)
    }

    /// The lowered form of a document tree; its absence is a test failure that
    /// prints the run's diagnostics.
    pub fn lowered(run: &Run, id: TreeId) -> &LoweredTree {
        try_lowered(run, id).unwrap_or_else(|| {
            panic!("{id} was not lowered; diagnostics:\n{}", run.diagnostics().render_all())
        })
    }

    /// The projection of a lowered tree (payloads shown, no spans).
    pub fn projection(run: &Run, id: TreeId) -> SerialValue {
        project_tree(&lowered(run, id).tree, &ProjectOptions::new())
    }

    /// The projection of the first tree's lowered form.
    pub fn lowered_projection(run: &Run) -> SerialValue {
        projection(run, TreeId::tree(0))
    }

    /// The text output; its absence is a test failure.
    pub fn output(run: &Run) -> &TextOutput {
        run.table(TEXT_OUTPUT).expect("the text output table is declared").unwrap_or_else(|| {
            panic!("no text output; diagnostics:\n{}", run.diagnostics().render_all())
        })
    }

    /// The printed text.
    pub fn text(run: &Run) -> String {
        output(run).text.clone()
    }

    /// Every diagnostic of the run, in the chained view's order.
    pub fn diagnostics(run: &Run) -> Vec<&FlmDiagnostic> {
        run.diagnostics().iter().collect()
    }

    // --- toy rules ---------------------------------------------------------------------

    type RuleFn = dyn Fn(
            NodeRef<'_, Flm>,
            &mut LowerCx<'_>,
            &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError>
        + Send
        + Sync;

    /// A rule implementation from a closure.
    pub struct ToyRule(Box<RuleFn>);

    impl Lower for ToyRule {
        fn lower(
            &self,
            node: NodeRef<'_, Flm>,
            cx: &mut LowerCx<'_>,
            lw: &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError> {
            (self.0)(node, cx, lw)
        }
    }

    /// A rule `name` lowering `element` through `body`, with the given
    /// declaration and default mark.
    pub fn rule(
        name: &str,
        element: &str,
        decl: LoweringDecl,
        default: DefaultMark,
        body: impl Fn(
            NodeRef<'_, Flm>,
            &mut LowerCx<'_>,
            &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError>
        + Send
        + Sync
        + 'static,
    ) -> LoweringRule {
        LoweringRule::new(name, element, decl, Arc::new(ToyRule(Box::new(body))), default)
    }

    /// A declaration producing the given elements.
    pub fn produces(elements: &[&str]) -> LoweringDecl {
        let mut decl = LoweringDecl::new();
        for element in elements {
            decl = decl.produces(*element);
        }
        decl
    }

    /// An element name.
    pub fn name(element: &str) -> ElementName {
        ElementName::from(element)
    }

    // --- a toy document language with rules -----------------------------------------------

    /// The toy declarations of the structural and end-to-end tests, one
    /// element per shape the default rules and the text emitter care about:
    ///
    /// - `\section{title}`: a block macro, `t.section`;
    /// - `\runin{…}`, `\qed`, `\marginnote{…}`: a `Leads`, a `Trails`, a
    ///   `Free` decorator (`t.runin`, `t.qed`, `t.marginnote`);
    /// - `\emph{…}`: inline content, `t.emph`;
    /// - `\verb|…|`: a verbatim argument, `t.verb`; `\begin{code}…\end{code}`:
    ///   a block environment with a verbatim body, `t.code`;
    /// - `\begin{quote}…\end{quote}`: a block environment with a blocks body,
    ///   `t.quote`.
    ///
    /// Display math (`\begin{equation}…\end{equation}`) is the real math
    /// feature's, listed by [`registry`].
    pub fn toy_elements() -> Vec<ElementDef> {
        use flm_core::model::aspects::{BlockRole, Placement};
        use flm_core::lang::mint::{ArgDef, ArgStdKind, BodyDef, SyntaxDef};

        use super::blocks::{element_with_role, env_of, macro_of};

        vec![
            macro_of("section", 1, element_with_role("t.section", BlockRole::Block)),
            macro_of(
                "runin",
                1,
                element_with_role("t.runin", BlockRole::BlockDecorator(Placement::Leads)),
            ),
            macro_of(
                "qed",
                0,
                element_with_role("t.qed", BlockRole::BlockDecorator(Placement::Trails)),
            ),
            macro_of(
                "marginnote",
                1,
                element_with_role("t.marginnote", BlockRole::BlockDecorator(Placement::Free)),
            ),
            macro_of("emph", 1, super::element("t.emph")),
            ElementDef::new(
                OWNER,
                ["verb"],
                SyntaxDef::Macro {
                    args: vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })],
                },
                super::element("t.verb"),
            ),
            env_of(
                "code",
                BodyDef::new().verbatim(),
                element_with_role("t.code", BlockRole::Block),
            ),
            env_of(
                "quote",
                BodyDef::new().blocks(),
                element_with_role("t.quote", BlockRole::Block),
            ),
        ]
    }

    /// The toy rules over [`toy_elements`]: a heading of level 1, the three
    /// decorations, an emphasis span, the verbatim argument staged as an
    /// inline `verbatim` by its own rule and the verbatim body lowered to its
    /// content (the default verbatim rule then takes the group), and a block
    /// over the quotation's structured body.
    pub fn toy_rules() -> Vec<LoweringRule> {
        use flm_core::stdcore::backends::lowering::copy_run_verbatim;
        use flm_core::stdcore::backends::pres::{
            HeadingPayload, BlockPayload, DecorationPayload, SpanPayload, SpanStyle,
            VerbatimDisplay, VerbatimKind, VerbatimPayload,
        };

        fn deco(rule_name: &str, element: &str, target: &'static str) -> LoweringRule {
            rule(
                rule_name,
                element,
                produces(&[target]),
                DefaultMark::Everywhere,
                move |node, cx, lw| {
                    let body = if node.argument_content_nodes(0).is_some() {
                        lw.lower_arg(cx, &node, 0)?
                    } else {
                        vec![lw.text(cx, "□")?]
                    };
                    Ok(Lowering::Emit(vec![lw.pres_with(
                        cx,
                        &name(target),
                        DecorationPayload,
                        body,
                    )?]))
                },
            )
        }

        vec![
            rule(
                "t.section.lower.default",
                "t.section",
                produces(&[pres::BLOCK_HEADING]),
                DefaultMark::Everywhere,
                |node, cx, lw| {
                    let title = lw.lower_arg(cx, &node, 0)?;
                    let payload = HeadingPayload {
                        name: flm_core::model::names::Name::from("t.section"),
                        level: 1,
                        in_toc: true,
                        anchor: None,
                        number: None,
                    };
                    Ok(Lowering::Emit(vec![lw.pres_with(
                        cx,
                        &name(pres::BLOCK_HEADING),
                        payload,
                        title,
                    )?]))
                },
            ),
            deco("t.runin.lower.default", "t.runin", pres::DECO_RUNIN),
            deco("t.qed.lower.default", "t.qed", pres::DECO_FINAL),
            deco("t.marginnote.lower.default", "t.marginnote", pres::DECO_MARGIN),
            rule(
                "t.emph.lower.default",
                "t.emph",
                produces(&[pres::SPAN]),
                DefaultMark::Everywhere,
                |node, cx, lw| {
                    let body = lw.lower_arg(cx, &node, 0)?;
                    let payload =
                        SpanPayload { style: Some(SpanStyle::Emphasis), ..SpanPayload::default() };
                    Ok(Lowering::Emit(vec![lw.pres_with(cx, &name(pres::SPAN), payload, body)?]))
                },
            ),
            rule(
                "t.verb.lower.default",
                "t.verb",
                produces(&[pres::VERBATIM]),
                DefaultMark::Everywhere,
                |node, cx, lw| {
                    // A verbatim argument's content nodes are the raw text
                    // inside the argument's verbatim group: the element's rule
                    // builds the presentation node itself.
                    let content = node
                        .argument_content_nodes(0)
                        .ok_or_else(|| LowerError::structure("no verbatim argument"))?;
                    let body = copy_run_verbatim(cx, content)?;
                    let payload = VerbatimPayload {
                        kind: VerbatimKind::Text,
                        display: VerbatimDisplay::Inline,
                        lang: None,
                    };
                    Ok(Lowering::Emit(vec![lw.pres_with(
                        cx,
                        &name(pres::VERBATIM),
                        payload,
                        body,
                    )?]))
                },
            ),
            rule(
                "t.code.lower.default",
                "t.code",
                produces(&[]),
                DefaultMark::Everywhere,
                |node, cx, lw| Ok(Lowering::Emit(lw.lower_body(cx, &node)?)),
            ),
            rule(
                "t.quote.lower.default",
                "t.quote",
                produces(&[pres::BLOCK]),
                DefaultMark::Everywhere,
                |node, cx, lw| {
                    let body = lw.lower_body(cx, &node)?;
                    let payload =
                        BlockPayload { role: Some("t.quote".into()), ..BlockPayload::default() };
                    Ok(Lowering::Emit(vec![lw.pres_with(cx, &name(pres::BLOCK), payload, body)?]))
                },
            ),
        ]
    }

    /// The registry over the toy language and its rules.
    pub fn toy_registry() -> Arc<FlmRegistry> {
        registry(toy_elements(), toy_rules())
    }

    // --- hand-built presentation trees ---------------------------------------------------

    /// A staging site for presentation trees built by hand, the way the
    /// emitter tests feed the emitter without a lowering: nodes are staged
    /// under the registry language's initial state on one empty synthetic
    /// source, presentation nodes with the vocabulary's specs and typed
    /// payloads, tiled by their slots as the driver tiles them.
    pub struct PresSite {
        registry: Arc<FlmRegistry>,
        builder: NodeTreeBuilder<Flm>,
        state: Arc<ParsingState<Flm>>,
        source: Arc<FlmSource>,
    }

    impl PresSite {
        /// A site over the registry's language.
        pub fn new(registry: &Arc<FlmRegistry>) -> PresSite {
            PresSite {
                registry: Arc::clone(registry),
                builder: NodeTreeBuilder::new(),
                state: Arc::clone(registry.lang().initial_state()),
                source: super::source(""),
            }
        }

        /// A site whose nodes stand on a source whose canonical path is
        /// `origin`, for a test that needs nodes with a position a host can
        /// name — what the HTML backend's `source_spans` option reports.
        ///
        /// The source is empty, as [`PresSite::new`]'s is, so every node's
        /// range is `0..0`: what such a site gives a test is the source's
        /// name, not a meaningful range.
        pub fn named(registry: &Arc<FlmRegistry>, origin: &str) -> PresSite {
            let mut site = PresSite::new(registry);
            site.source = Arc::new(FlmSource::new("").with_origin(FlmOrigin::new(origin)));
            site
        }

        fn span(&self) -> SourceSpan<flm_core::lang::FlmOrigin> {
            SourceSpan::new(&self.source, 0..0)
        }

        fn stage(&mut self, kind: NodeKind<Flm>, children: Vec<BuildId>) -> BuildId {
            let span = self.span();
            let staged = self.builder.staged_children(&children);
            let ext = Flm::make_node_ext(&kind, &span, &self.state, staged).expect("the ext mints");
            self.builder
                .add(kind, span, Arc::clone(&self.state), children, ext, ())
                .expect("the node stages")
        }

        /// An owned text node.
        pub fn text(&mut self, text: &str) -> BuildId {
            self.stage(NodeKind::chars(text), Vec::new())
        }

        /// A plain content group over `children`.
        pub fn group(&mut self, children: Vec<BuildId>) -> BuildId {
            let kind = NodeKind::group(GroupData::new(FlmGroupType::Content, "{", "}"));
            self.stage(kind, children)
        }

        /// A presentation node of `element` carrying `payload` over `body`.
        pub fn pres<P: PresPayload>(
            &mut self,
            element: &str,
            payload: P,
            body: Vec<BuildId>,
        ) -> BuildId {
            self.pres_attached(element, payload, body, Vec::new())
        }

        /// [`PresSite::pres`] with attached slots (name, decoration node), as
        /// the driver hoists decorations.
        pub fn pres_attached<P: PresPayload>(
            &mut self,
            element: &str,
            payload: P,
            body: Vec<BuildId>,
            attached: Vec<(&str, BuildId)>,
        ) -> BuildId {
            let mut children = body;
            let content_len = children.len() as u32;
            let mut slots = vec![ParsedSlot::new(
                ChildRegion::new(0..content_len, ContentNodes::InRegion(0..content_len)),
                pres::BODY_SLOT,
                SlotRole::Content,
                BodyMarker::make_body(),
            )];
            for (name, node) in attached {
                let start = children.len() as u32;
                children.push(node);
                slots.push(ParsedSlot::new(
                    ChildRegion::new(start..start + 1, ContentNodes::InRegion(0..1)),
                    name,
                    SlotRole::Attached,
                    BodyMarker::not_body(),
                ));
            }
            self.pres_with_slots(element, Arc::new(payload), children, slots)
        }

        /// [`PresSite::pres`] with the payload behind a pointer, for a test
        /// that holds several payload types in one list.
        pub fn pres_payload(
            &mut self,
            element: &str,
            payload: Arc<dyn PresPayload>,
            body: Vec<BuildId>,
        ) -> BuildId {
            let content_len = body.len() as u32;
            let slots = vec![ParsedSlot::new(
                ChildRegion::new(0..content_len, ContentNodes::InRegion(0..content_len)),
                pres::BODY_SLOT,
                SlotRole::Content,
                BodyMarker::make_body(),
            )];
            self.pres_with_slots(element, payload, body, slots)
        }

        /// A `flm.pres.list` node over `(label, item)` entries.
        pub fn list(
            &mut self,
            payload: ListPayload,
            entries: Vec<(Vec<BuildId>, Vec<BuildId>)>,
        ) -> BuildId {
            let mut children = Vec::new();
            let mut slots = Vec::new();
            for (label, item) in entries {
                for (name, nodes) in [(pres::LABEL_SLOT, label), (pres::ITEM_SLOT, item)] {
                    let start = children.len() as u32;
                    let len = nodes.len() as u32;
                    children.extend(nodes);
                    slots.push(ParsedSlot::new(
                        ChildRegion::new(start..start + len, ContentNodes::InRegion(0..len)),
                        name,
                        SlotRole::Content,
                        BodyMarker::not_body(),
                    ));
                }
            }
            self.pres_with_slots(pres::LIST, Arc::new(payload), children, slots)
        }

        /// A `flm.pres.cells` node over one content per cell, in the
        /// payload's order, each in a `cell` slot.
        pub fn cells(&mut self, payload: CellsPayload, contents: Vec<Vec<BuildId>>) -> BuildId {
            let mut children = Vec::new();
            let mut slots = Vec::new();
            for nodes in contents {
                let start = children.len() as u32;
                let len = nodes.len() as u32;
                children.extend(nodes);
                slots.push(ParsedSlot::new(
                    ChildRegion::new(start..start + len, ContentNodes::InRegion(0..len)),
                    pres::CELL_SLOT,
                    SlotRole::Content,
                    BodyMarker::not_body(),
                ));
            }
            self.pres_with_slots(pres::CELLS, Arc::new(payload), children, slots)
        }

        fn pres_with_slots(
            &mut self,
            element: &str,
            payload: Arc<dyn PresPayload>,
            children: Vec<BuildId>,
            slots: Vec<ParsedSlot<Flm>>,
        ) -> BuildId {
            let entry = self
                .registry
                .presentation_element(element)
                .unwrap_or_else(|| panic!("`{element}` is not a registered presentation element"));
            let spec = Arc::new(FlmPresentationSpec::new(Arc::clone(&entry.element)));
            let kind = NodeKind::callable(
                spec.callable_data(ParsedArguments::empty(), ParsedSlots::new(slots)),
            );
            let span = self.span();
            let staged = self.builder.staged_children(&children);
            let ext = Flm::make_node_ext(&kind, &span, &self.state, staged)
                .expect("the ext mints")
                .with_payload(payload);
            self.builder
                .add(kind, span, Arc::clone(&self.state), children, ext, ())
                .expect("the node stages")
        }

        /// Finishes the tree under a list root over `children`.
        pub fn finish(mut self, children: Vec<BuildId>) -> Arc<NodeTree<Flm>> {
            let root = self.stage(NodeKind::list(), children);
            Arc::new(self.builder.finish(root).expect("the tree finishes"))
        }
    }

    /// A lowered-trees table over `trees`, numbered `tree#0`, `tree#1`, …,
    /// each with no requirements and one round.
    pub fn lowered_trees(trees: Vec<Arc<NodeTree<Flm>>>) -> LoweredTrees {
        let mut table = LoweredTrees { trees: BTreeMap::new() };
        for (index, tree) in trees.into_iter().enumerate() {
            table.trees.insert(
                TreeId::tree(index as u32),
                LoweredTree { tree, requires: BTreeSet::new(), rounds: 1 },
            );
        }
        table
    }

    /// The emitter over one hand-built tree at `width`.
    pub fn emit_tree(tree: Arc<NodeTree<Flm>>, width: Option<u32>) -> Emitted {
        emit_trees(vec![tree], width)
    }

    /// The emitter over hand-built trees in order at `width`.
    pub fn emit_trees(trees: Vec<Arc<NodeTree<Flm>>>, width: Option<u32>) -> Emitted {
        emit_trees_under(trees, &TextRenderOptions { width, ..TextRenderOptions::default() })
    }

    /// The emitter over hand-built trees in order under `options` — for a test
    /// about an option other than the width.
    pub fn emit_trees_under(
        trees: Vec<Arc<NodeTree<Flm>>>,
        options: &TextRenderOptions,
    ) -> Emitted {
        let order: Vec<TreeId> = (0..trees.len()).map(|index| TreeId::tree(index as u32)).collect();
        emit(&lowered_trees(trees), &order, &LoweredPieces::default(), options)
    }

    // --- goldens -------------------------------------------------------------------------

    /// The path of the golden `tests/goldens/text/<name>`, for
    /// `flm_core::dump::golden::assert_text_golden` and `assert_json_golden`
    /// (`UPDATE_GOLDEN=1 cargo test`, exactly `"1"`, rewrites a golden instead
    /// of comparing).
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("text");
        p.push(name);
        p
    }
}

// ============================================================================
// end lowering and the text backend
// ============================================================================

// ============================================================================
// stdcore: helpers for `tests/stdcore_*.rs`
// ============================================================================

/// Helpers for the standard features' tests: registries over a chosen list
/// of the standard features (the `check` feature beside them, since the
/// `blocks`, `math` and `refs` features contribute members to its
/// collection), plans and runs of a named workflow, table projections through
/// a run's frozen trees, the diagnostics' identifiers, and a golden path
/// under `tests/goldens/stdcore/`.
pub mod stdcore {
    use std::path::PathBuf;
    use std::sync::Arc;

    use flm_core::core::plan::{Overrides, Plan, PlanError};
    use flm_core::core::registry::{FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::core::data::table::TableKey;
    use flm_core::dump::project::{ProjectOptions, Projector};
    use flm_core::stdcore::{StdcoreConfig, package};
    use techy::error::Severity;
    use techy::serialize::SerialValue;

    use super::pipeline;

    /// The diagnostics of `run`, one line each, in the order reported: the
    /// severity, the identifier, the message, and the first sixty characters
    /// of the text the diagnostic's span covers (`— at `…``), a line break
    /// written `\n`; then each frame of the diagnostic, in the order it holds
    /// them, as its title and the text its span covers, cut the same way
    /// (`— in `…` at `…``) — the form of the end-to-end suites' diagnostics
    /// goldens.
    pub fn diagnostic_lines(run: &Run) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for diagnostic in run.diagnostics().iter() {
            write!(
                out,
                "{:?} {}: {} — at `{}`",
                diagnostic.severity(),
                diagnostic.identifier(),
                diagnostic.message(),
                cut(diagnostic.span().content())
            )
            .expect("writing to a string");
            for frame in diagnostic.frames() {
                write!(out, " — in `{}` at `{}`", frame.title(), cut(frame.span().content()))
                    .expect("writing to a string");
            }
            out.push('\n');
        }
        out
    }

    /// The title of the frame a diagnostic raised inside a spliced value
    /// gains, spanning the node the value is shown at: pinned here in the
    /// words the lowering driver writes.
    pub const SHOWN_HERE: &str = "a copy of this content, shown here";

    /// The frames of each diagnostic of `run`, in the order reported: each
    /// frame's title and the byte offset its span starts at, in the order the
    /// diagnostic holds them.
    pub fn frames(run: &Run) -> Vec<Vec<(String, usize)>> {
        let frames = |d: &flm_core::lang::FlmDiagnostic| {
            d.frames().iter().map(|f| (f.title().to_string(), f.span().span().start())).collect()
        };
        run.diagnostics().iter().map(frames).collect()
    }

    /// The first sixty characters of `text`, a line break written `\n`.
    fn cut(text: &str) -> String {
        let at: String = text.chars().take(60).collect();
        at.replace('\n', "\\n")
    }

    /// The registry over `features`; a build error is a test failure.
    pub fn registry_over(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
        pipeline::registry(features)
    }

    /// The registry a host builds: the `stdcore` package under `config`
    /// through `RegistryBuilder::use_package`, default inputs.
    pub fn registry(config: StdcoreConfig) -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(config));
        builder.use_package(flm_core::math_passthrough::package());
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    /// The features of the `stdcore` package under `config`, in the package's
    /// order, then the feature of the package `math-passthrough`, but the one
    /// named `left_out`: what a test of a registry without one standard
    /// feature builds over.
    pub fn package_features_but(config: StdcoreConfig, left_out: &str) -> Vec<Arc<dyn FlmFeature>> {
        let features: Vec<Arc<dyn FlmFeature>> = package(config)
            .features
            .into_iter()
            .chain([flm_core::math_passthrough::feature()])
            .filter(|feature| feature.info().name.as_str() != left_out)
            .collect();
        features
    }

    /// The plan of the workflow `workflow` under `overrides`.
    pub fn try_plan(
        registry: &Arc<FlmRegistry>,
        workflow: &str,
        overrides: &Overrides,
    ) -> Result<Plan, PlanError> {
        Plan::for_workflow(registry, workflow, overrides)
    }

    /// The plan of the workflow `workflow` under `overrides`; a plan error
    /// is a test failure.
    pub fn plan(registry: &Arc<FlmRegistry>, workflow: &str, overrides: &Overrides) -> Arc<Plan> {
        Arc::new(
            try_plan(registry, workflow, overrides)
                .unwrap_or_else(|error| panic!("the `{workflow}` plan failed: {error}")),
        )
    }

    /// Runs the workflow `workflow` over one document of one tree per text,
    /// sequentially, with no externals.
    pub fn run_workflow(
        registry: &Arc<FlmRegistry>,
        workflow: &str,
        texts: &[&str],
        overrides: &Overrides,
    ) -> Run {
        run_workflow_with(registry, workflow, texts, overrides, Externals::none())
    }

    /// [`run_workflow`] with externals installed.
    pub fn run_workflow_with(
        registry: &Arc<FlmRegistry>,
        workflow: &str,
        texts: &[&str],
        overrides: &Overrides,
        externals: Externals,
    ) -> Run {
        let doc = pipeline::doc(registry, texts);
        let plan = plan(registry, workflow, overrides);
        pipeline::run_with(&doc, &plan, externals, &RunOptions::sequential())
    }

    /// The projection of a table the run holds, under its own name, node
    /// references resolved through the run's frozen trees; an undeclared or
    /// absent table is a test failure.
    pub fn projection<K: TableKey>(run: &Run, key: K) -> SerialValue {
        let name = key.table_name();
        let table = run
            .table(&key)
            .unwrap_or_else(|error| panic!("reading `{name}` failed: {error}"))
            .unwrap_or_else(|| panic!("`{name}` was not written"));
        Projector::new(ProjectOptions::new()).table(name.as_str(), table, run)
    }

    /// The identifiers of every diagnostic of the run, in the chained view's
    /// order.
    pub fn identifiers(run: &Run) -> Vec<String> {
        run.diagnostics().iter().map(|d| d.identifier().to_string()).collect()
    }

    /// The identifier and the severity of every diagnostic of the run, in the
    /// chained view's order: what a test asserts where the severity is part of
    /// the rule — an Error refuses what the document asked for, a Warning
    /// says that something changed nothing, a Note is a remark.
    pub fn reports(run: &Run) -> Vec<(String, Severity)> {
        run.diagnostics().iter().map(|d| (d.identifier().to_string(), d.severity())).collect()
    }

    /// The Warning an image draws where its size is not known: no option
    /// gives it and no inspector measured the file, as in most tests, which
    /// register no inspector.
    pub const UNKNOWN_SIZE: &str = "flm.graphics.unknown-size";

    /// [`identifiers`] but the Warnings [`UNKNOWN_SIZE`], for a test about
    /// something else than the size of its images.
    pub fn identifiers_but_unknown_size(run: &Run) -> Vec<String> {
        identifiers(run).into_iter().filter(|id| id != UNKNOWN_SIZE).collect()
    }

    /// [`reports`] but the Warnings [`UNKNOWN_SIZE`], for a test about
    /// something else than the size of its images.
    pub fn reports_but_unknown_size(run: &Run) -> Vec<(String, Severity)> {
        reports(run).into_iter().filter(|(id, _)| id != UNKNOWN_SIZE).collect()
    }

    /// The path of the golden `tests/goldens/stdcore/<name>`.
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("stdcore");
        p.push(name);
        p
    }
}

// ============================================================================
// end stdcore
// ============================================================================

// ============================================================================
// lists: the document `tests/stdcore_lists_html.rs` and
// `tests/stdcore_lists_latex.rs` share
// ============================================================================

/// What the two suites of the `lists` feature on the HTML backend and in the
/// recomposed LaTeX share: one document, written once so that the two
/// backends are tested on the same text.
pub mod lists {
    /// The document of the cross-feature tests, on the HTML backend and in
    /// the recomposed LaTeX alike: a formula, a reference, a verbatim block
    /// and a nested list inside an item, and a list standing in a
    /// paragraph's run of text.
    pub const ACROSS_FEATURES: &str = "Before the list, a list in running text \
        \\begin{itemize}\\item one\\item two\\end{itemize} and the sentence goes on.\n\n\
        \\begin{enumerate}\n\
        \\item\\label{item:math} A formula \\(x^2\\) and a display:\n\
        \\[ a = b \\]\n\
        \\item A reference to \\ref{item:math} and to \\ref{item:code}.\n\
        \\item\\label{item:code} Code:\n\
        \\begin{verbatimcode}\n\
        fn main() {}\n\
        \\end{verbatimcode}\n\
        \\item A nested list:\n\
        \\begin{itemize}\\item \\verbcode{inline} code\\item \\emph{emphasis}\\end{itemize}\n\
        \\end{enumerate}";
}

// ============================================================================
// end lists
// ============================================================================

// ============================================================================
// spine: helpers for `tests/spine_*.rs`
// ============================================================================

/// Helpers for the whole-pipeline tests: the two reference documents whose
/// text output at width 80 and 40 is pinned by goldens (one exercising every
/// standard feature, one written in the vocabulary `stdcore::package()`
/// offers), the user definitions they use, registries built from the package
/// alone (plus a test-only feature where a test needs one), renders of the
/// `text` workflow at a width, table projections by name, a whole-run
/// snapshot for
/// the determinism tests, and a golden path under `tests/goldens/spine/`.
pub mod spine {
    use std::path::PathBuf;
    use std::sync::Arc;

    use flm_core::model::options::OptionValues;
    use flm_core::core::plan::{Overrides, Plan};
    use flm_core::core::registry::{
        FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs,
    };
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::core::data::table::Table;
    use flm_core::dump::project::{ProjectOptions, Projector};
    use flm_core::dump::to_json_pretty;
    use flm_core::lang::mint::ArgDef;
    use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
    use flm_core::stdcore::backends::text::{self, TextRenderOptions};
    use flm_core::stdcore::features::refs::{RefsRenderOptions, UnresolvedRefs};
    use flm_core::stdcore::features::userdefs::UserdefsConfig;
    use flm_core::stdcore::{StdcoreConfig, package};
    use techy::serialize::SerialValue;

    use super::pipeline;

    // --- the documents -------------------------------------------------------------------

    /// The document exercising every standard feature: the five
    /// heading levels and a starred one, paragraphs and a comment, the four
    /// formatting commands and the typography substitutions, inline and
    /// display math with unknown macros inside, two math environments, three
    /// user definitions (a macro with an argument, an inline environment, an
    /// `always` macro), two labeled headings referenced with and without the
    /// prefix, an unresolved reference, and the five verbatim callables — the
    /// three macros inline, the inline `verbatimtext` environment and the
    /// `verbatimcode` block, with and without a language.
    pub const FEATURES_DOCUMENT: &str = "\
\\section{Introduction}\\label{sec:intro}

This document exercises every standard feature of build step 1: headings,
paragraphs, the base formatting commands, math, user definitions and
references. It is version \\version.

\\subsection{Formatting}\\label{sec:fmt}

Some \\emph{emphasized}, \\textbf{bold}, \\textit{italic} and \\texttt{monospace}
text --- with a dash, an ellipsis... and \"quotes\" resolved by typography.
%% A comment vanishes (the language's default comment prefix is `%%`).

\\subsubsection{Math}

Inline math \\(x_1 + \\alpha\\) and display math \\[ \\int_0^1 f(x)\\,dx = 1 \\] then
two environments:

\\begin{equation}
  E = mc^2
\\end{equation}

\\begin{align*}
  a &= b \\\\
  c &= d
\\end{align*}

\\subsubsection{Verbatim}

Inline \\verbtext+a{b}\\c+ text, the code \\verbcode[python]+f(x) = 1+ and the
identifier \\verba[rust]+struct+, then \\begin{verbatimtext}raw {text}\\end{verbatimtext}
in the same paragraph, and a code block:

\\begin{verbatimcode}[python]
def f(x):
    return {x} + 1
\\end{verbatimcode}

\\paragraph{Definitions}

\\hello{World} from a user macro, and \\begin{note}a user environment\\end{note}
in the same paragraph.

\\subparagraph{References}

See \\ref{sec:intro}, \\ref[noprefix]{sec:fmt} and \\ref{sec:missing}.

\\section*{Unnumbered}

A starred heading takes no number and stays in the contents.
";

    /// The second reference document, matching the emitter's hand-built golden
    /// `emit-document.txt` and written in the vocabulary the package offers:
    /// the run-in heading is bold text, the QED mark plain text, the two list
    /// items paragraphs (no feature stages a `list`), and the code block two
    /// monospace spans in one paragraph. It is kept exactly as written so that
    /// its goldens stay comparable with the emitter's; the verbatim feature is
    /// exercised by [`FEATURES_DOCUMENT`] instead.
    pub const SESSION06_DOCUMENT: &str = "\
\\section{Introduction}\\label{sec:intro}

See \\ref{sec:intro} and \\emph{this emphasized bit} here in a paragraph that
is long enough to wrap at forty columns.

\\textbf{Theorem.} Every finite thing is finite, and this sentence is long
enough to wrap somewhere. \u{25a1}

1. First item, with enough words in it to wrap across two lines nicely.

2. Second item text that also wraps a little.

\\texttt{let x = 1;} \\texttt{let y = 2;}
";

    /// The user definitions the documents use: `\\hello{name}`, the inline
    /// environment `note`, and the `always` macro `\\version`.
    pub fn definitions() -> Vec<Definition> {
        vec![
            Definition::macro_("hello", vec![ArgDef::mandatory("name")]).text("Hello, #{name}!"),
            Definition::environment("note", Vec::new()).text("[Note: #{body}]"),
            Definition::macro_("version", Vec::new()).text("1.0").expand(ExpandPolicy::Always),
        ]
    }

    /// The package configuration of the spine: the default text options and
    /// [`definitions`].
    pub fn config() -> StdcoreConfig {
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions()))
    }

    // --- registries ----------------------------------------------------------------------

    /// The registry every spine test starts from: `stdcore::package()` under
    /// [`config`], nothing else.
    pub fn registry() -> Arc<FlmRegistry> {
        registry_with(Vec::new())
    }

    /// [`registry`] with test-only features registered beside the package
    /// (the deliberately unknown element, the rule-reachability fixture).
    pub fn registry_with(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(config()));
        builder.use_package(flm_core::math_passthrough::package());
        for feature in features {
            builder.feature(feature);
        }
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    // --- plans and runs ------------------------------------------------------------------

    /// The render request of the spine: the text width (`None` never wraps)
    /// and whether an unresolved reference is tolerated.
    pub fn overrides(width: Option<u32>, unresolved: UnresolvedRefs) -> Overrides {
        let options = OptionValues::new()
            .with(RefsRenderOptions { unresolved })
            .with(TextRenderOptions { width, ..TextRenderOptions::default() });
        Overrides { render_options: options, ..Overrides::none() }
    }

    /// The `text` plan under `overrides`; a plan error is a test failure.
    pub fn text_plan(registry: &Arc<FlmRegistry>, overrides: &Overrides) -> Arc<Plan> {
        Arc::new(
            Plan::for_workflow(registry, text::WORKFLOW, overrides)
                .unwrap_or_else(|error| panic!("the text plan failed: {error}")),
        )
    }

    /// Renders one document of one tree per text through the `text`
    /// workflow, sequentially, with no externals.
    pub fn render(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
        let doc = pipeline::doc(registry, texts);
        let plan = text_plan(registry, overrides);
        pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential())
    }

    /// The text of the document rendered at `width` columns (`None` never
    /// wraps), the unresolved reference tolerated (the features document
    /// carries one on purpose).
    pub fn text_at(registry: &Arc<FlmRegistry>, document: &str, width: Option<u32>) -> String {
        let run = render(registry, &[document], &overrides(width, UnresolvedRefs::Allow));
        super::lowering::text(&run)
    }

    // --- readers -------------------------------------------------------------------------

    /// The projections of the named tables of the run as one map, each
    /// table's projection under its name (what `flm dump --table` prints for
    /// one of them), node references resolved through the run's frozen
    /// trees; a table the registry does not declare or the run did not write
    /// is a test failure.
    pub fn tables_projection(run: &Run, names: &[&str]) -> SerialValue {
        let registry = run.plan().registry();
        let projector = Projector::new(ProjectOptions::new());
        let entries = names
            .iter()
            .map(|name| {
                let id = registry.table_id(name).unwrap_or_else(|| panic!("`{name}` is declared"));
                let value =
                    run.computed().get_erased(id).unwrap_or_else(|| panic!("`{name}` was written"));
                let entry = registry.table(id);
                let projected =
                    entry.def.ty.project(value.as_ref(), &projector.table_projection(run));
                (name.to_string(), projected)
            })
            .collect();
        SerialValue::Map(entries)
    }

    /// A rendering of everything the run computed, for the determinism
    /// tests: every table of the store in `TableId` order through its
    /// projection (node references through the frozen trees), then the text
    /// output's projection (text and marks), then every diagnostic's
    /// identifier and span range — the projections as pretty JSON, so that a
    /// determinism failure diffs at a glance.
    pub fn snapshot(run: &Run) -> String {
        let registry = run.plan().registry();
        let projector = Projector::new(ProjectOptions::new());
        let json = |value: &SerialValue| {
            to_json_pretty(value).unwrap_or_else(|_| format!("{value:?}"))
        };
        let mut out = String::new();
        for id in run.computed().ids() {
            let entry = registry.table(id);
            let value = run.computed().get_erased(id).expect("a listed table is present");
            let projected =
                entry.def.ty.project(value.as_ref(), &projector.table_projection(run));
            out.push_str(&format!("{} = {}\n", entry.def.name, json(&projected)));
        }
        let output = super::lowering::output(run).project(&projector.table_projection(run));
        out.push_str(&format!("flm.output.text (whole) = {}\n", json(&output)));
        for diagnostic in run.diagnostics().iter() {
            out.push_str(&format!(
                "{:?} {} {:?}\n",
                diagnostic.severity(),
                diagnostic.identifier(),
                diagnostic.span().range()
            ));
        }
        out
    }

    /// The path of the golden `tests/goldens/spine/<name>`.
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("spine");
        p.push(name);
        p
    }
}

// ============================================================================
// end spine
// ============================================================================

// ============================================================================
// latex: helpers for `tests/latex_*.rs`
// ============================================================================

/// Helpers for the LaTeX family's and the LaTeX backends' tests: the golden
/// path under `tests/goldens/latex/`, and the two artifacts of a
/// `latex-recomposed` run as text and as one golden.
pub mod latex {
    use std::path::PathBuf;

    use flm_core::core::artifact::ArtifactSet;
    use flm_core::core::run::Run;
    use flm_core::dump::golden::assert_text_golden;
    use flm_core::stdcore::backends::latex_recomposed::{DOCUMENT, style_artifact};

    /// The path of the golden `tests/goldens/latex/<name>`.
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("latex");
        p.push(name);
        p
    }

    /// The artifact set the `latex-recomposed` output wrote; its absence is
    /// a test failure that prints the run's diagnostics.
    pub fn artifacts(run: &Run) -> &ArtifactSet {
        run.table(DOCUMENT).expect("the output table is declared").unwrap_or_else(|| {
            panic!("no latex-recomposed output; diagnostics:\n{}", run.diagnostics().render_all())
        })
    }

    /// The whole recomposed LaTeX: the entry artifact's bytes as text.
    pub fn body(run: &Run) -> String {
        let entry = artifacts(run).entry().expect("the entry artifact");
        String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
    }

    /// The recomposed LaTeX after its first line, the comment naming the style
    /// file (whose content hash a test does not pin by hand). The whole
    /// recomposed LaTeX where the document needed no style file and none was
    /// written.
    pub fn output(run: &Run) -> String {
        let text = body(run);
        let wrote_style = style_if_any(run).is_some();
        let Some((header, rest)) = text.split_once('\n') else {
            assert!(!wrote_style, "a run that wrote a style file has a header line: {text}");
            return text;
        };
        if !header.starts_with("% flm: style file ") {
            assert!(!wrote_style, "a run that wrote a style file names it first: {header}");
            return text;
        }
        assert!(header.ends_with(".sty"), "{header}");
        String::from(rest)
    }

    /// The style file if the run wrote one: its bytes as text, and its fixed
    /// path. A document that needs no package, declares no character, needs
    /// no definition piece and recomposes no user definition gets none.
    pub fn style_if_any(run: &Run) -> Option<(String, String)> {
        let style = style_artifact(artifacts(run))?;
        let text = String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
        let path =
            style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
        Some((text, path))
    }

    /// The style file, which the run must have written.
    pub fn style(run: &Run) -> (String, String) {
        style_if_any(run).expect("the style artifact")
    }

    /// The artifacts of a run as one golden text: the recomposed LaTeX, then the
    /// style file where there is one, each under a marker line that is
    /// itself a LaTeX comment — `% ===== latex-recomposed:document =====`
    /// and `% ===== latex-recomposed:style <path> =====`. A golden with no
    /// style marker is a document that needed no style file.
    pub fn golden_text(run: &Run) -> String {
        match style_if_any(run) {
            Some((style, path)) => format!(
                "% ===== latex-recomposed:document =====\n{}% ===== latex-recomposed:style {path} =====\n{style}",
                body(run)
            ),
            None => format!("% ===== latex-recomposed:document =====\n{}", body(run)),
        }
    }

    /// Asserts the run's golden text against `tests/goldens/latex/<name>`.
    pub fn assert_golden(name: &str, run: &Run) {
        assert_text_golden(golden_path(name), &golden_text(run));
    }

    /// Display-math environments in which a `\label` or a final `\\` stands on
    /// a line of its own, so that the whitespace before and after it meet once
    /// it is taken out of the line: after a nested environment, a final `\\`,
    /// between two lines of one equation line, after a comment, and after a
    /// macro that takes the newline after its name.
    pub const COMMANDS_ON_LINES_OF_THEIR_OWN: &str = "\
\\begin{gather}
  \\begin{aligned}
    a &\\to b\\ ;
    \\\\
    c &\\to d\\ ,
  \\end{aligned}
  \\label{eq:enc}
\\end{gather}

\\begin{align}
  a &= b
  \\\\
  c &= d
  \\\\
\\end{align}

\\begin{equation}
  a = b
  \\label{eq:mid}
  + c
\\end{equation}

\\begin{align}
  a &= b \\\\ %% note
  \\label{eq:comment}
  c &= d
\\end{align}

\\begin{equation}
  x \\to \\infty
  \\label{eq:macro}
\\end{equation}
";

    /// The lines of `latex` that hold only whitespace inside a display-math
    /// environment (`equation`, `align` or `gather`, starred or not), by their
    /// numbers from 1: LaTeX refuses a blank line inside mathematics.
    pub fn blank_lines_in_math(latex: &str) -> Vec<usize> {
        let marks = |line: &str, command: &str| {
            ["equation", "align", "gather"].iter().any(|name| {
                line.contains(&format!("\\{command}{{{name}}}")) || line.contains(&format!("\\{command}{{{name}*}}"))
            })
        };
        let mut inside = false;
        let mut blank = Vec::new();
        for (index, line) in latex.lines().enumerate() {
            if inside && line.trim().is_empty() {
                blank.push(index + 1);
            }
            if marks(line, "begin") {
                inside = true;
            }
            if marks(line, "end") {
                inside = false;
            }
        }
        blank
    }
}

// ============================================================================
// end latex
// ============================================================================

// ============================================================================
// html: helpers for `tests/html_*.rs` — golden paths, the emitter over
// hand-built presentation trees, a fragment run under a chosen layout, and the
// two readings of a run's HTML
// ============================================================================

/// Helpers for the HTML backend's tests: the golden path under
/// `tests/goldens/html/`, the emitter run over a hand-built presentation tree,
/// the `html` workflow run over one fragment parsed under a chosen layout
/// ([`run_one_fragment_under`]), and the two ways a test reads a run's HTML —
/// the trees' own HTML ([`fragment`]) and the whole page ([`page`]).
pub mod html {
    use std::path::PathBuf;
    use std::sync::Arc;

    use flm_core::core::artifact::ArtifactSet;
    use flm_core::core::plan::Overrides;
    use flm_core::core::registry::FlmRegistry;
    use flm_core::core::run::{Run, RunOptions};
    use flm_core::model::doc::FragmentLayout;
    use flm_core::dump::golden::assert_text_golden;
    use flm_core::lang::Flm;
    use flm_core::stdcore::backends::html::{
        DOCUMENT, EmittedTree, HtmlFragments, HtmlRenderOptions, OUTPUT_TABLE, WORKFLOW,
        emit_tree,
    };
    use techy::core::node::NodeTree;

    /// The path of the golden `tests/goldens/html/<name>`, for
    /// `flm_core::dump::golden::assert_text_golden` (`UPDATE_GOLDEN=1 cargo
    /// test`, exactly `"1"`, rewrites a golden instead of comparing).
    pub fn golden_path(name: &str) -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("goldens");
        p.push("html");
        p.push(name);
        p
    }

    /// The emitter over one hand-built presentation tree, under the default
    /// render options.
    pub fn emit(tree: Arc<NodeTree<Flm>>) -> EmittedTree {
        emit_with(tree, &HtmlRenderOptions::default())
    }

    /// The emitter over one hand-built presentation tree, under `options`.
    pub fn emit_with(tree: Arc<NodeTree<Flm>>, options: &HtmlRenderOptions) -> EmittedTree {
        emit_tree(&tree, options)
    }

    /// The HTML of an emitted tree: the bytes of its segments, run together.
    /// A tree the emission placed contributes nothing here, since only the
    /// page assembly can write one.
    pub fn html(emitted: &EmittedTree) -> String {
        use flm_core::stdcore::backends::html::HtmlSegment;
        let mut out = String::new();
        for segment in &emitted.segments {
            if let HtmlSegment::Html(text) = segment {
                out.push_str(text);
            }
        }
        out
    }

    /// The run of the `html` workflow over a document of one fragment holding
    /// `text`, parsed under `layout`, sequentially and with no externals.
    ///
    /// A fragment's *layout* — whether its content is one inline run, a
    /// sequence of blocks, or the default, which stays inline until a
    /// paragraph break or a block element appears — is declared when the
    /// fragment is parsed, and it is what decides whether a paragraph element
    /// is written. A test about the layout therefore builds its document
    /// here, rather than through `common::stdcore::run_workflow`, which parses
    /// every text under the default layout. The document itself is built by
    /// `common::wiring::doc`, the one helper that parses under a chosen
    /// layout.
    ///
    /// The run is answered whole, so that a test says for itself what it
    /// expects of the diagnostics; [`fragment`] reads the HTML out of it.
    pub fn run_one_fragment_under(
        registry: &Arc<FlmRegistry>,
        layout: FragmentLayout,
        text: &str,
    ) -> Run {
        let doc = super::wiring::doc(registry, layout, &[text]);
        let plan = super::stdcore::plan(registry, WORKFLOW, &Overrides::none());
        super::pipeline::run(&doc, &plan, &RunOptions::sequential())
    }

    /// The fragments table the run's emit stage wrote; its absence is a test
    /// failure that prints the run's diagnostics.
    pub fn fragments(run: &Run) -> &HtmlFragments {
        run.table(OUTPUT_TABLE).expect("the fragments table is declared").unwrap_or_else(|| {
            panic!("no html fragments; diagnostics:\n{}", run.diagnostics().render_all())
        })
    }

    /// The HTML of the run's document trees, in tree order, run together with
    /// one line break between two trees.
    ///
    /// This is the content without the page around it. A tree placed in
    /// another tree stands here at its own position in the order rather than
    /// where it was placed; a test about placement reads [`page`] instead.
    pub fn fragment(run: &Run) -> String {
        let mut parts: Vec<String> = Vec::new();
        for fragment in fragments(run).trees.values() {
            parts.push(fragment.html());
        }
        let mut out = parts.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    /// The artifact set the `html` output wrote; its absence is a test failure
    /// that prints the run's diagnostics.
    pub fn artifacts(run: &Run) -> &ArtifactSet {
        run.table(DOCUMENT).expect("the output table is declared").unwrap_or_else(|| {
            panic!("no html output; diagnostics:\n{}", run.diagnostics().render_all())
        })
    }

    /// The page: the entry artifact's bytes as text.
    pub fn page(run: &Run) -> String {
        let entry = artifacts(run).entry().expect("the entry artifact");
        String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
    }

    /// Asserts `text` against `tests/goldens/html/<name>`.
    ///
    /// The golden holds the text with a final line break, which HTML ignores,
    /// so that the file ends in one as every text file should.
    pub fn assert_golden(name: &str, text: &str) {
        assert_golden_at(golden_path(name), text);
    }

    /// [`assert_golden`] against a golden in another directory, for the
    /// whole-pipeline suite, whose HTML goldens live under
    /// `tests/goldens/spine/` beside the other backends' (`common::spine::golden_path`).
    /// The trailing line break is added here too, so that every HTML golden of
    /// the crate ends the same way.
    pub fn assert_golden_at(path: PathBuf, text: &str) {
        let mut text = String::from(text);
        if !text.ends_with('\n') {
            text.push('\n');
        }
        assert_text_golden(path, &text);
    }
}

// ============================================================================
// end html
// ============================================================================

// ============================================================================
// pieces: the test feature of `tests/stdcore_pieces.rs` and
// `tests/html_pieces.rs`
// ============================================================================

/// A test feature offering pieces: for each piece it is given, the piece's
/// table (of type `Piece`), a document-scope builder stage writing it, and the
/// piece's declaration, contributed to the slot `flm.pieces` as a peer. What a
/// builder writes is fixed per piece: a value parsed from a given source, a
/// copy of the first document tree's content, or nothing at all.
///
/// The feature also declares `\tpplace`, whose rule stages a
/// `flm.pres.subtree` node placing the document's first tree — what a piece
/// may not hold.
pub mod pieces {
    use std::sync::Arc;

    use flm_core::core::data::store::StageOut;
    use flm_core::core::lowering::{DefaultMark, Lowering};
    use flm_core::core::piece::{Piece, PieceDecl, PiecePlace};
    use flm_core::core::plan::PlanCx;
    use flm_core::core::registry::{
        BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder,
        RegistryInputs,
    };
    use flm_core::core::run::StageCx;
    use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
    use flm_core::lang::copy::{ValueContent, copy_run_into_value};
    use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
    use flm_core::model::addr::TreeId;
    use flm_core::model::doc::FragmentLayout;
    use flm_core::model::names::ElementName;
    use flm_core::stdcore::backends::pieces::SLOT;
    use flm_core::stdcore::backends::pres::{self, SubtreePayload};
    use flm_core::stdcore::{StdcoreConfig, package};

    use super::lowering::{produces, rule};

    /// The test feature's name.
    pub const OWNER: &str = "tp";

    /// What a builder writes into its piece.
    #[derive(Clone)]
    pub enum Content {
        /// A value parsed from this source.
        Source(&'static str),
        /// A copy of the first document tree's content.
        CopyOfFirstTree,
        /// `Piece { content: None }`.
        Nothing,
    }

    /// One piece the test feature offers: its name's last segment, its place,
    /// whether it is on by default, and what its builder writes.
    #[derive(Clone)]
    pub struct TestPiece {
        pub suffix: &'static str,
        pub place: PiecePlace,
        pub default_on: bool,
        pub content: Content,
    }

    impl TestPiece {
        /// A piece placed after the content, on by default.
        pub fn after(suffix: &'static str, content: Content) -> TestPiece {
            TestPiece { suffix, place: PiecePlace::AfterContent, default_on: true, content }
        }

        /// A piece placed before the content, on by default.
        pub fn before(suffix: &'static str, content: Content) -> TestPiece {
            TestPiece { suffix, place: PiecePlace::BeforeContent, default_on: true, content }
        }

        /// The same piece, off by default.
        pub fn off(mut self) -> TestPiece {
            self.default_on = false;
            self
        }

        /// The piece's name, which is also its table's.
        pub fn name(&self) -> String {
            format!("{OWNER}.{}", self.suffix)
        }

        /// The piece's declaration.
        pub fn decl(&self) -> PieceDecl {
            PieceDecl::new(self.name(), self.place, "a test piece").default_on(self.default_on)
        }

        /// The builder stage's name.
        pub fn builder(&self) -> String {
            format!("{OWNER}.build-{}", self.suffix)
        }
    }

    /// The builder of one piece: a document-scope stage writing the piece's
    /// table from the trees.
    struct Builder {
        piece: TestPiece,
    }

    impl Stage for Builder {
        fn decl(&self) -> StageDecl {
            StageDecl::new(self.piece.builder(), Effect::Compute, Scope::Document)
                .writes(self.piece.decl().table)
                .docs("builds a test piece")
        }

        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            Needs::new()
        }

        fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
            let content = match &self.piece.content {
                Content::Nothing => None,
                Content::Source(text) => {
                    let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
                    let fragment = parse_fragment(
                        cx.registry().lang(),
                        source,
                        FragmentLayout::default(),
                        &ParseOptions::default(),
                    )
                    .expect("the test piece's source parses");
                    Some(Arc::clone(fragment.tree()))
                }
                Content::CopyOfFirstTree => {
                    let (_, tree) = cx.trees().next().expect("the document has a tree");
                    let root = tree.root();
                    let run: Vec<_> = root.children().iter().collect();
                    Some(
                        copy_run_into_value(&run, &root, ValueContent::Own, cx.registry().lang().guard())
                            .expect("the first tree copies"),
                    )
                }
            };
            let handle = self.piece.decl().table;
            *out.table(&handle).expect("the builder writes its declared table") =
                Piece { content };
        }
    }

    /// The test feature: one table, one builder and one declaration per
    /// piece, and the command `\tpplace`.
    pub struct TestFeature {
        pub pieces: Vec<TestPiece>,
    }

    impl FlmFeature for TestFeature {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new(OWNER, "Test pieces").docs("pieces for the pieces tests")
        }

        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            for piece in &self.pieces {
                b.table_named::<Piece>(piece.name(), "a test piece")?;
                b.stage(Arc::new(Builder { piece: piece.clone() }))?;
                b.peer_slot(SLOT, Arc::new(piece.decl()))?;
            }
            b.element(super::macro_def(OWNER, "tpplace", 0))?;
            b.lowering_rule(rule(
                "tp.tpplace.lower",
                "tp.tpplace",
                produces(&[pres::SUBTREE]),
                DefaultMark::Everywhere,
                |_node, cx, lw| {
                    let payload = Arc::new(SubtreePayload { tree: TreeId::tree(0) });
                    let id = lw.pres(cx, &ElementName::from(pres::SUBTREE), payload, Vec::new())?;
                    Ok(Lowering::Emit(vec![id]))
                },
            ))?;
            Ok(())
        }
    }

    /// The `stdcore` package plus the test feature offering `pieces`.
    pub fn registry(pieces: Vec<TestPiece>) -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new()));
        builder.use_package(flm_core::math_passthrough::package());
        builder.feature(Arc::new(TestFeature { pieces }));
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }
}

// ============================================================================
// end pieces
// ============================================================================

// ============================================================================
// xnotes: a kind of xnote another feature contributes, the programmatic path
// ============================================================================

/// A test feature contributing a kind of xnote of its own — the path another
/// feature takes: the kind `addendum`, with no command, made
/// through `xnotes::contribute_kind`; the element `\taddendum{text}`, answering
/// the aspect `XnoteSource` with the region of its own argument and returning
/// `xnotes::ref_points` from its `Referenceable` answer; and a lowering rule
/// writing its mark through `xnotes::stage_mark`. Nothing else: the collect
/// stage, the numbers, the anchors, the kind's list, its builder and its rule
/// are the `xnotes` feature's machinery, the same as for `\footnote`.
pub mod xnotes {
    use std::sync::Arc;

    use flm_core::core::lowering::{
        DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
        NodeRegion,
    };
    use flm_core::core::registry::{
        BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder,
        RegistryInputs,
    };
    use flm_core::lang::Flm;
    use flm_core::lang::copy::ValueContent;
    use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
    use flm_core::model::aspects::{
        ApartFromText, BlockRole, BlockRoleAspect, ContentRef, CopiedAs, FixedBlockRole, InCopy,
        InlineSpacing, RefPoint, Referenceable,
    };
    use flm_core::model::element::ElementSpec;
    use flm_core::model::names::Name;
    use flm_core::stdcore::backends::latex::construct::{MACRO, RAW};
    use flm_core::stdcore::backends::latex_recomposed::{self, LatexRecomposedRenderOptions};
    use flm_core::stdcore::backends::{html, pres};
    use flm_core::stdcore::features::numbering::{COUNTERS, PrefixWords};
    use flm_core::stdcore::features::refs::{ANCHORS, SAFE_LABELS};
    use flm_core::stdcore::features::xnotes::{
        self, XNOTES, XnoteKind, XnoteOf, XnotePlacement, XnoteSource, XnotesConfig, contribute_kind,
        ref_points,
    };
    use flm_core::stdcore::{StdcoreConfig, package};
    use techy::core::node::NodeRef;

    /// The test feature's name.
    pub const OWNER: &str = "test-addenda";

    /// The element of `\taddendum`.
    pub const ADDENDUM: &str = "test.addendum";

    /// The contributed kind: numbered, `Add.~`/`Adds.~`, label type `add`,
    /// no command, listed under the title `Addenda`.
    pub fn kind() -> XnoteKind {
        XnoteKind::numbered("addendum", PrefixWords::new("Add.~", "Adds.~"))
            .label_type("add")
            .title("Addenda")
    }

    /// The value of `\taddendum{text}`: an xnote of the contributed kind whose
    /// content is the region of its own argument `text`; a mark attached to the
    /// word before it, as a listed footnote's.
    struct TestAddendum(Arc<XnoteKind>);

    impl XnoteSource for TestAddendum {
        fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
            NodeRegion::argument_named(node, "text")
                .map(|content| XnoteOf { kind: self.0.name.clone(), content })
                .into_iter()
                .collect()
        }
    }

    impl Referenceable for TestAddendum {
        fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
            ref_points(&self.0, &[], None)
        }
    }

    /// The rule of `\taddendum`: its mark, through the public helper.
    struct AddendumRule(Arc<XnoteKind>);

    impl Lower for AddendumRule {
        fn lower(
            &self,
            node: NodeRef<'_, Flm>,
            cx: &mut LowerCx<'_>,
            lw: &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError> {
            Ok(Lowering::Emit(xnotes::stage_mark(cx, lw, &node, &self.0, 0)?))
        }
    }

    /// The feature: `contribute_kind`, the element, its rule.
    pub struct Addenda;

    impl FlmFeature for Addenda {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new(OWNER, "Test addenda")
        }

        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            let kind = Arc::new(kind());
            contribute_kind(b, Arc::clone(&kind))?;
            let value = Arc::new(TestAddendum(Arc::clone(&kind)));
            b.element(ElementDef::new(
                b.feature().clone(),
                ["taddendum"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                Arc::new(
                    ElementSpec::new(ADDENDUM)
                        .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Inline(
                            InlineSpacing { absorb_before: true, absorb_after: false },
                        ))))
                        .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                        .with::<dyn Referenceable>(value as Arc<dyn Referenceable>),
                ),
            ))?;
            // The tables and presentation elements the rule names are the
            // peers `contribute_kind` declared for this feature.
            b.lowering_rule(LoweringRule::new(
                "test.addendum.lower",
                ADDENDUM,
                LoweringDecl::new()
                    .reads(XNOTES)
                    .reads_opt(COUNTERS)
                    .reads_opt(ANCHORS)
                    .produces(pres::SPAN)
                    .produces(pres::LINK)
                    .produces(pres::BLOCK)
                    .produces(pres::PAR),
                Arc::new(AddendumRule(kind)),
                DefaultMark::Everywhere,
            ))
        }
    }

    /// The standard package — its `xnotes` feature under the default
    /// configuration, the kind `footnote` — and the test feature beside it.
    pub fn registry() -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new()));
        builder.use_package(flm_core::math_passthrough::package());
        builder.feature(Arc::new(Addenda));
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    /// The element of `\ttwonotes`.
    pub const TWO_NOTES: &str = "test.two-notes";

    /// The value of `\ttwonotes{first}{second}`: two xnotes of one kind on
    /// one node, the first's content the argument `first`, the second's the
    /// argument `second` — the worked example of several xnotes on one node.
    /// Its answers go through the `xnotes` feature's public functions alone:
    /// one `XnoteOf` and one `ref_points` call per xnote, in the same order,
    /// the regions apart from the text, and the stand-in in a copy.
    struct TwoNotesValue(Arc<XnoteKind>);

    /// The two arguments, in order.
    const TWO_NOTES_ARGS: [&str; 2] = ["first", "second"];

    impl XnoteSource for TwoNotesValue {
        fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
            TWO_NOTES_ARGS
                .iter()
                .filter_map(|arg| NodeRegion::argument_named(node, arg))
                .map(|content| XnoteOf { kind: self.0.name.clone(), content })
                .collect()
        }
    }

    impl Referenceable for TwoNotesValue {
        fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
            TWO_NOTES_ARGS
                .iter()
                .flat_map(|arg| ref_points(&self.0, &[], Some(ContentRef::ArgumentNamed(arg.to_string()))))
                .collect()
        }
    }

    impl ApartFromText for TwoNotesValue {
        fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
            xnotes::regions_apart_from_text(node)
        }
    }

    impl InCopy for TwoNotesValue {
        fn in_copy(&self, node: &NodeRef<'_, Flm>, purpose: ValueContent) -> CopiedAs {
            xnotes::copied_as(node, purpose, vec![Arc::clone(&self.0)])
        }
    }

    impl BlockRoleAspect for TwoNotesValue {
        fn block_role(&self, _node: &NodeRef<'_, Flm>) -> BlockRole {
            match self.0.placement {
                XnotePlacement::List => {
                    BlockRole::Inline(InlineSpacing { absorb_before: true, absorb_after: false })
                }
                XnotePlacement::InPlace => BlockRole::Inline(InlineSpacing::CONTENT),
            }
        }
    }

    /// The rule of `\ttwonotes` on the text output and the page: each xnote
    /// through the staging helper of its kind's placement, the first then the
    /// second, side by side; on the page, an in-place kind's brackets drawn by
    /// the feature's stylesheet.
    struct TwoNotesRule {
        kind: Arc<XnoteKind>,
        brackets: xnotes::Brackets,
    }

    impl Lower for TwoNotesRule {
        fn lower(
            &self,
            node: NodeRef<'_, Flm>,
            cx: &mut LowerCx<'_>,
            lw: &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError> {
            let mut staged = Vec::new();
            for nth in 0..TWO_NOTES_ARGS.len() {
                staged.extend(match self.kind.placement {
                    XnotePlacement::List => xnotes::stage_mark(cx, lw, &node, &self.kind, nth)?,
                    XnotePlacement::InPlace => {
                        xnotes::stage_in_place(cx, lw, &node, &self.kind, nth, self.brackets)?
                    }
                });
            }
            if self.brackets == xnotes::Brackets::Drawn {
                lw.require(&Name::from(xnotes::STYLE));
            }
            Ok(Lowering::Emit(staged))
        }
    }

    /// The rule of `\ttwonotes` on the recomposed LaTeX: each xnote as its
    /// command, the first then the second.
    struct TwoNotesLatexRule(Arc<XnoteKind>);

    impl Lower for TwoNotesLatexRule {
        fn lower(
            &self,
            node: NodeRef<'_, Flm>,
            cx: &mut LowerCx<'_>,
            lw: &mut dyn Lowerer,
        ) -> Result<Lowering, LowerError> {
            let mut staged = Vec::new();
            for nth in 0..TWO_NOTES_ARGS.len() {
                staged.extend(xnotes::stage_latex(cx, lw, &node, &self.0, nth, &[])?);
            }
            Ok(Lowering::Emit(staged))
        }
    }

    /// The feature of `\ttwonotes{first}{second}`: one element carrying two
    /// xnotes of the kind `kind` on one node — a kind the `xnotes` feature
    /// configures, `footnote` by default — written by the same helpers as
    /// `\footnote`, on the text output, the page and the recomposed LaTeX.
    pub struct TwoNotes {
        kind: Arc<XnoteKind>,
    }

    impl TwoNotes {
        /// The feature whose notes are of `kind`, which must be a kind the
        /// registry's `xnotes` feature configures.
        pub fn new(kind: XnoteKind) -> TwoNotes {
            TwoNotes { kind: Arc::new(kind) }
        }
    }

    impl FlmFeature for TwoNotes {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("test-two-notes", "Two notes on one node")
        }

        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            let value = Arc::new(TwoNotesValue(Arc::clone(&self.kind)));
            b.element(ElementDef::new(
                b.feature().clone(),
                ["ttwonotes"],
                SyntaxDef::Macro {
                    args: TWO_NOTES_ARGS.iter().map(|arg| ArgDef::mandatory(*arg)).collect(),
                },
                Arc::new(
                    ElementSpec::new(TWO_NOTES)
                        .with::<dyn BlockRoleAspect>(Arc::clone(&value) as Arc<dyn BlockRoleAspect>)
                        .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                        .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>)
                        .with::<dyn ApartFromText>(Arc::clone(&value) as Arc<dyn ApartFromText>)
                        .with::<dyn InCopy>(value as Arc<dyn InCopy>),
                ),
            ))?;
            let decl = || {
                LoweringDecl::new()
                    .reads(XNOTES)
                    .reads_opt(COUNTERS)
                    .reads_opt(ANCHORS)
                    .produces(pres::SPAN)
                    .produces(pres::LINK)
                    .produces(pres::BLOCK)
                    .produces(pres::PAR)
            };
            b.lowering_rule(LoweringRule::new(
                "test.two-notes.lower",
                TWO_NOTES,
                decl(),
                Arc::new(TwoNotesRule { kind: Arc::clone(&self.kind), brackets: xnotes::Brackets::Text }),
                DefaultMark::Everywhere,
            ))?;
            b.lowering_rule(LoweringRule::new(
                "test.two-notes.lower.html",
                TWO_NOTES,
                decl(),
                Arc::new(TwoNotesRule { kind: Arc::clone(&self.kind), brackets: xnotes::Brackets::Drawn }),
                DefaultMark::OnBackends(vec![html::BACKEND.into()]),
            ))?;
            b.lowering_rule(LoweringRule::new(
                "test.two-notes.lower.latex-recomposed",
                TWO_NOTES,
                LoweringDecl::new()
                    .reads_opt(SAFE_LABELS)
                    .option::<LatexRecomposedRenderOptions>()
                    .produces(MACRO)
                    .produces(RAW),
                Arc::new(TwoNotesLatexRule(Arc::clone(&self.kind))),
                DefaultMark::OnBackends(vec![latex_recomposed::BACKEND.into()]),
            ))
        }
    }

    /// The standard package with its `xnotes` feature under `config`, and
    /// `\ttwonotes` beside it, its notes of the configured kind `kind`.
    pub fn two_notes_registry(config: XnotesConfig, kind: &str) -> Arc<FlmRegistry> {
        let of_kind = config.kinds.iter().find(|k| k.name.as_str() == kind).expect("a configured kind").clone();
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new().xnotes(config)));
        builder.use_package(flm_core::math_passthrough::package());
        builder.feature(Arc::new(TwoNotes::new(of_kind)));
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }
}

// ============================================================================
// end xnotes
// ============================================================================

// ============================================================================
// citations: the test providers of entries
// ============================================================================

/// Providers of the entries of cited works for the tests, and the registries
/// holding them: a document citing works renders without an Error only where
/// a provider answers every key it cites, since every workflow checks it.
#[allow(dead_code)]
pub mod citations {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use flm_core::core::registry::{FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
    use flm_core::lang::copy::{copy_pieces_into_value, ValueContent, ValuePiece};
    use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
    use flm_core::model::doc::FragmentLayout;
    use flm_core::stdcore::features::citations::{
        CitationAnswer, CitationEntry, CitationProvider, CitationProviderError, CitationRequest,
        CitationResult, ProviderCx, PROVIDERS_SLOT,
    };
    use flm_core::stdcore::{package, StdcoreConfig};

    /// Answers every key with an entry holding the key itself as plain text:
    /// the entry of `knuth84` reads `knuth84`.
    #[derive(Debug, Default)]
    pub struct EveryKey;

    impl CitationProvider for EveryKey {
        fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
            text_entry(cx, &request.key).map(|entry| Some(CitationAnswer::new(entry)))
        }
    }

    /// An entry holding `text` as characters, whatever they are.
    pub fn text_entry(cx: &ProviderCx<'_>, text: &str) -> Result<CitationEntry, CitationProviderError> {
        let source = Arc::new(FlmSource::new("").with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(cx.lang(), source, FragmentLayout::Inline, &ParseOptions::default())
            .map_err(|error| CitationProviderError::new(error.to_string()))?;
        let root = fragment.tree().root();
        let value = copy_pieces_into_value(&[ValuePiece::Text(text)], &root, ValueContent::Own, cx.lang().guard())
            .map_err(|error| CitationProviderError::new(error.to_string()))?;
        Ok(CitationEntry::new(value))
    }

    /// Answers the keys of its map, each entry FLM source parsed through the
    /// request's context under the origin `refs.flm`; "not mine" for any
    /// other key.
    #[derive(Debug, Default)]
    pub struct FromMap(pub BTreeMap<String, String>);

    impl FromMap {
        /// The provider of `entries`, pairs of a key and its FLM source.
        pub fn of(entries: &[(&str, &str)]) -> FromMap {
            FromMap(entries.iter().map(|(key, text)| (key.to_string(), text.to_string())).collect())
        }
    }

    impl CitationProvider for FromMap {
        fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
            let Some(text) = self.0.get(&request.key) else { return Ok(None) };
            let source = FlmSource::new(text.as_str()).with_origin(FlmOrigin::new("refs.flm"));
            cx.parse_entry(Arc::new(source)).map(|entry| Some(CitationAnswer::new(entry)))
        }
    }

    /// Fails for the keys it names, with the detail `offline`; "not mine"
    /// for any other key.
    #[derive(Debug, Default)]
    pub struct Failing(pub Vec<String>);

    impl CitationProvider for Failing {
        fn answer(&self, request: &CitationRequest, _cx: &ProviderCx<'_>) -> CitationResult {
            if self.0.contains(&request.key) {
                Err(CitationProviderError::new("offline"))
            } else {
                Ok(None)
            }
        }
    }

    /// Records every key it is asked about, then answers as the provider it
    /// wraps.
    #[derive(Debug, Clone, Default)]
    pub struct Asked(pub Arc<Mutex<Vec<String>>>);

    impl Asked {
        /// The keys asked so far, in order.
        pub fn keys(&self) -> Vec<String> {
            self.0.lock().expect("not poisoned").clone()
        }
    }

    /// A provider counting the requests it is given in `asked`, then
    /// answering as `inner`.
    pub struct Counting {
        pub asked: Asked,
        pub inner: Arc<dyn CitationProvider>,
    }

    impl CitationProvider for Counting {
        fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
            self.asked.0.lock().expect("not poisoned").push(request.key.clone());
            self.inner.answer(request, cx)
        }
    }

    /// The registry over `features` with `providers` contributed, in order.
    pub fn registry_over_with(
        features: Vec<Arc<dyn FlmFeature>>,
        providers: Vec<Arc<dyn CitationProvider>>,
    ) -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        for feature in features {
            builder.feature(feature);
        }
        for provider in providers {
            builder.contribute_to(PROVIDERS_SLOT, provider);
        }
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    /// The standard package under `config` with `providers` contributed, in
    /// order.
    pub fn registry_with(config: StdcoreConfig, providers: Vec<Arc<dyn CitationProvider>>) -> Arc<FlmRegistry> {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(config));
        builder.use_package(flm_core::math_passthrough::package());
        for provider in providers {
            builder.contribute_to(PROVIDERS_SLOT, provider);
        }
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        })
    }

    /// The standard package under `config`, every key answered by
    /// [`EveryKey`].
    pub fn registry(config: StdcoreConfig) -> Arc<FlmRegistry> {
        registry_with(config, vec![Arc::new(EveryKey)])
    }

    /// The registry over `features`, every key answered by [`EveryKey`].
    pub fn registry_over(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
        registry_over_with(features, vec![Arc::new(EveryKey)])
    }
}

// ============================================================================
// end citations
// ============================================================================

// ============================================================================
// naming: the tests' naming services, contributed to `flm.artifacts.namer`
// ============================================================================

/// Naming services of the tests' own, for the tests of the producers that ask
/// the naming service for a path during the run (the recomposed LaTeX's style
/// file, a compose template's files).
pub mod naming {
    use flm_core::core::artifact::{ArtifactNamer, ArtifactRef, NamingScope, RelPath};

    /// Names the entry artifact `job.EXT` and every other artifact `job-NAME.EXT`,
    /// `NAME` being its identity's name and `EXT` the extension it gives, else `bin`;
    /// a folder artifact is `job-NAME`, with no extension.
    pub struct JobNamer;

    impl ArtifactNamer for JobNamer {
        fn url_of(&self, artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
            let extension = artifact.extension.unwrap_or("bin");
            if artifact.folder {
                RelPath::new(format!("job-{}", artifact.id.name))
            } else if artifact.entry {
                RelPath::new(format!("job.{extension}"))
            } else {
                RelPath::new(format!("job-{}.{extension}", artifact.id.name))
            }
        }
    }

    /// Names every artifact at the one path it holds.
    pub struct FixedNamer(pub &'static str);

    impl ArtifactNamer for FixedNamer {
        fn url_of(&self, _artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
            RelPath::new(self.0)
        }
    }
}

// ============================================================================
// latex_rendered: helpers for the features' `tests/latex_rendered_<feature>.rs`
// ============================================================================

/// Helpers for the features' tests on the `latex-rendered` backend: a run of
/// its workflow that must report nothing, the document and the style file it
/// wrote, and a golden under `tests/goldens/latex-rendered/` holding both,
/// each under a marker line (the shape `tests/latex_rendered_compile.rs`
/// compiles with `lualatex`).
pub mod latex_rendered {
    use std::path::PathBuf;
    use std::sync::Arc;

    use flm_core::core::plan::Overrides;
    use flm_core::core::registry::FlmRegistry;
    use flm_core::core::run::Run;
    use flm_core::dump::golden::assert_text_golden;
    use flm_core::stdcore::backends::latex_rendered::{DOCUMENT, WORKFLOW, style_artifact};

    /// The run of the `latex-rendered` workflow over `texts` under
    /// `overrides`; any diagnostic but the Warning of an image whose size is
    /// not known ([`UNKNOWN_SIZE`](super::stdcore::UNKNOWN_SIZE)) is a test
    /// failure.
    pub fn render(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
        let run = super::stdcore::run_workflow(registry, WORKFLOW, texts, overrides);
        assert_eq!(
            super::stdcore::identifiers_but_unknown_size(&run),
            Vec::<String>::new(),
            "{}",
            run.diagnostics().render_all()
        );
        run
    }

    /// The entry artifact's text, the document.
    pub fn document(run: &Run) -> String {
        let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
            panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
        });
        let entry = set.entry().expect("the entry artifact");
        String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
    }

    /// The style file's text and its path.
    pub fn style(run: &Run) -> (String, String) {
        let set = run.table(DOCUMENT).expect("declared").expect("written");
        let style = style_artifact(set).expect("the style file");
        let text = String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
        let path =
            style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
        (text, path)
    }

    /// Asserts the run's document and style file against
    /// `tests/goldens/latex-rendered/<name>`, each under a marker line.
    pub fn assert_golden(name: &str, run: &Run) {
        let (style, path) = style(run);
        let text = format!(
            "% ===== latex-rendered:document =====\n{}% ===== latex-rendered:style {path} =====\n{style}",
            document(run)
        );
        let mut golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        golden.push("tests/goldens/latex-rendered");
        golden.push(name);
        assert_text_golden(golden, &text);
    }
}
