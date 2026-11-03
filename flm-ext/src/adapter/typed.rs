//! The typed route of the guest client (the cargo feature `native`): the
//! conversions between the guest's values and the extension API's, with the
//! running call's table at hand. When the extension is compiled into the
//! program, the bodies of the guest's forwarding methods use them in place
//! of JSON, and the handler objects of `binding::handlers` read an entry's
//! reply, the guest's value, with [`IntoApi`].
//!
//! # The mirror principle
//!
//! The two traits are the twins of the dispatcher's conversions
//! ([`FromArg`](crate::adapter::values::FromArg) and
//! [`IntoReply`](crate::adapter::values::IntoReply)), and a typed body is
//! the dispatcher's arm for the same method with the conversions swapped:
//!
//! - [`IntoApi`] is the twin of `FromArg`: where an arm reads the API's
//!   value of an argument from JSON, a typed body converts the guest's
//!   value into it. Plain data (the API's own data types, which the guest
//!   re-exports, texts, numbers, `bool`, `Value`, the unit enums) is the
//!   same value; a struct of a handle kind resolves its handle in the table
//!   as `FromArg` resolves the handle the JSON writes (a node copied out of
//!   the table, a user definition cloned, a restaged bundle taken out); a
//!   type of the guest's own resolves the handles and the durable tokens it
//!   holds, as the reader of its JSON form does.
//! - [`FromApi`] is the twin of `IntoReply`: where an arm writes the API's
//!   answer as JSON, lending the objects it holds into the table, a typed
//!   body builds the guest's value from it, lending the same objects into
//!   the table and building the guest's struct over each handle; a borrowed
//!   result is copied, as `IntoReply` copies it; a type of the guest's own is
//!   built as its JSON form would be read back.
//!
//! Both traits are implemented on the guest's types, as `Into` and `From`
//! are: `guest.into_api(table)` answers the API's value
//! ([`IntoApi::Api`]), and `Guest::from_api(api, table)` builds the guest's
//! value from the API's value `api`, whose type is the trait's parameter (one
//! guest type may be built from several API types: the guest's
//! `CompiledTemplate` from a template the call lent and from a durable one).
//! `'c` is the table's lifetime, the running call's.
//!
//! Here: the shapes that wrap another type (`Option`, `Vec`, pairs, `()`, a
//! `Result` of [`ExtError`] or of [`NotCharacters`]), mirroring
//! `IntoReply`'s; the plain data, both ways; and the structs of the handle
//! kinds, resolved and lent. The conversions of the guest's own types are in
//! `src/guest/hand/`, each beside its type's JSON form.
//!
//! No conversion checks a value of its own: the guest's value is well typed,
//! and a conversion fails only where the reading of the same value's JSON
//! form fails. Into the API's value, the dispatcher's reading fails on a
//! handle of another call or of another kind (an element's handler aspect
//! included, which crosses as the handle its lending answered), a durable
//! token that names no object of the call's durable table, a bundle taken
//! already. From the API's answer, the guest's reading fails on an argument
//! whose parser is a feature's own, which the guest's `ArgDef` does not
//! hold.
//!
//! A conversion runs inside the closure of the running call's frame
//! (`binding/frames.rs`, invariant 2), so it runs no code of the extension:
//! it reads the guest's value, and a value that may hold the extension's
//! handler objects is converted by reference, the guest's value dropped
//! after the closure ends.

use serde_json::Value;

use crate::adapter::handles::{CallTable, Entry, HandleKind};
use crate::adapter::values::{
    bundle_at, mismatch, node_from_table, not_characters, staged_at, take_restaged_argument, take_restaged_slot,
    tree_from_table,
};
use crate::api;
use crate::api::lang::text::NotCharacters;
use crate::guest;
use crate::runtime::ExtError;

/// How a guest's value is converted into the extension API's value of a
/// call's argument, with the running call's table at hand: the twin of
/// [`FromArg`](crate::adapter::values::FromArg).
///
/// One implementation per type a guest's forwarding method passes, the
/// borrowed forms included: a reference to plain data is the same reference;
/// a reference to a guest's value that holds handles is the API's owned
/// value, which the body passes by reference (the dispatcher's arm reads a
/// borrowed parameter as its owned form likewise).
pub(crate) trait IntoApi<'c> {
    /// The API's value.
    type Api;

    /// The API's value of this guest's value.
    ///
    /// # Errors
    ///
    /// As the dispatcher's reading of the value's JSON form: a handle it
    /// holds belongs to another call, or stands for an object of another
    /// kind ([`ExtError::StaleHandle`], [`ExtError::Argument`]); a restaged
    /// bundle it names was taken already ([`ExtError::Argument`]); a durable
    /// token it holds names no object of the call's durable table
    /// ([`ExtError::StaleDurable`]).
    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError>;
}

/// How a guest's value is built from the extension API's answer `A`,
/// lending the objects it holds into the running call's table: the twin of
/// [`IntoReply`](crate::adapter::values::IntoReply).
pub(crate) trait FromApi<'c, A>: Sized {
    /// The guest's value of the API's answer `api`, its objects lent into
    /// `table`.
    ///
    /// # Errors
    ///
    /// The error the API's answer carries (a `Result`'s, passed on as it is,
    /// or a [`NotCharacters`] as [`ExtError::Core`]); an argument whose
    /// parser is a feature's own, which the guest's `ArgDef` does not hold
    /// ([`ExtError::Value`], as the reading of its JSON form refuses it).
    fn from_api(api: A, table: &mut CallTable<'c>) -> Result<Self, ExtError>;
}

// The shapes that wrap another type.

impl<'c, T: IntoApi<'c>> IntoApi<'c> for Option<T> {
    type Api = Option<T::Api>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        self.map(|value| value.into_api(table)).transpose()
    }
}

impl<'c, T: IntoApi<'c>> IntoApi<'c> for Vec<T> {
    type Api = Vec<T::Api>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        self.into_iter().map(|item| item.into_api(table)).collect()
    }
}

impl<'c, A: IntoApi<'c>, B: IntoApi<'c>> IntoApi<'c> for (A, B) {
    type Api = (A::Api, B::Api);

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        Ok((self.0.into_api(table)?, self.1.into_api(table)?))
    }
}

impl<'c, A, T: FromApi<'c, A>> FromApi<'c, Option<A>> for Option<T> {
    fn from_api(api: Option<A>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        api.map(|value| T::from_api(value, table)).transpose()
    }
}

impl<'c, A, T: FromApi<'c, A>> FromApi<'c, Vec<A>> for Vec<T> {
    fn from_api(api: Vec<A>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        api.into_iter().map(|item| T::from_api(item, table)).collect()
    }
}

impl<'c, A, B, T: FromApi<'c, A>, U: FromApi<'c, B>> FromApi<'c, (A, B)> for (T, U) {
    fn from_api(api: (A, B), table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok((T::from_api(api.0, table)?, U::from_api(api.1, table)?))
    }
}

impl<'c> FromApi<'c, ()> for () {
    fn from_api(_: (), _: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(())
    }
}

/// The error, or the value of `T`.
impl<'c, A, T: FromApi<'c, A>> FromApi<'c, Result<A, ExtError>> for T {
    fn from_api(api: Result<A, ExtError>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        T::from_api(api?, table)
    }
}

/// The error as [`not_characters`] words it, or the value of `T`.
impl<'c, A, T: FromApi<'c, A>> FromApi<'c, Result<A, NotCharacters>> for T {
    fn from_api(api: Result<A, NotCharacters>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        T::from_api(api.map_err(not_characters)?, table)
    }
}

// Plain data: the same value, both ways.

/// The plain data types, owned: the same value both ways, and a reference to
/// one is the same reference.
macro_rules! plain {
    ($($type:ty),+ $(,)?) => {
        $(
            impl<'c> IntoApi<'c> for $type {
                type Api = $type;

                fn into_api(self, _: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
                    Ok(self)
                }
            }

            impl<'c, 'a> IntoApi<'c> for &'a $type {
                type Api = &'a $type;

                fn into_api(self, _: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
                    Ok(self)
                }
            }

            impl<'c> FromApi<'c, $type> for $type {
                fn from_api(api: $type, _: &mut CallTable<'c>) -> Result<Self, ExtError> {
                    Ok(api)
                }
            }
        )+
    };
}

plain!(
    String,
    usize,
    u32,
    bool,
    Value,
    api::core::stage::Effect,
    api::core::stage::Scope,
    api::core::registry::CallableKind,
    api::lang::mint::FallbackKind,
    api::lang::rewrites::userdefs::DefinitionKind,
    api::lang::rewrites::userdefs::DefinitionModes,
    api::lang::mint::ArgLayout,
    api::lang::mint::BodyLayout,
    api::lang::rewrites::userdefs::ExpandPolicy,
    api::model::aspects::Placement,
    api::techy::error::Severity,
    api::techy::node::NodeKindName,
    api::lang::mint::Mode,
    api::core::registry::StandardTable,
    api::core::services::Arity,
    api::host::FilesScope,
    api::stdcore::backends::latex::LatexArgShape,
    api::core::stage::StageDecl,
    api::core::stage::Needs,
    api::core::stage::RewriteDecl,
    api::core::lowering::LoweringDecl,
    api::core::lowering::DefaultMark,
    api::core::registry::OutputDef,
    api::core::registry::StyleDef,
    api::core::registry::ScriptDef,
    api::lang::rewrites::userdefs::TemplateSpec,
    api::lang::rewrites::userdefs::PlaceholderRef,
    api::stdcore::features::numbering::FormatOptions,
    api::stdcore::features::citations::EntryPiece,
    api::stdcore::features::refs::LabelUse,
    api::model::aspects::Label,
    api::model::aspects::MalformedLabel,
    api::techy::node::Span,
    api::lang::text::CharsContent,
    api::stdcore::features::math::NormalizedLine,
);

/// The unsized plain data, borrowed: the same reference.
macro_rules! plain_borrowed {
    ($($type:ty),+ $(,)?) => {
        $(
            impl<'c, 'a> IntoApi<'c> for &'a $type {
                type Api = &'a $type;

                fn into_api(self, _: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
                    Ok(self)
                }
            }
        )+
    };
}

plain_borrowed!(
    str,
    [u8],
    [api::stdcore::features::refs::LabelUse],
    [api::stdcore::features::citations::EntryPiece],
);

// The structs of the handle kinds, resolved.

/// A node: an owned copy out of the table ([`node_from_table`]), as `FromArg`
/// reads a node.
impl<'c> IntoApi<'c> for &guest::techy::node::Node {
    type Api = Box<dyn api::techy::node::Node<'c> + 'c>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        node_from_table(table, self.handle)
    }
}

/// A list of nodes, each an owned copy.
impl<'c> IntoApi<'c> for &[guest::techy::node::Node] {
    type Api = Vec<Box<dyn api::techy::node::Node<'c> + 'c>>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        self.iter().map(|node| node.into_api(table)).collect()
    }
}

/// A tree: an owned copy out of the table ([`tree_from_table`]).
impl<'c> IntoApi<'c> for &guest::techy::node::Tree {
    type Api = Box<dyn api::techy::node::Tree>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        tree_from_table(table, self.handle)
    }
}

/// A staged node: the entry's value ([`staged_at`]).
impl<'c> IntoApi<'c> for guest::lang::rewrites::userdefs::Staged {
    type Api = api::core::lowering::Staged;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        staged_at(table, self.handle)
    }
}

/// A restaged argument, taken out of the table once ([`bundle_at`]).
impl<'c> IntoApi<'c> for guest::techy::transform::RestagedArgument {
    type Api = Box<dyn api::techy::transform::RestagedArgument>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        bundle_at(table, self.handle, HandleKind::RestagedArgument, take_restaged_argument)
    }
}

/// A restaged slot, taken out of the table once ([`bundle_at`]).
impl<'c> IntoApi<'c> for guest::techy::transform::RestagedSlot {
    type Api = Box<dyn api::techy::transform::RestagedSlot>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        bundle_at(table, self.handle, HandleKind::RestagedSlot, take_restaged_slot)
    }
}

/// The structs of the kinds whose entry the table owns and a call clones
/// (`cloned_entry!` of the dispatcher's readers): the entry cloned.
macro_rules! cloned {
    ($($guest:ty => $variant:ident: $api:ty),+ $(,)?) => {
        $(
            impl<'c> IntoApi<'c> for $guest {
                type Api = $api;

                fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
                    match table.entry(self.handle)? {
                        Entry::$variant(inner) => Ok(inner.clone()),
                        other => Err(mismatch(HandleKind::$variant, other.kind())),
                    }
                }
            }
        )+
    };
}

cloned!(
    guest::lang::rewrites::userdefs::UserDefinition => UserDefinition: api::lang::rewrites::userdefs::UserDefinition,
    guest::core::lowering::LoweringRule => LoweringRule: api::core::lowering::LoweringRule,
    guest::lang::rewrites::userdefs::ValueBindings => ValueBindings: api::lang::rewrites::userdefs::ValueBindings,
    guest::core::registry::LibraryStage => LibraryStage: api::core::registry::LibraryStage,
);

// The structs of the handle kinds, lent.

/// The objects an API method answers that the table holds as their entry,
/// lent as `IntoReply` lends them (`lent`, `answered_lent!`); the guest's
/// struct holds the handle.
macro_rules! lent {
    ($($guest:ty => $variant:ident: $api:ty),+ $(,)?) => {
        $(
            impl<'c> FromApi<'c, $api> for $guest {
                fn from_api(api: $api, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
                    Ok(Self { handle: table.lend(Entry::$variant(api)) })
                }
            }
        )+
    };
}

lent!(
    guest::techy::node::Node => Node: Box<dyn api::techy::node::Node<'c> + 'c>,
    guest::techy::node::Tree => Tree: Box<dyn api::techy::node::Tree>,
    guest::techy::extract::KeyVals => KeyVals: Box<dyn api::techy::extract::KeyVals<'c> + 'c>,
    guest::techy::extract::KeyValEntry => KeyValEntry: Box<dyn api::techy::extract::KeyValEntry<'c> + 'c>,
    guest::lang::rewrites::userdefs::UserDefinition => UserDefinition: api::lang::rewrites::userdefs::UserDefinition,
    guest::core::lowering::LoweringRule => LoweringRule: api::core::lowering::LoweringRule,
    guest::lang::rewrites::userdefs::Staged => Staged: api::core::lowering::Staged,
    guest::core::registry::LibraryStage => LibraryStage: api::core::registry::LibraryStage,
    guest::techy::transform::RestagedArgument => RestagedArgument: Box<dyn api::techy::transform::RestagedArgument>,
    guest::techy::transform::RestagedSlot => RestagedSlot: Box<dyn api::techy::transform::RestagedSlot>,
    guest::stdcore::features::citations::CitationEntry => CitationEntry: Box<dyn api::stdcore::features::citations::CitationEntry>,
    guest::lang::rewrites::userdefs::ValueBindings => ValueBindings: api::lang::rewrites::userdefs::ValueBindings,
    guest::host::Files => Files: Box<dyn api::host::Files>,
    guest::host::HostService => HostService: Box<dyn api::host::HostService>,
    guest::core::data::table::ExtTableRead => ExtTableRead: Box<dyn api::core::data::table::ExtTableRead + 'c>,
    guest::core::stage::TreesTable => TreesTable: Box<dyn api::core::stage::TreesTable + 'c>,
    guest::stdcore::backends::lowering::LoweredTrees => LoweredTrees: Box<dyn api::stdcore::backends::lowering::LoweredTrees + 'c>,
);

/// The boxed façades with no lifetime of a tree, lent as their entry: a box
/// that outlives the call is lent as one of the call (`IntoReply`'s
/// `answered_lent_outliving!`).
macro_rules! lent_outliving {
    ($($guest:ty => $variant:ident: $trait:path),+ $(,)?) => {
        $(
            impl<'c, 'a: 'c> FromApi<'c, Box<dyn $trait + 'a>> for $guest {
                fn from_api(api: Box<dyn $trait + 'a>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
                    Ok(Self { handle: table.lend(Entry::$variant(api)) })
                }
            }
        )+
    };
}

lent_outliving!(
    guest::lang::rewrites::userdefs::ModeVariant => ModeVariant: api::lang::rewrites::userdefs::ModeVariant,
    guest::lang::rewrites::userdefs::PlaceholderBindings => PlaceholderBindings: api::lang::rewrites::userdefs::PlaceholderBindings,
);
