//! The lowering rules for a kept use site of a user definition: what a
//! lowering rule does with a use site of a `per-plan` definition that a plan
//! keeps in the tree (the capability `flm.userdefs.kept`) instead of
//! expanding it.
//!
//! A *kept definition* is a user definition whose use sites a plan left in the
//! tree (the `flm.userdefs.kept` alternative of the [`userdefs`](super)
//! feature): a lowering rule either writes the use site as a canonical call
//! beside the definition for the [style
//! file](crate::stdcore::backends::latex::style) (the `.sty` file written
//! beside the recomposed LaTeX document), or instantiates the template when
//! the definition cannot be written back as LaTeX or a rule needs a
//! placeholder's characters. A plan for the `latex-recomposed` backend keeps
//! the use sites of every `per-plan` definition, so this module decides how an
//! author's definitions appear in the recomposed LaTeX; the rendering guide
//! describes the result for authors ([user definitions in the recomposed
//! LaTeX](crate::guide::rendering#user-definitions-in-the-recomposed-latex)).
//! Under the expanded alternative (the default) the expand pass replaces every
//! use site before lowering, and the definition's `expand-in-tree` rule
//! ([`super::lowering`]) only gives the element a rule.
//!
//! # The two rules
//!
//! Every `per-plan` definition registers one rule of each kind for its element:
//!
//! | rule | name | default | what it does |
//! |---|---|---|---|
//! | `instantiate-at-lowering` ([`InstantiateAtLowering`]) | `flm.userdefs.<kind>.<name>.lower.instantiate-at-lowering` | never; a plan request selects it by name | expands the use site through the expand pass and lowers the result in its place |
//! | `emit-call` ([`EmitCall`]) | `flm.userdefs.<kind>.<name>.lower.emit-call` | on the `latex-recomposed` backend | writes the use site as a call of the definition, and the definition itself into the style file; instantiates the use site instead when the definition is not recomposable or a rule of its template needs a placeholder's characters |
//!
//! Both rules require `flm.userdefs.kept`, so selecting either puts the
//! `flm.userdefs.keep` marker stage into the plan and excludes the expansion of
//! the `per-plan` definitions. A definition whose expansion policy is `always`
//! has neither rule: its use sites never reach a lowering rule.
//!
//! # How the rule of a definition is chosen
//!
//! A `per-plan` definition has three rules for its element: `expand-in-tree`,
//! the default on every backend ([`DefaultMark::Everywhere`]); `emit-call`,
//! the default on the `latex-recomposed` backend alone
//! ([`DefaultMark::OnBackends`]); and `instantiate-at-lowering`, never a
//! default ([`DefaultMark::Never`]). The rule selection order
//! ([`SelectedRules`](crate::core::plan::SelectedRules)) decides between them
//! without a conflict: a rule the plan request
//! ([`PlanRequest`](crate::core::plan::PlanRequest)) prefers by name comes first,
//! then a rule marked default on the backend, then a rule marked default
//! everywhere. The recomposed LaTeX therefore gets `emit-call` and every other
//! backend gets `expand-in-tree`: a plan for the `latex-recomposed` backend
//! holds `flm.userdefs.keep` and not `flm.userdefs.expand`, and a plan for any
//! other backend (`html`, `text`) the other way round, unless the plan request
//! prefers `instantiate-at-lowering` by name, which keeps the use sites on that
//! backend too.
//!
//! # How `instantiate-at-lowering` lowers a use site
//!
//! The rule copies the use site into a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) of its own (a
//! small tree built during the run, outside the document's trees;
//! [`copy_run_into_value`]) and expands it with the expand pass ([`expand`])
//! over every definition, the `always` ones included, under the definition's
//! expansion depth limit ([`UserDefinition::recursion_limit`], the limit of the
//! feature's configuration). The diagnostics of the expansion (a depth limit
//! reached, a placeholder without a value) are reported through the
//! [`Lowerer`]. The rule then splices the expanded value
//! ([`Lowerer::splice`]): [value
//! catch-up](crate::guide::concepts_overview#value-catch-up) runs the plan's
//! rewrite stages and the per-tree stages over it, and its nodes are lowered
//! through the rules. On a backend whose plan lays out the trees in paragraphs
//! (`html`, `text`), the block-structure pass lays out the expanded value
//! during that catch-up, so the use site is written exactly as in the expanded
//! document.
//!
//! # Recomposable definitions
//!
//! `emit-call` writes a definition as one LaTeX definition only when it is
//! *recomposable*: when a backend can write it as one definition of its own
//! output language. [`NotRecomposable`] lists the reasons a definition may not
//! be: its nature (a special character sequence), its author's choice (the
//! expansion policy `always`), an error already reported (a refused
//! definition), or the use, in its template or a default value, of a
//! `per-plan` definition that is not recomposable; two further reasons exist
//! only for definitions declared in Rust by an extension. Every other definition is recomposable:
//!
//! - arguments of every shape a definition can declare, at any position: a
//!   `{…}` group, an optional `[…]` group, a characters-only group, a
//!   key-value group, a verbatim argument, a `*` marker and a trailing tack-on
//!   argument;
//! - a template and default values that are content around their placeholders
//!   and their conditionals;
//! - an environment's template, regardless of what it does with its body;
//! - any number of arguments;
//! - a definition that uses itself, directly or through other definitions,
//!   which becomes a LaTeX definition that calls itself, so that LaTeX runs the
//!   recursion;
//! - a name declared once for text mode and once for mathematics mode, which
//!   becomes two LaTeX definitions under two names, the [per-mode
//!   definitions](crate::stdcore::backends::latex::names#per-mode-definitions);
//!   each call is written under the name for the parsing mode of its use
//!   site.
//!
//! All but one of the decisions about the shape of the LaTeX definition are
//! taken when the registry is built: the `userdefs` feature's late-initialization build
//! hook calls [`record_recomposability`], which records on each definition
//! whether it is recomposable and the arguments its calls write
//! ([`RegistryBuildFacts`], read back through
//! [`UserDefinition::recomposable`] and
//! [`UserDefinition::effective_signature`]). The remaining decision is taken
//! when the template is lowered, and it is the same at every use site: whether a
//! rule of the template needs the characters of a placeholder (next section).
//! A use site of a recomposable definition is written as a call in every other
//! case.
//!
//! *Verbatim blocks* (content that LaTeX reads character by character from the
//! file it is written in) do not prevent recomposition. Wherever a verbatim
//! block reaches LaTeX inside a group that LaTeX reads as tokens (an argument
//! of a call, the body of a LaTeX definition), the `latex-recomposed` emitter
//! writes it as a call of
//! [`\flmVerbatimBlock`](crate::stdcore::backends::latex::preamble::VERBATIM_BLOCK_COMMAND),
//! which rebuilds the block where it is called. The emitter decides this from
//! where the block is written; the rules of this module do nothing for it.
//!
//! # Where a rule needs an argument's characters
//!
//! A placeholder becomes a LaTeX parameter **only where the template sets it
//! as content**: a rule that sets content lowers the placeholder as content,
//! and the placeholder rule of the LaTeX family (the parts every LaTeX backend
//! shares, [`crate::stdcore::backends::latex`];
//! [`PlaceholderLatexRule`](crate::stdcore::backends::latex::PlaceholderLatexRule))
//! writes the parameter, which LaTeX
//! replaces by what a call passes. A rule that needs the **characters** of
//! content instead (a link's address, a reference's label, a term's target,
//! verbatim content, anything computed from them) reads them through
//! [`chars_at_lowering`](crate::core::lowering::chars_at_lowering), or, where it
//! computes from content as written with its markup (a term's target, a list
//! label template), through
//! [`spelling_at_lowering`](crate::core::lowering::spelling_at_lowering). A
//! placeholder has no characters until the template is expanded, so the rule
//! returns [`LowerError::PlaceholderCharacters`].
//!
//! `emit-call` lowers the template as a synthesized value. When that lowering
//! returns `PlaceholderCharacters`, the rule expands the use site instead of
//! writing a call (as for a definition that is not recomposable) and stages
//! nothing for the style file (to *stage* a node is to build it into the
//! lowered tree through the [`Lowerer`]). The template is the same at every use site, so
//! every use site of the definition is expanded, and the definition is never
//! written as a LaTeX definition. For example, the use sites of a definition
//! whose template is `\verbcode{#1}`, `\url{#1}`, `\href{https://doi.org/#1}{#1}`,
//! `\ref{#1}`, `\ref{sec:#1}`, `\eqref{#1}`, `\hyperref[#1]{…}` or
//! `\term[#1]{…}` are expanded, while a template `see #1`, `\emph{#1}` or
//! `\ifblank{#1}{…}{…}` is written as a LaTeX definition. Nothing inspects the
//! shape of the template, so an element that a later feature adds behaves the
//! same way as long as it reads characters through those two functions. The
//! mathematics writer does the same for a use site inside mathematics.
//!
//! The diagnostics and requirements of the discarded lowering are dropped with
//! it ([`Lowerer::splice`]). Each use site of such a definition therefore
//! lowers the template once, discards that lowering and then lowers its
//! expansion: the lowering is done twice per use site.
//!
//! # A template that declares a reference point
//!
//! A reference point is one place in a document that can be referred to,
//! numbered, or given an anchor ([`RefPoint`](crate::model::aspects::RefPoint)).
//! `emit-call` lowers a kept definition's template as a synthesized value (the
//! body of the LaTeX definition, which the style file holds once per name),
//! and `instantiate-at-lowering` lowers each use site's expansion as one too. A
//! synthesized value may state no fact about the document as a whole: its value
//! catch-up refuses a [table piece](crate::core::data::table::PieceClass) (the
//! part of a table one stage invocation produced) that declares a label. A template that
//! declares a reference point carrying a label (a construct followed by
//! `\label`, a labeled xnote, a defined term) therefore cannot be lowered there,
//! and neither can a template holding a `\label` whose text a placeholder
//! completes: value catch-up does not recognize that `\label` as a label, so
//! this module refuses such a template itself, before lowering it.
//!
//! Every use site of such a definition is refused with the Error diagnostic
//! [`KeptTemplateDeclaresPoint`], which gives the definition and the node of
//! the template that declares the point, and lowers to a childless
//! `flm.pres.unsupported` stand-in node, which the `latex-recomposed` backend
//! writes as a comment. The remedy is the expansion policy
//! [`Always`](ExpandPolicy::Always): the use sites are then expanded in the
//! document before the labels are read, and each label is the document's own.
//! Selecting `instantiate-at-lowering` is no remedy, because the instance is a
//! synthesized value too and is refused in the same way.
//!
//! A reference in a template declares no reference point. A reference whose
//! target is fixed is written as a reference anywhere is, and one whose target
//! a placeholder completes needs that placeholder's characters, so the use
//! sites are expanded (previous section).
//!
//! # The canonical call
//!
//! **A call is written as the definition's name followed by one brace group
//! per effective argument, and nothing else.** FLM has already read the
//! document, so at every use site the value of every argument, and whether the
//! author wrote it, are known when the call is written. Written this way, the
//! call leaves LaTeX nothing to parse: no bracket it could take for an optional
//! argument, no star to look for, no default to fill in, and the LaTeX
//! definition needs no interface for reading arguments.
//!
//! An *effective argument* is one brace group of a call, in the order of the
//! definition's *effective signature* ([`EffectiveSignature`]):
//!
//! - first one **value** per declared argument a template can refer to, in the
//!   order declared, holding what the expansion would substitute for a
//!   placeholder of that argument: the content the use site wrote; or, where
//!   the use site left the argument out, the argument's declared default
//!   value, instantiated at that use site
//!   ([`ModeVariant::instantiate_default`]) and lowered in its place; or
//!   nothing. The argument's declared shape plays no part: a key-value
//!   argument's value is the group the use site wrote, keys included, since
//!   FLM never reads the keys separately; a verbatim argument's value is the
//!   characters written between its delimiters, written as text like any other
//!   content, since the delimiters belong to the use site's syntax. The group a
//!   call passes is therefore always an ordinary brace group, and no LaTeX
//!   verbatim argument (`v`) is ever declared;
//! - then one **flag** per declared argument the template asks a *provided*
//!   question about (whether the use site wrote it), in the order declared,
//!   holding `\BooleanTrue` where the use site wrote that argument and
//!   `\BooleanFalse` where it did not.
//!
//! A **tack-on argument** (the trailing argument that absorbs the field
//! macros, such as a `\label{…}`, written after a call;
//! [fields](crate::guide::concepts_overview#fields-data-a-construct-reads)) is
//! neither. A template cannot refer to one: what the call absorbed is whole
//! macros, which have no meaning as the value of a parameter, so a reference
//! to one refuses the definition when it is compiled, on every backend
//! ([`DefinitionProblem::TackOnPlaceholder`](crate::lang::rewrites::userdefs::DefinitionProblem::TackOnPlaceholder)).
//! A definition may declare a tack-on argument and not refer to it: it is
//! recomposed like any other, its calls write no group for that argument, and
//! what they absorb is not written. A placeholder that would give a template
//! access to the absorbed macros is not implemented.
//!
//! For example, where `\around` declares the arguments `[pre]{key}[post]` with
//! the default value `p.` for `post`, `\around[see]{Knuth}` is written
//! `\flmUDaround{see}{Knuth}{p.}`. The signature is computed once, when the
//! registry is built, so that the definition and every call of it agree on the
//! position of each group.
//!
//! # The definition written into the style file
//!
//! **A recomposed macro is a long, expandable macro over mandatory arguments**,
//! written `\providecommand\<name>[<n>]{<body>}`, and a recomposed environment
//! is a document environment with one `+m` per effective argument. Long, so an
//! argument may hold a paragraph break; expandable, so a call works inside
//! `\edef`, a PDF bookmark and a case change; guarded (`\providecommand`), so a
//! definition of that name that the author's class or preamble made before the
//! style file is loaded is the one used. Two render options of the
//! `latex-recomposed` backend change this form, and nothing else does:
//!
//! - A definition whose name the render option
//!   [`bare_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::bare_names)
//!   lists is written under the name the document gives it, without a guard
//!   (`\long\def`), since the name is listed so that the document's definition
//!   replaces LaTeX's own.
//! - A macro whose name the render option
//!   [`robust_names`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions::robust_names)
//!   lists is written `\ProvideDocumentCommand\<name>{+m …}`, one `+m` per
//!   effective argument (`\DeclareDocumentCommand` where both options list
//!   it), which makes it a robust command, protected from expansion, rather than
//!   an expandable macro.
//!
//! The emitter of the `latex-recomposed` backend
//! ([`crate::stdcore::backends::latex_recomposed::emit`]) writes these forms
//! from the definition construct's payload.
//!
//! **An environment's body.** LaTeX sets an environment's body between the
//! begin code and the end code. A template that refers to the body once, among
//! its own top-level nodes, is cut in two there. A template that refers to the
//! body anywhere else (inside a group, inside another command's argument, or in
//! two places), or not at all, is written whole as the begin code, and the
//! environment takes its body as a last argument `+b`; a template that does
//! not refer to its body never writes that argument, so LaTeX reads the body
//! and drops it, as FLM's expansion does. A call is the same either way.
//!
//! Every kept definition is written under a prefixed name of its own
//! ([`recomposed_name`]), at the definition and at every call: the macro
//! `\hello` as `\flmUDhello`, the environment `note` as `flmUEnote`, a macro
//! whose name holds anything other than ASCII letters under an encoding in
//! letters only ([the names of recomposed
//! definitions](crate::guide::rendering#the-names-of-recomposed-definitions)).
//! The definition therefore cannot replace a command that the
//! `latex-recomposed` output or LaTeX relies on. A macro and an environment of
//! one name are two definitions in FLM and remain two in the output, each under
//! the prefix of its kind. Whether a definition overrides another element plays
//! no part in any of this: the expansion policy alone decides whether a use
//! site is expanded or kept.
//!
//! **More than nine effective arguments: the packed group.** A LaTeX macro body
//! can refer to nine parameters, and values and flags count together, so a
//! definition of six arguments that asks about five of them already has eleven
//! effective arguments. Past nine ([`PLAIN_ARGUMENT_LIMIT`]), a call passes
//! every effective argument in **one** brace group, one pair of braces around
//! each, in the same order: `\flmUDbig{{a}{b}{c}…}`. The definition then takes
//! one parameter, and its body reads effective argument *n* out of it with the
//! style file's accessor: `\flmArg{<n>}{#1}` for a value, `\flmIfArgTF` for a
//! flag and `\flmIfArgBlankTF` for a blank test, which the definition piece
//! (a unit of LaTeX definitions registered under an identifier;
//! [`LatexPreambleDef`](crate::stdcore::backends::latex::LatexPreambleDef))
//! [`preamble::user_args_piece`] defines and a packed definition requires. A
//! definition of nine effective arguments or fewer keeps the plain form, with
//! `#1` to `#9`. No number of arguments prevents recomposition.
//!
//! # The conditionals of a template
//!
//! A template may ask about its own arguments with the conditional commands of
//! the [template language](crate::lang::rewrites::userdefs::template), and
//! what each conditional becomes is decided once, when the style file's
//! definition is written, from the definition's arguments and its effective
//! signature. **No conditional prevents recomposition.** Each becomes one of
//! three things:
//!
//! - the branch its answer selects, where the answer cannot change from one use
//!   site to the next: a question about an argument that is always written, a
//!   question about an argument with a declared default value (which always
//!   has a value), a *value*, *no value* or *provided* question whose test
//!   argument is not exactly one placeholder, or a blank test whose test
//!   argument holds content other than placeholders, or nothing;
//! - LaTeX's `\IfBooleanTF` over the flag the call passes, where the answer
//!   depends on whether the use site wrote the argument (a *provided*
//!   question, or a *value* or *no value* question about an argument that can
//!   be left out and has no default value);
//! - the style file's blank test (`\flmIfBlankTF`, the definition piece
//!   [`IF_BLANK`](crate::stdcore::backends::latex::preamble::IF_BLANK)) over
//!   the values, for `\ifblank` and `\notblank`.
//!
//! What a conditional becomes is written into the copy of the template that
//! `emit-call` lowers (section "The copy of the template" below), so a use
//! site carries none of it.
//!
//! A placeholder is written as the LaTeX parameter of its argument's value,
//! `#1` to `#9`. Every value group holds what FLM substitutes for that
//! placeholder, so nothing needs a guard or a second form: the value group of a
//! `*` marker holds the `*` the author wrote, LaTeX's `-NoValue-` marker never
//! reaches a recomposed body, and a blank test over several placeholders is one
//! test over their values together.
//!
//! # Use sites inside mathematics
//!
//! The `math-passthrough` feature's rules for the `latex-recomposed` backend write every
//! mathematics region (`\(…\)`, `\[…\]`, each line of a display-math
//! environment) from its nodes, through this module's mathematics writer
//! ([`write_math_run`]): the source byte for byte, except at a use site of a
//! user definition, a source comment and a text command (`\text{…}`, whose
//! argument is document text). The mathematics itself is never lowered
//! through the rules; a source comment and a text command are, so that the
//! comment is written by the writer of LaTeX output that every LaTeX backend
//! shares ([`LatexSink`](crate::stdcore::backends::latex::LatexSink)) and the
//! text is escaped as
//! text anywhere else is. A use site of a
//! recomposable definition is written as its canonical call (the definition's
//! recomposed name and one brace group per effective argument, the content of
//! each group written by the same writer, so that a use site nested in an
//! argument is handled too), and the definition construct is staged before it,
//! exactly as `emit-call` stages it in text. A definition that applies in
//! mathematics mode alone, and a definition used both in text and inside
//! mathematics, therefore each reach the style file once. A use site of a
//! definition that is not recomposable, or whose template needs a
//! placeholder's characters, is written as its expansion, the one
//! `instantiate-at-lowering` computes, written back as LaTeX source. A template for
//! mathematics mode is written into its definition construct by the same
//! writer, each placeholder as its parameter.
//!
//! # The copy of the template
//!
//! A placeholder node of a compiled template (`#1`, `#{name}`) carries only the
//! reference as written; what it refers to is recorded beside the compiled
//! template, keyed by the node, and value catch-up does not keep that key. So
//! `emit-call` lowers a copy of the template's nodes, made by this module, in
//! which every placeholder node is replaced by a childless copy whose element
//! value is a [`LatexPoint`] (the LaTeX text that a copy of a template writes
//! at one of its placeholders). The LaTeX family's placeholder rule
//! ([`PlaceholderLatexRule`](crate::stdcore::backends::latex::PlaceholderLatexRule))
//! writes that text and nothing else: the parameter of the argument's value,
//! `#<i + 1>` for the argument at index `i` (in a packed definition, the
//! accessor `\flmArg{<i + 1>}{#1}`), the same for a numbered and a named
//! reference, since both refer to an argument index, with the definition's
//! argument number offset applied. The same copy replaces each
//! conditional by the parts of the LaTeX it becomes, each a `LatexPoint` node,
//! with the branches it keeps between them. The body placeholder of an
//! environment whose template is cut in two never reaches the rule: `emit-call`
//! cuts the template at it before lowering the two halves.
//!
//! The copy belongs to this module rather than to the template compiler, so
//! that the compiled template stays as the language layer made it (one element
//! value shared by every placeholder), and every decision about what the
//! definition's body says is taken in one place, where the copy is made.
//!
//! # Items
//!
//! - [`InstantiateAtLowering`] and [`instantiate_at_lowering_rule`], with
//!   [`INSTANTIATE_AT_LOWERING`] and [`instantiate_rule_name`].
//! - [`EmitCall`] and [`emit_call_rule`], with [`EMIT_CALL`] and
//!   [`emit_call_rule_name`].
//! - [`record_recomposability`], called by the `userdefs` feature when the
//!   registry is built.
//! - [`KeptTemplateDeclaresPoint`], the condition of a template that declares
//!   a reference point.
//! - [`write_math_run`], [`stage_math_run`] and [`MathPiece`], the mathematics
//!   writer that the `math` library's line writer and the `math-passthrough`
//!   feature's rules call.
//! - [`register`], the peer declarations the `userdefs` feature registers once.

// The settlement of the contest between the three rules, the recomposability
// rule and where it is computed, and the stamped copy in place of an edit to
// the template compiler: see design rationale at
// [§flm-dd-dr:rend-kept-default], [§flm-dd-dr:std-userdefs-transcribable] and
// [§flm-dd-dr:rend-definition-construct-in-flow]. The call shape — one brace
// group per effective argument, values then flags — and the reason LaTeX is
// left nothing to parse: see decision rationale at
// [§flm-dd-dr:rend-latex-userdef-canonical-call]. How each conditional of a
// template is classified: see decision rationale at
// [§flm-dd-dr:std-userdefs-conditional-recomposition].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use techy::core::Lang;
use techy::core::node::{BuildId, NodeKind, NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::latexlike::{InvocationSyntaxData, Mode};
use techy::source::SourceSpan;
use techy::transform::{Restage, RestageContext, RestageError, RestageVisitor};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::copy::{ValueContent, copy_run_into_value};
use crate::lang::flm::FlmCallableType;
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::{DefinitionShape, FlmArgumentParser, is_math_text_command};
use crate::lang::rewrites::userdefs::definition::{is_tack_on, substituted_argument_nodes};
use crate::lang::rewrites::userdefs::instantiate::{is_blank_node, is_noise};
use crate::lang::rewrites::userdefs::template::{Branches, Conditional, ConditionalTest};
use crate::lang::rewrites::userdefs::{
    CompiledTemplate, Definition, DefinitionKind, DefinitionModes, EffectiveSignature,
    ExpandOptions, ExpandPolicy, ExpandSet, ModeVariant, NotRecomposable, PlaceholderTarget,
    RegistryBuildFacts, UserDefinition, expand,
};
use crate::lang::mint::is_field_named;
use crate::lang::text::read_chars;
use crate::lang::{Flm, FlmDiagnostics};
use crate::model::aspects::Referenceable;
use crate::model::element::{element_of, element_value};
use crate::model::names::{ElementName, Name, RuleName};
use crate::stdcore::backends::latex::construct::{
    ARG_SLOT, BEGIN_SLOT, DEFINITION, END_SLOT, ENVIRONMENT, MACRO, PLAIN_ARGUMENT_LIMIT, RAW,
    stamp_point,
};
use crate::stdcore::backends::latex::preamble;
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexDefinitionKind, LatexDefinitionMode, LatexDefinitionPayload,
    LatexEnvironmentPayload, LatexMacroPayload, LatexPoint, LatexRawPayload, is_family_name,
    recomposed_name,
};
use crate::stdcore::backends::latex_recomposed::{BACKEND, LatexRecomposedRenderOptions};
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::backends::pres::{BODY_SLOT, recompose_source};
use crate::stdcore::features::refs::{LABEL_FIELD, TARGET_ARG};

use super::{KEPT, element_name};

/// The last segment of the name of every `instantiate-at-lowering` rule,
/// `instantiate-at-lowering`.
pub const INSTANTIATE_AT_LOWERING: &str = "instantiate-at-lowering";

/// The last segment of the name of every `emit-call` rule, `emit-call`.
pub const EMIT_CALL: &str = "emit-call";

/// What a canonical call writes for the flag of an argument the use site
/// wrote: `\BooleanTrue`, LaTeX's own true value, which `\IfBooleanTF` reads.
const BOOLEAN_TRUE: &str = "\\BooleanTrue";

/// What a canonical call writes for the flag of an argument the use site did
/// not write, `\BooleanFalse`.
const BOOLEAN_FALSE: &str = "\\BooleanFalse";

/// Returns the name of the rule of `definition` whose last segment is
/// `variant`: `flm.userdefs.<kind>.<name>.lower.<variant>`, the definition's
/// element name followed by `.lower.` and `variant`.
fn variant_rule_name(definition: &Definition, variant: &str) -> RuleName {
    RuleName::from(format!("{}.lower.{variant}", element_name(definition)))
}

/// Returns the name of the `instantiate-at-lowering` rule of `definition`:
/// `flm.userdefs.<kind>.<name>.lower.instantiate-at-lowering`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, instantiate_rule_name};
///
/// let definition = Definition::macro_("foo", Vec::new());
/// assert_eq!(
///     instantiate_rule_name(&definition),
///     "flm.userdefs.macro.foo.lower.instantiate-at-lowering"
/// );
/// ```
pub fn instantiate_rule_name(definition: &Definition) -> RuleName {
    variant_rule_name(definition, INSTANTIATE_AT_LOWERING)
}

/// Returns the name of the `emit-call` rule of `definition`:
/// `flm.userdefs.<kind>.<name>.lower.emit-call`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, emit_call_rule_name};
///
/// let definition = Definition::environment("note", Vec::new());
/// assert_eq!(emit_call_rule_name(&definition), "flm.userdefs.environment.note.lower.emit-call");
/// ```
pub fn emit_call_rule_name(definition: &Definition) -> RuleName {
    variant_rule_name(definition, EMIT_CALL)
}

// --- recomposability -----------------------------------------------------------------------------

/// Returns the compiled templates a definition is recomposed from: those for
/// text mode, or those for mathematics mode when the definition applies in
/// mathematics mode alone. `None` for a definition without compiled
/// templates.
fn recomposed_variant(definition: &UserDefinition) -> Option<&ModeVariant> {
    let compiled = definition.compiled()?;
    compiled.variant(Mode::Text).or_else(|| compiled.variant(Mode::Math))
}

/// Returns the token of an argument shape that a recomposed definition cannot
/// take, when the argument's parser reports that no LaTeX argument expresses it
/// ([`DefinitionShape::NoShape`]); `None` otherwise.
///
/// **No standard argument parser is refused**: each returns
/// [`DefinitionShape::Value`]. A canonical call writes one brace group per
/// effective argument, so the form of an argument in the FLM source never
/// reaches LaTeX: a key-value argument is a value like any other, since FLM
/// never reads the keys separately, and a verbatim argument is the characters
/// the use site wrote between its delimiters, lowered in its value group like
/// any other content.
///
/// A *tack-on* argument (the trailing argument that absorbs the macros written
/// after a call, such as a `\label{…}`) is not refused either: a definition may
/// declare one and is still recomposed. It carries no effective argument,
/// because no template may refer to it (see the module documentation), so a
/// call writes no group for it.
///
/// The argument parser decides
/// ([`FlmArgumentParser::definition_shape`]), so a parser that a later version
/// adds gives its own answer, and a parser that no LaTeX argument expresses
/// makes its definition expanded rather than written as a LaTeX definition.
fn shape_token(kind: &dyn FlmArgumentParser) -> Option<&'static str> {
    match kind.definition_shape() {
        DefinitionShape::Value => None,
        DefinitionShape::NoShape { token } => Some(token),
    }
}

// --- the effective signature, and what each conditional becomes ------------------------------------

/// Whether an argument parsed by `kind` can be left out at a use site, so that
/// a template can meaningfully ask whether the use site wrote it. The parser
/// decides ([`can_match_empty`](techy::core::constructs::ArgumentParser::can_match_empty)).
///
/// A mandatory argument, a characters-only argument and a verbatim argument
/// are always written (a verbatim argument is delimited text the use site must
/// write), so every question about one is decided when the style file is
/// written. The three standard kinds that can be left out are an optional
/// argument, a key-value argument (parsed exactly like an optional one) and a
/// `*` marker. A tack-on argument is absent where the use site wrote no field
/// macro after the call, but no template can ask about one (a reference to it
/// refuses the definition, which is then not compiled), so the question is
/// never asked about it.
// Why techy's own answer rather than a question of FLM's trait: see decision
// rationale at [§flm-dd-dr:lang-arg-absent-techy].
fn can_be_absent(kind: &Arc<dyn FlmArgumentParser>) -> bool {
    kind.can_match_empty()
}

/// Returns the one node of a conditional's test argument that a value test
/// reads, when there is exactly one; `None` otherwise. Comments and whitespace
/// around it are skipped, as the tests themselves skip them.
fn single_test_node<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeRef<'t, Flm>> {
    let argument = node.argument_content_nodes(0)?;
    let mut significant = argument.iter().filter(|child| !is_noise(child));
    let first = significant.next()?;
    significant.next().is_none().then_some(first)
}

/// Returns the argument that a placeholder node of `template` refers to, as an
/// index in the order the arguments are written; `None` for any other node, a
/// placeholder of the body included.
fn placeholder_argument(template: &CompiledTemplate, node: &NodeRef<'_, Flm>) -> Option<usize> {
    if node.callable_type() != Some(FlmCallableType::Placeholder) {
        return None;
    }
    match template.placeholder(node.id())?.target {
        PlaceholderTarget::Argument(index) => Some(index),
        _ => None,
    }
}

/// One definition as the recomposition writes it: its declaration, the
/// compiled templates recomposed, and the effective signature its calls
/// write.
///
/// Every decision about what the LaTeX definition's body says and what one
/// call writes is made through this type: which effective argument holds an
/// argument's value, which one holds its flag, and what each conditional of
/// the template becomes.
struct Recomposition<'t> {
    definition: &'t Definition,
    variant: &'t ModeVariant,
    signature: &'t EffectiveSignature,
}

impl Recomposition<'_> {
    /// Returns the parser the declared argument `index` was declared with;
    /// `None` when the definition declares no such argument.
    fn kind(&self, index: usize) -> Option<&Arc<dyn FlmArgumentParser>> {
        self.definition.args.get(index).map(|arg| &arg.kind)
    }

    /// Whether the declared argument `index` has a default value, which is
    /// written at every call whose use site leaves the argument out.
    fn has_default(&self, index: usize) -> bool {
        self.variant.default_of(index).is_some()
    }

    /// Returns the effective argument that holds the value of the declared
    /// argument `index`, counting from one: the parameter by which the
    /// definition's body refers to it.
    fn value(&self, index: usize) -> Option<usize> {
        self.signature.value_position(index)
    }

    /// Returns the effective argument that holds the flag of the declared
    /// argument `index`, counting from one, when the template asks whether the
    /// use site wrote that argument.
    fn flag(&self, index: usize) -> Option<usize> {
        self.signature.flag_position(index)
    }

    /// Whether a call of this definition **packs** its arguments: passes every
    /// effective argument in one brace group, one pair of braces around each,
    /// because there are more of them than a LaTeX macro body can refer to
    /// ([`PLAIN_ARGUMENT_LIMIT`]).
    fn packs(&self) -> bool {
        self.signature.arity() > PLAIN_ARGUMENT_LIMIT
    }

    /// Returns how the definition's body reads the effective argument
    /// `position`, counting from one: the parameter `#<position>`, or, when
    /// the call packs its arguments, the style file's accessor over the one
    /// parameter the call passes (`\flmArg{<position>}{#1}`).
    fn read(&self, position: usize) -> String {
        if self.packs() {
            format!("\\{}{{{position}}}{{#1}}", preamble::ARG_COMMAND)
        } else {
            parameter_of(position)
        }
    }

    /// Returns how many LaTeX parameters the definition declares for its
    /// effective arguments: one each, or one for the group when the call packs
    /// them ([`packs`](Recomposition::packs)). An environment that takes its
    /// body as an argument declares one more, after these
    /// ([`body_parameter`](Recomposition::body_parameter)).
    ///
    /// The emitter writes the definition's head from the same count, read from
    /// the definition payload
    /// ([`LatexDefinitionPayload::args`](crate::stdcore::backends::latex::LatexDefinitionPayload::args)
    /// and its `packed` flag), so the two always agree.
    fn latex_parameters(&self) -> usize {
        if self.packs() { 1 } else { self.signature.arity() }
    }

    /// Returns how an environment's body reaches the LaTeX the definition is
    /// written as ([`BodyForm`]); `None` for a macro, which has no body.
    fn body_form(&self) -> Option<BodyForm> {
        (self.definition.kind == DefinitionKind::Environment)
            .then(|| body_form(self.variant.template()))
    }

    /// Returns the LaTeX parameter, counting from one, that holds the body of
    /// an environment that takes its body as an argument; `None` when the body
    /// is not an argument (a macro, or an environment whose template is cut in
    /// two at its body placeholder).
    ///
    /// The body is declared after every effective argument
    /// ([`latex_parameters`](Recomposition::latex_parameters)), so it is the
    /// last parameter, and it is never part of a packed group: a packed
    /// environment declares one parameter for the group and one for the body,
    /// which is `#2`.
    fn body_parameter(&self) -> Option<usize> {
        (self.body_form()? == BodyForm::Argument).then(|| self.latex_parameters() + 1)
    }

    /// Whether a flag of the call carries the answer to the question `test`
    /// about the declared argument `index`, rather than the answer being
    /// decided when the style file is written.
    ///
    /// Both the flags of an effective signature ([`effective_signature`]) and
    /// the LaTeX of a conditional ([`Recomposition::conditional_latex`]) are
    /// computed with this function, so the two cannot disagree. An argument
    /// that is always written needs no flag; a *provided* question ("did the use
    /// site write it?") always needs one; and a *no value* or *value* question
    /// needs one exactly when the argument has no declared default value,
    /// because the question is then the same as *provided* (with a default
    /// value there is always a value, regardless of what the use site wrote).
    fn flag_answers(&self, test: ConditionalTest, index: usize) -> bool {
        if !self.kind(index).is_some_and(can_be_absent) {
            return false;
        }
        match test {
            ConditionalTest::Boolean => true,
            ConditionalTest::NoValue | ConditionalTest::Value => !self.has_default(index),
            ConditionalTest::Blank | ConditionalTest::NotBlank => false,
        }
    }

    /// Returns what the test argument of the conditional `node` holds
    /// ([`TestArgument`]).
    fn test_argument(&self, node: &NodeRef<'_, Flm>) -> TestArgument {
        let template = self.variant.template();
        let mut found = TestArgument::default();
        if let Some(argument) = node.argument_content_nodes(0) {
            for child in argument.iter() {
                if is_blank_node(&child) {
                    continue;
                }
                match placeholder_argument(template, &child) {
                    Some(index) if index < self.signature.values() => {
                        found.placeholders.push(index);
                    }
                    _ => found.other_content = true,
                }
            }
        }
        found.single = found.placeholders.len() == 1
            && single_test_node(node)
                .is_some_and(|single| placeholder_argument(template, &single).is_some());
        found
    }

    /// Returns what the recomposition writes for the conditional `conditional`
    /// whose test argument holds `arg`.
    ///
    /// Three tests (*no value*, *value*, *provided*) read a test argument that
    /// is exactly one placeholder, and give "no" (for *value*, "yes") for any
    /// other test argument, so a test argument of any other form is decided
    /// here rather than when LaTeX runs. Where the question is about an
    /// argument the use site may or may not have written, the call carries the
    /// answer as a flag, and the body reads it with LaTeX's `\IfBooleanTF`.
    /// Where the answer cannot change from one use site to the next, the
    /// conditional is replaced by the branch it selects and nothing of the test
    /// is written. The blank test reads its whole test argument: anything that
    /// is neither a comment nor a placeholder decides the answer as "not
    /// blank". What is left is a run of placeholders, whose values the style
    /// file's blank test tests together; this is correct, since the run of
    /// value groups holds no token exactly when each of them holds none.
    fn conditional_latex(
        &self,
        conditional: Conditional,
        arg: &TestArgument,
    ) -> ConditionalLatex {
        match conditional.test {
            ConditionalTest::NoValue | ConditionalTest::Value | ConditionalTest::Boolean => {
                let single = arg.single.then(|| arg.placeholders.first().copied()).flatten();
                let Some(index) = single else {
                    // Nothing to substitute is the one thing a test argument
                    // that is not a single placeholder cannot be, so *no value*
                    // fails, *value* holds, and *provided* fails.
                    return ConditionalLatex::Settled(conditional.test == ConditionalTest::Value);
                };
                let flag =
                    self.flag_answers(conditional.test, index).then(|| self.flag(index)).flatten();
                match flag {
                    // The call wrote `\BooleanTrue` or `\BooleanFalse` there;
                    // *no value* is the question the other way round.
                    Some(parameter) => ConditionalLatex::Flag {
                        parameter,
                        invert: conditional.test == ConditionalTest::NoValue,
                    },
                    // A mandatory argument is always written, and an argument
                    // with a declared default always has a value: both answer
                    // "no" to *no value* and "yes" to the other two.
                    None => {
                        ConditionalLatex::Settled(conditional.test != ConditionalTest::NoValue)
                    }
                }
            }
            ConditionalTest::Blank | ConditionalTest::NotBlank => {
                let invert = conditional.test == ConditionalTest::NotBlank;
                if arg.other_content {
                    // Anything that is not a comment and not a placeholder is
                    // content, so the test argument is not blank.
                    return ConditionalLatex::Settled(invert);
                }
                if arg.placeholders.is_empty() {
                    // Nothing at all, or nothing but comments.
                    return ConditionalLatex::Settled(!invert);
                }
                let parameters =
                    arg.placeholders.iter().filter_map(|index| self.value(*index)).collect();
                ConditionalLatex::Blank { parameters, invert }
            }
        }
    }

    /// Whether the definition's LaTeX calls the style file's blank test, the
    /// definition piece [`preamble::IF_BLANK`]: some conditional of the
    /// template is written as a blank test rather than decided in advance or
    /// read from a flag.
    fn needs_blank_test(&self) -> bool {
        self.variant.template().tree().root().descendants().any(|node| {
            let Some(conditional) = element_value::<Conditional>(&node) else { return false };
            matches!(
                self.conditional_latex(*conditional, &self.test_argument(&node)),
                ConditionalLatex::Blank { .. }
            )
        })
    }
}

/// What one conditional's test argument holds, as far as the recomposition
/// needs it.
#[derive(Debug, Default)]
struct TestArgument {
    /// Every declared argument that a placeholder of the test argument refers
    /// to, in order.
    placeholders: Vec<usize>,
    /// Whether the test argument is exactly one placeholder of a declared
    /// argument, comments and whitespace aside. The three value tests require
    /// this before they give any answer other than "no".
    single: bool,
    /// Whether the test argument holds a node that is neither a comment nor a
    /// placeholder of a declared argument. Whitespace counts here, because
    /// FLM's blank test treats whitespace as content, so such a node decides
    /// that test's answer as "not blank".
    other_content: bool,
}

/// Returns the effective signature of `definition` recomposed from `variant`:
/// one value per declared argument a template can refer to, then one flag per
/// argument whose question needs a flag ([`Recomposition::flag_answers`]).
///
/// A tack-on argument has neither: no template may refer to it, so it carries
/// no value, and a question that cannot be asked needs no flag. It is the
/// command's last argument, so the values are the declared arguments before
/// it, which is the order [`EffectiveSignature`] records.
fn effective_signature(definition: &Definition, variant: &ModeVariant) -> EffectiveSignature {
    let values = definition.args.iter().take_while(|arg| !is_tack_on(arg)).count();
    let plain = EffectiveSignature::new(values, []);
    let recomposition = Recomposition { definition, variant, signature: &plain };
    let template = variant.template();
    let mut flagged = BTreeSet::new();
    for node in template.tree().root().descendants() {
        let Some(conditional) = element_value::<Conditional>(&node) else { continue };
        let Some(single) = single_test_node(&node) else { continue };
        let Some(index) = placeholder_argument(template, &single) else { continue };
        if recomposition.flag_answers(conditional.test, index) {
            flagged.insert(index);
        }
    }
    EffectiveSignature::new(values, flagged)
}

/// What the recomposition writes for one conditional of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConditionalLatex {
    /// The answer does not depend on the use site, so the conditional is
    /// replaced by the branch this answer selects, or by nothing when the
    /// conditional has no branch for it.
    Settled(bool),
    /// LaTeX's `\IfBooleanTF` over the flag the call wrote: the effective
    /// argument that holds the flag, counting from one, and whether LaTeX's
    /// answer is the negation of what FLM asks.
    Flag {
        /// The LaTeX parameter number of the flag, counting from one.
        parameter: usize,
        /// Whether LaTeX's answer is the negation of FLM's.
        invert: bool,
    },
    /// The blank test over the values of the test argument's placeholders,
    /// written with the style file's blank test (`\flmIfBlankTF`).
    Blank {
        /// The LaTeX parameter numbers whose content is tested together.
        parameters: Vec<usize>,
        /// Whether the conditional is `\notblank`.
        invert: bool,
    },
}

/// Returns the LaTeX command a conditional with the branches `branches` is
/// written as, and which of the conditional's branch arguments follow the test,
/// in the order they are written: the command name `family` with the letters
/// of the branches the conditional has, and the branches exchanged when LaTeX's
/// answer is the negation of FLM's.
fn spelling(family: &str, branches: Branches, invert: bool) -> (String, Vec<usize>) {
    match branches {
        Branches::Both if invert => (format!("{family}TF"), vec![2, 1]),
        Branches::Both => (format!("{family}TF"), vec![1, 2]),
        Branches::True => (format!("{family}{}", if invert { "F" } else { "T" }), vec![1]),
        Branches::False => (format!("{family}{}", if invert { "T" } else { "F" }), vec![1]),
    }
}

/// One part of what the copy of a conditional writes, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConditionalPart {
    /// LaTeX of the recomposition's own, byte for byte.
    Latex(String),
    /// The content of one of the conditional's branch arguments, by its index
    /// in invocation order.
    Branch(usize),
}

/// Returns what the copy of the conditional `conditional` writes, in order,
/// given `latex`, what the recomposition decided for it
/// ([`Recomposition::conditional_latex`]).
///
/// A definition whose call packs its arguments ([`Recomposition::packs`])
/// writes the style file's own tests instead of LaTeX's, because the arguments
/// are parts of one group rather than parameters of their own: `\flmIfArgTF`
/// for a flag and `\flmIfArgBlankTF` for a blank test
/// ([`preamble::user_args_piece`]). Both take a *then* and an *else* branch
/// regardless of the branches the template's conditional has, so a
/// conditional with one branch writes an empty group for the other.
fn conditional_parts(
    recomposition: &Recomposition<'_>,
    conditional: Conditional,
    latex: &ConditionalLatex,
) -> Vec<ConditionalPart> {
    // `\<name>{<test>}{<branch>}…`, the last branch closed and no more.
    let wrap = |name: String, test: String, order: Vec<usize>| {
        let mut parts = vec![ConditionalPart::Latex(format!("\\{name}{{{test}}}{{"))];
        for (at, branch) in order.iter().enumerate() {
            parts.push(ConditionalPart::Branch(*branch));
            let close = if at + 1 == order.len() { "}" } else { "}{" };
            parts.push(ConditionalPart::Latex(String::from(close)));
        }
        parts
    };
    // `\<name>{<test>}{#1}{<then>}{<else>}`, the branch LaTeX's answer selects
    // first and an empty group where the template's conditional has none.
    let wrap_packed = |name: &str, test: String, invert: bool| {
        let mut parts = vec![ConditionalPart::Latex(format!("\\{name}{{{test}}}{{#1}}{{"))];
        for (at, holds) in [!invert, invert].into_iter().enumerate() {
            if let Some(branch) = conditional.branch_index(holds) {
                parts.push(ConditionalPart::Branch(branch));
            }
            let close = if at == 0 { "}{" } else { "}" };
            parts.push(ConditionalPart::Latex(String::from(close)));
        }
        parts
    };
    match latex {
        ConditionalLatex::Settled(holds) => conditional
            .branch_index(*holds)
            .map(|index| vec![ConditionalPart::Branch(index)])
            .unwrap_or_default(),
        ConditionalLatex::Flag { parameter, invert } => {
            if recomposition.packs() {
                return wrap_packed(preamble::IF_ARG_COMMAND, parameter.to_string(), *invert);
            }
            let (name, order) = spelling("IfBoolean", conditional.branches, *invert);
            wrap(name, parameter_of(*parameter), order)
        }
        ConditionalLatex::Blank { parameters, invert } => {
            if recomposition.packs() {
                // The arguments tested together, as the comma-separated list
                // the style file's blank test reads.
                let test =
                    parameters.iter().map(usize::to_string).collect::<Vec<String>>().join(",");
                return wrap_packed(preamble::IF_ARG_BLANK_COMMAND, test, *invert);
            }
            let (_, order) = spelling("", conditional.branches, *invert);
            let test: String = parameters.iter().copied().map(parameter_of).collect();
            wrap(String::from(preamble::BLANK_COMMAND), test, order)
        }
    }
}

/// Returns the LaTeX parameter `number`, counting from one: `#1`, `#2`. This
/// is the only form of a parameter, for a placeholder of an argument and for
/// the test of a conditional alike.
fn parameter_of(number: usize) -> String {
    format!("#{number}")
}

/// Checks that every placeholder of a compiled tree (a template or a default
/// value) is a placeholder of a declared argument or of the body, written
/// without a substitution argument, as recomposition requires; returns
/// [`NotRecomposable::HookPlaceholder`] otherwise.
///
/// Where a placeholder is placed plays no part here: one placed where a rule
/// needs its characters is found when the template is lowered, and the use
/// sites are expanded then (see the [module
/// documentation](self#where-a-rule-needs-an-arguments-characters)).
///
/// No conditional is refused here or anywhere else: each of the fourteen is
/// either decided when the style file is written, written as a LaTeX test over
/// a flag the call carries ([`Recomposition::conditional_latex`]), or, in a
/// default value, evaluated at the use site where the default value is
/// instantiated ([`resolved_default`]).
fn check_placeholders(template: &CompiledTemplate) -> Result<(), NotRecomposable> {
    for node in template.tree().root().descendants() {
        if node.callable_type() != Some(FlmCallableType::Placeholder) {
            continue;
        }
        // Neither branch below is reachable through a `UserDefinition` today,
        // so no test exercises them: its vocabulary holds the arguments and
        // the body and no hook-answered name, a substitution argument on a
        // base reference is refused at compilation (the definition is then
        // `NotCompiled`), and every placeholder node a compiled template
        // holds is in its placeholder map. They guard an `Expand`
        // implementation that answers placeholders of its own.
        let Some(reference) = template.placeholder(node.id()) else {
            return Err(NotRecomposable::HookPlaceholder {
                reference: node.name().unwrap_or("").to_string(),
            });
        };
        if reference.substitution || matches!(reference.target, PlaceholderTarget::Extra(_)) {
            return Err(NotRecomposable::HookPlaceholder { reference: reference.written.clone() });
        }
    }
    Ok(())
}

/// Whether `node` is the body placeholder of `template`: the placeholder that
/// stands for what a use site of the environment writes between `\begin` and
/// `\end`.
fn is_body_placeholder(template: &CompiledTemplate, node: &NodeRef<'_, Flm>) -> bool {
    template
        .placeholder(node.id())
        .is_some_and(|reference| reference.target == PlaceholderTarget::Body)
}

/// How a recomposed environment's body reaches the LaTeX the environment is
/// written as.
///
/// LaTeX sets an environment's body between the begin code and the end code.
/// A template that refers to the body once, among its own top-level nodes, is
/// cut in two at that place ([`BodyForm::Split`]). A template that refers to
/// the body anywhere else (inside a group, inside another command's argument,
/// or in two places) cannot be cut that way, so the environment takes the
/// body as an argument instead ([`BodyForm::Argument`]) and the template is
/// written whole. A template that does not refer to the body at all takes
/// the same argument form and never writes the argument, so LaTeX reads the
/// body and drops it, as FLM's own expansion does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyForm {
    /// The template refers to the body exactly once among its top-level nodes:
    /// the template is cut there into the begin code and the end code, and
    /// LaTeX sets the body between them.
    Split,
    /// The template refers to the body somewhere else, more than once, or not
    /// at all: the environment declares LaTeX's body argument `+b` as its last
    /// argument, LaTeX passes the body in it, and the whole template is the
    /// begin code, with the body written as that argument's parameter. Where
    /// the template does not refer to the body, the begin code never writes
    /// that parameter, so LaTeX reads the body and drops it, as the expansion
    /// does; [`body_form`] states the one difference that remains.
    Argument,
}

/// Returns how the body of an environment with the template `template`
/// reaches the LaTeX the environment is written as ([`BodyForm`]).
///
/// A template that does not refer to the body takes it as an argument too,
/// and the begin code never writes that argument: LaTeX reads the body, passes
/// it, and the definition drops it, as FLM's own expansion does. One difference
/// remains: LaTeX *reads* what it drops, so a body whose braces or
/// environments do not balance is a LaTeX error, where the expansion drops it
/// unread (such content would not have parsed in FLM in the first place).
///
/// This function is the only place where the form is decided, so that the
/// environment's argument specification, its begin and end code and the
/// parameter a body placeholder is written as cannot disagree.
fn body_form(template: &CompiledTemplate) -> BodyForm {
    let root = template.tree().root();
    let mut form = None;
    for node in root.descendants() {
        if !is_body_placeholder(template, &node) {
            continue;
        }
        let at_top_level = node.parent().is_some_and(|parent| parent.id() == root.id());
        form = match (form, at_top_level) {
            // The first body placeholder, at the top level: splitting is still
            // possible. A second one of any kind, or one anywhere else, settles
            // the answer at the body-grabbing form.
            (None, true) => Some(BodyForm::Split),
            _ => Some(BodyForm::Argument),
        };
    }
    // A template that names the body nowhere grabs it and drops it.
    form.unwrap_or(BodyForm::Argument)
}

/// Checks that the template's own content allows recomposition: the check of
/// [`check_placeholders`], which applies to every compiled tree. Nothing about
/// an environment's body prevents recomposition: [`body_form`] gives a form
/// for every template, including one that does not refer to its body.
///
/// A *verbatim block* in the template's own content does not prevent it
/// either: it is written into the definition's body as a call of
/// `\flmVerbatimBlock`, an ordinary macro call over brace groups, which can be
/// written wherever any other call can.
fn check_template(template: &CompiledTemplate) -> Result<(), NotRecomposable> {
    check_placeholders(template)
}

/// Returns the effective signature of the definition `value` when nothing in
/// the definition alone prevents recomposition, or the reason it cannot be
/// recomposed. The definition alone is its expansion policy, its compilation,
/// its kind, its arguments, its template and the default value of each of its
/// arguments.
///
/// Another definition of the same kind and name, declared for the other
/// parsing mode, is no reason: the two are per-mode definitions and are written
/// under two names of their own ([`another_definition_shares_its_name`],
/// [`latex_mode`]). Nor is a definition of the same name and the other kind (a
/// macro beside an environment), since each kind is written under a prefix of
/// its own, so the two names cannot collide.
///
/// The caller records the effective signature ([`effective_signature`]) on the
/// definition's element data: every call and the definition itself are written
/// from it.
fn intrinsic(value: &UserDefinition) -> Result<EffectiveSignature, NotRecomposable> {
    let definition = value.definition();
    if definition.expand == ExpandPolicy::Always {
        return Err(NotRecomposable::AlwaysExpanded);
    }
    let Some(variant) = recomposed_variant(value) else {
        return Err(NotRecomposable::NotCompiled);
    };
    if definition.kind == DefinitionKind::Specials {
        return Err(NotRecomposable::Specials);
    }
    for (index, arg) in definition.args.iter().enumerate() {
        if let Some(shape) = shape_token(&*arg.kind) {
            return Err(NotRecomposable::ArgumentShape { index, shape: String::from(shape) });
        }
    }
    check_template(variant.template())?;
    // A default is written at the call site, in its argument's own group,
    // wherever the use site left the argument out, and it is resolved there by
    // the use site's own bindings ([`resolved_default`]) — so a default
    // holding placeholders of other arguments or conditionals of its own is
    // written like any other content, and the only thing it may not hold is a
    // placeholder nothing at the use site answers. A verbatim block is no
    // reason here either: what the default resolves to is written into its
    // argument's own value group at the call, and a verbatim block in it is
    // written there as the escaped call, like any other value.
    for (_, default) in variant.defaults() {
        check_placeholders(default)?;
    }
    Ok(effective_signature(definition, variant))
}

/// Whether another definition of `roster` declares the same kind and name as
/// `value`. The two are then per-mode definitions, one for each parsing mode,
/// which FLM allows because their modes do not overlap.
///
/// The `latex-recomposed` backend writes each of them under a name of its own
/// ([`latex_mode`]), so this is a fact the recomposition needs, not a reason
/// against recomposition.
fn another_definition_shares_its_name(
    value: &UserDefinition,
    roster: &[Arc<UserDefinition>],
) -> bool {
    let definition = value.definition();
    roster.iter().any(|other| {
        !std::ptr::eq(&**other, value)
            && other.definition().kind == definition.kind
            && other.definition().name == definition.name
    })
}

/// Returns the `per-plan` definitions of `roster` that `value`'s compiled
/// templates (the template and its default values) use, as indices into
/// `roster`, each once, in order of first use.
fn uses_of(value: &UserDefinition, roster: &[Arc<UserDefinition>]) -> Vec<usize> {
    let mut used = Vec::new();
    let Some(variant) = recomposed_variant(value) else { return used };
    let trees = std::iter::once(variant.template()).chain(variant.defaults().map(|(_, t)| t));
    for template in trees {
        for node in template.tree().root().descendants() {
            let Some(other) = element_value::<UserDefinition>(&node) else { continue };
            if other.definition().expand != ExpandPolicy::PerPlan {
                continue;
            }
            if let Some(index) = roster.iter().position(|entry| std::ptr::eq(&**entry, other)) {
                if !used.contains(&index) {
                    used.push(index);
                }
            }
        }
    }
    used
}

/// Computes, for each definition of `roster` (all the definitions the
/// `userdefs` feature registers), the facts the registry build decides about
/// it, and records them on the definition
/// ([`UserDefinition::record_registry_build_facts`]): whether one LaTeX
/// definition can express it (whether it is recomposable) and, if so, the
/// arguments its calls write ([`EffectiveSignature`]); and whether another
/// definition of the same kind and name is declared for the other parsing mode,
/// in which case the recomposition writes the two under two names.
///
/// Nothing is reported. A definition is not recomposable because of its nature
/// (a special character sequence), its author's choice (the expansion policy
/// `always`), a refusal already reported when the registry was built, or the
/// use, in its template or a default value, of a `per-plan` definition that is
/// not recomposable; two further reasons apply only to definitions declared in
/// Rust ([`NotRecomposable`]). None of these needs a report: a refusal was
/// reported when it happened, and the other reasons are not mistakes in the
/// document. A host that wants to know why a definition is written expanded
/// calls [`UserDefinition::recomposable`].
///
/// The `userdefs` feature calls this function from its late-initialization
/// build hook, after the templates are compiled, with all its definitions:
/// two of the facts involve two definitions (a definition of the same name for
/// the other parsing mode, and a use of a definition that is not
/// recomposable), so they need every definition. A definition that uses a
/// definition that is not recomposable is not recomposable itself, however
/// long the chain of uses. A definition that uses **itself**, directly or
/// through a chain of uses, remains recomposable: it is written as a LaTeX
/// definition that calls itself, and LaTeX runs the recursion.
///
/// The facts are recorded once per definition; a second call records nothing
/// new.
pub fn record_recomposability(roster: &[Arc<UserDefinition>]) {
    let uses: Vec<Vec<usize>> = roster.iter().map(|value| uses_of(value, roster)).collect();
    let mut outcomes: Vec<Result<EffectiveSignature, NotRecomposable>> = roster
        .iter()
        .map(|value| intrinsic(value))
        .collect();
    // A use of a non-recomposable definition spreads to the user, until
    // nothing changes.
    loop {
        let mut changed = false;
        for index in 0..roster.len() {
            if outcomes[index].is_err() {
                continue;
            }
            let failing = uses[index].iter().copied().find(|used| outcomes[*used].is_err());
            if let Some(used) = failing {
                outcomes[index] =
                    Err(NotRecomposable::Uses { name: roster[used].definition().display_name() });
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (value, outcome) in roster.iter().zip(outcomes) {
        value.record_registry_build_facts(RegistryBuildFacts {
            recomposition: outcome,
            shares_its_name: another_definition_shares_its_name(value, roster),
        });
    }
}

// --- the stamped copy -----------------------------------------------------------------------------

/// Returns the failure to copy a template as a [`LowerError`].
fn copy_failed(error: impl std::fmt::Display) -> LowerError {
    LowerError::other(format!("copying the template failed: {error}"))
}

/// The visitor that copies a compiled template for a definition construct
/// (see "The copy of the template" in the module documentation).
///
/// It copies every node of the template unchanged, except a placeholder node,
/// which it replaces by a childless copy carrying the LaTeX the placeholder is
/// written as ([`LatexPoint`]), and a conditional node, which it replaces by
/// the parts of the LaTeX the conditional becomes, with the branches it keeps
/// copied between them.
struct Stamp<'r, 't> {
    recomposition: &'r Recomposition<'t>,
}

impl Stamp<'_, '_> {
    /// Stages one [`LatexPoint`] node of the copy: a childless copy of `at`, a
    /// node of the template, carrying `text` as the LaTeX written there
    /// ([`stamp_point`]).
    fn point(
        &self,
        at: &NodeRef<'_, Flm>,
        text: String,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<BuildId, LowerError> {
        stamp_point(at, text, cx)
    }

    /// Copies a conditional node: the parts of the LaTeX it becomes, with the
    /// branches it keeps copied between them. A conditional decided in advance
    /// keeps one branch and writes no LaTeX of its own; every other outcome
    /// writes LaTeX around the test, and each part of that LaTeX is staged as a
    /// copy of the test argument's placeholder.
    fn conditional(
        &mut self,
        node: NodeRef<'_, Flm>,
        conditional: Conditional,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, LowerError> {
        let arg = self.recomposition.test_argument(&node);
        let latex = self.recomposition.conditional_latex(conditional, &arg);
        let parts = conditional_parts(self.recomposition, conditional, &latex);
        let at = node
            .argument_content_nodes(0)
            .and_then(|argument| {
                argument
                    .iter()
                    .find(|child| {
                        placeholder_argument(self.recomposition.variant.template(), child).is_some()
                    })
            });
        let mut ids = Vec::new();
        for part in parts {
            match part {
                ConditionalPart::Latex(text) => {
                    let Some(at) = at else {
                        return Err(LowerError::structure(
                            "a conditional written as a LaTeX test without a placeholder to test",
                        ));
                    };
                    ids.push(self.point(&at, text, cx)?);
                }
                ConditionalPart::Branch(index) => {
                    if let Some(branch) = node.argument_content_nodes(index) {
                        for child in branch.iter() {
                            ids.extend(cx.restage_subtree(child, self).map_err(convert)?);
                        }
                    }
                }
            }
        }
        Ok(Restage::Emit(ids))
    }
}

impl RestageVisitor<Flm, (), ()> for Stamp<'_, '_> {
    type Error = LowerError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, LowerError> {
        if let Some(conditional) = element_value::<Conditional>(&node) {
            return self.conditional(node, *conditional, cx);
        }
        if node.callable_type() != Some(FlmCallableType::Placeholder) {
            return Ok(Restage::Descend(()));
        }
        let Some(index) = placeholder_argument(self.recomposition.variant.template(), &node) else {
            // The body placeholder of an environment that takes its body as an
            // argument: it is written as that argument's parameter, like any
            // other. The body placeholder of an environment whose template is
            // split at it never reaches here, the template being cut in two
            // there before it is copied.
            if is_body_placeholder(self.recomposition.variant.template(), &node) {
                if let Some(parameter) = self.recomposition.body_parameter() {
                    let id = self.point(&node, parameter_of(parameter), cx)?;
                    return Ok(Restage::Emit(vec![id]));
                }
            }
            return Err(LowerError::structure("a placeholder the template does not know"));
        };
        let Some(parameter) = self.recomposition.value(index) else {
            return Err(LowerError::structure("a placeholder of an argument that is not declared"));
        };
        let id = self.point(&node, self.recomposition.read(parameter), cx)?;
        Ok(Restage::Emit(vec![id]))
    }
}

/// The visitor that produces the copied template as a synthesized value: it is
/// called once, at the root of the template's tree, and stages a new list node
/// over the copies of `nodes` ([`Stamp`]).
struct StampedCopy<'n, 'r, 't> {
    nodes: &'n [NodeRef<'t, Flm>],
    recomposition: &'r Recomposition<'t>,
}

impl RestageVisitor<Flm, (), ()> for StampedCopy<'_, '_, '_> {
    type Error = LowerError;

    fn restage(
        &mut self,
        root: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, LowerError> {
        let mut stamp = Stamp { recomposition: self.recomposition };
        let mut children = Vec::with_capacity(self.nodes.len());
        for node in self.nodes {
            children.extend(cx.restage_subtree(*node, &mut stamp).map_err(convert)?);
        }
        let (span, state) = match (self.nodes.first(), self.nodes.last()) {
            (Some(first), Some(last)) if first.span().same_source(last.span()) => (
                SourceSpan::new(first.span().source(), first.span().start()..last.span().end()),
                Arc::clone(first.parsing_state()),
            ),
            (Some(first), _) => (first.span().clone(), Arc::clone(first.parsing_state())),
            _ => {
                let at = root.span();
                (
                    SourceSpan::new(at.source(), at.start()..at.start()),
                    Arc::clone(root.parsing_state()),
                )
            }
        };
        let kind = NodeKind::list();
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&children))
            .map_err(|error| LowerError::Build { detail: error.to_string() })?;
        let id = cx
            .builder()
            .add(kind, span, state, children, ext, ())
            .map_err(|error| LowerError::Build { detail: error.to_string() })?;
        Ok(Restage::Emit(vec![id]))
    }
}

/// Returns a failure of the template copy as the rule's own failure: the
/// visitor's error as it is, or any other failure as text.
fn convert(error: RestageError<LowerError>) -> LowerError {
    match error {
        RestageError::Visitor(error) => error,
        other => copy_failed(other),
    }
}

/// Returns a run of a compiled template's top-level nodes copied into a
/// synthesized value of its own ([`StampedCopy`]), with every placeholder and
/// conditional replaced as [`Stamp`] does: the value's root is a list over the
/// copies.
fn stamped_value<'t>(
    recomposition: &Recomposition<'t>,
    template: &'t CompiledTemplate,
    nodes: &[NodeRef<'t, Flm>],
) -> Result<NodeTree<Flm>, LowerError> {
    let mut copy = StampedCopy { nodes, recomposition };
    guard::restager(&mut copy, GuardPolicy::StackBudget).restage(template.tree()).map_err(convert)
}

/// Stages the content of one content slot of a definition construct from a run
/// of a compiled template's top-level nodes, and returns the staged nodes.
///
/// For a template for text mode, the copy of the run is spliced
/// ([`Lowerer::splice`]): brought up to date by value catch-up and lowered
/// through the rules, so that anything the template holds is lowered as
/// document content is. For a template for mathematics mode, the copy is
/// written by the mathematics writer ([`write_math_run`]): the source byte for
/// byte, each placeholder as its parameter, each use site handled.
///
/// A run for text mode that declares a reference point is refused with
/// [`KeptTemplateDeclaresPoint`] (see the [module
/// documentation](self#a-template-that-declares-a-reference-point)): value
/// catch-up refuses a synthesized value that states a fact about the document,
/// and a `\label` whose text holds a placeholder, which value catch-up does not
/// see as a label, is refused before the splice.
fn template_run<'t>(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    recomposition: &Recomposition<'t>,
    template: &'t CompiledTemplate,
    nodes: &[NodeRef<'t, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let value = stamped_value(recomposition, template, nodes)?;
    if template.mode() == Mode::Math {
        let copies: Vec<NodeRef<'_, Flm>> = value.root().children().iter().collect();
        let mut writer = MathWriter::new(cx, lw, Nodes::Synthesized);
        writer.run(&copies)?;
        let pieces = writer.finish();
        stage_math_pieces(cx, lw, pieces)
    } else {
        // A label whose spelling a template substitution completes is a label
        // all the same, but it reads as no label, so the catch-up below sees no
        // fact in it: it is refused here, by the same condition.
        // See decision rationale at [§flm-dd-dr:std-userdefs-kept-points].
        if nodes.iter().any(|node| holds_placeholder_label(node)) {
            return Err(refusal(recomposition.definition, template));
        }
        match lw.splice(cx, &Arc::new(value)) {
            Err(LowerError::DocumentFactInValue { .. }) => {
                Err(refusal(recomposition.definition, template))
            }
            other => other,
        }
    }
}

// --- a template that declares a reference point ---------------------------------------------------

/// Condition: the template of a user definition whose use sites are kept in
/// the tree (not expanded before lowering) declares a reference point that
/// carries a label: a construct followed by `\label`, a labeled xnote, a
/// defined term, or any construct whose reference point carries a label; or it
/// holds a `\label` whose text a placeholder completes.
///
/// A kept definition's template is lowered as a synthesized value: the body of
/// the LaTeX definition that `emit-call` writes, or the expansion that
/// `instantiate-at-lowering` builds for a use site. A label is a fact about
/// the document as a whole (other places refer to it), and a synthesized value
/// may state no such fact, so the template cannot be lowered there (see the
/// [module documentation](self#a-template-that-declares-a-reference-point)).
/// The remedy is the expansion policy [`Always`](ExpandPolicy::Always): the
/// definition's use sites are then expanded in the document before the labels
/// are read, and each label is the document's own. Selecting
/// `instantiate-at-lowering` is no remedy, since its expansion is a
/// synthesized value too.
///
/// Reported as an Error at the node of the template that declares the point
/// (at the start of the template when no node can be identified), once at each
/// use site of the definition. Recovery: the use site lowers to a childless
/// `flm.pres.unsupported` stand-in node ([`LowerError::Refused`]), which the
/// `latex-recomposed` backend writes as a comment.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.userdefs.kept-template-declares-point",
    message = "the template of `{definition}` declares a labeled place that other parts of the \
               document can refer to ({declared_by}), which is allowed only when the definition \
               is expanded in the document itself, before the output is produced; give the \
               definition the expansion policy `always` (`expand: always`), or remove the label \
               from its template"
)]
pub struct KeptTemplateDeclaresPoint {
    /// The definition, as an author writes it (`\titled`).
    pub definition: String,
    /// The node of the template that declares the point, as written and in
    /// backquotes (`` `\section` ``, `` `\label` ``), or the words "somewhere in
    /// its template" when no node can be identified.
    pub declared_by: String,
}

/// Whether `node`, or a node under it, is a `\label` whose text holds a
/// placeholder of the template.
fn holds_placeholder_label(node: &NodeRef<'_, Flm>) -> bool {
    std::iter::once(*node).chain(node.descendants()).any(|node| {
        is_field_named(&node, LABEL_FIELD)
            && node
                .argument_content_nodes_named(TARGET_ARG)
                .ok()
                .flatten()
                .and_then(|content| read_chars(content.iter()).ok())
                .is_some_and(|read| read.has_placeholder())
    })
}

/// Returns the first node of `template`, in document order, that declares a
/// labeled reference point: a construct whose point carries a label, or a
/// `\label` field (the label of a construct whose own point does not show it,
/// because its text holds a placeholder).
fn declaring_node<'t>(template: &'t CompiledTemplate) -> Option<NodeRef<'t, Flm>> {
    template.tree().root().descendants().find(|node| {
        let labeled = element_of(node).aspect::<dyn Referenceable>().is_some_and(|aspect| {
            aspect.points(node).iter().any(|point| !point.labels.is_empty())
        });
        labeled || is_field_named(node, LABEL_FIELD)
    })
}

/// Returns how a node of a template is written in
/// [`KeptTemplateDeclaresPoint`]: a macro or a field as `\name`, an environment
/// as `\begin{name}`.
fn spelled(node: &NodeRef<'_, Flm>) -> String {
    let name = node.name().unwrap_or_default();
    match node.callable_type() {
        Some(FlmCallableType::Environment) => format!("\\begin{{{name}}}"),
        _ => format!("\\{name}"),
    }
}

/// Returns the refusal of a definition whose template declares a labeled
/// reference point: [`KeptTemplateDeclaresPoint`] at the node that declares
/// it, or at the start of the template when no node can be identified. The
/// lowering stage reports it at each use site the rule refuses
/// ([`LowerError::Refused`]).
fn refusal(definition: &Definition, template: &CompiledTemplate) -> LowerError {
    let (span, declared_by) = match declaring_node(template) {
        Some(node) => (node.span().clone(), format!("`{}`", spelled(&node))),
        None => (
            SourceSpan::new(template.source(), 0..0),
            String::from("somewhere in its template"),
        ),
    };
    let condition = KeptTemplateDeclaresPoint { definition: definition.display_name(), declared_by };
    LowerError::Refused(Box::new(Diagnostic::error(condition, span)))
}

// --- instantiating a use site ---------------------------------------------------------------------

/// Returns the expansion of the use site `node` as a synthesized value of its
/// own: the use site copied into a value and expanded over every definition,
/// the `always` ones included (a use site inside a template has had no
/// rewrite stage run over it), under the definition's expansion depth limit
/// ([`UserDefinition::recursion_limit`]). The diagnostics of the expansion (a
/// depth limit reached, a placeholder without a value) are reported through
/// `lw`. [`instantiate`] splices the result, and the mathematics writer
/// writes it as source.
fn expansion_of(
    node: NodeRef<'_, Flm>,
    value: &UserDefinition,
    lw: &mut dyn Lowerer,
) -> Result<Arc<NodeTree<Flm>>, LowerError> {
    // The copy and the expansion run under the fixed stack budget, the
    // policy every registry outside a test runs under: the lowering façade
    // exposes no guard policy, and the use site is one subtree on the
    // lowering stage's worker thread. See design rationale at
    // [§flm-dd-dr:std-userdefs-lowering] and [§flm-dd-dr:lang-stack-budget].
    let copied = copy_run_into_value(&[node], &node, ValueContent::Own, GuardPolicy::StackBudget)
        .map_err(|error| LowerError::other(format!("copying the use site failed: {error}")))?;
    let options =
        ExpandOptions { recursion_limit: value.recursion_limit(), guard: GuardPolicy::StackBudget };
    let mut diags = FlmDiagnostics::new();
    let expanded = expand(&copied, lw.lang(), &options, ExpandSet::Both, &mut diags)
        .map_err(|failure| LowerError::other(failure.to_string()))?;
    for diagnostic in diags.iter() {
        lw.diagnose(diagnostic.clone());
    }
    Ok(expanded)
}

/// Returns the data of the element of the use site `node`
/// ([`UserDefinition`]), which every use site of a user definition carries; a
/// structure error for any other node.
fn value_of<'t>(node: &NodeRef<'t, Flm>) -> Result<&'t UserDefinition, LowerError> {
    element_value::<UserDefinition>(node)
        .ok_or_else(|| LowerError::structure("a user-definition node without its element's data"))
}

/// Instantiates one use site: expands it ([`expansion_of`]) and splices the
/// expansion in its place. This is the body of `instantiate-at-lowering`, and
/// of `emit-call` for a definition that is not recomposable or whose template
/// needs a placeholder's characters (see the module documentation).
///
/// An expansion that declares a labeled reference point is refused with
/// [`KeptTemplateDeclaresPoint`], as a template written as a LaTeX definition
/// is.
fn instantiate(
    node: NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Lowering, LowerError> {
    let value = value_of(&node)?;
    let expanded = expansion_of(node, value, lw)?;
    match lw.splice(cx, &expanded) {
        Ok(ids) => Ok(Lowering::Emit(ids)),
        Err(LowerError::DocumentFactInValue { stage, table }) => match recomposed_variant(value) {
            Some(variant) => Err(refusal(value.definition(), variant.template())),
            None => Err(LowerError::DocumentFactInValue { stage, table }),
        },
        Err(error) => Err(error),
    }
}

/// The body of every `instantiate-at-lowering` rule: it expands the use site
/// with the expand pass and lowers the expansion in its place (see the module
/// documentation). The expansion depth limit is the definition's
/// ([`UserDefinition::recursion_limit`]).
///
/// Build the rule of a definition with [`instantiate_at_lowering_rule`].
#[derive(Debug, Default, Clone, Copy)]
pub struct InstantiateAtLowering;

impl Lower for InstantiateAtLowering {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        instantiate(node, cx, lw)
    }
}

/// Returns the `instantiate-at-lowering` rule of `definition`, a definition
/// whose expansion policy is `per-plan`: the rule targets the definition's
/// element, requires `flm.userdefs.kept`, declares no presentation element of
/// its own (the rules of the expansion's elements stage what it becomes) and
/// is never a default, so a plan request selects it by name. See the module
/// documentation.
pub fn instantiate_at_lowering_rule(definition: &Definition) -> LoweringRule {
    LoweringRule::new(
        instantiate_rule_name(definition),
        element_name(definition),
        LoweringDecl::new().requires(KEPT),
        Arc::new(InstantiateAtLowering),
        DefaultMark::Never,
    )
    .docs(format!(
        "a use site of `{}` kept in the tree is expanded at lowering and the expansion is \
         lowered in its place (requires `flm.userdefs.kept`); never a default, a plan request \
         selects it by name",
        definition.display_name()
    ))
}

// --- emitting a call ------------------------------------------------------------------------------

/// Returns which of a name's definitions `value` is, for the name it is
/// recomposed under ([`LatexDefinitionMode`]).
///
/// A definition declared once is *single*, regardless of the parsing modes it
/// applies in. Where the same kind and name are declared twice
/// ([`UserDefinition::shares_its_name`]), each of the two per-mode definitions
/// takes the name for the mode it applies in, and the recomposition writes the
/// one for the use site's mode: `emit-call` runs in text and the mathematics
/// writer inside mathematics, and each finds only the definition available in
/// its own mode.
///
/// A definition that applies in *both* modes is single here, and this cannot
/// put two definitions under one name: two declarations of one name whose modes
/// overlap make the registry build fail, so a pair that reaches this function
/// declares text mode and mathematics mode, one each. No document reaches that
/// branch; it exists because the type allows the value.
fn latex_mode(value: &UserDefinition) -> LatexDefinitionMode {
    if !value.shares_its_name() {
        return LatexDefinitionMode::Single;
    }
    match value.definition().modes {
        DefinitionModes::Text => LatexDefinitionMode::Text,
        DefinitionModes::Math => LatexDefinitionMode::Mathematics,
        DefinitionModes::Both => LatexDefinitionMode::Single,
    }
}

/// Returns the LaTeX kind a definition is recomposed as. This function is the
/// only place where FLM's kinds of definitions are mapped to LaTeX's. A special
/// character sequence has no LaTeX kind and is never recomposed, so it gives a
/// structure error here rather than being written as a macro.
fn latex_kind(definition: &Definition) -> Result<LatexDefinitionKind, LowerError> {
    match definition.kind {
        DefinitionKind::Macro => Ok(LatexDefinitionKind::Macro),
        DefinitionKind::Environment => Ok(LatexDefinitionKind::Environment),
        DefinitionKind::Specials => {
            Err(LowerError::structure(
                "a definition of a special character sequence is never written as a LaTeX \
                 definition",
            ))
        }
    }
}

/// Whether the definition of kind `kind` declared as `name` is written as a
/// **robust** command rather than as the expandable macro every recomposed
/// definition otherwise becomes: the render option
/// [`robust_names`](LatexRecomposedRenderOptions::robust_names) lists the name,
/// the definition is a macro, and the name does not belong to the library's
/// own LaTeX (it does not begin with `flm`).
///
/// An environment is robust regardless of how it is written, so listing one
/// changes nothing, as the option's documentation states; listing every name a
/// document declares is therefore accepted. A listed name beginning with `flm`
/// is reported once per document by the backend's regions stage
/// (`flm.latex-recomposed.definition-name-refused`); here it just does not make
/// the definition robust.
fn asks_for_robustness(
    kind: LatexDefinitionKind,
    name: &str,
    robust_names: &std::collections::BTreeSet<String>,
) -> bool {
    kind == LatexDefinitionKind::Macro && !is_family_name(name) && robust_names.contains(name)
}

/// Stages the definition construct (`flm.pres.latex.definition`) of a
/// recomposable definition, which the emitter writes into the style file (see
/// the module documentation), and returns it.
///
/// The construct holds the compiled template's content: in the `body` slot
/// for a macro; cut at its body placeholder into the `begin` and `end` slots
/// for an environment LaTeX sets the body of; or whole in the `begin` slot,
/// with an empty `end` slot, for an environment that takes its body as an
/// argument ([`BodyForm::Argument`]). Each slot's content comes from the
/// template through [`template_run`]. The construct carries the definition's
/// recomposed name ([`recomposed_name`]), the name the document declared (for
/// the comment line of the style file), and the number of effective arguments
/// every call writes. `emit-call` stages it at a use site in text and the
/// mathematics writer at a use site inside mathematics; the emitter writes it
/// once per name.
///
/// The definition holds no default values: a default value is written at every
/// call whose use site left the argument out, in that argument's own group, so
/// the definition finds a value there like any other.
///
/// Returns `None` when the definition's own construct is already being staged
/// further up this call, which happens for a definition that uses **itself**,
/// directly or through a chain of definitions. The use site of the definition
/// inside its own template is written as a call like any other, and the
/// construct being staged further up is the definition that call calls;
/// staging a second one would write the same definition into the style file
/// twice and would never stop. The recomposed definition then calls itself and
/// LaTeX runs the recursion. The thread-local set [`BEING_STAGED`] records the
/// names being staged, through the guard [`Staging`].
fn definition_construct(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    value: &UserDefinition,
) -> Result<Option<BuildId>, LowerError> {
    let name = {
        let options = lw.option::<LatexRecomposedRenderOptions>()?;
        recomposed_name(
            latex_kind(value.definition())?,
            value.definition().name.as_str(),
            latex_mode(value),
            &options.bare_names,
        )
    };
    // The recursion guard: a definition that uses itself is written as a LaTeX
    // definition that calls itself, so its template's use site of it must not
    // start a second staging of the same construct.
    // See decision rationale at [§flm-dd-dr:std-userdefs-transcribable].
    let Some(_staging) = Staging::begin(name) else {
        return Ok(None);
    };
    definition_body(cx, lw, value).map(Some)
}

thread_local! {
    /// The recomposed names whose definition construct this thread is staging
    /// ([`definition_construct`]).
    ///
    /// This set is the only state a rule of this backend keeps across nodes,
    /// and it is held by the thread because the lowering interface gives a rule
    /// no other place for it. [`LowerCx`] is techy's
    /// `RestageContext<'_, Flm, (), ()>`, whose two state type parameters are
    /// both the unit type (`crate::core::lowering`), and [`Lowerer`] has no
    /// per-run store, so neither has a place for state kept during one run. The
    /// only other thread-local state of this kind in this crate is the
    /// worker-thread marker of `crate::lang::guard`. Lowering runs one tree at
    /// a time on a worker thread, so "per thread" means "per run" in practice.
    /// If a second rule needs state of its own, the state should be passed
    /// through the lowering interface instead.
    ///
    /// Names are added and removed only through [`Staging`], so that a panic
    /// that unwinds cannot leave a name in the set for the next run on this
    /// thread.
    // Why the backend keeps this one piece of state across nodes at all, and
    // what to do when a second rule needs state of its own.
    // See decision rationale at [§flm-dd-dr:std-userdefs-transcribable].
    static BEING_STAGED: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

/// One recomposed name in the set [`BEING_STAGED`], removed when this value is
/// dropped.
///
/// The name must be removed from the set on **every** way out of
/// [`definition_construct`], the unwinding of a panic included: lowering runs
/// on a worker thread that the pool reuses, so a name left in the set would
/// make [`definition_construct`] return `None` for that definition in the next
/// document lowered on that thread, and that document's style file would lack
/// the definition without any error. The `Drop` implementation guarantees the
/// removal.
struct Staging {
    name: String,
}

impl Staging {
    /// Adds `name` to the set [`BEING_STAGED`] and returns the guard that
    /// removes it; returns `None` when the name is already in the set, which
    /// is the recursion this guard detects.
    fn begin(name: String) -> Option<Self> {
        BEING_STAGED
            .with(|names| names.borrow_mut().insert(name.clone()))
            .then(|| Self { name })
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        BEING_STAGED.with(|names| names.borrow_mut().remove(&self.name));
    }
}

/// Stages the definition construct of `value` and returns it; the caller
/// ([`definition_construct`]) has already added its recomposed name to the
/// set [`BEING_STAGED`].
fn definition_body(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    value: &UserDefinition,
) -> Result<BuildId, LowerError> {
    let definition = value.definition();
    let variant = recomposed_variant(value)
        .ok_or_else(|| LowerError::structure("a recomposable definition without a template"))?;
    let signature = value.effective_signature().ok_or_else(|| {
        LowerError::structure("a recomposable definition without its effective signature")
    })?;
    let recomposition = Recomposition { definition, variant, signature };
    let template = variant.template();
    let root = template.tree().root();
    let children: Vec<NodeRef<'_, Flm>> = root.children().iter().collect();
    let mut slots = Vec::new();
    let kind = latex_kind(definition)?;
    match kind {
        // A user definition is a macro or an environment ([`latex_kind`]).
        LatexDefinitionKind::Declaration => {
            return Err(LowerError::structure("a user definition recomposed as a declaration"));
        }
        LatexDefinitionKind::Macro => {
            slots.push((
                String::from(BODY_SLOT),
                template_run(cx, lw, &recomposition, template, &children)?,
            ));
        }
        LatexDefinitionKind::Environment => match recomposition.body_form() {
            Some(BodyForm::Split) => {
                let at = children
                    .iter()
                    .position(|child| is_body_placeholder(template, child))
                    .ok_or_else(|| {
                        LowerError::structure(
                            "a recomposable environment without its body placeholder",
                        )
                    })?;
                slots.push((
                    String::from(BEGIN_SLOT),
                    template_run(cx, lw, &recomposition, template, &children[..at])?,
                ));
                slots.push((
                    String::from(END_SLOT),
                    template_run(cx, lw, &recomposition, template, &children[at + 1..])?,
                ));
            }
            // The body arrives as the last argument, so the whole template is
            // the begin code — the body written where the template puts it, as
            // that argument's parameter — and the end code is empty.
            Some(BodyForm::Argument) => {
                slots.push((
                    String::from(BEGIN_SLOT),
                    template_run(cx, lw, &recomposition, template, &children)?,
                ));
                slots.push((String::from(END_SLOT), Vec::new()));
            }
            // A macro has no body form; an environment always has one.
            None => {
                return Err(LowerError::structure(
                    "an environment whose template has no body form",
                ));
            }
        },
    }
    // Required once the template is lowered, which may yet expand the use
    // sites instead (see the module documentation).
    if recomposition.packs() {
        // A packed definition's body reads its arguments out of the one group
        // the call hands over, with the style file's accessor and its two
        // tests; nothing else needs that piece, and a definition of nine
        // effective arguments or fewer names its parameters directly. The
        // blank-test piece comes with it whether or not this definition asks a
        // blank test, since the packed blank test calls `\flmIfBlankTF`:
        // `preamble::user_args_piece` requires `preamble::IF_BLANK`.
        lw.require(&Name::from(preamble::USER_ARGS));
    } else if recomposition.needs_blank_test() {
        lw.require(&Name::from(preamble::IF_BLANK));
    }
    let (recomposed, robust) = {
        let options = lw.option::<LatexRecomposedRenderOptions>()?;
        let name = recomposed_name(
            kind,
            definition.name.as_str(),
            latex_mode(value),
            &options.bare_names,
        );
        (name, asks_for_robustness(kind, definition.name.as_str(), &options.robust_names))
    };
    let mut payload = LatexDefinitionPayload::new(kind, recomposed, signature.arity())
        .with_source_name(definition.name.as_str());
    if recomposition.packs() {
        payload = payload.packed();
    }
    if robust {
        payload = payload.robust();
    }
    if recomposition.body_parameter().is_some() {
        payload = payload.body_argument();
    }
    lw.pres_slots_with(cx, &ElementName::from(DEFINITION), payload, slots)
}

/// Returns what the use site `node` substitutes for the declared argument
/// `index` when the use site left that argument out: the argument's declared
/// default value from the recomposed templates, instantiated at the use site
/// ([`ModeVariant::instantiate_default`]), with every placeholder in it
/// replaced by what the use site wrote and every conditional in it evaluated.
/// The same function computes what the expansion of that use site would
/// substitute there, so the two are identical. `None` when the argument has no
/// declared default value: the call then writes an empty group in its place.
///
/// The call writes the result as that argument's value, like any other value:
/// lowered through the rules in text, and written from its nodes inside
/// mathematics ([`MathWriter::arguments`]).
fn resolved_default(
    node: &NodeRef<'_, Flm>,
    variant: &ModeVariant,
    index: usize,
) -> Result<Option<NodeTree<Flm>>, LowerError> {
    // The instantiation runs under the fixed stack budget, the policy every
    // registry outside a test runs under: the lowering façade exposes no
    // guard policy, and the use site is one subtree on the lowering stage's
    // worker thread. See design rationale at
    // [§flm-dd-dr:std-userdefs-lowering] and [§flm-dd-dr:lang-stack-budget].
    //
    // A failure here is not a user's mistake, which is why it is a
    // `LowerError` with prose rather than a diagnostic of its own: the two
    // ways a default could fail to resolve are both refused when the registry
    // is built. A default holding a placeholder no use site can answer is
    // refused by `check_placeholders` above (`HookPlaceholder`; a placeholder
    // written with a substitution argument refuses the definition earlier
    // still, at compilation, leaving it `NotCompiled`), and defaults that
    // refer to each other in a circle are refused by the default-cycle check
    // of `lang::rewrites::userdefs::definition`. What can still reach this arm
    // is a guard refusal or an implementation failure.
    variant.instantiate_default(*node, index, GuardPolicy::StackBudget).transpose().map_err(
        |error| {
            LowerError::other(format!(
                "resolving the default of the argument at index {index} failed: {error}"
            ))
        },
    )
}

/// The body of every `emit-call` rule: it writes a use site as a canonical call
/// of the definition and stages the definition construct for the style file,
/// or instantiates the use site when the definition is not recomposable or its
/// template needs a placeholder's characters (see the module documentation).
/// The expansion depth limit of the use sites it instantiates is the
/// definition's ([`UserDefinition::recursion_limit`]).
///
/// Build the rule of a definition with [`emit_call_rule`].
#[derive(Debug, Default, Clone, Copy)]
pub struct EmitCall;

impl EmitCall {
    /// Stages the call construct of the use site `node` and returns it: the
    /// macro or environment construct under the definition's recomposed name,
    /// with one mandatory argument slot per effective argument, in the order of
    /// the effective signature. This is the *canonical call*, one brace group
    /// per effective argument and nothing else, so that LaTeX parses nothing of
    /// its own at a call. An environment's body follows in its body slot.
    ///
    /// Each value slot holds what the expansion would substitute for a
    /// placeholder of its declared argument: the content the use site wrote, or
    /// the argument's declared default value lowered in its place, or nothing.
    /// A tack-on argument has no slot, because no template may refer to it, so
    /// the macros the use site wrote after the call are not written. Each flag
    /// slot holds `\BooleanTrue` where the use site wrote that argument and
    /// `\BooleanFalse` where it did not, which the body's `\IfBooleanTF`
    /// reads.
    fn call_construct(
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
        node: &NodeRef<'_, Flm>,
        value: &UserDefinition,
    ) -> Result<BuildId, LowerError> {
        let definition = value.definition();
        let variant = recomposed_variant(value)
            .ok_or_else(|| LowerError::structure("a recomposable definition without a template"))?;
        let signature = value.effective_signature().ok_or_else(|| {
            LowerError::structure("a recomposable definition without its effective signature")
        })?;
        let recomposition = Recomposition { definition, variant, signature };
        let mut shapes = Vec::new();
        let mut slots = Vec::new();
        for index in 0..signature.values() {
            shapes.push(LatexArgShape::Mandatory);
            let content = match substituted_argument_nodes(node, index) {
                Some(nodes) => lw.lower_nodes(cx, nodes)?,
                None => match resolved_default(node, variant, index)? {
                    Some(resolved) => lw.splice(cx, &Arc::new(resolved))?,
                    None => Vec::new(),
                },
            };
            slots.push((String::from(ARG_SLOT), content));
        }
        for index in signature.flagged() {
            shapes.push(LatexArgShape::Mandatory);
            let written = node.argument_content_nodes(*index).is_some();
            slots.push((String::from(ARG_SLOT), vec![flag_group(cx, lw, written)?]));
        }
        let kind = latex_kind(definition)?;
        let name = {
            let bare = &lw.option::<LatexRecomposedRenderOptions>()?.bare_names;
            recomposed_name(kind, definition.name.as_str(), latex_mode(value), bare)
        };
        let packs = recomposition.packs();
        match kind {
            // A user definition is a macro or an environment ([`latex_kind`]).
            LatexDefinitionKind::Declaration => {
                Err(LowerError::structure("a user definition recomposed as a declaration"))
            }
            LatexDefinitionKind::Macro => {
                let mut payload = LatexMacroPayload::new(name, shapes);
                if packs {
                    payload = payload.packed();
                }
                lw.pres_slots_with(cx, &ElementName::from(MACRO), payload, slots)
            }
            LatexDefinitionKind::Environment => {
                let body = if node.body().is_some() { lw.lower_body(cx, node)? } else { Vec::new() };
                slots.push((String::from(BODY_SLOT), body));
                let mut payload = LatexEnvironmentPayload::new(name, shapes);
                if packs {
                    payload = payload.packed();
                }
                // The call says what the definition says: whether LaTeX hands
                // the body over as an argument, read from the same `body_form`
                // the definition is written from. The emitter needs it at the
                // call, where the body is written — a body LaTeX *sets* stands
                // on lines of the file, and a body handed over as an argument
                // is a group of tokens, which is what decides how verbatim
                // content inside it is written.
                if recomposition.body_parameter().is_some() {
                    payload = payload.body_argument();
                }
                lw.pres_slots_with(cx, &ElementName::from(ENVIRONMENT), payload, slots)
            }
        }
    }
}

/// Stages the content of one flag of a canonical call as a raw node
/// (`flm.pres.latex.raw`, raw LaTeX written byte for byte) and returns it:
/// `\BooleanTrue` where the use site wrote the argument, `\BooleanFalse`
/// where it did not. LaTeX's `\IfBooleanTF`, with which the definition's body
/// asks the question, reads exactly these two values.
fn flag_group(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    written: bool,
) -> Result<BuildId, LowerError> {
    let text = if written { BOOLEAN_TRUE } else { BOOLEAN_FALSE };
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

impl Lower for EmitCall {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = value_of(&node)?;
        if value.recomposable().is_err() {
            return instantiate(node, cx, lw);
        }
        // A rule of the template needs the characters of a placeholder: the
        // definition cannot be written for LaTeX, so the use site is expanded,
        // as every use site of it is, since its template is the same at each.
        // See decision rationale at [§flm-dd-dr:rend-kept-placeholder-characters].
        let definition_node = match definition_construct(cx, lw, value) {
            Err(LowerError::PlaceholderCharacters) => return instantiate(node, cx, lw),
            other => other?,
        };
        let call = Self::call_construct(cx, lw, &node, value)?;
        // A definition that uses itself meets its own construct while that
        // construct is being staged, and stages nothing a second time: the
        // definition the outer staging is writing is the one this call calls.
        Ok(Lowering::Emit(match definition_node {
            Some(definition_node) => vec![definition_node, call],
            None => vec![call],
        }))
    }
}

/// Returns the `emit-call` rule of `definition`, a definition whose expansion
/// policy is `per-plan`. The rule targets the definition's element, requires
/// `flm.userdefs.kept`, reads the render options of the `latex-recomposed`
/// backend ([`LatexRecomposedRenderOptions`]), and is the default on that
/// backend alone. It declares the LaTeX constructs it produces: the definition
/// construct, the call construct of the definition's kind (the macro construct
/// for a macro, the environment construct for an environment) and the raw
/// node that holds a flag; for a special character sequence, which is never
/// recomposable, it declares none. A definition that is not recomposable
/// stages nothing of its own: its use sites are instantiated, and the rules of
/// the elements its template holds stage what the expansion becomes. See the
/// module documentation.
pub fn emit_call_rule(definition: &Definition) -> LoweringRule {
    let decl = LoweringDecl::new().requires(KEPT).option::<LatexRecomposedRenderOptions>();
    let decl = match definition.kind {
        DefinitionKind::Macro => decl.produces(DEFINITION).produces(MACRO).produces(RAW),
        DefinitionKind::Environment => {
            decl.produces(DEFINITION).produces(ENVIRONMENT).produces(RAW)
        }
        DefinitionKind::Specials => decl,
    };
    LoweringRule::new(
        emit_call_rule_name(definition),
        element_name(definition),
        decl,
        Arc::new(EmitCall),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(format!(
        "a use site of `{}` kept in the tree is written as a call, and the definition \
         itself into the style file (requires `flm.userdefs.kept`); the use site is expanded \
         instead when the definition cannot be written as one LaTeX definition or its \
         template needs the characters of an argument",
        definition.display_name()
    ))
}

// --- mathematics ----------------------------------------------------------------------------------

/// One part of the mathematics that [`write_math_run`] writes for the output:
/// LaTeX to write byte for byte, a source comment the writer staged, or
/// another node the writer staged, such as the definition construct
/// ([`flm.pres.latex.definition`](DEFINITION)) of a recomposable definition
/// used in the run, which the emitter writes into the style file.
#[derive(Debug)]
pub enum MathPiece {
    /// LaTeX to write byte for byte.
    Text(String),
    /// A source comment, staged as its comment node: the emitter writes it
    /// and ends its line, or writes nothing for it, whitespace included,
    /// under the render option
    /// [`drop_comments`](LatexRecomposedRenderOptions::drop_comments). A
    /// writer of lines needs to know where a line ends (see the `math`
    /// library's [newline
    /// rule](crate::stdcore::features::math::latex#the-newline-rule)).
    Comment(BuildId),
    /// Any other node the writer staged, to be written at its place in the
    /// run.
    Node(BuildId),
}

/// Writes a run of nodes in mathematics mode for the `latex-recomposed` output
/// (see "Mathematics" in the module documentation), and returns the parts
/// written ([`MathPiece`]).
///
/// The nodes' source is written byte for byte, except for:
///
/// - a use site of a recomposable user definition, written as its canonical
///   call (the definition's recomposed name and one brace group per effective
///   argument, their content written by the same writer), preceded by the
///   definition construct for the style file;
/// - a use site of any other user definition, written as its expansion,
///   written back as LaTeX source, with the use sites inside the expansion handled in
///   turn;
/// - a [`LatexPoint`] node of a copied template, written as the LaTeX it
///   carries;
/// - a source comment and a text command (`\text{…}`), lowered through the
///   rules and returned as staged nodes ([`MathPiece::Comment`] for a
///   comment).
///
/// A node that holds one of these inside it is written from its source with
/// the inner node's span cut out and the inner node written in its place. The
/// mathematics itself is never lowered through the rules.
///
/// The callers are the `math` library's line writer of the display-math
/// environments
/// ([`stage_lines`](crate::stdcore::features::math::latex::stage_lines)), the
/// `math-passthrough` feature's rule for `flm.math` on the `latex-recomposed`
/// backend and, for a template for mathematics mode, this module's staging of
/// a definition construct. [`stage_math_run`] writes the run and
/// stages the result.
///
/// # Errors
///
/// A [`LowerError`] when a use site cannot be copied or expanded, or a
/// construct cannot be staged.
pub fn write_math_run(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    nodes: &[NodeRef<'_, Flm>],
) -> Result<Vec<MathPiece>, LowerError> {
    let mut writer = MathWriter::new(cx, lw, Nodes::OfTheTree);
    writer.run(nodes)?;
    Ok(writer.finish())
}

/// Stages the parts [`write_math_run`] wrote, in order: each text part as a
/// raw node ([`flm.pres.latex.raw`](RAW)), each node as it is. Returns the
/// staged nodes.
///
/// # Errors
///
/// A [`LowerError`] when a raw node cannot be staged.
fn stage_math_pieces(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    pieces: Vec<MathPiece>,
) -> Result<Vec<BuildId>, LowerError> {
    pieces
        .into_iter()
        .map(|piece| match piece {
            MathPiece::Text(text) => {
                lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
            }
            MathPiece::Comment(id) | MathPiece::Node(id) => Ok(id),
        })
        .collect()
}

/// Writes a run of nodes in mathematics mode with [`write_math_run`] and stages
/// the result: each text part as a raw node (`flm.pres.latex.raw`), each staged
/// node as it is. Returns the staged nodes, in order.
///
/// # Errors
///
/// A [`LowerError`] when [`write_math_run`] fails, or when a raw node cannot
/// be staged.
pub fn stage_math_run(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    nodes: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let pieces = write_math_run(cx, lw, nodes)?;
    stage_math_pieces(cx, lw, pieces)
}

/// Whether the mathematics writer must write `node` itself rather than copy it
/// as source: a use site of a user definition, a [`LatexPoint`] node of a
/// copied template, a source comment, or a construct that reads an argument of
/// its own as text ([`is_math_text_command`]).
///
/// A source comment is in this list because a comment reaches the recomposed
/// output through a single writer, the LaTeX family's sink, which ends the
/// comment's line itself; the writer therefore stages the comment node for the
/// emitter instead of copying its `%` into text that nothing would end.
///
/// A construct that reads an argument of its own as text (a text command
/// inside a formula, `\text{…}`) is in this list because that argument is
/// document text, not notation: it goes through the lowering rules, which
/// escape it, write a user definition used in it as its call, and write
/// mathematics nested in it from its nodes, as they do for text anywhere
/// else.
fn is_written(node: &NodeRef<'_, Flm>) -> bool {
    node.comment().is_some()
        || element_value::<UserDefinition>(node).is_some()
        || element_value::<LatexPoint>(node).is_some()
        || is_math_text_command(node)
}

/// Whether `node`, or one of its descendants, is something the mathematics
/// writer must write itself ([`is_written`]).
fn needs_writing(node: &NodeRef<'_, Flm>) -> bool {
    is_written(node) || node.descendants().any(|descendant| is_written(&descendant))
}

/// The mathematics writer behind [`write_math_run`]: it collects text and
/// adds it as one text part before each staged node.
struct MathWriter<'a, 'c> {
    cx: &'a mut LowerCx<'c>,
    lw: &'a mut dyn Lowerer,
    pieces: Vec<MathPiece>,
    text: String,
    nodes: Nodes,
}

/// Which tree the nodes being written belong to, which decides how the
/// mathematics writer has a node lowered through the rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nodes {
    /// The tree being lowered: a node is lowered in place.
    OfTheTree,
    /// A tree that the writer's caller built (a template's copy, a use site's
    /// expansion), which the lowering does not hold: a node is copied into a
    /// synthesized value of its own and spliced, since only the nodes of the
    /// tree being lowered, of a spliced value, or of a lowered node region can
    /// be lowered.
    Synthesized,
}

impl<'a, 'c> MathWriter<'a, 'c> {
    fn new(cx: &'a mut LowerCx<'c>, lw: &'a mut dyn Lowerer, nodes: Nodes) -> Self {
        MathWriter { cx, lw, pieces: Vec::new(), text: String::new(), nodes }
    }

    /// Returns the parts written, the text collected last included.
    fn finish(mut self) -> Vec<MathPiece> {
        self.flush();
        self.pieces
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            self.pieces.push(MathPiece::Text(std::mem::take(&mut self.text)));
        }
    }

    /// Writes a run of nodes: each stretch of nodes that need no writing of
    /// their own is written back as LaTeX source, and every other node is written
    /// ([`write`](MathWriter::write)).
    fn run<'t>(&mut self, nodes: &[NodeRef<'t, Flm>]) -> Result<(), LowerError> {
        let mut stretch: Vec<NodeRef<'t, Flm>> = Vec::new();
        for node in nodes {
            if needs_writing(node) {
                self.text.push_str(&recompose_source(&stretch));
                stretch.clear();
                self.write(*node)?;
            } else {
                stretch.push(*node);
            }
        }
        self.text.push_str(&recompose_source(&stretch));
        Ok(())
    }

    /// Writes one node that is, or holds, a use site, a [`LatexPoint`] node, a
    /// source comment or a construct that reads an argument of its own as
    /// text.
    fn write(&mut self, node: NodeRef<'_, Flm>) -> Result<(), LowerError> {
        if node.comment().is_some() {
            return self.lowered(node);
        }
        if is_math_text_command(&node) {
            return self.lowered(node);
        }
        if let Some(value) = element_value::<UserDefinition>(&node) {
            return self.use_site(node, value);
        }
        if let Some(point) = element_value::<LatexPoint>(&node) {
            self.text.push_str(&point.text);
            return Ok(());
        }
        if let Some((open, close)) = node.group_delimiters() {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            self.text.push_str(open);
            self.run(&children)?;
            self.text.push_str(close);
            return Ok(());
        }
        if node.is_list() {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            return self.run(&children);
        }
        self.with_holes(node)
    }

    /// Writes a node that holds a use site among its descendants without being
    /// a group or a list (a command whose argument or body holds one): its
    /// source with each child's span cut out and the child written in its
    /// place, byte for byte where the children's spans lie in order inside the
    /// node's own span. Where they do not (a child inserted from another
    /// source, or a span that does not cut the source at a character
    /// boundary), the whole node is written back as LaTeX source, with every use site
    /// inside it written as the author typed it, after the definition construct
    /// of each recomposable definition used inside it has been staged, so that
    /// the recomposed LaTeX still compiles ([`whole`](MathWriter::whole)).
    fn with_holes(&mut self, node: NodeRef<'_, Flm>) -> Result<(), LowerError> {
        let span = node.span();
        let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
        let tiles = children.iter().all(|child| {
            child.span().same_source(span)
                && child.span().start() >= span.start()
                && child.span().end() <= span.end()
        }) && children.windows(2).all(|pair| pair[0].span().end() <= pair[1].span().start());
        let content = span.source().content();
        let mut pieces = Vec::with_capacity(children.len() + 1);
        let mut at = span.start();
        let mut cut = tiles;
        for child in &children {
            match content.get(at..child.span().start()) {
                Some(gap) if cut => pieces.push(gap),
                _ => cut = false,
            }
            at = child.span().end();
        }
        let tail = content.get(at..span.end());
        if !cut || tail.is_none() {
            return self.whole(node);
        }
        for (gap, child) in pieces.iter().zip(&children) {
            self.text.push_str(gap);
            self.run(std::slice::from_ref(child))?;
        }
        self.text.push_str(tail.unwrap_or_default());
        Ok(())
    }

    /// The fallback of [`with_holes`](Self::with_holes): stages the definition
    /// construct of every recomposable definition used inside `node`, then
    /// writes the whole node back as LaTeX source.
    fn whole(&mut self, node: NodeRef<'_, Flm>) -> Result<(), LowerError> {
        for descendant in node.descendants() {
            if let Some(value) = element_value::<UserDefinition>(&descendant) {
                if value.recomposable().is_ok() {
                    if let Some(construct) = definition_construct(self.cx, self.lw, value)? {
                        self.flush();
                        self.pieces.push(MathPiece::Node(construct));
                    }
                }
            }
        }
        self.text.push_str(&recompose_source(&[node]));
        Ok(())
    }

    /// Has one node lowered through the rules and adds the staged nodes as parts
    /// of their own, rather than copying the node as source.
    ///
    /// Two kinds of node take this route. A **source comment**, so that the
    /// emitter writes it through the sink's comment writer (the prefix, the
    /// text, the newline that ends the comment's line, and the whitespace that
    /// followed it in the source except that newline), or writes nothing for
    /// it where comments are dropped. A **text command**, so that its argument
    /// is written as the document text it is; the `math` feature's rule for the
    /// element writes the command back and lowers the argument.
    fn lowered(&mut self, node: NodeRef<'_, Flm>) -> Result<(), LowerError> {
        let ids = match self.nodes {
            Nodes::OfTheTree => lower_run(self.cx, self.lw, std::slice::from_ref(&node))?,
            Nodes::Synthesized => {
                let value = copy_run_into_value(&[node], &node, ValueContent::Own, GuardPolicy::StackBudget)
                    .map_err(|error| LowerError::other(format!("copying the node failed: {error}")))?;
                self.lw.splice(self.cx, &value)?
            }
        };
        self.flush();
        let comment = node.comment().is_some();
        for id in ids {
            self.pieces.push(if comment { MathPiece::Comment(id) } else { MathPiece::Node(id) });
        }
        Ok(())
    }

    /// Writes a use site: the definition construct and the canonical call for
    /// a recomposable definition; the expansion, written back as LaTeX source, for any
    /// other definition and for one whose template needs a placeholder's
    /// characters.
    fn use_site(&mut self, node: NodeRef<'_, Flm>, value: &UserDefinition) -> Result<(), LowerError> {
        if value.recomposable().is_err() {
            return self.expansion(node, value);
        }
        // As `emit-call` does: a template whose rules need the characters of
        // a placeholder has its use sites expanded.
        let construct = match definition_construct(self.cx, self.lw, value) {
            Err(LowerError::PlaceholderCharacters) => return self.expansion(node, value),
            other => other?,
        };
        if let Some(construct) = construct {
            self.flush();
            self.pieces.push(MathPiece::Node(construct));
        }
        let definition = value.definition();
        let kind = latex_kind(definition)?;
        let name = {
            let bare = &self.lw.option::<LatexRecomposedRenderOptions>()?.bare_names;
            recomposed_name(kind, definition.name.as_str(), latex_mode(value), bare)
        };
        match kind {
            // A user definition is a macro or an environment ([`latex_kind`]).
            LatexDefinitionKind::Declaration => {
                return Err(LowerError::structure("a user definition recomposed as a declaration"));
            }
            LatexDefinitionKind::Macro => {
                self.text.push('\\');
                self.text.push_str(&name);
                // The call's own groups follow the name directly. Where it
                // writes none — the effective signature is empty, the
                // definition declaring no argument or only a tack-on argument,
                // which carries neither a value nor a flag — the space the
                // writer left after the name is written back, so that a letter
                // after it does not extend the name.
                if value.effective_signature().is_some_and(|signature| signature.arity() == 0) {
                    if let Some(InvocationSyntaxData::Macro { post_space, .. }) =
                        node.invocation_syntax()
                    {
                        self.text.push_str(post_space.resolve(node.span().source()));
                    }
                }
                self.arguments(&node, value)?;
            }
            LatexDefinitionKind::Environment => {
                self.text.push_str(&format!("\\begin{{{name}}}"));
                self.arguments(&node, value)?;
                if let Some(body) = node.body() {
                    let nodes: Vec<NodeRef<'_, Flm>> = body.iter().collect();
                    self.run(&nodes)?;
                }
                self.text.push_str(&format!("\\end{{{name}}}"));
            }
        }
        Ok(())
    }

    /// Writes a use site as its expansion, written back as LaTeX source, with the use
    /// sites inside the expansion handled in turn.
    fn expansion(&mut self, node: NodeRef<'_, Flm>, value: &UserDefinition) -> Result<(), LowerError> {
        let expanded = expansion_of(node, value, self.lw)?;
        let children: Vec<NodeRef<'_, Flm>> = expanded.root().children().iter().collect();
        let nodes = std::mem::replace(&mut self.nodes, Nodes::Synthesized);
        let written = self.run(&children);
        self.nodes = nodes;
        written
    }

    /// Writes the use site's effective arguments, in the order of the
    /// definition's effective signature: one brace group per declared argument
    /// that carries a value (every one except a tack-on argument), holding what
    /// the expansion would substitute there (the content the use site wrote,
    /// written by this writer so that a use site inside an argument is handled
    /// too; or the argument's declared default value; or nothing), then one
    /// brace group per flag. When the call packs its arguments, one more pair
    /// of braces surrounds them all. This is the same canonical call that
    /// `emit-call` stages in text, written here as text.
    fn arguments(
        &mut self,
        node: &NodeRef<'_, Flm>,
        value: &UserDefinition,
    ) -> Result<(), LowerError> {
        let definition = value.definition();
        let variant = recomposed_variant(value)
            .ok_or_else(|| LowerError::structure("a recomposable definition without a template"))?;
        let signature = value.effective_signature().ok_or_else(|| {
            LowerError::structure("a recomposable definition without its effective signature")
        })?;
        let packs = Recomposition { definition, variant, signature }.packs();
        if packs {
            self.text.push('{');
        }
        for index in 0..signature.values() {
            self.text.push('{');
            match substituted_argument_nodes(node, index) {
                Some(content) => {
                    let nodes: Vec<NodeRef<'_, Flm>> = content.iter().collect();
                    self.run(&nodes)?;
                }
                None => {
                    if let Some(resolved) = resolved_default(node, variant, index)? {
                        let nodes: Vec<NodeRef<'_, Flm>> =
                            resolved.root().children().iter().collect();
                        self.run(&nodes)?;
                    }
                }
            }
            self.text.push('}');
        }
        for index in signature.flagged() {
            let written = node.argument_content_nodes(*index).is_some();
            let flag = if written { BOOLEAN_TRUE } else { BOOLEAN_FALSE };
            self.text.push_str(&format!("{{{flag}}}"));
        }
        if packs {
            self.text.push('}');
        }
        Ok(())
    }
}

/// Registers, through the feature builder `b`, what the `userdefs` feature
/// contributes once for the `latex-recomposed` backend, regardless of the
/// definitions it holds: a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the backend and of each
/// of the four LaTeX constructs the `emit-call` rules stage, with the
/// construct's payload type as its signature.
///
/// The `userdefs` feature registers neither the backend nor the construct
/// elements, which the LaTeX features own. Because they are peer
/// declarations, a registry without the LaTeX features still builds: the
/// `emit-call` rules are then set aside (excluded from rule selection; see
/// [`PeerName`](crate::core::registry::PeerName)).
///
/// The rules of each definition are created beside its element by the
/// feature's own registration; the rule that writes the placeholders of a
/// recomposed template belongs to the LaTeX family
/// ([`placeholder_rule`](crate::stdcore::backends::latex::placeholder_rule)).
///
/// # Errors
///
/// Never fails: the builder's peer declarations report their problems when
/// the registry is built.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the `latex-recomposed` backend, on which the `emit-call` rules of the user \
         definitions are the default",
    )?;
    b.peer_element::<LatexDefinitionPayload>(
        DEFINITION,
        "the LaTeX definition construct, which the `emit-call` rules produce for the style \
         file",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX macro-call construct, which the `emit-call` rules produce for a call of a \
         macro",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        ENVIRONMENT,
        "the LaTeX environment construct, which the `emit-call` rules produce for a call of \
         an environment",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the raw LaTeX construct, which the `emit-call` rules produce for the flags of a \
         call",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::mint::{ArgDef, ArgStdKind, TackOnField};
    use crate::lang::{LangInputs, build_language};
    use crate::model::names::CapName;

    #[test]
    fn the_two_rules_of_a_definition_require_kept_and_differ_in_their_default_mark() {
        let definition = Definition::macro_("foo", vec![ArgDef::mandatory("x")]).content("#1");
        let instantiate = instantiate_at_lowering_rule(&definition);
        assert_eq!(instantiate.name, "flm.userdefs.macro.foo.lower.instantiate-at-lowering");
        assert_eq!(instantiate.element, "flm.userdefs.macro.foo");
        assert_eq!(instantiate.decl.requires, [CapName::from(KEPT)]);
        assert!(instantiate.decl.produces.is_empty());
        assert_eq!(instantiate.default, DefaultMark::Never);
        let emit = emit_call_rule(&definition);
        assert_eq!(emit.name, "flm.userdefs.macro.foo.lower.emit-call");
        assert_eq!(emit.decl.requires, [CapName::from(KEPT)]);
        assert_eq!(
            emit.decl.produces,
            [ElementName::from(DEFINITION), ElementName::from(MACRO), ElementName::from(RAW)]
        );
        assert!(emit.default.is_specific_to(&BACKEND.into()));
        let environment = Definition::environment("env", Vec::new()).content("#{body}");
        assert_eq!(
            emit_call_rule(&environment).decl.produces,
            [ElementName::from(DEFINITION), ElementName::from(ENVIRONMENT), ElementName::from(RAW)]
        );
        let specials = Definition::specials("@", Vec::new()).content("at");
        assert!(emit_call_rule(&specials).decl.produces.is_empty());
    }

    #[test]
    fn a_math_only_definition_names_its_rules_with_the_math_segment() {
        let definition = Definition::macro_("calN", Vec::new()).math("\\mathcal{N}");
        assert_eq!(emit_call_rule_name(&definition), "flm.userdefs.macro.math.calN.lower.emit-call");
    }

    /// The roster compiled against a language holding nothing but the
    /// definitions themselves, recomposability recorded.
    fn roster(definitions: Vec<Definition>) -> Vec<Arc<UserDefinition>> {
        let values: Vec<Arc<UserDefinition>> =
            definitions.into_iter().map(UserDefinition::new).collect();
        let mut inputs = LangInputs::new();
        inputs.elements = values
            .iter()
            .map(|value| value.element_def("userdefs", element_name(value.definition())))
            .collect();
        let lang = build_language(inputs);
        for value in &values {
            let _ = value.compile(&lang);
        }
        record_recomposability(&values);
        values
    }

    fn reason(value: &UserDefinition) -> Option<NotRecomposable> {
        value.recomposable().err()
    }

    /// A tack-on argument named `fields`, accepting one field `\mark`: the
    /// shape the `refs` feature's `\label` field takes.
    fn tack_on_arg() -> ArgDef {
        ArgDef::named(
            "fields",
            ArgStdKind::TackOn {
                fields: vec![TackOnField::new("mark", Vec::new())],
            },
        )
    }

    #[test]
    fn a_template_naming_a_tack_on_argument_is_refused_when_the_definition_is_compiled() {
        // Naming a tack-on argument is a definition error at registry build,
        // on every backend rather than in this one's recomposition: what the
        // call absorbed is whole macros, which have no meaning as the value of
        // a parameter. The refusal is raised where the definition is compiled,
        // so a template that names one never reaches a recomposability
        // outcome at all.
        let definition =
            Definition::macro_("tack", vec![ArgDef::mandatory("a"), tack_on_arg()])
                .content("#1#{fields}");
        let value = UserDefinition::new(definition);
        let mut inputs = LangInputs::new();
        inputs.elements =
            vec![value.element_def("userdefs", element_name(value.definition()))];
        let lang = build_language(inputs);
        let errors = value.compile(&lang).expect_err("the definition is refused");
        // The definition declares both modes and its text is compiled once per
        // mode, so the same placeholder is refused in each compilation.
        assert_eq!(errors.len(), 2, "{errors:?}");
        for error in &errors {
            assert_eq!(error.identifier(), "flm.userdefs.bad-definition");
            assert!(
                error.message().contains("a template cannot refer to"),
                "{}",
                error.message()
            );
        }
        // The same template, with the placeholder inside a conditional's test
        // rather than written plainly, is refused by the same rule.
        let conditional =
            Definition::macro_("tack", vec![ArgDef::mandatory("a"), tack_on_arg()])
                .content("#1\\IfProvidedT{#{fields}}{!}");
        let value = UserDefinition::new(conditional);
        let mut inputs = LangInputs::new();
        inputs.elements =
            vec![value.element_def("userdefs", element_name(value.definition()))];
        let lang = build_language(inputs);
        let errors = value.compile(&lang).expect_err("the definition is refused");
        assert!(
            errors.iter().any(|error| error.identifier() == "flm.userdefs.bad-definition"),
            "{errors:?}"
        );
    }

    #[test]
    fn a_plain_macro_and_environment_are_recomposable() {
        let values = roster(vec![
            Definition::macro_("hello", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
                .content("#{greeting}, #2!")
                .default("greeting", "Hello"),
            Definition::environment("note", Vec::new()).content("[Note: #{body}]"),
        ]);
        assert_eq!(values[0].recomposable(), Ok(()));
        assert_eq!(values[1].recomposable(), Ok(()));
    }

    #[test]
    fn every_intrinsic_reason_is_found() {
        let values = roster(vec![
            Definition::specials("@", Vec::new()).content("[at]"),
            // Every argument shape is recomposable: a key-value argument's
            // group is written as the use site wrote it, and a verbatim
            // argument's characters as the content they are. Two declared
            // arguments, two values.
            Definition::macro_(
                "s",
                vec![
                    ArgDef::new(ArgStdKind::KeyVals),
                    ArgDef::new(ArgStdKind::Verbatim { delimiters: None }),
                ],
            )
            .content("#1#2"),
            // An optional argument at any position is recomposable: every
            // argument is a value group at the call.
            Definition::macro_("o", vec![ArgDef::mandatory("a"), ArgDef::optional("b")]).content("#1#2"),
            // A conditional asking whether an argument with a declared
            // default was *written*: the call carries the answer as a flag, so
            // this is recomposable and its effective signature is two.
            Definition::macro_("c", vec![ArgDef::optional("a")])
                .content("\\IfProvidedTF{#1}{x}{y}")
                .default("a", "dflt"),
            // A default that holds a placeholder of another argument is
            // resolved at the use site, by the use site's own bindings, and
            // written there: it is recomposable, and so is a default holding
            // a conditional.
            Definition::macro_(
                "d",
                vec![ArgDef::optional("a"), ArgDef::mandatory("b"), ArgDef::optional("c")],
            )
            .content("#1#2#3")
            .default("c", "#2")
            .default("a", "\\IfProvidedTF{#3}{#3}{none}"),
            // An environment whose template never names its body: recomposable,
            // taking the body as an argument its begin code never writes.
            Definition::environment("e0", Vec::new()).content("no body"),
            // An environment that names its body twice, and one that names it
            // inside a group: both recomposable, each taking its body as an
            // argument of its own.
            Definition::environment("e2", Vec::new()).content("#{body}#{body}"),
            Definition::environment("en", Vec::new()).content("{#{body}}"),
            // A placeholder inside mathematics is written as a parameter, so
            // this one is recomposable. Where a placeholder stands elsewhere
            // plays no part here either (verbatim content, an address): that
            // is found when the template is lowered, an integration test.
            Definition::macro_("v", vec![ArgDef::mandatory("a")]).content("\\(#1\\)"),
            // One name declared twice, once per parsing mode: two definitions,
            // both recomposable, each written under a twin name of its own.
            Definition::macro_("twin", Vec::new()).text("t"),
            Definition::macro_("twin", Vec::new()).math("m"),
            Definition::macro_("always", Vec::new()).content("a").expand(ExpandPolicy::Always),
            Definition::macro_("refused", Vec::new()).content("#1"),
            // Ten arguments is one more than a LaTeX macro body can name, and
            // is recomposable all the same: the call packs.
            Definition::macro_("ten", (0..10).map(|i| ArgDef::mandatory(format!("a{i}"))).collect())
                .content("x"),
            Definition::macro_("pair", Vec::new()).content("m"),
            Definition::environment("pair", Vec::new()).content("#{body}"),
            // A `*` marker is an argument like any other: its value group
            // holds the star the writer wrote, its declared default where they
            // wrote none, and a question about it reads the flag.
            Definition::macro_("star", vec![ArgDef::new(ArgStdKind::Star), ArgDef::mandatory("x")])
                .content("\\IfBooleanTF{#1}{S}{s}#2"),
            Definition::macro_("stardflt", vec![ArgDef::new(ArgStdKind::Star)])
                .content("#1")
                .default("1", "d"),
            // A blank test over two arguments at once, one of which a value
            // test also asks about: every value group holds plain content, so
            // the one test stands over the run of them.
            Definition::macro_("blank", vec![ArgDef::optional("a"), ArgDef::mandatory("b")])
                .content("\\IfNoValueTF{#1}{x}{y}\\ifblank{#1#2}{b}{n}"),
            // Ten effective arguments: five declared, five of them asked
            // about. Values and flags count together, so this is one more than
            // a LaTeX macro body can name, and the call packs.
            Definition::macro_(
                "flagged",
                (0..5).map(|i| ArgDef::optional(format!("a{i}"))).collect(),
            )
            .content(
                "\\IfProvidedT{#1}{a}\\IfProvidedT{#2}{b}\\IfProvidedT{#3}{c}\\IfProvidedT{#4}{d}\\IfProvidedT{#5}{e}#5",
            ),
            // A default on a *mandatory* argument is written at the call site
            // like any other — at a use site recovered with the argument
            // missing — and is resolved there like any other too.
            Definition::macro_("dm", vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")])
                .content("#1#2")
                .default("b", "#1"),
            // A definition may declare a tack-on argument — the macros a call
            // absorbs after it — and not name it in its template. It is
            // recomposable, and the tack-on argument carries no effective
            // argument at all: one value for `a` and nothing else.
            Definition::macro_("tack", vec![ArgDef::mandatory("a"), tack_on_arg()])
                .content("#1!"),
        ]);
        let reasons: Vec<Option<NotRecomposable>> = values.iter().map(|v| reason(v)).collect();
        assert_eq!(reasons[0], Some(NotRecomposable::Specials));
        assert_eq!(reasons[1], None);
        assert_eq!(reasons[2], None);
        assert_eq!(reasons[3], None);
        assert_eq!(reasons[4], None);
        // An environment whose template never names its body grabs the body
        // and drops it, exactly as FLM's own expansion does.
        assert_eq!(reasons[5], None);
        assert_eq!(reasons[6], None);
        assert_eq!(reasons[7], None);
        assert_eq!(reasons[8], None);
        assert_eq!(reasons[9], None);
        assert_eq!(reasons[10], None);
        assert!(values[9].shares_its_name());
        assert!(values[10].shares_its_name());
        // A name the document declares once shares nothing, whichever modes it
        // applies in.
        assert!(!values[8].shares_its_name());
        assert_eq!(reasons[11], Some(NotRecomposable::AlwaysExpanded));
        assert_eq!(reasons[12], Some(NotRecomposable::NotCompiled));
        assert_eq!(reasons[13], None);
        // A macro and an environment of one name are both recomposable: each
        // kind is written under a prefix of its own.
        assert_eq!(reasons[14], None);
        assert_eq!(reasons[15], None);
        assert_eq!(reasons[16], None);
        assert_eq!(reasons[17], None);
        assert_eq!(reasons[18], None);
        assert_eq!(reasons[19], None);
        assert_eq!(reasons[20], None);
        assert_eq!(reasons[21], None);
        // The effective signature of a recomposable definition: one value per
        // declared argument, then one flag per argument a conditional asks a
        // *provided* question about. A key-value and a verbatim argument each
        // carry one value like any other.
        assert_eq!(values[1].effective_signature(), Some(&EffectiveSignature::new(2, [])));
        assert_eq!(values[3].effective_signature(), Some(&EffectiveSignature::new(1, [0])));
        assert_eq!(values[16].effective_signature(), Some(&EffectiveSignature::new(2, [0])));
        assert_eq!(values[18].effective_signature(), Some(&EffectiveSignature::new(2, [0])));
        // More effective arguments than a macro body has parameters is no
        // reason: ten values, and five values with five flags.
        assert_eq!(values[13].effective_signature(), Some(&EffectiveSignature::new(10, [])));
        assert_eq!(values[19].effective_signature(), Some(&EffectiveSignature::new(5, 0..5)));
        assert_eq!(values[19].effective_signature().unwrap().arity(), 10);
        // The declared tack-on argument carries neither a value nor a flag:
        // one value, for the mandatory argument, and nothing else.
        assert_eq!(values[21].effective_signature(), Some(&EffectiveSignature::new(1, [])));
        // A definition that is not recomposable carries none.
        assert_eq!(values[0].effective_signature(), None);
        // Three of this roster are written expanded, one of each class that a
        // document can reach: the specials by its nature, the `always`
        // definition by the author's choice, the refused one by its own
        // error. Every other definition here is recomposed.
        let expanded: Vec<Option<NotRecomposable>> =
            reasons.iter().filter(|reason| reason.is_some()).cloned().collect();
        assert_eq!(
            expanded,
            [
                Some(NotRecomposable::Specials),
                Some(NotRecomposable::AlwaysExpanded),
                Some(NotRecomposable::NotCompiled),
            ]
        );
    }

    #[test]
    fn a_use_of_a_non_recomposable_definition_spreads_and_a_cycle_is_no_reason() {
        let values = roster(vec![
            Definition::macro_("a", Vec::new()).content("\\b"),
            Definition::macro_("b", Vec::new()).content("\\c"),
            // A definition the registry build refuses — its template names an
            // argument it does not declare — is the leaf that cannot be
            // recomposed, and the refusal spreads up the chain.
            Definition::macro_("c", Vec::new()).content("#1"),
            // A cycle of definitions, and a definition that uses itself: both
            // are recomposed as LaTeX definitions that call each other and
            // itself. Whether such a chain ever stops is the document's
            // business, and LaTeX's to find where it does not.
            Definition::macro_("p", Vec::new()).content("\\q"),
            Definition::macro_("q", Vec::new()).content("\\p"),
            Definition::macro_("self", Vec::new()).content("\\self"),
            Definition::macro_("fine", Vec::new()).content("\\alwaysfine"),
            Definition::macro_("alwaysfine", Vec::new()).content("y").expand(ExpandPolicy::Always),
        ]);
        assert_eq!(reason(&values[0]), Some(NotRecomposable::Uses { name: "\\b".into() }));
        assert_eq!(reason(&values[1]), Some(NotRecomposable::Uses { name: "\\c".into() }));
        assert_eq!(values[3].recomposable(), Ok(()));
        assert_eq!(values[4].recomposable(), Ok(()));
        assert_eq!(values[5].recomposable(), Ok(()));
        assert_eq!(values[6].recomposable(), Ok(()));
    }
}
