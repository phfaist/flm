//! User definitions: the data a caller declares ([`Definition`]), its
//! compilation against a built language ([`compile`], [`CompiledDefinition`]),
//! and the data of the definition's element, which expands its use sites
//! ([`UserDefinition`], the standard implementation of the [`Expand`] aspect).
//!
//! A [user definition](super) is a macro, environment or special character
//! sequence defined by substitution. It enters the registry as an ordinary
//! element declaration ([`UserDefinition::element_def`]) whose element carries
//! the `Expand` aspect and the declared block role, and which is available in
//! the parsing modes the definition is defined for ([`DefinitionModes`]) and in
//! no other.
//!
//! The templates are compiled *after* the language is built, because a template
//! may use any element the registry declares, other user definitions included.
//! The data of the definition's element ([`UserDefinition`]) therefore holds a
//! field for its compiled template that is filled once, when the
//! registry is built: the `userdefs` feature's late-initialization build hook
//! ([`LateInit`](crate::core::registry::LateInit)) calls
//! [`UserDefinition::compile_with`] for each of its definitions, over one
//! [`TemplateLanguage`] built for all of them. [`UserDefinition::compile`] does
//! the same over a template language built for that one call.
//!
//! Compilation returns the warnings of the template parse to the caller, which
//! reports them with the registry's build diagnostics; an error refuses the
//! definition. A use site of a refused definition, and a use site in a parsing
//! mode where the definition's template did not parse, are each replaced by
//! their literal source text and reported with `flm.userdefs.bad-definition`
//! ([`BadDefinition`]).
//!
//! # Rules of a definition
//!
//! - **One template text, for the modes the definition is defined for.** A
//!   definition carries one template text ([`Definition::content`]) and the
//!   parsing modes it is defined for ([`Definition::modes`]; both text and
//!   mathematics mode by default). The text is compiled once per declared mode,
//!   because a template is parsed in the mode where it is substituted; both
//!   compilations come from the one declared text. A definition declared for
//!   both modes ([`DefinitionModes::Both`], the default) applies in each mode
//!   **where its text parses**: a template that holds a command available only in formulas,
//!   such as `\lvert{#1}\rangle`, applies in formulas, and a use of it in
//!   running text is an Error reported at that use. Such a definition is refused
//!   when the registry is built only when its text parses in neither mode. A
//!   definition declared for one mode only must parse in that mode, or it is
//!   refused when the registry is built. A name whose two modes need two
//!   different templates is declared as two definitions, which is allowed
//!   because their modes do not overlap.
//! - **Default argument values** are keyed by the argument's declared name or
//!   by the number a template writes the argument as (`"1"` for `#1`, the
//!   argument number offset applied). They are compiled once per declared mode,
//!   in that mode, so that a default value's nodes carry the mode they are
//!   substituted in. Default values that refer to each other in a cycle are
//!   refused when the registry is built.
//! - **One numbering everywhere.** The argument number offset shifts a numbered
//!   default key exactly as it shifts a numbered placeholder, so `#1` and the
//!   default value keyed `"1"` always refer to the same argument. A definition
//!   that declares no offset, the common case, is unaffected, since the two
//!   numberings then coincide.
//! - **An environment's body is laid out according to its block role**
//!   ([`BlockRole`]): as blocks when the role is [`BlockRole::Block`], as inline
//!   content otherwise.
//! - **A template uses only the elements available at the top level of a
//!   document**: it is parsed in the language's initial parsing state, so what
//!   it means does not depend on where it is used.
//! - **A placeholder is replaced by the argument's content nodes, and nothing
//!   else**, regardless of the kind of argument declared: an ordinary argument,
//!   a verbatim argument and a key-value argument alike. The delimiters or the
//!   group a use site wrote around the argument are syntax of the call, not part
//!   of its content, so a verbatim argument written `\code{x%y}` substitutes the
//!   characters `x%y`, and a template `see #1` shows them as text. The
//!   substitution does not depend on the argument's kind, so an argument kind
//!   added later needs no change here. When a placeholder is written inside
//!   verbatim content of the template, as in `\verbcode{#1}`, the verbatim
//!   element reads what is substituted there; content there that is not
//!   characters is reported by that element as an Error, not by the
//!   substitution.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, OnceLock};

use techy::core::node::{NodeId, NodeRef, NodeSlice, NodeTree};
use techy::error::{Diagnostic, DiagnosticValue, Severity, ToDiagnosticValue};
use techy::latexlike::Mode;
use techy::source::{Source, SourceSpan};

use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::ElementSpec;
use crate::model::names::{ElementName, FeatureName, Name};
use crate::lang::build::BuiltLang;
use crate::lang::flm::Flm;
use crate::lang::guard::GuardPolicy;
use crate::lang::mint::{
    ArgDef, BodyDef, BodyLayout, CallableKind, ElementDef, OverrideDef, SyntaxDef,
};
use crate::lang::origin::{FlmDiagnostic, FlmOrigin, FlmSource};
use crate::lang::rewrites::userdefs::diagnostics::{
    BadDefinition, BadPlaceholder, DefinitionProblem, PlaceholderProblem,
};
use crate::lang::rewrites::userdefs::hook::{
    Expand, ExpandCx, ExpandError, ExpandPolicy, Expander, Expansion, PlaceholderBindings,
    PlaceholderValue,
};
use crate::lang::rewrites::userdefs::instantiate::instantiate;
use crate::lang::rewrites::userdefs::pass::DEFAULT_RECURSION_LIMIT;
use crate::lang::rewrites::userdefs::template::{
    compile_in_modes, compile_template_with, minting_diagnostics, CompiledTemplate,
    ExtraPlaceholder, PlaceholderTarget, PlaceholderVocabulary, TemplateLanguage, TemplateSpec,
};

/// The kind of command a definition declares: a macro, an environment or a
/// special character sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefinitionKind {
    /// A macro, written `\name` followed by its arguments.
    Macro,
    /// An environment, written `\begin{name}…\end{name}`; its template refers to
    /// the environment's body as `#{body}`.
    Environment,
    /// A special character sequence: a sequence of characters, written without a
    /// backslash, that triggers the definition.
    Specials,
}

impl DefinitionKind {
    /// The kind's lowercase token, `macro`, `environment` or `specials`: the
    /// segment of the definition's element name that says its kind
    /// ([`element_name`](crate::stdcore::features::userdefs::element_name)).
    pub fn token(self) -> &'static str {
        match self {
            DefinitionKind::Macro => "macro",
            DefinitionKind::Environment => "environment",
            DefinitionKind::Specials => "specials",
        }
    }
}

/// The parsing modes a definition is defined for: text mode, mathematics mode,
/// or both.
///
/// A definition has one template text, which is its template in every mode
/// listed here, and the definition's element is declared available in those
/// modes alone. In any other mode the name is not defined: a use site there is
/// an unknown command, reported the way the language reports every name that no
/// feature defines. With [`Both`](DefinitionModes::Both), the default, the
/// definition applies in each of the two modes where its template text parses;
/// a use in a mode where the text does not parse is an Error at that use
/// ([`DefinitionProblem::NotInForceInMode`]).
///
/// Two definitions of one name whose modes do not overlap, for example a `\vec`
/// for text mode and a `\vec` for mathematics mode, may be declared side by
/// side, and each applies in its own mode. Two whose modes overlap may not: only
/// one of them could apply where they overlap, so registering the
/// [`userdefs` feature](crate::stdcore::features::userdefs) fails, and with it
/// the registry build, with the error "two definitions of `vec` apply in the
/// same parsing mode".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DefinitionModes {
    /// Text mode only.
    Text,
    /// Mathematics mode only.
    Math,
    /// Text and mathematics mode, in each mode where the template text parses. This
    /// is the default.
    #[default]
    Both,
}

impl DefinitionModes {
    /// The parsing modes listed, in a fixed order: text mode first.
    pub fn modes(self) -> &'static [Mode] {
        match self {
            DefinitionModes::Text => &[Mode::Text],
            DefinitionModes::Math => &[Mode::Math],
            DefinitionModes::Both => &[Mode::Text, Mode::Math],
        }
    }

    /// Whether `mode` is one of the parsing modes listed.
    pub fn includes(self, mode: Mode) -> bool {
        self.modes().contains(&mode)
    }

    /// Whether some parsing mode is listed both in `self` and in `other`. Two
    /// definitions of one name and kind may be declared together exactly when their
    /// modes do not overlap.
    pub fn overlaps(self, other: DefinitionModes) -> bool {
        self.modes().iter().any(|mode| other.includes(*mode))
    }

    /// The stable token for the value: `text`, `math` or `both`. The canonical
    /// rendering of the `userdefs` feature's configuration
    /// ([`UserdefsConfig::digest_bytes`](crate::stdcore::features::userdefs::UserdefsConfig::digest_bytes))
    /// writes it, so it does not change when a variant is renamed.
    pub fn token(self) -> &'static str {
        match self {
            DefinitionModes::Text => "text",
            DefinitionModes::Math => "math",
            DefinitionModes::Both => "both",
        }
    }
}

/// A user definition is a macro, environment or special character sequence
/// that a document's author or the host defines by substitution (a name, an
/// argument syntax, a template and the parsing modes it is defined for); it
/// enters the registry as an element carrying the [`Expand`] aspect.
///
/// This type holds the definition as declared: the name, the kind of command,
/// the argument syntax, the template text and the parsing modes it is defined
/// for, the default argument values, the argument number offset, the block
/// role, the expansion policy, and whether it explicitly replaces another
/// feature's definition of the same command.
///
/// A definition is created with [`Definition::macro_`],
/// [`Definition::environment`] or [`Definition::specials`] and completed with
/// the builder methods, such as [`text`](Definition::text),
/// [`math`](Definition::math) and [`default`](Definition::default). The
/// [`userdefs` feature](crate::stdcore::features::userdefs) registers the
/// definitions of its configuration
/// ([`UserdefsConfig`](crate::stdcore::features::userdefs::UserdefsConfig)); an
/// extension that registers definitions of its own wraps each in a
/// [`UserDefinition`].
///
/// The rules a declaration must satisfy, and what each field means for the
/// compiled templates, are in [the module documentation](self).
#[derive(Debug, Clone)]
pub struct Definition {
    /// The command's name, without the backslash, or the character sequence of a
    /// special character sequence.
    pub name: Name,
    /// The kind of command declared.
    pub kind: DefinitionKind,
    /// The arguments, in the order they are written, each declared as the language
    /// layer declares an argument ([`ArgDef`]).
    pub args: Vec<ArgDef>,
    /// The template text: FLM text with placeholders, which replaces every use
    /// site. `None` when no template is declared, which refuses the definition when
    /// its templates are compiled; a definition meant to expand to nothing declares
    /// an empty template instead.
    pub content: Option<String>,
    /// The parsing modes the definition is defined for: both text and mathematics
    /// mode unless declared otherwise.
    pub modes: DefinitionModes,
    /// The default argument values, as template text. Each is keyed by the
    /// argument's declared name or by the number a template writes the argument as,
    /// written as text (`"1"`), the argument number offset applied.
    pub defaults: BTreeMap<String, String>,
    /// The argument number offset: how many leading declared arguments the numbered
    /// placeholders skip, so that `#1` is the argument at this index. Zero unless
    /// declared.
    pub argument_number_offset: usize,
    /// The block role of the definition's element ([`BlockRole`]): whether a use
    /// site is a block or inline content, and, for an environment, whether its body
    /// is laid out as blocks.
    pub block_role: BlockRole,
    /// When the use sites are expanded ([`ExpandPolicy`]).
    pub expand: ExpandPolicy,
    /// Whether this definition explicitly replaces another feature's definition of
    /// the same command (the language layer's override declaration,
    /// [`OverrideDef`]).
    pub overrides: bool,
    /// The documentation of the definition, which its element and its element
    /// declaration carry.
    pub docs: String,
    /// The placeholder names, besides the arguments and the body, whose content an
    /// expansion hook computes at each use: what the templates may write as
    /// `#{name}` or `#[…]{name}`. Empty unless declared.
    ///
    /// Only an extension that attaches an [`Expand`] implementation of its own,
    /// whose placeholder bindings compute these names and pass every other
    /// placeholder to [`DefinitionBindings`], declares any. A definition that the
    /// `userdefs` feature registers computes none of them: where its template
    /// refers to one, each use site fails with `flm.userdefs.bad-placeholder`.
    pub extra_placeholders: Vec<ExtraPlaceholder>,
}

impl Definition {
    /// A definition of the kind `kind`, named `name`, with the arguments `args` and
    /// no template yet. The other fields take their defaults: both parsing modes, no
    /// default values, no argument number offset, the default block role
    /// ([`BlockRole::DEFAULT`]), expansion per plan, no override, no documentation
    /// and no extra placeholder names.
    pub fn new(kind: DefinitionKind, name: impl Into<Name>, args: Vec<ArgDef>) -> Definition {
        Definition {
            name: name.into(),
            kind,
            args,
            content: None,
            modes: DefinitionModes::default(),
            defaults: BTreeMap::new(),
            argument_number_offset: 0,
            block_role: BlockRole::DEFAULT,
            expand: ExpandPolicy::default(),
            overrides: false,
            docs: String::new(),
            extra_placeholders: Vec::new(),
        }
    }

    /// A macro definition, written `Definition::macro_("foo", args)`; the trailing
    /// underscore avoids Rust's reserved word `macro`.
    pub fn macro_(name: impl Into<Name>, args: Vec<ArgDef>) -> Definition {
        Definition::new(DefinitionKind::Macro, name, args)
    }

    /// An environment definition.
    pub fn environment(name: impl Into<Name>, args: Vec<ArgDef>) -> Definition {
        Definition::new(DefinitionKind::Environment, name, args)
    }

    /// A special character sequence definition: a use site is the characters
    /// `trigger`, written without a backslash, followed by the arguments.
    pub fn specials(trigger: impl Into<Name>, args: Vec<ArgDef>) -> Definition {
        Definition::new(DefinitionKind::Specials, trigger, args)
    }

    /// Builder: sets the template text and leaves the parsing modes unchanged (both
    /// text and mathematics mode, unless [`modes`](Definition::modes) was called).
    pub fn content(mut self, template: impl Into<String>) -> Definition {
        self.content = Some(template.into());
        self
    }

    /// Builder: sets the parsing modes the definition is defined for.
    pub fn modes(mut self, modes: DefinitionModes) -> Definition {
        self.modes = modes;
        self
    }

    /// Builder: sets the template text and defines the definition for text mode
    /// only. Shorthand for [`content`](Definition::content) followed by
    /// [`modes`](Definition::modes) with [`DefinitionModes::Text`].
    ///
    /// A definition has one template, so calling this after
    /// [`math`](Definition::math), or calling either of them twice, replaces both
    /// the text and the modes the earlier call set. To declare one name in two
    /// modes with two templates, declare two definitions.
    pub fn text(self, template: impl Into<String>) -> Definition {
        self.content(template).modes(DefinitionModes::Text)
    }

    /// Builder: sets the template text and defines the definition for mathematics
    /// mode only. Shorthand for [`content`](Definition::content) followed by
    /// [`modes`](Definition::modes) with [`DefinitionModes::Math`]; see
    /// [`text`](Definition::text) for what calling both does.
    pub fn math(self, template: impl Into<String>) -> Definition {
        self.content(template).modes(DefinitionModes::Math)
    }

    /// Builder: adds the default value `template` for the argument that `key`
    /// refers to: its declared name, or the number a template writes it as, written
    /// as text.
    pub fn default(mut self, key: impl Into<String>, template: impl Into<String>) -> Definition {
        self.defaults.insert(key.into(), template.into());
        self
    }

    /// Builder: sets the argument number offset, so that `#1` and the default value
    /// keyed `"1"` refer to the argument at index `offset`.
    pub fn argument_number_offset(mut self, offset: usize) -> Definition {
        self.argument_number_offset = offset;
        self
    }

    /// Builder: sets the block role.
    pub fn block_role(mut self, role: BlockRole) -> Definition {
        self.block_role = role;
        self
    }

    /// Builder: sets the expansion policy.
    pub fn expand(mut self, policy: ExpandPolicy) -> Definition {
        self.expand = policy;
        self
    }

    /// Builder: marks the definition as explicitly replacing another feature's
    /// definition of the same command.
    pub fn overriding(mut self) -> Definition {
        self.overrides = true;
        self
    }

    /// Builder: sets the documentation.
    pub fn docs(mut self, docs: impl Into<String>) -> Definition {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Builder: adds a placeholder name whose content an expansion hook computes
    /// ([`extra_placeholders`](Definition::extra_placeholders)). The templates may
    /// write it with a substitution argument (`#[…]{name}`) when `substitution` is
    /// `true`.
    pub fn extra_placeholder(mut self, name: impl Into<String>, substitution: bool) -> Definition {
        self.extra_placeholders.push(ExtraPlaceholder { name: name.into(), substitution });
        self
    }

    /// The number of declared arguments.
    pub fn arity(&self) -> usize {
        self.args.len()
    }

    /// The command as an author writes it: `\foo`, `\begin{foo}`, or the character
    /// sequence of a special character sequence.
    pub fn display_name(&self) -> String {
        match self.kind {
            DefinitionKind::Macro => format!("\\{}", self.name),
            DefinitionKind::Environment => format!("\\begin{{{}}}", self.name),
            DefinitionKind::Specials => self.name.to_string(),
        }
    }

    /// The kind of command the definition registers, as the language layer's
    /// [`CallableKind`].
    pub fn callable_kind(&self) -> CallableKind {
        match self.kind {
            DefinitionKind::Macro => CallableKind::Macro,
            DefinitionKind::Environment => CallableKind::Environment,
            DefinitionKind::Specials => CallableKind::Specials,
        }
    }

    /// The index of the argument that `key` refers to: the argument declared under
    /// the name `key`, else the argument a template writes as `#key`, where `"1"` is
    /// the argument at the definition's argument number offset (the rule *one
    /// numbering everywhere* of the module documentation). `None` when no argument
    /// matches.
    pub fn argument_index(&self, key: &str) -> Option<usize> {
        if let Some(index) = self.args.iter().position(|arg| arg.name.as_deref() == Some(key)) {
            return Some(index);
        }
        let number: usize = key.parse().ok()?;
        let index = number.checked_sub(1)?.checked_add(self.argument_number_offset)?;
        (number >= 1 && index < self.args.len()).then_some(index)
    }

    /// What the templates may refer to ([`PlaceholderVocabulary`]): the arguments by
    /// number and by name, the body for an environment, and the [extra placeholder
    /// names](Definition::extra_placeholders).
    pub fn vocabulary(&self) -> PlaceholderVocabulary {
        let mut vocabulary = PlaceholderVocabulary::new(self.args.len())
            .names(self.args.iter().map(|arg| arg.name.clone()).collect());
        if self.kind == DefinitionKind::Environment {
            vocabulary = vocabulary.with_body();
        }
        for extra in &self.extra_placeholders {
            vocabulary = vocabulary.extra(extra.name.clone(), extra.substitution);
        }
        vocabulary
    }

    /// The syntax declaration of the definition's element ([`SyntaxDef`]): its
    /// arguments, and for an environment a body laid out as blocks when the block
    /// role is [`BlockRole::Block`] and as inline content otherwise.
    pub fn syntax(&self) -> SyntaxDef {
        match self.kind {
            DefinitionKind::Macro => SyntaxDef::Macro { args: self.args.clone() },
            DefinitionKind::Environment => {
                let layout = match self.block_role {
                    BlockRole::Block => BodyLayout::Blocks,
                    _ => BodyLayout::Inline,
                };
                SyntaxDef::Environment {
                    args: self.args.clone(),
                    body: BodyDef::new().layout(layout),
                }
            }
            DefinitionKind::Specials => SyntaxDef::Specials {
                trigger: self.name.to_string(),
                args: self.args.clone(),
            },
        }
    }
}

/// The compiled templates of one parsing mode: the template and the default
/// values that apply in that mode. A [`CompiledDefinition`] holds one for each
/// mode in which the definition's text compiled ([`CompiledDefinition::variant`]).
pub struct ModeVariant {
    template: CompiledTemplate,
    defaults: BTreeMap<usize, CompiledTemplate>,
}

impl ModeVariant {
    /// The compiled template.
    pub fn template(&self) -> &CompiledTemplate {
        &self.template
    }

    /// The compiled default value of the argument at `index`, if one is declared.
    pub fn default_of(&self, index: usize) -> Option<&CompiledTemplate> {
        self.defaults.get(&index)
    }

    /// The compiled default values with their argument indices, in increasing order
    /// of index.
    pub fn defaults(&self) -> impl Iterator<Item = (usize, &CompiledTemplate)> + '_ {
        self.defaults.iter().map(|(index, template)| (*index, template))
    }

    /// The content that the use site `site` substitutes for the declared argument
    /// `index` when the use site left that argument out: this variant's default
    /// value for it, instantiated at the use site.
    ///
    /// Instantiating the default value replaces every placeholder in it by what
    /// `site` wrote (or by another argument's default value in turn), evaluates
    /// every conditional in it, and copies the result onto a source created for
    /// this use, so that the result can be traced back to both the declaration and
    /// the use site. This is the same instantiation that the expansion of a use
    /// site performs, with the same function and the same placeholder bindings
    /// ([`DefinitionBindings`]), so a caller that needs a default value's content
    /// outside an expansion gets exactly what the expansion would have put there.
    /// The kept-definition lowering rules of the `userdefs` feature
    /// ([`kept`](crate::stdcore::features::userdefs::kept)) use it to write an
    /// omitted argument's default value into the call they write in place of the
    /// use site.
    ///
    /// The result is *not* run through the [expand pass](super::expand): a use site
    /// of another definition inside the default value is left as it is, for the
    /// caller to handle like any other use site.
    ///
    /// `guard` is the policy of the [recursion guard](crate::lang::guard) that the
    /// two traversals run under, normally the built language's
    /// ([`crate::lang::BuiltLang::guard`]). Under [`GuardPolicy::StackBudget`] the
    /// caller must be on a [worker thread](crate::lang::guard::on_worker_thread).
    ///
    /// Returns `None` when this variant declares no default value for that
    /// argument; the use site then substitutes nothing there.
    ///
    /// # Errors
    ///
    /// [`ExpandError::BadPlaceholder`] when the default value holds a placeholder
    /// that the use site cannot give a value for: a name an expansion hook would
    /// compute, or a placeholder written with a substitution argument. Any other
    /// [`ExpandError`] when building the nodes fails or the recursion guard refuses.
    pub fn instantiate_default(
        &self,
        site: NodeRef<'_, Flm>,
        index: usize,
        guard: GuardPolicy,
    ) -> Option<Result<NodeTree<Flm>, ExpandError>> {
        let default = self.default_of(index)?;
        let bindings = DefinitionBindings::new(site, self);
        Some(instantiate(site.span(), default, &bindings, guard))
    }
}

impl fmt::Debug for ModeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModeVariant")
            .field("template", &self.template)
            .field("defaults", &self.defaults.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A definition's templates, compiled once when the registry is built: one
/// [`ModeVariant`] for each parsing mode in which the definition's text
/// compiled, and the warnings of the compilation.
///
/// [`compile`] and [`compile_with`] produce it, and [`UserDefinition::compiled`]
/// returns the one recorded in the data of a definition's element.
pub struct CompiledDefinition {
    name: Name,
    display: String,
    arity: usize,
    text: Option<ModeVariant>,
    math: Option<ModeVariant>,
    diagnostics: Vec<FlmDiagnostic>,
}

impl CompiledDefinition {
    /// The definition's name.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The command as an author writes it (`\foo`).
    pub fn display_name(&self) -> &str {
        &self.display
    }

    /// The number of declared arguments.
    pub fn arity(&self) -> usize {
        self.arity
    }

    /// The compiled templates for `mode`, when the definition's text compiled in that
    /// mode; `None` otherwise, including for a mode the definition is not defined
    /// for.
    pub fn variant(&self, mode: Mode) -> Option<&ModeVariant> {
        match mode {
            Mode::Text => self.text.as_ref(),
            Mode::Math => self.math.as_ref(),
            _ => None,
        }
    }

    /// The diagnostics of the compilation that are not errors: the warnings of the
    /// template parse, which the caller reports with the registry's build
    /// diagnostics.
    pub fn diagnostics(&self) -> &[FlmDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Debug for CompiledDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledDefinition")
            .field("name", &self.name)
            .field("text", &self.text)
            .field("math", &self.math)
            .finish()
    }
}

/// An empty source that represents the declaration of the definition `display`
/// itself. It is labeled `declaration of \foo`, so that a diagnostic reported
/// there says which definition it is about. A problem with the definition as a
/// whole, rather than with a template's text, is reported there.
fn declaration_source(display: &str) -> Arc<FlmSource> {
    let origin = FlmOrigin::synthetic(format!("declaration of {display}"));
    Arc::new(Source::new(String::new()).with_origin(origin))
}

/// Compiles one mode's templates: the template, then the default values, then
/// the check for a cycle among the default values. Errors go to `diagnostics`,
/// and so do the diagnostics of every template that compiled that are not
/// errors (the warnings of the template parse, which the caller reports with the
/// registry's build diagnostics). Returns `None` when there was any error.
fn compile_variant(
    definition: &Definition,
    lang: &BuiltLang,
    language: &TemplateLanguage,
    mode: Mode,
    text: &str,
    default_indices: &BTreeMap<usize, &str>,
    diagnostics: &mut Vec<FlmDiagnostic>,
) -> Option<ModeVariant> {
    let display = definition.display_name();
    let vocabulary = definition.vocabulary();
    let label = format!("expansion of {display}");
    let origin_label = format!("template of {display}");
    let spec = TemplateSpec {
        text,
        mode,
        vocabulary: &vocabulary,
        offset: definition.argument_number_offset,
        definition: &display,
        label: &label,
        origin_label: &origin_label,
        user_definition: true,
    };
    let mut failed = false;
    let template = match compile_template_with(&spec, lang, language) {
        Ok(template) => {
            diagnostics.extend(template.diagnostics().iter().cloned());
            Some(template)
        }
        Err(errors) => {
            diagnostics.extend(errors);
            failed = true;
            None
        }
    };
    let mut defaults = BTreeMap::new();
    for (index, default_text) in default_indices {
        // How the template writes the argument the default belongs to: `#n`
        // under the offset, or — for an argument the offset puts out of the
        // numbering's reach — its declared name, else its declared position.
        let label = match index.checked_sub(definition.argument_number_offset) {
            Some(number) => format!("default value of #{} of {display}", number + 1),
            None => {
                let named = definition.args[*index].name.clone();
                let what = named.unwrap_or_else(|| format!("argument {}", index + 1));
                format!("default value of {what} of {display}")
            }
        };
        let spec = TemplateSpec {
            text: default_text,
            mode,
            vocabulary: &vocabulary,
            offset: definition.argument_number_offset,
            definition: &display,
            label: &label,
            origin_label: &label,
            user_definition: true,
        };
        match compile_template_with(&spec, lang, language) {
            Ok(compiled) => {
                diagnostics.extend(compiled.diagnostics().iter().cloned());
                defaults.insert(*index, compiled);
            }
            Err(errors) => {
                diagnostics.extend(errors);
                failed = true;
            }
        }
    }
    if let Some((template, placeholder)) = default_cycle(&defaults) {
        let span = template
            .tree()
            .get(placeholder)
            .map(|node| node.span().clone())
            .unwrap_or_else(|| SourceSpan::new(template.source(), 0..0));
        let written = template
            .placeholder(placeholder)
            .map(|reference| reference.written.clone())
            .unwrap_or_default();
        diagnostics.push(Diagnostic::error(
            BadPlaceholder::new(display.clone(), written, PlaceholderProblem::DefaultCycle),
            span,
        ));
        failed = true;
    }
    match (failed, template) {
        (false, Some(template)) => Some(ModeVariant { template, defaults }),
        _ => None,
    }
}

/// A cycle among the default values: a default value whose placeholder refers to
/// an argument whose default value refers back to it, through any number of
/// default values. Returns the compiled default value and the placeholder node
/// that closes the cycle, if there is a cycle.
fn default_cycle(
    defaults: &BTreeMap<usize, CompiledTemplate>,
) -> Option<(&CompiledTemplate, NodeId)> {
    fn visit<'d>(
        index: usize,
        defaults: &'d BTreeMap<usize, CompiledTemplate>,
        open: &mut Vec<usize>,
        done: &mut BTreeSet<usize>,
    ) -> Option<(&'d CompiledTemplate, NodeId)> {
        if done.contains(&index) {
            return None;
        }
        let template = defaults.get(&index)?;
        open.push(index);
        for (node, reference) in template.placeholders() {
            let PlaceholderTarget::Argument(target) = &reference.target else {
                continue;
            };
            if !defaults.contains_key(target) {
                continue;
            }
            if open.contains(target) {
                return Some((template, node));
            }
            if let Some(cycle) = visit(*target, defaults, open, done) {
                return Some(cycle);
            }
        }
        open.pop();
        done.insert(index);
        None
    }
    let mut done = BTreeSet::new();
    for index in defaults.keys() {
        let mut open = Vec::new();
        if let Some(cycle) = visit(*index, defaults, &mut open, &mut done) {
            return Some(cycle);
        }
    }
    None
}

/// Compiles a definition's templates against the built language `lang`, over a
/// template language built for this call alone; [`compile_with`] shares one
/// template language across many definitions.
///
/// On success, the compiled definition keeps the diagnostics of the compilation
/// that are not errors (the warnings of the template parse), for the caller to
/// report with the registry's build diagnostics
/// ([`CompiledDefinition::diagnostics`]).
///
/// # Errors
///
/// Every diagnostic of the compilation, when any of them is an Error: the
/// template parse errors, the placeholder problems
/// (`flm.userdefs.bad-placeholder`) and the problems with the definition as a
/// whole (`flm.userdefs.bad-definition`). The definition cannot be used then.
/// When the template language itself cannot be built, the result holds that
/// failure as `flm.userdefs.bad-definition`: its declarations, which are
/// constants of the [`template`](super::template) module, failed
/// [minting](crate::lang::mint), the step that turns element declarations into
/// the techy packages a parse looks commands up in.
pub fn compile(
    definition: &Definition,
    lang: &BuiltLang,
) -> Result<CompiledDefinition, Vec<FlmDiagnostic>> {
    match TemplateLanguage::new(lang) {
        Ok(language) => compile_with(definition, lang, &language),
        Err(errors) => Err(minting_diagnostics(&definition.display_name(), &errors)),
    }
}

/// [`compile`] over a template language that is already built: one
/// [`TemplateLanguage::new`] for all the definitions of a registry, then one call
/// per definition; the `userdefs` feature calls it this way.
///
/// # Errors
///
/// The same as [`compile`], except the failure to build the template language,
/// which happens, or not, before this call.
pub fn compile_with(
    definition: &Definition,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledDefinition, Vec<FlmDiagnostic>> {
    let display = definition.display_name();
    let mut diagnostics: Vec<FlmDiagnostic> = Vec::new();
    let declaration = declaration_source(&display);
    let at_declaration = || SourceSpan::new(&declaration, 0..0);

    if definition.content.is_none() {
        diagnostics.push(Diagnostic::error(
            BadDefinition::new(display.clone(), DefinitionProblem::NoTemplate),
            at_declaration(),
        ));
    }
    let mut default_indices: BTreeMap<usize, &str> = BTreeMap::new();
    for (key, text) in &definition.defaults {
        match definition.argument_index(key) {
            Some(index) => {
                default_indices.insert(index, text);
            }
            None => diagnostics.push(Diagnostic::error(
                BadDefinition::new(
                    display.clone(),
                    DefinitionProblem::UnknownDefault { key: key.clone() },
                ),
                at_declaration(),
            )),
        }
    }

    // One template text, but one compilation per declared mode: a template is
    // parsed under the mode it is substituted in, so the same text yields one
    // compiled tree per mode. A definition declared by one text with no mode
    // named is in force in each mode where the text compiles: the mode where
    // it does not is no build problem, and a use there is reported at the
    // use — the rule `compile_in_modes` holds. See decision rationale at
    // [§flm-dd-dr:lang-userdef-modes].
    let declared_modes =
        [Mode::Text, Mode::Math].into_iter().filter(|&mode| definition.modes.includes(mode));
    let (variants, found) = match definition.content.as_deref() {
        Some(text) => compile_in_modes(declared_modes, |mode, found| {
            compile_variant(definition, lang, language, mode, text, &default_indices, found)
        }),
        None => (Vec::new(), Vec::new()),
    };
    diagnostics.extend(found);
    let (mut text, mut math) = (None, None);
    for (mode, variant) in variants {
        match mode {
            Mode::Text => text = Some(variant),
            _ => math = Some(variant),
        }
    }

    // A template cannot yet use what a tack-on argument absorbs, on any
    // backend: the absorbed run is whole macros, and a `\label` written into a
    // parameter is not a command a body can hold. Declaring such an argument
    // and leaving it unnamed is allowed; naming it — plainly, or inside a
    // conditional's test or branch, which are placeholders of the same tree —
    // is refused here, where every per-definition build problem is raised, so
    // that the refusal is the same on every backend rather than a property of
    // one output language. See decision rationale at
    // [§flm-dd-dr:std-userdefs-transcribable].
    for variant in [text.as_ref(), math.as_ref()].into_iter().flatten() {
        let trees =
            std::iter::once(variant.template()).chain(variant.defaults().map(|(_, tree)| tree));
        for tree in trees {
            for (node, reference) in tree.placeholders() {
                let PlaceholderTarget::Argument(index) = reference.target else { continue };
                if !definition.args.get(index).is_some_and(is_tack_on) {
                    continue;
                }
                let span = tree
                    .tree()
                    .get(node)
                    .map(|node| node.span().clone())
                    .unwrap_or_else(|| SourceSpan::new(tree.source(), 0..0));
                diagnostics.push(Diagnostic::error(
                    BadDefinition::new(
                        display.clone(),
                        DefinitionProblem::TackOnPlaceholder {
                            reference: reference.written.clone(),
                        },
                    ),
                    span,
                ));
            }
        }
    }

    if diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error) {
        return Err(diagnostics);
    }
    Ok(CompiledDefinition {
        name: definition.name.clone(),
        display,
        arity: definition.arity(),
        text,
        math,
        diagnostics,
    })
}

/// Whether `arg` is a **tack-on argument**: the trailing argument that absorbs
/// the field macros written *after* a call, such as the `\label{…}` of
/// `\thm{Euclid}\label{thm:euclid}`, rather than content between braces (see
/// [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)).
///
/// It is the only declared argument shape that a template cannot refer to: what
/// the call absorbed is whole macros, which have no meaning as the content of a
/// placeholder, so a placeholder of such an argument is refused when the
/// definition is compiled ([`DefinitionProblem::TackOnPlaceholder`]). A
/// definition may declare one and leave it unused in its templates; it then
/// contributes no effective argument ([`EffectiveSignature`]).
pub(crate) fn is_tack_on(arg: &ArgDef) -> bool {
    arg.kind.tack_on_fields().is_some()
}

/// The arguments that one call of a recomposed definition writes, in the order
/// it writes them: the definition's *effective signature*.
///
/// A backend *recomposes* a definition when it writes the definition once, as a
/// definition of its own output language, and writes each use site as a call of
/// it; the `latex-recomposed` backend writes LaTeX definitions this way. Such a
/// backend writes every call of the definition in one fixed shape: one group per
/// *effective argument*, and nothing else. There are two kinds of effective
/// argument, and this type records how many there are of each and which
/// declared argument each one belongs to:
///
/// - a **value**, one per declared argument that a template can refer to, in
///   declared order: what the expansion substitutes for a placeholder of that
///   argument, that is, the content the use site wrote, else the argument's
///   default value, else nothing;
/// - a **flag**, which says whether the use site *wrote* the argument, one per
///   declared argument for which a conditional of the template needs to know
///   that, in declared order, after all the values.
///
/// A flag exists only where a conditional needs it, so an ordinary definition's
/// effective signature is one value per declared argument and nothing more.
///
/// A **tack-on argument** (the trailing argument that absorbs the field macros
/// written after a call, such as `\label`) has neither. A template cannot refer
/// to it ([`DefinitionProblem::TackOnPlaceholder`]), so it has no value, and no
/// conditional can test it, so it has no flag. A definition that declares one
/// and leaves it unused is still written, and nothing that its calls absorb is
/// written anywhere. A tack-on argument is the command's last argument, so the
/// values are the declared arguments before it.
///
/// The recomposing backend decides which arguments carry a flag, from the
/// conditionals the template uses; this type only records the outcome, so that
/// the calls and the definition agree on the position of each group.
///
/// The signature is computed once, when the registry is built, together with the
/// recomposability outcome ([`NotRecomposable`]), and is read from
/// [`UserDefinition::effective_signature`].
///
/// # Examples
///
/// ```
/// use flm_core::lang::rewrites::userdefs::EffectiveSignature;
///
/// // Two declared arguments, the first of which the template asks about:
/// // a call writes three groups, the flag last.
/// let signature = EffectiveSignature::new(2, [0]);
/// assert_eq!(signature.arity(), 3);
/// assert_eq!(signature.value_position(0), Some(1));
/// assert_eq!(signature.value_position(1), Some(2));
/// assert_eq!(signature.flag_position(0), Some(3));
/// assert_eq!(signature.flag_position(1), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectiveSignature {
    values: usize,
    flagged: Vec<usize>,
}

impl EffectiveSignature {
    /// The signature of a definition whose first `values` declared arguments carry
    /// a value, among which those at the indices `flagged` (counting from zero)
    /// carry a flag.
    ///
    /// `values` is the number of declared arguments that a template can refer to:
    /// all of them except a trailing tack-on argument.
    ///
    /// An index not below `values`, and a repeated index, are left out: the flags
    /// are the listed arguments, each once, in declared order, regardless of the
    /// order in which they are given.
    pub fn new(values: usize, flagged: impl IntoIterator<Item = usize>) -> EffectiveSignature {
        let named: BTreeSet<usize> = flagged.into_iter().filter(|index| *index < values).collect();
        EffectiveSignature { values, flagged: named.into_iter().collect() }
    }

    /// How many declared arguments carry a value, which is how many value
    /// groups a call writes.
    pub fn values(&self) -> usize {
        self.values
    }

    /// The declared arguments that carry a flag, as indices counting from zero, in
    /// declared order.
    pub fn flagged(&self) -> &[usize] {
        &self.flagged
    }

    /// How many effective arguments a call writes: one value per declared
    /// argument that carries one, and one flag per flagged argument.
    pub fn arity(&self) -> usize {
        self.values + self.flagged.len()
    }

    /// The position of the value of the declared argument `index` among the
    /// effective arguments, counting from one; `None` when that argument carries no
    /// value (it is not a declared argument, or it is a tack-on argument).
    pub fn value_position(&self, index: usize) -> Option<usize> {
        (index < self.values).then_some(index + 1)
    }

    /// The position of the flag of the declared argument `index` among the
    /// effective arguments, counting from one; `None` when that argument carries no
    /// flag.
    pub fn flag_position(&self, index: usize) -> Option<usize> {
        let at = self.flagged.iter().position(|flagged| *flagged == index)?;
        Some(self.values + at + 1)
    }
}

/// Why a definition is not *recomposable*: why a backend that writes definitions
/// in its own output language cannot write this one as a single definition of
/// that language, and expands its use sites instead.
///
/// A backend recomposes a definition when it writes the definition once, as a
/// definition of its output language, and writes each use site as a call of it.
/// The `latex-recomposed` backend does this for LaTeX, through the
/// kept-definition lowering rules of the `userdefs` feature
/// ([`kept`](crate::stdcore::features::userdefs::kept)).
///
/// **Every definition an author can write is recomposable**, apart from the
/// reasons listed below. This covers arguments of every shape a definition can
/// declare, at any position: a `{…}` group, an optional `[…]` group, a
/// characters-only group, a key-value group, a verbatim argument, a `*` marker,
/// and a trailing tack-on argument, which the definition may declare and no
/// template may refer to ([`DefinitionProblem::TackOnPlaceholder`]). It covers
/// any number of arguments, beyond what the output language's own definitions
/// can take. It covers a template, and every declared default value, that is
/// plain content around its placeholders and its conditionals (with no
/// substitution argument), wherever a placeholder is placed; an environment's
/// template, regardless of what it does with its body (places it between two
/// parts, wraps it, repeats it, or drops it); a template that uses the
/// definition **itself**, which becomes a definition of the output language that
/// calls itself; and verbatim content in the template itself (content that the
/// output language reads character by character from the file it is written
/// in), which the backend writes in a form that survives being read as tokens.
///
/// No conditional prevents recomposition: each of the fourteen conditional
/// commands of the template language is either
/// decided when the definition is written, tested by the output language on a
/// value the call carries, or, in a default value, evaluated where that default
/// value is instantiated for one use site ([`ModeVariant::instantiate_default`]).
/// No default value prevents it either: a default value that holds placeholders
/// of other arguments, or conditionals of its own, is resolved at each use site
/// that left its argument out, and the call carries the resolved content.
///
/// Where a template's placeholder is placed plays no part in this property. A
/// template that places a placeholder where a lowering rule needs its
/// *characters* (an address, a reference key, verbatim content, anything
/// computed from them) cannot be written as one definition either, since those
/// characters are known only at each call; but the backend finds this out when
/// it lowers the template, and then expands the definition's use sites. No
/// variant of this type records it.
///
/// The remaining reasons do not come from how the backend writes definitions.
/// They fall into four classes:
///
/// - *nature*: [`Specials`](NotRecomposable::Specials): a special character
///   sequence is written without a backslash, so it is not a command of the
///   output language and cannot become one; and [`Uses`](NotRecomposable::Uses),
///   which takes the class of the reason at the end of the chain of uses.
/// - *choice*: [`AlwaysExpanded`](NotRecomposable::AlwaysExpanded): the
///   definition asks to be expanded in every plan, so the question does not
///   arise.
/// - *error*: [`NotCompiled`](NotRecomposable::NotCompiled): the definition was
///   refused, which was reported with its own reason when it happened.
/// - *unreachable*: [`HookPlaceholder`](NotRecomposable::HookPlaceholder) and
///   [`ArgumentShape`](NotRecomposable::ArgumentShape), which **no definition an
///   author writes in settings can reach**. They exist for code that extends the
///   library and for an argument shape that a later version may add.
///
/// Nothing is reported when a definition is not recomposable for one of these
/// reasons: *nature* and *choice* follow from what the author wrote and decided,
/// an *error* is already reported, and an *unreachable* reason cannot be reached
/// from a document.
///
/// The `userdefs` feature's late-initialization build hook computes this
/// property once, when the registry is built, and records it on the definition;
/// it is read from [`UserDefinition::recomposable`]. The lowering rule that
/// writes a kept use site as a call beside its definition (`emit-call`, in the
/// `userdefs` feature) acts on it.
///
/// Each variant carries what a user needs to know about why the definition is
/// written expanded. As diagnostic data, the reason is a map with a stable
/// `kind` token ([`kind`](NotRecomposable::kind)) and the variant's fields,
/// never the displayed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotRecomposable {
    /// *Error.* The definition was refused when the registry was built (or
    /// declares no template), so there is no compiled template to recompose.
    /// The refusal was reported with its own reason when it happened.
    NotCompiled,
    /// *Choice.* The definition is expanded in every plan
    /// ([`ExpandPolicy::Always`]), so no use site of it ever reaches a lowering
    /// rule; the question does not arise, and the definition asks for this.
    AlwaysExpanded,
    /// *Nature.* A special character sequence definition: a sequence of
    /// characters is not a macro or an environment, and no definition of the
    /// output language can be one.
    Specials,
    /// *Unreachable.* An argument of a shape that a definition of the output language
    /// cannot take.
    ///
    /// **No shape that a definition can declare today reaches this variant.** A call
    /// passes every argument as one brace group holding the content the use site
    /// wrote, so the argument's shape in the FLM source never reaches the output
    /// language: a key-value argument's group is written unchanged, a verbatim
    /// argument's characters are written as content like any other value, and a
    /// tack-on argument (the macros a call absorbs after it, such as a `\label{…}`
    /// written after it) carries no effective argument at all, because no template
    /// may refer to it ([`DefinitionProblem::TackOnPlaceholder`]). The variant is
    /// kept for an argument shape that a later version may add and that the output
    /// language cannot express.
    ArgumentShape {
        /// The argument's index in the order the arguments are written,
        /// counting from zero.
        index: usize,
        /// A stable token for the argument's shape.
        shape: String,
    },
    /// *Unreachable.* The template holds a placeholder written with a substitution
    /// argument (`#[…]{…}`), or a placeholder whose content an expansion hook
    /// computes rather than an argument or the body.
    ///
    /// **No definition that an author writes in settings reaches this variant.** The
    /// placeholders of such a definition are its arguments and its body and nothing
    /// else, and a substitution argument on them is refused when the definition is
    /// compiled. The variant exists for definitions declared in Rust with
    /// placeholder names that an expansion hook computes
    /// ([`Definition::extra_placeholder`]).
    HookPlaceholder {
        /// The reference as written.
        reference: String,
    },
    /// *Nature, inherited.* The template uses another definition, expanded per plan,
    /// that is itself not recomposable, so its use site inside the recomposed
    /// template could not be written as a call. The chain of uses ends at one of the
    /// other variants, and this variant takes that variant's class.
    Uses {
        /// The used definition, as an author writes it (`\foo`).
        name: String,
    },
}

impl NotRecomposable {
    /// The stable token for the reason in diagnostic data: `not-compiled`,
    /// `always-expanded`, `specials`, `argument-shape`, `hook-placeholder` or
    /// `uses`.
    pub fn kind(&self) -> &'static str {
        match self {
            NotRecomposable::NotCompiled => "not-compiled",
            NotRecomposable::AlwaysExpanded => "always-expanded",
            NotRecomposable::Specials => "specials",
            NotRecomposable::ArgumentShape { .. } => "argument-shape",
            NotRecomposable::HookPlaceholder { .. } => "hook-placeholder",
            NotRecomposable::Uses { .. } => "uses",
        }
    }
}

impl fmt::Display for NotRecomposable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotRecomposable::NotCompiled => {
                f.write_str("the definition's template was not compiled")
            }
            NotRecomposable::AlwaysExpanded => f.write_str("the definition is always expanded"),
            NotRecomposable::Specials => {
                f.write_str("a special character sequence is not a command of the output language")
            }
            NotRecomposable::ArgumentShape { index, shape } => {
                write!(
                    f,
                    "argument {} has a shape ({shape}) the output language cannot express",
                    index + 1
                )
            }
            NotRecomposable::HookPlaceholder { reference } => {
                write!(
                    f,
                    "the template uses ‘#{reference}’, which is not an argument or the body"
                )
            }
            NotRecomposable::Uses { name } => {
                write!(
                    f,
                    "the template uses ‘{name}’, which cannot be written as a definition of the output language"
                )
            }
        }
    }
}

/// Diagnostic data: a map holding the [`kind`](NotRecomposable::kind) token
/// and the variant's fields, never the displayed text.
impl ToDiagnosticValue for NotRecomposable {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        let mut fields = vec![(String::from("kind"), self.kind().to_diagnostic_value())];
        match self {
            NotRecomposable::ArgumentShape { index, shape } => {
                fields.push((String::from("index"), index.to_diagnostic_value()));
                fields.push((String::from("shape"), shape.to_diagnostic_value()));
            }
            NotRecomposable::HookPlaceholder { reference } => {
                fields.push((String::from("reference"), reference.to_diagnostic_value()));
            }
            NotRecomposable::Uses { name } => {
                fields.push((String::from("name"), name.to_diagnostic_value()));
            }
            _ => {}
        }
        DiagnosticValue::Map(fields)
    }
}

/// What the registry build decided about one user definition, beyond its
/// compiled templates: the facts a later stage needs that can only be decided by
/// looking at all the definitions the `userdefs` feature registers.
///
/// The `userdefs` feature's late-initialization build hook works them out once,
/// over all its definitions, and records them in the data of each definition's
/// element ([`UserDefinition::record_registry_build_facts`]). They form one value
/// because they are decided together, once, so a stage that reads any of them
/// reads a consistent set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryBuildFacts {
    /// How a backend that recomposes definitions (writes each as one definition of
    /// its own output language) can write this one: the arguments one call writes
    /// ([`EffectiveSignature`]), or the reason no single definition of that language
    /// can express it ([`NotRecomposable`]).
    ///
    /// The two are one field because the signature exists exactly when the
    /// definition is recomposable: a definition that no backend writes a call of has
    /// no call to describe.
    pub recomposition: Result<EffectiveSignature, NotRecomposable>,
    /// Whether the `userdefs` feature also registers a definition of the same kind
    /// and name for the other parsing mode. The two are *per-mode definitions* (one
    /// name declared once for text mode and once for mathematics mode), and an
    /// output language with one command per name needs a separate name for each.
    pub shares_its_name: bool,
}

/// The data of a user definition's element ([`ElementSpec::value`]): the
/// [`Expand`] implementation that the `userdefs` feature attaches to the
/// definition's element.
///
/// It holds the declaration and, once the `userdefs` feature's
/// late-initialization build hook has run ([`UserDefinition::compile_with`]),
/// the compiled templates and the facts the registry build decided about the
/// definition ([`RegistryBuildFacts`]). A lowering rule or an extension finds it
/// on a use site with [`element_value`](crate::model::element::element_value).
pub struct UserDefinition {
    definition: Definition,
    compiled: OnceLock<Arc<CompiledDefinition>>,
    registry_build_facts: OnceLock<RegistryBuildFacts>,
    recursion_limit: OnceLock<usize>,
}

impl UserDefinition {
    /// The data for the element of `definition`, with its templates not yet
    /// compiled.
    pub fn new(definition: Definition) -> Arc<UserDefinition> {
        Arc::new(UserDefinition {
            definition,
            compiled: OnceLock::new(),
            registry_build_facts: OnceLock::new(),
            recursion_limit: OnceLock::new(),
        })
    }

    /// The expansion depth limit that applies to this definition: how many
    /// expansions one chain may nest before a use site is replaced by its literal
    /// source text and reported with `flm.userdefs.recursion-limit`. It is the
    /// `userdefs` feature's configured limit once the feature has recorded it
    /// ([`record_recursion_limit`](UserDefinition::record_recursion_limit)), and
    /// [`DEFAULT_RECURSION_LIMIT`] before. A lowering rule that expands a use site
    /// outside the expansion stages reads the limit here, so that every expansion of
    /// a definition runs under the same configured limit.
    pub fn recursion_limit(&self) -> usize {
        self.recursion_limit.get().copied().unwrap_or(DEFAULT_RECURSION_LIMIT)
    }

    /// Records the expansion depth limit, once; a second call is ignored. The
    /// `userdefs` feature calls this when it creates the data of the definition's
    /// element, with its configured limit; no other caller should.
    pub fn record_recursion_limit(&self, limit: usize) {
        let _ = self.recursion_limit.set(limit);
    }

    /// The declaration.
    pub fn definition(&self) -> &Definition {
        &self.definition
    }

    /// The compiled templates, once [`compile`](UserDefinition::compile) or
    /// [`compile_with`](UserDefinition::compile_with) has succeeded; `None` before,
    /// and for a definition whose compilation failed.
    pub fn compiled(&self) -> Option<&Arc<CompiledDefinition>> {
        self.compiled.get()
    }

    /// Records the facts the registry build decided about this definition
    /// ([`RegistryBuildFacts`]), once: a second call changes nothing and returns
    /// `false`.
    ///
    /// The `userdefs` feature calls this from its late-initialization build hook,
    /// after the templates are compiled, for every definition it registers
    /// ([`record_recomposability`](crate::stdcore::features::userdefs::kept::record_recomposability));
    /// no other caller should. The facts are read back through
    /// [`recomposable`](UserDefinition::recomposable),
    /// [`effective_signature`](UserDefinition::effective_signature) and
    /// [`shares_its_name`](UserDefinition::shares_its_name).
    pub fn record_registry_build_facts(&self, facts: RegistryBuildFacts) -> bool {
        self.registry_build_facts.set(facts).is_ok()
    }

    /// Whether the definition is recomposable, that is, whether a backend can write
    /// it as one definition of its output language, in the sense [`NotRecomposable`]
    /// states. The result is known once the `userdefs` feature's
    /// late-initialization build hook has recorded the facts of the registry build
    /// ([`record_registry_build_facts`](UserDefinition::record_registry_build_facts)).
    ///
    /// `Err` carries the reason, in one of the four classes that [`NotRecomposable`]
    /// describes. Nothing is reported when a definition is not recomposable, so this
    /// is where a host finds out why a definition is written expanded. Before the
    /// facts are recorded, the result is [`NotRecomposable::NotCompiled`], the same
    /// result a refused definition keeps.
    pub fn recomposable(&self) -> Result<(), NotRecomposable> {
        match self.registry_build_facts.get() {
            Some(facts) => facts.recomposition.as_ref().map(|_| ()).map_err(Clone::clone),
            None => Err(NotRecomposable::NotCompiled),
        }
    }

    /// The arguments that one call of this definition writes when it is recomposed
    /// ([`EffectiveSignature`]), once the `userdefs` feature's late-initialization
    /// build hook has recorded the facts of the registry build
    /// ([`record_registry_build_facts`](UserDefinition::record_registry_build_facts));
    /// `None` before, and for a definition that is not recomposable, of which no
    /// backend writes a call.
    pub fn effective_signature(&self) -> Option<&EffectiveSignature> {
        self.registry_build_facts.get()?.recomposition.as_ref().ok()
    }

    /// Whether the `userdefs` feature also registers a definition of the same kind
    /// and name for the other parsing mode, once its late-initialization build hook
    /// has recorded the facts of the registry build
    /// ([`record_registry_build_facts`](UserDefinition::record_registry_build_facts));
    /// `false` before. The two are *per-mode definitions*: one name declared once
    /// for text mode and once for mathematics mode.
    ///
    /// This is a fact about all the definitions the feature registers, not about
    /// this one, which is why it is recorded rather than computed: only the build
    /// hook sees every definition. A backend that writes definitions in its own
    /// output language needs it, because that language has one command per name:
    /// the `latex-recomposed` backend writes each per-mode definition under a name
    /// of its own and calls the one for the use site's parsing mode.
    pub fn shares_its_name(&self) -> bool {
        self.registry_build_facts.get().is_some_and(|facts| facts.shares_its_name)
    }

    /// Compiles the templates against the built language `lang` and keeps the
    /// result, over a template language built for this call alone;
    /// [`compile_with`](UserDefinition::compile_with) shares one template language
    /// across many definitions.
    ///
    /// Returns the warnings of the compilation, for the caller to report with the
    /// registry's build diagnostics. Once a compilation has succeeded, a further
    /// call compiles nothing and returns no diagnostics, and the result stays
    /// available through [`compiled`](UserDefinition::compiled).
    ///
    /// # Errors
    ///
    /// The diagnostics of a failed compilation, the same as for the module-level
    /// [`compile`]. The definition then stays uncompiled, and every use site of it is
    /// replaced by its literal source text and reported with
    /// `flm.userdefs.bad-definition` ([`DefinitionProblem::NotCompiled`]).
    pub fn compile(&self, lang: &BuiltLang) -> Result<Vec<FlmDiagnostic>, Vec<FlmDiagnostic>> {
        if self.compiled.get().is_some() {
            return Ok(Vec::new());
        }
        match TemplateLanguage::new(lang) {
            Ok(language) => self.compile_with(lang, &language),
            Err(errors) => Err(minting_diagnostics(&self.definition.display_name(), &errors)),
        }
    }

    /// [`compile`](UserDefinition::compile) over a template language that is already
    /// built: the `userdefs` feature builds one [`TemplateLanguage`] for all its
    /// definitions and calls this once per definition.
    ///
    /// # Errors
    ///
    /// The same as [`compile`](UserDefinition::compile).
    pub fn compile_with(
        &self,
        lang: &BuiltLang,
        language: &TemplateLanguage,
    ) -> Result<Vec<FlmDiagnostic>, Vec<FlmDiagnostic>> {
        if self.compiled.get().is_some() {
            return Ok(Vec::new());
        }
        let compiled = compile_with(&self.definition, lang, language)?;
        let warnings = compiled.diagnostics().to_vec();
        // A concurrent first compilation of the same definition produces the same
        // value; whichever lands first is kept.
        let _ = self.compiled.set(Arc::new(compiled));
        Ok(warnings)
    }

    /// The definition's element, under the element name `name`: an [`ElementSpec`]
    /// that carries this value as its data ([`ElementSpec::value`]), this value as
    /// its [`Expand`] aspect, the declared block role as its [`BlockRoleAspect`],
    /// and the definition's documentation.
    pub fn element(self: &Arc<Self>, name: impl Into<ElementName>) -> Arc<ElementSpec> {
        let hook: Arc<dyn Expand> = self.clone();
        self.element_with(name, hook)
    }

    /// The definition's element, under the element name `name`, whose `Expand`
    /// aspect is `hook` instead of this value. Everything else is what
    /// [`element`](UserDefinition::element) gives: this value as the element's data,
    /// so that [`element_value`](crate::model::element::element_value) finds the
    /// definition on a use site, the declared block role, and the documentation.
    ///
    /// This is for a feature that expands its definitions with an [`Expand`]
    /// implementation of its own, one that computes the content of placeholder
    /// names it declares ([`Definition::extra_placeholder`]) with placeholder
    /// bindings that pass every other placeholder to [`DefinitionBindings`]. That
    /// implementation should find its compiled templates through
    /// [`variant_for_use`](UserDefinition::variant_for_use), so that a use the
    /// definition cannot expand is reported as it is for every user definition.
    pub fn element_with(
        self: &Arc<Self>,
        name: impl Into<ElementName>,
        hook: Arc<dyn Expand>,
    ) -> Arc<ElementSpec> {
        let value: Arc<dyn Any + Send + Sync> = self.clone();
        let role: Arc<dyn BlockRoleAspect> = Arc::new(FixedBlockRole(self.definition.block_role));
        Arc::new(
            ElementSpec::new(name)
                .docs(self.definition.docs.clone())
                .with_value(value)
                .with::<dyn Expand>(hook)
                .with::<dyn BlockRoleAspect>(role),
        )
    }

    /// The element declaration ([`ElementDef`]) that the `userdefs` feature
    /// registers for this definition, owned by the feature `owner`: the command name
    /// it is written as (none for a special character sequence, whose characters are
    /// part of its syntax), its syntax ([`Definition::syntax`]), the parsing modes it
    /// is available in, and its element, under the element name `element_name`.
    pub fn element_def(
        self: &Arc<Self>,
        owner: impl Into<FeatureName>,
        element_name: impl Into<ElementName>,
    ) -> ElementDef {
        let hook: Arc<dyn Expand> = self.clone();
        self.element_def_with(owner, element_name, hook)
    }

    /// The element declaration of [`element_def`](UserDefinition::element_def), with
    /// `hook` as the element's `Expand` aspect
    /// ([`element_with`](UserDefinition::element_with)): the declaration a feature
    /// registers for a definition that it expands with an `Expand` implementation of
    /// its own.
    pub fn element_def_with(
        self: &Arc<Self>,
        owner: impl Into<FeatureName>,
        element_name: impl Into<ElementName>,
        hook: Arc<dyn Expand>,
    ) -> ElementDef {
        let names: Vec<Name> = match self.definition.kind {
            DefinitionKind::Specials => Vec::new(),
            _ => vec![self.definition.name.clone()],
        };
        let element = self.element_with(element_name, hook);
        ElementDef::new(owner, names, self.definition.syntax(), element)
            .docs(self.definition.docs.clone())
            .modes(self.definition.modes.modes().iter().copied())
    }

    /// The compiled templates with which the use site `node` is expanded: those of
    /// the use site's parsing mode.
    ///
    /// # Errors
    ///
    /// The condition ([`BadDefinition`]) with which a use that the definition cannot
    /// expand is reported at the use site: [`DefinitionProblem::NotCompiled`] when
    /// the definition was refused when the registry was built (or its templates are
    /// not compiled yet), and [`DefinitionProblem::NotInForceInMode`] when its one
    /// template text did not parse in the use site's parsing mode. An [`Expand`]
    /// implementation reports it as an Error at the use site through
    /// [`Expander::recover`].
    pub fn variant_for_use(&self, node: &NodeRef<'_, Flm>) -> Result<&ModeVariant, BadDefinition> {
        let display = self.definition.display_name();
        let mode = node.parsing_state().mode();
        let Some(compiled) = self.compiled.get() else {
            return Err(BadDefinition::new(display, DefinitionProblem::NotCompiled));
        };
        compiled.variant(mode).ok_or_else(|| {
            // The definition was declared by one text, which compiled in the
            // other mode only: that is where it is in force (the module
            // documentation's first rule).
            BadDefinition::new(display, DefinitionProblem::NotInForceInMode { mode })
        })
    }

    /// The override declaration ([`OverrideDef`]) that the feature `owner` registers
    /// when the definition explicitly replaces another feature's definition of the
    /// same command ([`Definition::overriding`]); `None` otherwise.
    pub fn override_def(&self, owner: impl Into<FeatureName>) -> Option<OverrideDef> {
        self.definition.overrides.then(|| OverrideDef {
            owner: owner.into(),
            kind: self.definition.callable_kind(),
            name: self.definition.name.clone(),
        })
    }
}

impl fmt::Debug for UserDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDefinition")
            .field("definition", &self.definition)
            .field("compiled", &self.compiled.get().is_some())
            .field("registry_build_facts", &self.registry_build_facts.get())
            .finish()
    }
}

/// The placeholder bindings of one use site of a definition
/// ([`PlaceholderBindings`]): a numbered or named placeholder is replaced by the
/// content of that argument at the use site, an argument the use site left out
/// by its compiled default value for the use site's parsing mode, and `#{body}`
/// by the use site's body. A placeholder name whose content an expansion hook
/// computes ([`Definition::extra_placeholders`]) has no value here
/// ([`Unresolvable`](PlaceholderValue::Unresolvable)).
///
/// The [`Expand`] implementation of [`UserDefinition`] instantiates its template
/// with these bindings. A feature that attaches an `Expand` implementation of
/// its own to a definition's element, to compute the content of extra
/// placeholder names, builds its bindings on top of these: it returns the values
/// of its own names and passes every other placeholder to
/// [`value`](PlaceholderBindings::value) here, so that arguments, default values
/// and the body mean the same in its templates as in every user definition's.
///
/// # Examples
///
/// ```
/// use flm_core::lang::Flm;
/// use flm_core::lang::rewrites::userdefs::{
///     DefinitionBindings, PlaceholderBindings, PlaceholderTarget, PlaceholderValue,
/// };
/// use flm_core::techy::core::node::NodeSlice;
///
/// /// Bindings that give `#{stamp}` no value and treat every other reference
/// /// as a user definition does.
/// struct Stamped<'t> {
///     base: DefinitionBindings<'t>,
/// }
///
/// impl PlaceholderBindings for Stamped<'_> {
///     fn value<'a>(
///         &'a self,
///         target: &PlaceholderTarget,
///         substitution: Option<NodeSlice<'_, Flm>>,
///     ) -> PlaceholderValue<'a> {
///         match target {
///             PlaceholderTarget::Extra(name) if name == "stamp" => PlaceholderValue::NoValue,
///             _ => self.base.value(target, substitution),
///         }
///     }
/// }
/// ```
pub struct DefinitionBindings<'t> {
    site: NodeRef<'t, Flm>,
    variant: &'t ModeVariant,
}

impl<'t> DefinitionBindings<'t> {
    /// The bindings of the use site `site`, with the default values of `variant`:
    /// the compiled templates of the use site's parsing mode
    /// ([`UserDefinition::variant_for_use`], or [`CompiledDefinition::variant`]).
    pub fn new(site: NodeRef<'t, Flm>, variant: &'t ModeVariant) -> DefinitionBindings<'t> {
        DefinitionBindings { site, variant }
    }
}

impl fmt::Debug for DefinitionBindings<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefinitionBindings").field("site", &self.site.id()).finish()
    }
}

impl PlaceholderBindings for DefinitionBindings<'_> {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        _substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        match target {
            PlaceholderTarget::Argument(index) => match substituted_argument_nodes(
                &self.site,
                *index,
            ) {
                Some(content) => PlaceholderValue::Provided(content),
                None => match self.variant.default_of(*index) {
                    Some(template) => PlaceholderValue::Default(template),
                    None => PlaceholderValue::NoValue,
                },
            },
            PlaceholderTarget::Body => match self.site.body() {
                Some(body) => PlaceholderValue::Provided(body),
                None => PlaceholderValue::NoValue,
            },
            PlaceholderTarget::Extra(_) => PlaceholderValue::Unresolvable,
        }
    }
}

/// The nodes that replace a placeholder of the declared argument `index` at the
/// use site `site`: the argument's content nodes, and nothing else. `None` when
/// the use site did not write that argument; the argument's default value, or
/// nothing, then takes its place.
///
/// The content nodes are the same regardless of the kind of argument declared
/// (the last rule of the [module documentation](self)): for a verbatim argument,
/// they are the characters between its delimiters, not the group the parse put
/// around them.
///
/// Every consumer that needs an argument's content calls this function: the
/// expansion, through the use site's placeholder bindings
/// ([`PlaceholderBindings`]), and the kept-definition lowering rules of the
/// `userdefs` feature, which write the same nodes into the argument's group of
/// the call they write for the `latex-recomposed` backend. The expanded document
/// and the document with kept definitions therefore cannot disagree about an
/// argument's content.
// See decision rationale at [§flm-dd-dr:lang-placeholder-content].
pub(crate) fn substituted_argument_nodes<'t>(
    site: &NodeRef<'t, Flm>,
    index: usize,
) -> Option<NodeSlice<'t, Flm>> {
    site.argument_content_nodes(index)
}

impl Expand for UserDefinition {
    fn policy(&self) -> ExpandPolicy {
        self.definition.expand
    }

    /// The expansion of a user definition at one use site: the compiled templates of
    /// the use site's parsing mode
    /// ([`variant_for_use`](UserDefinition::variant_for_use)), then the template
    /// instantiated with the use site's arguments through
    /// [`Expander::instantiate`], which also enforces the expansion depth limit. A
    /// use that the definition cannot expand is replaced by its literal source text
    /// and reported with the condition `variant_for_use` returns.
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        let variant = match self.variant_for_use(&node) {
            Ok(variant) => variant,
            Err(condition) => {
                let diagnostic = Diagnostic::error(condition, node.span().clone());
                return Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?));
            }
        };
        let bindings = DefinitionBindings::new(node, variant);
        Ok(Expansion::Emit(xp.instantiate(cx, node, variant.template(), &bindings)?))
    }
}
