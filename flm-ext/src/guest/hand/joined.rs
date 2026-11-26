//! What the guest's sequences ([`Nodes`](crate::guest::techy::node::Nodes),
//! [`StagedNodes`](crate::guest::lang::rewrites::userdefs::StagedNodes)) are
//! made of: [`Joined`], parts in order, each an item of the extension or a
//! sequence the host answered; and the macro [`sequence_type`], which writes
//! each sequence's struct over it.

use serde_json::Value;

use crate::adapter::handles::{Handle, HandleKind};
use crate::adapter::values::{handle_from_value, sequence_to_value, JsonForm};
use crate::guest::abi;
use crate::runtime::ExtError;

/// An item of a guest's sequence: a handle of the call, which the host also
/// answers in sequences of the kind [`SEQUENCE`](Item::SEQUENCE).
pub(crate) trait Item: Copy + JsonForm {
    /// The kind of the handle of a sequence of such items.
    const SEQUENCE: HandleKind;

    /// The handle the host lent for the item.
    #[cfg(feature = "native")]
    fn handle(self) -> Handle;

    /// The item at the index `i` of the host's sequence `sequence`, or
    /// `None` past its end (a crossing).
    ///
    /// # Errors
    ///
    /// The crossing fails.
    fn sequence_get(sequence: Handle, i: usize) -> Result<Option<Self>, ExtError>;

    /// The group of items from the index `from` of the host's sequence
    /// `sequence` (a crossing, at most [`abi::PULL_GROUP`] items).
    ///
    /// # Errors
    ///
    /// The crossing fails.
    fn sequence_pull(sequence: Handle, from: usize) -> Result<Vec<Self>, ExtError>;
}

/// Items in order, as parts: an item, or a sequence the host answered, kept
/// as its handle and its length until it is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Joined<T> {
    /// The parts, in order.
    parts: Vec<Part<T>>,
    /// The number of items of all the parts.
    len: usize,
}

/// One part of a [`Joined`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part<T> {
    /// One item.
    Item(T),
    /// A sequence the host answered: its handle and its length.
    Sequence {
        /// The handle the host lent for it.
        handle: Handle,
        /// The number of its items.
        len: usize,
    },
}

impl<T> Default for Joined<T> {
    fn default() -> Joined<T> {
        Joined { parts: Vec::new(), len: 0 }
    }
}

impl<T: Item> Joined<T> {
    /// The items `items`, in order.
    pub(crate) fn from_items(items: Vec<T>) -> Joined<T> {
        let len = items.len();
        Joined { parts: items.into_iter().map(Part::Item).collect(), len }
    }

    /// The host's sequence `handle` of `len` items.
    #[cfg(feature = "native")]
    pub(crate) fn sequence(handle: Handle, len: usize) -> Joined<T> {
        Joined { parts: vec![Part::Sequence { handle, len }], len }
    }

    /// The number of items.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Appends `item`.
    pub(crate) fn push(&mut self, item: T) {
        self.push_part(Part::Item(item));
    }

    /// Appends the parts of `other`, in order.
    pub(crate) fn push_all(&mut self, other: &Joined<T>) {
        self.parts.extend_from_slice(&other.parts);
        self.len += other.len;
    }

    /// The item at the index `i`, or `None` past the last item; an item of a
    /// sequence the host answered is fetched (a crossing).
    pub(crate) fn get(&self, i: usize) -> Result<Option<T>, ExtError> {
        let mut at = i;
        for part in &self.parts {
            match *part {
                Part::Item(item) if at == 0 => return Ok(Some(item)),
                Part::Item(_) => at -= 1,
                Part::Sequence { handle, len } if at < len => return T::sequence_get(handle, at),
                Part::Sequence { len, .. } => at -= len,
            }
        }
        Ok(None)
    }

    /// The items, in order; the items of a sequence the host answered are
    /// pulled in groups.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Result<T, ExtError>> + '_ {
        self.parts.iter().flat_map(|part| -> Box<dyn Iterator<Item = Result<T, ExtError>>> {
            match *part {
                Part::Item(item) => Box::new(std::iter::once(Ok(item))),
                Part::Sequence { handle, len } => Box::new(abi::Paged::new(len, move |from| T::sequence_pull(handle, from))),
            }
        })
    }

    /// The handle of each part, in order: an item's, or a sequence's.
    #[cfg(feature = "native")]
    pub(crate) fn handles(&self) -> impl Iterator<Item = Handle> + '_ {
        self.parts.iter().map(|part| match *part {
            Part::Item(item) => item.handle(),
            Part::Sequence { handle, .. } => handle,
        })
    }

    /// A sequence the host answered is its handle with its length
    /// (`sequence_to_value`); joined parts are the joined list of their
    /// handles, which the host reads part by part.
    pub(crate) fn to_value(&self) -> Value {
        match self.parts.as_slice() {
            [Part::Sequence { handle, len }] => sequence_to_value(*handle, *len),
            parts => Value::Array(parts.iter().map(Part::written).collect()),
        }
    }

    /// The parts `value` writes: a sequence's handle, an item's, or a joined
    /// list of them.
    ///
    /// # Errors
    ///
    /// A part is neither ([`ExtError::Argument`]).
    pub(crate) fn from_value(value: &Value) -> Result<Joined<T>, ExtError> {
        let mut joined = Joined::default();
        match value {
            Value::Array(parts) => {
                for part in parts {
                    joined.push_part(Part::read(part)?);
                }
            }
            part => joined.push_part(Part::read(part)?),
        }
        Ok(joined)
    }

    /// Appends `part`.
    fn push_part(&mut self, part: Part<T>) {
        self.len += match part {
            Part::Item(_) => 1,
            Part::Sequence { len, .. } => len,
        };
        self.parts.push(part);
    }
}

impl<T: Item> Part<T> {
    /// The part's handle, a sequence's with its length.
    fn written(&self) -> Value {
        match self {
            Part::Item(item) => item.to_value(),
            Part::Sequence { handle, len } => sequence_to_value(*handle, *len),
        }
    }

    /// The part a handle of an item or of a sequence of items writes.
    fn read(value: &Value) -> Result<Part<T>, ExtError> {
        if handle_from_value(value)?.kind == T::SEQUENCE {
            let (handle, len) = abi::sequence_of(value, T::SEQUENCE)?;
            Ok(Part::Sequence { handle, len })
        } else {
            Ok(Part::Item(T::from_value(value)?))
        }
    }
}

/// Writes the struct `$name` of a guest's sequence of `$item` over a
/// [`Joined`] (its rustdoc given first), with its methods, its conversions
/// and the [`Item`] of `$item`: the host's sequences are of the handle kind
/// and the entry `$variant`, which `$push` reads part by part on the typed
/// route, the api's list being a `Vec<$api>`; `$what` names an item in the
/// methods' rustdoc.
macro_rules! sequence_type {
    (
        $(#[$meta:meta])*
        $name:ident of $item:ty, $what:literal, api $api:ty, entry $variant:ident, push $push:path
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub struct $name($crate::guest::hand::joined::Joined<$item>);

        impl $name {
            #[doc = concat!("The number of ", $what, "s, with no crossing.")]
            pub fn len(&self) -> usize {
                self.0.len()
            }

            #[doc = concat!("Whether it has no ", $what, ", with no crossing.")]
            pub fn is_empty(&self) -> bool {
                self.0.len() == 0
            }

            #[doc = concat!("Appends the ", $what, " `item`, with no crossing.")]
            pub fn push(&mut self, item: $item) {
                self.0.push(item);
            }

            #[doc = concat!("Appends the ", $what, "s of `other`, in order, with no crossing: a sequence the host answered is")]
            /// appended as its handle.
            pub fn push_all(&mut self, other: &$name) {
                self.0.push_all(&other.0);
            }

            #[doc = concat!("The ", $what, " at the index `i`, or `None` past the last one; one of a sequence the host")]
            /// answered is fetched (a crossing).
            ///
            /// # Errors
            ///
            /// The crossing fails.
            pub fn get(&self, i: usize) -> Result<Option<$item>, $crate::runtime::ExtError> {
                self.0.get(i)
            }

            #[doc = concat!("The ", $what, "s, in order; those of a sequence the host answered are pulled from the host")]
            /// in groups. An item is a `Result`, since a pull crosses.
            pub fn iter(&self) -> impl Iterator<Item = Result<$item, $crate::runtime::ExtError>> + '_ {
                self.0.iter()
            }

            #[doc = concat!("Every ", $what, ", in order, in a `Vec`: those of a sequence the host answered are pulled.")]
            ///
            /// # Errors
            ///
            /// A pull fails.
            pub fn to_vec(&self) -> Result<Vec<$item>, $crate::runtime::ExtError> {
                self.0.iter().collect()
            }
        }

        #[doc = concat!("The sequence of the ", $what, "s `items`, in order.")]
        impl From<Vec<$item>> for $name {
            fn from(items: Vec<$item>) -> $name {
                $name($crate::guest::hand::joined::Joined::from_items(items))
            }
        }

        /// A sequence the host answered is its handle with its length
        /// (`sequence_to_value`); a sequence the extension joined is the
        /// joined list of its parts, which the host reads part by part.
        impl $crate::adapter::values::JsonForm for $name {
            fn to_value(&self) -> serde_json::Value {
                self.0.to_value()
            }

            fn from_value(value: &serde_json::Value) -> Result<Self, $crate::runtime::ExtError> {
                $crate::guest::hand::joined::Joined::from_value(value).map($name)
            }
        }

        impl $crate::guest::hand::joined::Item for $item {
            const SEQUENCE: $crate::adapter::handles::HandleKind = $crate::adapter::handles::HandleKind::$variant;

            #[cfg(feature = "native")]
            fn handle(self) -> $crate::adapter::handles::Handle {
                self.handle
            }

            fn sequence_get(
                sequence: $crate::adapter::handles::Handle,
                i: usize,
            ) -> Result<Option<$item>, $crate::runtime::ExtError> {
                #[cfg(not(feature = "native"))]
                {
                    let reply = $crate::guest::abi::call(sequence, "get", vec![$crate::guest::abi::ToArg::arg(&i)])?;
                    $crate::adapter::values::JsonForm::from_value(&reply)
                }
                #[cfg(feature = "native")]
                {
                    $crate::guest::abi::typed("get", |table| {
                        let this = $crate::adapter::dispatch::entry!(table, sequence, $variant, $variant);
                        let answer = this.get(i);
                        $crate::adapter::typed::FromApi::from_api(answer, table)
                    })
                }
            }

            fn sequence_pull(
                sequence: $crate::adapter::handles::Handle,
                from: usize,
            ) -> Result<Vec<$item>, $crate::runtime::ExtError> {
                #[cfg(not(feature = "native"))]
                {
                    let args = vec![
                        $crate::guest::abi::ToArg::arg(&from),
                        $crate::guest::abi::ToArg::arg(&$crate::guest::abi::PULL_GROUP),
                        $crate::guest::abi::ToArg::arg(&$crate::guest::abi::PULL_BYTES),
                    ];
                    let reply = $crate::guest::abi::call(sequence, "pull", args)?;
                    $crate::adapter::values::JsonForm::from_value(&reply)
                }
                #[cfg(feature = "native")]
                {
                    $crate::guest::abi::typed("pull", |table| {
                        let this = $crate::adapter::dispatch::entry!(table, sequence, $variant, $variant);
                        let answer = this.group(from, $crate::guest::abi::PULL_GROUP, $crate::guest::abi::PULL_BYTES);
                        $crate::adapter::typed::FromApi::from_api($crate::adapter::values::Group(answer), table)
                    })
                }
            }
        }

        /// The api's list, lent as one entry, a sequence, as the host lends
        /// it.
        #[cfg(feature = "native")]
        impl<'c> $crate::adapter::typed::FromApi<'c, Vec<$api>> for $name {
            fn from_api(
                api: Vec<$api>,
                table: &mut $crate::adapter::handles::CallTable<'c>,
            ) -> Result<Self, $crate::runtime::ExtError> {
                let len = api.len();
                let handle = table.lend($crate::adapter::handles::Entry::$variant($crate::adapter::handles::SequenceEntry::new(api)));
                Ok($name($crate::guest::hand::joined::Joined::sequence(handle, len)))
            }
        }

        /// The api's list of the items of every part, as the host reads the
        /// joined list.
        #[cfg(feature = "native")]
        impl<'c> $crate::adapter::typed::IntoApi<'c> for &$name {
            type Api = Vec<$api>;

            fn into_api(self, table: &mut $crate::adapter::handles::CallTable<'c>) -> Result<Self::Api, $crate::runtime::ExtError> {
                let mut items = Vec::with_capacity(self.len());
                for handle in self.0.handles() {
                    $push(table, handle, &mut items)?;
                }
                Ok(items)
            }
        }

        /// As the sequence's reference does.
        #[cfg(feature = "native")]
        impl<'c> $crate::adapter::typed::IntoApi<'c> for $name {
            type Api = Vec<$api>;

            fn into_api(self, table: &mut $crate::adapter::handles::CallTable<'c>) -> Result<Self::Api, $crate::runtime::ExtError> {
                (&self).into_api(table)
            }
        }
    };
}
pub(crate) use sequence_type;
