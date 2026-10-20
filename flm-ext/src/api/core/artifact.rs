//! Mirrors [`flm_core::core::artifact`]: the files an output consists of, as
//! data.
//!
//! What differs: the artifact types are re-exported as plain data; the set
//! itself ([`ArtifactSet`](flm_core::core::artifact::ArtifactSet)) is a table
//! type a stage writes one artifact at a time
//! ([`TablePiece::push`](crate::api::core::data::store::TablePiece::push) on
//! a table declared with
//! [`StandardTable::ArtifactSet`](crate::api::core::registry::StandardTable::ArtifactSet))
//! and reads as data ([`StageCx::get`](crate::api::core::run::StageCx::get),
//! see [below](#reading-the-files-of-an-output)), never a value an extension
//! builds. The host's naming service (`ArtifactNamer`, `NamingScope`,
//! `ArtifactRef`) is not mirrored: a read gives the path of each file
//! instead. A file reference (`ArtifactData::FileRef`), which no feature of
//! the library produces, is not mirrored either.
//!
//! # The data form of an artifact
//!
//! Across a binding, an artifact is the map
//!
//! ```text
//! ARTIFACT = { id: { owner, name }, media_type, entry, data, fixed_path, refs, extension }
//! data     = { bytes: CONTENT } | { files: [[path, CONTENT], …] }
//! CONTENT  = { text: TEXT } | { base64: TEXT }
//! ```
//!
//! `id`, `media_type` and `data` are required; the other fields may be left
//! out or `null` (`entry` then `false`, `refs` empty). `refs` is a list of
//! identifiers `{ owner, name }`. `data` holds the bytes of a one-file
//! artifact, or the files of a folder artifact, each by its path within the
//! folder.
//!
//! A `CONTENT`, the bytes of a file, is `{ text }` for bytes that are UTF-8
//! text, the bytes being those of the text, and `{ base64 }` for bytes of
//! any kind, written in base64 (the standard alphabet, with padding). An
//! extension that writes an artifact may also give a `CONTENT` as a bare
//! text, the short form of `{ text }`. When an artifact is read, each
//! `CONTENT` is `{ text }` when the bytes are UTF-8 and `{ base64 }`
//! otherwise. The host service `programs` takes and gives the files of a
//! program in the same two forms, so an extension passes a file between an
//! artifact and a program without converting it.
//!
//! # Reading the files of an output
//!
//! A stage that declares a read of a table of an output's files gets, from
//! [`StageCx::get`](crate::api::core::run::StageCx::get), the value
//! ([`TableValue::Projection`](crate::api::core::data::table::TableValue::Projection))
//!
//! ```text
//! { artifacts: [ARTIFACT, …], violations: [TEXT, …] }
//! ```
//!
//! `artifacts` holds every artifact of the set, ordered by identity, in the
//! data form above with one more field, `path`: the path, relative to the
//! folder the output is written to, at which the host writes the artifact (a
//! folder artifact's files are written under it, each at `path/FILE`).
//! `path` is the artifact's `fixed_path` when it has one, else the path the
//! host's naming of files gives it (the rule of
//! [`path_of`](flm_core::core::artifact::path_of)); it is `null` when the
//! artifact has no fixed path and the host contributes no naming of files.
//! The links between the files of an output (a LaTeX document loading its
//! style file, a page showing an image) are written with these paths, so a
//! stage that hands the files to another program, each at its `path`, keeps
//! the links intact. `violations` holds the text of each problem recorded
//! while the set was combined from the pieces several stages wrote (two
//! files of one identity, two main files); the stage that finishes the
//! output reports them.
//!
//! An artifact read can be pushed into another table of files as it is: a
//! push ignores the field `path`. A read refuses a set holding an artifact
//! that refers to a file instead of holding its bytes
//! ([`ExtError::Value`](crate::ExtError::Value)).

/// Mirrors the artifact types of [`flm_core::core::artifact`]: an
/// [`Artifact`] (one file, or a folder of files, of an output), its identity
/// [`ArtifactId`] and its content [`ArtifactData`]. Nothing differs: they are
/// plain data, re-exported; across a binding they cross in the data form the
/// [module documentation](self) gives.
pub use flm_core::core::artifact::{Artifact, ArtifactData, ArtifactId};

/// Mirrors [`media_type_of`](flm_core::core::artifact::media_type_of): the
/// media type the host gives a file of extension `extension` (without the
/// dot, in any case), or `None` for an extension the table does not list.
/// What differs: an owned `String`.
pub fn media_type_of(extension: &str) -> Option<String> {
    flm_core::core::artifact::media_type_of(extension).map(str::to_string)
}
