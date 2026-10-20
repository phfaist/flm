//! The element declarations a registry was built from, kept as declared:
//! [`Declarations`], and the references into them that its accessors return.
//!
//! A feature declares what an author may write in a document with element
//! declarations ([`ElementDef`]: the names a command is written under, its
//! syntax, its arguments, its parsing modes and its documentation), scoped
//! packages ([`ScopedPackageDef`]), fallbacks ([`FallbackDef`]) and overrides
//! ([`OverrideDef`]). The registry build turns them into the FLM language
//! that documents are parsed with
//! ([`FlmRegistry::lang`](super::FlmRegistry::lang)), which keeps only what a
//! parse needs. The registry also keeps the declarations themselves in a
//! [`Declarations`] value
//! ([`FlmRegistry::declarations`](super::FlmRegistry::declarations)), so
//! that a program can describe what an author may write: every command with
//! its syntax, its arguments and their roles, and its documentation.
//!
//! The declarations are the ones the build used: a [peer element
//! value](super::FeatureBuilder::peer_element_value) whose element type some
//! feature declares is kept where it was declared, and one whose element type
//! no feature declares is not there, since the build dropped it. Each element
//! declaration holds the element value the registry numbered, with the
//! aspects features attached to it.
//!
//! Keeping the declarations costs little: a declaration shares its parsers and
//! its element value with the language through `Arc`s, and only its strings
//! are its own.

use crate::lang::mint::packages::feature_stack_order;
use crate::lang::mint::{
    ArgDef, CallableKind, ElementDef, FallbackDef, OverrideDef, ScopedPackageDef, SyntaxDef,
    TackOnField,
};

/// The element declarations, scoped packages, fallbacks and overrides that
/// the features of a registry declared, as declared.
///
/// The element declarations are grouped by feature, in the order in which
/// the language stacks the features' definitions (see
/// [`elements`](Declarations::elements)), so that the last declaration of a
/// command is the one in force. Everything else is in registration order.
///
/// Get it from
/// [`FlmRegistry::declarations`](super::FlmRegistry::declarations). The list
/// accessors ([`elements`](Declarations::elements),
/// [`scoped_packages`](Declarations::scoped_packages),
/// [`fallbacks`](Declarations::fallbacks),
/// [`overrides`](Declarations::overrides)) return everything; the other
/// accessors answer the usual questions of a program that describes the
/// language: the commands of one feature
/// ([`elements_of`](Declarations::elements_of)), the declarations of one
/// command ([`named`](Declarations::named)), a scoped package and the places
/// that make it available ([`scoped_package`](Declarations::scoped_package),
/// [`scoped_package_uses`](Declarations::scoped_package_uses)), and the
/// arguments that hold a given kind of thing
/// ([`arguments`](Declarations::arguments),
/// [`role_uses`](Declarations::role_uses)).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, build_registry};
/// use flm_core::lang::mint::{ArgDef, CallableKind, ElementDef, SyntaxDef};
/// use flm_core::model::element::ElementSpec;
///
/// struct Shout;
/// impl FlmFeature for Shout {
///     fn info(&self) -> FeatureInfo { FeatureInfo::new("shout", "Shouting") }
///     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
///         let element = Arc::new(ElementSpec::new("shout.shout"));
///         let syntax = SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] };
///         b.element(ElementDef::new("shout", ["shout"], syntax, element).docs("Shouted text."))
///     }
/// }
///
/// let registry = build_registry([Arc::new(Shout) as Arc<dyn FlmFeature>])?;
/// let found = registry.declarations().named(CallableKind::Macro, "shout");
/// assert_eq!(found.len(), 1);
/// # #[cfg(feature = "docs")]
/// assert_eq!(found[0].docs, "Shouted text.");
/// assert_eq!(found[0].syntax.args()[0].name.as_deref(), Some("text"));
/// # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Declarations {
    elements: Vec<ElementDef>,
    scoped_packages: Vec<ScopedPackageDef>,
    fallbacks: Vec<FallbackDef>,
    overrides: Vec<OverrideDef>,
}

/// One argument declaration among a registry's [`Declarations`], with where
/// it is declared: the element declaration it belongs to, the scoped package
/// that holds that declaration, and the tack-on field whose own argument list
/// holds it.
///
/// [`Declarations::arguments`] and [`Declarations::role_uses`] return it.
#[derive(Debug, Clone, Copy)]
pub struct ArgumentRef<'a> {
    /// The element declaration the argument belongs to. Its
    /// [`owner`](ElementDef::owner) is the feature that declared it.
    pub declaration: &'a ElementDef,
    /// The scoped package that holds the element declaration, or `None` for
    /// a declaration a feature made directly
    /// ([`FeatureBuilder::element`](super::FeatureBuilder::element)).
    pub within: Option<&'a ScopedPackageDef>,
    /// The [tack-on field](TackOnField) whose own argument list holds the
    /// argument (the innermost one, for a field written in another field's
    /// argument), or `None` for an argument of the element declaration
    /// itself.
    pub field: Option<&'a TackOnField>,
    /// The argument's position, counting from zero, in the argument list
    /// that holds it: the element declaration's own, or the field's.
    pub index: usize,
    /// The argument declaration.
    pub arg: &'a ArgDef,
}

/// One place where an element declaration makes a scoped package available:
/// an environment's body ([`BodyDef::scoped`](crate::lang::mint::BodyDef::scoped))
/// or an argument ([`ArgDef::scoped`]).
///
/// [`Declarations::scoped_package_uses`] returns it.
#[derive(Debug, Clone, Copy)]
pub enum ScopedPackageUse<'a> {
    /// The body of an environment makes the package available inside it.
    Body {
        /// The environment's declaration. Its [`owner`](ElementDef::owner)
        /// is the feature that declared it.
        declaration: &'a ElementDef,
        /// The scoped package that holds the declaration, or `None` for a
        /// declaration a feature made directly.
        within: Option<&'a ScopedPackageDef>,
    },
    /// An argument makes the package available inside it.
    Argument(ArgumentRef<'a>),
}

impl<'a> ScopedPackageUse<'a> {
    /// The element declaration whose body or argument makes the package
    /// available.
    pub fn declaration(&self) -> &'a ElementDef {
        match self {
            ScopedPackageUse::Body { declaration, .. } => declaration,
            ScopedPackageUse::Argument(argument) => argument.declaration,
        }
    }

    /// The scoped package that holds that declaration, or `None` for a
    /// declaration a feature made directly.
    pub fn within(&self) -> Option<&'a ScopedPackageDef> {
        match self {
            ScopedPackageUse::Body { within, .. } => *within,
            ScopedPackageUse::Argument(argument) => argument.within,
        }
    }
}

/// Adds to `out` every argument of the element declaration `def`, held in
/// the scoped package `within` if any, in the order written: each argument
/// of the declaration, followed by the arguments of each of its tack-on
/// fields, and so on for a field's own tack-on arguments.
pub(crate) fn arguments_of<'a>(
    def: &'a ElementDef,
    within: Option<&'a ScopedPackageDef>,
    out: &mut Vec<ArgumentRef<'a>>,
) {
    arguments_in(def, within, None, def.syntax.args(), out);
}

/// Adds to `out` the arguments `args` of the declaration `def`, held in the
/// list of the tack-on field `field` (or the declaration's own list), each
/// followed by the arguments of its tack-on fields.
fn arguments_in<'a>(
    def: &'a ElementDef,
    within: Option<&'a ScopedPackageDef>,
    field: Option<&'a TackOnField>,
    args: &'a [ArgDef],
    out: &mut Vec<ArgumentRef<'a>>,
) {
    for (index, arg) in args.iter().enumerate() {
        out.push(ArgumentRef { declaration: def, within, field, index, arg });
        for inner in arg.kind.tack_on_fields().unwrap_or_default() {
            arguments_in(def, within, Some(inner), &inner.args, out);
        }
    }
}

impl Declarations {
    /// The declarations as the build leaves them, moved into the frozen
    /// registry, with the element declarations grouped by feature in the
    /// order of [`feature_stack_order`].
    pub(crate) fn new(
        mut elements: Vec<ElementDef>,
        scoped_packages: Vec<ScopedPackageDef>,
        fallbacks: Vec<FallbackDef>,
        overrides: Vec<OverrideDef>,
    ) -> Declarations {
        // The build lists a finalizer's declarations after every declaration
        // of the first registration round, while the language stacks each
        // feature's definitions together, at the place of its first
        // declaration. The sort is stable, so each feature's declarations
        // keep the order it made them in.
        let order = feature_stack_order(&elements);
        elements.sort_by_key(|def| order.iter().position(|owner| *owner == def.owner));
        Declarations { elements, scoped_packages, fallbacks, overrides }
    }

    /// Every element declaration a feature made directly
    /// ([`FeatureBuilder::element`](super::FeatureBuilder::element)),
    /// grouped by feature. The declarations held by scoped packages are not
    /// among them: see [`scoped_packages`](Declarations::scoped_packages).
    ///
    /// The features come in the order in which the language stacks their
    /// definitions, a later feature's definition of a command replacing an
    /// earlier one's: the order of each feature's first element declaration,
    /// which is the final order of the features
    /// ([`FlmRegistry::features`](super::FlmRegistry::features)) among those
    /// that declare elements when they register. A feature whose only
    /// element declarations come from a [`Finalizer`](super::Finalizer)
    /// comes after them. Each feature's declarations are in the order it made
    /// them, its finalizers' last.
    pub fn elements(&self) -> &[ElementDef] {
        &self.elements
    }

    /// Every scoped package, with the element declarations it holds, in
    /// registration order.
    pub fn scoped_packages(&self) -> &[ScopedPackageDef] {
        &self.scoped_packages
    }

    /// Every fallback, in registration order.
    pub fn fallbacks(&self) -> &[FallbackDef] {
        &self.fallbacks
    }

    /// Every override, in registration order.
    pub fn overrides(&self) -> &[OverrideDef] {
        &self.overrides
    }

    /// The element declarations that the feature called `feature` made
    /// directly, in the order it made them. The declarations of its scoped
    /// packages are not among them: see
    /// [`scoped_packages_of`](Declarations::scoped_packages_of).
    pub fn elements_of<'a>(&'a self, feature: &'a str) -> impl Iterator<Item = &'a ElementDef> + 'a {
        self.elements.iter().filter(move |def| def.owner == feature)
    }

    /// The scoped packages that the feature called `feature` declared, in the
    /// order it declared them.
    pub fn scoped_packages_of<'a>(
        &'a self,
        feature: &'a str,
    ) -> impl Iterator<Item = &'a ScopedPackageDef> + 'a {
        self.scoped_packages.iter().filter(move |package| package.owner == feature)
    }

    /// The element declarations, across features, that define the command
    /// `name` of the callable kind `kind`, in the order of
    /// [`elements`](Declarations::elements). `name` is
    /// written without the escape character (`section`, `itemize`); for a
    /// special character sequence it is the trigger (`~`). A file-inclusion
    /// macro is a macro.
    ///
    /// Several declarations define one command when their parsing modes
    /// differ ([`ElementDef::modes`]), or when one feature's definition
    /// replaces another's. In a given parsing mode, the definition in force
    /// is the last one in this list that is available in that mode (a
    /// feature declares a command at most once per parsing mode, so this is
    /// the definition of the feature whose definitions the language stacks
    /// last). Only the declarations
    /// that features made directly are searched; a scoped package's command
    /// is found through [`scoped_package`](Declarations::scoped_package).
    pub fn named(&self, kind: CallableKind, name: &str) -> Vec<&ElementDef> {
        self.elements.iter().filter(|def| defines(def, kind, name)).collect()
    }

    /// The scoped package called `name`, or `None` when no feature declares
    /// one. A registry holds at most one scoped package of a name.
    pub fn scoped_package(&self, name: &str) -> Option<&ScopedPackageDef> {
        self.scoped_packages.iter().find(|package| package.name == *name)
    }

    /// Every place where an element declaration makes the scoped package
    /// called `name` available: each environment body and each argument
    /// whose declaration lists the package ([`BodyDef::scoped`](crate::lang::mint::BodyDef::scoped),
    /// [`ArgDef::scoped`]). The declarations that features made directly come
    /// first, in the order of [`elements`](Declarations::elements), then
    /// those held by scoped packages, in registration order.
    /// A use by a feature that declared the package as a peer is listed
    /// whether or not some feature declares the package.
    pub fn scoped_package_uses(&self, name: &str) -> Vec<ScopedPackageUse<'_>> {
        let mut uses = Vec::new();
        for (def, within) in self.all_elements() {
            if let SyntaxDef::Environment { body, .. } = &def.syntax {
                if body.scoped.iter().any(|package| package == name) {
                    uses.push(ScopedPackageUse::Body { declaration: def, within });
                }
            }
            let mut arguments = Vec::new();
            arguments_of(def, within, &mut arguments);
            uses.extend(
                arguments
                    .into_iter()
                    .filter(|argument| argument.arg.scoped.iter().any(|package| package == name))
                    .map(ScopedPackageUse::Argument),
            );
        }
        uses
    }

    /// Every argument of every element declaration, including the arguments
    /// of the tack-on fields: the declarations that features made directly
    /// first, in the order of [`elements`](Declarations::elements), then
    /// those held by scoped packages, in registration order; each
    /// declaration's arguments come in the order written.
    pub fn arguments(&self) -> Vec<ArgumentRef<'_>> {
        let mut out = Vec::new();
        for (def, within) in self.all_elements() {
            arguments_of(def, within, &mut out);
        }
        out
    }

    /// Every argument whose [role](ArgDef::role) is `role`, in the order of
    /// [`arguments`](Declarations::arguments).
    pub fn role_uses(&self, role: &str) -> Vec<ArgumentRef<'_>> {
        self.arguments()
            .into_iter()
            .filter(|argument| argument.arg.role.as_ref().is_some_and(|name| name == role))
            .collect()
    }

    /// Every element declaration with the scoped package that holds it: the
    /// declarations that features made directly first, then those of each
    /// scoped package.
    fn all_elements(&self) -> impl Iterator<Item = (&ElementDef, Option<&ScopedPackageDef>)> {
        self.elements.iter().map(|def| (def, None)).chain(
            self.scoped_packages
                .iter()
                .flat_map(|package| package.elements.iter().map(move |def| (def, Some(package)))),
        )
    }
}

/// Whether the declaration `def` defines the command `name` of the callable
/// kind `kind`: one of its names, or its trigger for a special character
/// sequence.
fn defines(def: &ElementDef, kind: CallableKind, name: &str) -> bool {
    if def.syntax.kind() != Some(kind) {
        return false;
    }
    match &def.syntax {
        SyntaxDef::Specials { trigger, .. } => trigger == name,
        _ => def.names.iter().any(|written| written == name),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use techy::latexlike::Mode;

    use crate::core::registry::{
        build_registry, BuildError, FeatureBuilder, FeatureInfo, Finalizer, FlmFeature,
        FlmRegistry, RegistryBuildError, UnresolvedPeer,
    };
    use crate::lang::mint::{
        ArgDef, ArgStdKind, BodyDef, CallableKind, ElementDef, FallbackDef, FallbackKind,
        ScopedPackageDef, Symbol, SyntaxDef, TackOnField,
    };
    use crate::model::element::ElementSpec;
    use crate::model::names::Name;

    type Register = dyn Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync;

    /// A feature called `name` whose registration is the closure it holds.
    struct Test {
        name: &'static str,
        register: Box<Register>,
    }

    impl FlmFeature for Test {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new(self.name, self.name)
        }

        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            (self.register)(b)
        }
    }

    fn feature(
        name: &'static str,
        register: impl Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync + 'static,
    ) -> Arc<dyn FlmFeature> {
        Arc::new(Test { name, register: Box::new(register) })
    }

    fn built(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
        build_registry(features).unwrap_or_else(|error| panic!("{}", error.render()))
    }

    fn refused(features: Vec<Arc<dyn FlmFeature>>) -> RegistryBuildError {
        match build_registry(features) {
            Ok(_) => panic!("the registry build was expected to fail"),
            Err(error) => error,
        }
    }

    fn element(name: &str) -> Arc<ElementSpec> {
        Arc::new(ElementSpec::new(name))
    }

    /// A macro `\name` of `owner` taking `args`, whose element is `owner.name`.
    fn macro_with(owner: &str, name: &str, args: Vec<ArgDef>) -> ElementDef {
        ElementDef::new(owner, [name], SyntaxDef::Macro { args }, element(&format!("{owner}.{name}")))
    }

    fn arg_names(def: &ElementDef) -> Vec<Option<&str>> {
        def.syntax.args().iter().map(|arg| arg.name.as_deref()).collect()
    }

    fn notes(registry: &FlmRegistry) -> Vec<String> {
        registry.build_diagnostics().iter().map(|diagnostic| diagnostic.message()).collect()
    }

    // --- the retained declarations ---------------------------------------------------------

    #[test]
    fn a_registry_retains_every_declaration_in_registration_order() {
        let registry = built(vec![
            feature("t", |b| {
                b.element(macro_with("t", "a", vec![ArgDef::mandatory("x")]).docs("The a."))?;
                b.element(macro_with("t", "b", Vec::new()))?;
                b.element(ElementDef::new(
                    "t",
                    ["list"],
                    SyntaxDef::Environment { args: Vec::new(), body: BodyDef::new().scoped("t.items") },
                    element("t.list"),
                ))?;
                b.scoped_package(ScopedPackageDef {
                    owner: "t".into(),
                    name: Name::from("t.items"),
                    elements: vec![macro_with("t", "item", Vec::new())],
                    docs: String::from("The items."),
                })?;
                b.fallback(FallbackDef {
                    owner: "t".into(),
                    modes: vec![Mode::Math],
                    kind: FallbackKind::Macro,
                    element: element("t.any"),
                })
            }),
            feature("u", |b| {
                b.element(macro_with("u", "a", Vec::new()).modes([Mode::Math]))?;
                b.override_callable(CallableKind::Macro, "b")
            }),
        ]);
        let declarations = registry.declarations();
        let written: Vec<(&str, &str)> =
            declarations.elements().iter().map(|def| (def.owner.as_str(), def.names[0].as_str())).collect();
        assert_eq!(written, [("t", "a"), ("t", "b"), ("t", "list"), ("u", "a")]);
        assert_eq!(declarations.scoped_packages().len(), 1);
        assert_eq!(declarations.fallbacks().len(), 1);
        assert_eq!(declarations.overrides().len(), 1);
        assert_eq!(declarations.elements_of("t").count(), 3);
        assert_eq!(declarations.elements_of("u").count(), 1);
        assert_eq!(declarations.scoped_packages_of("t").count(), 1);

        let a = declarations.named(CallableKind::Macro, "a");
        let owners: Vec<&str> = a.iter().map(|def| def.owner.as_str()).collect();
        assert_eq!(owners, ["t", "u"]);
        #[cfg(feature = "docs")]
        assert_eq!(a[0].docs, "The a.");
        assert_eq!(arg_names(a[0]), [Some("x")]);
        assert!(declarations.named(CallableKind::Environment, "a").is_empty());
        assert!(declarations.named(CallableKind::Macro, "item").is_empty());

        let items = declarations.scoped_package("t.items").expect("declared");
        assert_eq!(items.elements[0].names[0], "item");
        let uses = declarations.scoped_package_uses("t.items");
        assert_eq!(uses.len(), 1);
        assert!(matches!(uses[0], super::ScopedPackageUse::Body { within: None, .. }));
        assert_eq!(uses[0].declaration().names[0], "list");
    }

    /// A finalizer that declares the macro `\x` for its feature.
    struct LateX;

    impl Finalizer for LateX {
        fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.element(macro_with("a", "x", Vec::new()))
        }
    }

    #[test]
    fn a_finalizer_s_declarations_join_its_feature_s_and_the_last_one_named_is_in_force() {
        // `a` declares `\x` from its finalizer, after `b` declared `\x` (with an
        // override) in the first round; the language stacks `b`'s definitions
        // above all of `a`'s, so `b`'s `\x` is in force.
        let registry = built(vec![
            feature("a", |b| {
                b.element(macro_with("a", "y", Vec::new()))?;
                b.finalizer(Arc::new(LateX))
            }),
            feature("b", |b| {
                b.element(macro_with("b", "x", Vec::new()))?;
                b.override_callable(CallableKind::Macro, "x")
            }),
        ]);
        let declarations = registry.declarations();
        let written: Vec<(&str, &str)> =
            declarations.elements().iter().map(|def| (def.owner.as_str(), def.names[0].as_str())).collect();
        assert_eq!(written, [("a", "y"), ("a", "x"), ("b", "x")]);
        let x = declarations.named(CallableKind::Macro, "x");
        let owners: Vec<&str> = x.iter().map(|def| def.owner.as_str()).collect();
        assert_eq!(owners, ["a", "b"]);
        let symbol = Symbol { kind: CallableKind::Macro, name: String::from("x") };
        let in_force = registry.lang().symbols().get(&symbol).expect("`\\x` is defined");
        assert_eq!(x.last().map(|def| &def.owner), Some(in_force));
    }

    #[test]
    fn a_resolved_peer_element_value_is_kept_and_an_unresolved_one_is_not() {
        let registry = built(vec![
            feature("t", |b| b.element(macro_with("t", "own", Vec::new()))),
            feature("u", |b| {
                b.peer_element_value(ElementDef::new("u", ["also"], SyntaxDef::Macro { args: Vec::new() }, element("t.own")))?;
                b.peer_element_value(ElementDef::new("u", ["none"], SyntaxDef::Macro { args: Vec::new() }, element("v.none")))
            }),
        ]);
        let names: Vec<&str> =
            registry.declarations().elements().iter().map(|def| def.names[0].as_str()).collect();
        assert_eq!(names, ["own", "also"]);
    }

    // --- argument roles ---------------------------------------------------------------------

    #[test]
    fn a_declared_role_is_listed_and_found_on_its_arguments() {
        let registry = built(vec![feature("t", |b| {
            b.argument_role("t.key", "A key.")?;
            b.element(macro_with("t", "k", vec![ArgDef::mandatory("key").role("t.key")]))
        })]);
        let role = registry.argument_role_named("t.key").expect("declared");
        assert_eq!(role.owner, "t");
        #[cfg(feature = "docs")]
        assert_eq!(role.docs, "A key.");
        assert_eq!(registry.argument_roles().len(), 1);
        let uses = registry.declarations().role_uses("t.key");
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].arg.name.as_deref(), Some("key"));
        assert_eq!(uses[0].declaration.names[0], "k");
        assert!(registry.argument_role_named("t.other").is_none());
    }

    #[test]
    fn a_role_no_feature_declares_is_an_error_naming_the_feature_command_argument_and_role() {
        let error = refused(vec![feature("t", |b| {
            b.argument_role("t.key", "A key.")?;
            let field = TackOnField::new("tag", vec![ArgDef::mandatory("name").role("t.tag")]);
            b.element(macro_with(
                "t",
                "k",
                vec![ArgDef::new(ArgStdKind::Mandatory).role("t.kye"), ArgDef::new(ArgStdKind::TackOn { fields: vec![field] })],
            ))
        })]);
        let messages: Vec<String> = error.errors.iter().map(ToString::to_string).collect();
        assert_eq!(
            messages,
            [
                "feature `t`: argument #1 of `\\k` has the argument role `t.kye`, which no \
                 registered feature declares; a feature that sets another feature's argument role \
                 must declare it with `FeatureBuilder::peer_argument_role`; declared argument role \
                 names: [t.key]",
                "feature `t`: the argument `name` of the field `\\tag` of `\\k` has the argument \
                 role `t.tag`, which no registered feature declares; a feature that sets another \
                 feature's argument role must declare it with `FeatureBuilder::peer_argument_role`; \
                 declared argument role names: [t.key]",
            ]
        );
        match &error.errors[0] {
            BuildError::UndeclaredArgumentRole { feature, place, role, roster } => {
                assert_eq!((feature.as_str(), place.command.as_str(), role.as_str()), ("t", "\\k", "t.kye"));
                assert_eq!(roster, &[Name::from("t.key")]);
                assert_eq!((place.argument.as_deref(), place.index, place.field.as_deref()), (None, 0, None));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn another_feature_s_role_needs_no_peer_declaration_when_that_feature_is_present() {
        let registry = built(vec![
            feature("t", |b| b.argument_role("t.key", "A key.")),
            feature("u", |b| b.element(macro_with("u", "k", vec![ArgDef::mandatory("key").role("t.key")]))),
        ]);
        assert_eq!(registry.declarations().role_uses("t.key").len(), 1);
        assert!(registry.peer_argument_roles().is_empty());
    }

    #[test]
    fn a_peer_declared_role_resolves_and_an_unresolved_one_is_a_note() {
        let user = || {
            feature("u", |b| {
                b.peer_argument_role("t.key")?;
                b.peer_argument_role("t.key")?;
                b.element(macro_with("u", "k", vec![ArgDef::mandatory("key").role("t.key")]))
            })
        };
        let resolved = built(vec![feature("t", |b| b.argument_role("t.key", "A key.")), user()]);
        assert_eq!(resolved.peer_argument_roles().len(), 1);
        assert!(resolved.peer_argument_roles()[0].resolved);
        assert!(notes(&resolved).is_empty(), "{:?}", notes(&resolved));

        let alone = built(vec![user()]);
        assert_eq!(alone.peer_argument_roles().len(), 1);
        assert!(!alone.peer_argument_roles()[0].resolved);
        assert_eq!(
            notes(&alone),
            ["feature `u` refers to the argument role `t.key` and expects another feature to \
              declare it, but no feature of this registry declares it"]
        );
        let note = alone.build_diagnostics().iter().next().expect("one note");
        let data = note.data().downcast_ref::<UnresolvedPeer>().expect("an unresolved peer");
        assert_eq!(data.kind, "argument role");
        assert_eq!(alone.declarations().role_uses("t.key").len(), 1);
    }

    #[test]
    fn two_declarations_of_one_role_are_an_error_naming_both_features() {
        let error = refused(vec![
            feature("t", |b| b.argument_role("x.key", "A key.")),
            feature("u", |b| b.argument_role("x.key", "Another key.")),
        ]);
        let messages: Vec<String> = error.errors.iter().map(ToString::to_string).collect();
        assert_eq!(
            messages,
            ["argument role `x.key` is declared twice: first by feature `t`, then by feature `u`"]
        );
    }

    #[test]
    fn a_role_is_not_an_input_of_the_digest() {
        let with = |role: bool| {
            built(vec![feature("t", move |b| {
                b.argument_role("t.key", "A key.")?;
                let mut arg = ArgDef::mandatory("key");
                if role {
                    arg = arg.role("t.key");
                }
                b.element(macro_with("t", "k", vec![arg]))
            })])
            .digest()
        };
        assert_eq!(with(false), with(true));
        let without_declaration =
            built(vec![feature("t", |b| b.element(macro_with("t", "k", vec![ArgDef::mandatory("key")])))]).digest();
        assert_eq!(with(false), without_declaration);
    }
}
