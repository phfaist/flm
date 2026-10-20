//! The mapping table: the generator's one table from the Rust types of the
//! inventory to their conversions and their TypeScript shapes, kept by hand
//! (the plan's sections 3.3 and 4, `dev-docs/ext/e2b`).
//!
//! It holds the kind rows ([`KINDS`], one per handle kind), the opaque value
//! types ([`OPAQUE_TYPES`]), the type rows ([`TYPES`], one per leaf type that
//! crosses or that a re-export or a data type names), the tokens of the unit
//! enums ([`unit_enum_tokens`], evaluated through the crate), the frozen
//! objects of static constructors ([`STATIC_OBJECTS`]), the six lists
//! [`NOT_DISPATCHED`], [`OPTIONAL_IN_SCRIPTS`], [`CALL_OVERRIDES`],
//! [`EXTRA_METHODS`], [`HAND_ARMS`] and [`HAND_WRITTEN`], and the two lists
//! of the WebAssembly guest client ([`GUEST_OWN`], [`GUEST_HAND`]). The test
//! `the_mapping_table_matches_the_inventory` checks it against the inventory
//! in both directions.
//!
//! A type is matched against the rows first; when no row names it, the
//! structural rules of [`leaves`] apply (the plan's section 4.2), outermost
//! first, and what is left must have a row.

#![allow(dead_code, reason = "the emitters of the generator's outputs read every field of the table; this part of the generator reads some")]

use serde_json::Value;

use flm_ext::adapter::values::JsonForm;

/// How the dispatcher takes the receiver of a method of a handle kind (the
/// plan's section 5.1).
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

/// A kind row.
const fn kind(kind: &'static str, rust: &'static str, entry: &'static str, receiver: Receiver) -> KindRow {
    KindRow { kind, rust: Some(rust), entry: Some(entry), receiver, rehearsable: false }
}

/// The 31 handle kinds, in the order of `HandleKind` (`src/adapter/handles.rs`).
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
    // An argument a helper made (`refs.label_fields_arg()`): `ArgDef` is a
    // re-export of `flm-core`'s, so the row names no item under `src/api/`;
    // it has no method, an argument list takes it in place of a code.
    KindRow { kind: "arg_def", rust: None, entry: Some("ArgDef"), receiver: Receiver::Cloned, rehearsable: false },
    // The built language: lent to a late-init hook, or recorded as the
    // lowerer or the expander whose `lang()` answered it (WP-3): every
    // function that takes it is a hand arm.
    kind("built_lang", "BuiltLang", "BuiltLang", Receiver::Recorded),
    kind("flm_diagnostics", "FlmDiagnostics", "FlmDiagnostics", Receiver::LentMut),
    kind("late_init", "LateInit", "LateInit", Receiver::Arc),
    kind("finalizer", "Finalizer", "Finalizer", Receiver::Arc),
    // The tables a reader answers typed (WP-2b): every method is a hand
    // arm, which reads the point back through the façade and asks a
    // recorded table of its lowerer again.
    kind("counters", "Counters", "Counters", Receiver::Recorded),
    kind("anchors", "Anchors", "Anchors", Receiver::Recorded),
    // A template language, resolved for the call from its durable token
    // (WP-3); no method of its own.
    kind("template_language", "TemplateLanguage", "TemplateLanguage", Receiver::Cloned),
    kind("value_bindings", "ValueBindings", "ValueBindings", Receiver::Cloned),
];

/// The kind row of the trait or type `rust`.
pub fn kind_of(rust: &str) -> Option<&'static KindRow> {
    KINDS.iter().find(|row| row.rust == Some(rust))
}

/// The opaque value types (rule 3 of the plan's section 3.2): `api` structs
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

/// The type rows (the plan's section 4.3), by group.
pub const TYPES: &[TypeRow] = &[
    // Scalars and data.
    read("str", Ts::Text("string"), "Args::text, passed by reference"),
    both("String", Ts::Text("string"), "Args::text; a string"),
    read("Option<String>", Ts::Text("string | null"), "the FromArg of Option<String>: null, or a string"),
    both("usize", Ts::Text("number"), "Args::index; a number"),
    answered("u32", Ts::Text("number"), "a number"),
    both("bool", Ts::Text("boolean"), "Args::flag; a boolean"),
    TypeRow { rust: "[u8]", from_arg: Some("Vec<u8>"), into_reply: None, ts: Ts::Text("string"), note: "Args::text, its UTF-8 bytes" },
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
    TypeRow {
        rust: "FilesScope",
        from_arg: Some("FilesScope"),
        into_reply: Some("FilesScope"),
        ts: Ts::Union,
        note: "JsonForm for FilesScope; its FromArg refuses another scope by name",
    },
    read(
        "Contributed",
        Ts::Text("any"),
        "data, as it is, or a handle of kind citation_provider or template_module (FromArg for Contributed)",
    ),
    answered("HostHandle", Ts::Text("Files | HostService"), "lent as Entry::Files or Entry::HostService"),
    read("Vec<Mode>", Ts::Text("Mode[]"), "Mode's JsonForm, per item"),
    // The citation providers and the template modules (E6).
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
        Ts::Text("interface TemplateDescription { output: string; accepts: Accepts; requires?: string[]; config?: { [key: string]: any } }"),
        "template_description_from_value",
    ),
    inside(
        "TemplateFile",
        Ts::Text(
            "type TemplateFile = { name: string; extension: string; media_type: string; text: string } | { name: string; file: string; scope: FilesScope; media_type?: string | null }",
        ),
        "template_files_from_value",
    ),
    inside(
        "TemplateContext",
        Ts::Text(
            "interface TemplateContext { html: boolean; regions: [string, string][]; pieces: PieceValue[]; meta: any; config: { [key: string]: any }; artifacts: [string, string][] }",
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
    both("Expansion", Ts::Text("\"default\" | { emit: Staged[] }"), "expansion_from_value; expansion_to_value"),
    both("Lowering", Ts::Text("\"default\" | { emit: Staged[] }"), "lowering_from_value; lowering_to_value"),
    read(
        "PresTree",
        Ts::Text("type PresTree = { element: string; payload?: any; children?: PresTree[] } | { build: Staged } | { text: string }"),
        "pres_tree_from_value",
    ),
    both("Vec<Staged>", Ts::Text("Staged[]"), "staged_list_from_table; each lent as Entry::Staged"),
    both("Staged", Ts::Handle, "staged_from_table; lent as Entry::Staged"),
    read("Vec<(String, Vec<Staged>)>", Ts::Text("[string, Staged[]][]"), "the reader of the pres_slots arm, in values.rs"),
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
    answered("Vec<(String, Box<dyn Node>)>", Ts::Text("[string, Node][]"), "each pair a list, the node lent"),
    answered(
        "EquationLine",
        Ts::Text(
            "interface EquationLine { span: Span | null; content: Node[]; labels: LabelUse[]; tag: Tag | null; nonumber: boolean; linebreak: LineBreak | null }",
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
    inside("Tag", Ts::Text("interface Tag { starred: boolean; content: Node[] }"), "equation_line_to_value, inside EquationLine"),
    inside("LineBreak", Ts::Text("interface LineBreak { spacing: string | null }"), "equation_line_to_value, inside EquationLine"),
    both(
        "LineFrame",
        Ts::Text("interface LineFrame { environment: Node; index: number; line: EquationLine; anchor: string | null; shows: LineShows }"),
        "line_frame_from_value; line_frame_to_value",
    ),
    inside(
        "LineShows",
        Ts::Text(
            "type LineShows = { kind: \"nothing\" } | { kind: \"number\"; segments: Node[]; text: string } | { kind: \"tag\"; starred: boolean; content: Node[] }",
        ),
        "line_frame_to_value, inside LineFrame",
    ),
    inside("FrameReader", Ts::Text("StageCx | Lowerer"), "the hand-written arm line_frames"),
    answered("NormalizedLine", Ts::Text("interface NormalizedLine { text: string; trailing: string }"), "JsonForm for NormalizedLine"),
    TypeRow {
        rust: "ArgDef",
        from_arg: None,
        into_reply: Some("ArgDef"),
        ts: Ts::Hand,
        note: "lent as Entry::ArgDef (label_fields_arg); the hand-written arm arg_def_from_code writes it as data (arg_def_to_value)",
    },
    inside("BodyDef", Ts::Hand, "body_def_from_value, inside SyntaxDef"),
    both("UserDefinition", Ts::Handle, "the entry cloned; lent as Entry::UserDefinition"),
    both("LoweringRule", Ts::Handle, "the entry cloned; lent as Entry::LoweringRule"),
    answered("Box<dyn ModeVariant>", Ts::Handle, "lent as Entry::ModeVariant"),
    answered("Box<dyn CompiledTemplate>", Ts::Handle, "lent as Entry::CompiledTemplate"),
    answered("Box<dyn PlaceholderBindings>", Ts::Handle, "lent as Entry::PlaceholderBindings"),
    // The templates a handler compiles (WP-3).
    both("TemplateLanguage", Ts::Handle, "the entry cloned (a durable token lent by the dispatcher); answered as its durable token"),
    answered("DurableTemplateFacade", Ts::Text("CompiledTemplate"), "answered as its durable token (durable_to_value)"),
    both("ValueBindings", Ts::Handle, "the entry cloned; lent as Entry::ValueBindings"),
    // Its class (`new TemplateSpec({ … })`, plan section 3.6) is
    // hand-written (`HAND_WRITTEN`, WP-4b); the row is its data form.
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
        Ts::Text("\"block\" | { inline: InlineSpacing } | { \"block-decorator\": Placement }"),
        "JsonForm for BlockRole",
    ),
    inside("InlineSpacing", Ts::Text("{ absorb_before: boolean; absorb_after: boolean }"), "JsonForm for InlineSpacing"),
    // Not offered: re-exports under api/ with no JavaScript form.
    inside("FixedBlockRole", Ts::NotOffered, "the aspect { BlockRoleAspect: role } holds the block role itself"),
    inside("NotCharacters", Ts::NotOffered, "an error, rendered as ExtError::Core of kind not-characters"),
    inside("NodeRegion", Ts::NotOffered, "taken by lower_region only, which is not dispatched"),
    inside("ArgStdKind", Ts::NotOffered, "a Rust-side reader of an argument's kind; no stub uses it"),
    // The artifacts of an output (E5's WP-1a): plain data, re-exported; an
    // artifact crosses as the item of `TablePiece::push`, read by
    // artifact_from_value.
    inside("Artifact", Ts::Hand, "artifact_from_value, the item of TablePiece::push; the class is a fragment"),
    inside("ArtifactId", Ts::Hand, "artifact_from_value, inside Artifact; the class is a fragment"),
    inside(
        "ArtifactData",
        Ts::Text("{ bytes: string | { text: string } | { base64: string } } | { files: [string, string | { text: string } | { base64: string }][] }"),
        "artifact_from_value, inside Artifact",
    ),
    // The backends and the outputs of E5 (WP-1b): a backend's stages are
    // objects of the kinds `library_stage` and `stage`, read from the table
    // by backend_def_from_table; an output is plain data.
    read("BackendDef", Ts::Hand, "backend_def_from_table, its stages taken out of the table; the class is a fragment"),
    inside("BackendStage", Ts::Text("LibraryStage | Stage"), "backend_def_from_table, inside BackendDef"),
    answered("LibraryStage", Ts::Handle, "lent as Entry::LibraryStage; read inside BackendDef"),
    read("OutputDef", Ts::Hand, "JsonForm for OutputDef; the class is a fragment"),
    // The rewrite stages of E5 (WP-2): the declaration is data; the answer
    // of a handler is written as a lowering is; the bundles a restager
    // answers are lent as their entries and taken out of the table by
    // `restage_invocation`, each once.
    read("RewriteDecl", Ts::Class, "JsonForm for RewriteDecl"),
    both("Restage", Ts::Text("\"descend\" | { emit: Staged[] }"), "restage_from_value; restage_to_value"),
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
        "the sealed trait of durable_template, a Rust handler's route to a kept token (S26); a script passes the token itself",
    ),
    // The data of the handler aspects (`flm:model/aspects`), answered by a
    // script's `Referenceable` and `Sectioning`; the binding's conversions are
    // WP-2a's and WP-4a's.
    inside("u32", Ts::Text("number"), "a number; read by the binding's aspect objects (WP-2a)"),
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
        Ts::Text("interface NumberSource { counter: string; instances: Node[]; style: string; advance: number[]; number_format: string | null }"),
        "inside RefPoint (WP-2a)",
    ),
    inside(
        "AnchorPolicy",
        Ts::Text(
            "type AnchorPolicy = \"from_first_label\" | { explicit: string } | { slug: { prefix: string; content: ContentRef } } | { from_value: { prefix: string } } | { auto: { prefix: string } }",
        ),
        "inside RefPoint (WP-2a)",
    ),
    inside(
        "ContentRef",
        Ts::Text("type ContentRef = { argument: number } | { argument_named: string } | \"body\" | { node: Node } | { synthesized: Node }"),
        "inside RefPoint; answered by Sectioning.toc_title (WP-2a)",
    ),
    inside("AcceptedRefTypes", Ts::Text("type AcceptedRefTypes = \"any\" | { these: string[] }"), "inside RefPoint (WP-2a)"),
    both(
        "Label",
        Ts::Text("interface Label { ref_type: string; ref_label: string }"),
        "the FromArg (as Option<Label>) and IntoReply of values.rs; inside RefPoint (ref_point_from_value)",
    ),
    inside(
        "PointId",
        Ts::Text("interface PointId { addr: NodeAddr; index: number }"),
        "point_id_to_value writes it; taken by the Counters and Anchors façades (WP-2b reads it back)",
    ),
    // The builder's rows of E4 (WP-2a).
    read("ScopedPackageDef", Ts::Class, "scoped_package_def_from_table"),
    read("Arc<dyn LateInit>", Ts::Text("LateInit"), "the entry's Arc, cloned (Entry::LateInit)"),
    read("Arc<dyn Finalizer>", Ts::Text("Finalizer"), "the entry's Arc, cloned (Entry::Finalizer)"),
    // The numbering library (WP-2b): the declarations are flm-core's own,
    // read by the service slots' readers (adapter/slots.rs) from the data
    // `contribute_to`, `contribute_first_to` and `peer_slot` take
    // (`Contributed::Data`, routed by the slot's name); their classes are the
    // binding's (WP-4a).
    answered(
        "CounterRecord",
        Ts::Text("interface CounterRecord { counter: string; value: number[]; style: string; instance: NodeAddr | null }"),
        "counter_record_to_value",
    ),
    answered("NumberSegment", Ts::Text("interface NumberSegment { content: Node; link: PointId | null }"), "the IntoReply of values.rs"),
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
    // The LaTeX family (WP-2b): the payloads are given as data to `pres` and
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
/// the crate's own conversion ([`JsonForm::to_value`]; the plan's section
/// 3.4): the variants are named here, the tokens come from `values.rs`.
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
        "FilesScope" => api::host::FilesScope: [Extension, Document],
        "LatexArgShape" => api::stdcore::backends::latex::LatexArgShape: [Star, Optional, Mandatory, EscVerb],
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

/// The frozen object of static constructors of a data enum (the plan's
/// section 4.4), and of `InlineSpacing`, whose object holds a constant.
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

/// The frozen objects: `Restage` (E5), the seven data enums of the plan's
/// section 4.4, then
/// `InlineSpacing`.
pub const STATIC_OBJECTS: &[StaticObject] = &[
    StaticObject {
        rust: "Restage",
        reader: "restage_from_value",
        members: &[
            member("Descend", "Descend: \"descend\"", "Descend: \"descend\""),
            member("Emit", "Emit(staged) { return { emit: staged }; }", "Emit(staged: Staged[]): { emit: Staged[] }"),
        ],
    },
    StaticObject {
        rust: "Lowering",
        reader: "lowering_from_value",
        members: &[
            member("Default", "Default: \"default\"", "Default: \"default\""),
            member("Emit", "Emit(staged) { return { emit: staged }; }", "Emit(staged: Staged[]): { emit: Staged[] }"),
        ],
    },
    StaticObject {
        rust: "Expansion",
        reader: "expansion_from_value",
        members: &[
            member("Default", "Default: \"default\"", "Default: \"default\""),
            member("Emit", "Emit(staged) { return { emit: staged }; }", "Emit(staged: Staged[]): { emit: Staged[] }"),
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

/// The methods not dispatched (the plan's section 3.3).
pub const NOT_DISPATCHED: &[Exclusion] = &[
    excluded("Lowerer", "lower_region", "an address crosses as data that nothing takes back"),
    excluded("ExtTableRead", "items_at", "reached through `Lowerer.get_at`"),
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

/// The methods optional on a script's object (the plan's section 3.2, rule 2).
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

/// The overrides (the plan's section 3.3).
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
}

/// The extras (the plan's sections 3.3 and 5.1).
pub const EXTRA_METHODS: &[Extra] = &[
    Extra {
        owner: "stage_out",
        name: "table",
        ts: "table(name: string): TablePiece",
        js: None,
        reason: "answers a borrow of the receiver, which cannot be lent beside it; the entry `TablePiece { out, name }` records how to ask again",
        rust: "fn table(&mut self, name: &str) -> Result<TablePiece, ExtError>",
    },
    Extra {
        owner: "table_piece",
        name: "push",
        ts: "push(item: any): void",
        js: None,
        reason: "resolves the piece anew through the output's entry on every push",
        rust: "fn push(&mut self, item: Value) -> Result<(), ExtError>",
    },
    Extra {
        owner: "lowerer",
        name: "get_at",
        ts: "get_at(name: string, node: Node): any[] | null",
        js: None,
        reason: "no façade method: `addr_of`, then `get`, then `items_at`, composed in the dispatcher",
        rust: "fn get_at(&self, name: &str, node: &Node) -> Result<Option<Vec<Value>>, ExtError>",
    },
    Extra {
        owner: "lowerer",
        name: "pres_tree",
        ts: "pres_tree(cx: LowerCx, tree: PresTree): Staged",
        js: Some("pres_tree(cx, tree) { return pres_tree(this, cx, tree); }"),
        reason: "a stub-only convenience forwarding to the free function `pres_tree`",
        rust: "fn pres_tree(&mut self, cx: &mut LowerCx, tree: &PresTree) -> Result<Staged, ExtError>",
    },
    Extra {
        owner: "lang/mint",
        name: "arg_def_from_code",
        ts: "arg_def_from_code(code: string): ArgDef",
        js: None,
        reason: "its reply, an `ArgDef`, is written with the code it was read from",
        rust: "fn arg_def_from_code(code: &str) -> Result<ArgDef, ExtError>",
    },
    Extra {
        owner: "stdcore/features/math",
        name: "line_frames",
        ts: "line_frames(reader: StageCx | Lowerer, anchors: boolean, counters: boolean, node: Node): LineFrame[]",
        js: None,
        reason: "its first argument is one of two kinds, the reader borrowing the entry across the call",
        rust: "fn line_frames(reader: FrameReader, anchors: bool, counters: bool, node: &Node) -> Result<Vec<LineFrame>, ExtError>",
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
}

/// The hand-written arms of façade methods; the stub and the declarations
/// are generated for them as for any other method.
pub const HAND_ARMS: &[HandArm] = &[HandArm {
    kind: "expander",
    method: "instantiate",
    reason: "an argument in the wrong place is reported at the first wrong position among the context, the use site, the template and \
             the bindings, which the generated arm, copying the use site first, would not do (`tests/boundary.rs`, \"wrong order\")",
},
    HandArm {
        kind: "lowerer",
        method: "get",
        reason: "`get` answers a borrow of the lowerer, which cannot be lent beside it: an extension's own table and a rewrite's trees \
                 are answered as owned copies (`get_owned`), the counters and anchors tables recorded and asked of the lowerer again",
    },
    HandArm {
        kind: "counters",
        method: "record",
        reason: "the point is read back from data by the façade, against the trees its reader sees",
    },
    HandArm {
        kind: "counters",
        method: "format",
        reason: "the point is read back from data by the façade, against the trees its reader sees",
    },
    HandArm {
        kind: "anchors",
        method: "id",
        reason: "the point is read back from data by the façade, against the trees its reader sees",
    },
    HandArm {
        kind: "lowerer",
        method: "lang",
        reason: "`lang` answers a borrow of the lowerer, which cannot be lent beside it: the language is recorded as the lowerer's \
                 handle and asked of the lowerer again on every use (WP-3)",
    },
    HandArm {
        kind: "expander",
        method: "lang",
        reason: "`lang` answers a borrow of the expander, which cannot be lent beside it: the language is recorded as the expander's \
                 handle and asked of the expander again on every use (WP-3)",
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "TemplateLanguage::new",
        reason: "takes a built language, which a recorded entry asks of its lowerer or expander again (WP-3)",
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "compile_template_with",
        reason: "takes a built language, which a recorded entry asks of its lowerer or expander again (WP-3)",
    },
    HandArm {
        kind: "lang/rewrites/userdefs",
        method: "instantiate",
        reason: "the bindings are of one of two kinds, `placeholder_bindings` or `value_bindings` (WP-3)",
}];

/// Whether the method `method` of the kind `kind` has an arm of
/// [`HAND_ARMS`].
pub fn is_hand_arm(kind: &str, method: &str) -> bool {
    HAND_ARMS.iter().any(|entry| entry.kind == kind && entry.method == method)
}

/// An export or a method of a prelude module whose JavaScript is a fragment
/// (the plan's section 3.5), with its TypeScript text.
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

/// The hand list (the plan's section 3.5): the four protocol methods, the
/// three methods that take a handler object (`contribute_to`,
/// `contribute_first_to`, `peer_slot`: E6), `FeatureBuilder.feature`, the
/// classes of `flm-core` types (`StyleDef` and `ScriptDef` among them: their row is the class) and the dual `ElementDef`,
/// `DefinitionBindings`, and `arg_def_from_code`.
pub const HAND_WRITTEN: &[Hand] = &[
    Hand { module: "core/registry", item: "FeatureBuilder.feature", ts: "feature(): string;" },
    Hand { module: "core/registry", item: "FeatureBuilder.stage", ts: "stage(stage: Stage): void;" },
    Hand { module: "core/registry", item: "FeatureBuilder.rewrite_stage", ts: "rewrite_stage(stage: RewriteStage): void;" },
    Hand {
        module: "core/registry",
        item: "BackendDef",
        ts: "export class BackendDef {
  /** Mirrors `BackendDef::new`; a stage object given as a stage is recorded and lent. */
  constructor(name: string, native: string[], lowering: BackendStage, emit: BackendStage, final_stage: BackendStage);
  /** Mirrors the field `BackendDef::name`. */
  name: string;
  /** Mirrors the field `BackendDef::native`. */
  native: string[];
  /** Mirrors `BackendDef::docs`. */
  docs(docs: string): this;
  /** No flm-core counterpart: the data form that crosses to the host. */
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
  constructor(code: string);
  code: string;
  name: string | null;
  static named(name: string, code: string): ArgDef;
  static mandatory(name: string): ArgDef;
  static optional(name: string): ArgDef;
  layout(layout: ArgLayout): this;
  blocks(): this;
  docs(text: string): this;
  mode(mode: Mode): this;
  scoped(name: string): this;
  role(name: string): this;
  toJSON(): { code: string; name: string | null; layout: ArgLayout; docs: string; mode: Mode | null; scoped: string[]; role: string | null };
}",
    },
    Hand { module: "lang/mint", item: "arg_def_from_code", ts: "export function arg_def_from_code(code: string): ArgDef;" },
    Hand {
        module: "lang/mint",
        item: "BodyDef",
        ts: "export class BodyDef {
  constructor();
  layout(layout: BodyLayout): this;
  blocks(): this;
  verbatim(): this;
  mode(mode: Mode): this;
  scoped(name: string): this;
  toJSON(): { layout: BodyLayout; verbatim: boolean; mode: Mode | null; scoped: string[] };
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
        module: "stdcore/backends/lowering",
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
    // WP-4a: the protocol for handler objects (aspects, late-init hooks,
    // finalizers) and the classes of the data the E4 modules take.
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
  constructor(fields: { counter: string; style: string; advance: number[]; instances?: Node[]; number_format?: string | null });
  counter: string;
  instances: Node[];
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
  Synthesized(root: Node): { synthesized: Node };
}>;
export type ContentRef = { argument: number } | { argument_named: string } | \"body\" | { node: Node } | { synthesized: Node };",
    },
    Hand {
        module: "model/aspects",
        item: "AcceptedRefTypes",
        ts: "export const AcceptedRefTypes: Readonly<{ Any: \"any\"; these(types: string[]): { these: string[] } }>;
export type AcceptedRefTypes = \"any\" | { these: string[] };",
    },
    Hand {
        module: "stdcore/features/numbering",
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
        module: "stdcore/features/numbering",
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
        module: "stdcore/features/numbering",
        item: "ParentSetDecl",
        ts: "export class ParentSetDecl {
  constructor(name: string, counters: string[]);
  name: string;
  counters: string[];
  toJSON(): { name: string; counters: string[] };
}",
    },
    Hand {
        module: "stdcore/features/numbering",
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
        module: "stdcore/features/numbering",
        item: "PluralWords",
        ts: "export class PluralWords {
  constructor(one: string, other: string);
  one: string;
  other: string;
  toJSON(): { one: string; other: string };
}",
    },
    Hand {
        module: "stdcore/features/numbering",
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
        module: "stdcore/features/numbering",
        item: "Delimiters",
        ts: "export class Delimiters {
  constructor(open: string, close: string);
  open: string;
  close: string;
  toJSON(): { open: string; close: string };
}",
    },
    Hand {
        module: "stdcore/features/numbering",
        item: "NumberTemplate",
        ts: "export const NumberTemplate: Readonly<{ formats(text: string): { formats: string }; initial(text: string): { initial: string } }>;
export type NumberTemplate = { formats: string } | { initial: string };",
    },
    Hand {
        module: "stdcore/features/numbering",
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
        module: "stdcore/backends/latex",
        item: "LatexDefinitionKind",
        ts: "export const LatexDefinitionKind: Readonly<{ Macro: \"macro\"; Environment: \"environment\"; Declaration: \"declaration\" }>;
export type LatexDefinitionKind = \"macro\" | \"environment\" | \"declaration\";",
    },
    Hand {
        module: "stdcore/backends/latex",
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
        module: "stdcore/backends/latex",
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
        module: "stdcore/backends/latex",
        item: "LatexRawPayload",
        ts: "export class LatexRawPayload {
  constructor(text: string);
  text: string;
  toJSON(): { text: string };
}",
    },
    Hand {
        module: "stdcore/backends/latex",
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
        module: "stdcore/backends/latex/preamble",
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
        module: "stdcore/backends/latex/packages",
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
    // WP-4b: the templates a handler compiles. `TemplateLanguage` is a
    // durable class (it extends the runtime's `Durable`, writing its token
    // as `@durable`); `ValueBindings` extends `PlaceholderBindings` (S31), its
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
  /** Mirrors `ValueBindings::with`: gives the extra placeholder `name` the tree whose root is `node`; answers the same instance. */
  with(name: string, node: Node): this;
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
    "RefPoint",
    "ContentRef",
    "NumberSource",
    "AnchorPolicy",
    "NumberSegment",
    "CitationResult",
    "CitationAnswer",
    "HostHandle",
    // The dual types, data or a handle.
    "ArgDef",
    "ElementDef",
    "Box<dyn CompiledTemplate>",
    "DurableTemplateFacade",
    "TemplateLanguage",
    // Data that holds one of these.
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
    "FeatureBuilder.contribute_to",
    "FeatureBuilder.contribute_first_to",
    "FeatureBuilder.peer_slot",
    "FeatureBuilder.attach",
    "FeatureBuilder.late_init",
    "FeatureBuilder.finalizer",
    "ElementSpec.with",
    "UserDefinition.element_def_with",
    "LoweringRule.constructor",
    "BackendDef",
];

// --- the structural rules (the plan's section 4.2) ------------------------------------------

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

/// The leaves of the normalized type `text` (the plan's section 4.2): a row
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
        let node = leaves("Result<Option<Vec<Box<dyn Node>>>, ExtError>");
        assert_eq!((node.types.as_slice(), node.kinds.as_slice()), (&[][..], &["Node".to_string()][..]));
        assert_eq!(leaves("&[Box<dyn Node>]").kinds, ["Node"]);
        assert_eq!(leaves("&mut dyn LowerCx").kinds, ["LowerCx"]);
        assert_eq!(leaves("Vec<(String, Box<dyn Node>)>").types, ["Vec<(String, Box<dyn Node>)>"]);
        assert_eq!(leaves("Result<Result<String, NotCharacters>, ExtError>").types, ["String"]);
        assert_eq!(leaves("impl IntoIterator<Item = Mode>").types, ["Vec<Mode>"]);
        assert_eq!(leaves("impl Into<String>").types, ["String"]);
        assert_eq!(leaves("&str").types, ["str"]);
        assert_eq!(leaves("()"), Leaves::default());
        assert_eq!(leaves("NodeRegion").types, ["NodeRegion"]);
        assert_eq!(leaves("HashMap<String, usize>").types, ["HashMap<String, usize>"]);
    }
}
