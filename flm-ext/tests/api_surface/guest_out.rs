//! The emitter of the guest client's generated surface: one file,
//! `src/guest/generated.rs`, which
//! `src/guest/mod.rs` includes as `mod generated; pub use generated::*;`.
//!
//! # The shape of the output
//!
//! One inline module per module of the extension API, nested as the paths
//! nest (`flm_ext::guest::core::lowering` mirrors `flm:core/lowering`), in
//! the order of the paths; a folder that is no module of the surface (`core`,
//! `stdlib/features`) is a module holding its submodules. A module holds,
//! in order: the import of the names every body uses (`prelude`), the
//! imports of the types its signatures name from other modules, then its
//! re-exports (`pub use crate::api::a::b::{…}` for every data type, unit enum
//! and constant it defines; `pub use crate::guest::hand::a_b::{…}` for every
//! type of `GUEST_OWN` and every extension trait it defines, and every type
//! `GUEST_ADDED` adds to it; `pub use
//! crate::guest::<home>::X;` for an item it re-exports from another module),
//! then its items in source order: one struct per handle kind whose row names
//! a façade trait or an opaque type of the module (an extension trait is the
//! hand-written trait of that name; a type of `GUEST_OWN` the hand-written
//! type), with its `JsonForm` (the `@handle` form, its kind checked) and one
//! method per dispatched method that is not in `GUEST_HAND`, plus the kind's
//! `EXTRA_METHODS` in their Rust signature; an empty enum per opaque type
//! without a kind of its own, holding its constructor; one function per free
//! function that is not in `GUEST_OMITTED`. Last, after the modules, the block of compile-time assertions
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
//! `DurableTemplateFacade` are both the own `CompiledTemplate`; the list and
//! the slice of a list sequence its class, `Vec<Box<dyn Node>>` and
//! `[Box<dyn Node>]` the own `Nodes`); any other
//! type is the api's, re-exported; a borrowed result is answered owned (`&str`
//! is `String`, `&[T]` is `Vec<T>`, `&T` is `T`); a result is a `Result<_,
//! ExtError>` (`Result<T, NotCharacters>`, which crosses as an error of kind
//! `not-characters`, is `T` inside it; `()` is `Result<(), ExtError>`).
//!
//! # The forwarding bodies
//!
//! A method or a function has two bodies, chosen by `flm-ext`'s cargo
//! feature `native`, its signature and its documentation written once.
//!
//! - **The JSON body** (`cfg(not(feature = "native"))`): a method sends
//!   `abi::call(self.handle, "<method>", vec![…])`, a function or a
//!   constructor `abi::call_module("<path>", "<function>", vec![…])` (a
//!   constructor's function is `Type::new`), each argument converted by
//!   `ToArg::arg(value)`, the reply read by its type's `JsonForm`, a unit
//!   reply dropped.
//! - **The typed body** (`cfg(feature = "native")`): `abi::typed("<method>",
//!   |table| { … })`, the closure the dispatcher's arm for the same method
//!   with the conversions swapped, rendered from the arm's plan
//!   ([`rust_out::plan`]): every value argument converted by `IntoApi` into a
//!   local `a<i>` (a value of a type of `GUEST_OWN` by reference; an `impl
//!   Into<T>` converted and an `impl IntoIterator` collected before the
//!   closure), the receiver and the borrowed façade arguments taken from the
//!   table by the handles their structs hold, the api's method or function
//!   called (a module function by its path: no module handle), the answer
//!   converted by `FromApi` (`Ok(())` for a unit answer). A method or a
//!   function whose dispatcher arm is hand-written (`HAND_ARMS`,
//!   `EXTRA_METHODS`) has the typed body the mapping table writes for it,
//!   which calls the function of `dispatch.rs`'s module `extra` that the arm
//!   calls. A parameter named as a local of the body (`table`) is bound to
//!   `<name>_arg` before the closure.
//!
//! On `FeatureBuilder`, whose kind is rehearsable, a method answers `Ok(())`
//! without a request when the builder is a rehearsal's (its handle `None`),
//! before either body. An extra with a stub of its own (`Lowerer::pres_tree`)
//! forwards to the module's function of its name, the receiver first.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::inventory::{Function, Inventory, Item, ItemClass, ItemKind, Module};
use crate::js_out::{defined_elsewhere, plain};
use crate::mapping::{self, KindRow, Shape, Ts};
use crate::rust_out::{self, ArmPlan, Param, Source, Target};

/// The generated surface, from the crate's root.
pub const GENERATED_RS: &str = "src/guest/generated.rs";

/// The names of the prelude every module with code imports, and the names of
/// the standard library a signature uses; never imported
/// from a module.
const PRELUDE: &[&str] = &["Value", "Handle", "HandleKind", "JsonForm", "ExtError", "Result", "Option", "Vec", "String", "Self", "Into", "IntoIterator", "Item"];

/// The guest's own type a row of `GUEST_OWN` stands for: its own name, but
/// for the two rows of the one own `CompiledTemplate`, and for the list and
/// the slice of a list sequence, its class (`Nodes`).
fn own_name(row: &str) -> &str {
    match row {
        "Box<dyn CompiledTemplate>" | "DurableTemplateFacade" => "CompiledTemplate",
        other => mapping::list_of(other).map_or(other, |list| list.class()),
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

    /// The argument list of the request: each parameter converted by
    /// `ToArg::arg`.
    fn args(&self) -> String {
        format!("vec![{}]", self.arg_values().join(", "))
    }

    /// Each parameter converted by `ToArg::arg`, in order.
    fn arg_values(&self) -> Vec<String> {
        self
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
                    format!("&{}", collected(name, item))
                } else {
                    format!("&{name}")
                };
                format!("ToArg::arg({value})")
            })
            .collect()
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
    /// Reads the homes: a list sequence's class is at home in the module of
    /// its row; an item's name in the first module that holds it (every
    /// module of the guest holds the names it re-exports); a type of
    /// `GUEST_OWN` that no module holds, in the first module whose items name
    /// it.
    fn new(inventory: &'a Inventory) -> Surface<'a> {
        let mut homes = BTreeMap::new();
        // A list sequence's class is at home in the module of its row.
        for list in mapping::LISTS {
            homes.insert(list.class().to_string(), list.module.to_string());
        }
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
    if let Some(items) = mapping::kind_of(&item.name).and_then(mapping::KindRow::pulls) {
        out.push(items.to_string());
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
         //! The guest client's surface: per module of the extension API, the\n\
         //! re-exports of its data types, unit enums and constants,\n\
         //! of the guest's own types and of the handler traits (`src/guest/hand/`), one\n\
         //! struct per handle kind with its forwarding methods, and the module's\n\
         //! functions and constructors, each forwarding through `abi` (as a JSON\n\
         //! request, or with the cargo feature `native` as a typed call through\n\
         //! `abi::typed`); last, the compile-time assertions of the hand-written items.\n\
         \n\
         /// The names every module with code uses: those of both routes, of the\n\
         /// JSON route (`ToArg`), and of the typed route (the cargo feature `native`).\n\
         mod prelude {\n\
         \x20   pub(crate) use serde_json::Value;\n\
         \n\
         \x20   pub(crate) use crate::adapter::handles::{Handle, HandleKind};\n\
         \x20   pub(crate) use crate::adapter::values::{handle_to_value, JsonForm};\n\
         \x20   pub(crate) use crate::guest::abi;\n\
         \x20   pub(crate) use crate::runtime::ExtError;\n\
         \n\
         \x20   #[cfg(not(feature = \"native\"))]\n\
         \x20   pub(crate) use crate::guest::abi::ToArg;\n\
         \n\
         \x20   #[cfg(feature = \"native\")]\n\
         \x20   pub(crate) use crate::adapter::dispatch::{entry, extra};\n\
         \x20   #[cfg(feature = \"native\")]\n\
         \x20   pub(crate) use crate::adapter::handles::Entry;\n\
         \x20   #[cfg(feature = \"native\")]\n\
         \x20   pub(crate) use crate::adapter::typed::{FromApi, IntoApi};\n\
         \x20   #[cfg(feature = \"native\")]\n\
         \x20   pub(crate) use crate::adapter::values::{mismatch, Group};\n\
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

/// The block of compile-time assertions: one per item
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
        "/// The compile-time assertions of the hand-written items: one per item of\n\
         /// `GUEST_HAND` and per method of each extension trait, binding the item\n\
         /// written in `src/guest/hand/` to the signature the type mapping derives\n\
         /// from the api, so that a hand-written item that is missing or whose types\n\
         /// drifted fails the build.\n\
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
        if !offered(item) || (item.class == ItemClass::Function && mapping::is_guest_omitted(&module.path, &item.name)) {
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
    // The types the guest adds to the module.
    for (path, name) in mapping::GUEST_ADDED {
        if *path == module.path {
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

/// The locals a typed body binds: the closure's table, the receiver, the
/// answer (and the value arguments, `a<i>`). A parameter of one of these
/// names is bound to `<name>_arg` before the closure ([`typed_name`]).
const TYPED_LOCALS: &[&str] = &["table", "this", "answer"];

/// The name a typed body reaches the parameter `name` by: its own, or
/// `<name>_arg` for the name of a local of the body.
fn typed_name(name: &str) -> String {
    let numbered = name.len() > 1 && name.starts_with('a') && name[1..].chars().all(|c| c.is_ascii_digit());
    if numbered || TYPED_LOCALS.contains(&name) { format!("{name}_arg") } else { name.to_string() }
}

/// The typed body of a request (the cargo feature `native`): the method's
/// or the function's name as the JSON request sends it, and the lines inside
/// `abi::typed`'s closure.
struct Typed {
    /// The name, which `abi::typed` gives the errors and the refusal.
    method: String,
    /// The lines inside the closure.
    lines: Vec<String>,
    /// Whether the body reads the count `unhanded` of a traversal's
    /// method ([`ArmPlan::unhanded`]), which the request sends last.
    unhanded: bool,
}

impl Typed {
    /// A typed body written in the mapping table (an arm of `HAND_ARMS` or an
    /// extra of `EXTRA_METHODS`): its lines.
    fn written(method: &str, text: &str) -> Typed {
        Typed { method: method.to_string(), lines: text.lines().map(str::to_string).collect(), unhanded: false }
    }

    /// The typed body of the request `plan`, the dispatcher's arm with the
    /// conversions swapped: each value argument of `sig` converted by
    /// `IntoApi`, the receiver's handle `receiver` and each borrowed façade
    /// argument's handle taken from the table as the arm takes them, the
    /// answer converted by `FromApi`. `display` names the item in the
    /// generator's messages.
    fn planned(plan: &ArmPlan, sig: &Sig, receiver: &str, method: &str, display: &str) -> Typed {
        let mut lines = Vec::new();
        for (i, (param, (name, ty))) in plan.params.iter().zip(&sig.params).enumerate() {
            if let Param::Value { .. } = param {
                lines.push(format!("let a{i} = IntoApi::into_api({}, table)?;", guest_value(&typed_name(name), ty)));
            }
        }
        let handle = |source: &Source| match source {
            Source::Receiver => receiver.to_string(),
            Source::Arg(i) => {
                let (name, ty) = &sig.params[*i];
                let bare = ty.trim_start_matches("&mut ").trim_start_matches('&');
                assert!(
                    !is_own(bare),
                    "the borrowed façade `{name}` of `{display}` is of the guest's own type `{bare}`: the method must have a hand arm with a \
                     typed body (`HAND_ARMS`)"
                );
                format!("{}.handle", typed_name(name))
            }
        };
        let passed: Vec<String> = plan
            .passed
            .iter()
            .zip(&sig.params)
            .map(|(pass, (_, ty))| if borrowed_plain(ty) { pass.trim_start_matches('&').to_string() } else { pass.clone() })
            .collect();
        lines.extend(plan.take_and_call(handle, &passed, &format!("\"{method}\"")));
        lines.push(if plan.unit { "Ok(())".to_string() } else { "FromApi::from_api(answer, table)".to_string() });
        Typed { method: method.to_string(), lines, unhanded: plan.unhanded }
    }
}

/// The line that prepares the parameter `name` of the guest's type `ty`
/// before the typed body's closure, if it needs one: an `impl Into<T>`
/// converted to `T` and an `impl IntoIterator<Item = T>` collected into a
/// `Vec<T>` (the types the arm's `FromArg` reads), since the extension's code
/// may run in either and the frame's closure runs none (`binding/frames.rs`,
/// invariant 2); a parameter named as a local of the body rebound
/// ([`typed_name`]).
fn prepared(name: &str, ty: &str) -> Option<String> {
    let local = typed_name(name);
    if let Some(target) = ty.strip_prefix("impl Into<").and_then(|rest| rest.strip_suffix('>')) {
        return Some(format!("let {local} = Into::<{target}>::into({name});"));
    }
    if let Some(item) = ty.strip_prefix("impl IntoIterator<Item = ").and_then(|rest| rest.strip_suffix('>')) {
        return Some(format!("let {local} = {};", collected(name, item)));
    }
    (local != name).then(|| format!("let {local} = {name};"))
}

/// The expression that collects the parameter `name`, an `impl
/// IntoIterator<Item = ITEM>`, into a `Vec`: of `T` for an item `impl
/// Into<T>`, each item converted, else of `ITEM`.
fn collected(name: &str, item: &str) -> String {
    match item.strip_prefix("impl Into<").and_then(|rest| rest.strip_suffix('>')) {
        Some(target) => format!("{name}.into_iter().map(Into::into).collect::<Vec<{target}>>()"),
        None => format!("{name}.into_iter().collect::<Vec<{item}>>()"),
    }
}

/// Whether the guest's borrowed type `ty` (`&T`, `&[T]`) is plain data,
/// neither a type of `GUEST_OWN` nor the struct of a handle kind: its
/// `IntoApi` answers the same reference, which the call takes as it is,
/// where the dispatcher's arm borrows the owned value it read.
fn borrowed_plain(ty: &str) -> bool {
    let Some(inner) = ty.strip_prefix('&') else { return false };
    if inner.starts_with("mut ") {
        return false;
    }
    let inner = inner.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).unwrap_or(inner);
    !is_own(inner) && mapping::kind_of(inner).is_none()
}

/// The value a typed body converts for the parameter `name` (as the body
/// reaches it) of the guest's type `ty`: the parameter, reborrowed when
/// `&mut`, borrowed when it is a value of a type of `GUEST_OWN`, which may
/// hold handler objects of the extension and is then dropped after the
/// call, outside the frame's closure.
fn guest_value(name: &str, ty: &str) -> String {
    if ty.starts_with("&mut ") {
        return format!("&*{name}");
    }
    if is_own(ty) {
        return format!("&{name}");
    }
    name.to_string()
}

/// A method's or a function's text: its doc, `pub fn`, the guard, then its
/// two bodies: the JSON one (`request`, then the reply read) under
/// `cfg(not(feature = "native"))`, the typed one (`typed`, inside
/// `abi::typed`) under `cfg(feature = "native")`.
fn function_text(sig: &Sig, doc_text: &str, request: &str, typed: &Typed, guard: Option<&str>, attrs: &str, indent: &str) -> String {
    let mut out = doc(doc_text, indent);
    out.push_str(attrs);
    let _ = writeln!(out, "{indent}pub fn {} {{", sig.text());
    if let Some(guard) = guard {
        let _ = writeln!(out, "{indent}    {guard}");
    }
    let _ = writeln!(out, "{indent}    #[cfg(not(feature = \"native\"))]");
    let _ = writeln!(out, "{indent}    {{");
    let body = sig.finish(request).replace("\n        ", &format!("\n{indent}        "));
    let _ = writeln!(out, "{indent}        {body}");
    let _ = writeln!(out, "{indent}    }}");
    let _ = writeln!(out, "{indent}    #[cfg(feature = \"native\")]");
    let _ = writeln!(out, "{indent}    {{");
    for line in sig.params.iter().filter_map(|(name, ty)| prepared(name, ty)) {
        let _ = writeln!(out, "{indent}        {line}");
    }
    let table = if typed.lines.iter().any(|line| line.contains("table")) { "table" } else { "_" };
    let _ = writeln!(out, "{indent}        abi::typed(\"{}\", |{table}| {{", typed.method);
    for line in &typed.lines {
        let _ = writeln!(out, "{indent}            {line}");
    }
    let _ = writeln!(out, "{indent}        }})");
    let _ = writeln!(out, "{indent}    }}");
    let _ = writeln!(out, "{indent}}}");
    out
}

/// The typed body written in the mapping table for the hand-written arm or
/// the extra of `owner` (a kind, or a module's path) named `name`, if it has
/// one.
fn written_typed(owner: &str, name: &str) -> Option<Typed> {
    let extra = mapping::EXTRA_METHODS.iter().find(|extra| extra.owner == owner && extra.name == name).map(|extra| extra.typed);
    let hand = mapping::HAND_ARMS.iter().find(|entry| entry.kind == owner && entry.method == name).map(|entry| entry.typed);
    let text = extra.or(hand)?;
    let text = text.unwrap_or_else(|| panic!("the hand-written arm or the extra `{owner}.{name}` must have a typed body in the mapping table"));
    Some(Typed::written(name, text))
}

/// The typed body of the method `method` of the kind `kind` in the guest's
/// signature `sig`: the one the mapping table writes for a hand-written arm
/// or an extra, or the method's planned one, its receiver's handle
/// `receiver`.
fn method_typed(kind: &'static KindRow, owner: &str, method: &Function, sig: &Sig, file: &str, receiver: &str) -> Typed {
    if let Some(typed) = written_typed(kind.kind, &method.name) {
        return typed;
    }
    assert!(
        matches!(
            kind.receiver,
            mapping::Receiver::LentRef
                | mapping::Receiver::LentMut
                | mapping::Receiver::Boxed
                | mapping::Receiver::Cloned
                | mapping::Receiver::Pulled(_)
                | mapping::Receiver::Indexed(_)
        ),
        "the method `{owner}::{}` of the kind `{}` must have a hand-written arm with a typed body: the dispatcher generates no arm for its kind",
        method.name,
        kind.kind
    );
    let mode = method.receiver.as_deref().unwrap_or("&self");
    let plan = rust_out::plan(Some(owner), method, file, &Target::Method { kind, receiver: mode });
    Typed::planned(&plan, sig, receiver, &sig.name, &format!("{owner}::{}", method.name))
}

/// The typed body of the free function or constructor `function` of the
/// module `module` (a constructor's name `Type::new`, its `owner` the type).
fn module_typed(module: &Module, owner: Option<&str>, function: &Function, name: &str, sig: &Sig) -> Typed {
    if let Some(typed) = written_typed(&module.path, name) {
        return typed;
    }
    let path = format!("crate::api::{}::{name}", module.path.replace('/', "::"));
    let plan = rust_out::plan(owner, function, &module.file, &Target::Path(path));
    Typed::planned(&plan, sig, "", name, name)
}

/// The struct of a handle kind, its `JsonForm` and its methods.
fn handle_struct(surface: &Surface<'_>, module: &Module, item: &Item, indent: &str, used: &mut BTreeSet<String>) -> String {
    let name = &item.name;
    let kind = mapping::kind_of(name).unwrap_or_else(|| panic!("the item `{name}` of `{}` must have a kind row", module.file));
    let handle_kind = variant(kind.kind);
    if let Some(items) = kind.pulls() {
        let mut out = match kind.receiver {
            mapping::Receiver::Indexed(_) => sequence_struct(surface, item, kind, items, indent, used),
            _ => traversal_struct(surface, item, kind, items, indent, used),
        };
        out.push_str(&methods_block(surface, module, item, kind, indent, used));
        return out;
    }
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
    out.push_str(&methods_block(surface, module, item, kind, indent, used));
    out
}

/// The `impl` block of the methods of the handle kind `kind`, whose struct
/// is `item`: its forwarding methods, its constructors and its extras; empty
/// when it has none.
fn methods_block(surface: &Surface<'_>, module: &Module, item: &Item, kind: &'static KindRow, indent: &str, used: &mut BTreeSet<String>) -> String {
    let name = &item.name;
    let mut out = String::new();
    let inner = format!("{indent}    ");
    let mut body = Vec::new();
    for method in methods(item) {
        if mapping::is_excluded(name, &method.name) || is_guest_hand(name, &method.name) {
            continue;
        }
        if method.is_constructor() {
            let sig = surface.sig(method, used);
            let function = format!("{name}::{}", method.name);
            let request = format!("abi::call_module(\"{}\", \"{function}\", {})", module.path, sig.args());
            let typed = module_typed(module, Some(name), method, &function, &sig);
            body.push(function_text(&sig, &method.doc, &request, &typed, None, "", &inner));
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
        let receiver = if kind.rehearsable { "handle" } else { "self.handle" };
        let typed = method_typed(kind, name, method, &sig, &module.file, receiver);
        body.push(method_text(&sig, &method.doc, kind.rehearsable, &typed, &inner));
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
            let typed = written_typed(kind.kind, extra.name).unwrap_or_else(|| panic!("the extra `{}.{}` must have a typed body", kind.kind, extra.name));
            body.push(method_text(&sig, &doc_text, kind.rehearsable, &typed, &inner));
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

/// The struct of the traversal kind `kind`, whose items are of the
/// (normalized) type `items`: its handle and the items pulled and not handed
/// over yet ([`abi::Pulled`]), its `JsonForm` (the `@handle` form, its kind
/// checked), and its `Iterator` of `Result`s, whose `next` pulls a group when
/// the last one is spent, by either body (a request `pull`, or the typed
/// body that takes the items from the traversal's entry).
fn traversal_struct(surface: &Surface<'_>, item: &Item, kind: &KindRow, items: &str, indent: &str, used: &mut BTreeSet<String>) -> String {
    let name = &item.name;
    let handle_kind = variant(kind.kind);
    let items = surface.ty(items, true, used);
    let mut out = doc(&item.doc, indent);
    let _ = writeln!(out, "{indent}#[derive(Debug)]");
    let _ = writeln!(out, "{indent}pub struct {name} {{");
    let _ = writeln!(out, "{indent}    /// The handle the host lent for it.");
    let _ = writeln!(out, "{indent}    pub(crate) handle: Handle,");
    let _ = writeln!(out, "{indent}    /// The items pulled and not handed over yet.");
    let _ = writeln!(out, "{indent}    pub(crate) pulled: abi::Pulled<{items}>,");
    let _ = writeln!(out, "{indent}}}");
    out.push('\n');
    let _ = writeln!(out, "{indent}impl JsonForm for {name} {{");
    let _ = writeln!(out, "{indent}    fn to_value(&self) -> Value {{");
    let _ = writeln!(out, "{indent}        handle_to_value(self.handle)");
    let _ = writeln!(out, "{indent}    }}");
    out.push('\n');
    let _ = writeln!(out, "{indent}    fn from_value(value: &Value) -> Result<Self, ExtError> {{");
    let _ = writeln!(out, "{indent}        Ok({name} {{ handle: abi::handle_of(value, HandleKind::{handle_kind})?, pulled: abi::Pulled::default() }})");
    let _ = writeln!(out, "{indent}    }}");
    let _ = writeln!(out, "{indent}}}");
    out.push('\n');
    let _ = writeln!(out, "{indent}impl Iterator for {name} {{");
    let _ = writeln!(out, "{indent}    type Item = Result<{items}, ExtError>;");
    out.push('\n');
    let _ = writeln!(out, "{indent}    fn next(&mut self) -> Option<Result<{items}, ExtError>> {{");
    let _ = writeln!(out, "{indent}        let handle = self.handle;");
    let _ = writeln!(out, "{indent}        self.pulled.next(|max| {{");
    let _ = writeln!(out, "{indent}            #[cfg(not(feature = \"native\"))]");
    let _ = writeln!(out, "{indent}            {{");
    let _ = writeln!(out, "{indent}                let reply = abi::call(handle, \"pull\", vec![ToArg::arg(&max)])?;");
    let _ = writeln!(out, "{indent}                JsonForm::from_value(&reply)");
    let _ = writeln!(out, "{indent}            }}");
    let _ = writeln!(out, "{indent}            #[cfg(feature = \"native\")]");
    let _ = writeln!(out, "{indent}            {{");
    let _ = writeln!(out, "{indent}                abi::typed(\"pull\", |table| {{");
    for line in rust_out::pull_lines(kind, "handle", &["max"]) {
        let _ = writeln!(out, "{indent}                    {line}");
    }
    let _ = writeln!(out, "{indent}                    FromApi::from_api(Group(answer), table)");
    let _ = writeln!(out, "{indent}                }})");
    let _ = writeln!(out, "{indent}            }}");
    let _ = writeln!(out, "{indent}        }})");
    let _ = writeln!(out, "{indent}    }}");
    let _ = writeln!(out, "{indent}}}");
    out
}

/// The struct of the sequence kind `kind`, whose items are of the
/// (normalized) type `items`: its handle and its length, its `JsonForm` (the
/// `@handle` form, its kind checked and its length read), and the methods
/// that need no crossing: `len`, `is_empty`, and `iter`, an iterator of
/// `Result`s that pulls the items in groups ([`abi::Paged`]) by either body
/// (a request `pull`, or the typed body that reads the group from the
/// sequence's entry).
fn sequence_struct(surface: &Surface<'_>, item: &Item, kind: &KindRow, items: &str, indent: &str, used: &mut BTreeSet<String>) -> String {
    let name = &item.name;
    let handle_kind = variant(kind.kind);
    let items = surface.ty(items, true, used);
    let mut out = doc(&item.doc, indent);
    let _ = writeln!(out, "{indent}#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    let _ = writeln!(out, "{indent}pub struct {name} {{");
    let _ = writeln!(out, "{indent}    /// The handle the host lent for it.");
    let _ = writeln!(out, "{indent}    pub(crate) handle: Handle,");
    let _ = writeln!(out, "{indent}    /// The number of its items, which crossed with the handle.");
    let _ = writeln!(out, "{indent}    pub(crate) len: usize,");
    let _ = writeln!(out, "{indent}}}");
    out.push('\n');
    let _ = writeln!(out, "{indent}impl JsonForm for {name} {{");
    let _ = writeln!(out, "{indent}    fn to_value(&self) -> Value {{");
    let _ = writeln!(out, "{indent}        handle_to_value(self.handle)");
    let _ = writeln!(out, "{indent}    }}");
    out.push('\n');
    let _ = writeln!(out, "{indent}    fn from_value(value: &Value) -> Result<Self, ExtError> {{");
    let _ = writeln!(out, "{indent}        let (handle, len) = abi::sequence_of(value, HandleKind::{handle_kind})?;");
    let _ = writeln!(out, "{indent}        Ok({name} {{ handle, len }})");
    let _ = writeln!(out, "{indent}    }}");
    let _ = writeln!(out, "{indent}}}");
    out.push('\n');
    let _ = writeln!(out, "{indent}impl {name} {{");
    let _ = writeln!(out, "{indent}    /// The number of items, with no crossing.");
    let _ = writeln!(out, "{indent}    pub fn len(&self) -> usize {{");
    let _ = writeln!(out, "{indent}        self.len");
    let _ = writeln!(out, "{indent}    }}");
    out.push('\n');
    let _ = writeln!(out, "{indent}    /// Whether it has no item, with no crossing.");
    let _ = writeln!(out, "{indent}    pub fn is_empty(&self) -> bool {{");
    let _ = writeln!(out, "{indent}        self.len == 0");
    let _ = writeln!(out, "{indent}    }}");
    out.push('\n');
    let _ = writeln!(out, "{indent}    /// The items, in order, pulled from the host in groups: an item is a `Result`, since a pull crosses.");
    let _ = writeln!(out, "{indent}    pub fn iter(&self) -> impl Iterator<Item = Result<{items}, ExtError>> {{");
    let _ = writeln!(out, "{indent}        let handle = self.handle;");
    let _ = writeln!(out, "{indent}        abi::Paged::new(self.len, move |from| {{");
    let _ = writeln!(out, "{indent}            #[cfg(not(feature = \"native\"))]");
    let _ = writeln!(out, "{indent}            {{");
    let _ = writeln!(
        out,
        "{indent}                let reply = abi::call(handle, \"pull\", vec![ToArg::arg(&from), ToArg::arg(&abi::PULL_GROUP), ToArg::arg(&abi::PULL_BYTES)])?;"
    );
    let _ = writeln!(out, "{indent}                JsonForm::from_value(&reply)");
    let _ = writeln!(out, "{indent}            }}");
    let _ = writeln!(out, "{indent}            #[cfg(feature = \"native\")]");
    let _ = writeln!(out, "{indent}            {{");
    let _ = writeln!(out, "{indent}                abi::typed(\"pull\", |table| {{");
    for line in rust_out::pull_lines(kind, "handle", &["from", "abi::PULL_GROUP", "abi::PULL_BYTES"]) {
        let _ = writeln!(out, "{indent}                    {line}");
    }
    let _ = writeln!(out, "{indent}                    FromApi::from_api(Group(answer), table)");
    let _ = writeln!(out, "{indent}                }})");
    let _ = writeln!(out, "{indent}            }}");
    let _ = writeln!(out, "{indent}        }})");
    let _ = writeln!(out, "{indent}    }}");
    let _ = writeln!(out, "{indent}}}");
    out
}

/// A forwarding method on `self.handle`, its typed body `typed`; for a
/// traversal's method, the count `unhanded` ([`abi::Pulled`]'s `held`) sent
/// last and read by the typed body.
fn method_text(sig: &Sig, doc_text: &str, rehearsable: bool, typed: &Typed, indent: &str) -> String {
    if typed.unhanded {
        // A traversal's method sends, last, the count of the items its
        // struct pulled and has not handed over; the typed body reads the
        // same local.
        let mut values = sig.arg_values();
        values.push("ToArg::arg(&unhanded)".to_string());
        let request = format!("abi::call(self.handle, \"{}\", vec![{}])", sig.name, values.join(", "));
        return function_text(sig, doc_text, &request, typed, Some("let unhanded = self.pulled.held();"), "", indent);
    }
    let args = sig.args();
    if rehearsable {
        assert_eq!(sig.output, "Result<(), ExtError>", "a method of a rehearsable kind must answer nothing: `{}`", sig.name);
        let request = format!("abi::call(handle, \"{}\", {args})", sig.name);
        return function_text(sig, doc_text, &request, typed, Some("let Some(handle) = self.handle else { return Ok(()) };"), "", indent);
    }
    let request = format!("abi::call(self.handle, \"{}\", {args})", sig.name);
    function_text(sig, doc_text, &request, typed, None, "", indent)
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
        let request = format!("abi::call_module(\"{}\", \"{function}\", {})", module.path, sig.args());
        let typed = module_typed(module, Some(name), method, &function, &sig);
        let attrs = format!("{inner}#[allow(clippy::new_ret_no_self, reason = \"the type has no object of its own: its constructor answers one of another kind\")]\n");
        body.push(function_text(&sig, &method.doc, &request, &typed, None, &attrs, &inner));
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
    let request = format!("abi::call_module(\"{}\", \"{}\", {})", module.path, function.name, sig.args());
    let typed = module_typed(module, None, function, &function.name, &sig);
    function_text(&sig, &item.doc, &request, &typed, None, "", indent)
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
