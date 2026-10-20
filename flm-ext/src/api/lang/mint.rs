//! Mirrors [`flm_core::lang::mint`]: the declarations of elements and
//! fallbacks, and the argument and body definitions they carry.

use std::fmt;
use std::sync::Arc;

use flm_core::lang::rewrites::userdefs::UserDefinition as CoreUserDefinition;

use crate::api::lang::rewrites::userdefs::Expand;
use crate::api::model::element::ElementSpec;
use crate::runtime::ExtError;

pub use flm_core::lang::mint::{ArgDef, ArgLayout, ArgStdKind, BodyDef, BodyLayout, CallableKind, FallbackKind};
pub use flm_core::techy::latexlike::Mode;

/// Mirrors [`ElementDef`](flm_core::lang::mint::ElementDef): an element and the
/// syntax that produces it, as a feature declares it.
///
/// What differs: the element is the extension API's [`ElementSpec`]; the owner
/// is overwritten by the builder, as `flm-core` does. A value made by a helper
/// ([`UserDefinition::element_def`](crate::api::lang::rewrites::userdefs::UserDefinition::element_def)
/// or [`element_def_with`](crate::api::lang::rewrites::userdefs::UserDefinition::element_def_with))
/// holds what the helper was given, and the `flm-core` declaration is made when
/// [`FeatureBuilder::element`](crate::api::core::registry::FeatureBuilder::element)
/// registers it; the accessors answer `None` on such a value.
///
/// As in `flm-core`, a declaration is available in text mode unless it lists
/// its parsing modes with [`ElementDef::modes`]: a feature that defines a
/// command of mathematics lists `[Mode::Math]`, and one that defines a command
/// for both modes lists both. A declaration a helper made takes the modes of
/// its user definition unless [`ElementDef::modes`] replaces them.
#[derive(Clone)]
pub struct ElementDef {
    pub(crate) form: ElementDefForm,
}

/// The two forms of an [`ElementDef`]: built from data, or made by a helper.
#[derive(Clone)]
pub(crate) enum ElementDefForm {
    /// Built from data with [`ElementDef::new`].
    Data {
        owner: String,
        names: Vec<String>,
        syntax: SyntaxDef,
        element: ElementSpec,
        docs: String,
        /// `[Mode::Text]` unless [`ElementDef::modes`] sets others.
        modes: Vec<Mode>,
    },
    /// Made by a user definition's helper: what the helper was given, from
    /// which the registration makes the `flm-core` declaration
    /// (`UserDefinition::element_def`, or `element_def_with` when a hook is
    /// given), then sets the docs and modes given here, if any.
    Helper {
        definition: Arc<CoreUserDefinition>,
        owner: String,
        element_name: String,
        hook: Option<Arc<dyn Expand>>,
        docs: Option<String>,
        modes: Option<Vec<Mode>>,
    },
}

impl ElementDef {
    /// Mirrors [`ElementDef::new`](flm_core::lang::mint::ElementDef::new): the
    /// declaration is available in text mode only until [`ElementDef::modes`]
    /// says otherwise. What differs: names are strings, the element is the
    /// extension API's.
    pub fn new(
        owner: impl Into<String>,
        names: impl IntoIterator<Item = impl Into<String>>,
        syntax: SyntaxDef,
        element: ElementSpec,
    ) -> ElementDef {
        ElementDef {
            form: ElementDefForm::Data {
                owner: owner.into(),
                names: names.into_iter().map(Into::into).collect(),
                syntax,
                element,
                docs: String::new(),
                modes: vec![Mode::Text],
            },
        }
    }

    /// The declaration a user definition's helper answers: what the helper
    /// was given.
    pub(crate) fn from_helper(
        definition: Arc<CoreUserDefinition>,
        owner: &str,
        element_name: &str,
        hook: Option<Arc<dyn Expand>>,
    ) -> ElementDef {
        ElementDef {
            form: ElementDefForm::Helper {
                definition,
                owner: owner.to_string(),
                element_name: element_name.to_string(),
                hook,
                docs: None,
                modes: None,
            },
        }
    }

    /// Mirrors [`ElementDef::docs`](flm_core::lang::mint::ElementDef::docs).
    pub fn docs(mut self, text: impl Into<String>) -> ElementDef {
        match &mut self.form {
            ElementDefForm::Data { docs, .. } => *docs = text.into(),
            ElementDefForm::Helper { docs, .. } => *docs = Some(text.into()),
        }
        self
    }

    /// Mirrors [`ElementDef::modes`](flm_core::lang::mint::ElementDef::modes):
    /// sets the parsing modes the declaration is available in, replacing the
    /// default, text mode (on a declaration a helper made, the modes of its
    /// user definition).
    pub fn modes(mut self, list: impl IntoIterator<Item = Mode>) -> ElementDef {
        let list: Vec<Mode> = list.into_iter().collect();
        match &mut self.form {
            ElementDefForm::Data { modes, .. } => *modes = list,
            ElementDefForm::Helper { modes, .. } => *modes = Some(list),
        }
        self
    }

    /// No flm-core counterpart: reads the field `owner`, which is public in
    /// `flm-core`; `None` on a declaration a helper made.
    pub fn owner(&self) -> Option<&str> {
        match &self.form {
            ElementDefForm::Data { owner, .. } => Some(owner),
            ElementDefForm::Helper { .. } => None,
        }
    }

    /// No flm-core counterpart: reads the field `names`, which is public in
    /// `flm-core`; `None` on a declaration a helper made.
    pub fn names(&self) -> Option<&[String]> {
        match &self.form {
            ElementDefForm::Data { names, .. } => Some(names),
            ElementDefForm::Helper { .. } => None,
        }
    }

    /// No flm-core counterpart: reads the field `syntax`, which is public in
    /// `flm-core`; `None` on a declaration a helper made.
    pub fn syntax(&self) -> Option<&SyntaxDef> {
        match &self.form {
            ElementDefForm::Data { syntax, .. } => Some(syntax),
            ElementDefForm::Helper { .. } => None,
        }
    }

    /// No flm-core counterpart: reads the field `element`, which is public in
    /// `flm-core`; `None` on a declaration a helper made.
    pub fn element(&self) -> Option<&ElementSpec> {
        match &self.form {
            ElementDefForm::Data { element, .. } => Some(element),
            ElementDefForm::Helper { .. } => None,
        }
    }
}

impl fmt::Debug for ElementDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.form {
            ElementDefForm::Data { owner, names, syntax, element, docs, modes } => f
                .debug_struct("ElementDef")
                .field("owner", owner)
                .field("names", names)
                .field("syntax", syntax)
                .field("element", element)
                .field("docs", docs)
                .field("modes", modes)
                .finish(),
            ElementDefForm::Helper { definition, owner, element_name, hook, docs, modes } => f
                .debug_struct("ElementDef")
                .field("definition", &definition.definition().display_name())
                .field("owner", owner)
                .field("element_name", element_name)
                .field("hook", &hook.as_ref().map(|_| "…"))
                .field("docs", docs)
                .field("modes", modes)
                .finish(),
        }
    }
}

/// Mirrors [`SyntaxDef`](flm_core::lang::mint::SyntaxDef): how an element is
/// written. What differs: `Inclusion` is not offered (an inclusion's parser is
/// the host's concern).
#[derive(Debug, Clone)]
pub enum SyntaxDef {
    /// A macro and its arguments.
    Macro {
        /// The arguments, in order.
        args: Vec<ArgDef>,
    },
    /// An environment, its arguments and its body.
    Environment {
        /// The arguments, in order.
        args: Vec<ArgDef>,
        /// The body.
        body: BodyDef,
    },
    /// A specials sequence and its arguments.
    Specials {
        /// The characters that make the sequence.
        trigger: String,
        /// The arguments, in order.
        args: Vec<ArgDef>,
    },
    /// No syntax: the element is produced by a rewrite.
    Synthesized,
}

/// Mirrors [`FallbackDef`](flm_core::lang::mint::FallbackDef): the element an
/// unknown macro or environment becomes. What differs: the element is the
/// extension API's [`ElementSpec`]; the owner is the builder's feature.
#[derive(Debug, Clone)]
pub struct FallbackDef {
    /// The modes in which the fallback answers.
    pub modes: Vec<Mode>,
    /// Whether it answers unknown macros or unknown environments.
    pub kind: FallbackKind,
    /// The element the unknown callable becomes.
    pub element: ElementSpec,
}

impl FallbackDef {
    /// No flm-core counterpart: a fallback from its three fields, which
    /// `flm-core` builds as a struct literal.
    pub fn new(modes: impl IntoIterator<Item = Mode>, kind: FallbackKind, element: ElementSpec) -> FallbackDef {
        FallbackDef { modes: modes.into_iter().collect(), kind, element }
    }
}

/// Mirrors [`arg_def_from_code`](flm_core::lang::mint::arg_def_from_code): the
/// argument an argument code stands for (`{`, `[`, `*`, …). What differs: an
/// [`ArgCodeError`](flm_core::lang::mint::ArgCodeError) is [`ExtError::Core`].
///
/// # Errors
///
/// `code` is not an argument code ([`ExtError::Core`] of kind `arg-code`).
pub fn arg_def_from_code(code: &str) -> Result<ArgDef, ExtError> {
    flm_core::lang::mint::arg_def_from_code(code)
        .map_err(|error| ExtError::Core { kind: "arg-code".to_string(), detail: error.to_string(), data: serde_json::Value::Null })
}

/// Mirrors [`ScopedPackageDef`](flm_core::lang::mint::ScopedPackageDef): a
/// scoped package, a set of elements whose commands are legal only inside
/// the arguments and bodies that name the package, which
/// [`FeatureBuilder::scoped_package`](crate::api::core::registry::FeatureBuilder::scoped_package)
/// declares.
///
/// What differs: there is no `owner` field (the builder sets it to the
/// feature that declares the package, as `flm-core` does); the elements are
/// the extension API's [`ElementDef`]s, so a declaration a helper made may be
/// one of them.
#[derive(Debug, Clone)]
pub struct ScopedPackageDef {
    /// The package's name, which a body or an argument names to make the
    /// package's commands legal inside it.
    pub name: String,
    /// The elements the package declares.
    pub elements: Vec<ElementDef>,
    /// Documentation prose about the package.
    pub docs: String,
}

impl ScopedPackageDef {
    /// No flm-core counterpart (`flm-core` builds the struct from its public
    /// fields): the package `name` declaring `elements`, documented by
    /// `docs`.
    pub fn new(name: impl Into<String>, elements: Vec<ElementDef>, docs: impl Into<String>) -> ScopedPackageDef {
        ScopedPackageDef { name: name.into(), elements, docs: docs.into() }
    }
}
