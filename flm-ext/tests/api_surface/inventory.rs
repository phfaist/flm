//! The reader of the extension API's sources: every `.rs` file under
//! `src/api/`, in sorted path order, parsed with `syn`, read into the
//! *inventory* — the surface as the generator sees it — and each item
//! classified by the rules of the plan's section 3.2 (`dev-docs/ext/e2b`).
//!
//! # What is read
//!
//! Per module (the file's path under `api/` without `.rs`, a `mod.rs`
//! contributing to its folder's module; a module with no item is left out,
//! and the modules are in the order of their paths): the module's documentation; every
//! `pub trait` with its supertraits and methods (receiver, parameters and
//! their types, result type, whether it has a default body); every `pub
//! struct` and `pub enum` with its fields or variants and its derives, and
//! the `pub fn` items of its inherent `impl` blocks, each with its first
//! statement read as one of the three builder patterns (a push onto a field,
//! an assignment to a field, a reference to a field) or as none of them;
//! every `pub const` with its type; every module-level `pub fn`; every `pub
//! use`, one item per name it re-exports. Each item carries the first
//! sentence of its rustdoc. Trait implementations, private items, inline
//! modules (the `#[cfg(test)]` ones among them) and functions named
//! `_object_safe` are not read.
//!
//! # The normalization of types (the contract the emitters code against)
//!
//! Every type is written as one text, so that one type has one text
//! wherever it appears:
//!
//! - every lifetime is removed: the lifetime arguments of a path
//!   (`Node<'t>` is `Node`), the lifetime of a reference (`&'c str` is
//!   `&str`), and every lifetime bound of a trait object or an `impl` type
//!   (`dyn Node<'t> + 't` is `dyn Node`);
//! - whitespace is collapsed to the one form below: no space inside angle
//!   brackets or around `::`, one space after each comma, `&mut ` with its
//!   space, `dyn ` and `impl ` with theirs, ` + ` between bounds, `; ` in an
//!   array type;
//! - nothing else is rewritten: `impl Into<String>` stays as it is, and so
//!   does a path written with its module (`serde_json::Value`).
//!
//! So `Option<Box<dyn Node<'_> + '_>>`, `Option<Box<dyn Node<'t> + 't>>` and
//! `Option<Box<dyn Node<'c> + 'c>>` are all `Option<Box<dyn Node>>`, and
//! `&'c mut dyn LowerCx` is `&mut dyn LowerCx`. The types are printed from
//! `syn`'s tree, the identifiers and literals through `quote`; `quote`'s own
//! printing of a type (`Option < Box < dyn Node < '_ > + '_ > >`) keeps the
//! lifetimes and spaces every token, which is why the text is built here.

#![allow(dead_code, reason = "the emitters of the generator's outputs read every field of the inventory; this part of the generator reads some")]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use quote::ToTokens;

use crate::mapping;

/// The folder of the extension API's sources, from the crate's root.
pub const API_DIR: &str = "src/api";

/// The extension API's surface as read from its sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    /// The modules that have items, in the order of their module paths.
    pub modules: Vec<Module>,
}

/// One module of the extension API: one file under `src/api/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The module's path under `api/`, without `.rs` (`core/registry`).
    pub path: String,
    /// The file, from the crate's root (`src/api/core/registry.rs`).
    pub file: String,
    /// The first sentence of the module's documentation.
    pub doc: String,
    /// The items, in source order.
    pub items: Vec<Item>,
}

/// One item of a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The item's name.
    pub name: String,
    /// The first sentence of the item's rustdoc.
    pub doc: String,
    /// How the generator treats the item (the plan's section 3.2).
    pub class: ItemClass,
    /// Whether the item takes a lifetime parameter (`Node<'t>`), which its
    /// normalized name in a type leaves out: the generated dispatcher writes
    /// the call's lifetime there. Not written to the inventory golden.
    pub lifetime: bool,
    /// What the item is, with what was read of it.
    pub kind: ItemKind,
}

/// The class of an item: the rule of the plan's section 3.2 it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemClass {
    /// Rule 1: a sealed façade trait (a supertrait `Sealed` or `…Core`).
    SealedTrait,
    /// Rule 2: an extension-side trait (the supertraits `Send + Sync`).
    ExtensionTrait,
    /// Rule 3: an opaque value type, named in [`mapping::OPAQUE_TYPES`].
    OpaqueType,
    /// Rule 4: a data struct whose class the generator writes (a row of
    /// shape [`mapping::Ts::Class`]).
    DataClass,
    /// Rule 5: a data type with a row giving its JavaScript shape.
    DataType,
    /// Rule 6: a constant.
    Constant,
    /// Rule 7: a module-level function.
    Function,
    /// Rule 8: a re-export of an `api` item.
    ApiReexport,
    /// Rule 8: a re-export of a `flm-core` type, whose row decides.
    CoreReexport,
}

impl ItemClass {
    /// The class's name in the inventory golden.
    pub fn name(self) -> &'static str {
        match self {
            ItemClass::SealedTrait => "sealed_trait",
            ItemClass::ExtensionTrait => "extension_trait",
            ItemClass::OpaqueType => "opaque_type",
            ItemClass::DataClass => "data_class",
            ItemClass::DataType => "data_type",
            ItemClass::Constant => "constant",
            ItemClass::Function => "function",
            ItemClass::ApiReexport => "api_reexport",
            ItemClass::CoreReexport => "core_reexport",
        }
    }
}

/// What an item is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemKind {
    /// A trait.
    Trait {
        /// The supertraits, normalized (`Sealed`, `NodeCore`, `Send`).
        supertraits: Vec<String>,
        /// The methods, in source order.
        methods: Vec<Function>,
    },
    /// A struct or an enum.
    Type {
        /// `true` for an enum.
        is_enum: bool,
        /// The derives, as written (`Debug`, `serde::Serialize`).
        derives: Vec<String>,
        /// A struct's fields (a tuple struct's named `0`, `1`, …).
        fields: Vec<Field>,
        /// An enum's variants.
        variants: Vec<Variant>,
        /// The `pub fn` items of its inherent `impl` blocks, in source order.
        methods: Vec<Function>,
    },
    /// A constant.
    Const {
        /// Its type, normalized.
        ty: String,
    },
    /// A module-level function.
    Function(Function),
    /// A re-export of one name.
    Reexport {
        /// The path it re-exports, ending in the name
        /// (`flm_core::core::stage::Effect`).
        path: String,
    },
}

/// A field of a struct or of an enum's variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The field's name (its position for a tuple field).
    pub name: String,
    /// Its type, normalized.
    pub ty: String,
    /// Whether the field is `pub`.
    pub public: bool,
}

/// A variant of an enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The variant's name.
    pub name: String,
    /// Its fields (none for a unit variant).
    pub fields: Vec<Field>,
}

/// A function, a trait's method or an inherent method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The function's name.
    pub name: String,
    /// The first sentence of its rustdoc.
    pub doc: String,
    /// The receiver (`&self`, `&mut self`, `self`, `mut self`), if any.
    pub receiver: Option<String>,
    /// The parameters after the receiver, in order.
    pub params: Vec<Param>,
    /// The result type, normalized (`()` when none is written).
    pub output: String,
    /// A trait's method with a default body.
    pub default_body: bool,
    /// An inherent method's first statement, read as a builder pattern.
    pub first: Option<FirstStatement>,
}

impl Function {
    /// Whether the function is a constructor: an inherent function with no
    /// receiver.
    pub fn is_constructor(&self) -> bool {
        self.receiver.is_none() && self.first.is_some()
    }
}

/// A parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The parameter's name (`_` when it binds none).
    pub name: String,
    /// Its type, normalized.
    pub ty: String,
}

/// The first statement of an inherent method, read as one of the three
/// builder patterns of the plan's section 3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstStatement {
    /// `self.<field>.push(…);`
    Push(String),
    /// `self.<field> = …;`
    Set(String),
    /// `self.<field> = <a boolean literal>;`, the setter of a flag, which
    /// takes no parameter (`RewriteDecl::default`).
    SetTo(String, bool),
    /// `&self.<field>`
    Get(String),
    /// Anything else (or an empty body).
    Other,
}

impl FirstStatement {
    /// The statement as the inventory golden writes it.
    pub fn text(&self) -> String {
        match self {
            FirstStatement::Push(field) => format!("push {field}"),
            FirstStatement::Set(field) => format!("set {field}"),
            FirstStatement::SetTo(field, value) => format!("set {field} = {value}"),
            FirstStatement::Get(field) => format!("get {field}"),
            FirstStatement::Other => "other".to_string(),
        }
    }
}

impl Inventory {
    /// Reads every `.rs` file under `crate_root/src/api/` and classifies
    /// every item.
    ///
    /// # Panics
    ///
    /// With the texts of the plan's section 8: a file that does not parse;
    /// an item that matches no rule; a builder method of a data class whose
    /// first statement is none of the three patterns.
    pub fn read(crate_root: &Path) -> Inventory {
        let root = crate_root.join(API_DIR);
        let mut files = Vec::new();
        collect_files(&root, &mut files);
        files.sort();
        // Only the modules with items, in the order of their module paths
        // (the supervisor's ruling S6): a `mod.rs` with no item is no module
        // of the surface.
        let mut modules: Vec<Module> =
            files.iter().map(|file| read_module(crate_root, &root, file)).filter(|module| !module.items.is_empty()).collect();
        modules.sort_by(|a, b| a.path.cmp(&b.path));
        Inventory { modules }
    }

    /// The module of path `path`.
    pub fn module(&self, path: &str) -> Option<&Module> {
        self.modules.iter().find(|module| module.path == path)
    }

    /// The names of the submodules of the module `path` that are modules of
    /// the surface (a file one level below it with items), in path order: a
    /// stub exports each as a namespace (`export * as preamble from
    /// "flm:stdcore/backends/latex/preamble"`), as the Rust path
    /// `latex::preamble` reaches it.
    pub fn submodules(&self, path: &str) -> Vec<&str> {
        let prefix = format!("{path}/");
        self.modules
            .iter()
            .filter_map(|module| module.path.strip_prefix(&prefix))
            .filter(|name| !name.contains('/'))
            .collect()
    }

    /// Every item with its module, in order.
    pub fn items(&self) -> impl Iterator<Item = (&Module, &Item)> {
        self.modules.iter().flat_map(|module| module.items.iter().map(move |item| (module, item)))
    }

    /// The items of class `class`, with their modules.
    pub fn of_class(&self, class: ItemClass) -> impl Iterator<Item = (&Module, &Item)> {
        self.items().filter(move |(_, item)| item.class == class)
    }

    /// The item named `name` (the first, in module order).
    pub fn find(&self, name: &str) -> Option<(&Module, &Item)> {
        self.items().find(|(_, item)| item.name == name)
    }

    /// The definition of `name`: the item of that name that is not a
    /// re-export.
    pub fn definition(&self, name: &str) -> Option<(&Module, &Item)> {
        self.items().find(|(_, item)| item.name == name && !matches!(item.kind, ItemKind::Reexport { .. }))
    }

    /// The inventory golden's text: one JSON object per module.
    pub fn to_json(&self) -> String {
        let modules = self.modules.iter().map(module_json).collect();
        let mut out = String::new();
        write_json(&mut out, &Json::Array(modules), 0);
        out.push('\n');
        out
    }
}

/// Every `.rs` file under `dir`.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("the folder `{}` must be readable: {error}", dir.display()));
    for entry in entries {
        let path = entry.unwrap_or_else(|error| panic!("the folder `{}` must be readable: {error}", dir.display())).path();
        if path.is_dir() {
            collect_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// The module of `file`, read and classified.
fn read_module(crate_root: &Path, root: &Path, file: &Path) -> Module {
    let relative = file.strip_prefix(crate_root).unwrap_or(file);
    let file_text = relative.to_string_lossy().replace('\\', "/");
    let under = file.strip_prefix(root).unwrap_or(file).to_string_lossy().replace('\\', "/");
    let path = match under.strip_suffix("mod.rs") {
        Some(folder) => folder.trim_end_matches('/').to_string(),
        None => under.trim_end_matches(".rs").to_string(),
    };
    let source = std::fs::read_to_string(file).unwrap_or_else(|error| panic!("the file `{file_text}` must be readable: {error}"));
    let parsed = syn::parse_file(&source).unwrap_or_else(|error| panic!("the file `{file_text}` must parse as Rust: {error}"));
    let mut items: Vec<Item> = Vec::new();
    let mut impls: Vec<&syn::ItemImpl> = Vec::new();
    for item in &parsed.items {
        match item {
            syn::Item::Trait(item) if is_pub(&item.vis) => items.push(read_trait(item)),
            syn::Item::Struct(item) if is_pub(&item.vis) => items.push(read_struct(item)),
            syn::Item::Enum(item) if is_pub(&item.vis) => items.push(read_enum(item)),
            syn::Item::Const(item) if is_pub(&item.vis) => items.push(Item {
                name: item.ident.to_string(),
                doc: first_sentence(&item.attrs),
                class: ItemClass::Constant,
                lifetime: false,
                kind: ItemKind::Const { ty: type_text(&item.ty) },
            }),
            syn::Item::Fn(item) if is_pub(&item.vis) && item.sig.ident != "_object_safe" => {
                let function = read_function(&item.attrs, &item.sig, false, None);
                items.push(Item {
                    name: function.name.clone(),
                    doc: function.doc.clone(),
                    class: ItemClass::Function,
                    lifetime: false,
                    kind: ItemKind::Function(function),
                });
            }
            syn::Item::Use(item) if is_pub(&item.vis) => {
                let mut paths = Vec::new();
                use_paths(&item.tree, String::new(), &mut paths);
                for (name, path) in paths {
                    let class = if path.starts_with("crate::api::") { ItemClass::ApiReexport } else { ItemClass::CoreReexport };
                    items.push(Item { name, doc: first_sentence(&item.attrs), class, lifetime: false, kind: ItemKind::Reexport { path } });
                }
            }
            syn::Item::Impl(item) if item.trait_.is_none() => impls.push(item),
            _ => {}
        }
    }
    for block in impls {
        attach_impl(&mut items, block, &file_text);
    }
    let mut module = Module { path, file: file_text, doc: first_sentence(&parsed.attrs), items };
    classify(&mut module);
    module
}

/// Whether `vis` is `pub` (not `pub(crate)`).
fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// The names a `use` tree re-exports, each with its full path.
fn use_paths(tree: &syn::UseTree, prefix: String, out: &mut Vec<(String, String)>) {
    match tree {
        syn::UseTree::Path(path) => use_paths(&path.tree, format!("{prefix}{}::", path.ident), out),
        syn::UseTree::Name(name) => out.push((name.ident.to_string(), format!("{prefix}{}", name.ident))),
        syn::UseTree::Rename(rename) => out.push((rename.rename.to_string(), format!("{prefix}{}", rename.ident))),
        syn::UseTree::Group(group) => group.items.iter().for_each(|tree| use_paths(tree, prefix.clone(), out)),
        syn::UseTree::Glob(_) => out.push(("*".to_string(), format!("{prefix}*"))),
    }
}

/// A trait.
fn read_trait(item: &syn::ItemTrait) -> Item {
    let supertraits = item.supertraits.iter().filter_map(bound_text).collect();
    let methods = item
        .items
        .iter()
        .filter_map(|member| match member {
            syn::TraitItem::Fn(method) => {
                let mut function = read_function(&method.attrs, &method.sig, false, None);
                function.default_body = method.default.is_some();
                Some(function)
            }
            _ => None,
        })
        .collect();
    Item {
        name: item.ident.to_string(),
        doc: first_sentence(&item.attrs),
        class: ItemClass::SealedTrait,
        lifetime: has_lifetime(&item.generics),
        kind: ItemKind::Trait { supertraits, methods },
    }
}

/// A struct.
fn read_struct(item: &syn::ItemStruct) -> Item {
    Item {
        name: item.ident.to_string(),
        doc: first_sentence(&item.attrs),
        class: ItemClass::DataType,
        lifetime: has_lifetime(&item.generics),
        kind: ItemKind::Type {
            is_enum: false,
            derives: derives(&item.attrs),
            fields: fields(&item.fields),
            variants: Vec::new(),
            methods: Vec::new(),
        },
    }
}

/// An enum.
fn read_enum(item: &syn::ItemEnum) -> Item {
    let variants =
        item.variants.iter().map(|variant| Variant { name: variant.ident.to_string(), fields: fields(&variant.fields) }).collect();
    Item {
        name: item.ident.to_string(),
        doc: first_sentence(&item.attrs),
        class: ItemClass::DataType,
        lifetime: has_lifetime(&item.generics),
        kind: ItemKind::Type { is_enum: true, derives: derives(&item.attrs), fields: Vec::new(), variants, methods: Vec::new() },
    }
}

/// Whether `generics` declares a lifetime parameter.
fn has_lifetime(generics: &syn::Generics) -> bool {
    generics.lifetimes().next().is_some()
}

/// The fields of a struct or a variant.
fn fields(fields: &syn::Fields) -> Vec<Field> {
    fields
        .iter()
        .enumerate()
        .map(|(position, field)| Field {
            name: field.ident.as_ref().map_or_else(|| position.to_string(), ToString::to_string),
            ty: type_text(&field.ty),
            public: is_pub(&field.vis),
        })
        .collect()
}

/// The derives of an item, as written.
fn derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("derive")) {
        let _ = attr.parse_nested_meta(|meta| {
            out.push(path_text(&meta.path));
            Ok(())
        });
    }
    out
}

/// Attaches the `pub fn` items of an inherent `impl` block to the type it
/// implements, which must be an item of the same file.
fn attach_impl(items: &mut [Item], block: &syn::ItemImpl, file: &str) {
    let syn::Type::Path(self_type) = &*block.self_ty else { return };
    let Some(name) = self_type.path.segments.last().map(|segment| segment.ident.to_string()) else { return };
    let functions: Vec<Function> = block
        .items
        .iter()
        .filter_map(|member| match member {
            syn::ImplItem::Fn(method) if is_pub(&method.vis) => {
                Some(read_function(&method.attrs, &method.sig, true, method.block.stmts.first()))
            }
            _ => None,
        })
        .collect();
    if functions.is_empty() {
        return;
    }
    let target = items.iter_mut().find(|item| item.name == name && matches!(item.kind, ItemKind::Type { .. }));
    match target {
        Some(Item { kind: ItemKind::Type { methods, .. }, .. }) => methods.extend(functions),
        _ => panic!("the inherent `impl` of `{name}` in `{file}` must stand in the file that defines `{name}`"),
    }
}

/// A function's signature and documentation; `first`, for an inherent
/// method, its body's first statement.
fn read_function(attrs: &[syn::Attribute], sig: &syn::Signature, inherent: bool, first: Option<&syn::Stmt>) -> Function {
    let mut receiver = None;
    let mut params = Vec::new();
    for input in &sig.inputs {
        match input {
            syn::FnArg::Receiver(this) => {
                receiver = Some(match (&this.reference, this.mutability) {
                    (Some(_), Some(_)) => "&mut self",
                    (Some(_), None) => "&self",
                    (None, Some(_)) => "mut self",
                    (None, None) => "self",
                }
                .to_string());
            }
            syn::FnArg::Typed(typed) => {
                let name = match &*typed.pat {
                    syn::Pat::Ident(ident) => ident.ident.to_string(),
                    other => collapse(&other.to_token_stream().to_string()),
                };
                params.push(Param { name, ty: type_text(&typed.ty) });
            }
        }
    }
    let output = match &sig.output {
        syn::ReturnType::Default => "()".to_string(),
        syn::ReturnType::Type(_, ty) => type_text(ty),
    };
    Function {
        name: sig.ident.to_string(),
        doc: first_sentence(attrs),
        receiver,
        params,
        output,
        default_body: false,
        first: inherent.then(|| first.map_or(FirstStatement::Other, first_statement)),
    }
}

/// A statement read as one of the three builder patterns.
fn first_statement(stmt: &syn::Stmt) -> FirstStatement {
    let syn::Stmt::Expr(expr, _) = stmt else { return FirstStatement::Other };
    match expr {
        syn::Expr::MethodCall(call) if call.method == "push" => self_field(&call.receiver).map_or(FirstStatement::Other, FirstStatement::Push),
        syn::Expr::Assign(assign) => match (self_field(&assign.left), &*assign.right) {
            (Some(field), syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(value), .. })) => FirstStatement::SetTo(field, value.value),
            (field, _) => field.map_or(FirstStatement::Other, FirstStatement::Set),
        },
        syn::Expr::Reference(reference) if reference.mutability.is_none() => {
            self_field(&reference.expr).map_or(FirstStatement::Other, FirstStatement::Get)
        }
        _ => FirstStatement::Other,
    }
}

/// The field `f` of an expression `self.f`.
fn self_field(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Field(field) = expr else { return None };
    let syn::Expr::Path(base) = &*field.base else { return None };
    if !base.path.is_ident("self") {
        return None;
    }
    match &field.member {
        syn::Member::Named(name) => Some(name.to_string()),
        syn::Member::Unnamed(index) => Some(index.index.to_string()),
    }
}

/// The first sentence of the documentation `attrs` hold: the first
/// paragraph, its lines joined, up to the first period outside backticks
/// that ends the text or is followed by a space.
pub fn first_sentence(attrs: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("doc")) {
        if let syn::Meta::NameValue(syn::MetaNameValue { value: syn::Expr::Lit(lit), .. }) = &attr.meta
            && let syn::Lit::Str(text) = &lit.lit
        {
            lines.push(text.value());
        }
    }
    let mut paragraph = String::new();
    for line in &lines {
        let line = line.trim();
        if line.is_empty() {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line);
    }
    let chars: Vec<char> = paragraph.chars().collect();
    let mut in_code = false;
    for (position, &c) in chars.iter().enumerate() {
        match c {
            '`' => in_code = !in_code,
            '.' if !in_code && chars.get(position + 1).is_none_or(|next| next.is_whitespace()) => {
                return chars[..=position].iter().collect();
            }
            _ => {}
        }
    }
    paragraph
}

// --- the normalization of types -------------------------------------------------------------

/// The normalized text of `ty` (the module documentation states the form).
pub fn type_text(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) if path.qself.is_none() => path_text(&path.path),
        syn::Type::Reference(reference) => {
            let mutability = if reference.mutability.is_some() { "mut " } else { "" };
            format!("&{mutability}{}", type_text(&reference.elem))
        }
        syn::Type::TraitObject(object) => format!("dyn {}", bounds_text(object.bounds.iter())),
        syn::Type::ImplTrait(implementation) => format!("impl {}", bounds_text(implementation.bounds.iter())),
        syn::Type::Tuple(tuple) => {
            let elems: Vec<String> = tuple.elems.iter().map(type_text).collect();
            if elems.len() == 1 { format!("({},)", elems[0]) } else { format!("({})", elems.join(", ")) }
        }
        syn::Type::Slice(slice) => format!("[{}]", type_text(&slice.elem)),
        syn::Type::Array(array) => format!("[{}; {}]", type_text(&array.elem), collapse(&array.len.to_token_stream().to_string())),
        syn::Type::Paren(paren) => type_text(&paren.elem),
        syn::Type::Group(group) => type_text(&group.elem),
        other => collapse(&strip_lifetimes(&other.to_token_stream().to_string())),
    }
}

/// The text of the trait bounds of a trait object or an `impl` type, the
/// lifetime bounds left out.
fn bounds_text<'a>(bounds: impl Iterator<Item = &'a syn::TypeParamBound>) -> String {
    bounds.filter_map(bound_text).collect::<Vec<_>>().join(" + ")
}

/// The text of one trait bound; `None` for a lifetime bound.
fn bound_text(bound: &syn::TypeParamBound) -> Option<String> {
    match bound {
        syn::TypeParamBound::Trait(bound) => {
            let maybe = if matches!(bound.modifier, syn::TraitBoundModifier::Maybe(_)) { "?" } else { "" };
            Some(format!("{maybe}{}", path_text(&bound.path)))
        }
        syn::TypeParamBound::Lifetime(_) => None,
        other => Some(collapse(&strip_lifetimes(&other.to_token_stream().to_string()))),
    }
}

/// The text of a path, its lifetime arguments left out.
fn path_text(path: &syn::Path) -> String {
    let mut out = String::new();
    if path.leading_colon.is_some() {
        out.push_str("::");
    }
    for (position, segment) in path.segments.iter().enumerate() {
        if position > 0 {
            out.push_str("::");
        }
        out.push_str(&segment.ident.to_string());
        match &segment.arguments {
            syn::PathArguments::None => {}
            syn::PathArguments::AngleBracketed(arguments) => {
                let texts: Vec<String> = arguments
                    .args
                    .iter()
                    .filter_map(|argument| match argument {
                        syn::GenericArgument::Lifetime(_) => None,
                        syn::GenericArgument::Type(ty) => Some(type_text(ty)),
                        syn::GenericArgument::AssocType(assoc) => Some(format!("{} = {}", assoc.ident, type_text(&assoc.ty))),
                        other => Some(collapse(&strip_lifetimes(&other.to_token_stream().to_string()))),
                    })
                    .collect();
                if !texts.is_empty() {
                    let _ = write!(out, "<{}>", texts.join(", "));
                }
            }
            syn::PathArguments::Parenthesized(arguments) => {
                let inputs: Vec<String> = arguments.inputs.iter().map(type_text).collect();
                let _ = write!(out, "({})", inputs.join(", "));
                if let syn::ReturnType::Type(_, ty) = &arguments.output {
                    let _ = write!(out, " -> {}", type_text(ty));
                }
            }
        }
    }
    out
}

/// `text` without its lifetimes (`'t`, `'_`, and a `+` before one).
fn strip_lifetimes(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<&str> = Vec::new();
    let mut skip_comma = false;
    for token in tokens {
        if token.starts_with('\'') {
            if out.last() == Some(&"+") {
                out.pop();
            }
            skip_comma = true;
            continue;
        }
        if skip_comma && token == "," {
            skip_comma = false;
            continue;
        }
        skip_comma = false;
        out.push(token);
    }
    out.join(" ")
}

/// `quote`'s spaced text in the normalized spacing.
fn collapse(text: &str) -> String {
    let mut out = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for (from, to) in [(" < ", "<"), ("< ", "<"), (" >", ">"), (" :: ", "::"), (" ,", ","), ("& ", "&"), ("( ", "("), (" )", ")"), (" ;", ";")] {
        out = out.replace(from, to);
    }
    out
}

// --- the classification (the plan's section 3.2) --------------------------------------------

/// Classifies every item of `module` by the rules of the plan's section 3.2.
fn classify(module: &mut Module) {
    let file = module.file.clone();
    for item in &mut module.items {
        item.class = match &item.kind {
            ItemKind::Trait { supertraits, .. } => {
                if supertraits.iter().any(|name| name == "Sealed" || name.ends_with("Core")) {
                    ItemClass::SealedTrait
                } else if supertraits.len() == 2 && supertraits.iter().all(|name| name == "Send" || name == "Sync") {
                    ItemClass::ExtensionTrait
                } else {
                    no_rule(&item.name, &file)
                }
            }
            ItemKind::Type { is_enum, methods, .. } => {
                if mapping::OPAQUE_TYPES.contains(&item.name.as_str()) {
                    ItemClass::OpaqueType
                } else {
                    match mapping::row(&item.name).map(|row| row.ts) {
                        Some(mapping::Ts::Class) if !is_enum && methods.iter().any(|method| method.name == "new") => {
                            check_builders(&item.name, methods, &file);
                            ItemClass::DataClass
                        }
                        Some(mapping::Ts::Class) | None => no_rule(&item.name, &file),
                        Some(_) => ItemClass::DataType,
                    }
                }
            }
            ItemKind::Const { .. } => ItemClass::Constant,
            ItemKind::Function(_) => ItemClass::Function,
            ItemKind::Reexport { path } => {
                if path.starts_with("crate::api::") {
                    ItemClass::ApiReexport
                } else if mapping::row(&item.name).is_some() {
                    ItemClass::CoreReexport
                } else {
                    no_rule(&item.name, &file)
                }
            }
        };
    }
}

/// The failure of an item that matches no rule.
fn no_rule(name: &str, file: &str) -> ! {
    panic!(
        "the item `{name}` of `{file}` must be a sealed façade trait, an extension-side trait, a data type with a row in \
         `tests/api_surface/mapping.rs`, a constant, a function or a re-export"
    )
}

/// Checks that every builder method of the data class `name` (a method with
/// a receiver, not written by hand) begins with one of the three patterns.
fn check_builders(name: &str, methods: &[Function], file: &str) {
    for method in methods.iter().filter(|method| method.receiver.is_some()) {
        if mapping::is_hand_written(name, &method.name) {
            continue;
        }
        if matches!(method.first, Some(FirstStatement::Other) | None) {
            panic!(
                "the first statement of `{name}::{}` (`{file}`) must push onto a field, assign a field or answer a field, so \
                 that the generated class can hold it",
                method.name
            );
        }
    }
}

// --- the inventory golden -------------------------------------------------------------------

/// A JSON value with its keys in a fixed order.
pub enum Json {
    /// A string.
    Str(String),
    /// A boolean.
    Bool(bool),
    /// A list.
    Array(Vec<Json>),
    /// A map, its keys in the order given.
    Object(Vec<(&'static str, Json)>),
}

/// A string value.
fn s(text: &str) -> Json {
    Json::Str(text.to_string())
}

/// A list of strings.
fn strings<'a>(texts: impl IntoIterator<Item = &'a String>) -> Json {
    Json::Array(texts.into_iter().map(|text| s(text)).collect())
}

/// The JSON object of a module.
fn module_json(module: &Module) -> Json {
    Json::Object(vec![
        ("module", s(&module.path)),
        ("file", s(&module.file)),
        ("doc", s(&module.doc)),
        ("items", Json::Array(module.items.iter().map(item_json).collect())),
    ])
}

/// The JSON object of an item.
fn item_json(item: &Item) -> Json {
    let mut fields = vec![("name", s(&item.name)), ("class", s(item.class.name())), ("doc", s(&item.doc))];
    match &item.kind {
        ItemKind::Trait { supertraits, methods } => {
            fields.push(("supertraits", strings(supertraits)));
            fields.push(("methods", Json::Array(methods.iter().map(function_json).collect())));
        }
        ItemKind::Type { is_enum, derives, fields: members, variants, methods } => {
            fields.push(("derives", strings(derives)));
            if *is_enum {
                fields.push(("variants", Json::Array(variants.iter().map(variant_json).collect())));
            } else {
                fields.push(("fields", Json::Array(members.iter().map(field_json).collect())));
            }
            fields.push(("methods", Json::Array(methods.iter().map(function_json).collect())));
        }
        ItemKind::Const { ty } => fields.push(("type", s(ty))),
        ItemKind::Function(function) => {
            fields.push(("params", Json::Array(function.params.iter().map(param_json).collect())));
            fields.push(("output", s(&function.output)));
        }
        ItemKind::Reexport { path } => fields.push(("path", s(path))),
    }
    Json::Object(fields)
}

/// The JSON object of a function or a method.
fn function_json(function: &Function) -> Json {
    let mut fields = vec![("name", s(&function.name)), ("doc", s(&function.doc))];
    if let Some(receiver) = &function.receiver {
        fields.push(("receiver", s(receiver)));
    }
    fields.push(("params", Json::Array(function.params.iter().map(param_json).collect())));
    fields.push(("output", s(&function.output)));
    if function.default_body {
        fields.push(("default_body", Json::Bool(true)));
    }
    if let Some(first) = &function.first {
        fields.push(("first", s(&first.text())));
    }
    Json::Object(fields)
}

/// A parameter as `name: type`.
fn param_json(param: &Param) -> Json {
    s(&format!("{}: {}", param.name, param.ty))
}

/// A field as `name: type`, `pub` before a public one.
fn field_json(field: &Field) -> Json {
    let visibility = if field.public { "pub " } else { "" };
    s(&format!("{visibility}{}: {}", field.name, field.ty))
}

/// A variant as its name and its fields.
fn variant_json(variant: &Variant) -> Json {
    if variant.fields.is_empty() {
        s(&variant.name)
    } else {
        let fields: Vec<String> = variant.fields.iter().map(|field| format!("{}: {}", field.name, field.ty)).collect();
        s(&format!("{} {{ {} }}", variant.name, fields.join(", ")))
    }
}

/// Writes `value` at the indentation `depth`: a map or a list one entry per
/// line, except a list of strings alone, on one line.
pub fn write_json(out: &mut String, value: &Json, depth: usize) {
    let pad = "  ".repeat(depth + 1);
    let close = "  ".repeat(depth);
    match value {
        Json::Str(text) => out.push_str(&serde_json::Value::String(text.clone()).to_string()),
        Json::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Array(items) if items.iter().all(|item| matches!(item, Json::Str(_))) => {
            out.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    out.push_str(", ");
                }
                write_json(out, item, depth);
            }
            out.push(']');
        }
        Json::Array(items) => {
            out.push_str("[\n");
            for (position, item) in items.iter().enumerate() {
                out.push_str(&pad);
                write_json(out, item, depth + 1);
                out.push_str(if position + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&close);
            out.push(']');
        }
        Json::Object(fields) => {
            out.push_str("{\n");
            for (position, (key, item)) in fields.iter().enumerate() {
                let _ = write!(out, "{pad}\"{key}\": ");
                write_json(out, item, depth + 1);
                out.push_str(if position + 1 < fields.len() { ",\n" } else { "\n" });
            }
            out.push_str(&close);
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The normalized text of a type written in Rust.
    fn normalized(text: &str) -> String {
        type_text(&syn::parse_str(text).expect("a type"))
    }

    #[test]
    fn the_four_forms_of_a_node_are_one_type() {
        for written in ["Option<Box<dyn Node<'_> + '_>>", "Option<Box<dyn Node<'t> + 't>>", "Option<Box<dyn Node<'c> + 'c>>", "Option<Box<dyn Node>>"] {
            assert_eq!(normalized(written), "Option<Box<dyn Node>>", "{written}");
        }
        assert_eq!(normalized("&'c mut dyn LowerCx"), "&mut dyn LowerCx");
        assert_eq!(normalized("& 't dyn Node < 't >"), "&dyn Node");
        assert_eq!(normalized("&[Box<dyn Node<'t> + 't>]"), "&[Box<dyn Node>]");
        assert_eq!(normalized("Result<Option<TableValue<'_>>, ExtError>"), "Result<Option<TableValue>, ExtError>");
        assert_eq!(normalized("Vec<(String, Vec<Staged>)>"), "Vec<(String, Vec<Staged>)>");
        assert_eq!(normalized("impl IntoIterator<Item = Mode>"), "impl IntoIterator<Item = Mode>");
        assert_eq!(normalized("impl Into<String>"), "impl Into<String>");
        assert_eq!(normalized("[(&str, bool); 6]"), "[(&str, bool); 6]");
        assert_eq!(normalized("()"), "()");
    }

    #[test]
    fn the_first_sentence_ends_at_a_period_outside_code() {
        let attrs: Vec<syn::Attribute> = syn::parse_quote! {
            /// Mirrors [`PAR`](names::PAR): `flm.pres.par`, a laid-out
            /// paragraph. More.
            ///
            /// A second paragraph.
        };
        assert_eq!(first_sentence(&attrs), "Mirrors [`PAR`](names::PAR): `flm.pres.par`, a laid-out paragraph.");
    }
}
