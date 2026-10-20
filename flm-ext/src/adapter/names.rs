//! The rule that every name a scripted feature declares begins with one of
//! its extension's namespaces and a dot.
//!
//! An extension's namespaces ([`namespaces`]) are its stem and, for a stem
//! `profile-X`, `X` as well: the extension of the profile `X` names its output,
//! tables, stages and diagnostics after the profile (`profile-pdf-latex`
//! declares the output `pdf-latex` and the diagnostic `pdf-latex.failed`).
//!
//! The rule covers the elements, tables, stages and capabilities a feature
//! declares through the builder, and the identifiers of the diagnostics its
//! handlers report. A name another feature owns — a table a stage reads, a
//! macro a feature overrides — is not checked: naming it is the point.
//!
//! A backend's name and an output's name, which users write (`-f typst`), may
//! also be a namespace itself ([`check_backend`]); the table the library's
//! lowering stage writes for such a backend, `flm.lowered.<backend>`, is
//! accepted as a table's name ([`check_table`]).

use std::sync::Arc;

use crate::adapter::durable::DurableTable;
use crate::runtime::ExtError;

/// The beginning of the stem of a profile's extension, `profile-`: an
/// extension whose stem is `profile-X` has `X` among its [`namespaces`].
pub const PROFILE_PREFIX: &str = "profile-";

/// The namespaces of the extension whose stem is `stem`, the beginnings its
/// names may take: the stem, and, for a stem `profile-X` ([`PROFILE_PREFIX`])
/// with `X` not empty, `X`.
pub fn namespaces(stem: &str) -> impl Iterator<Item = &str> {
    let profile = stem.strip_prefix(PROFILE_PREFIX).filter(|name| !name.is_empty());
    std::iter::once(stem).chain(profile)
}

/// Checks that `name`, a `what` (`element`, `table`, `stage`, `capability`,
/// `diagnostic`, `option`) declared by the package `package` of the extension
/// whose stem is `stem`, begins with one of the extension's [`namespaces`]
/// followed by a dot.
///
/// # Errors
///
/// [`ExtError::NameOutsideExtension`] when it does not; its text is "the
/// table `words` declared by the package `_` of the extension `wc` must
/// begin with `wc.`", and its identifier, in a build error, is
/// [`NAME_OUTSIDE_EXTENSION`].
pub fn check(what: &str, name: &str, stem: &str, package: &str) -> Result<(), ExtError> {
    let inside = namespaces(stem).any(|namespace| name.strip_prefix(namespace).is_some_and(|rest| rest.starts_with('.')));
    if inside {
        Ok(())
    } else {
        Err(ExtError::NameOutsideExtension {
            what: what.to_string(),
            name: name.to_string(),
            package: package.to_string(),
            extension: stem.to_string(),
        })
    }
}

/// Checks that `name`, a `what` (`backend` or `output`) declared by the
/// package `package` of the extension whose stem is `stem`, is one of the
/// extension's [`namespaces`] or begins with one followed by a dot.
///
/// # Errors
///
/// [`ExtError::NameOutsideExtension`] when it is neither; its text is "the
/// backend `other` declared by the package `_` of the extension `typst` must
/// be `typst` or begin with `typst.`".
pub fn check_backend(what: &str, name: &str, stem: &str, package: &str) -> Result<(), ExtError> {
    if namespaces(stem).any(|namespace| name == namespace) {
        Ok(())
    } else {
        check(what, name, stem, package)
    }
}

/// What the rule requires of the name of a `what` declared by the extension
/// whose stem is `stem`, as the text of [`ExtError::NameOutsideExtension`]
/// ends: "begin with `wc.`", or, for a backend or an output, "be `typst` or
/// begin with `typst.`"; for the extension `profile-draft`, "begin with
/// `profile-draft.` or `draft.`" and "be `profile-draft` or `draft`, or begin
/// with `profile-draft.` or `draft.`".
pub(crate) fn requirement(what: &str, stem: &str) -> String {
    let namespaces: Vec<&str> = namespaces(stem).collect();
    let under = namespaces.iter().map(|namespace| format!("`{namespace}.`")).collect::<Vec<_>>().join(" or ");
    if what == "backend" || what == "output" {
        let itself = namespaces.iter().map(|namespace| format!("`{namespace}`")).collect::<Vec<_>>().join(" or ");
        let comma = if namespaces.len() > 1 { "," } else { "" };
        format!("be {itself}{comma} or begin with {under}")
    } else {
        format!("begin with {under}")
    }
}

/// Checks the name of a table the package declares: [`check`], or the name
/// `flm.lowered.<backend>` of the table of a backend's lowered trees, where
/// `<backend>` passes [`check_backend`].
///
/// # Errors
///
/// As [`check`], for a name of neither form.
pub fn check_table(name: &str, stem: &str, package: &str) -> Result<(), ExtError> {
    let lowered = name.strip_prefix(LOWERED_PREFIX).is_some_and(|backend| check_backend("backend", backend, stem, package).is_ok());
    if lowered {
        Ok(())
    } else {
        check("table", name, stem, package)
    }
}

/// The beginning of the name of a backend's lowered-trees table
/// (`flm.lowered.<backend>`, as `lowered_table_name` writes it).
const LOWERED_PREFIX: &str = "flm.lowered.";

/// The identifier of the name rule's failure, which a registry build error
/// carries at the start of its detail.
pub const NAME_OUTSIDE_EXTENSION: &str = "flm.ext.name-outside-extension";

/// Who declares a name: the extension's stem and the package's name within
/// the extension, carried by the façades that check the rule; and the durable
/// table of the feature's registration, carried by every handler object the
/// registration makes, so that the table lives as long as the registry holds
/// them.
#[derive(Debug, Clone)]
pub(crate) struct Owner {
    /// The extension's stem.
    pub(crate) stem: String,
    /// The package's name within the extension.
    pub(crate) package: String,
    /// The durable table of the feature's registration.
    pub(crate) durable: Arc<DurableTable>,
}

impl Owner {
    /// The owner of one registration of the feature `feature` of the package
    /// `package` of the extension whose stem is `stem`, with a new durable
    /// table.
    pub(crate) fn new(stem: &str, package: &str, feature: &str) -> Owner {
        Owner {
            stem: stem.to_string(),
            package: package.to_string(),
            durable: Arc::new(DurableTable::new(stem, package, feature)),
        }
    }

    /// [`check`] for this owner.
    pub(crate) fn check(&self, what: &str, name: &str) -> Result<(), ExtError> {
        check(what, name, &self.stem, &self.package)
    }

    /// [`check_backend`] for this owner.
    pub(crate) fn check_backend(&self, what: &str, name: &str) -> Result<(), ExtError> {
        check_backend(what, name, &self.stem, &self.package)
    }

    /// [`check_table`] for this owner.
    pub(crate) fn check_table(&self, name: &str) -> Result<(), ExtError> {
        check_table(name, &self.stem, &self.package)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_under_the_stem_and_a_dot_passes() {
        assert_eq!(check("table", "wc.words", "wc", "_"), Ok(()));
        assert_eq!(check("element", "wc.macro.shout", "wc", "core"), Ok(()));
    }

    #[test]
    fn a_name_outside_the_stem_fails() {
        for name in ["words", "wcx.words", "wc", "other.wc.words", ""] {
            let error = check("table", name, "wc", "core").expect_err(name);
            assert_eq!(
                error.to_string(),
                format!("the table `{name}` declared by the package `core` of the extension `wc` must begin with `wc.`")
            );
        }
    }

    #[test]
    fn a_backend_or_an_output_may_be_the_stem_or_under_it() {
        for name in ["typst", "typst.draft", "typst.a.b"] {
            assert_eq!(check_backend("backend", name, "typst", "_"), Ok(()));
            assert_eq!(check_backend("output", name, "typst", "_"), Ok(()));
        }
        for name in ["other", "typstx", "typst-", "other.typst", ""] {
            let error = check_backend("backend", name, "typst", "_").expect_err(name);
            assert_eq!(
                error.to_string(),
                format!("the backend `{name}` declared by the package `_` of the extension `typst` must be `typst` or begin with `typst.`")
            );
            let error = check_backend("output", name, "typst", "main").expect_err(name);
            assert_eq!(
                error.to_string(),
                format!("the output `{name}` declared by the package `main` of the extension `typst` must be `typst` or begin with `typst.`")
            );
        }
    }

    #[test]
    fn a_profile_s_extension_names_things_under_the_profile_too() {
        assert_eq!(check_backend("output", "draft", "profile-draft", "_"), Ok(()));
        assert_eq!(check("diagnostic", "draft.failed", "profile-draft", "_"), Ok(()));
        assert_eq!(check("table", "profile-draft.document", "profile-draft", "_"), Ok(()));
        assert!(check_backend("output", "other", "profile-draft", "_").is_err());
        assert!(check("table", "other.document", "profile-draft", "_").is_err());
    }

    #[test]
    fn a_table_of_lowered_trees_is_named_after_its_backend() {
        for name in ["flm.lowered.tb", "flm.lowered.tb.sub", "tb.words"] {
            assert_eq!(check_table(name, "tb", "_"), Ok(()));
        }
        for name in ["flm.lowered.other", "flm.lowered.", "flm.lowered.tbx", "flm.lowered", "flm.output.tb", "tb"] {
            let error = check_table(name, "tb", "_").expect_err(name);
            assert_eq!(
                error.to_string(),
                format!("the table `{name}` declared by the package `_` of the extension `tb` must begin with `tb.`")
            );
        }
    }
}
