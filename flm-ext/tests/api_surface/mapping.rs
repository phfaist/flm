//! The mapping table: the generator's one table from the Rust types of the
//! inventory to their conversions and their TypeScript shapes, kept by hand.
//!
//! It holds the kind rows ([`KINDS`], one per handle kind), the sequences of
//! the lists of the typed API ([`LISTS`]), the lists of handles the interface
//! hands an extension with their reasons ([`HANDLE_LISTS`]), the opaque value
//! types ([`OPAQUE_TYPES`]), the type rows ([`TYPES`], one per leaf type that
//! crosses or that a re-export or a data type names), the tokens of the unit
//! enums ([`unit_enum_tokens`], evaluated through the crate), the frozen
//! objects of static constructors ([`STATIC_OBJECTS`]), the six lists
//! [`NOT_DISPATCHED`], [`OPTIONAL_IN_SCRIPTS`], [`CALL_OVERRIDES`],
//! [`EXTRA_METHODS`], [`HAND_ARMS`] and [`HAND_WRITTEN`], and the four lists
//! of the WebAssembly guest client ([`GUEST_OWN`], [`GUEST_HAND`],
//! [`GUEST_ADDED`], [`GUEST_OMITTED`]). The test
//! `the_mapping_table_matches_the_inventory` checks it against the inventory
//! in both directions.
//!
//! A type is matched against the rows first; when no row names it, the
//! structural rules of [`leaves`] apply, outermost first, and what is left
//! must have a row.

#![allow(dead_code, reason = "the emitters of the generator's outputs read every field of the table; this part of the generator reads some")]

use serde_json::Value;

use flm_ext::adapter::values::JsonForm;

/// How the dispatcher takes the receiver of a method of a handle kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    /// A shared borrow lent for the call, copied out of the entry
    /// (`*entry!(…)`).
    LentRef,
    /// An exclusive borrow lent for the call (`entry!(…)`).
    LentMut,
    /// A box the table owns (`&**entry!(…)`).
    Boxed,
    /// A value the table owns, cloned (`entry!(…).clone()`).
    Cloned,
    /// An object the binding lent in, held as an `Arc`.
    Arc,
    /// A `Copy` value the table owns.
    Copy,
    /// An entry recording how to ask for the object again (`TablePiece {
    /// out, name }`): every method is a hand-written arm.
    Recorded,
    /// A traversal the table owns (a `TraversalEntry`) of items of the
    /// type given, as the inventory writes a type, advanced through an
    /// exclusive borrow (`entry!(…)`) by the generated arm `pull(max)`, which
    /// answers the next items, at most `max`, fewer only at the end, and
    /// keeps a copy of them in the entry. Each of its methods consumes it and
    /// takes one more trailing argument, the count `unhanded` of the items of
    /// that group its binding has not handed over: its receiver is the
    /// traversal of the last `unhanded` kept items followed by the items not
    /// pulled yet (`TraversalEntry::rest`), and the entry is left with none.
    /// Its JavaScript class extends `Traversal`, its declaration is an
    /// iterable iterator of the items, and its guest struct an `Iterator` of
    /// `Result`s of them; the class and the struct send, last in each
    /// method's request, the number of the items of their group not handed
    /// over (`Traversal._release()`, `abi::Pulled::held`).
    Pulled(&'static str),
    /// A sequence the table owns of items of the type given, as the
    /// inventory writes a type, read through an exclusive borrow
    /// (`entry!(…)`): its methods by index, and the generated arm `pull(from,
    /// max, bytes)`, which answers the items from the index `from`, at most
    /// `max` of them and within `bytes` bytes of JSON text (the first
    /// whatever its size), through its type's `group`. Its handle crosses
    /// with its length; its JavaScript class extends `Sequence`, its
    /// declaration is an iterable of the items with `len()` and
    /// `is_empty()`, and its guest struct holds its length and iterates
    /// `Result`s of the items.
    Indexed(&'static str),
    /// A module handle: the free functions and the constructors.
    Module,
}

/// One handle kind.
#[derive(Debug, Clone, Copy)]
pub struct KindRow {
    /// The kind's name (`HandleKind`'s serde name).
    pub kind: &'static str,
    /// The trait or type it stands for; `None` for a module handle.
    pub rust: Option<&'static str>,
    /// The variant of `Entry` that holds it; `None` for a module handle.
    pub entry: Option<&'static str>,
    /// How the receiver is taken.
    pub receiver: Receiver,
    /// Whether every forwarding method of its class is guarded by `if
    /// (this._token !== null)`: the rehearsal's builder has a `null` token.
    pub rehearsable: bool,
}

impl KindRow {
    /// The type of the items of a traversal or of a sequence, which the kind
    /// answers through its arm `pull`; `None` for any other kind. A pull is
    /// the bindings' own: no item of `src/api/` declares it.
    pub fn pulls(&self) -> Option<&'static str> {
        match self.receiver {
            Receiver::Pulled(items) | Receiver::Indexed(items) => Some(items),
            _ => None,
        }
    }
}

/// A kind row.
const fn kind(kind: &'static str, rust: &'static str, entry: &'static str, receiver: Receiver) -> KindRow {
    KindRow { kind, rust: Some(rust), entry: Some(entry), receiver, rehearsable: false }
}

/// The kind row of a traversal whose items are of the type `items`.
const fn traversal(kind: &'static str, rust: &'static str, items: &'static str) -> KindRow {
    KindRow { kind, rust: Some(rust), entry: Some(rust), receiver: Receiver::Pulled(items), rehearsable: false }
}

/// The kind row of a sequence whose items are of the type `items`.
const fn sequence(kind: &'static str, rust: &'static str, items: &'static str) -> KindRow {
    KindRow { kind, rust: Some(rust), entry: Some(rust), receiver: Receiver::Indexed(items), rehearsable: false }
}

/// The 59 handle kinds, in the order of `HandleKind` (`src/adapter/handles.rs`).
pub const KINDS: &[KindRow] = &[
    KindRow { rehearsable: true, ..kind("feature_builder", "FeatureBuilder", "Builder", Receiver::LentMut) },
    kind("stage_cx", "StageCx", "StageCx", Receiver::LentRef),
    kind("stage_out", "StageOut", "StageOut", Receiver::LentMut),
    kind("table_piece", "TablePiece", "TablePiece", Receiver::Recorded),
    kind("ext_table_read", "ExtTableRead", "ExtTableRead", Receiver::Boxed),
    kind("trees_table", "TreesTable", "TreesTable", Receiver::Boxed),
    kind("node", "Node", "Node", Receiver::Boxed),
    kind("key_vals", "KeyVals", "KeyVals", Receiver::Boxed),
    kind("key_val_entry", "KeyValEntry", "KeyValEntry", Receiver::Boxed),
    kind("expand_cx", "ExpandCx", "ExpandCx", Receiver::LentMut),
    kind("expander", "Expander", "Expander", Receiver::LentMut),
    kind("user_definition", "UserDefinition", "UserDefinition", Receiver::Cloned),
    kind("mode_variant", "ModeVariant", "ModeVariant", Receiver::Boxed),
    kind("compiled_template", "CompiledTemplate", "CompiledTemplate", Receiver::Boxed),
    kind("placeholder_bindings", "PlaceholderBindings", "PlaceholderBindings", Receiver::Boxed),
    kind("staged", "Staged", "Staged", Receiver::Copy),
    kind("stage", "Stage", "Stage", Receiver::Arc),
    kind("expand", "Expand", "Expand", Receiver::Arc),
    kind("lowering_rule", "LoweringRule", "LoweringRule", Receiver::Cloned),
    kind("lower_cx", "LowerCx", "LowerCx", Receiver::LentMut),
    kind("lowerer", "Lowerer", "Lowerer", Receiver::LentMut),
    kind("lower", "Lower", "Lower", Receiver::Arc),
    kind("element_def", "ElementDef", "ElementDef", Receiver::Cloned),
    kind("rewrite_stage", "RewriteStage", "RewriteStage", Receiver::Arc),
    kind("rewrite_cx", "RewriteCx", "RewriteCx", Receiver::LentRef),
    kind("restager", "Restager", "Restager", Receiver::LentMut),
    kind("restaged_argument", "RestagedArgument", "RestagedArgument", Receiver::Boxed),
    kind("restaged_slot", "RestagedSlot", "RestagedSlot", Receiver::Boxed),
    kind("lowered_trees", "LoweredTrees", "LoweredTrees", Receiver::Boxed),
    kind("library_stage", "LibraryStage", "LibraryStage", Receiver::Cloned),
    kind("host_service", "HostService", "HostService", Receiver::Boxed),
    kind("files", "Files", "Files", Receiver::Boxed),
    kind("citation_provider", "CitationProvider", "CitationProvider", Receiver::Arc),
    kind("citation_entry", "CitationEntry", "CitationEntry", Receiver::Boxed),
    kind("provider_cx", "ProviderCx", "ProviderCx", Receiver::LentRef),
    kind("template_module", "TemplateModule", "TemplateModule", Receiver::Arc),
    kind("template_cx", "TemplateCx", "TemplateCx", Receiver::LentRef),
    KindRow { kind: "module", rust: None, entry: None, receiver: Receiver::Module, rehearsable: false },
    kind("referenceable", "Referenceable", "Referenceable", Receiver::Arc),
    kind("sectioning", "Sectioning", "Sectioning", Receiver::Arc),
    // The built language: lent to a late-init hook, or recorded as the
    // lowerer or the expander whose `lang()` answered it: every
    // function that takes it is a hand arm.
    kind("built_lang", "BuiltLang", "BuiltLang", Receiver::Recorded),
    kind("flm_diagnostics", "FlmDiagnostics", "FlmDiagnostics", Receiver::LentMut),
    kind("late_init", "LateInit", "LateInit", Receiver::Arc),
    kind("finalizer", "Finalizer", "Finalizer", Receiver::Arc),
    // The tables a reader answers typed: every method is a hand
    // arm, which reads the point back through the façade and asks a
    // recorded table of its lowerer again.
    kind("counters", "Counters", "Counters", Receiver::Recorded),
    kind("anchors", "Anchors", "Anchors", Receiver::Recorded),
    // A template language, resolved for the call from its durable token; no
    // method of its own.
    kind("template_language", "TemplateLanguage", "TemplateLanguage", Receiver::Cloned),
    kind("value_bindings", "ValueBindings", "ValueBindings", Receiver::Cloned),
    // A tree shared through an `Arc`; taken as an argument, it is copied out
    // of the table, as a node is.
    kind("tree", "Tree", "Tree", Receiver::Boxed),
    // The handles of the progress sheet: boxes the table owns; a method that
    // starts or ends a row takes the `flm-core` handle out of its façade.
    kind("progress_scope", "ProgressScope", "ProgressScope", Receiver::Boxed),
    kind("pending", "Pending", "Pending", Receiver::Boxed),
    kind("task", "Task", "Task", Receiver::Boxed),
    kind("counted", "Counted", "Counted", Receiver::Boxed),
    // The traversals: each pulled through its arm `pull`, its items lent as
    // they are pulled.
    traversal("walk", "Walk", "WalkEntry"),
    traversal("descendants", "Descendants", "Box<dyn Node>"),
    traversal("trees", "Trees", "(String, Box<dyn Tree>)"),
    // The sequences: their length crosses with their handle, their items
    // are pulled through their arm `pull`.
    sequence("rows", "Rows", "Value"),
    // The sequences a binding makes of a list of the typed API: their class
    // is the binding's own ([`LISTS`]).
    sequence("nodes", "Nodes", "Box<dyn Node>"),
    sequence("staged_nodes", "StagedNodes", "Staged"),
];

/// The kind row of the trait or type `rust`.
pub fn kind_of(rust: &str) -> Option<&'static KindRow> {
    KINDS.iter().find(|row| row.rust == Some(rust))
}

/// A sequence that a binding makes of a list of the typed API, which keeps
/// the list as a `Vec` (no item of `src/api/` stands for the sequence): its
/// kind row ([`Receiver::Indexed`], `rust` naming the class) holds its
/// entry, a `SequenceEntry` of the items (`src/adapter/handles.rs`). The
/// dispatcher has the arms `pull(from, max, bytes)` and `get(i)` (a copy of
/// the item, or `null`), generated; the JavaScript class extends `Sequence`
/// with `get(i)`, and the declaration is the class of a sequence, both in the
/// module `module`; the guest client's struct is written by hand in that
/// module's file of `src/guest/hand/`, since an extension also joins one of
/// its own ([`list`](ListRow::list) and `taken` are rows of [`GUEST_OWN`]).
/// Where the typed API takes several items, the declarations take the
/// sequence, or an array of items and sequences. Its class and its items are
/// its kind row's ([`ListRow::class`], [`ListRow::items`]).
#[derive(Debug, Clone, Copy)]
pub struct ListRow {
    /// The kind of its handle.
    pub kind: &'static str,
    /// The module of the surface whose stub, declarations and guest module
    /// hold the class.
    pub module: &'static str,
    /// The type with which the typed API takes several items (a slice, or
    /// the list type itself when it is taken by value): a row read as a
    /// sequence, or as a joined list of items and sequences.
    pub taken: &'static str,
    /// The class's description, which the stub and the declarations carry.
    pub doc: &'static str,
}

impl ListRow {
    /// Its kind row, a sequence's.
    fn row(&self) -> &'static KindRow {
        KINDS
            .iter()
            .find(|row| row.kind == self.kind && matches!(row.receiver, Receiver::Indexed(_)))
            .unwrap_or_else(|| panic!("the kind `{}` of `LISTS` must be a sequence of `KINDS`", self.kind))
    }

    /// Its class in the bindings, the guest's struct included: the `rust`
    /// of its kind row.
    pub fn class(&self) -> &'static str {
        self.row().rust.expect("a sequence's kind row names its class")
    }

    /// The type of its items: its kind row's.
    pub fn items(&self) -> &'static str {
        self.row().pulls().expect("a sequence's kind row names its items")
    }

    /// The list type of the typed API that crosses as the sequence, a `Vec`
    /// of its items: a row answered as one handle with its length.
    pub fn list(&self) -> String {
        format!("Vec<{}>", self.items())
    }
}

/// The sequences of the lists of the typed API.
pub const LISTS: &[ListRow] = &[
    ListRow {
        kind: "nodes",
        module: "techy/node",
        taken: "[Box<dyn Node>]",
        doc: "A sequence of nodes: several nodes in order, which a method returns as one object, with `len()`, `is_empty()`, `get(i)` \
              and iteration, which can be repeated. A function that takes several nodes takes a sequence, or an array of nodes and \
              sequences, joined in order.",
    },
    ListRow {
        kind: "staged_nodes",
        module: "lang/rewrites/userdefs",
        taken: "Vec<Staged>",
        doc: "A sequence of staged nodes: several staged nodes in order, which a method returns as one object, with `len()`, \
              `is_empty()`, `get(i)` and iteration, which can be repeated. A function that takes several staged nodes takes a \
              sequence, or an array of staged nodes and sequences, joined in order.",
    },
];

/// The list sequence of the kind `kind`, if it is one.
pub fn list_of_kind(kind: &str) -> Option<&'static ListRow> {
    LISTS.iter().find(|list| list.kind == kind)
}

/// The list sequence whose class is `class`, or whose list type
/// ([`ListRow::list`]) or taken type ([`ListRow::taken`]) is `class`, if
/// any.
pub fn list_of(class: &str) -> Option<&'static ListRow> {
    LISTS.iter().find(|list| list.class() == class || list.list() == class || list.taken == class)
}

/// A member of `src/api/` that hands an extension a list whose items hold
/// handles, with the reason the list stays one: its length is bounded by one
/// construct of the document, never by the document.
#[derive(Debug, Clone, Copy)]
pub struct HandleList {
    /// The trait or type (an enum's variant as `Enum::Variant`; for a free
    /// function, the module path).
    pub owner: &'static str,
    /// The method, the function or the field.
    pub member: &'static str,
    /// Why the list is bounded.
    pub reason: &'static str,
}

/// The lists of handles the interface hands an extension. Every other list
/// of handles crosses as a sequence ([`LISTS`]) or a traversal; the
/// generator refuses one that is neither.
pub const HANDLE_LISTS: &[HandleList] = &[
    HandleList {
        owner: "stdlib/features/math",
        member: "equation_lines",
        reason: "the lines of one display-math environment, as records; the content of each line is a sequence",
    },
    HandleList {
        owner: "stdlib/features/math",
        member: "line_frames",
        reason: "the frames of the lines of one display-math environment, as records; the content of each line is a sequence",
    },
    HandleList { owner: "KeyVals", member: "iter", reason: "the entries of one key-value argument" },
    HandleList {
        owner: "KeyValEntry",
        member: "value_pieces",
        reason: "the pieces of the value of one entry of a key-value argument, a node or a text each",
    },
    HandleList { owner: "LineShows::Number", member: "segments", reason: "the number of one line: one tree per level of its counter" },
    HandleList { owner: "Counters", member: "format", reason: "the number of one point: one segment per level of its counter" },
    HandleList {
        owner: "stdlib/features/math",
        member: "group_tex",
        reason: "the TeX of one formula: its TeX text between its text commands, and a hole for each of them",
    },
    HandleList {
        owner: "LineTex",
        member: "parts",
        reason: "the TeX of one equation line: its TeX text between its text commands, and a hole for each of them",
    },
    HandleList {
        owner: "ModeVariant",
        member: "defaults",
        reason: "the default values of the optional arguments of one definition, one per argument",
    },
];

/// The opaque value types (rule 3 of `ItemClass`): `api` structs
/// a binding cannot build from data. `DefinitionBindings` and `LoweringStage`
/// have no kind of their own ([`KINDLESS`]): their constructors answer a
/// `placeholder_bindings` and a `library_stage`.
pub const OPAQUE_TYPES: &[&str] = &[
    "UserDefinition",
    "LoweringRule",
    "ElementDef",
    "Staged",
    "DefinitionBindings",
    "LibraryStage",
    "LoweringStage",
    "TemplateLanguage",
    "ValueBindings",
    "Walk",
    "Descendants",
    "Trees",
    "Rows",
];

/// The opaque types with no kind of their own: only a constructor, which
/// answers an object of another kind.
pub const KINDLESS: &[&str] = &["DefinitionBindings", "LoweringStage"];

/// The TypeScript shape of a type row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ts {
    /// The text given.
    Text(&'static str),
    /// A unit enum: a union of its tokens and a frozen object of the same
    /// name (the tokens from [`unit_enum_tokens`]).
    Union,
    /// A data struct whose class the generator writes (rule 4).
    Class,
    /// A class written by hand: its text is [`HAND_WRITTEN`]'s.
    Hand,
    /// The class of a handle kind: the row's own name.
    Handle,
    /// Not offered to a script, for the reason in the row's note.
    NotOffered,
}

/// One leaf type.
#[derive(Debug, Clone, Copy)]
pub struct TypeRow {
    /// The type as the inventory writes it (normalized, borrows taken off).
    pub rust: &'static str,
    /// The type `FromArg` is implemented on, when the type is read from an
    /// argument (a borrowed type is read as its owned form).
    pub from_arg: Option<&'static str>,
    /// The type `IntoReply` is implemented on, when the type is answered.
    pub into_reply: Option<&'static str>,
    /// Its TypeScript shape.
    pub ts: Ts,
    /// What the row depends on: the reader or writer of `values.rs` the
    /// implementations wrap, or the reason the type is not offered.
    pub note: &'static str,
}

/// A row read and answered as the same type.
const fn both(rust: &'static str, ts: Ts, note: &'static str) -> TypeRow {
    TypeRow { rust, from_arg: Some(rust), into_reply: Some(rust), ts, note }
}

/// A row read only.
const fn read(rust: &'static str, ts: Ts, note: &'static str) -> TypeRow {
    TypeRow { rust, from_arg: Some(rust), into_reply: None, ts, note }
}

/// A row answered only.
const fn answered(rust: &'static str, ts: Ts, note: &'static str) -> TypeRow {
    TypeRow { rust, from_arg: None, into_reply: Some(rust), ts, note }
}

/// A row that crosses only inside another type, or is written by hand.
const fn inside(rust: &'static str, ts: Ts, note: &'static str) -> TypeRow {
    TypeRow { rust, from_arg: None, into_reply: None, ts, note }
}

/// A unit enum's row: both ways through its `JsonForm`.
const fn unit(rust: &'static str) -> TypeRow {
    both(rust, Ts::Union, "JsonForm (values.rs, string_form! or serde)")
}

/// The type rows, by group.
pub const TYPES: &[TypeRow] = &[
    // Scalars and data.
    read("str", Ts::Text("string"), "Args::text, passed by reference"),
    both("String", Ts::Text("string"), "Args::text; a string"),
    read("Option<String>", Ts::Text("string | null"), "the FromArg of Option<String>: null, or a string"),
    both("usize", Ts::Text("number"), "Args::index; a number"),
    answered("u32", Ts::Text("number"), "a number"),
    read("u64", Ts::Text("number"), "the FromArg of u64: a number from 0 up"),
    both("bool", Ts::Text("boolean"), "Args::flag; a boolean"),
    TypeRow {
        rust: "[u8]",
        from_arg: Some("Vec<u8>"),
        into_reply: None,
        ts: Ts::Text("string | { text: string } | { base64: string }"),
        note: "a form of an artifact's content (content_from_value): a text, its UTF-8 bytes, { text } or { base64 }",
    },
    both("Value", Ts::Text("any"), "the argument as it is; check_data on the reply"),
    // The enums with one string per variant.
    unit("Effect"),
    unit("Scope"),
    unit("CallableKind"),
    unit("FallbackKind"),
    unit("DefinitionKind"),
    unit("DefinitionModes"),
    unit("ArgLayout"),
    unit("BodyLayout"),
    unit("ExpandPolicy"),
    unit("Placement"),
    unit("Severity"),
    unit("NodeKindName"),
    unit("Mode"),
    unit("StandardTable"),
    unit("Arity"),
    unit("Unit"),
    read(
        "Contributed",
        Ts::Text("any"),
        "data, as it is, or a handle of kind citation_provider or template_module (FromArg for Contributed)",
    ),
    answered("HostHandle", Ts::Text("Files | HostService"), "lent as Entry::Files or Entry::HostService"),
    read("Vec<Mode>", Ts::Text("Mode[]"), "Mode's JsonForm, per item"),
    // The citation providers and the template modules.
    inside("CitationRequest", Ts::Text("interface CitationRequest { key: string }"), "citation_requests_to_value"),
    inside(
        "CitationProviderError",
        Ts::Text("interface CitationProviderError { error: string }"),
        "a result item `{ error }`, read by citation_results_from_value",
    ),
    inside(
        "CitationResult",
        Ts::Text("CitationAnswer | CitationProviderError | null"),
        "citation_results_from_value, one item per request",
    ),
    // The fault a feature's configuration reader answers (the entry `read_settings`).
    inside(
        "ReadError",
        Ts::Text("interface ReadError { path?: string; detail: string }"),
        "its serde form, the `error` of a configuration reader's answer, read by read_answer_from_value",
    ),
    inside(
        "CitationAnswer",
        Ts::Text("interface CitationAnswer { entry: CitationEntry; identity?: string | null; warnings?: string[] }"),
        "citation_results_from_value, the entry a handle resolved in the call's table",
    ),
    inside(
        "EntryPiece",
        Ts::Text(
            "type EntryPiece = { text: string } | { source: string } | { emphasized: EntryPiece[] } | { strong: EntryPiece[] } | { link: { address: string; content: EntryPiece[] } }",
        ),
        "its serde form, inside the list of build_entry",
    ),
    TypeRow {
        rust: "[EntryPiece]",
        from_arg: Some("Vec<EntryPiece>"),
        into_reply: None,
        ts: Ts::Text("EntryPiece[]"),
        note: "FromArg for Vec<EntryPiece>, each part in its serde form",
    },
    answered("Box<dyn CitationEntry>", Ts::Handle, "lent as Entry::CitationEntry"),
    inside("Accepts", Ts::Text("type Accepts = \"all\" | string[]"), "template_description_from_value, inside TemplateDescription"),
    inside(
        "TemplateDescription",
        Ts::Text("interface TemplateDescription { accepts: Accepts; requires?: string[]; config?: { [key: string]: any } }"),
        "template_description_from_value",
    ),
    inside(
        "TemplateFile",
        Ts::Text(
            "type TemplateFile = { name: string; extension: string; media_type: string; text: string } | { name: string; file: string; media_type?: string | null }",
        ),
        "template_files_from_value",
    ),
    inside(
        "TemplateContext",
        Ts::Text(
            "interface TemplateContext { html: boolean; regions: [string, string][]; pieces: PieceValue[]; metadata: any; config: { [key: string]: any }; artifacts: [string, string][] }",
        ),
        "template_context_to_value",
    ),
    inside(
        "PieceValue",
        Ts::Text("interface PieceValue { name: string; place: string; text: string; words: string }"),
        "template_context_to_value, inside TemplateContext",
    ),
    // The data structs and enums read or written by a function of values.rs.
    read("StageDecl", Ts::Class, "JsonForm for StageDecl"),
    read("Needs", Ts::Class, "JsonForm for Needs"),
    read("LoweringDecl", Ts::Class, "JsonForm for LoweringDecl"),
    inside(
        "UncheckedOutputDecl",
        Ts::Text("interface UncheckedOutputDecl { backend: string; written: string; reader: string }"),
        "JsonForm for LoweringDecl, inside LoweringDecl",
    ),
    read("DefaultMark", Ts::Text("\"everywhere\" | \"never\" | { on_backends: string[] }"), "JsonForm for DefaultMark"),
    both("ElementDef", Ts::Hand, "element_def_from_table; lent as Entry::ElementDef"),
    read("FallbackDef", Ts::Class, "fallback_def_from_table"),
    inside("ElementSpec", Ts::Class, "element_spec_from_value, inside ElementDef and FallbackDef"),
    read("StyleDef", Ts::Hand, "definition_parts_from_value (\"a style definition\"); the class is a fragment"),
    read("ScriptDef", Ts::Hand, "definition_parts_from_value (\"a script definition\"); the class is a fragment"),
    inside(
        "DefContent",
        Ts::Text("{ inline: string } | { file: string } | { url: string } | { table: string }"),
        "definition_part_from_value, inside StyleDef and ScriptDef",
    ),
    TypeRow {
        rust: "Definition",
        from_arg: Some("Definition"),
        into_reply: Some("&Definition"),
        ts: Ts::Hand,
        note: "definition_from_value; definition_to_value",
    },
    read(
        "Diagnostic",
        Ts::Text("interface Diagnostic { id: string; severity: Severity; message: string; data?: any; at?: Node }"),
        "diagnostic_from_value",
    ),
    both("Expansion", Ts::Text("\"default\" | { emit: StagedNodes | (Staged | StagedNodes)[] }"), "expansion_from_value; expansion_to_value"),
    both("Lowering", Ts::Text("\"default\" | { emit: StagedNodes | (Staged | StagedNodes)[] }"), "lowering_from_value; lowering_to_value"),
    read(
        "PresTree",
        Ts::Text("type PresTree = { element: string; payload?: any; children?: PresTree[] } | { build: Staged | StagedNodes | (Staged | StagedNodes)[] } | { text: string }"),
        "pres_tree_from_value",
    ),
    // The lists of staged nodes: a sequence (`LISTS`).
    both(
        "Vec<Staged>",
        Ts::Text("StagedNodes"),
        "lend_staged: lent as one Entry::StagedNodes, its length beside its handle; staged_list_from_table: a sequence, or a joined \
         list of staged nodes and sequences",
    ),
    both("Staged", Ts::Handle, "staged_from_table; lent as Entry::Staged"),
    read("Vec<(String, Vec<Staged>)>", Ts::Text("[string, StagedNodes | (Staged | StagedNodes)[]][]"), "the reader of the pres_slots arm, in values.rs"),
    answered("WalkEntry", Ts::Text("interface WalkEntry { addr: NodeAddr; node: Node }"), "walk_entry_to_value"),
    answered("NodeAddr", Ts::Text("interface NodeAddr { tree: string; node: number }"), "node_addr_to_value"),
    answered(
        "TableValue",
        Ts::Text("interface TableValue { ext?: ExtTableRead; trees?: TreesTable; lowered?: LoweredTrees; projection?: any; counters?: Counters; anchors?: Anchors }"),
        "table_value_to_value",
    ),
    answered("Span", Ts::Text("interface Span { start: number; end: number }"), "JsonForm for Span"),
    answered("CharsContent", Ts::Text("interface CharsContent { text: string | null; has_placeholder: boolean }"), "JsonForm for CharsContent"),
    answered("Option<(String, String)>", Ts::Text("[string, string] | null"), "Option, then the pair as a list"),
    answered("Vec<(String, String)>", Ts::Text("[string, string][]"), "each pair a list"),
    answered("Walk", Ts::Handle, "lent as Entry::Walk"),
    answered("Descendants", Ts::Handle, "lent as Entry::Descendants"),
    answered("Trees", Ts::Handle, "lent as Entry::Trees"),
    answered("Rows", Ts::Handle, "lent as Entry::Rows, its length beside its handle (sequence_to_value)"),
    // The lists of nodes: a sequence (`LISTS`).
    answered("Vec<Box<dyn Node>>", Ts::Text("Nodes"), "lend_nodes: lent as one Entry::Nodes, its length beside its handle"),
    TypeRow {
        rust: "[Box<dyn Node>]",
        from_arg: Some("Vec<Box<dyn Node>>"),
        into_reply: None,
        ts: Ts::Text("Nodes | (Node | Nodes)[]"),
        note: "nodes_from_value: a sequence, or a joined list of nodes and sequences, passed by reference",
    },
    answered(
        "EquationLine",
        Ts::Text(
            "interface EquationLine { span: Span | null; content: Nodes; labels: LabelUse[]; tag: Tag | null; nonumber: boolean; linebreak: LineBreak | null }",
        ),
        "equation_line_to_value",
    ),
    answered(
        "LabelUse",
        Ts::Text("interface LabelUse { text: string; span: Span; label: Label | null }"),
        "the IntoReply of values.rs; inside EquationLine (equation_line_to_value)",
    ),
    read("Option<Label>", Ts::Text("Label | null"), "the FromArg of Option<Label>: null, or a label"),
    TypeRow {
        rust: "[LabelUse]",
        from_arg: Some("Vec<LabelUse>"),
        into_reply: None,
        ts: Ts::Text("LabelUse[]"),
        note: "the FromArg of Vec<LabelUse>, passed by reference",
    },
    inside("Tag", Ts::Text("interface Tag { starred: boolean; content: Nodes }"), "equation_line_to_value, inside EquationLine"),
    inside("LineBreak", Ts::Text("interface LineBreak { spacing: string | null }"), "equation_line_to_value, inside EquationLine"),
    both(
        "LineFrame",
        Ts::Text("interface LineFrame { environment: Node; index: number; line: EquationLine; anchor: string | null; shows: LineShows }"),
        "line_frame_from_value; line_frame_to_value",
    ),
    inside(
        "LineShows",
        Ts::Text(
            "type LineShows = { kind: \"nothing\" } | { kind: \"number\"; segments: Tree[]; text: string } | { kind: \"tag\"; starred: boolean; content: Nodes }",
        ),
        "line_frame_to_value, inside LineFrame",
    ),
    inside("FrameReader", Ts::Text("StageCx | Lowerer"), "the hand-written arm line_frames"),
    answered("NormalizedLine", Ts::Text("interface NormalizedLine { text: string; trailing: string }"), "JsonForm for NormalizedLine"),
    answered("WrittenPiece", Ts::Text("type WrittenPiece = { node: Node } | { text: string }"), "written_piece_to_value"),
    answered("TexPart", Ts::Text("type TexPart = { tex: string } | { hole: TextHole }"), "tex_part_to_value"),
    inside("TextHole", Ts::Text("interface TextHole { command: string; node: Node }"), "tex_part_to_value, inside TexPart"),
    answered("LineTex", Ts::Text("interface LineTex { parts: TexPart[]; trailing: string }"), "line_tex_to_value"),
    TypeRow {
        rust: "ArgDef",
        from_arg: None,
        into_reply: Some("ArgDef"),
        ts: Ts::Hand,
        note: "written as data, with its code or its structured syntax (arg_def_to_value), which arg_defs_from_value reads back",
    },
    // The structured syntax of an argument: the kinds the host reads and
    // writes, a tack-on argument with its fields.
    inside(
        "ArgStdKind",
        Ts::Text(
            "{ syntax: \"mandatory\" } | { syntax: \"optional\" } | { syntax: \"star\" } | { syntax: \"marker\"; marker: string } | { syntax: \"delimited-group\"; delimiters: [string, string] } | { syntax: \"optional-delimited-group\"; delimiters: [string, string] } | { syntax: \"any-delimited-group\" } | { syntax: \"optional-any-delimited-group\" } | { syntax: \"braced-only\" } | { syntax: \"embellishments\"; markers: string[] } | { syntax: \"key-vals\" } | { syntax: \"chars-group\" } | { syntax: \"verbatim\"; delimiters?: [string, string] } | { syntax: \"tack-on\"; fields: TackOnField[] } | { syntax: \"terminated\"; terminator: string; repeat_char?: string; opener?: string; head?: string; raw?: boolean }",
        ),
        "JsonForm for ArgStdKind (serde), inside ArgDef: arg_def_to_value and arg_defs_from_value",
    ),
    inside(
        "TackOnField",
        Ts::Text("interface TackOnField { name: string; args?: ArgDef[] | string; repeatable?: boolean; docs?: string }"),
        "inside ArgStdKind, its structured syntax tack-on",
    ),
    inside("BodyDef", Ts::Hand, "body_def_from_value, inside SyntaxDef"),
    both("UserDefinition", Ts::Handle, "the entry cloned; lent as Entry::UserDefinition"),
    both("LoweringRule", Ts::Handle, "the entry cloned; lent as Entry::LoweringRule"),
    answered("Box<dyn ModeVariant>", Ts::Handle, "lent as Entry::ModeVariant"),
    answered("Box<dyn CompiledTemplate>", Ts::Handle, "lent as Entry::CompiledTemplate"),
    answered("Box<dyn PlaceholderBindings>", Ts::Handle, "lent as Entry::PlaceholderBindings"),
    // The placeholders a compiled template lists, and the values of the
    // extra placeholders a hook computes: `ExtraValues` is data, read as the
    // receiver of the hand-written arm `ExtraValues::over`; its class is
    // hand-written (`HAND_WRITTEN`).
    answered(
        "PlaceholderRef",
        Ts::Text("interface PlaceholderRef { written: string; target: PlaceholderTarget; substitution: string | null }"),
        "JsonForm for PlaceholderRef (answered_as_its_form)",
    ),
    inside(
        "PlaceholderTarget",
        Ts::Text("{ argument: number } | \"body\" | { extra: string }"),
        "JsonForm for PlaceholderTarget, inside PlaceholderRef",
    ),
    read("ExtraValues", Ts::Hand, "extra_values_from_value"),
    // The templates a handler compiles.
    both("TemplateLanguage", Ts::Handle, "the entry cloned (a durable token lent by the dispatcher); answered as its durable token"),
    answered("DurableTemplateFacade", Ts::Text("CompiledTemplate"), "answered as its durable token (durable_to_value)"),
    both("ValueBindings", Ts::Handle, "the entry cloned; lent as Entry::ValueBindings"),
    // Its class (`new TemplateSpec({ … })`) is hand-written
    // (`HAND_WRITTEN`); the row is its data form.
    read(
        "TemplateSpec",
        Ts::Text("interface TemplateSpec { text: string; mode: Mode; vocabulary: PlaceholderVocabulary; definition: string; label: string; origin_label: string; user_definition?: boolean; offset?: number }"),
        "template_spec_from_value",
    ),
    inside(
        "PlaceholderVocabulary",
        Ts::Text("interface PlaceholderVocabulary { arity: number; names?: (string | null)[]; body?: boolean; extras?: { name: string; substitution: boolean }[] }"),
        "template_spec_from_value, inside TemplateSpec",
    ),
    answered("Box<dyn KeyVals>", Ts::Handle, "lent as Entry::KeyVals"),
    answered("Box<dyn KeyValEntry>", Ts::Handle, "lent as Entry::KeyValEntry"),
    read("Arc<dyn Stage>", Ts::Text("Stage"), "the entry's Arc, cloned (Entry::Stage)"),
    read("Arc<dyn Expand>", Ts::Text("Expand"), "expand_from_table"),
    read("Arc<dyn Lower>", Ts::Text("Lower"), "lower_from_table"),
    // The data enums with static constructors, crossing inside the above.
    read(
        "Aspect",
        Ts::Text("{ BlockRoleAspect: BlockRole } | { Expand: Expand } | { Referenceable: Referenceable } | { Sectioning: Sectioning }"),
        "aspect_from_value, inside ElementSpec and taken by FeatureBuilder.attach",
    ),
    inside(
        "SyntaxDef",
        Ts::Text(
            "{ macro: { args: ArgDef[] | string } } | { environment: { args: ArgDef[] | string; body: BodyDef } } | { specials: { trigger: string; args: ArgDef[] | string } } | \"synthesized\"",
        ),
        "syntax_def_from_value",
    ),
    inside(
        "BlockRole",
        Ts::Text("\"block\" | { inline: InlineSpacing } | \"displayed\" | { \"block-decorator\": Placement }"),
        "JsonForm for BlockRole",
    ),
    inside("InlineSpacing", Ts::Text("{ absorb_before: boolean; absorb_after: boolean }"), "JsonForm for InlineSpacing"),
    // Not offered: re-exports under api/ with no JavaScript form.
    inside("FixedBlockRole", Ts::NotOffered, "the aspect { BlockRoleAspect: role } holds the block role itself"),
    inside("NotCharacters", Ts::NotOffered, "an error, rendered as ExtError::Core of kind not-characters"),
    inside("NodeRegion", Ts::NotOffered, "taken by lower_region only, which is not dispatched"),
    // The artifacts of an output: plain data, re-exported; an
    // artifact crosses as the item of `TablePiece::push`, read by
    // artifact_from_value.
    inside("Artifact", Ts::Hand, "artifact_from_value, the item of TablePiece::push; the class is a fragment"),
    inside("ArtifactId", Ts::Hand, "artifact_from_value, inside Artifact; the class is a fragment"),
    inside(
        "ArtifactData",
        Ts::Text("{ bytes: string | { text: string } | { base64: string } } | { files: [string, string | { text: string } | { base64: string }][] }"),
        "artifact_from_value, inside Artifact",
    ),
    // The backends and the outputs: a backend's stages are
    // objects of the kinds `library_stage` and `stage`, read from the table
    // by backend_def_from_table; an output is plain data.
    read("BackendDef", Ts::Hand, "backend_def_from_table, its stages taken out of the table; the class is a fragment"),
    inside("BackendStage", Ts::Text("LibraryStage | Stage"), "backend_def_from_table, inside BackendDef"),
    answered("LibraryStage", Ts::Handle, "lent as Entry::LibraryStage; read inside BackendDef"),
    read("OutputDef", Ts::Hand, "JsonForm for OutputDef; the class is a fragment"),
    // The rewrite stages: the declaration is data; the answer
    // of a handler is written as a lowering is; the bundles a restager
    // answers are lent as their entries and taken out of the table by
    // `restage_invocation`, each once.
    read("RewriteDecl", Ts::Class, "JsonForm for RewriteDecl"),
    both("Restage", Ts::Text("\"descend\" | { emit: StagedNodes | (Staged | StagedNodes)[] }"), "restage_from_value; restage_to_value"),
    read("Arc<dyn RewriteStage>", Ts::Text("RewriteStage"), "the entry's Arc, cloned (Entry::RewriteStage)"),
    answered("Box<dyn RestagedArgument>", Ts::Handle, "lent as Entry::RestagedArgument"),
    answered("Box<dyn RestagedSlot>", Ts::Handle, "lent as Entry::RestagedSlot"),
    read(
        "Vec<Box<dyn RestagedArgument>>",
        Ts::Text("RestagedArgument[]"),
        "bundles_from_table: each entry taken out of the table once",
    ),
    read("Vec<Box<dyn RestagedSlot>>", Ts::Text("RestagedSlot[]"), "bundles_from_table: each entry taken out of the table once"),
    inside(
        "DurableOwner",
        Ts::NotOffered,
        "the sealed trait of durable_template, a Rust handler's route to a kept token; a script passes the token itself",
    ),
    // The data of the handler aspects (`flm:model/aspects`), answered by a
    // script's `Referenceable` and `Sectioning` and read by the binding's
    // aspect objects.
    inside("u32", Ts::Text("number"), "a number; read by the binding's aspect objects"),
    inside(
        "RefPoint",
        Ts::Text(
            "interface RefPoint { kind: string; labels: Label[]; computed: number; malformed: MalformedLabel[]; anchor: AnchorPolicy; number: NumberSource | null; content: ContentRef | null; accepts: AcceptedRefTypes }",
        ),
        "ref_point_from_value and ref_point_to_core (adapter/aspects.rs); answered by Referenceable.points",
    ),
    answered(
        "MalformedLabel",
        Ts::Text("interface MalformedLabel { text: string; span: Span; position: number }"),
        "the IntoReply of values.rs (point_labels); inside RefPoint (ref_point_from_value)",
    ),
    inside(
        "NumberSource",
        Ts::Text("interface NumberSource { counter: string; instances: Nodes | (Node | Nodes)[]; style: string; advance: number[]; number_format: string | null }"),
        "inside RefPoint",
    ),
    inside(
        "AnchorPolicy",
        Ts::Text(
            "type AnchorPolicy = \"from_first_label\" | { explicit: string } | { slug: { prefix: string; content: ContentRef } } | { from_value: { prefix: string } } | { auto: { prefix: string } }",
        ),
        "inside RefPoint",
    ),
    inside(
        "ContentRef",
        Ts::Text("type ContentRef = { argument: number } | { argument_named: string } | \"body\" | { node: Node } | { synthesized: Node }"),
        "inside RefPoint; answered by Sectioning.toc_title",
    ),
    inside("AcceptedRefTypes", Ts::Text("type AcceptedRefTypes = \"any\" | { these: string[] }"), "inside RefPoint"),
    both(
        "Label",
        Ts::Text("interface Label { ref_type: string; ref_label: string }"),
        "the FromArg (as Option<Label>) and IntoReply of values.rs; inside RefPoint (ref_point_from_value)",
    ),
    inside(
        "PointId",
        Ts::Text("interface PointId { addr: NodeAddr; index: number }"),
        "point_id_to_value writes it; the Counters and Anchors façades take it and read it back",
    ),
    // The builder's rows.
    read("ScopedPackageDef", Ts::Class, "scoped_package_def_from_table"),
    read("Arc<dyn LateInit>", Ts::Text("LateInit"), "the entry's Arc, cloned (Entry::LateInit)"),
    read("Arc<dyn Finalizer>", Ts::Text("Finalizer"), "the entry's Arc, cloned (Entry::Finalizer)"),
    // The numbering library: the declarations are flm-core's own,
    // read by the service slots' readers (adapter/slots.rs) from the data
    // `contribute_to`, `contribute_first_to` and `peer_slot` take
    // (`Contributed::Data`, routed by the slot's name); their classes are the
    // binding's.
    answered(
        "CounterRecord",
        Ts::Text("interface CounterRecord { counter: string; value: number[]; style: string; instance: NodeAddr | null }"),
        "counter_record_to_value",
    ),
    answered("NumberSegment", Ts::Text("interface NumberSegment { content: Tree; link: PointId | null }"), "the IntoReply of values.rs"),
    read(
        "FormatOptions",
        Ts::Text("interface FormatOptions { variant?: string | null; with_word?: boolean; with_delimiters?: boolean }"),
        "format_options_from_value",
    ),
    inside(
        "CounterDecl",
        Ts::Text("interface CounterDecl { name: string; default_style: string; docs?: string | null; within?: Within | null; nesting?: WithinJoinerSpec | null }"),
        "the reader of flm.numbering.counters (adapter/slots.rs)",
    ),
    inside(
        "CounterStyleDecl",
        Ts::Text(
            "interface CounterStyleDecl { name: string; number_format?: NumberTemplate | null; sub_levels?: any[] | null; prefix_words?: PrefixWords | null; delimiters?: Delimiters | null; join?: any; name_in_link?: boolean | null; repeat_parents_in_range?: boolean | null; ranges?: boolean | null; number_writer?: string | null }",
        ),
        "the reader of flm.numbering.styles (adapter/slots.rs), each field by its serde form",
    ),
    inside("ParentSetDecl", Ts::Text("interface ParentSetDecl { name: string; counters: string[] }"), "the reader of flm.numbering.parent-sets (adapter/slots.rs)"),
    inside(
        "Within",
        Ts::Text("interface Within { parents: Parents; joiner: WithinJoinerSpec; no_restart: boolean }"),
        "its serde form, inside CounterDecl",
    ),
    inside("Parents", Ts::Text("type Parents = { one: string } | { any: string[] } | { set: string }"), "its serde form, inside Within"),
    inside("WithinJoinerSpec", Ts::Text("type WithinJoinerSpec = any"), "its serde form, inside Within and CounterDecl"),
    inside("PrefixWords", Ts::Text("interface PrefixWords { base: PluralWords; variants?: { [name: string]: PluralWords } }"), "its serde form, inside CounterStyleDecl"),
    inside("PluralWords", Ts::Text("interface PluralWords { one: string; other: string }"), "its serde form, inside PrefixWords"),
    inside("Delimiters", Ts::Text("interface Delimiters { open: string; close: string }"), "its serde form, inside CounterStyleDecl"),
    inside("NumberTemplate", Ts::Text("type NumberTemplate = { formats: string } | { initial: string }"), "its serde form, inside CounterStyleDecl"),
    // The LaTeX family: the payloads are given as data to `pres` and
    // `pres_slots` (adapter/payloads.rs reads them through their
    // `from_projection`).
    unit("LatexArgShape"),
    inside("LatexDefinitionKind", Ts::Text("\"macro\" | \"environment\" | \"declaration\""), "its projection, inside LatexDefinitionPayload"),
    inside(
        "LatexMacroPayload",
        Ts::Text("interface LatexMacroPayload { name: string; args?: LatexArgShape[]; packed?: boolean }"),
        "LatexMacroPayload::from_projection (adapter/payloads.rs)",
    ),
    inside(
        "LatexEnvironmentPayload",
        Ts::Text("interface LatexEnvironmentPayload { name: string; args?: LatexArgShape[]; verbatim: boolean; packed?: boolean; body?: boolean }"),
        "LatexEnvironmentPayload::from_projection (adapter/payloads.rs)",
    ),
    inside("LatexRawPayload", Ts::Text("interface LatexRawPayload { text: string }"), "LatexRawPayload::from_projection (adapter/payloads.rs)"),
    inside(
        "LatexDefinitionPayload",
        Ts::Text("interface LatexDefinitionPayload { kind: LatexDefinitionKind; name: string; args: number; [field: string]: any }"),
        "LatexDefinitionPayload::from_projection (adapter/payloads.rs)",
    ),
    inside(
        "LatexPreambleDef",
        Ts::Text("interface LatexPreambleDef { name: string; content: string; docs?: string | null; requires?: string[] | null }"),
        "the reader of flm.latex.preamble (adapter/slots.rs)",
    ),
    inside(
        "LatexPackageDef",
        Ts::Text("interface LatexPackageDef { id: string; package: string; options: string[]; docs: string }"),
        "the reader of flm.latex.packages (adapter/slots.rs), its serde form",
    ),
];

/// The type row of `rust`.
pub fn row(rust: &str) -> Option<&'static TypeRow> {
    TYPES.iter().find(|row| row.rust == rust)
}

/// The tokens of the unit enum `rust`, variant by variant, evaluated through
/// the crate's own conversion ([`JsonForm::to_value`]): the variants are
/// named here, the tokens come from `values.rs`.
pub fn unit_enum_tokens(rust: &str) -> Option<Vec<(&'static str, Value)>> {
    macro_rules! tokens {
        ($($name:literal => $type:path: [$($variant:ident),+ $(,)?]),+ $(,)?) => {
            match rust {
                $($name => Some(vec![$((stringify!($variant), JsonForm::to_value(&<$type>::$variant))),+]),)+
                _ => None,
            }
        };
    }
    use flm_ext::api;
    tokens!(
        "Effect" => api::core::stage::Effect: [Compute, Check, Io],
        "Scope" => api::core::stage::Scope: [PerTree, Document],
        "CallableKind" => api::lang::mint::CallableKind: [Macro, Environment, Specials],
        "FallbackKind" => api::lang::mint::FallbackKind: [Macro, Environment],
        "DefinitionKind" => api::lang::rewrites::userdefs::DefinitionKind: [Macro, Environment, Specials],
        "DefinitionModes" => api::lang::rewrites::userdefs::DefinitionModes: [Both, Text, Math],
        "ArgLayout" => api::lang::mint::ArgLayout: [Inline, Blocks, InlineOrBlocks],
        "BodyLayout" => api::lang::mint::BodyLayout: [Inline, Blocks, FromRole],
        "ExpandPolicy" => api::lang::rewrites::userdefs::ExpandPolicy: [Always, PerPlan],
        "Placement" => api::model::aspects::Placement: [Leads, Trails, Free],
        "Severity" => api::techy::error::Severity: [Error, Warning, Note],
        "NodeKindName" => api::techy::node::NodeKindName: [Chars, Group, Callable, Comment, List],
        "Mode" => api::lang::mint::Mode: [Text, Math],
        "StandardTable" => api::core::registry::StandardTable: [LoweredTrees, LoweredPieces, Regions, ArtifactSet],
        "Arity" => api::core::services::Arity: [Single, OrderedMany],
        "Unit" => api::progress::Unit: [Items, Bytes],
        "LatexArgShape" => api::stdlib::backends::latex::LatexArgShape: [Star, Optional, Mandatory, EscVerb],
    )
}

/// One member of a frozen object of static constructors.
#[derive(Debug, Clone, Copy)]
pub struct Member {
    /// The member's name.
    pub name: &'static str,
    /// The member as the stub's object literal writes it.
    pub js: &'static str,
    /// The member as the declarations write it.
    pub ts: &'static str,
}

/// The frozen object of static constructors of a data enum, and of
/// `InlineSpacing`, whose object holds a constant.
#[derive(Debug, Clone, Copy)]
pub struct StaticObject {
    /// The type.
    pub rust: &'static str,
    /// The reader of `values.rs` whose shape the members write.
    pub reader: &'static str,
    /// The members, in order: a constructor per variant, a token per unit
    /// variant, the associated constants.
    pub members: &'static [Member],
}

/// A member.
const fn member(name: &'static str, js: &'static str, ts: &'static str) -> Member {
    Member { name, js, ts }
}

/// The frozen objects: `Restage`, the seven data enums, then
/// `InlineSpacing`.
pub const STATIC_OBJECTS: &[StaticObject] = &[
    StaticObject {
        rust: "Restage",
        reader: "restage_from_value",
        members: &[
            member("Descend", "Descend: \"descend\"", "Descend: \"descend\""),
            member("Emit", "Emit(staged) { return { emit: staged }; }", "Emit(staged: StagedNodes | (Staged | StagedNodes)[]): { emit: StagedNodes | (Staged | StagedNodes)[] }"),
        ],
    },
    StaticObject {
        rust: "Lowering",
        reader: "lowering_from_value",
        members: &[
            member("Default", "Default: \"default\"", "Default: \"default\""),
            member("Emit", "Emit(staged) { return { emit: staged }; }", "Emit(staged: StagedNodes | (Staged | StagedNodes)[]): { emit: StagedNodes | (Staged | StagedNodes)[] }"),
        ],
    },
    StaticObject {
        rust: "Expansion",
        reader: "expansion_from_value",
        members: &[
            member("Default", "Default: \"default\"", "Default: \"default\""),
            member("Emit", "Emit(staged) { return { emit: staged }; }", "Emit(staged: StagedNodes | (Staged | StagedNodes)[]): { emit: StagedNodes | (Staged | StagedNodes)[] }"),
        ],
    },
    StaticObject {
        rust: "Aspect",
        reader: "aspect_from_value",
        members: &[
            member(
                "BlockRoleAspect",
                "BlockRoleAspect(role) { return { BlockRoleAspect: role }; }",
                "BlockRoleAspect(role: BlockRole): { BlockRoleAspect: BlockRole }",
            ),
            member("Expand", "Expand(hook) { return { Expand: hook }; }", "Expand(hook: Expand): { Expand: Expand }"),
            member(
                "Referenceable",
                "Referenceable(aspect) { return { Referenceable: aspect }; }",
                "Referenceable(aspect: Referenceable): { Referenceable: Referenceable }",
            ),
            member(
                "Sectioning",
                "Sectioning(aspect) { return { Sectioning: aspect }; }",
                "Sectioning(aspect: Sectioning): { Sectioning: Sectioning }",
            ),
        ],
    },
    StaticObject {
        rust: "SyntaxDef",
        reader: "syntax_def_from_value",
        members: &[
            member("Macro", "Macro(args) { return { macro: { args } }; }", "Macro(args: ArgDef[] | string): { macro: { args: ArgDef[] | string } }"),
            member(
                "Environment",
                "Environment(args, body) { return { environment: { args, body } }; }",
                "Environment(args: ArgDef[] | string, body: BodyDef): { environment: { args: ArgDef[] | string; body: BodyDef } }",
            ),
            member(
                "Specials",
                "Specials(trigger, args) { return { specials: { trigger, args } }; }",
                "Specials(trigger: string, args: ArgDef[] | string): { specials: { trigger: string; args: ArgDef[] | string } }",
            ),
            member("Synthesized", "Synthesized: \"synthesized\"", "Synthesized: \"synthesized\""),
        ],
    },
    StaticObject {
        rust: "DefaultMark",
        reader: "JsonForm for DefaultMark",
        members: &[
            member("Everywhere", "Everywhere: \"everywhere\"", "Everywhere: \"everywhere\""),
            member("Never", "Never: \"never\"", "Never: \"never\""),
            member(
                "OnBackends",
                "OnBackends(backends) { return { on_backends: backends }; }",
                "OnBackends(backends: string[]): { on_backends: string[] }",
            ),
        ],
    },
    StaticObject {
        rust: "BlockRole",
        reader: "JsonForm for BlockRole",
        members: &[
            member("Block", "Block: \"block\"", "Block: \"block\""),
            member("Inline", "Inline(spacing) { return { inline: spacing }; }", "Inline(spacing: InlineSpacing): { inline: InlineSpacing }"),
            member("Displayed", "Displayed: \"displayed\"", "Displayed: \"displayed\""),
            member(
                "BlockDecorator",
                "BlockDecorator(placement) { return { \"block-decorator\": placement }; }",
                "BlockDecorator(placement: Placement): { \"block-decorator\": Placement }",
            ),
            // The stub defines `BlockRole` before `InlineSpacing` (the module's
            // order), so the value is written out rather than read from
            // `InlineSpacing.CONTENT`.
            member(
                "DEFAULT",
                "DEFAULT: Object.freeze({ inline: Object.freeze({ absorb_before: false, absorb_after: false }) })",
                "DEFAULT: { inline: InlineSpacing }",
            ),
        ],
    },
    StaticObject {
        rust: "DefContent",
        reader: "definition_part_from_value",
        members: &[
            member("Inline", "Inline(text) { return { inline: text }; }", "Inline(text: string): { inline: string }"),
            member("File", "File(path) { return { file: path }; }", "File(path: string): { file: string }"),
            member("Url", "Url(address) { return { url: address }; }", "Url(address: string): { url: string }"),
            member("Table", "Table(name) { return { table: name }; }", "Table(name: string): { table: string }"),
        ],
    },
    StaticObject {
        rust: "InlineSpacing",
        reader: "JsonForm for InlineSpacing",
        members: &[member(
            "CONTENT",
            "CONTENT: Object.freeze({ absorb_before: false, absorb_after: false })",
            "CONTENT: InlineSpacing",
        )],
    },
];

/// A method or function left out of the dispatcher, the stub and the
/// declarations, with the reason.
#[derive(Debug, Clone, Copy)]
pub struct Exclusion {
    /// The trait or type (or, for a free function, the module path).
    pub owner: &'static str,
    /// The method.
    pub method: &'static str,
    /// Why it is left out.
    pub reason: &'static str,
}

/// An exclusion.
const fn excluded(owner: &'static str, method: &'static str, reason: &'static str) -> Exclusion {
    Exclusion { owner, method, reason }
}

/// The reason shared by the readers of a lowering rule and of an element
/// declaration.
const RUST_READER: &str = "a reader for Rust callers";

/// The methods not dispatched.
pub const NOT_DISPATCHED: &[Exclusion] = &[
    excluded("Lowerer", "lower_region", "an address crosses as data that nothing takes back"),
    excluded("ExtTableRead", "items_at", "reached through `Lowerer.get_at`"),
    excluded("Rows", "len", "a sequence's length crosses with its handle: each binding answers it"),
    excluded("Rows", "is_empty", "a sequence's length crosses with its handle: each binding answers it"),
    excluded("Rows", "iter", "each binding iterates a sequence by its pulls"),
    excluded("LoweringRule", "name", RUST_READER),
    excluded("LoweringRule", "element", RUST_READER),
    excluded("LoweringRule", "mark", RUST_READER),
    excluded("LoweringRule", "description", RUST_READER),
    excluded("ElementDef", "new", "built from data: the stub's `ElementDef` holds the fields itself"),
    excluded("ElementDef", "owner", "a reader for Rust callers; the stub's `ElementDef` answers its fields itself"),
    excluded("ElementDef", "names", "a reader for Rust callers; the stub's `ElementDef` answers its fields itself"),
    excluded("ElementDef", "syntax", "a reader for Rust callers; the stub's `ElementDef` answers its fields itself"),
    excluded("ElementDef", "element", "a reader for Rust callers; the stub's `ElementDef` answers its fields itself"),
    excluded("Diagnostic", "new", "a diagnostic is a plain object"),
    excluded("Diagnostic", "error", "a diagnostic is a plain object"),
    excluded("Diagnostic", "warning", "a diagnostic is a plain object"),
    excluded("Diagnostic", "note", "a diagnostic is a plain object"),
    excluded("Diagnostic", "with_data", "a diagnostic is a plain object"),
    excluded("Diagnostic", "at", "a diagnostic is a plain object"),
    excluded("Aspect", "kind", "a reader for Rust callers; an aspect is a plain object"),
    excluded("Expand", "policy", "an optional method of the script's object, not a method of a handle"),
    excluded("CitationAnswer", "new", "an answer is a plain object"),
    excluded("CitationAnswer", "with_identity", "an answer is a plain object"),
    excluded("CitationAnswer", "with_warning", "an answer is a plain object"),
    excluded("TemplateLanguage", "token", "a reader for Rust callers; a script's template language is its token"),
];

/// A method of an extension-side trait that a script's object may leave out
/// although the Rust method has no default body, because the stub supplies
/// the default; the declarations write it optional (`needs?()`), as they
/// write a method with a default body.
#[derive(Debug, Clone, Copy)]
pub struct Optional {
    /// The extension-side trait.
    pub owner: &'static str,
    /// The method.
    pub method: &'static str,
    /// What the stub supplies.
    pub reason: &'static str,
}

/// The methods optional on a script's object (rule 2 of `ItemClass`).
pub const OPTIONAL_IN_SCRIPTS: &[Optional] = &[
    Optional {
        owner: "Stage",
        method: "needs",
        reason: "`FeatureBuilder.stage` supplies `new Needs()` when the object has no `needs`",
    },
    Optional {
        owner: "CitationProvider",
        method: "answer",
        reason: "the binding calls `answer_all` when the object has it, else `answer` once per request",
    },
];

/// Whether `owner::method` is in [`OPTIONAL_IN_SCRIPTS`].
pub fn is_optional_in_scripts(owner: &str, method: &str) -> bool {
    OPTIONAL_IN_SCRIPTS.iter().any(|entry| entry.owner == owner && entry.method == method)
}

/// Whether `owner::method` is in [`NOT_DISPATCHED`].
pub fn is_excluded(owner: &str, method: &str) -> bool {
    NOT_DISPATCHED.iter().any(|entry| entry.owner == owner && entry.method == method)
}

/// A generated arm that calls something other than the façade's method.
#[derive(Debug, Clone, Copy)]
pub struct Override {
    /// The handle kind.
    pub kind: &'static str,
    /// The method.
    pub method: &'static str,
    /// The call the arm makes on the receiver, the arguments named `a0`, …
    pub call: &'static str,
    /// Why.
    pub reason: &'static str,
}

/// The overrides.
pub const CALL_OVERRIDES: &[Override] = &[];

/// A dispatcher-only method or function with a hand-written arm, its
/// signature written here for the stub and the declarations.
#[derive(Debug, Clone, Copy)]
pub struct Extra {
    /// The handle kind of a method, or the module path of a free function.
    pub owner: &'static str,
    /// The method's or the function's name.
    pub name: &'static str,
    /// The TypeScript signature.
    pub ts: &'static str,
    /// The stub's method, when it does not forward to the dispatcher under
    /// its own name (`None`: it forwards, its parameters those of [`ts`](Extra::ts)).
    pub js: Option<&'static str>,
    /// Why the arm is hand-written.
    pub reason: &'static str,
    /// The Rust signature of the guest client's method or function
    /// (`flm_ext::guest`), in the guest's types: a façade borrowed or boxed
    /// is the struct of its handle kind, a type of [`GUEST_OWN`] the guest's
    /// own, lifetimes dropped, the result a `Result<_, ExtError>`.
    pub rust: &'static str,
    /// The typed body of the guest client's method or function (the cargo
    /// feature `native`): the lines inside `abi::typed(…, |table| { … })`,
    /// calling the function of `dispatch.rs`'s module `extra` that the
    /// hand-written arm calls, the guest's values converted as a generated
    /// typed body converts them; `None` for a method whose stub has a body of
    /// its own ([`js`](Extra::js)), which the guest forwards likewise.
    pub typed: Option<&'static str>,
}

/// The extras.
pub const EXTRA_METHODS: &[Extra] = &[
    Extra {
        owner: "stage_out",
        name: "table",
        ts: "table(name: string): TablePiece",
        js: None,
        reason: "answers a borrow of the receiver, which cannot be lent beside it; the entry `TablePiece { out, name }` records how to ask again",
        rust: "fn table(&mut self, name: &str) -> Result<TablePiece, ExtError>",
        typed: Some("Ok(TablePiece { handle: extra::stage_out_table(table, self.handle, name)? })"),
    },
    Extra {
        owner: "table_piece",
        name: "push",
        ts: "push(item: any): void",
        js: None,
        reason: "resolves the piece anew through the output's entry on every push",
        rust: "fn push(&mut self, item: Value) -> Result<(), ExtError>",
        typed: Some("extra::table_piece_push(table, self.handle, item)"),
    },
    Extra {
        owner: "lowerer",
        name: "get_at",
        ts: "get_at(name: string, node: Node): any[] | null",
        js: None,
        reason: "no façade method: `addr_of`, then `get`, then `items_at`, composed in the dispatcher",
        rust: "fn get_at(&self, name: &str, node: &Node) -> Result<Option<Vec<Value>>, ExtError>",
        typed: Some("let node = IntoApi::into_api(node, table)?;\n\
                     extra::lowerer_get_at(table, self.handle, name, &*node)"),
    },
    Extra {
        owner: "lowerer",
        name: "pres_tree",
        ts: "pres_tree(cx: LowerCx, tree: PresTree): StagedNodes",
        js: Some("pres_tree(cx, tree) { return pres_tree(this, cx, tree); }"),
        reason: "a stub-only convenience forwarding to the free function `pres_tree`",
        rust: "fn pres_tree(&mut self, cx: &mut LowerCx, tree: &PresTree) -> Result<StagedNodes, ExtError>",
        typed: None,
    },
    Extra {
        owner: "stdlib/features/math",
        name: "line_frames",
        ts: "line_frames(reader: StageCx | Lowerer, anchors: boolean, counters: boolean, node: Node): LineFrame[]",
        js: None,
        reason: "its first argument is one of two kinds, the reader borrowing the entry across the call",
        rust: "fn line_frames(reader: FrameReader, anchors: bool, counters: bool, node: &Node) -> Result<Vec<LineFrame>, ExtError>",
        typed: Some("let node = IntoApi::into_api(node, table)?;\n\
                     let frames = extra::line_frames(table, reader.handle(), anchors, counters, &*node)?;\n\
                     FromApi::from_api(frames, table)"),
    },
];

/// Whether `owner` (a kind or a module path) has the extra `name`.
pub fn is_extra(owner: &str, name: &str) -> bool {
    EXTRA_METHODS.iter().any(|extra| extra.owner == owner && extra.name == name)
}

/// A façade method whose dispatcher arm stays hand-written in `dispatch.rs`
/// although the generator could express it, because the generated arm would
/// answer differently; or a free function or a constructor of a module
/// (`kind` the module's path, `method` the function's name or
/// `Type::new`), whose arm is then in the module function of `extra`.
#[derive(Debug, Clone, Copy)]
pub struct HandArm {
    /// The handle kind.
    pub kind: &'static str,
    /// The method.
    pub method: &'static str,
    /// Why the arm is hand-written.
    pub reason: &'static str,
    /// The typed body of the guest client's method or function, as
    /// [`Extra::typed`]; `None` for an item of a type of [`GUEST_OWN`],
    /// which `src/guest/hand/` writes with both its bodies.
    pub typed: Option<&'static str>,
}

/// The hand-written arms of façade methods; the stub and the declarations
/// are generated for them as for any other method.
pub const HAND_ARMS: &[HandArm] = &[HandArm {
    kind: "expander",
    method: "instantiate",
    reason: "an argument in the wrong place is reported at the first wrong position among the context, the use site, the template and \
             the bindings, which the generated arm, copying the use site first, would not do (`tests/boundary.rs`, \"wrong order\")",
    typed: Some("let template = template.lent(table)?;\n\
                 let staged = extra::expander_instantiate(table, [self.handle, cx.handle, site.handle, template, bindings.handle])?;\n\
                 FromApi::from_api(staged, table)"),
},
    HandArm {
        kind: "lowerer",
        method: "get",
        reason: "`get` answers a borrow of the lowerer, which cannot be lent beside it: an extension's own table and a rewrite's trees \
                 are answered as owned copies (`get_owned`), the counters and anchors tables recorded and asked of the lowerer again",
        typed: Some("let got = extra::lowerer_get(table, self.handle, name)?;\n\
                     FromApi::from_api(got, table)"),
    },
    HandArm {
        kind: "counters",
        method: "record",
        reason: "the point is read back from data by the façade, against the trees its reader sees",
        typed: Some("let point = IntoApi::into_api(point, table)?;\n\
                     let record = extra::counters_record(table, self.handle, &point)?;\n\
                     FromApi::from_api(record, table)"),
    },
    HandArm {
        kind: "counters",
        method: "format",
        reason: "the point is read back from data by the façade, against the trees its reader sees",
        typed: Some("let point = IntoApi::into_api(point, table)?;\n\
                     let segments = extra::counters_format(table, self.handle, &point, options)?;\n\
                     FromApi::from_api(segments, table)"),
    },
    HandArm {
        kind: "anchors",
        method: "id",
        reason: "the point is read back from data by the façade, against the trees its reader sees",
        typed: Some("let point = IntoApi::into_api(point, table)?;\n\
                     extra::anchors_id(table, self.handle, &point)"),
    },
    HandArm {
        kind: "lowerer",
        method: "lang",
        reason: "`lang` answers a borrow of the lowerer, which cannot be lent beside it: the language is recorded as the lowerer's \
                 handle and asked of the lowerer again on every use",
        typed: Some("Ok(BuiltLang { handle: extra::lowerer_lang(table, self.handle)? })"),
    },
    HandArm {
        kind: "expander",
        method: "lang",
        reason: "`lang` answers a borrow of the expander, which cannot be lent beside it: the language is recorded as the expander's \
                 handle and asked of the expander again on every use",
        typed: Some("Ok(BuiltLang { handle: extra::expander_lang(table, self.handle)? })"),
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "TemplateLanguage::new",
        reason: "takes a built language, which a recorded entry asks of its lowerer or expander again",
        typed: None,
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "compile_template_with",
        reason: "takes a built language, which a recorded entry asks of its lowerer or expander again",
        typed: Some("let language = IntoApi::into_api(language, table)?;\n\
                     let template = extra::compile_template_with(table, spec, lang.handle, &language)?;\n\
                     FromApi::from_api(template, table)"),
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "instantiate",
        reason: "the bindings are of one of two kinds, `placeholder_bindings` or `value_bindings`",
        typed: Some("let template = template.lent(table)?;\n\
                     let site = IntoApi::into_api(site, table)?;\n\
                     let tree = extra::instantiate(table, &*site, template, bindings.handle)?;\n\
                     FromApi::from_api(tree, table)"),
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "ExtraValues::over",
        reason: "its receiver is data, read with the call's table, and its base is of one of two kinds, `placeholder_bindings` or \
                 `value_bindings`",
        typed: None,
}];

/// Whether the method `method` of the kind `kind` has an arm of
/// [`HAND_ARMS`].
pub fn is_hand_arm(kind: &str, method: &str) -> bool {
    HAND_ARMS.iter().any(|entry| entry.kind == kind && entry.method == method)
}

/// An export or a method of a prelude module whose JavaScript is a fragment,
/// with its TypeScript text.
#[derive(Debug, Clone, Copy)]
pub struct Hand {
    /// The prelude module, without `flm:` (`core/registry`).
    pub module: &'static str,
    /// The export (`ArgDef`), or the method as `Class.method`
    /// (`FeatureBuilder.stage`; a class's constructor is `Class.constructor`,
    /// which stands for the Rust `new`).
    pub item: &'static str,
    /// The TypeScript text.
    pub ts: &'static str,
}

/// The hand list: the four protocol methods, the three methods that take a
/// handler object (`contribute_to`, `contribute_first_to`, `peer_slot`),
/// `FeatureBuilder.feature`, the
/// classes of `flm-core` types (`StyleDef` and `ScriptDef` among them: their row is the class) and the dual `ElementDef`,
/// `DefinitionBindings`, and the two functions that answer an `ArgDef`
/// (`arg_def_from_code`, `label_fields_arg`), which read their answer back
/// as one, and `host::path`, which takes a module's `import.meta` where Rust
/// takes the module's path.
pub const HAND_WRITTEN: &[Hand] = &[
    Hand { module: "core/registry", item: "FeatureBuilder.feature", ts: "feature(): string;" },
    Hand { module: "core/registry", item: "FeatureBuilder.stage", ts: "stage(stage: Stage): void;" },
    Hand { module: "core/registry", item: "FeatureBuilder.rewrite_stage", ts: "rewrite_stage(stage: RewriteStage): void;" },
    Hand { module: "core/registry", item: "FeatureBuilder.backend", ts: "backend(def: BackendDef): void;" },
    Hand {
        module: "core/registry",
        item: "BackendDef",
        ts: "export class BackendDef {
  /** Mirrors `BackendDef::new`. */
  constructor(name: string, native: string[], lowering: BackendStage, emit: BackendStage, final_stage: BackendStage);
  /** Mirrors the field `BackendDef::name`. */
  name: string;
  /** Mirrors the field `BackendDef::native`. */
  native: string[];
  /** Mirrors the field `BackendDef::lowering`. */
  lowering: BackendStage;
  /** Mirrors the field `BackendDef::emit`. */
  emit: BackendStage;
  /** Mirrors the field `BackendDef::final_stage`. */
  final_stage: BackendStage;
  /** Mirrors `BackendDef::docs`. */
  docs(docs: string): this;
  /** No flm-core counterpart: the data form, each stage as the backend holds it; `FeatureBuilder.backend` sends it with each stage object lent. */
  toJSON(): { name: string; native: string[]; lowering: BackendStage; emit: BackendStage; final_stage: BackendStage; docs: string };
}",
    },
    Hand {
        module: "core/registry",
        item: "OutputDef",
        ts: "export class OutputDef {
  /** Mirrors `OutputDef::new`. */
  constructor(name: string, capability: string, table: string, media_type: string, extension: string);
  /** Mirrors the field `OutputDef::name`. */
  name: string;
  /** Mirrors the field `OutputDef::capability`. */
  capability: string;
  /** Mirrors the field `OutputDef::table`. */
  table: string;
  /** Mirrors the field `OutputEntry::media_type` of `OutputDef::entry`. */
  media_type: string;
  /** Mirrors the field `OutputEntry::extension` of `OutputDef::entry`. */
  extension: string;
  /** Mirrors `OutputDef::made_from`: adds an output whose files this output is made from. */
  made_from(output: string): this;
  /** Mirrors `OutputDef::docs`. */
  docs(docs: string): this;
  /** No flm-core counterpart: the data form that crosses to the host. */
  toJSON(): { name: string; capability: string; table: string; media_type: string; extension: string; made_from: string[]; docs: string };
}",
    },
    Hand {
        module: "core/artifact",
        item: "ArtifactId",
        ts: "export class ArtifactId {
  /** Mirrors `ArtifactId::new`. */
  constructor(owner: string, name: string);
  /** Mirrors the field `ArtifactId::owner`. */
  owner: string;
  /** Mirrors the field `ArtifactId::name`. */
  name: string;
  /** No flm-core counterpart: the data form that crosses to the host. */
  toJSON(): { owner: string; name: string };
}",
    },
    Hand {
        module: "core/artifact",
        item: "Artifact",
        ts: "export class Artifact {
  /** Mirrors `Artifact::bytes`; the bytes are a content: a text, `{ text }` or `{ base64 }`. */
  static bytes(id: ArtifactId, media_type: string, content: string | { text: string } | { base64: string }): Artifact;
  /** Mirrors `Artifact::folder`; each file is a path and a content. */
  static folder(id: ArtifactId, files: [string, string | { text: string } | { base64: string }][]): Artifact;
  /** Mirrors `Artifact::with_extension`. */
  with_extension(extension: string): this;
  /** Mirrors `Artifact::as_entry`. */
  as_entry(): this;
  /** No flm-core counterpart: the data form that crosses to the host. */
  toJSON(): { id: ArtifactId; media_type: string; data: ArtifactData; entry: boolean; extension: string | null };
}",
    },
    Hand { module: "host", item: "path", ts: "export function path(meta: ImportMeta, relative: string): string;" },
    Hand { module: "core/registry", item: "FeatureBuilder.contribute_to", ts: "contribute_to(slot: string, value: Contributed): void;" },
    Hand {
        module: "core/registry",
        item: "FeatureBuilder.contribute_first_to",
        ts: "contribute_first_to(slot: string, value: Contributed): void;",
    },
    Hand { module: "core/registry", item: "FeatureBuilder.peer_slot", ts: "peer_slot(slot: string, value: Contributed): void;" },
    Hand {
        module: "core/registry",
        item: "StyleDef",
        ts: "export class StyleDef {
  constructor(id: string, content: DefContent[], docs?: string);
  id: string;
  content: DefContent[];
  docs: string;
  toJSON(): { id: string; content: DefContent[]; docs: string };
}",
    },
    Hand {
        module: "core/registry",
        item: "ScriptDef",
        ts: "export class ScriptDef {
  constructor(id: string, content: DefContent[], docs?: string);
  id: string;
  content: DefContent[];
  docs: string;
  toJSON(): { id: string; content: DefContent[]; docs: string };
}",
    },
    Hand { module: "model/element", item: "ElementSpec.with", ts: "with(aspect: Aspect): this;" },
    Hand {
        module: "lang/rewrites/userdefs",
        item: "UserDefinition.element_def_with",
        ts: "element_def_with(owner: string, element_name: string, hook: Expand): ElementDef;",
    },
    Hand {
        module: "core/lowering",
        item: "LoweringRule.constructor",
        ts: "constructor(name: string, element: string, decl: LoweringDecl, body: Lower, mark: DefaultMark);",
    },
    Hand {
        module: "lang/mint",
        item: "ArgDef",
        ts: "export class ArgDef {
  constructor(syntax: string | ArgStdKind);
  code: string | null;
  name: string | null;
  static named(name: string, syntax: string | ArgStdKind): ArgDef;
  static mandatory(name: string): ArgDef;
  static optional(name: string): ArgDef;
  layout(layout: ArgLayout): this;
  blocks(): this;
  with_opaque_wrapper(): this;
  docs(text: string): this;
  mode(mode: Mode): this;
  scoped(name: string): this;
  role(name: string): this;
  toJSON(): ({ code: string } | ArgStdKind) & { name: string | null; layout: ArgLayout; opaque_wrapper: boolean; docs: string; mode: Mode | null; scoped: string[]; role: string | null };
}",
    },
    Hand { module: "lang/mint", item: "arg_def_from_code", ts: "export function arg_def_from_code(code: string): ArgDef;" },
    // `refs.label_fields_arg()` reads its answer back as an `ArgDef`, as
    // `arg_def_from_code` does.
    Hand { module: "stdlib/features/refs", item: "label_fields_arg", ts: "export function label_fields_arg(): ArgDef;" },
    Hand {
        module: "lang/mint",
        item: "BodyDef",
        ts: "export class BodyDef {
  constructor();
  layout(layout: BodyLayout): this;
  blocks(): this;
  with_opaque_wrapper(): this;
  verbatim(): this;
  mode(mode: Mode): this;
  scoped(name: string): this;
  toJSON(): { layout: BodyLayout; opaque_wrapper: boolean; verbatim: boolean; mode: Mode | null; scoped: string[] };
}",
    },
    Hand {
        module: "lang/mint",
        item: "ElementDef",
        ts: "export class ElementDef {
  constructor(owner: string, names: string[], syntax: SyntaxDef, element: ElementSpec);
  docs(text: string): this;
  modes(list: Mode[]): this;
  owner(): string | null;
  names(): string[] | null;
  syntax(): SyntaxDef | null;
  element(): ElementSpec | null;
  toJSON(): any;
}",
    },
    Hand {
        module: "lang/rewrites/userdefs",
        item: "Definition",
        ts: "export class Definition {
  constructor(kind: DefinitionKind, name: string, args: ArgDef[] | string);
  kind: DefinitionKind;
  name: string;
  args: ArgDef[] | string;
  defaults: { [key: string]: string };
  overrides: boolean;
  static macro_(name: string, args: ArgDef[] | string): Definition;
  static environment(name: string, args: ArgDef[] | string): Definition;
  static specials(trigger: string, args: ArgDef[] | string): Definition;
  content(template: string): this;
  modes(modes: DefinitionModes): this;
  text(template: string): this;
  math(template: string): this;
  default(key: string, template: string): this;
  argument_number_offset(offset: number): this;
  block_role(role: BlockRole): this;
  expand(policy: ExpandPolicy): this;
  overriding(): this;
  docs(text: string): this;
  extra_placeholders: { name: string; substitution: boolean }[];
  extra_placeholder(name: string, substitution: boolean): this;
  toJSON(): any;
}",
    },
    Hand {
        module: "stdlib/backends/lowering",
        item: "LoweringStage",
        // `extends LibraryStage` gives `LibraryStage` a protected constructor
        // in the declarations (`is_extended_by_hand`).
        ts: "export class LoweringStage extends LibraryStage {
  constructor(backend: string);
}",
    },
    Hand {
        module: "lang/rewrites/userdefs",
        item: "DefinitionBindings",
        // `extends PlaceholderBindings` gives `PlaceholderBindings` a
        // protected constructor in the declarations (`is_extended_by_hand`).
        ts: "export class DefinitionBindings extends PlaceholderBindings {
  constructor(site: Node, variant: ModeVariant);
}",
    },
    // The protocol for handler objects (aspects, late-init hooks, finalizers)
    // and the classes of the data their modules take.
    Hand { module: "core/registry", item: "FeatureBuilder.attach", ts: "attach(element: string, aspect: Aspect): void;" },
    Hand { module: "core/registry", item: "FeatureBuilder.late_init", ts: "late_init(hook: LateInit): void;" },
    Hand { module: "core/registry", item: "FeatureBuilder.finalizer", ts: "finalizer(hook: Finalizer): void;" },
    Hand {
        module: "model/aspects",
        item: "RefPoint",
        ts: "export class RefPoint {
  constructor(fields: {
    kind: string;
    labels?: Label[];
    computed?: number;
    malformed?: MalformedLabel[];
    anchor: AnchorPolicy;
    number?: NumberSource | null;
    content?: ContentRef | null;
    accepts?: AcceptedRefTypes;
  });
  kind: string;
  labels: Label[];
  computed: number;
  malformed: MalformedLabel[];
  anchor: AnchorPolicy;
  number: NumberSource | null;
  content: ContentRef | null;
  accepts: AcceptedRefTypes;
  toJSON(): any;
}",
    },
    Hand {
        module: "model/aspects",
        item: "NumberSource",
        ts: "export class NumberSource {
  constructor(fields: { counter: string; style: string; advance: number[]; instances?: Nodes | (Node | Nodes)[]; number_format?: string | null });
  counter: string;
  instances: Nodes | (Node | Nodes)[];
  style: string;
  advance: number[];
  number_format: string | null;
  toJSON(): any;
}",
    },
    Hand {
        module: "model/aspects",
        item: "AnchorPolicy",
        ts: "export const AnchorPolicy: Readonly<{
  Explicit(id: string): { explicit: string };
  FromFirstLabel: \"from_first_label\";
  Slug(fields: { prefix: string; content: ContentRef }): { slug: { prefix: string; content: ContentRef } };
  FromValue(fields: { prefix: string }): { from_value: { prefix: string } };
  Auto(fields: { prefix: string }): { auto: { prefix: string } };
}>;
export type AnchorPolicy = \"from_first_label\" | { explicit: string } | { slug: { prefix: string; content: ContentRef } } | { from_value: { prefix: string } } | { auto: { prefix: string } };",
    },
    Hand {
        module: "model/aspects",
        item: "ContentRef",
        ts: "export const ContentRef: Readonly<{
  Argument(position: number): { argument: number };
  ArgumentNamed(name: string): { argument_named: string };
  Body: \"body\";
  Node(node: Node): { node: Node };
  Synthesized(tree: Tree): { synthesized: Tree };
}>;
export type ContentRef = { argument: number } | { argument_named: string } | \"body\" | { node: Node } | { synthesized: Tree };",
    },
    Hand {
        module: "model/aspects",
        item: "AcceptedRefTypes",
        ts: "export const AcceptedRefTypes: Readonly<{ Any: \"any\"; these(types: string[]): { these: string[] } }>;
export type AcceptedRefTypes = \"any\" | { these: string[] };",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "CounterDecl",
        ts: "export class CounterDecl {
  constructor(name: string, default_style: string);
  name: string;
  default_style: string;
  docs(text: string): this;
  within(within: Within): this;
  nesting(spec: WithinJoinerSpec): this;
  toJSON(): any;
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "CounterStyleDecl",
        ts: "export class CounterStyleDecl {
  constructor(name: string);
  name: string;
  number_format: NumberTemplate | null;
  sub_levels: any[] | null;
  prefix_words: PrefixWords | null;
  delimiters: Delimiters | null;
  join: any;
  name_in_link: boolean | null;
  repeat_parents_in_range: boolean | null;
  ranges: boolean | null;
  number_writer: string | null;
  with_number_writer(name: string): this;
  toJSON(): any;
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "ParentSetDecl",
        ts: "export class ParentSetDecl {
  constructor(name: string, counters: string[]);
  name: string;
  counters: string[];
  toJSON(): { name: string; counters: string[] };
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "Within",
        ts: "export class Within {
  private constructor();
  static parent(name: string): Within;
  static parents(names: string[]): Within;
  static parent_set(name: string): Within;
  parents: Parents;
  joiner: WithinJoinerSpec;
  no_restart(): this;
  joined_by(text: string): this;
  template(text: string): this;
  displayed_bare(): this;
  joiner_named(name: string): this;
  toJSON(): { parents: Parents; joiner: WithinJoinerSpec; no_restart: boolean };
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "PluralWords",
        ts: "export class PluralWords {
  constructor(one: string, other: string);
  one: string;
  other: string;
  toJSON(): { one: string; other: string };
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "PrefixWords",
        ts: "export class PrefixWords {
  constructor(one: string, other: string);
  base: PluralWords;
  variants: { [name: string]: PluralWords };
  with_variant(name: string, words: PluralWords): this;
  toJSON(): any;
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "Delimiters",
        ts: "export class Delimiters {
  constructor(open: string, close: string);
  open: string;
  close: string;
  toJSON(): { open: string; close: string };
}",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "NumberTemplate",
        ts: "export const NumberTemplate: Readonly<{ formats(text: string): { formats: string }; initial(text: string): { initial: string } }>;
export type NumberTemplate = { formats: string } | { initial: string };",
    },
    Hand {
        module: "stdlib/features/numbering",
        item: "FormatOptions",
        ts: "export class FormatOptions {
  constructor();
  with_word: boolean;
  with_delimiters: boolean;
  variant(name: string): this;
  without_word(): this;
  without_delimiters(): this;
  toJSON(): { variant: string | null; with_word: boolean; with_delimiters: boolean };
}",
    },
    Hand {
        module: "stdlib/backends/latex",
        item: "LatexDefinitionKind",
        ts: "export const LatexDefinitionKind: Readonly<{ Macro: \"macro\"; Environment: \"environment\"; Declaration: \"declaration\" }>;
export type LatexDefinitionKind = \"macro\" | \"environment\" | \"declaration\";",
    },
    Hand {
        module: "stdlib/backends/latex",
        item: "LatexMacroPayload",
        ts: "export class LatexMacroPayload {
  constructor(name: string, args: LatexArgShape[]);
  name: string;
  args: LatexArgShape[];
  packed(): this;
  toJSON(): { name: string; args: LatexArgShape[]; packed: boolean };
}",
    },
    Hand {
        module: "stdlib/backends/latex",
        item: "LatexEnvironmentPayload",
        ts: "export class LatexEnvironmentPayload {
  constructor(name: string, args: LatexArgShape[]);
  name: string;
  args: LatexArgShape[];
  verbatim(): this;
  packed(): this;
  body_argument(): this;
  toJSON(): { name: string; args: LatexArgShape[]; verbatim: boolean; packed: boolean; body: boolean };
}",
    },
    Hand {
        module: "stdlib/backends/latex",
        item: "LatexRawPayload",
        ts: "export class LatexRawPayload {
  constructor(text: string);
  text: string;
  toJSON(): { text: string };
}",
    },
    Hand {
        module: "stdlib/backends/latex",
        item: "LatexDefinitionPayload",
        ts: "export class LatexDefinitionPayload {
  constructor(kind: LatexDefinitionKind, name: string, args: number);
  kind: LatexDefinitionKind;
  name: string;
  source_name: string;
  args: number;
  feature: string | null;
  of_feature(feature: string): this;
  with_source_name(name: string): this;
  packed(): this;
  robust(): this;
  body_argument(): this;
  toJSON(): any;
}",
    },
    Hand {
        module: "stdlib/backends/latex/preamble",
        item: "LatexPreambleDef",
        ts: "export class LatexPreambleDef {
  constructor(name: string, content: string, docs?: string);
  name: string;
  content: string;
  docs: string;
  requires(ids: string[]): this;
  toJSON(): { name: string; content: string; docs: string; requires: string[] };
}",
    },
    Hand {
        module: "stdlib/backends/latex/packages",
        item: "LatexPackageDef",
        ts: "export class LatexPackageDef {
  constructor(name: string, docs?: string);
  package: string;
  docs: string;
  id(id: string): this;
  options(list: string[]): this;
  toJSON(): { id: string; package: string; options: string[]; docs: string };
}",
    },
    // The templates a handler compiles. `TemplateLanguage` is a
    // durable class (it extends the runtime's `Durable`, writing its token
    // as `@durable`); `ValueBindings` extends `PlaceholderBindings`, its
    // `with` answering the same instance; `TemplateSpec` is a data class.
    Hand {
        module: "lang/rewrites/userdefs",
        item: "TemplateSpec",
        ts: "export class TemplateSpec {
  constructor(fields: { text: string; mode: Mode; vocabulary: PlaceholderVocabulary; definition: string; label: string; origin_label: string; user_definition?: boolean; offset?: number });
  text: string;
  mode: Mode;
  vocabulary: PlaceholderVocabulary;
  definition: string;
  label: string;
  origin_label: string;
  user_definition: boolean;
  offset: number;
  toJSON(): { text: string; mode: Mode; vocabulary: PlaceholderVocabulary; definition: string; label: string; origin_label: string; user_definition: boolean; offset: number };
}",
    },
    Hand {
        module: "lang/rewrites/userdefs",
        item: "TemplateLanguage",
        ts: "export class TemplateLanguage {
  /** Mirrors `TemplateLanguage::new`: the template language of `lang`, built once per feature and registry. */
  constructor(lang: BuiltLang);
}",
    },
    Hand {
        module: "lang/rewrites/userdefs",
        item: "ValueBindings",
        ts: "export class ValueBindings extends PlaceholderBindings {
  /** Mirrors `ValueBindings::new`: bindings that hold no value. */
  constructor();
  /** Mirrors `ValueBindings::with`: gives the extra placeholder `name` the tree `tree`; answers the same instance. */
  with(name: string, tree: Tree): this;
}",
    },
    // `ExtraValues` is a data class whose `over` is answered by the host.
    Hand {
        module: "lang/rewrites/userdefs",
        item: "ExtraValues",
        ts: "export class ExtraValues {
  /** Mirrors `ExtraValues::new`: values that hold nothing. */
  constructor();
  /** Mirrors `ExtraValues::template`: gives the extra placeholder `name`, written with the substitution text `substitution` (`null` for none), the compiled template `template`, a variant's template or default value or a template that `compile_template_with` answered; answers the same instance. */
  template(name: string, substitution: string | null, template: CompiledTemplate): this;
  /** Mirrors `ExtraValues::tree`: gives the extra placeholder `name`, written with the substitution text `substitution`, the tree `tree`; answers the same instance. */
  tree(name: string, substitution: string | null, tree: Tree): this;
  /** Mirrors `ExtraValues::no_value`: gives the extra placeholder `name`, written with the substitution text `substitution`, no value; answers the same instance. */
  no_value(name: string, substitution: string | null): this;
  /** Mirrors `ExtraValues::over`: the bindings that answer an extra placeholder from these values and every other placeholder from `base`. */
  over(base: PlaceholderBindings): PlaceholderBindings;
  /** No flm-core counterpart: the data form that crosses to the host. */
  toJSON(): { name: string; substitution: string | null; value: { template: CompiledTemplate } | { tree: Tree } | \"no-value\" }[];
}",
    },
];

/// Whether a hand-written declaration extends the class `class`, which the
/// declarations then give a `protected constructor()` rather than a private
/// one: TypeScript refuses to extend a class whose constructor is private,
/// and a protected one still refuses `new` from a script.
pub fn is_extended_by_hand(class: &str) -> bool {
    let extends = format!(" extends {class} {{");
    HAND_WRITTEN.iter().any(|hand| hand.ts.contains(&extends))
}

/// Whether the method `method` of the type or trait `owner` is written by
/// hand in its stub (`new` stands for `Class.constructor`).
pub fn is_hand_written(owner: &str, method: &str) -> bool {
    let method = if method == "new" { "constructor" } else { method };
    HAND_WRITTEN.iter().any(|hand| hand.item.split_once('.') == Some((owner, method)))
}

// --- the WebAssembly guest client (`flm_ext::guest`) ------------------------------------------

/// The type rows the guest client defines itself (`src/guest/hand/`), each a
/// row of [`TYPES`], by the rule: a type whose crossing form can hold a
/// handle or a handler object is the guest's own, with the api type's name
/// and a lent handle where the api holds an object. Every other type row
/// that is data crosses in the guest as the api's own type, through its
/// [`JsonForm`] (`adapter/values.rs`).
///
/// Beyond that rule, a type the guest cannot hold as the api's type is its
/// own too: one that cannot be made from its crossing form outside
/// `flm-core` (`NodeAddr` and `PointId` hold a node identifier, which only a
/// tree makes, and `CounterRecord` holds a `NodeAddr`; `CitationRequest` has
/// no public constructor), and one whose crossing form leaves out a field
/// the api type holds (`LineBreak`'s span, which the line's span gives).
/// `ArgDef` is the guest's own because the guest declares an argument by its
/// standard kind or reads it from its code, where the api's holds the parser
/// a kind or a code stands for, which `flm-core` makes from it; `ArgStdKind` and `TackOnField` are
/// the guest's own with it, since a tack-on argument's fields hold
/// arguments.
/// `Box<dyn CompiledTemplate>` (a handle) and `DurableTemplateFacade` (a
/// durable token) are both the guest's own `CompiledTemplate`, written back
/// in the form it was read.
pub const GUEST_OWN: &[&str] = &[
    // Values that carry handles.
    "Diagnostic",
    "Expansion",
    "Lowering",
    "Restage",
    "PresTree",
    "WalkEntry",
    "TableValue",
    "EquationLine",
    "Tag",
    "LineShows",
    "LineFrame",
    "FrameReader",
    "WrittenPiece",
    "TexPart",
    "TextHole",
    "LineTex",
    "RefPoint",
    "ContentRef",
    "NumberSource",
    "AnchorPolicy",
    "NumberSegment",
    "CitationResult",
    "CitationAnswer",
    "HostHandle",
    // Several items: a sequence the host answered, or one the extension
    // joined (`LISTS`).
    "Vec<Box<dyn Node>>",
    "[Box<dyn Node>]",
    "Vec<Staged>",
    // The dual types, data or a handle.
    "ElementDef",
    "Box<dyn CompiledTemplate>",
    "DurableTemplateFacade",
    "TemplateLanguage",
    // An argument declared by its kind or read from its code, where the api's
    // holds the parser either stands for, and the kind whose tack-on fields
    // hold arguments.
    "ArgDef",
    "ArgStdKind",
    "TackOnField",
    // Data that holds one of these.
    "ExtraValues",
    "Definition",
    "SyntaxDef",
    "ElementSpec",
    "FallbackDef",
    "ScopedPackageDef",
    "BackendDef",
    // Values that hold handler objects.
    "Aspect",
    "BackendStage",
    "Contributed",
    // Types the guest cannot hold as the api's.
    "NodeAddr",
    "PointId",
    "CounterRecord",
    "CitationRequest",
    "LineBreak",
];

/// The rows of [`HAND_WRITTEN`] (by their item) the guest client writes by
/// hand: the record-and-lend items, each recording the handler object it
/// takes in the running registration's record, then lending it through its
/// native. Every other row of [`HAND_WRITTEN`] is generated for the guest (a
/// re-export, a type of [`GUEST_OWN`], or a constructor).
pub const GUEST_HAND: &[&str] = &[
    "FeatureBuilder.feature",
    "FeatureBuilder.stage",
    "FeatureBuilder.rewrite_stage",
    "FeatureBuilder.backend",
    "FeatureBuilder.contribute_to",
    "FeatureBuilder.contribute_first_to",
    "FeatureBuilder.peer_slot",
    "FeatureBuilder.attach",
    "FeatureBuilder.late_init",
    "FeatureBuilder.finalizer",
    "ElementSpec.with",
    "UserDefinition.element_def_with",
    "LoweringRule.constructor",
];

/// The types the guest client adds to a module (by its path), written by
/// hand in the module's file of `src/guest/hand/` and re-exported from the
/// module with its own types: a type that no item of the api names, which a
/// constructor of a type of [`GUEST_OWN`] takes where the api's constructor
/// takes a type that does not cross, or a data type that an extension's
/// settings hold, which mirrors a `flm-core` type that the api does not name
/// (`ArgListData`, an argument list written as data).
pub const GUEST_ADDED: &[(&str, &str)] = &[("lang/mint", "ArgListData")];

/// The free functions of the api (by their module's path) that the guest
/// client does not offer; the dispatcher answers each for the JavaScript
/// binding. `arg_def_from_code` is the guest's `ArgDef::from_code`, which
/// reads the code with `flm-core`'s reader where the argument is made,
/// `define` included: a constructor of a type of [`GUEST_OWN`] makes the
/// answer where the guest runs, with no request. `host::path` joins a path
/// to the folder of a module of a script, which a compiled extension does
/// not have: it writes its `self:` paths.
pub const GUEST_OMITTED: &[(&str, &str)] = &[("lang/mint", "arg_def_from_code"), ("host", "path")];

/// Whether the free function `function` of the module `path` is in
/// [`GUEST_OMITTED`].
pub fn is_guest_omitted(path: &str, function: &str) -> bool {
    GUEST_OMITTED.iter().any(|(omitted_path, name)| *omitted_path == path && *name == function)
}

// --- the structural rules -------------------------------------------------------------------

/// A normalized type, split at its outermost shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// `&T` or `&mut T`.
    Ref {
        /// Whether the borrow is `&mut`.
        mutable: bool,
        /// `T`.
        inner: String,
    },
    /// A path with generic arguments: `Head<A, B>`.
    Generic {
        /// The path before `<`.
        head: String,
        /// The arguments.
        args: Vec<String>,
    },
    /// A tuple, `()` included.
    Tuple(Vec<String>),
    /// `[T]`.
    Slice(String),
    /// `dyn Bound + …`, its bounds.
    Dyn(Vec<String>),
    /// `impl Bound + …`, its bounds.
    Impl(Vec<String>),
    /// Anything else: a plain path, an array.
    Plain(String),
}

/// Splits `text` at its top-level occurrences of `separator` (outside `<>`,
/// `()` and `[]`).
fn split_top(text: &str, separator: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut position = 0;
    while position < bytes.len() {
        match bytes[position] {
            b'<' | b'(' | b'[' => depth += 1,
            b'>' | b')' | b']' => depth -= 1,
            _ => {}
        }
        if depth == 0 && text[position..].starts_with(separator) {
            parts.push(text[start..position].trim().to_string());
            position += separator.len();
            start = position;
            continue;
        }
        position += 1;
    }
    let last = text[start..].trim();
    if !last.is_empty() {
        parts.push(last.to_string());
    }
    parts
}

/// The outermost shape of the normalized type `text`.
pub fn shape(text: &str) -> Shape {
    if let Some(inner) = text.strip_prefix("&mut ") {
        return Shape::Ref { mutable: true, inner: inner.to_string() };
    }
    if let Some(inner) = text.strip_prefix('&') {
        return Shape::Ref { mutable: false, inner: inner.to_string() };
    }
    if let Some(bounds) = text.strip_prefix("dyn ") {
        return Shape::Dyn(split_top(bounds, " + "));
    }
    if let Some(bounds) = text.strip_prefix("impl ") {
        return Shape::Impl(split_top(bounds, " + "));
    }
    if let Some(inner) = text.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        return Shape::Tuple(split_top(inner, ","));
    }
    if let Some(inner) = text.strip_prefix('[').and_then(|rest| rest.strip_suffix(']'))
        && split_top(inner, ";").len() == 1
    {
        return Shape::Slice(inner.to_string());
    }
    if let Some((head, rest)) = text.split_once('<')
        && let Some(inner) = rest.strip_suffix('>')
        && !head.contains(' ')
    {
        return Shape::Generic { head: head.to_string(), args: split_top(inner, ",") };
    }
    Shape::Plain(text.to_string())
}

/// What a type is made of, once the rows and the structural rules are
/// applied: its leaf types (each must have a row) and the façade or
/// extension-side traits it names (each a kind row's).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Leaves {
    /// The leaf types, as the rows name them.
    pub types: Vec<String>,
    /// The traits named as `&dyn F`, `Box<dyn F>` or `Arc<dyn H>` with no
    /// row of their own.
    pub kinds: Vec<String>,
}

/// The leaves of the normalized type `text`: a row
/// is matched first; then `Result<T, ExtError>` and `Result<T,
/// NotCharacters>` are `T`; `Option<T>`, `Vec<T>`, `[T]` and `&T` are `T`;
/// a tuple is its elements, `()` nothing; `impl Into<T>` is `T` and `impl
/// IntoIterator<Item = T>` is `Vec<T>` (the two forms a constructor's
/// parameter takes); `dyn F`, `Box<dyn F>` and `Arc<dyn F>`, `F` a trait of
/// a kind row, name the kind; anything else is a leaf.
pub fn leaves(text: &str) -> Leaves {
    let mut out = Leaves::default();
    collect_leaves(text, &mut out);
    out
}

/// [`leaves`], into `out`.
fn collect_leaves(text: &str, out: &mut Leaves) {
    if row(text).is_some() {
        out.types.push(text.to_string());
        return;
    }
    match shape(text) {
        Shape::Ref { inner, .. } | Shape::Slice(inner) => collect_leaves(&inner, out),
        Shape::Tuple(elems) => elems.iter().for_each(|elem| collect_leaves(elem, out)),
        Shape::Generic { head, args } => match (head.as_str(), args.as_slice()) {
            ("Result", [ok, error]) if error == "ExtError" || error == "NotCharacters" => collect_leaves(ok, out),
            ("Option" | "Vec", [inner]) => collect_leaves(inner, out),
            ("Box" | "Arc", [inner]) if facade(inner).is_some() => out.kinds.extend(facade(inner)),
            _ => out.types.push(text.to_string()),
        },
        Shape::Dyn(_) => match facade(text) {
            Some(name) => out.kinds.push(name),
            None => out.types.push(text.to_string()),
        },
        Shape::Impl(bounds) => match bounds.as_slice() {
            [bound] => match shape(bound) {
                Shape::Generic { head, args } if head == "Into" && args.len() == 1 => collect_leaves(&args[0], out),
                Shape::Generic { head, args } if head == "IntoIterator" && args.len() == 1 => match args[0].strip_prefix("Item = ") {
                    Some(item) => collect_leaves(&format!("Vec<{item}>"), out),
                    None => out.types.push(text.to_string()),
                },
                _ => out.types.push(text.to_string()),
            },
            _ => out.types.push(text.to_string()),
        },
        Shape::Plain(_) => out.types.push(text.to_string()),
    }
}

/// The trait of a kind row that `dyn F` names, if any.
fn facade(text: &str) -> Option<String> {
    let Shape::Dyn(bounds) = shape(text) else { return None };
    let [bound] = bounds.as_slice() else { return None };
    kind_of(bound).map(|_| bound.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_structural_rules_find_the_leaves() {
        let node = leaves("Result<Option<Box<dyn Node>>, ExtError>");
        assert_eq!((node.types.as_slice(), node.kinds.as_slice()), (&[][..], &["Node".to_string()][..]));
        assert_eq!(leaves("Result<Option<Vec<Box<dyn Node>>>, ExtError>").types, ["Vec<Box<dyn Node>>"]);
        assert_eq!(leaves("&[Box<dyn Node>]").types, ["[Box<dyn Node>]"]);
        assert_eq!(leaves("&mut dyn LowerCx").kinds, ["LowerCx"]);
        let pairs = leaves("Vec<(String, Box<dyn Tree>)>");
        assert_eq!((pairs.types.as_slice(), pairs.kinds.as_slice()), (&["String".to_string()][..], &["Tree".to_string()][..]));
        assert_eq!(leaves("Result<Result<String, NotCharacters>, ExtError>").types, ["String"]);
        assert_eq!(leaves("impl IntoIterator<Item = Mode>").types, ["Vec<Mode>"]);
        assert_eq!(leaves("impl Into<String>").types, ["String"]);
        assert_eq!(leaves("&str").types, ["str"]);
        assert_eq!(leaves("()"), Leaves::default());
        assert_eq!(leaves("NodeRegion").types, ["NodeRegion"]);
        assert_eq!(leaves("HashMap<String, usize>").types, ["HashMap<String, usize>"]);
    }
}
