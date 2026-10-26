//! The generator of the extension API's dispatcher, prelude and declarations
//! (step E2b, `dev-docs/ext/e2b/plan.md`): it reads the sources under
//! `src/api/` with `syn` ([`inventory`]), checks them against the mapping
//! table ([`mapping`]), and writes its outputs as golden files
//! (`UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface` rewrites
//! them). It reads the surface, writes the inventory golden
//! `tests/goldens/api_surface/inventory.json`, checks the table, and writes
//! the generated dispatcher `src/adapter/generated.rs` ([`rust_out`]) and the
//! guest client's surface `src/guest/generated.rs` ([`guest_out`]); the
//! emitters of the JavaScript and TypeScript outputs follow.
//!
//! The test target is this file alone (cargo compiles `tests/*.rs` as
//! targets, not the files of the folder beside it), so its modules are
//! declared with their paths under `tests/api_surface/`.

#[path = "api_surface/docs_out.rs"]
mod docs_out;
#[path = "api_surface/guest_out.rs"]
mod guest_out;
#[path = "api_surface/inventory.rs"]
mod inventory;
#[path = "api_surface/js_out.rs"]
mod js_out;
#[path = "api_surface/mapping.rs"]
mod mapping;
#[path = "api_surface/rust_out.rs"]
mod rust_out;
#[path = "api_surface/ts_out.rs"]
mod ts_out;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use flm_core::dump::golden::assert_text_golden;
use inventory::{Function, Inventory, Item, ItemClass, ItemKind, Module};

/// The inventory golden, from the crate's root.
const INVENTORY_GOLDEN: &str = "tests/goldens/api_surface/inventory.json";

/// The mapping table's file, as the messages name it.
const MAPPING_FILE: &str = "tests/api_surface/mapping.rs";

/// The crate's root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The inventory of the crate's sources.
fn read_inventory() -> Inventory {
    Inventory::read(&crate_root())
}

/// The outputs written before the values of the constants are needed, as
/// `(path from the crate's root, text)`: the inventory, the generated
/// dispatcher, which holds the table of constants, and the guest client's
/// surface, which re-exports the constants by path.
fn rust_outputs(inventory: &Inventory) -> Vec<(String, String)> {
    vec![
        (INVENTORY_GOLDEN.to_string(), inventory.to_json()),
        (rust_out::GENERATED_RS.to_string(), rust_out::emit(inventory)),
        (guest_out::GENERATED_RS.to_string(), guest_out::emit(inventory)),
    ]
}

/// The outputs written with the values of the constants (`(module, name,
/// value)`, from the compiled table): the prelude's stubs, `flm:_api` and
/// the module list, `flm.d.ts`, and the module page's table.
fn later_outputs(inventory: &Inventory, constants: &[(String, String, Value)]) -> Vec<(String, String)> {
    let prelude = js_out::emit(&crate_root(), inventory, constants);
    let mut out = prelude.files;
    out.push((ts_out::FLM_D_TS.to_string(), ts_out::emit(inventory, constants)));
    out.push((docs_out::API_MODULES_MD.to_string(), docs_out::emit(inventory, &prelude.exports)));
    out
}

/// Every output of the generator, in a fixed order.
///
/// # Panics
///
/// When the compiled table of constants lacks a constant of the inventory.
fn outputs(inventory: &Inventory) -> Vec<(String, String)> {
    let mut out = rust_outputs(inventory);
    let constants = constant_values(inventory, &flm_ext::adapter::constants(), false).unwrap_or_else(|message| panic!("{message}"));
    out.extend(later_outputs(inventory, &constants));
    out
}

#[test]
fn the_generated_files_are_current() {
    let inventory = read_inventory();
    let updating = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut rewritten = false;
    let check = |path: &str, text: &str| {
        let file = crate_root().join(path);
        if updating && let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("the folder of `{path}` must be writable: {error}"));
        }
        assert_text_golden(file, text);
    };
    for (path, text) in rust_outputs(&inventory) {
        if updating && path == rust_out::GENERATED_RS {
            rewritten = std::fs::read_to_string(crate_root().join(&path)).ok().as_deref() != Some(text.as_str());
        }
        check(&path, &text);
    }
    // The values of the constants come from the compiled table (the plan's
    // section 3.4); a constant the compiled crate lacks needs a build and a
    // second run.
    let constants = constant_values(&inventory, &flm_ext::adapter::constants(), rewritten).unwrap_or_else(|message| panic!("{message}"));
    let later = later_outputs(&inventory, &constants);
    for (path, text) in &later {
        check(path, text);
    }
    // No stub is left for a module the inventory no longer has.
    let written: BTreeSet<&str> = later.iter().map(|(path, _)| path.as_str()).collect();
    for stub in files_under(&crate_root().join(js_out::API_DIR)) {
        let path = stub.strip_prefix(crate_root()).unwrap_or(&stub).to_string_lossy().replace('\\', "/");
        assert!(written.contains(path.as_str()), "the file `{path}` must be a module of the extension API; the inventory has none at that path");
    }
}

/// Every file under `dir`, recursively, in sorted order (none when `dir` is
/// absent).
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

#[test]
fn two_runs_write_the_same_bytes() {
    let first = outputs(&read_inventory());
    let second = outputs(&read_inventory());
    assert_eq!(first.len(), second.len(), "two runs of the generator must write the same files");
    for ((path, first), (_, second)) in first.iter().zip(&second) {
        if first != second {
            let line = first.lines().zip(second.lines()).position(|(a, b)| a != b).unwrap_or(first.lines().count().min(second.lines().count()));
            panic!("two runs of the generator must write the same bytes for `{path}`; the first difference is at line {}", line + 1);
        }
    }
}

#[test]
fn the_fragments_are_spliced_once() {
    let inventory = read_inventory();
    let outputs = outputs(&inventory);
    for hand in mapping::HAND_WRITTEN {
        let stub_path = format!("{}/{}.js", js_out::API_DIR, hand.module);
        let stub = &outputs.iter().find(|(path, _)| *path == stub_path).unwrap_or_else(|| panic!("the generator must write the stub `{stub_path}`")).1;
        let count = match hand.item.split_once('.') {
            None => ["class", "function", "const"].iter().map(|word| stub.matches(&format!("export {word} {} ", hand.item)).count()
                + stub.matches(&format!("export {word} {}(", hand.item)).count()).sum::<usize>(),
            Some((class, method)) => {
                let start = stub.find(&format!("export class {class} ")).unwrap_or_else(|| panic!("the stub `{stub_path}` must hold the class `{class}`"));
                let body = &stub[start..start + stub[start..].find("\n}\n").unwrap_or(stub.len() - start)];
                body.lines().filter(|line| line.starts_with(&format!("  {method}("))).count()
            }
        };
        assert_eq!(count, 1, "the hand-written `{}` must appear exactly once in `{stub_path}`", hand.item);
    }
    for stub in outputs.iter().filter(|(path, _)| path.ends_with(".js")) {
        assert!(!stub.1.contains("// @hand "), "the stub `{}` must hold no marker", stub.0);
    }
}

#[test]
fn the_surface_has_the_pinned_counts() {
    let inventory = read_inventory();
    let count = |class: ItemClass| inventory.of_class(class).count();
    assert_eq!(count(ItemClass::SealedTrait), 30, "the sealed façade traits");
    assert_eq!(count(ItemClass::ExtensionTrait), 10, "the extension-side traits");
    assert_eq!(count(ItemClass::OpaqueType), 9, "the opaque value types");
    assert_eq!(count(ItemClass::Constant), 113, "the constants");
    let dispatched: Vec<&str> = inventory.modules.iter().filter(|module| has_module_arms(module)).map(|module| module.path.as_str()).collect();
    assert_eq!(
        dispatched,
        [
            "core/artifact",
            "core/lowering",
            "lang/mint",
            "lang/rewrites/blocks",
            "lang/rewrites/userdefs",
            "lang/structural",
            "lang/text",
            "stdcore/backends/html",
            "stdcore/backends/latex",
            "stdcore/backends/latex/packages",
            "stdcore/backends/latex/preamble",
            "stdcore/backends/lowering",
            "stdcore/backends/pres",
            "stdcore/features/citations",
            "stdcore/features/compose",
            "stdcore/features/math",
            "stdcore/features/numbering",
            "stdcore/features/refs",
            "stdcore/features/userdefs",
            "techy/extract",
        ],
        "the modules with functions, constructors or constants"
    );
    // The guest client's surface: its structs, the methods of its impl
    // blocks and its module functions (including workflow lowering preferences).
    assert_eq!(guest_out::counts(&guest_out::emit(&inventory)), (34, 156, 35), "the structs, methods and functions of `src/guest/generated.rs`");
}

/// Whether `module` is reached through a module handle: it has a free
/// function, a constant, or an opaque type's constructor that is dispatched.
fn has_module_arms(module: &Module) -> bool {
    module.items.iter().any(|item| match (&item.class, &item.kind) {
        (ItemClass::Function | ItemClass::Constant, _) => true,
        (ItemClass::OpaqueType, ItemKind::Type { methods, .. }) => {
            methods.iter().any(|method| method.is_constructor() && !mapping::is_excluded(&item.name, &method.name))
        }
        _ => false,
    })
}

#[test]
fn the_mapping_table_matches_the_inventory() {
    let inventory = read_inventory();
    let mut failures = Vec::new();
    check_crossing_types(&inventory, &mut failures);
    check_kinds(&inventory, &mut failures);
    check_lists(&inventory, &mut failures);
    check_unit_enums(&inventory, &mut failures);
    check_written_out_values(&mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The member of the frozen object of `rust` named `name`, as the stub
/// writes it.
fn static_member_js(rust: &str, name: &str) -> Option<&'static str> {
    let object = mapping::STATIC_OBJECTS.iter().find(|object| object.rust == rust)?;
    object.members.iter().find(|member| member.name == name).map(|member| member.js)
}

/// A value the stub writes out a second time agrees with its source:
/// `BlockRole.DEFAULT` holds the value of `InlineSpacing.CONTENT` (the plan's
/// ruling S13).
fn check_written_out_values(failures: &mut Vec<String>) {
    let (Some(default), Some(content)) = (static_member_js("BlockRole", "DEFAULT"), static_member_js("InlineSpacing", "CONTENT")) else {
        failures.push(format!("`{MAPPING_FILE}` must hold the members `BlockRole.DEFAULT` and `InlineSpacing.CONTENT`"));
        return;
    };
    let inline = default.strip_prefix("DEFAULT: Object.freeze({ inline: ").and_then(|rest| rest.strip_suffix(" })"));
    let spacing = content.strip_prefix("CONTENT: ");
    if inline.is_none() || inline != spacing {
        failures.push(format!(
            "the value `BlockRole.DEFAULT` gives for `inline` must be the text of `InlineSpacing.CONTENT` in `{MAPPING_FILE}`; \
             `BlockRole.DEFAULT` is `{default}` and `InlineSpacing.CONTENT` is `{content}`"
        ));
    }
}

/// The rows the crossing types use, and the failures found.
struct Crossing<'a> {
    /// The rows used so far.
    used: BTreeSet<String>,
    /// The failures, appended to the test's.
    failures: &'a mut Vec<String>,
}

impl Crossing<'_> {
    /// Checks the type `ty` at `position` of an item of `file`.
    fn check(&mut self, ty: &str, position: &str, file: &str) {
        for leaf in mapping::leaves(ty).types {
            if mapping::row(&leaf).is_some() {
                self.used.insert(leaf);
            } else {
                self.failures.push(format!(
                    "the type `{leaf}` of {position} (`{file}`) must have a row in `{MAPPING_FILE}`, or the method must be listed in \
                     `NOT_DISPATCHED` there"
                ));
            }
        }
    }

    /// Checks the parameters and the result of `function`, a method of
    /// `owner` (or a free function when `owner` is its own name).
    fn check_signature(&mut self, owner: &str, function: &Function, file: &str) {
        let name = if owner == function.name { owner.to_string() } else { format!("{owner}::{}", function.name) };
        for param in &function.params {
            self.check(&param.ty, &format!("the parameter `{}` of `{name}`", param.name), file);
        }
        self.check(&function.output, &format!("the result of `{name}`"), file);
    }
}

/// Every crossing type has a row, and every row is used.
fn check_crossing_types(inventory: &Inventory, failures: &mut Vec<String>) {
    let mut crossing = Crossing { used: BTreeSet::new(), failures };
    for (module, item) in inventory.items() {
        let file = module.file.as_str();
        let owner = item.name.as_str();
        match (&item.class, &item.kind) {
            (ItemClass::SealedTrait | ItemClass::ExtensionTrait | ItemClass::OpaqueType, ItemKind::Trait { methods, .. } | ItemKind::Type { methods, .. }) => {
                let kind = mapping::kind_of(owner).map(|row| row.kind);
                for method in methods {
                    // An extension-side trait's methods are the interface a
                    // script's object implements: all of them cross.
                    let extension = item.class == ItemClass::ExtensionTrait;
                    let extra = kind.is_some_and(|kind| mapping::is_extra(kind, &method.name));
                    if !extension && (mapping::is_excluded(owner, &method.name) || extra) {
                        continue;
                    }
                    crossing.check_signature(owner, method, file);
                }
            }
            (ItemClass::DataClass, ItemKind::Type { fields, methods, .. }) => {
                for method in methods {
                    crossing.check_signature(owner, method, file);
                }
                for field in fields {
                    crossing.check(&field.ty, &format!("the field `{}` of `{owner}`", field.name), file);
                }
                crossing.used.insert(owner.to_string());
            }
            (ItemClass::DataType | ItemClass::CoreReexport, _) => {
                crossing.used.insert(owner.to_string());
            }
            (ItemClass::Function, ItemKind::Function(function))
                if !mapping::is_extra(&module.path, owner) && !mapping::is_excluded(&module.path, owner) =>
            {
                crossing.check_signature(owner, function, file);
            }
            _ => {}
        }
    }
    for row in mapping::TYPES {
        if !crossing.used.contains(row.rust) {
            crossing.failures.push(format!("the row `{}` of `{MAPPING_FILE}` must name a type a surface item uses", row.rust));
        }
    }
}

/// The kind rows against `HandleKind` and `Entry` (read from
/// `src/adapter/handles.rs` with `syn`) and against the inventory.
fn check_kinds(inventory: &Inventory, failures: &mut Vec<String>) {
    let handles = Path::new("src/adapter/handles.rs");
    let source = std::fs::read_to_string(crate_root().join(handles)).expect("`src/adapter/handles.rs` is readable");
    let parsed = syn::parse_file(&source).unwrap_or_else(|error| panic!("the file `src/adapter/handles.rs` must parse as Rust: {error}"));
    let variants = |name: &str| -> Vec<String> {
        parsed
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Enum(item) if item.ident == name => Some(item.variants.iter().map(|variant| variant.ident.to_string()).collect()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the file `src/adapter/handles.rs` must define the enum `{name}`"))
    };
    let handle_kinds: Vec<String> = variants("HandleKind").iter().map(|name| snake_case(name)).collect();
    let entries = variants("Entry");
    for row in mapping::KINDS {
        if !handle_kinds.iter().any(|kind| kind == row.kind) {
            failures.push(format!("the kind `{}` of `{MAPPING_FILE}` must be a variant of `HandleKind` in `src/adapter/handles.rs`", row.kind));
        }
        if let Some(entry) = row.entry
            && !entries.iter().any(|name| name == entry)
        {
            failures.push(format!("the entry `{entry}` of the kind `{}` of `{MAPPING_FILE}` must be a variant of `Entry` in `src/adapter/handles.rs`", row.kind));
        }
        if let Some(rust) = row.rust
            && !inventory.definition(rust).is_some_and(|(_, item)| matches!(item.class, ItemClass::SealedTrait | ItemClass::ExtensionTrait | ItemClass::OpaqueType))
        {
            failures.push(format!("the kind `{}` of `{MAPPING_FILE}` must name a trait or an opaque type under `src/api/`, not `{rust}`", row.kind));
        }
    }
    let rows: Vec<&str> = mapping::KINDS.iter().map(|row| row.kind).collect();
    if rows != handle_kinds {
        failures.push(format!("the kind rows of `{MAPPING_FILE}` must be the variants of `HandleKind` in its order: {}", handle_kinds.join(", ")));
    }
    for entry in &entries {
        if !mapping::KINDS.iter().any(|row| row.entry == Some(entry.as_str())) {
            failures.push(format!("the variant `{entry}` of `Entry` in `src/adapter/handles.rs` must be the entry of a kind of `{MAPPING_FILE}`"));
        }
    }
    for (module, item) in inventory.items() {
        let has_kind = mapping::kind_of(&item.name).is_some();
        let needs_kind = matches!(item.class, ItemClass::SealedTrait | ItemClass::ExtensionTrait)
            || item.class == ItemClass::OpaqueType && !mapping::KINDLESS.contains(&item.name.as_str());
        if needs_kind && !has_kind {
            failures.push(format!("the item `{}` of `{}` must have a kind row in `{MAPPING_FILE}`", item.name, module.file));
        }
    }
    for name in mapping::OPAQUE_TYPES {
        if !inventory.definition(name).is_some_and(|(_, item)| item.class == ItemClass::OpaqueType) {
            failures.push(format!("the opaque type `{name}` of `{MAPPING_FILE}` must be a struct under `src/api/`"));
        }
    }
}

/// `HandleKind`'s serde name of the variant `name` (`FeatureBuilder` is
/// `feature_builder`).
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (position, c) in name.chars().enumerate() {
        if c.is_uppercase() && position > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Every entry of `NOT_DISPATCHED`, `CALL_OVERRIDES`, `OPTIONAL_IN_SCRIPTS`,
/// `EXTRA_METHODS`, `HAND_ARMS`, `HAND_WRITTEN` and `STATIC_OBJECTS` names an
/// item that exists; every entry of `GUEST_OWN` is a type row, every entry
/// of `GUEST_HAND` a row of `HAND_WRITTEN`, every entry of `GUEST_ADDED` a
/// module that holds no item of its name, and every entry of
/// `EXTRA_METHODS` has its Rust signature.
fn check_lists(inventory: &Inventory, failures: &mut Vec<String>) {
    let method_of = |owner: &str, method: &str| -> bool {
        inventory.items().any(|(module, item)| {
            if item.name == owner {
                methods(item).iter().any(|function| function.name == method)
            } else {
                // A free function, by its module.
                module.path == owner && item.name == method && item.class == ItemClass::Function
            }
        })
    };
    for entry in mapping::NOT_DISPATCHED {
        if !method_of(entry.owner, entry.method) {
            failures.push(format!(
                "the entry `{}::{}` of `NOT_DISPATCHED` must name a method of a trait or a type under `src/api/`",
                entry.owner, entry.method
            ));
        }
    }
    for entry in mapping::CALL_OVERRIDES {
        let owner = mapping::KINDS.iter().find(|row| row.kind == entry.kind).and_then(|row| row.rust);
        if !owner.is_some_and(|owner| method_of(owner, entry.method)) {
            failures.push(format!(
                "the entry `{}::{}` of `CALL_OVERRIDES` must name a method of a façade trait under `src/api/`",
                entry.kind, entry.method
            ));
        }
    }
    for entry in mapping::HAND_ARMS {
        let owner = mapping::KINDS.iter().find(|row| row.kind == entry.kind).and_then(|row| row.rust);
        // A module's free function or constructor (`Type::new`).
        let in_module = inventory.module(entry.kind).is_some_and(|module| match entry.method.split_once("::") {
            Some((ty, method)) => module.items.iter().any(|item| item.name == ty && methods(item).iter().any(|function| function.name == method)),
            None => module.items.iter().any(|item| item.name == entry.method && item.class == ItemClass::Function),
        });
        if !in_module && !owner.is_some_and(|owner| method_of(owner, entry.method)) {
            failures.push(format!(
                "the entry `{}::{}` of `HAND_ARMS` must name a method of a façade trait under `src/api/`",
                entry.kind, entry.method
            ));
        }
    }
    for entry in mapping::OPTIONAL_IN_SCRIPTS {
        let required = inventory.items().any(|(_, item)| {
            item.name == entry.owner
                && item.class == ItemClass::ExtensionTrait
                && methods(item).iter().any(|function| function.name == entry.method && !function.default_body)
        });
        if !required {
            failures.push(format!(
                "the entry `{}::{}` of `OPTIONAL_IN_SCRIPTS` must name a method without a default body of an extension-side trait under `src/api/`",
                entry.owner, entry.method
            ));
        }
    }
    for entry in mapping::EXTRA_METHODS {
        let is_kind = mapping::KINDS.iter().any(|row| row.kind == entry.owner);
        if !is_kind && inventory.module(entry.owner).is_none() {
            failures.push(format!(
                "the entry `{}::{}` of `EXTRA_METHODS` must name a handle kind of `{MAPPING_FILE}` or a module under `src/api/`",
                entry.owner, entry.name
            ));
        }
        if !entry.rust.starts_with(&format!("fn {}(", entry.name)) {
            failures.push(format!(
                "the entry `{}::{}` of `EXTRA_METHODS` must give the guest's Rust signature `fn {}(…)`, not `{}`",
                entry.owner, entry.name, entry.name, entry.rust
            ));
        }
    }
    for own in mapping::GUEST_OWN {
        if mapping::row(own).is_none() {
            failures.push(format!("the entry `{own}` of `GUEST_OWN` must name a type row of `{MAPPING_FILE}`"));
        }
    }
    for hand in mapping::GUEST_HAND {
        if !mapping::HAND_WRITTEN.iter().any(|entry| entry.item == *hand) {
            failures.push(format!("the entry `{hand}` of `GUEST_HAND` must name an item of `HAND_WRITTEN`"));
        }
    }
    for (path, name) in mapping::GUEST_ADDED {
        if !inventory.module(path).is_some_and(|module| module.items.iter().all(|item| item.name != *name)) {
            failures.push(format!("the entry `{path}::{name}` of `GUEST_ADDED` must name a module that holds no item `{name}`"));
        }
    }
    for entry in mapping::HAND_WRITTEN {
        let found = inventory.module(entry.module).is_some_and(|module| match entry.item.split_once('.') {
            Some((class, method)) => {
                let method = if method == "constructor" { "new" } else { method };
                module.items.iter().any(|item| item.name == class && methods(item).iter().any(|function| function.name == method))
            }
            None => module.items.iter().any(|item| item.name == entry.item),
        });
        if !found {
            failures.push(format!(
                "the entry `{}` of `HAND_WRITTEN` must name an export or a method of the module `flm:{}`",
                entry.item, entry.module
            ));
        }
    }
    for object in mapping::STATIC_OBJECTS {
        if mapping::row(object.rust).is_none() {
            failures.push(format!("the entry `{}` of `STATIC_OBJECTS` must name a type with a row in `{MAPPING_FILE}`", object.rust));
        }
        // An enum of `api/`: one member per variant.
        if let Some((_, Item { kind: ItemKind::Type { is_enum: true, variants, .. }, .. })) = inventory.definition(object.rust) {
            for variant in variants {
                if !object.members.iter().any(|member| member.name == variant.name) {
                    failures.push(format!(
                        "the entry `{}` of `STATIC_OBJECTS` must have a member for the variant `{}`",
                        object.rust, variant.name
                    ));
                }
            }
        }
    }
}

/// The methods of a trait or a type; none for any other item.
fn methods(item: &Item) -> &[Function] {
    match &item.kind {
        ItemKind::Trait { methods, .. } | ItemKind::Type { methods, .. } => methods,
        _ => &[],
    }
}

/// Every unit enum's row lists its variants, whose tokens the crate answers
/// as distinct strings; an enum of `api/` lists exactly its variants.
fn check_unit_enums(inventory: &Inventory, failures: &mut Vec<String>) {
    for row in mapping::TYPES.iter().filter(|row| row.ts == mapping::Ts::Union) {
        let Some(tokens) = mapping::unit_enum_tokens(row.rust) else {
            failures.push(format!("the row `{}` of `{MAPPING_FILE}` must list its variants in `unit_enum_tokens`", row.rust));
            continue;
        };
        let texts: BTreeSet<&str> = tokens.iter().filter_map(|(_, token)| token.as_str()).collect();
        if texts.len() != tokens.len() {
            failures.push(format!("the tokens of `{}` must be distinct strings, not {tokens:?}", row.rust));
        }
        if let Some((_, Item { kind: ItemKind::Type { variants, .. }, .. })) = inventory.definition(row.rust) {
            let listed: Vec<&str> = tokens.iter().map(|(name, _)| *name).collect();
            let source: Vec<&str> = variants.iter().map(|variant| variant.name.as_str()).collect();
            if listed != source {
                failures.push(format!("the variants of `{}` in `unit_enum_tokens` must be the source's: {}", row.rust, source.join(", ")));
            }
        }
    }
}

// --- the values of the constants (the plan's section 3.4) -----------------------------------

/// The value of every constant of the inventory, read from the compiled
/// table of constants (`(module, name, value)`, the generated
/// `constants()`), in the inventory's order.
///
/// # Errors
///
/// The message of the plan's section 8 for the first constant the table
/// lacks; `rewritten` says whether the generated Rust file was just
/// rewritten.
fn constant_values(inventory: &Inventory, compiled: &[(&str, &str, Value)], rewritten: bool) -> Result<Vec<(String, String, Value)>, String> {
    let mut values = Vec::new();
    for (module, item) in inventory.of_class(ItemClass::Constant) {
        let found = compiled.iter().find(|(path, name, _)| *path == module.path && *name == item.name);
        match found {
            Some((_, _, value)) => values.push((module.path.clone(), item.name.clone(), value.clone())),
            None => {
                let rewritten = if rewritten { " `src/adapter/generated.rs` was rewritten;" } else { "" };
                return Err(format!(
                    "the constant `{}` of `{}` must be in the compiled table of constants before the JavaScript and the TypeScript \
                     can be written:{rewritten} build the crate and run `UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface` again",
                    item.name, module.file
                ));
            }
        }
    }
    Ok(values)
}

#[test]
fn the_constants_are_read_from_the_compiled_table() {
    let inventory = read_inventory();
    let names: Vec<(String, String)> =
        inventory.of_class(ItemClass::Constant).map(|(module, item)| (module.path.clone(), item.name.clone())).collect();
    let table: Vec<(&str, &str, Value)> = names.iter().map(|(module, name)| (module.as_str(), name.as_str(), json!(name))).collect();
    let values = constant_values(&inventory, &table, false).expect("every constant");
    assert_eq!(values.len(), 113);
    assert_eq!(values[0].2, json!(values[0].1));
    let missing = table.iter().filter(|(_, name, _)| *name != "TAG").cloned().collect::<Vec<_>>();
    assert_eq!(
        constant_values(&inventory, &missing, true),
        Err("the constant `TAG` of `src/api/stdcore/backends/html.rs` must be in the compiled table of constants before the \
             JavaScript and the TypeScript can be written: `src/adapter/generated.rs` was rewritten; build the crate and run \
             `UPDATE_GOLDEN=1 cargo test -p flm-ext --test api_surface` again"
            .to_string())
    );
    assert_eq!(
        constant_values(&inventory, &missing, false),
        Err("the constant `TAG` of `src/api/stdcore/backends/html.rs` must be in the compiled table of constants before the \
             JavaScript and the TypeScript can be written: build the crate and run `UPDATE_GOLDEN=1 cargo test -p flm-ext --test \
             api_surface` again"
            .to_string())
    );
}

#[test]
fn the_compiled_table_holds_every_constant_with_its_value() {
    let inventory = read_inventory();
    let compiled = flm_ext::adapter::constants();
    let values = constant_values(&inventory, &compiled, false).unwrap_or_else(|message| panic!("{message}"));
    assert_eq!(values.len(), compiled.len(), "the compiled table holds the inventory's constants and no other");
    let tag = values.iter().find(|(module, name, _)| module == "stdcore/backends/html" && name == "TAG").map(|(_, _, value)| value);
    assert_eq!(tag, Some(&json!(flm_ext::api::stdcore::backends::html::TAG)));
}

#[test]
fn names_answers_every_constant_of_its_module() {
    use flm_ext::adapter::dispatch::call;
    use flm_ext::adapter::handles::{module_handle, CallTable};
    let inventory = read_inventory();
    let compiled = flm_ext::adapter::constants();
    let modules: BTreeSet<&str> = compiled.iter().map(|(module, _, _)| *module).collect();
    assert_eq!(modules.into_iter().collect::<Vec<_>>(), [
        "lang/structural",
        "stdcore/backends/html",
        "stdcore/backends/latex",
        "stdcore/backends/latex/packages",
        "stdcore/backends/latex/preamble",
        "stdcore/backends/pres",
        "stdcore/features/citations",
        "stdcore/features/compose",
        "stdcore/features/math",
        "stdcore/features/numbering",
        "stdcore/features/refs",
    ]);
    for module in inventory.modules.iter().filter(|module| module.items.iter().any(|item| item.class == ItemClass::Constant)) {
        let handle = module_handle(&module.path).unwrap_or_else(|| panic!("the module `{}` has a handle", module.path));
        let mut table = CallTable::new(1);
        let names = call(&mut table, handle, "names", &json!([])).unwrap_or_else(|error| panic!("`names` of `{}`: {error}", module.path));
        let expected: serde_json::Map<String, Value> = compiled
            .iter()
            .filter(|(path, _, _)| *path == module.path)
            .map(|(_, name, value)| ((*name).to_string(), value.clone()))
            .collect();
        let listed: Vec<&str> = module.items.iter().filter(|item| item.class == ItemClass::Constant).map(|item| item.name.as_str()).collect();
        assert_eq!(expected.keys().map(String::as_str).collect::<Vec<_>>(), listed, "{}", module.path);
        assert_eq!(names, Value::Object(expected), "{}", module.path);
    }
    // The presentation elements and slots, against the library's own.
    let mut table = CallTable::new(1);
    let pres = call(&mut table, module_handle("stdcore/backends/pres").expect("a handle"), "names", &json!([])).expect("names");
    assert_eq!(pres["ALL"], json!(flm_core::stdcore::backends::pres::names::ALL));
    assert_eq!(pres["BODY_SLOT"], json!(flm_core::stdcore::backends::pres::names::BODY_SLOT));
    assert_eq!(pres.as_object().map(serde_json::Map::len), Some(27));
}
