//! Tools for inspecting and testing what the library holds in memory: parsed
//! and processed trees, computed tables, and diagnostics.
//!
//! The library's own tests, the tests of a program or extension crate built on
//! it, and the `flm dump` command of the `flm` program all use this module. It
//! is ordinary public interface rather than test-only code: no cargo feature
//! switches it on, so a program writes its test fixtures against the same
//! functions the library's tests use.
//!
//! The module renders what the library holds in two different forms, and keeps
//! them apart:
//!
//! - **The projection** ([`project`]) is a one-way rendering of a tree, a node,
//!   a [table](crate::core::data::table::Table) or a
//!   [diagnostic](crate::guide::concepts_overview#diagnostics) as a techy
//!   serialized value ([`SerialValue`](crate::techy::serialize::SerialValue),
//!   techy's format-independent data model),
//!   in a vocabulary this library owns and keeps stable (`elem`, `chars`,
//!   `args`, `children`, and so on; spans as `line:col` on request). It is what
//!   tests compare and what `flm dump --view compact` prints. It never shows a
//!   parsing state, and it cannot be read back into a tree.
//! - **The wire form** ([`session`]) is techy's serialization of a parse, which
//!   [`crate::lang::serialize`] implements for FLM. It is lossless and verbose,
//!   and it can be read back into in-memory objects with their sharing and
//!   identity intact. `flm dump --view wire` prints it as JSON, one segment
//!   (the unit of output a session emits) per line ([`json::segment_line`]).
//!
//! A program that writes such data as JSON opens its output with a header,
//! [`Meta`]: the kind of output, the version of FLM that wrote it, and the
//! version of the kind's format.
//!
//! # Map
//!
//! - [`project`] — the projection: [`Projector`], [`ProjectOptions`], the
//!   free functions [`project_tree`], [`project_node`], [`project_diagnostics`]
//!   and the others, and [`TableProjection`], through which every table
//!   projects itself.
//! - [`golden`] — golden files: a test's expected output kept in a file,
//!   compared with [`assert_text_golden`] or [`assert_json_golden`] and
//!   rewritten when the environment variable [`UPDATE_GOLDEN`] is `1`.
//! - [`diag`] — assertions on diagnostics by identifier and data, never by
//!   message: [`expect`], [`expect_none`] and the narrowing steps of
//!   [`DiagExpect`].
//! - [`json`] — the JSON text of serialized values and of wire segments:
//!   [`to_json`], [`to_json_pretty`], [`from_json`], [`from_serde`],
//!   [`segment_line`], [`segment_from_line`].
//! - [`meta`] — the header that opens a JSON output written about FLM (its
//!   kind, the version of FLM that wrote it, the version of its format):
//!   [`Meta`].
//! - [`session`] — the serialization sessions over a built language
//!   ([`writer`], [`reader`], [`reader_with`]) and the reading and writing of
//!   fragments and trees through them ([`write_fragment`], [`read_fragment`],
//!   [`write_tree`], [`read_tree`]).
//!
//! Every public item of the submodules is re-exported here.
//!
//! [`golden`] is the only part of `flm-core` that reads or writes files; no
//! stage, rule or entry point of the processing touches the file system.

pub mod diag;
pub mod golden;
pub mod json;
pub mod meta;
pub mod project;
pub mod session;

pub use diag::{DiagExpect, ExpectDiag, expect, expect_none};
pub use golden::{
    GoldenError, GoldenMode, UPDATE_GOLDEN, assert_json_golden, assert_text_golden,
    check_json_golden, check_text_golden,
};
pub use json::{
    JsonError, from_json, from_serde, segment_from_line, segment_line, to_json, to_json_pretty,
};
pub use meta::Meta;
pub use project::{
    ProjectOptions, Projector, TableProjection, TreeSource, node_ref_value, project_diagnostic,
    project_diagnostics, project_node, project_nodes, project_payload, project_table,
    project_tree, tree_ref_value,
};
pub use session::{
    AmbiguousElement, ElementResolver, ElementTable, FALLBACK_GATE_IDENTIFIER, FRAGMENT_IDENTIFIER,
    FRAGMENTS_TABLE, FragmentIndex, FragmentSerdeDriver, PRESENTATION_IDENTIFIER, PayloadReader,
    PayloadReaders, ReadEnvironment, SYNTHESIZED_IDENTIFIER, WriteEnvironment, fragments_table,
    known_providers, read_fragment, read_tree, reader, reader_with, register, write_fragment,
    write_tree, writer,
};
