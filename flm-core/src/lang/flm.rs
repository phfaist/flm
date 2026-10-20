//! The FLM language type, [`Flm`]: the type parameter techy's parser and node
//! trees take when they handle FLM.
//!
//! techy is generic over a **language**: a set of types, fixed at compile time,
//! that says which kinds of groups and of callables exist, which data is
//! attached to parsing states and to nodes, and which object holds the behavior
//! of a running parse. [`Flm`] is FLM's language. It is built on techy's
//! LaTeX-like preset (techy's ready-made specification types, argument parsers
//! and parse behavior for markup in the style of LaTeX), so the preset's parts
//! work with [`Flm`] directly.
//!
//! This module defines:
//!
//! - [`FlmGroupType`] — the group classes: content groups, mathematics,
//!   verbatim text and paragraphs.
//! - [`FlmCallableType`] — the forms a callable node takes: macro, environment,
//!   special character sequence, and two forms the library creates itself.
//! - [`FlmNodeExts`] — which data is attached to nodes, to arguments and to
//!   node slots (techy's `ParsedSlot`, the parts of a node's content outside
//!   its arguments, such as an environment's body).
//! - [`Flm`] itself. Its required method
//!   [`make_node_ext`](Lang::make_node_ext) gives every node its
//!   [element](crate::model::element::ElementSpec).
//!
//! Every node receives an element, and finding it needs no lookup by name. A
//! callable node's specification is always one of the wrapper types that
//! [minting](crate::lang::mint) builds, each carrying its element; every other
//! node shape takes its element from the [structural
//! elements](crate::lang::structural) the parsing state carries. See
//! [`resolve_element`] for the full rule.

use std::sync::Arc;

use techy::core::node::{NodeBuildError, NodeKind, StagedChildren};
use techy::core::specs::{check_provider_commands_shadowed_by_escape, ScopeStack};
use techy::core::token::{SpecialsMatch, SpecialsScanError, StdTokenization, TriggerChars};
use techy::core::{
    AllLangFeatures, ClosedVocabulary, FinalizeError, Lang, NodeExtTypes, ParsingState,
    StateData,
};
use techy::error::Diagnostics;
use techy::latexlike::{
    builtin_package, default_token_rules, BodyMarker, Event, InvocationSyntaxData,
    LatexlikeCallableType, LatexlikeGroupType, LatexlikeLang, MathGroupForm, Mode,
    StdEnvironmentSyntax,
};
use techy::serialize::{DeserializableValue, SerializableValue};
use techy::source::{Source, SourceSpan};

use crate::lang::driver::FlmDriver;
use crate::lang::ext::{FlmNodeExt, FlmStateExt};
use crate::lang::mint::resolve_element;
use crate::lang::origin::FlmOrigin;

/// The group classes: what a group node holds (content, mathematics, raw text
/// or a paragraph).
///
/// A group node records its class separately from the delimiters it was
/// written with. Which delimiter pairs exist is a setting of the tokenizer
/// ([`crate::lang::rules`]), and several delimiter pairs may open groups of the
/// same class (`\(…\)` opens inline mathematics, and so does `$…$` when `$`
/// is enabled as a delimiter). Because the class is recorded on the node, code
/// anywhere in the library can ask whether a node is mathematics, verbatim text
/// or a paragraph without looking at its delimiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(SerializableValue, DeserializableValue)]
pub enum FlmGroupType {
    /// A plain `{…}` content group, and the group an argument parser creates
    /// around a braced argument.
    #[serial(name = "content")]
    Content,
    /// A mathematics group, inline or display.
    #[serial(name = "math")]
    Math(MathGroupForm),
    /// A group of raw text. Every run of verbatim content is a child of a group
    /// of this class; see [`crate::lang::verbatim`].
    #[serial(name = "verbatim")]
    Verbatim,
    /// A paragraph. Paragraphs are created by the block-structure rewrite pass
    /// ([`crate::lang::rewrites::blocks`]); no delimiter in the source opens one.
    #[serial(name = "paragraph")]
    Paragraph,
}

impl LatexlikeGroupType for FlmGroupType {
    fn content_group() -> FlmGroupType {
        FlmGroupType::Content
    }

    fn math_group(form: MathGroupForm) -> FlmGroupType {
        FlmGroupType::Math(form)
    }

    fn verbatim_group() -> FlmGroupType {
        FlmGroupType::Verbatim
    }

    fn math_form(self) -> Option<MathGroupForm> {
        match self {
            FlmGroupType::Math(form) => Some(form),
            _ => None,
        }
    }
}

impl ClosedVocabulary for FlmGroupType {
    const ALL: &'static [FlmGroupType] = &[
        FlmGroupType::Content,
        FlmGroupType::Math(MathGroupForm::Inline),
        FlmGroupType::Math(MathGroupForm::Display),
        FlmGroupType::Verbatim,
        FlmGroupType::Paragraph,
    ];
}

/// The callable forms: how a callable node was written in the source (macro,
/// environment, special character sequence), or one of two forms that only the
/// library creates (placeholder, presentation).
///
/// An author can write the first three forms. The last two are created by the
/// library only: no source text parses to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(SerializableValue, DeserializableValue)]
pub enum FlmCallableType {
    /// A macro invocation (`\emph{…}`).
    #[serial(name = "macro")]
    Macro,
    /// An environment (`\begin{…}…\end{…}`).
    #[serial(name = "environment")]
    Environment,
    /// A special character sequence, recognized wherever it appears rather than
    /// after an escape character (`~`, `---`). Paragraph-break nodes have this
    /// form too, and so do the nodes of synthesized elements, which the library
    /// creates ([`FlmSynthesizedSpec`](crate::lang::mint::FlmSynthesizedSpec)).
    #[serial(name = "specials")]
    Specials,
    /// A placeholder in the template of a [user
    /// definition](crate::lang::rewrites::userdefs::Definition) (`#1`,
    /// `#{name}`): the place where the content of an argument is inserted when
    /// the definition is used. Placeholder nodes are created when a template is
    /// compiled, never by the parse of a document.
    #[serial(name = "placeholder")]
    Placeholder,
    /// A node of a presentation tree, the tree that lowering builds from a
    /// frozen document tree for one backend. Such a node carries its typed
    /// payload in its node extension
    /// ([`FlmNodeExt::payload`](crate::lang::ext::FlmNodeExt::payload)). No
    /// source text parses to this form.
    #[serial(name = "presentation")]
    Presentation,
}

impl LatexlikeCallableType for FlmCallableType {
    fn macro_callable() -> FlmCallableType {
        FlmCallableType::Macro
    }

    fn environment_callable() -> FlmCallableType {
        FlmCallableType::Environment
    }

    fn specials_callable() -> FlmCallableType {
        FlmCallableType::Specials
    }
}

impl ClosedVocabulary for FlmCallableType {
    const ALL: &'static [FlmCallableType] = &[
        FlmCallableType::Macro,
        FlmCallableType::Environment,
        FlmCallableType::Specials,
        FlmCallableType::Placeholder,
        FlmCallableType::Presentation,
    ];
}

/// The data FLM attaches to the three parts of a tree where techy lets a
/// language attach data: nodes, arguments and node slots (techy's
/// `ParsedSlot`, the parts of a node's content outside its arguments, such as
/// an environment's body). (The data attached to
/// parsing states is [`FlmStateExt`].)
///
/// - Nodes carry [`FlmNodeExt`], which holds the node's element.
/// - Arguments carry nothing (`()`), as the preset's argument parsers expect.
/// - Slots carry the preset's body marker, with which the preset marks the slot
///   that holds an environment's body; asking a node for its body relies on it.
#[derive(Debug, Clone, Copy)]
pub struct FlmNodeExts;

impl NodeExtTypes for FlmNodeExts {
    type NodeExt = FlmNodeExt;
    type ArgumentExt = ();
    type SlotExt = BodyMarker;
}

/// The FLM language: the type parameter of every techy type that handles FLM.
///
/// This type holds no data. Its implementation of techy's `Lang` trait states
/// FLM's choices (the group classes, the callable forms, the attached data, the
/// parse driver), and every techy type that depends on a language (parsing
/// states, node trees, tokens, diagnostics) takes it as its type parameter:
/// `NodeTree<Flm>` is a tree of FLM nodes. The [module documentation](self)
/// lists the choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flm;

impl Lang for Flm {
    type Features = AllLangFeatures;
    type GroupTypeId = FlmGroupType;
    type CallableTypeId = FlmCallableType;
    type ModeId = Mode;
    type StateExt = FlmStateExt;
    type Event = Event;
    type SessionExt = ();
    type SourceOrigin = FlmOrigin;
    type Tokenization = StdTokenization;
    type NodeExts = FlmNodeExts;
    type InvocationSyntax = InvocationSyntaxData<StdEnvironmentSyntax<Flm>>;
    type Driver = FlmDriver;

    /// Returns the data of the language's initial parsing state (techy's
    /// `ParsingState::lang_initial`), before anything is configured: the
    /// preset's standard tokenizer rules, techy's built-in package that handles
    /// `\begin` and `\end`, text mode, and the default state extension.
    ///
    /// No document is parsed from this state. [`crate::lang::build`] derives
    /// the state every parse starts from, in one step: it applies the parsing
    /// settings, the features' packages, the fallback provider and the
    /// registry's structural elements.
    ///
    /// # Errors
    ///
    /// `FinalizeError` if the state cannot be finalized. This state is built
    /// from constants of this crate and always finalizes.
    fn initial_state_data() -> Result<StateData<Self>, FinalizeError> {
        let mut scopes = ScopeStack::new();
        scopes.push(builtin_package());
        Ok(StateData {
            rules: default_token_rules(),
            scopes,
            mode: Mode::Text,
            ext: FlmStateExt::default(),
        })
    }

    /// Checks the events that come with a change of parsing state made outside
    /// a parse.
    ///
    /// One event can only be interpreted with the stack of enclosing parsing
    /// states: the request to leave a mathematics context, which restores the
    /// context that enclosed it. [`FlmDriver`] interprets that event during a
    /// parse. Outside a parse there is no enclosing stack, so this method
    /// refuses the event with an error instead of dropping it.
    ///
    /// # Errors
    ///
    /// `FinalizeError` when the events include the request to leave a
    /// mathematics context. Such a state can only be derived during a parse.
    fn finalize_transition(
        new: &mut StateData<Self>,
        prev: &ParsingState<Self>,
        events: &[Event],
    ) -> Result<(), FinalizeError> {
        let _ = (new, prev);
        if events.iter().any(|event| matches!(event, Event::ExitMathContext)) {
            return Err(FinalizeError::new(
                "the ExitMathContext event needs the enclosing-state stack: derive \
                 through a parse context, where the driver lowers it",
            ));
        }
        Ok(())
    }

    /// Looks for a special character sequence at byte position `pos` in
    /// `content`, and returns the match if there is one. Every provider on the
    /// state's scope stack is consulted, from the innermost outwards, and the
    /// longest match wins.
    ///
    /// # Errors
    ///
    /// `SpecialsScanError` when a provider fails while it looks for a match.
    fn scan_specials(
        state: &ParsingState<Self>,
        content: &str,
        pos: usize,
    ) -> Result<Option<SpecialsMatch<Self>>, SpecialsScanError> {
        state.scopes().scan_specials(state, content, pos)
    }

    /// The characters that can start a special character sequence under this
    /// state: the union over every provider on its scope stack. The tokenizer
    /// uses this set to skip the scan at any other character.
    fn specials_trigger_chars(data: &StateData<Self>) -> TriggerChars {
        data.scopes.specials_trigger_chars()
    }

    /// Builds the extension ([`FlmNodeExt`]) of a node that techy is about to
    /// add to a tree. This is the only place where a node receives its
    /// [element](crate::model::element::ElementSpec).
    ///
    /// Every node receives an element: a callable node the one its
    /// specification carries, every other node one of the structural elements
    /// the parsing state carries. [`resolve_element`] states the full rule. The
    /// extension's source map and payload are left empty here: the
    /// block-structure rewrite pass sets the source map on the text nodes it
    /// rewrites, and the code that builds a presentation node, a synthesized
    /// node or a rebuilt node sets its payload ([`FlmNodeExt::payload`]).
    ///
    /// # Errors
    ///
    /// `NodeBuildError::ExtMintFailed` when the node is a callable whose
    /// specification is none of the wrapper types that
    /// [minting](crate::lang::mint) builds, and therefore carries no element.
    /// No document can cause this: it requires a caller that builds a tree by
    /// hand with a specification of its own. See
    /// [`UnmintedSpec`](crate::lang::mint::UnmintedSpec).
    fn make_node_ext(
        kind: &NodeKind<Self>,
        _span: &SourceSpan<FlmOrigin>,
        state: &Arc<ParsingState<Self>>,
        _children: StagedChildren<'_, Self>,
    ) -> Result<FlmNodeExt, NodeBuildError> {
        // The one failing case is a caller-contract violation, unreachable from
        // document input: a consumer-built node whose spec is not flm-minted
        // (`UnmintedSpec`). techy's `ExtMintFailed` is the channel that exists
        // for exactly that, and it never panics.
        match resolve_element(kind, state.ext()) {
            Ok(element) => Ok(FlmNodeExt::new(element)),
            Err(error) => Err(NodeBuildError::ExtMintFailed { detail: error.to_string() }),
        }
    }
}

impl LatexlikeLang for Flm {
    /// Checks the registered names once, when a parse starts, and reports a
    /// warning for each provider whose definitions can never be reached because
    /// their names were registered with the escape character included (`\emph`
    /// instead of `emph`). The warnings are added to the parse's diagnostics.
    fn check_parse_start(
        source: &Arc<Source<FlmOrigin>>,
        seed: &Arc<ParsingState<Self>>,
        diagnostics: &mut Diagnostics<FlmOrigin>,
    ) {
        check_provider_commands_shadowed_by_escape(seed, source, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use techy::core::ParsingStateDelta;

    #[test]
    fn the_vocabularies_play_the_preset_roles() {
        assert_eq!(FlmGroupType::content_group(), FlmGroupType::Content);
        assert_eq!(FlmGroupType::verbatim_group(), FlmGroupType::Verbatim);
        assert!(FlmGroupType::Math(MathGroupForm::Display).is_math());
        assert!(!FlmGroupType::Paragraph.is_math());
        assert!(FlmCallableType::Macro.is_macro());
        assert!(!FlmCallableType::Placeholder.is_specials());
    }

    #[test]
    fn the_seed_is_finalize_coherent() {
        let seed = ParsingState::<Flm>::lang_initial().expect("seed state");
        let rederived = seed.derived(&ParsingStateDelta::new()).unwrap();
        assert_eq!(seed.rules(), rederived.rules());
        assert_eq!(seed.mode(), rederived.mode());
        assert_eq!(seed.scopes().provider_names().collect::<Vec<_>>(), ["_builtin"]);
    }

    #[test]
    fn an_exit_math_event_is_refused_out_of_parse() {
        let seed = ParsingState::<Flm>::lang_initial().expect("seed state");
        assert!(seed.derived(&ParsingStateDelta::new().event(Event::ExitMathContext)).is_err());
    }
}
