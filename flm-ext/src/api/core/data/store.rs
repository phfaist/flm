//! Mirrors [`flm_core::core::data::store`]: what a stage writes through.

use serde_json::Value;

use crate::api::techy::error::Diagnostic;
use crate::runtime::ExtError;
use crate::sealed::Sealed;

/// Mirrors [`StageOut`](flm_core::core::data::store::StageOut): what a running
/// stage writes — table pieces and diagnostics.
///
/// What differs: [`table`](StageOut::table) takes the table's name and always
/// answers a piece of the extension's own table; [`diags`](StageOut::diags) is
/// a method pushing one diagnostic, where `flm-core` has a public field.
///
/// The trait is sealed: the adapter alone implements it.
pub trait StageOut: Sealed {
    /// Mirrors [`StageOut::table`](flm_core::core::data::store::StageOut::table):
    /// the piece of the table named `name` this stage writes. What differs: by
    /// name; always a table of the extension's own.
    ///
    /// # Errors
    ///
    /// The stage did not declare that it writes the table ([`ExtError::Core`]).
    fn table(&mut self, name: &str) -> Result<&mut dyn TablePiece, ExtError>;

    /// Mirrors a push on the field
    /// [`StageOut::diags`](flm_core::core::data::store::StageOut::diags):
    /// reports `diagnostic`. What differs: a method. A diagnostic with no
    /// place stands at the stage's tree.
    ///
    /// # Errors
    ///
    /// The diagnostic's identifier does not begin with the extension's stem and a dot, or its data
    /// cannot cross.
    fn diags(&mut self, diagnostic: Diagnostic) -> Result<(), ExtError>;
}

/// No flm-core counterpart: the piece of an extension's own table a stage
/// writes (the method `push` of the adapter's `ExtTable`, a table type of this
/// crate).
///
/// The trait is sealed: the adapter alone implements it.
pub trait TablePiece: Sealed {
    /// Appends `item` to the table.
    ///
    /// # Errors
    ///
    /// The item cannot cross (a floating-point number, a reserved key).
    fn push(&mut self, item: Value) -> Result<(), ExtError>;
}

fn _object_safe(_: &dyn StageOut, _: &dyn TablePiece) {}
