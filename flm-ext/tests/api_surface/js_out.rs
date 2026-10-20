//! The emitter of the generated prelude (the plan's section 5.2,
//! `dev-docs/ext/e2b`): one stub per module of the extension API under
//! `src/quickjs/js/api/`, the module `flm:_api` that imports them all
//! (`api/_api.js`), and the module list `src/quickjs/generated_modules.rs`.
//!
//! # The shape of a stub
//!
//! The header names the generator and the source file; the imports of every
//! name of `flm:_runtime`, then the import lines of the module's fragment
//! file; the re-exports (each re-exported item from its home module, then
//! each submodule of the surface as a namespace, `export * as preamble from
//! "flm:stdcore/backends/latex/preamble"`); the module's token
//! (`const MODULE = module(…)`) when a function or a constructor forwards to
//! it; then the items in source
//! order: a handle class per façade trait and opaque type, registered under
//! its kind; a class per data struct (its fields held under a trailing
//! underscore exactly when a method bears the field's name, `toJSON()`
//! answering the fields in declaration order); a frozen object of tokens per
//! unit enum, of static constructors per data enum; a constant per `const`,
//! its value read from the compiled table; a forwarding function per free
//! function. Each item carries the first sentence of its rustdoc as a
//! comment.
//!
//! # The fragments
//!
//! What does not forward (the plan's section 3.5) is written by hand under
//! `src/quickjs/js/hand/`, one file per module that needs any
//! (`core/registry` is `core-registry.js`), each part introduced by a marker
//! line, `// @hand export <Name>` or `// @hand method <Class>.<method>`
//! (`<Class>.constructor` for a constructor). A marker's text, up to the next
//! marker, is emitted in place of the generated export or method; the import
//! lines before the first marker are copied into the stub.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;

use crate::inventory::{FirstStatement, Function, Inventory, Item, ItemClass, ItemKind, Module};
use crate::mapping::{self, Ts};

/// The folder of the generated stubs, from the crate's root.
pub const API_DIR: &str = "src/quickjs/js/api";

/// The folder of the fragments, from the crate's root.
pub const HAND_DIR: &str = "src/quickjs/js/hand";

/// The generated module list, from the crate's root.
pub const MODULES_RS: &str = "src/quickjs/generated_modules.rs";

/// The names of `flm:_runtime` every stub imports (the plan's section 3.5).
const RUNTIME_NAMES: &str =
    "Handle, call, module, register_class, record_stage, registering_feature, lend_stage, lend_expand, record_lower, lend_lower";

/// The width the comments are wrapped at.
const WIDTH: usize = 100;

/// The generated prelude: its files, and the exports of each module.
pub struct Prelude {
    /// Every file, as `(path from the crate's root, text)`: the stubs in the
    /// order of their module paths, then `api/_api.js`, then the module list.
    pub files: Vec<(String, String)>,
    /// The exports of each module, in the stub's order, by module path.
    pub exports: BTreeMap<String, Vec<String>>,
}

/// The hand-written fragments of one module.
#[derive(Debug, Default)]
pub struct Fragments {
    /// The fragment file, from the crate's root.
    pub file: String,
    /// The import lines before the first marker.
    pub imports: Vec<String>,
    /// The text of each marker, by the marker's item (`ArgDef`,
    /// `FeatureBuilder.stage`), in the file's order.
    pub parts: Vec<(String, String)>,
}

impl Fragments {
    /// The text of the marker for `item`.
    pub fn part(&self, item: &str) -> Option<&str> {
        self.parts.iter().find(|(name, _)| name == item).map(|(_, text)| text.as_str())
    }
}

/// The fragment file of the module `path` (`core/registry` is
/// `src/quickjs/js/hand/core-registry.js`).
pub fn fragment_file(path: &str) -> String {
    format!("{HAND_DIR}/{}.js", path.replace('/', "-"))
}

/// Reads the fragment files of every module the hand list names, and checks
/// the markers against the hand list and the inventory.
///
/// # Panics
///
/// With the texts of the plan's section 8: a marker naming no item of the
/// surface; an entry of the hand list without its marker.
pub fn read_fragments(crate_root: &Path, inventory: &Inventory) -> BTreeMap<String, Fragments> {
    let mut out: BTreeMap<String, Fragments> = BTreeMap::new();
    let mut paths: Vec<&str> = mapping::HAND_WRITTEN.iter().map(|hand| hand.module).collect();
    paths.sort_unstable();
    paths.dedup();
    for path in paths {
        let file = fragment_file(path);
        let text = std::fs::read_to_string(crate_root.join(&file)).unwrap_or_default();
        let fragments = parse_fragments(&file, &text);
        for (item, _) in &fragments.parts {
            let (kind, shown) = if item.contains('.') { ("method", item.clone()) } else { ("export", item.clone()) };
            if !names_item(inventory, path, item) {
                let article = if kind == "export" { "an" } else { "a" };
                panic!("the marker `@hand {kind} {shown}` of `{file}` must name {article} {kind} of the surface");
            }
            if !mapping::HAND_WRITTEN.iter().any(|hand| hand.module == path && hand.item == item) {
                panic!("the marker `@hand {kind} {shown}` of `{file}` must have an entry in `HAND_WRITTEN` of `tests/api_surface/mapping.rs`");
            }
        }
        out.insert(path.to_string(), fragments);
    }
    for hand in mapping::HAND_WRITTEN {
        let fragments = &out[hand.module];
        if fragments.part(hand.item).is_none() {
            let (kind, what) = if hand.item.contains('.') { ("method", "the method") } else { ("export", "the export") };
            panic!(
                "{what} `{}` of `flm:{}` is listed in `HAND_WRITTEN` and must have a marker `@hand {kind} {}` in `{}`",
                hand.item, hand.module, hand.item, fragments.file
            );
        }
    }
    out
}

/// Whether `item` (an export, or `Class.method`) names an item of the module
/// `path` of the inventory.
fn names_item(inventory: &Inventory, path: &str, item: &str) -> bool {
    let Some(module) = inventory.module(path) else { return false };
    match item.split_once('.') {
        Some((class, method)) => {
            let method = if method == "constructor" { "new" } else { method };
            module.items.iter().any(|candidate| candidate.name == class && methods(candidate).iter().any(|function| function.name == method))
        }
        None => module.items.iter().any(|candidate| candidate.name == item),
    }
}

/// The fragments of the file `file`, its text `text`.
pub fn parse_fragments(file: &str, text: &str) -> Fragments {
    let mut fragments = Fragments { file: file.to_string(), ..Fragments::default() };
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in text.lines() {
        let marker = line.strip_prefix("// @hand export ").or_else(|| line.strip_prefix("// @hand method "));
        if let Some(name) = marker {
            if let Some((name, lines)) = current.take() {
                fragments.parts.push((name, join_part(&lines)));
            }
            current = Some((name.trim().to_string(), Vec::new()));
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        } else if line.starts_with("import ") {
            fragments.imports.push(line.to_string());
        }
    }
    if let Some((name, lines)) = current {
        fragments.parts.push((name, join_part(&lines)));
    }
    fragments
}

/// The lines of one part, the trailing blank lines dropped.
fn join_part(lines: &[&str]) -> String {
    let end = lines.iter().rposition(|line| !line.trim().is_empty()).map_or(0, |last| last + 1);
    lines[..end].join("\n")
}

/// The methods of a trait or a type; none for any other item.
fn methods(item: &Item) -> &[Function] {
    match &item.kind {
        ItemKind::Trait { methods, .. } | ItemKind::Type { methods, .. } => methods,
        _ => &[],
    }
}

// --- the text helpers, shared with the declarations' emitter --------------------------------

/// `text` with its intra-doc links reduced to their text: `` [`X`](path) ``
/// and `` [`X`] `` are `` `X` ``.
pub fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = &after[..close];
        let tail = &after[close + 1..];
        if let Some(target) = tail.strip_prefix('(')
            && let Some(end) = target.find(')')
        {
            out.push_str(inner);
            rest = &target[end + 1..];
        } else if inner.starts_with('`') && inner.ends_with('`') && inner.len() > 1 {
            out.push_str(inner);
            rest = tail;
        } else {
            out.push('[');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// `text` as `//` comment lines at `indent`, wrapped at [`WIDTH`]; nothing
/// for an empty text.
pub fn comment(text: &str, indent: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    for word in plain(text).split_whitespace() {
        if !line.is_empty() && indent.len() + 3 + line.len() + 1 + word.len() > WIDTH {
            let _ = writeln!(out, "{indent}// {line}");
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        let _ = writeln!(out, "{indent}// {line}");
    }
    out
}

/// The JavaScript literal of the JSON value `value`, on one line.
pub fn literal(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("[{}]", items.iter().map(literal).collect::<Vec<_>>().join(", ")),
        Value::Object(fields) => {
            let fields: Vec<String> = fields.iter().map(|(key, value)| format!("{}: {}", Value::String(key.clone()), literal(value))).collect();
            format!("{{ {} }}", fields.join(", "))
        }
        other => other.to_string(),
    }
}

/// The first paragraph of the documentation of the module in the file
/// `file` (its `//!` lines up to the first empty one).
pub fn module_paragraph(crate_root: &Path, file: &str) -> String {
    let source = std::fs::read_to_string(crate_root.join(file)).unwrap_or_else(|error| panic!("the file `{file}` must be readable: {error}"));
    let mut lines = Vec::new();
    for line in source.lines() {
        let Some(text) = line.strip_prefix("//!") else { break };
        let text = text.trim();
        if text.is_empty() {
            if lines.is_empty() {
                continue;
            }
            break;
        }
        lines.push(text);
    }
    lines.join(" ")
}

/// The module that defines the re-exported item `path`
/// (`crate::api::lang::rewrites::userdefs::Staged` is `lang/rewrites/userdefs`).
pub fn api_module_of(path: &str) -> String {
    let inner = path.strip_prefix("crate::api::").unwrap_or(path);
    let (module, _) = inner.rsplit_once("::").unwrap_or(("", inner));
    module.replace("::", "/")
}

/// The module that defines `name` when the module `path` re-exports it: an
/// `api` item's module, or the first module in sorted order re-exporting
/// the same `flm-core` type (the plan's section 3.2, rule 8); `None` when
/// `path` defines it.
pub fn defined_elsewhere(inventory: &Inventory, path: &str, item: &Item) -> Option<String> {
    match (&item.class, &item.kind) {
        (ItemClass::ApiReexport, ItemKind::Reexport { path: target }) => Some(api_module_of(target)),
        (ItemClass::CoreReexport, ItemKind::Reexport { path: target }) => {
            let first = inventory.items().find(|(_, other)| matches!(&other.kind, ItemKind::Reexport { path } if path == target))?.0;
            (first.path != path).then(|| first.path.clone())
        }
        _ => None,
    }
}

/// Whether `module` forwards to its own token: it has a free function or an
/// opaque type's constructor that is dispatched.
fn uses_token(module: &Module) -> bool {
    module.items.iter().any(|item| match (&item.class, &item.kind) {
        (ItemClass::Function, _) => true,
        (ItemClass::OpaqueType, ItemKind::Type { methods, .. }) => {
            methods.iter().any(|method| method.is_constructor() && !mapping::is_excluded(&item.name, &method.name))
        }
        _ => false,
    })
}

/// The hand-written export `name` of the module `path`, if the hand list
/// has it.
fn hand_export(path: &str, name: &str) -> bool {
    mapping::HAND_WRITTEN.iter().any(|hand| hand.module == path && hand.item == name)
}

/// The parameter names of the TypeScript signature `ts` (`get_at(name:
/// string, node: Node): …` has `name`, `node`).
pub fn signature_params(ts: &str) -> Vec<String> {
    let inner = ts.split_once('(').and_then(|(_, rest)| rest.split_once(')')).map_or("", |(inner, _)| inner);
    inner.split(',').filter_map(|param| param.split(':').next()).map(|name| name.trim().trim_end_matches('?').to_string()).filter(|name| !name.is_empty()).collect()
}

// --- the emitter ----------------------------------------------------------------------------

/// The generated prelude, the values of the constants read from the
/// compiled table (`(module, name, value)`).
pub fn emit(crate_root: &Path, inventory: &Inventory, constants: &[(String, String, Value)]) -> Prelude {
    let fragments = read_fragments(crate_root, inventory);
    let mut files = Vec::new();
    let mut exports = BTreeMap::new();
    for module in &inventory.modules {
        let empty = Fragments::default();
        let fragments = fragments.get(&module.path).unwrap_or(&empty);
        let (text, names) = emit_module(crate_root, inventory, module, fragments, constants);
        files.push((format!("{API_DIR}/{}.js", module.path), text));
        exports.insert(module.path.clone(), names);
    }
    files.push((format!("{API_DIR}/_api.js"), emit_api(inventory)));
    files.push((MODULES_RS.to_string(), emit_modules_rs(inventory)));
    Prelude { files, exports }
}

/// The module `flm:_api`.
fn emit_api(inventory: &Inventory) -> String {
    let mut out = String::from(
        "// flm:_api — generated by `tests/api_surface.rs` from the sources under\n\
         // `flm-ext/src/api/`; do not edit.\n\
         //\n\
         // Imports every module of the extension API, in the order of their paths, so\n\
         // that every handle class is registered with the reviver before any handler\n\
         // runs; `flm:_entry` imports this module.\n",
    );
    for module in &inventory.modules {
        let _ = writeln!(out, "import \"flm:{}\";", module.path);
    }
    out
}

/// The module list `src/quickjs/generated_modules.rs`.
fn emit_modules_rs(inventory: &Inventory) -> String {
    let mut out = String::from(
        "//! Generated by `tests/api_surface.rs` from the sources under `src/api/`; do not\n\
         //! edit. `UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface` rewrites it.\n\
         //!\n\
         //! The modules of the extension API in the prelude: the module `flm:a/b` is the\n\
         //! generated stub `js/api/a/b.js`, which mirrors `flm_ext::api::a::b`.\n\
         \n\
         /// The module `flm:_api`, which imports every API module.\n\
         pub(crate) const API_JS: &str = include_str!(\"js/api/_api.js\");\n\
         \n\
         /// The API modules, (module name, source), in the order of their paths.\n\
         pub(crate) const API_MODULES: &[(&str, &str)] = &[\n",
    );
    for module in &inventory.modules {
        let _ = writeln!(out, "    (\"flm:{0}\", include_str!(\"js/api/{0}.js\")),", module.path);
    }
    out.push_str("];\n");
    out
}

/// One stub and its exports.
fn emit_module(
    crate_root: &Path,
    inventory: &Inventory,
    module: &Module,
    fragments: &Fragments,
    constants: &[(String, String, Value)],
) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut exports = Vec::new();
    let _ = writeln!(out, "// flm:{} — generated by `tests/api_surface.rs` from", module.path);
    let _ = writeln!(out, "// `flm-ext/{}`; do not edit.", module.file);
    let paragraph = module_paragraph(crate_root, &module.file);
    if !paragraph.is_empty() {
        out.push_str("//\n");
        out.push_str(&comment(&paragraph, ""));
    }
    let _ = writeln!(out, "import {{ {RUNTIME_NAMES} }} from \"flm:_runtime\";");
    for line in &fragments.imports {
        let _ = writeln!(out, "{line}");
    }
    for item in &module.items {
        if let Some(home) = defined_elsewhere(inventory, &module.path, item)
            && offered(item)
        {
            let _ = writeln!(out, "export {{ {} }} from \"flm:{home}\";", item.name);
            exports.push(item.name.clone());
        }
    }
    for name in inventory.submodules(&module.path) {
        let _ = writeln!(out, "export * as {name} from \"flm:{}/{name}\";", module.path);
        exports.push(name.to_string());
    }
    if uses_token(module) {
        let _ = write!(out, "\nconst MODULE = module(\"{}\");\n", module.path);
    }
    for item in &module.items {
        if defined_elsewhere(inventory, &module.path, item).is_some() {
            continue;
        }
        let text = emit_item(module, item, fragments, constants);
        if !text.is_empty() {
            out.push('\n');
            out.push_str(&text);
            exports.push(item.name.clone());
        }
    }
    (out, exports)
}

/// Whether a re-exported item is offered to a script (its row is not
/// [`Ts::NotOffered`]).
fn offered(item: &Item) -> bool {
    mapping::row(&item.name).is_none_or(|row| row.ts != Ts::NotOffered)
}

/// The text of one item of the module (empty when the item has no
/// JavaScript form).
fn emit_item(module: &Module, item: &Item, fragments: &Fragments, constants: &[(String, String, Value)]) -> String {
    if hand_export(&module.path, &item.name) {
        let text = fragments.part(&item.name).expect("the fragments were checked against the hand list");
        return format!("{text}\n");
    }
    let doc = comment(&item.doc, "");
    match (&item.class, &item.kind) {
        (ItemClass::SealedTrait | ItemClass::OpaqueType, _) => format!("{doc}{}", handle_class(item, fragments)),
        (ItemClass::DataClass, ItemKind::Type { .. }) => format!("{doc}{}", data_class(item, fragments, &module.path)),
        (ItemClass::DataType | ItemClass::CoreReexport, _) => match enum_object(&item.name) {
            Some(text) => format!("{doc}{text}"),
            None => String::new(),
        },
        (ItemClass::Constant, _) => {
            let value = constants
                .iter()
                .find(|(path, name, _)| *path == module.path && *name == item.name)
                .map(|(_, _, value)| value)
                .expect("the values of the constants were checked against the inventory");
            let value = if value.is_array() { format!("Object.freeze({})", literal(value)) } else { literal(value) };
            format!("{doc}export const {} = {value};\n", item.name)
        }
        (ItemClass::Function, ItemKind::Function(function)) => {
            let params = param_names(function);
            format!("{doc}export function {0}({params}) {{ return call(MODULE, \"{0}\", [{params}]); }}\n", function.name)
        }
        _ => String::new(),
    }
}

/// The parameter names of `function`, joined.
fn param_names(function: &Function) -> String {
    function.params.iter().map(|param| param.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// The frozen object of a unit enum's tokens or of a data enum's static
/// constructors, if the type has one.
fn enum_object(name: &str) -> Option<String> {
    if let Some(row) = mapping::row(name)
        && row.ts == Ts::Union
    {
        let tokens = mapping::unit_enum_tokens(name).expect("the table test checks the tokens of every unit enum");
        let members: Vec<String> = tokens.iter().map(|(variant, token)| format!("{variant}: {token}")).collect();
        return Some(format!("export const {name} = Object.freeze({{ {} }});\n", members.join(", ")));
    }
    let object = mapping::STATIC_OBJECTS.iter().find(|object| object.rust == name)?;
    let mut out = format!("export const {name} = Object.freeze({{\n");
    for member in object.members {
        let _ = writeln!(out, "  {},", member.js);
    }
    out.push_str("});\n");
    Some(out)
}

/// The handle class of a façade trait or an opaque type, registered under
/// its kind.
fn handle_class(item: &Item, fragments: &Fragments) -> String {
    let kind = mapping::kind_of(&item.name).expect("the table test checks the kind of every handle");
    let mut body = String::new();
    for method in methods(item) {
        if mapping::is_excluded(&item.name, &method.name) {
            continue;
        }
        if mapping::is_hand_written(&item.name, &method.name) {
            let key = if method.is_constructor() { format!("{}.constructor", item.name) } else { format!("{}.{}", item.name, method.name) };
            let text = fragments.part(&key).expect("the fragments were checked against the hand list");
            let _ = writeln!(body, "{text}");
            continue;
        }
        body.push_str(&comment(&method.doc, "  "));
        let params = param_names(method);
        if method.is_constructor() {
            let _ = writeln!(body, "  constructor({params}) {{ return call(MODULE, \"{}::{}\", [{params}]); }}", item.name, method.name);
        } else if kind.rehearsable {
            let _ = writeln!(body, "  {0}({params}) {{ if (this._token !== null) return call(this, \"{0}\", [{params}]); }}", method.name);
        } else {
            let _ = writeln!(body, "  {0}({params}) {{ return call(this, \"{0}\", [{params}]); }}", method.name);
        }
    }
    for extra in mapping::EXTRA_METHODS.iter().filter(|extra| extra.owner == kind.kind) {
        if methods(item).iter().any(|method| method.name == extra.name) {
            continue;
        }
        match extra.js {
            Some(js) => {
                let _ = writeln!(body, "  {js}");
            }
            None => {
                let params = signature_params(extra.ts).join(", ");
                let _ = writeln!(body, "  {0}({params}) {{ return call(this, \"{0}\", [{params}]); }}", extra.name);
            }
        }
    }
    let class = if body.is_empty() {
        format!("export class {} extends Handle {{}}\n", item.name)
    } else {
        format!("export class {} extends Handle {{\n{body}}}\n", item.name)
    };
    format!("{class}register_class(\"{}\", {});\n", kind.kind, item.name)
}

/// The name a data class holds the field `field` under: with a trailing
/// underscore exactly when a method of the class bears the field's name.
pub fn holder(item: &Item, field: &str) -> String {
    if methods(item).iter().any(|method| method.name == field) { format!("{field}_") } else { field.to_string() }
}

/// The default of a field of a data class that the constructor does not
/// take, by its type.
fn default_of(item: &Item, field: &str, ty: &str, module: &str) -> &'static str {
    if ty.starts_with("Vec<") {
        "[]"
    } else if ty == "String" {
        "\"\""
    } else if ty.starts_with("Option<") {
        "null"
    } else if ty == "bool" {
        "false"
    } else {
        panic!(
            "the field `{field}` of `{}` (`src/api/{module}.rs`) must be a parameter of `new` or a list, a string, an option or a \
             boolean, so that the generated class can give it a default",
            item.name
        )
    }
}

/// The class of a data struct (rule 4 of the plan's section 3.2).
fn data_class(item: &Item, fragments: &Fragments, module: &str) -> String {
    let ItemKind::Type { fields, methods, .. } = &item.kind else { unreachable!("a data class is a struct") };
    let new = methods.iter().find(|method| method.name == "new");
    let mut out = format!("export class {} {{\n", item.name);
    let params = new.map(param_names).unwrap_or_default();
    let _ = writeln!(out, "  constructor({params}) {{");
    if let Some(new) = new {
        for param in &new.params {
            if !fields.iter().any(|field| field.name == param.name) {
                panic!(
                    "the parameter `{}` of `{}::new` (`src/api/{module}.rs`) must name a field of `{}`, so that the generated class can hold it",
                    param.name, item.name, item.name
                );
            }
        }
    }
    for field in fields {
        let taken = new.is_some_and(|new| new.params.iter().any(|param| param.name == field.name));
        let value = if taken { field.name.clone() } else { default_of(item, &field.name, &field.ty, module).to_string() };
        let _ = writeln!(out, "    this.{} = {value};", holder(item, &field.name));
    }
    out.push_str("  }\n");
    for method in methods.iter().filter(|method| !method.is_constructor()) {
        if mapping::is_hand_written(&item.name, &method.name) {
            let text = fragments.part(&format!("{}.{}", item.name, method.name)).expect("the fragments were checked against the hand list");
            let _ = writeln!(out, "{text}");
            continue;
        }
        out.push_str(&comment(&method.doc, "  "));
        let param = method.params.first().map_or("", |param| param.name.as_str());
        let line = match &method.first {
            Some(FirstStatement::Push(field)) => format!("{}({param}) {{ this.{}.push({param}); return this; }}", method.name, holder(item, field)),
            Some(FirstStatement::Set(field)) if !param.is_empty() => {
                format!("{}({param}) {{ this.{} = {param}; return this; }}", method.name, holder(item, field))
            }
            Some(FirstStatement::SetTo(field, value)) => format!("{}() {{ this.{} = {value}; return this; }}", method.name, holder(item, field)),
            Some(FirstStatement::Get(field)) => format!("{}() {{ return this.{}; }}", method.name, holder(item, field)),
            _ => panic!(
                "the first statement of `{}::{}` (`src/api/{module}.rs`) must push onto a field, assign a field (the method's parameter, or \
                 a boolean literal when it has none) or answer a field, so that the generated class can hold it",
                item.name, method.name
            ),
        };
        let _ = writeln!(out, "  {line}");
    }
    let fields: Vec<String> = fields.iter().map(|field| format!("{}: this.{}", field.name, holder(item, &field.name))).collect();
    let _ = writeln!(out, "  toJSON() {{ return {{ {} }}; }}", fields.join(", "));
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_reduced_to_their_text() {
        assert_eq!(plain("Mirrors [`StageOut::table`](flm_core::x::StageOut::table): the [`Node`]."), "Mirrors `StageOut::table`: the `Node`.");
        assert_eq!(plain("a [b] c"), "a [b] c");
    }

    #[test]
    fn fragments_are_read_by_marker() {
        let fragments = parse_fragments(
            "f.js",
            "// header\nimport { X } from \"flm:a\";\n\n// @hand export A\nexport class A {}\n\n// @hand method B.m\n  m() {}\n",
        );
        assert_eq!(fragments.imports, ["import { X } from \"flm:a\";"]);
        assert_eq!(fragments.parts, [("A".to_string(), "export class A {}".to_string()), ("B.m".to_string(), "  m() {}".to_string())]);
    }
}
