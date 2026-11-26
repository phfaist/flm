//! This module defines [`LangOwned`], the cell in which an element stores a
//! value that was built against the language the element belongs to.
//!
//! # The reference cycle that the cell prevents
//!
//! A [`BuiltLang`] owns its techy packages, and each package owns the elements
//! declared in it, with their data and their aspects. A tree parsed under the
//! language refers back to the language: each node refers to its parsing
//! state, the parsing state refers to the packages, and a callable node refers
//! to its element.
//!
//! A feature often builds such a tree for one of its elements, once the
//! language is built. For instance, the template of a [user
//! definition](crate::lang::rewrites::userdefs::UserDefinition) is compiled
//! into a tree by a [`LateInit`](crate::core::registry::LateInit) build hook,
//! and the element of the definition reads the tree at each use site. If the
//! element owned the tree (through an `Arc`), the element and the tree would
//! refer to each other, and neither would ever be freed: the memory of the
//! whole language would stay allocated after the registry is dropped.
//!
//! # The ownership rule
//!
//! A value that refers to a built language (a parsed tree, a parsing state, a
//! compiled template) is never owned by an element of that language, directly
//! or through other values. The language owns the value, and the element
//! stores a [`LangOwned`], which refers to the value without owning it.
//!
//! The rule concerns only what an element's data and aspects own. A lowering
//! rule, a stage, a rewrite or a service is owned by the registry and not by
//! the language, so it may own such a value directly.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, OnceLock, Weak};

use super::BuiltLang;

/// A `LangOwned<T>` is a write-once cell for a value of type `T` that is
/// owned by a [`BuiltLang`] and read by an element of that language.
///
/// Use it for every value that an element's data or aspect needs and that
/// refers to the language the element belongs to: a compiled template, a
/// parsed tree, a parsing state. The [module documentation](self) explains
/// why the element cannot own such a value.
///
/// The cell is created empty ([`LangOwned::new`]), usually when the feature
/// creates the element's data. [`set`](LangOwned::set) gives the value to the
/// language, usually in the feature's
/// [`LateInit`](crate::core::registry::LateInit) build hook.
/// [`get`](LangOwned::get) returns the value for as long as the language
/// exists. [`LangOwned::with_value`] creates a cell that already contains its
/// value.
///
/// # Examples
///
/// ```
/// use flm_core::lang::{build_language, LangInputs, LangOwned};
///
/// let cell: LangOwned<String> = LangOwned::new();
/// assert!(cell.get().is_none());
///
/// let lang = build_language(LangInputs::new());
/// assert!(cell.set(&lang, String::from("compiled against `lang`")));
/// assert_eq!(cell.get().as_deref().map(String::as_str), Some("compiled against `lang`"));
///
/// // The language owns the value: the cell is empty once the language is dropped.
/// drop(lang);
/// assert!(cell.get().is_none());
/// ```
pub struct LangOwned<T> {
    value: OnceLock<Weak<T>>,
}

impl<T> LangOwned<T> {
    /// Creates an empty cell.
    pub const fn new() -> LangOwned<T> {
        LangOwned { value: OnceLock::new() }
    }

    /// Returns the value, or `None` when [`set`](LangOwned::set) has not been
    /// called yet or the language that owns the value has been dropped.
    ///
    /// A registry owns its language, so the value is available during every
    /// operation that runs with the registry: a parse, a rewrite, a stage, a
    /// lowering rule.
    pub fn get(&self) -> Option<Arc<T>> {
        self.value.get()?.upgrade()
    }
}

impl<T: Any + Send + Sync> LangOwned<T> {
    /// Creates a cell that contains `value` and makes `lang` the owner of
    /// `value`. `lang` must be the language that `value` refers to.
    ///
    /// Returns the cell and the value. Use this constructor when the value is
    /// built at the same time as the cell, e.g. for an entry of a cache that
    /// an element reads.
    pub fn with_value(lang: &BuiltLang, value: T) -> (LangOwned<T>, Arc<T>) {
        let value = Arc::new(value);
        let cell = LangOwned { value: OnceLock::from(Arc::downgrade(&value)) };
        lang.own(Arc::clone(&value) as Arc<dyn Any + Send + Sync>);
        (cell, value)
    }

    /// Stores `value` in the cell and makes `lang` its owner. `lang` must be
    /// the language that `value` refers to.
    ///
    /// Returns `true` if the value was stored. The cell is written once: a
    /// later call drops its `value`, changes nothing and returns `false`.
    pub fn set(&self, lang: &BuiltLang, value: T) -> bool {
        let value = Arc::new(value);
        if self.value.set(Arc::downgrade(&value)).is_err() {
            return false;
        }
        lang.own(value);
        true
    }
}

impl<T> Default for LangOwned<T> {
    fn default() -> LangOwned<T> {
        LangOwned::new()
    }
}

impl<T> fmt::Debug for LangOwned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LangOwned").field("available", &self.get().is_some()).finish()
    }
}
