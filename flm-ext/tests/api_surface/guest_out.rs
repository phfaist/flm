//! The emitter of the guest client's generated surface (refresh section 6.2,
//! `dev-docs/ext/e7`): one file, `src/guest/generated.rs`, which
//! `src/guest/mod.rs` includes as `mod generated; pub use generated::*;`.
//!
//! # The shape of the output
//!
//! One inline module per module of the extension API, nested as the paths
//! nest (`flm_ext::guest::core::lowering` mirrors `flm:core/lowering`), in
//! the order of the paths; a folder that is no module of the surface (`core`,
//! `stdcore/features`) is a module holding its submodules. A module holds,
//! in order: the import of the names every body uses (`prelude`), the
//! imports of the types its signatures name from other modules, then its
//! re-exports (`pub use crate::api::a::b::{…}` for every data type, unit enum
//! and constant it defines; `pub use crate::guest::hand::a_b::{…}` for every
//! type of `GUEST_OWN` and every extension trait it defines; `pub use
//! crate::guest::<home>::X;` for an item it re-exports from another module),
//! then its items in source order: one struct per handle kind whose row names
//! a façade trait or an opaque type of the module (an extension trait is the
//! hand-written trait of that name; a type of `GUEST_OWN` the hand-written
//! type), with its `JsonForm` (the `@handle` form, its kind checked) and one
//! method per dispatched method that is not in `GUEST_HAND`, plus the kind's
//! `EXTRA_METHODS` in their Rust signature; an empty enum per opaque type
//! without a kind of its own, holding its constructor; one function per free
//! function. Last, after the modules, the block of compile-time assertions
//! of the hand-written items (`mod assertions`): one per item of
//! `GUEST_HAND` and per method of each extension trait, binding the item
//! written in `src/guest/hand/` to the signature the type mapping derives
//! from the api (`let _: fn(&mut FeatureBuilder, Arc<dyn Stage>) ->
//! Result<(), ExtError> = FeatureBuilder::stage;`), so that a hand-written
//! item that is missing or whose types drifted fails the build.
//!
//! # The type mapping
//!
//! A signature's types, from the inventory's normalized text: `&dyn X`,
//! `&mut dyn X` and `Box<dyn X>` of a kind row are `&X`, `&mut X` and `X`,
//! also inside `Option`, `Vec`, slices and tuples; a type of `GUEST_OWN` is
//! the guest's own type of that name (`Box<dyn CompiledTemplate>` and
//! `DurableTemplateFacade` are both the own `CompiledTemplate`); any other
//! type is the api's, re-exported; a borrowed result is answered owned (`&str`
//! is `String`, `&[T]` is `Vec<T>`, `&T` is `T`); a result is a `Result<_,
//! ExtError>` (`Result<T, NotCharacters>`, which crosses as an error of kind
//! `not-characters`, is `T` inside it; `()` is `Result<(), ExtError>`).
//!
//! # The forwarding bodies
//!
//! A method sends `abi::call(self.handle, "<method>", vec![…])`, a function
//! or a constructor `abi::call_module("<path>", "<function>", vec![…])` (a
//! constructor's function is `Type::new`), each argument converted by
//! `arg(value, "<method>")` (`ToArg`, with the method's name filled in on an
//! `ExtError::Argument`), the reply read by its type's `JsonForm`, a unit
//! reply dropped. On `FeatureBuilder`, whose kind is rehearsable, a method
//! answers `Ok(())` without a call when the builder is a rehearsal's (its
//! handle `None`). An extra with a stub of its own (`Lowerer::pres_tree`)
//! forwards to the module's function of its name, the receiver first.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::inventory::{Function, Inventory, Item, ItemClass, ItemKind, Module};
use crate::js_out::{defined_elsewhere, plain};
use crate::mapping::{self, Shape, Ts};

/// The generated surface, from the crate's root.
pub const GENERATED_RS: &str = "src/guest/generated.rs";

/// The names of the prelude every module with code imports, and the names of
/// the standard library a signature uses; never imported
/// from a module.
const PRELUDE: &[&str] = &["Value", "Handle", "HandleKind", "JsonForm", "ExtError", "Result", "Option", "Vec", "String", "Self", "Into", "IntoIterator", "Item"];

/// The guest's own type a row of `GUEST_OWN` stands for: its own name, but
/// for the two rows of the one own `CompiledTemplate` (ruling S7).
fn own_name(row: &str) -> &str {
    match row {
        "Box<dyn CompiledTemplate>" | "DurableTemplateFacade" => "CompiledTemplate",
        other => other,
    }
}

/// Whether `name` (a type's name, or a row) is a type of `GUEST_OWN`.
fn is_own(name: &str) -> bool {
    mapping::GUEST_OWN.iter().any(|row| *row == name || own_name(row) == name)
}

/// Whether the method `method` of `owner` (`new` for a constructor) is
/// written by hand in the guest (`GUEST_HAND`).
fn is_guest_hand(owner: &str, method: &str) -> bool {
    let method = if method == "new" { "constructor" } else { method };
    mapping::GUEST_HAND.iter().any(|item| item.split_once('.') == Some((owner, method)))
}

/// `HandleKind`'s variant for the kind `kind` (`feature_builder` is
/// `FeatureBuilder`).
fn variant(kind: &str) -> String {
    kind.split('_').map(|part| {
        let mut chars = part.chars();
        chars.next().map(|first| first.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default()
    }).collect()
}

/// The Rust path of the guest module of the API module `path`
/// (`core/lowering` is `crate::guest::core::lowering`).
fn guest_path(path: &str) -> String {
    format!("crate::guest::{}", path.replace('/', "::"))
}

/// The file of `src/guest/hand/` that holds the hand-written items of the
/// module `path` (`core/registry` is `core_registry`).
pub fn hand_file(path: &str) -> String {
    path.replace('/', "_")
}

/// The methods of a trait or a type; none for any other item.
fn methods(item: &Item) -> &[Function] {
    match &item.kind {
        ItemKind::Trait { methods, .. } | ItemKind::Type { methods, .. } => methods,
        _ => &[],
    }
}

/// Whether an item is offered to a script, and so to the guest (its row is
/// not [`Ts::NotOffered`]).
fn offered(item: &Item) -> bool {
    mapping::row(&item.name).is_none_or(|row| row.ts != Ts::NotOffered)
}

/// A signature in the guest's types.
struct Sig {
    /// The function's name.
    name: String,
    /// The receiver, as written (`&self`, `&mut self`, `self`).
    receiver: Option<String>,
    /// The parameters, `(name, type)`.
    params: Vec<(String, String)>,
    /// The result, a `Result<_, ExtError>`.
    output: String,
}

impl Sig {
    /// The signature's text after `pub fn`.
    fn text(&self) -> String {
        let mut parts: Vec<String> = self.receiver.iter().cloned().collect();
        parts.extend(self.params.iter().map(|(name, ty)| format!("{name}: {ty}")));
        format!("{}({}) -> {}", self.name, parts.join(", "), self.output)
    }

    /// The argument list of the request: each parameter converted by `arg`.
    fn args(&self, method: &str) -> String {
        let args: Vec<String> = self
            .params
            .iter()
            .map(|(name, ty)| {
                let value = if ty.starts_with("&mut ") {
                    format!("&*{name}")
                } else if ty.starts_with('&') {
                    name.clone()
                } else if let Some(target) = ty.strip_prefix("impl Into<").and_then(|rest| rest.strip_suffix('>')) {
                    format!("&Into::<{target}>::into({name})")
                } else if let Some(item) = ty.strip_prefix("impl IntoIterator<Item = ").and_then(|rest| rest.strip_suffix('>')) {
                    format!("&{name}.into_iter().collect::<Vec<{item}>>()")
                } else {
                    format!("&{name}")
                };
                format!("arg({value}, \"{method}\")?")
            })
            .collect();
        format!("vec![{}]", args.join(", "))
    }

    /// The body's lines after the request `request` (an expression answering
    /// the reply).
    fn finish(&self, request: &str) -> String {
        if self.output == "Result<(), ExtError>" {
            format!("{request}?;\n        Ok(())")
        } else {
            format!("let reply = {request}?;\n        JsonForm::from_value(&reply)")
        }
    }
}

/// Parses a Rust signature of `EXTRA_METHODS` (`fn table(&mut self, name:
/// &str) -> Result<TablePiece, ExtError>`).
fn parse_rust(text: &str) -> Sig {
    let rest = text.strip_prefix("fn ").unwrap_or_else(|| panic!("the Rust signature `{text}` must begin with `fn `"));
    let (name, rest) = rest.split_once('(').unwrap_or_else(|| panic!("the Rust signature `{text}` must have parameters"));
    let close = rest.rfind(") -> ").unwrap_or_else(|| panic!("the Rust signature `{text}` must give its result"));
    let mut receiver = None;
    let mut params = Vec::new();
    for part in split_params(&rest[..close]) {
        if part.ends_with("self") {
            receiver = Some(part);
        } else {
            let (param, ty) = part.split_once(": ").unwrap_or_else(|| panic!("the parameter `{part}` of `{text}` must be `name: type`"));
            params.push((param.to_string(), ty.to_string()));
        }
    }
    Sig { name: name.to_string(), receiver, params, output: rest[close + 5..].to_string() }
}

/// Splits a parameter list at its top-level commas.
fn split_params(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

/// The capitalized identifiers of a type's text (`Result<Vec<LineFrame>,
/// ExtError>` names `Result`, `Vec`, `LineFrame`, `ExtError`).
fn type_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() || c == '_' {
            current.push(c);
        } else {
            if current.starts_with(|first: char| first.is_uppercase()) {
                names.push(current.clone());
            }
            current.clear();
        }
    }
    names
}

/// The generator's view of the surface: where each name is defined.
struct Surface<'a> {
    inventory: &'a Inventory,
    /// The module each type name lives in, in the guest.
    homes: BTreeMap<String, String>,
}

impl<'a> Surface<'a> {
    /// Reads the homes: an item's name is at home in the first module that
    /// holds it (every module of the guest holds the names it re-exports); a
    /// type of `GUEST_OWN` that no module holds, in the first module whose
    /// items name it.
    fn new(inventory: &'a Inventory) -> Surface<'a> {
        let mut homes = BTreeMap::new();
        for (module, item) in inventory.items() {
            if offered(item) {
                homes.entry(item.name.clone()).or_insert_with(|| module.path.clone());
            }
        }
        for row in mapping::GUEST_OWN {
            let name = own_name(row);
            if homes.contains_key(name) {
                continue;
            }
            let home = inventory
                .items()
                .find(|(_, item)| item_types(item).iter().any(|ty| mapping::leaves(ty).types.iter().any(|leaf| leaf == row)))
                .map(|(module, _)| module.path.clone())
                .unwrap_or_else(|| panic!("the type `{row}` of `GUEST_OWN` must be named by an item under `src/api/`"));
            homes.insert(name.to_string(), home);
        }
        Surface { inventory, homes }
    }

    /// The guest's type for the normalized type `text`, a parameter's or
    /// (`result`) a result's; the names it uses are added to `used`.
    fn ty(&self, text: &str, result: bool, used: &mut BTreeSet<String>) -> String {
        if mapping::GUEST_OWN.contains(&text) {
            let name = own_name(text);
            used.insert(name.to_string());
            return name.to_string();
        }
        match mapping::shape(text) {
            Shape::Ref { mutable, inner } => {
                if result {
                    return self.ty(&inner, true, used);
                }
                let prefix = if mutable { "&mut " } else { "&" };
                format!("{prefix}{}", self.ty(&inner, false, used))
            }
            Shape::Dyn(bounds) => match bounds.as_slice() {
                [bound] if mapping::kind_of(bound).is_some() => {
                    used.insert(bound.clone());
                    bound.clone()
                }
                _ => panic!("the type `{text}` has no guest form"),
            },
            Shape::Generic { head, args } => match (head.as_str(), args.as_slice()) {
                ("Result", [ok, error]) if error == "ExtError" || error == "NotCharacters" => self.ty(ok, result, used),
                ("Option" | "Vec", [inner]) => format!("{head}<{}>", self.ty(inner, result, used)),
                ("Box", [inner]) => self.ty(inner, result, used),
                _ => panic!("the type `{text}` has no guest form: only `Option`, `Vec`, `Result` and `Box<dyn …>` are mapped"),
            },
            Shape::Tuple(elems) => format!("({})", elems.iter().map(|elem| self.ty(elem, result, used)).collect::<Vec<_>>().join(", ")),
            Shape::Slice(inner) => {
                let inner = self.ty(&inner, result, used);
                if result { format!("Vec<{inner}>") } else { format!("[{inner}]") }
            }
            Shape::Impl(_) => {
                for name in type_names(text) {
                    used.insert(name);
                }
                text.to_string()
            }
            Shape::Plain(name) => match name.as_str() {
                "str" if result => "String".to_string(),
                "Value" | "serde_json::Value" => "Value".to_string(),
                _ => {
                    if name.starts_with(|first: char| first.is_uppercase()) {
                        used.insert(name.clone());
                    }
                    name
                }
            },
        }
    }

    /// The guest's result for the normalized result `output`.
    fn output(&self, output: &str, used: &mut BTreeSet<String>) -> String {
        if let Shape::Generic { head, args } = mapping::shape(output)
            && head == "Result"
            && args.len() == 2
            && args[1] == "ExtError"
        {
            return self.output_inner(&args[0], used);
        }
        self.output_inner(output, used)
    }

    /// The `Result<_, ExtError>` of the value `inner`.
    fn output_inner(&self, inner: &str, used: &mut BTreeSet<String>) -> String {
        if inner == "()" {
            return "Result<(), ExtError>".to_string();
        }
        format!("Result<{}, ExtError>", self.ty(inner, true, used))
    }

    /// The guest's signature of the inventory's `function`.
    fn sig(&self, function: &Function, used: &mut BTreeSet<String>) -> Sig {
        let receiver = function.receiver.as_deref().map(|receiver| if receiver == "mut self" { "self".to_string() } else { receiver.to_string() });
        let params = function.params.iter().map(|param| (param.name.clone(), self.ty(&param.ty, false, used))).collect();
        Sig { name: function.name.clone(), receiver, params, output: self.output(&function.output, used) }
    }
}

/// Every type text of an item's crossing signatures: its methods that are
/// dispatched (an extension trait's all), a function's, a data type's
/// fields.
fn item_types(item: &Item) -> Vec<String> {
    let mut out = Vec::new();
    let mut add = |function: &Function| {
        out.extend(function.params.iter().map(|param| param.ty.clone()));
        out.push(function.output.clone());
    };
    match &item.kind {
        ItemKind::Trait { methods, .. } | ItemKind::Type { methods, .. } => {
            for method in methods {
                if item.class == ItemClass::ExtensionTrait || !mapping::is_excluded(&item.name, &method.name) {
                    add(method);
                }
            }
        }
        ItemKind::Function(function) => add(function),
        _ => {}
    }
    if let ItemKind::Type { fields, .. } = &item.kind {
        out.extend(fields.iter().map(|field| field.ty.clone()));
    }
    out
}

/// `text` as `///` lines at `indent`.
fn doc(text: &str, indent: &str) -> String {
    let text = plain(text);
    if text.is_empty() {
        return String::new();
    }
    format!("{indent}/// {text}\n")
}

/// The generated surface.
pub fn emit(inventory: &Inventory) -> String {
    let surface = Surface::new(inventory);
    let mut out = String::from(
        "//! Generated by `tests/api_surface.rs` from the sources under `src/api/`; do\n\
         //! not edit. `UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface`\n\
         //! rewrites it.\n\
         //!\n\
         //! The guest client's surface (refresh section 6.2): per module of the\n\
         //! extension API, the re-exports of its data types, unit enums and constants,\n\
         //! of the guest's own types and of the handler traits (`src/guest/hand/`), one\n\
         //! struct per handle kind with its forwarding methods, and the module's\n\
         //! functions and constructors, each forwarding through `abi`; last, the\n\
         //! compile-time assertions of the hand-written items.\n\
         \n\
         /// The names every module with code uses.\n\
         mod prelude {\n\
         \x20   pub(crate) use serde_json::Value;\n\
         \n\
         \x20   pub(crate) use crate::adapter::handles::{Handle, HandleKind};\n\
         \x20   pub(crate) use crate::adapter::values::{handle_to_value, JsonForm};\n\
         \x20   pub(crate) use crate::guest::abi;\n\
         \x20   pub(crate) use crate::guest::hand::arg;\n\
         \x20   pub(crate) use crate::runtime::ExtError;\n\
         }\n",
    );
    // Every module path and the folders above it.
    let mut paths: BTreeSet<String> = BTreeSet::new();
    for module in &inventory.modules {
        let mut prefix = String::new();
        for part in module.path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            paths.insert(prefix.clone());
        }
    }
    for path in paths.iter().filter(|path| !path.contains('/')) {
        out.push('\n');
        emit_tree(&surface, &paths, path, 0, &mut out);
    }
    out.push('\n');
    out.push_str(&assertions(&surface));
    out
}

/// `name` in snake case (`CitationProvider` is `citation_provider`).
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The type a parameter of a hand-written item has in its assertion: as
/// [`Surface::ty`] maps it, but `impl Into<T>` is `T`, `impl
/// IntoIterator<Item = T>` is `Vec<T>` (a function pointer names one type),
/// and `Arc<dyn X>` of an extension trait is the guest's `Arc<dyn X>`.
fn assertion_ty(surface: &Surface<'_>, text: &str, used: &mut BTreeSet<String>) -> String {
    if let Some(target) = text.strip_prefix("impl Into<").and_then(|rest| rest.strip_suffix('>')) {
        return surface.ty(target, false, used);
    }
    if let Some(item) = text.strip_prefix("impl IntoIterator<Item = ").and_then(|rest| rest.strip_suffix('>')) {
        return format!("Vec<{}>", surface.ty(item, false, used));
    }
    if let Shape::Generic { head, args } = mapping::shape(text)
        && head == "Arc"
        && let [inner] = args.as_slice()
        && let Shape::Dyn(bounds) = mapping::shape(inner)
        && let [bound] = bounds.as_slice()
    {
        used.insert("Arc".to_string());
        used.insert(bound.clone());
        return format!("Arc<dyn {bound}>");
    }
    surface.ty(text, false, used)
}

/// The result of an extension trait's method in the guest: the api's, its
/// types mapped, not wrapped (the handler answers it).
fn trait_output(surface: &Surface<'_>, output: &str, used: &mut BTreeSet<String>) -> String {
    if output == "()" {
        return "()".to_string();
    }
    if let Shape::Generic { head, args } = mapping::shape(output)
        && head == "Result"
        && let [ok, error] = args.as_slice()
        && error == "ExtError"
    {
        return format!("Result<{}, ExtError>", surface.ty(ok, true, used));
    }
    surface.ty(output, true, used)
}

/// The block of compile-time assertions (refresh section 6.2): one per item
/// of `GUEST_HAND` (a method, a constructor, or a whole type's
/// constructors) and per method of each extension trait, binding the
/// hand-written item to the signature the type mapping derives from the
/// api, so that a hand-written item that is missing or whose types drifted
/// fails the build under the feature `guest`.
fn assertions(surface: &Surface<'_>) -> String {
    let mut used = BTreeSet::new();
    let mut hand = String::new();
    for entry in mapping::GUEST_HAND {
        let (owner, method) = match entry.split_once('.') {
            Some((owner, method)) => (owner, Some(if method == "constructor" { "new" } else { method })),
            None => (*entry, None),
        };
        let item = surface
            .inventory
            .items()
            .map(|(_, item)| item)
            .find(|item| item.name == owner && offered(item))
            .unwrap_or_else(|| panic!("the item `{owner}` of `GUEST_HAND` must be under `src/api/`"));
        let functions: Vec<&Function> = methods(item)
            .iter()
            .filter(|function| match method {
                Some(method) => function.name == method,
                None => function.receiver.is_none(),
            })
            .collect();
        assert!(!functions.is_empty(), "the entry `{entry}` of `GUEST_HAND` must name a method or a constructor of `{owner}`");
        used.insert(owner.to_string());
        for function in functions {
            let mut params: Vec<String> = function.receiver.iter().map(|receiver| match receiver.as_str() {
                "&self" => format!("&{owner}"),
                "&mut self" => format!("&mut {owner}"),
                _ => owner.to_string(),
            }).collect();
            params.extend(function.params.iter().map(|param| assertion_ty(surface, &param.ty, &mut used)));
            let output = surface.output(&function.output, &mut used);
            let _ = writeln!(hand, "        let _: fn({}) -> {output} = {owner}::{};", params.join(", "), function.name);
        }
    }
    let mut traits = String::new();
    for (_, item) in surface.inventory.items() {
        if item.class != ItemClass::ExtensionTrait || !offered(item) {
            continue;
        }
        let name = &item.name;
        used.insert(name.clone());
        let _ = writeln!(traits, "\n    /// The methods of `{name}`, on any type implementing it.");
        let _ = writeln!(traits, "    fn {}_methods<T: {name} + ?Sized>() {{", snake(name));
        for function in methods(item) {
            let mut params: Vec<String> = function.receiver.iter().map(|receiver| match receiver.as_str() {
                "&self" => "&T".to_string(),
                "&mut self" => "&mut T".to_string(),
                _ => "T".to_string(),
            }).collect();
            params.extend(function.params.iter().map(|param| assertion_ty(surface, &param.ty, &mut used)));
            let output = trait_output(surface, &function.output, &mut used);
            let _ = writeln!(traits, "        let _: fn({}) -> {output} = T::{};", params.join(", "), function.name);
        }
        let _ = writeln!(traits, "    }}");
    }
    let mut out = String::from(
        "/// The compile-time assertions of the hand-written items (refresh section\n\
         /// 6.2): one per item of `GUEST_HAND` and per method of each extension\n\
         /// trait, binding the item written in `src/guest/hand/` to the signature the\n\
         /// type mapping derives from the api, so that a hand-written item that is\n\
         /// missing or whose types drifted fails the build.\n\
         #[allow(dead_code, clippy::type_complexity, reason = \"the assertions are checked when the crate compiles, nothing calls them, and each writes a whole signature\")]\n\
         mod assertions {\n",
    );
    if used.contains("Arc") {
        out.push_str("    use std::sync::Arc;\n\n");
    }
    out.push_str("    use crate::guest::generated::prelude::*;\n");
    for name in &used {
        if name == "Arc" || PRELUDE.contains(&name.as_str()) {
            continue;
        }
        let home = surface.homes.get(name).unwrap_or_else(|| panic!("the type `{name}` of an assertion must have a home in the guest"));
        let _ = writeln!(out, "    use {}::{name};", guest_path(home));
    }
    out.push_str("\n    /// The record-and-lend items (`GUEST_HAND`).\n    fn hand_written() {\n");
    out.push_str(&hand);
    out.push_str("    }\n");
    out.push_str(&traits);
    out.push_str("}\n");
    out
}

/// The module `path` and its submodules, at `depth`.
fn emit_tree(surface: &Surface<'_>, paths: &BTreeSet<String>, path: &str, depth: usize, out: &mut String) {
    let indent = "    ".repeat(depth);
    let name = path.rsplit('/').next().unwrap_or(path);
    let module = surface.inventory.module(path);
    match module {
        Some(module) => {
            let paragraph = plain(&module.doc);
            let _ = writeln!(out, "{indent}/// The guest's side of `flm:{path}`. {paragraph}");
        }
        None => {
            let _ = writeln!(out, "{indent}/// The guest's side of the modules under `flm:{path}`.");
        }
    }
    let _ = writeln!(out, "{indent}pub mod {name} {{");
    let mut first = true;
    if let Some(module) = module {
        let body = emit_module(surface, module, &format!("{indent}    "));
        if !body.is_empty() {
            out.push_str(&body);
            first = false;
        }
    }
    let prefix = format!("{path}/");
    let children: Vec<&String> = paths.iter().filter(|other| other.strip_prefix(&prefix).is_some_and(|rest| !rest.contains('/'))).collect();
    for child in children {
        if !first {
            out.push('\n');
        }
        first = false;
        emit_tree(surface, paths, child, depth + 1, out);
    }
    let _ = writeln!(out, "{indent}}}");
}

/// The contents of the module `module`, at `indent`.
fn emit_module(surface: &Surface<'_>, module: &Module, indent: &str) -> String {
    let mut api = Vec::new();
    let mut hand = Vec::new();
    let mut elsewhere = Vec::new();
    let mut items = Vec::new();
    let mut used = BTreeSet::new();
    let mut local = BTreeSet::new();
    for item in &module.items {
        if !offered(item) {
            continue;
        }
        local.insert(item.name.clone());
        if let Some(home) = defined_elsewhere(surface.inventory, &module.path, item) {
            elsewhere.push(format!("{indent}pub use {}::{};\n", guest_path(&home), item.name));
            continue;
        }
        match &item.class {
            ItemClass::ExtensionTrait => hand.push(item.name.clone()),
            _ if is_own(&item.name) => hand.push(item.name.clone()),
            ItemClass::Constant | ItemClass::DataClass | ItemClass::DataType | ItemClass::CoreReexport => api.push(item.name.clone()),
            ItemClass::SealedTrait | ItemClass::OpaqueType if mapping::KINDLESS.contains(&item.name.as_str()) => {
                items.push(kindless(surface, module, item, indent, &mut used));
            }
            ItemClass::SealedTrait | ItemClass::OpaqueType => items.push(handle_struct(surface, module, item, indent, &mut used)),
            ItemClass::Function => {
                if let ItemKind::Function(function) = &item.kind {
                    items.push(function_item(surface, module, item, function, indent, &mut used));
                }
            }
            ItemClass::ApiReexport => unreachable!("an `api` re-export is defined elsewhere"),
        }
    }
    // The own types of no module's item, at home here.
    for row in mapping::GUEST_OWN {
        let name = own_name(row);
        if surface.homes.get(name).is_some_and(|home| *home == module.path) && !local.contains(name) && !hand.iter().any(|other| other == name) {
            hand.push(name.to_string());
            local.insert(name.to_string());
        }
    }
    let mut out = String::new();
    if !items.is_empty() {
        let _ = writeln!(out, "{indent}use crate::guest::generated::prelude::*;");
    }
    let imports: Vec<String> = used
        .iter()
        .filter(|name| !local.contains(*name) && !PRELUDE.contains(&name.as_str()))
        .map(|name| {
            let home = surface.homes.get(name).unwrap_or_else(|| panic!("the type `{name}` of the module `{}` must have a home in the guest", module.path));
            format!("{indent}use {}::{name};\n", guest_path(home))
        })
        .collect();
    for line in &imports {
        out.push_str(line);
    }
    if !out.is_empty() && (!api.is_empty() || !hand.is_empty() || !elsewhere.is_empty()) {
        out.push('\n');
    }
    if !api.is_empty() {
        let names = if api.len() == 1 { api[0].clone() } else { format!("{{{}}}", api.join(", ")) };
        let _ = writeln!(out, "{indent}pub use crate::api::{}::{names};", module.path.replace('/', "::"));
    }
    if !hand.is_empty() {
        let names = if hand.len() == 1 { hand[0].clone() } else { format!("{{{}}}", hand.join(", ")) };
        let _ = writeln!(out, "{indent}pub use crate::guest::hand::{}::{names};", hand_file(&module.path));
    }
    for line in &elsewhere {
        out.push_str(line);
    }
    for item in items {
        out.push('\n');
        out.push_str(&item);
    }
    out
}

/// A method's or a function's text: its doc, `pub fn`, its body.
fn function_text(sig: &Sig, doc_text: &str, request: &str, guard: Option<&str>, attrs: &str, indent: &str) -> String {
    let mut out = doc(doc_text, indent);
    out.push_str(attrs);
    let _ = writeln!(out, "{indent}pub fn {} {{", sig.text());
    if let Some(guard) = guard {
        let _ = writeln!(out, "{indent}    {guard}");
    }
    let body = sig.finish(request).replace("\n        ", &format!("\n{indent}    "));
    let _ = writeln!(out, "{indent}    {body}");
    let _ = writeln!(out, "{indent}}}");
    out
}

/// The struct of a handle kind, its `JsonForm` and its methods.
fn handle_struct(surface: &Surface<'_>, module: &Module, item: &Item, indent: &str, used: &mut BTreeSet<String>) -> String {
    let name = &item.name;
    let kind = mapping::kind_of(name).unwrap_or_else(|| panic!("the item `{name}` of `{}` must have a kind row", module.file));
    let handle_kind = variant(kind.kind);
    let mut out = doc(&item.doc, indent);
    let _ = writeln!(out, "{indent}#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    let _ = writeln!(out, "{indent}pub struct {name} {{");
    if kind.rehearsable {
        let _ = writeln!(out, "{indent}    /// The handle the host lent for it; `None` in a rehearsal, whose builder sends nothing.");
        let _ = writeln!(out, "{indent}    pub(crate) handle: Option<Handle>,");
    } else {
        let _ = writeln!(out, "{indent}    /// The handle the host lent for it.");
        let _ = writeln!(out, "{indent}    pub(crate) handle: Handle,");
    }
    let _ = writeln!(out, "{indent}}}");
    out.push('\n');
    let _ = writeln!(out, "{indent}impl JsonForm for {name} {{");
    if kind.rehearsable {
        let _ = writeln!(out, "{indent}    fn to_value(&self) -> Value {{");
        let _ = writeln!(out, "{indent}        self.handle.map_or(Value::Null, handle_to_value)");
        let _ = writeln!(out, "{indent}    }}");
        out.push('\n');
        let _ = writeln!(out, "{indent}    fn from_value(value: &Value) -> Result<Self, ExtError> {{");
        let _ = writeln!(out, "{indent}        if value.is_null() {{");
        let _ = writeln!(out, "{indent}            return Ok({name} {{ handle: None }});");
        let _ = writeln!(out, "{indent}        }}");
        let _ = writeln!(out, "{indent}        Ok({name} {{ handle: Some(abi::handle_of(value, HandleKind::{handle_kind})?) }})");
        let _ = writeln!(out, "{indent}    }}");
    } else {
        let _ = writeln!(out, "{indent}    fn to_value(&self) -> Value {{");
        let _ = writeln!(out, "{indent}        handle_to_value(self.handle)");
        let _ = writeln!(out, "{indent}    }}");
        out.push('\n');
        let _ = writeln!(out, "{indent}    fn from_value(value: &Value) -> Result<Self, ExtError> {{");
        let _ = writeln!(out, "{indent}        Ok({name} {{ handle: abi::handle_of(value, HandleKind::{handle_kind})? }})");
        let _ = writeln!(out, "{indent}    }}");
    }
    let _ = writeln!(out, "{indent}}}");
    let inner = format!("{indent}    ");
    let mut body = Vec::new();
    for method in methods(item) {
        if mapping::is_excluded(name, &method.name) || is_guest_hand(name, &method.name) {
            continue;
        }
        if method.is_constructor() {
            let sig = surface.sig(method, used);
            let function = format!("{name}::{}", method.name);
            let request = format!("abi::call_module(\"{}\", \"{function}\", {})", module.path, sig.args(&function));
            body.push(function_text(&sig, &method.doc, &request, None, "", &inner));
            continue;
        }
        let extra = mapping::EXTRA_METHODS.iter().find(|extra| extra.owner == kind.kind && extra.name == method.name);
        let sig = match extra {
            Some(extra) => {
                type_names(extra.rust).into_iter().for_each(|name| {
                    used.insert(name);
                });
                parse_rust(extra.rust)
            }
            None => surface.sig(method, used),
        };
        body.push(method_text(&sig, &method.doc, kind.rehearsable, &inner));
    }
    for extra in mapping::EXTRA_METHODS.iter().filter(|extra| extra.owner == kind.kind) {
        if methods(item).iter().any(|method| method.name == extra.name) {
            continue;
        }
        type_names(extra.rust).into_iter().for_each(|name| {
            used.insert(name);
        });
        let sig = parse_rust(extra.rust);
        let doc_text = format!("{} (a method of the guest client only).", extra.reason.split(';').next().unwrap_or(extra.reason));
        if extra.js.is_some() {
            let mut names: Vec<String> = vec!["self".to_string()];
            names.extend(sig.params.iter().map(|(param, _)| param.clone()));
            let mut text = doc(&doc_text, &inner);
            let _ = writeln!(text, "{inner}pub fn {} {{", sig.text());
            let _ = writeln!(text, "{inner}    {}({})", sig.name, names.join(", "));
            let _ = writeln!(text, "{inner}}}");
            body.push(text);
        } else {
            body.push(method_text(&sig, &doc_text, kind.rehearsable, &inner));
        }
    }
    if !body.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "{indent}impl {name} {{");
        out.push_str(&body.join("\n"));
        let _ = writeln!(out, "{indent}}}");
    }
    out
}

/// A forwarding method on `self.handle`.
fn method_text(sig: &Sig, doc_text: &str, rehearsable: bool, indent: &str) -> String {
    let args = sig.args(&sig.name);
    if rehearsable {
        assert_eq!(sig.output, "Result<(), ExtError>", "a method of a rehearsable kind must answer nothing: `{}`", sig.name);
        let request = format!("abi::call(handle, \"{}\", {args})", sig.name);
        return function_text(sig, doc_text, &request, Some("let Some(handle) = self.handle else { return Ok(()) };"), "", indent);
    }
    let request = format!("abi::call(self.handle, \"{}\", {args})", sig.name);
    function_text(sig, doc_text, &request, None, "", indent)
}

/// An opaque type without a kind of its own: an empty enum holding its
/// constructor, which answers an object of another kind.
fn kindless(surface: &Surface<'_>, module: &Module, item: &Item, indent: &str, used: &mut BTreeSet<String>) -> String {
    let name = &item.name;
    let mut out = doc(&item.doc, indent);
    let _ = writeln!(out, "{indent}#[derive(Debug)]");
    let _ = writeln!(out, "{indent}pub enum {name} {{}}");
    let inner = format!("{indent}    ");
    let mut body = Vec::new();
    for method in methods(item).iter().filter(|method| method.is_constructor() && !mapping::is_excluded(name, &method.name)) {
        let sig = surface.sig(method, used);
        let function = format!("{name}::{}", method.name);
        let request = format!("abi::call_module(\"{}\", \"{function}\", {})", module.path, sig.args(&function));
        let attrs = format!("{inner}#[allow(clippy::new_ret_no_self, reason = \"the type has no object of its own: its constructor answers one of another kind\")]\n");
        body.push(function_text(&sig, &method.doc, &request, None, &attrs, &inner));
    }
    if !body.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "{indent}impl {name} {{");
        out.push_str(&body.join("\n"));
        let _ = writeln!(out, "{indent}}}");
    }
    out
}

/// A module function, forwarding on the module's handle; an extra's Rust
/// signature where the function is an extra.
fn function_item(surface: &Surface<'_>, module: &Module, item: &Item, function: &Function, indent: &str, used: &mut BTreeSet<String>) -> String {
    let extra = mapping::EXTRA_METHODS.iter().find(|extra| extra.owner == module.path && extra.name == function.name);
    let sig = match extra {
        Some(extra) => {
            type_names(extra.rust).into_iter().for_each(|name| {
                used.insert(name);
            });
            parse_rust(extra.rust)
        }
        None => surface.sig(function, used),
    };
    let request = format!("abi::call_module(\"{}\", \"{}\", {})", module.path, function.name, sig.args(&function.name));
    function_text(&sig, &item.doc, &request, None, "", indent)
}

/// The counts of the output: its structs, the methods of its impl blocks
/// (the constructors of the kindless types among them) and its module
/// functions.
pub fn counts(text: &str) -> (usize, usize, usize) {
    let (mut structs, mut methods, mut functions) = (0, 0, 0);
    // The indents of the open impl blocks.
    let mut impls: Vec<usize> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if trimmed.starts_with("impl ") && trimmed.ends_with('{') {
            impls.push(indent);
        } else if trimmed == "}" && impls.last() == Some(&indent) {
            impls.pop();
        } else if trimmed.starts_with("pub struct ") {
            structs += 1;
        } else if trimmed.starts_with("pub fn ") {
            if impls.is_empty() { functions += 1 } else { methods += 1 }
        }
    }
    (structs, methods, functions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rust_signature_of_an_extra_is_parsed() {
        let sig = parse_rust("fn get_at(&self, name: &str, node: &Node) -> Result<Option<Vec<Value>>, ExtError>");
        assert_eq!(sig.receiver.as_deref(), Some("&self"));
        assert_eq!(sig.params, [("name".to_string(), "&str".to_string()), ("node".to_string(), "&Node".to_string())]);
        assert_eq!(sig.output, "Result<Option<Vec<Value>>, ExtError>");
        assert_eq!(variant("ext_table_read"), "ExtTableRead");
        assert_eq!(type_names("Result<Vec<LineFrame>, ExtError>"), ["Result", "Vec", "LineFrame", "ExtError"]);
    }
}
