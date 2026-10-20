//! The emitter of the generated dispatcher `src/adapter/generated.rs` (the
//! plan's section 5.1, `dev-docs/ext/e2b`): the module list `MODULE_PATHS`,
//! the table of constants, the `names` arms, the kind `match` of the
//! dispatcher, one function per handle kind with one arm per dispatched
//! method, and the module function with the free functions, the
//! constructors and the `names` arms.
//!
//! # The shape of an arm
//!
//! In a fixed order, so that the borrows of the call's table work: the
//! arity; every value argument read into a local `a<i>` through `FromArg`,
//! in parameter order (a node copied out of the table); the receiver and the
//! borrowed façade arguments taken together through `entries_mut`, each
//! mismatch reported by position (or the one of them alone through
//! `entry!`, in the line of its kind's receiver mode); the call, `?` applied
//! when it answers a `Result` of `ExtError`; the reply through `IntoReply`
//! (`null` for a method answering nothing).
//!
//! # What is not generated
//!
//! The methods of `NOT_DISPATCHED`; the extras of `EXTRA_METHODS`, whose
//! arms `dispatch.rs` writes by hand (`StageOut::table`, `TablePiece::push`,
//! `Lowerer.get_at`, `arg_def_from_code`, `line_frames`; `Lowerer.pres_tree`
//! is a stub-only method with no arm); and the arms of `HAND_ARMS`, façade
//! methods whose generated arm would answer differently from the
//! hand-written one. Each kind function ends in the hand-written extras of
//! its kind, the module function in those of the modules.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::inventory::{Function, Inventory, Item, ItemClass, ItemKind};
use crate::mapping::{self, KindRow, Receiver};

/// The generated file, from the crate's root.
pub const GENERATED_RS: &str = "src/adapter/generated.rs";

/// The paths of the modules reached through a module handle, in sorted
/// order: the modules with a free function, a constant or a dispatched
/// constructor.
pub fn module_paths(inventory: &Inventory) -> Vec<String> {
    let mut paths: Vec<String> =
        inventory.modules.iter().filter(|module| crate::has_module_arms(module)).map(|module| module.path.clone()).collect();
    paths.sort();
    paths
}

/// The `HandleKind` (and `Entry`) variant of the kind `name`
/// (`feature_builder` is `FeatureBuilder`).
fn camel(name: &str) -> String {
    name.split('_').map(|part| {
        let mut chars = part.chars();
        chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
    }).collect()
}

/// Whether the kind `kind` has hand-written arms in `dispatch.rs`.
fn has_extras(kind: &str) -> bool {
    mapping::EXTRA_METHODS.iter().any(|extra| extra.owner == kind) || mapping::HAND_ARMS.iter().any(|entry| entry.kind == kind)
}

/// How one parameter is passed.
enum Param {
    /// A value read through `FromArg` into a local: its type (with the
    /// call's lifetime) and the expression that passes the local.
    Value { ty: String, pass: String },
    /// A borrowed façade taken from the table: its kind row and whether the
    /// borrow is `&mut`.
    Facade { kind: &'static KindRow, mutable: bool },
}

/// Where a parameter stands, as the generator's messages name it.
struct Site<'a> {
    /// The parameter's name.
    param: &'a str,
    /// The method (`Lowerer::lower_region`) or the free function.
    function: &'a str,
    /// The file that defines it, from the crate's root.
    file: &'a str,
}

/// The plan of the parameter of type `ty` (normalized) at position `i`.
fn param(ty: &str, i: usize, site: &Site<'_>) -> Param {
    let local = format!("a{i}");
    let facade = |text: &str| -> Option<&'static KindRow> {
        let bound = text.strip_prefix("dyn ")?;
        mapping::kind_of(bound).filter(|row| row.entry.is_some())
    };
    if let Some((inner, mutable)) = ty.strip_prefix("&mut ").map(|inner| (inner, true)).or_else(|| ty.strip_prefix('&').map(|inner| (inner, false))) {
        if inner == "dyn Node" {
            // A node is copied out of the table (plan A, A16).
            return Param::Value { ty: "Box<dyn Node>".to_string(), pass: format!("&*{local}") };
        }
        if let Some(kind) = facade(inner) {
            return Param::Facade { kind, mutable };
        }
        return Param::Value { ty: owned_form(inner, site), pass: format!("&{local}") };
    }
    if let Some(inner) = ty.strip_prefix("impl Into<").and_then(|rest| rest.strip_suffix('>')) {
        return Param::Value { ty: owned_form(inner, site), pass: local };
    }
    if let Some(item) = ty.strip_prefix("impl IntoIterator<Item = ").and_then(|rest| rest.strip_suffix('>')) {
        return Param::Value { ty: owned_form(&format!("Vec<{item}>"), site), pass: local };
    }
    Param::Value { ty: owned_form(ty, site), pass: local }
}

/// The type `FromArg` reads for the (unborrowed) type `ty`: the row's
/// `from_arg`, `String` for `str`, `Vec<T>` for a slice `[T]` with no row.
fn owned_form(ty: &str, site: &Site<'_>) -> String {
    if let Some(row) = mapping::row(ty) {
        return match row.from_arg {
            Some("str") => "String".to_string(),
            Some(from_arg) => from_arg.to_string(),
            None => panic!(
                "the row `{ty}` of `tests/api_surface/mapping.rs` must name a `FromArg` type, as the parameter `{}` of `{}` (`{}`) reads it",
                site.param, site.function, site.file
            ),
        };
    }
    if let Some(item) = ty.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
        return format!("Vec<{item}>");
    }
    panic!(
        "the type `{ty}` of the parameter `{}` of `{}` (`{}`) must have a row in `tests/api_surface/mapping.rs`, or the method must be \
         listed in `NOT_DISPATCHED` there",
        site.param, site.function, site.file
    )
}

/// `ty` with the call's lifetime on the types of `src/api/` that take a
/// lifetime parameter, as the reader found them (`Box<dyn Node>` is
/// `Box<dyn Node<'c> + 'c>`).
fn with_lifetime(ty: &str, inventory: &Inventory) -> String {
    let mut out = String::new();
    let mut rest = ty;
    while !rest.is_empty() {
        let start = rest.find(|c: char| c.is_alphanumeric() || c == '_').unwrap_or(rest.len());
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let word = &rest[..end];
        out.push_str(word);
        if inventory.definition(word).is_some_and(|(_, item)| item.lifetime) {
            if out[..out.len() - word.len()].ends_with("dyn ") {
                out.push_str("<'c> + 'c");
            } else {
                out.push_str("<'c>");
            }
        }
        rest = &rest[end..];
    }
    out
}

/// The identifiers of a type text.
fn identifiers(ty: &str) -> impl Iterator<Item = &str> {
    ty.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|word| !word.is_empty())
}

/// How an arm's call is made.
enum Target<'a> {
    /// A method on the receiver of the kind `kind`, through its entry.
    Method { kind: &'static KindRow, receiver: &'a str },
    /// A free function or a constructor, by its path.
    Path(String),
}

/// The emitter's state: the names the generated file must import.
struct Emitter<'a> {
    /// The inventory.
    inventory: &'a Inventory,
    /// The identifiers of the types the arms read.
    names: BTreeSet<String>,
    /// Whether an arm calls an override that needs the sealed token.
    token: bool,
}

impl Emitter<'_> {
    /// The lines of the arm for `function`, a method of `owner` (or a free
    /// function when `owner` is `None`) defined in `file`, at the
    /// indentation of an arm's body (12 spaces).
    fn arm(&mut self, owner: Option<&str>, function: &Function, file: &str, target: &Target<'_>) -> Vec<String> {
        let display = owner.map_or_else(|| function.name.clone(), |owner| format!("{owner}::{}", function.name));
        let params: Vec<Param> = function
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| param(&p.ty, i, &Site { param: &p.name, function: &display, file }))
            .collect();
        let mut lines = vec![format!("args.arity({})?;", params.len())];
        let mut entries: Vec<(String, String, &'static KindRow)> = Vec::new();
        if let Target::Method { kind, .. } = target {
            entries.push(("this".to_string(), "handle".to_string(), kind));
        }
        let mut passed = Vec::new();
        for (i, plan) in params.iter().enumerate() {
            match plan {
                Param::Value { ty, pass } => {
                    for name in identifiers(ty) {
                        self.names.insert(name.to_string());
                    }
                    lines.push(format!("let a{i} = <{} as FromArg>::from_arg(table, args, {i})?;", with_lifetime(ty, self.inventory)));
                    passed.push(pass.clone());
                }
                Param::Facade { kind, mutable } => {
                    entries.push((format!("a{i}"), format!("args.handle({i})?"), kind));
                    passed.push(if *mutable { format!("&mut **a{i}") } else { format!("&**a{i}") });
                }
            }
        }
        let arguments = passed.join(", ");
        let call = match target {
            Target::Method { kind, receiver } => {
                let overridden = mapping::CALL_OVERRIDES.iter().find(|entry| entry.kind == kind.kind && entry.method == function.name);
                let consumes = matches!(*receiver, "self" | "mut self") && entries.len() > 1;
                assert!(!consumes, "the method `{display}` (`{file}`) must take its receiver by reference, since it also takes a borrowed façade");
                match overridden {
                    Some(entry) => {
                        self.token |= entry.call.contains("Token::");
                        format!("this.{}", entry.call)
                    }
                    None => format!("this.{}({arguments})", function.name),
                }
            }
            Target::Path(path) => format!("{path}({arguments})"),
        };
        let (fallible, inner) = match function.output.strip_prefix("Result<").and_then(|rest| rest.strip_suffix(", ExtError>")) {
            Some(inner) => (true, inner),
            None => (false, function.output.as_str()),
        };
        let unit = inner == "()";
        let question = if fallible { "?" } else { "" };
        let bind = if unit { String::new() } else { "let answer = ".to_string() };
        match entries.as_slice() {
            [] => lines.push(format!("{bind}{call}{question};")),
            [(binding, handle, kind)] => {
                let variant = kind.entry.expect("a kind with an entry");
                let taken = format!("entry!(table, {handle}, {variant}, {})", camel(kind.kind));
                let line = if binding == "this" {
                    match kind.receiver {
                        Receiver::LentRef => format!("*{taken}"),
                        Receiver::Boxed => format!("&**{taken}"),
                        Receiver::Cloned => format!("{taken}.clone()"),
                        _ => taken,
                    }
                } else {
                    taken
                };
                lines.push(format!("let {binding} = {line};"));
                lines.push(format!("{bind}{call}{question};"));
            }
            many => {
                let handles: Vec<&str> = many.iter().map(|(_, handle, _)| handle.as_str()).collect();
                lines.push(format!("{bind}match table.entries_mut(method, [{}])? {{", handles.join(", ")));
                let full: Vec<String> =
                    many.iter().map(|(binding, _, kind)| format!("Entry::{}({binding})", kind.entry.expect("a kind with an entry"))).collect();
                lines.push(format!("    [{}] => {call},", full.join(", ")));
                for position in (0..many.len()).rev() {
                    let pattern: Vec<String> = many
                        .iter()
                        .enumerate()
                        .map(|(k, (_, _, kind))| match k.cmp(&position) {
                            std::cmp::Ordering::Less => format!("Entry::{}(_)", kind.entry.expect("a kind with an entry")),
                            std::cmp::Ordering::Equal => "other".to_string(),
                            std::cmp::Ordering::Greater => "_".to_string(),
                        })
                        .collect();
                    lines.push(format!(
                        "    [{}] => return Err(mismatch(HandleKind::{}, other.kind())),",
                        pattern.join(", "),
                        camel(many[position].2.kind)
                    ));
                }
                lines.push(format!("}}{question};"));
            }
        }
        lines.push(if unit { "Ok(Value::Null)".to_string() } else { "answer.into_reply(table)".to_string() });
        lines
    }

    /// The `use` line of the type named `name`, if it is an item of
    /// `src/api/` (the standard types need none).
    fn use_line(&self, name: &str) -> Option<String> {
        match name {
            "Arc" => return Some("use std::sync::Arc;".to_string()),
            "Value" => return None,
            _ => {}
        }
        let (module, _) = self.inventory.definition(name).or_else(|| self.inventory.find(name))?;
        Some(format!("use crate::api::{}::{name};", module.path.replace('/', "::")))
    }
}

/// The methods of `item` the kind `kind` dispatches, with their receivers:
/// every method with a receiver, less the exclusions, the extras and the
/// hand-written arms.
fn dispatched<'i>(item: &'i Item, kind: &str) -> Vec<&'i Function> {
    let methods = match &item.kind {
        ItemKind::Trait { methods, .. } | ItemKind::Type { methods, .. } => methods,
        _ => return Vec::new(),
    };
    methods
        .iter()
        .filter(|method| method.receiver.is_some())
        .filter(|method| !mapping::is_excluded(&item.name, &method.name))
        .filter(|method| !mapping::is_extra(kind, &method.name) && !mapping::is_hand_arm(kind, &method.name))
        .collect()
}

/// Appends an arm keyed `key` with the body `lines` to `out`.
fn push_arm(out: &mut String, key: &str, lines: &[String]) {
    let _ = writeln!(out, "        {key} => {{");
    for line in lines {
        let _ = writeln!(out, "            {line}");
    }
    out.push_str("        }\n");
}

/// The text of `src/adapter/generated.rs`.
pub fn emit(inventory: &Inventory) -> String {
    let mut emitter = Emitter { inventory, names: BTreeSet::new(), token: false };
    let paths = module_paths(inventory);

    // The kind functions.
    let mut kinds = String::new();
    let mut dispatch_arms: Vec<String> = Vec::new();
    let mut no_methods: Vec<String> = Vec::new();
    for row in mapping::KINDS {
        let variant = camel(row.kind);
        let item = row
            .rust
            .filter(|_| matches!(row.receiver, Receiver::LentRef | Receiver::LentMut | Receiver::Boxed | Receiver::Cloned | Receiver::Recorded))
            .and_then(|rust| inventory.definition(rust));
        let methods = item.map(|(_, item)| dispatched(item, row.kind)).unwrap_or_default();
        if methods.is_empty() {
            if has_extras(row.kind) {
                dispatch_arms.push(format!("        HandleKind::{variant} => extra::{}(table, handle, method, args),", row.kind));
            } else {
                no_methods.push(format!("HandleKind::{variant}"));
            }
            continue;
        }
        let (module, item) = item.expect("a kind with methods has an item");
        dispatch_arms.push(format!("        HandleKind::{variant} => {}(table, handle, method, args),", row.kind));
        let _ = writeln!(kinds, "/// The methods of the kind `{}` (`{}`).", row.kind, item.name);
        let _ = writeln!(
            kinds,
            "fn {}<'c>(table: &mut CallTable<'c>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {{",
            row.kind
        );
        kinds.push_str("    match method {\n");
        for method in methods {
            let receiver = method.receiver.as_deref().unwrap_or("&self");
            let lines = emitter.arm(Some(&item.name), method, &module.file, &Target::Method { kind: row, receiver });
            push_arm(&mut kinds, &format!("\"{}\"", method.name), &lines);
        }
        if has_extras(row.kind) {
            let _ = writeln!(kinds, "        _ => extra::{}(table, handle, method, args),", row.kind);
        } else {
            let _ = writeln!(kinds, "        _ => Err(no_method(HandleKind::{variant}, method)),");
        }
        kinds.push_str("    }\n}\n\n");
    }

    // The module function.
    let mut module_fn = String::new();
    let mut constants: Vec<(String, String)> = Vec::new();
    for path in &paths {
        let module = inventory.module(path).expect("a listed module");
        let rust_path = format!("crate::api::{}", path.replace('/', "::"));
        for item in &module.items {
            match (&item.class, &item.kind) {
                (ItemClass::Function, ItemKind::Function(function))
                    if !mapping::is_extra(path, &item.name)
                        && !mapping::is_excluded(path, &item.name)
                        && !mapping::is_hand_arm(path, &item.name) =>
                {
                    let lines = emitter.arm(None, function, &module.file, &Target::Path(format!("{rust_path}::{}", item.name)));
                    push_arm(&mut module_fn, &format!("(\"{path}\", \"{}\")", item.name), &lines);
                }
                (ItemClass::OpaqueType, ItemKind::Type { methods, .. }) => {
                    for method in methods.iter().filter(|method| {
                        method.is_constructor()
                            && !mapping::is_excluded(&item.name, &method.name)
                            && !mapping::is_hand_arm(path, &format!("{}::{}", item.name, method.name))
                    }) {
                        let owner = format!("{}::{}", item.name, method.name);
                        let lines = emitter.arm(Some(&item.name), method, &module.file, &Target::Path(format!("{rust_path}::{owner}")));
                        push_arm(&mut module_fn, &format!("(\"{path}\", \"{owner}\")"), &lines);
                    }
                }
                (ItemClass::Constant, _) => constants.push((path.clone(), item.name.clone())),
                _ => {}
            }
        }
        if constants.iter().any(|(module, _)| module == path) {
            push_arm(&mut module_fn, &format!("(\"{path}\", \"names\")"), &["args.arity(0)?;".to_string(), format!("Ok(names(\"{path}\"))")]);
        }
    }

    // The file.
    let mut out = String::new();
    out.push_str(
        "//! Generated by `tests/api_surface.rs` from the sources under `src/api/`; do not\n\
         //! edit. `UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface` rewrites it.\n\
         //!\n\
         //! The dispatcher's arms: the module list, the table of constants, the kind\n\
         //! `match`, one function per handle kind with one arm per method, and the module\n\
         //! function. The hand-written arms are in `dispatch.rs`'s module `extra`.\n\n",
    );
    out.push_str("use serde_json::{json, Value};\n\n");
    let mut uses: BTreeSet<String> = emitter.names.iter().filter_map(|name| emitter.use_line(name)).collect();
    uses.insert("use crate::adapter::dispatch::{entry, extra, no_method};".to_string());
    uses.insert("use crate::adapter::handles::{CallTable, Entry, Handle, HandleKind};".to_string());
    uses.insert("use crate::adapter::values::{mismatch, Args, FromArg, IntoReply};".to_string());
    uses.insert("use crate::runtime::ExtError;".to_string());
    if emitter.token {
        uses.insert("use crate::sealed::Token;".to_string());
    }
    let (std_uses, crate_uses): (Vec<&String>, Vec<&String>) = uses.iter().partition(|line| line.starts_with("use std::"));
    for line in &std_uses {
        let _ = writeln!(out, "{line}");
    }
    if !std_uses.is_empty() {
        out.push('\n');
    }
    for line in &crate_uses {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');

    out.push_str(
        "/// The modules of the extension API whose free functions, constructors or\n\
         /// constants are dispatched on a module handle, by their path under `api`, in\n\
         /// sorted order; a module's handle has its position here as its index.\n",
    );
    out.push_str("pub const MODULE_PATHS: &[&str] = &[\n");
    for path in &paths {
        let _ = writeln!(out, "    \"{path}\",");
    }
    out.push_str("];\n\n");

    out.push_str(
        "/// Every constant of the extension API, as `(module, name, value)`: the\n\
         /// compiled table the generator reads the constants' values from.\n",
    );
    out.push_str("pub fn constants() -> Vec<(&'static str, &'static str, Value)> {\n    vec![\n");
    for (path, name) in &constants {
        let _ = writeln!(out, "        (\"{path}\", \"{name}\", json!(crate::api::{}::{name})),", path.replace('/', "::"));
    }
    out.push_str("    ]\n}\n\n");

    out.push_str(
        "/// The constants of the module `path`, as its `names` function answers them: a\n\
         /// map from each constant's name to its value.\n",
    );
    out.push_str(
        "fn names(path: &str) -> Value {\n    \
         Value::Object(constants().into_iter().filter(|(module, _, _)| *module == path).map(|(_, name, value)| (name.to_string(), value)).collect())\n\
         }\n\n",
    );

    out.push_str(
        "/// Calls `method` on the object `handle` stands for, by the handle's kind; a\n\
         /// kind with no methods answers that it has none.\n",
    );
    out.push_str(
        "pub(crate) fn dispatch_kind<'c>(table: &mut CallTable<'c>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {\n",
    );
    out.push_str("    match handle.kind {\n");
    for arm in &dispatch_arms {
        let _ = writeln!(out, "{arm}");
    }
    let _ = writeln!(out, "        {} => Err(no_method(handle.kind, method)),", no_methods.join("\n        | "));
    out.push_str("    }\n}\n\n");

    out.push_str(&kinds);

    out.push_str("/// The free functions, the constructors and the `names` of the module `path`.\n");
    out.push_str(
        "pub(crate) fn module<'c>(table: &mut CallTable<'c>, path: &str, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {\n",
    );
    out.push_str("    match (path, method) {\n");
    out.push_str(&module_fn);
    out.push_str("        _ => extra::module(table, path, method, args),\n");
    out.push_str("    }\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::{param, with_lifetime, Site};

    #[test]
    #[should_panic(
        expected = "the type `Unmapped` of the parameter `region` of `Lowerer::lower_region` (`src/api/core/lowering.rs`) must have a row \
                    in `tests/api_surface/mapping.rs`, or the method must be listed in `NOT_DISPATCHED` there"
    )]
    fn a_parameter_type_without_a_row_is_named_with_its_parameter_and_file() {
        let site = Site { param: "region", function: "Lowerer::lower_region", file: "src/api/core/lowering.rs" };
        let _ = param("Unmapped", 0, &site);
    }

    #[test]
    fn the_call_lifetime_goes_on_the_types_the_reader_found_with_a_lifetime() {
        let inventory = crate::read_inventory();
        assert_eq!(with_lifetime("Box<dyn Node>", &inventory), "Box<dyn Node<'c> + 'c>");
        assert_eq!(with_lifetime("Vec<LineFrame>", &inventory), "Vec<LineFrame<'c>>");
        assert_eq!(with_lifetime("Option<TableValue>", &inventory), "Option<TableValue<'c>>");
        assert_eq!(with_lifetime("Vec<String>", &inventory), "Vec<String>");
        assert_eq!(with_lifetime("ArgDef", &inventory), "ArgDef");
    }
}
