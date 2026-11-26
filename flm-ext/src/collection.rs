//! The format of a collection: one WebAssembly module that holds several
//! extensions whole. The code of its extensions written in Rust is the
//! module's code; the files of every extension, the manifest among them, are
//! in the module's custom section `flmextensions` ([`SECTION`]), each as it is
//! in the extension's folder: a manifest is its YAML text, unchanged, and no
//! file has a second form. A custom section is part of the module's file and
//! is not loaded into an instance's memory.
//!
//! The section is a sequence of records, one per file, and the order of the
//! records carries no meaning. A record is the following bytes, each number an
//! unsigned 32-bit integer in little-endian order:
//!
//! | Bytes | What |
//! |---|---|
//! | 0..4 | the record's length in bytes, these four included |
//! | 4..8 | the record's format version, [`VERSION`] |
//! | 8..12 | the length of the folder's name |
//! | 12..16 | the length of the stem |
//! | 16..20 | the length of the file's path |
//! | 20.. | the folder's name (`data`, `js` or `rust`), the stem and the file's path, as UTF-8 text, then the file's bytes |
//!
//! The file's path is relative to the extension's folder, with `/` between the
//! names. The length and the version are the first two numbers of a record in
//! every version of the format, so that a reader tells a record it does not
//! read from one that is cut short.
//!
//! The macro [`record!`] writes one record as a static of the crate it is
//! invoked in, which rustc places in the custom section when the crate is
//! compiled for `wasm32`; the statics of several records stand one after
//! another in the section of the module. [`read_section`] reads a section back
//! into the extensions it holds. The crate `flm-extensions` builds a
//! collection with its cargo feature `collection`, whose build script invokes
//! [`record!`] once per file of every extension whose feature is on. A tool
//! that removes custom sections from a module (`wasm-strip`) removes the
//! files of a collection.
//!
//! # `unsafe`
//!
//! The attribute `#[unsafe(link_section = "flmextensions")]` on the static
//! that [`record!`] writes in the invoking crate, compiled only for `wasm32`
//! in a build without the feature `native`: the static is plain bytes, as the
//! attribute requires on `wasm32`.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

/// The name of the custom section that holds the files of a collection. The
/// macro [`record!`] writes the same name in its attribute, which takes a
/// literal.
pub const SECTION: &str = "flmextensions";

/// The format version of the records that [`record!`] writes, the one version
/// that [`read_section`] reads.
pub const VERSION: u32 = 1;

/// The bytes of a record before its texts: five numbers of four bytes.
const HEADER_LEN: usize = 20;

/// One extension of a collection: the folder that holds it among the folders
/// of extensions (`data`, `js`, `rust`), its stem, and its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionExtension<'a> {
    /// The folder that holds the extension: `data`, `js` or `rust`.
    pub folder: &'a str,
    /// The extension's stem: the name of its folder, or of its file without
    /// `.yaml`.
    pub stem: &'a str,
    /// The extension's files, in path order.
    pub files: Vec<SectionFile<'a>>,
}

/// One file of an extension of a collection ([`SectionExtension`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionFile<'a> {
    /// The path relative to the extension's folder, with `/` between the
    /// names.
    pub path: &'a str,
    /// The file's bytes, as they are in the extension's folder.
    pub bytes: &'a [u8],
}

/// Why [`read_section`] refused a section: the record at fault, by its first
/// byte in the section, and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionError {
    /// The first byte of the record at fault, from the start of the section.
    pub offset: usize,
    /// What is wrong with the record.
    pub kind: SectionErrorKind,
}

/// What is wrong with a record of a section ([`SectionError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionErrorKind {
    /// The section ends inside the record: `needed` bytes, of which the
    /// section holds `remaining`.
    Truncated {
        /// The bytes the record needs: its length and version, or the length
        /// it states.
        needed: usize,
        /// The bytes from the record's start to the section's end.
        remaining: usize,
    },
    /// The record is of a format version that the reader does not read.
    Version(u32),
    /// The record's texts do not fit in the length that it states.
    Length(u32),
    /// A text of the record (`folder`, `stem` or `path`) is not UTF-8.
    NotText(&'static str),
    /// A second record of the same file of the same extension.
    DuplicateFile {
        /// The extension's folder.
        folder: String,
        /// The extension's stem.
        stem: String,
        /// The file's path.
        path: String,
    },
}

impl fmt::Display for SectionError {
    /// "Truncated record at byte N of the section ‘flmextensions’ (…)",
    /// "Unsupported format version V of the record at byte N …", "Invalid
    /// length L of the record at byte N …", "Invalid UTF-8 in the stem of the
    /// record at byte N …", "Duplicate file ‘P’ of the extension ‘F/S’ in the
    /// section ‘flmextensions’".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let offset = self.offset;
        match &self.kind {
            SectionErrorKind::Truncated { needed, remaining } => write!(
                f,
                "Truncated record at byte {offset} of the section ‘{SECTION}’ ({needed} bytes needed, {remaining} remain)"
            ),
            SectionErrorKind::Version(version) => write!(
                f,
                "Unsupported format version {version} of the record at byte {offset} of the section ‘{SECTION}’ \
                 (supported: {VERSION})"
            ),
            SectionErrorKind::Length(length) => write!(
                f,
                "Invalid length {length} of the record at byte {offset} of the section ‘{SECTION}’ \
                 (shorter than its header and texts)"
            ),
            SectionErrorKind::NotText(field) => write!(
                f,
                "Invalid UTF-8 in the {field} of the record at byte {offset} of the section ‘{SECTION}’"
            ),
            SectionErrorKind::DuplicateFile { folder, stem, path } => {
                write!(f, "Duplicate file ‘{path}’ of the extension ‘{folder}/{stem}’ in the section ‘{SECTION}’")
            }
        }
    }
}

impl std::error::Error for SectionError {}

/// The extensions that the section `section` holds (the bytes of a module's
/// custom section `flmextensions`), ordered by folder then stem, each with its
/// files in path order. An empty section holds none.
///
/// # Errors
///
/// A record that the section cuts short, a record of a format version other
/// than [`VERSION`], a record whose texts do not fit in its length or are not
/// UTF-8, and two records of the same file of the same extension.
pub fn read_section(section: &[u8]) -> Result<Vec<SectionExtension<'_>>, SectionError> {
    let mut extensions: BTreeMap<(&str, &str), BTreeMap<&str, &[u8]>> = BTreeMap::new();
    let mut offset = 0;
    while offset < section.len() {
        let error = |kind| SectionError { offset, kind };
        let rest = &section[offset..];
        if rest.len() < 8 {
            return Err(error(SectionErrorKind::Truncated { needed: 8, remaining: rest.len() }));
        }
        let length = number(rest, 0);
        if length as usize > rest.len() {
            return Err(error(SectionErrorKind::Truncated { needed: length as usize, remaining: rest.len() }));
        }
        let version = number(rest, 4);
        if version != VERSION {
            return Err(error(SectionErrorKind::Version(version)));
        }
        let record = &rest[..length as usize];
        if record.len() < HEADER_LEN {
            return Err(error(SectionErrorKind::Length(length)));
        }
        let lengths = [number(record, 8), number(record, 12), number(record, 16)].map(|n| n as usize);
        let texts_end = lengths.iter().try_fold(HEADER_LEN, |end, &n| end.checked_add(n));
        let Some(texts_end) = texts_end.filter(|&end| end <= record.len()) else {
            return Err(error(SectionErrorKind::Length(length)));
        };
        let mut start = HEADER_LEN;
        let mut text = |len: usize, field: &'static str| {
            let bytes = &record[start..start + len];
            start += len;
            std::str::from_utf8(bytes).map_err(|_| error(SectionErrorKind::NotText(field)))
        };
        let folder = text(lengths[0], "folder")?;
        let stem = text(lengths[1], "stem")?;
        let path = text(lengths[2], "path")?;
        match extensions.entry((folder, stem)).or_default().entry(path) {
            Entry::Vacant(vacant) => {
                vacant.insert(&record[texts_end..]);
            }
            Entry::Occupied(_) => {
                return Err(error(SectionErrorKind::DuplicateFile {
                    folder: folder.to_string(),
                    stem: stem.to_string(),
                    path: path.to_string(),
                }));
            }
        }
        offset += record.len();
    }
    Ok(extensions
        .into_iter()
        .map(|((folder, stem), files)| SectionExtension {
            folder,
            stem,
            files: files.into_iter().map(|(path, bytes)| SectionFile { path, bytes }).collect(),
        })
        .collect())
}

/// The number at `at` of `bytes` (four bytes, little-endian), which hold it.
fn number(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The bytes of a record before the file's bytes (its numbers and texts),
/// written by [`record!`]; not part of the crate's interface.
#[doc(hidden)]
pub const fn head_len(folder: &str, stem: &str, path: &str) -> usize {
    HEADER_LEN + folder.len() + stem.len() + path.len()
}

/// The numbers and texts of the record of the file `path` of the extension
/// `stem` of the folder `folder`, whose bytes are `bytes_len` long; `N` is
/// [`head_len`] of the three texts. Written by [`record!`]; not part of the
/// crate's interface.
#[doc(hidden)]
pub const fn head<const N: usize>(folder: &str, stem: &str, path: &str, bytes_len: usize) -> [u8; N] {
    assert!(N == head_len(folder, stem, path), "the head's length must be head_len of its texts");
    assert!(N + bytes_len <= u32::MAX as usize, "a record of a collection must be shorter than 4 GiB");
    let mut head = [0u8; N];
    let numbers = [(N + bytes_len) as u32, VERSION, folder.len() as u32, stem.len() as u32, path.len() as u32];
    let mut at = 0;
    while at < numbers.len() {
        let bytes = numbers[at].to_le_bytes();
        let mut i = 0;
        while i < 4 {
            head[4 * at + i] = bytes[i];
            i += 1;
        }
        at += 1;
    }
    let texts = [folder.as_bytes(), stem.as_bytes(), path.as_bytes()];
    let mut start = HEADER_LEN;
    let mut t = 0;
    while t < texts.len() {
        let mut i = 0;
        while i < texts[t].len() {
            head[start + i] = texts[t][i];
            i += 1;
        }
        start += texts[t].len();
        t += 1;
    }
    head
}

/// A record as [`record!`] lays it out in a static: its numbers and texts,
/// then the file's bytes; plain bytes, with no padding. Not part of the
/// crate's interface.
#[doc(hidden)]
#[repr(C)]
pub struct Record<const H: usize, const B: usize> {
    /// The numbers and texts ([`head`]).
    pub head: [u8; H],
    /// The file's bytes.
    pub bytes: [u8; B],
}

/// One record of the section `flmextensions`: the file at `$file` (a path
/// that `include_bytes!` reads, absolute or relative to the invoking source
/// file), as the file `$path` of the extension `$stem` of the folder
/// `$folder`; each argument a string literal:
/// `flm_ext::collection::record!("data", "lecturenotes", "flmextension.yaml", "/…/flmextension.yaml");`.
///
/// It writes a static that rustc places in the module's custom section
/// `flmextensions`, compiled only for `wasm32` in a build where `flm-ext`'s
/// cargo feature `native` is off, as the exports of
/// `flm_ext::guest::extensions!` are (the crate is then a WebAssembly module
/// of its own). In any other build it writes nothing: the file is neither
/// read nor linked into a program.
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_collection_record {
    ($folder:literal, $stem:literal, $path:literal, $file:literal) => {
        $crate::__flm_collection_static!($folder, $stem, $path, $file);
    };
}

/// The static of one record for [`record!`], in `flm-ext`'s form without the
/// cargo feature `native`: compiled for `wasm32`, the static; for another
/// target, nothing. A `cfg` written in the body of a `macro_rules!` macro is
/// read in the crate where the macro expands, so `flm-ext` chooses by its own
/// feature which form of this macro it defines.
#[cfg(not(feature = "native"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_collection_static {
    ($folder:literal, $stem:literal, $path:literal, $file:literal) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            const FOLDER: &str = $folder;
            const STEM: &str = $stem;
            const PATH: &str = $path;
            const BYTES: &[u8] = include_bytes!($file);
            // The name is `flm_ext::collection::SECTION`; the attribute takes a literal.
            #[unsafe(link_section = "flmextensions")]
            #[used]
            static RECORD: $crate::collection::Record<
                { $crate::collection::head_len(FOLDER, STEM, PATH) },
                { BYTES.len() },
            > = $crate::collection::Record {
                head: $crate::collection::head(FOLDER, STEM, PATH, BYTES.len()),
                bytes: match BYTES.first_chunk() {
                    Some(bytes) => *bytes,
                    None => panic!("the bytes of a record are the file's bytes"),
                },
            };
        };
    };
}

/// The static of one record for [`record!`], in `flm-ext`'s form with the
/// cargo feature `native`: nothing, on every target, since the crate is
/// compiled into a program (see the form without the feature).
#[cfg(feature = "native")]
#[doc(hidden)]
#[macro_export]
macro_rules! __flm_collection_static {
    ($folder:literal, $stem:literal, $path:literal, $file:literal) => {};
}

#[doc(inline)]
pub use crate::__flm_collection_record as record;

#[cfg(test)]
mod tests {
    use super::*;

    /// The record of the file `path` of the extension `stem` of the folder
    /// `folder`, holding `bytes`, as [`record!`] lays it out.
    fn record(folder: &str, stem: &str, path: &str, bytes: &[u8]) -> Vec<u8> {
        let mut record = Vec::new();
        record.extend_from_slice(&((head_len(folder, stem, path) + bytes.len()) as u32).to_le_bytes());
        record.extend_from_slice(&VERSION.to_le_bytes());
        for text in [folder, stem, path] {
            record.extend_from_slice(&(text.len() as u32).to_le_bytes());
        }
        for text in [folder, stem, path] {
            record.extend_from_slice(text.as_bytes());
        }
        record.extend_from_slice(bytes);
        record
    }

    #[test]
    fn a_record_cut_short_is_refused() {
        let mut section = record("data", "notes", "flmextension.yaml", b"name: notes\n");
        let second = section.len();
        section.extend(record("data", "notes", "README.md", b"# Notes\n"));
        assert_eq!(read_section(&section).expect("whole").len(), 1);
        for end in [second + 3, second + 12, section.len() - 1] {
            let error = read_section(&section[..end]).expect_err("cut short");
            assert_eq!(error.offset, second);
            assert!(matches!(error.kind, SectionErrorKind::Truncated { .. }), "{error:?}");
        }
    }

    #[test]
    fn a_record_of_another_format_version_is_refused() {
        let mut section = record("rust", "pair", "flmextension.yaml", b"engine: wasm\n");
        section[4..8].copy_from_slice(&(VERSION + 1).to_le_bytes());
        let error = read_section(&section).expect_err("another version");
        assert_eq!(error, SectionError { offset: 0, kind: SectionErrorKind::Version(VERSION + 1) });
    }
}
