//! The host's citation provider: it obtains the bibliographic details of each work a
//! document cites, fetched from the arXiv or doi.org or read from bibliography files, and
//! makes each work's citation entry, the content shown for the work in the list of cited
//! works, or in its citation marks when the library shows the citation entries in place.
//! The module exists when the cargo feature `citations` of flm-host is enabled.
//!
//! The library's `citations` feature
//! ([`flm_core::stdlib::features::citations`]) handles `\cite` and `\R`: it numbers the
//! cited works, shows the citation marks and lists the cited works. It performs no input or
//! output, so it asks *citation providers* ([`CitationProvider`]) for the citation entry of
//! each *citation key* (the text written between the braces of `\cite`, which refers to a
//! cited work). This module holds
//! the provider that the `flm` program uses, [`StandardProvider`], and two outputs that
//! write the details of the cited works into a file (see "The exports" below). The feature
//! `citations-standard` of the host's package `std`
//! ([`crate::stdpkg::citations_standard`]) registers both; its settings are
//! [`ProviderConfig`].
//!
//! # Citation keys, prefixes and record sources
//!
//! A *record* is the bibliographic details of one work (authors, title, journal, year) as
//! data in the format CSL-JSON, the JSON form of the Citation Style Language. A *record
//! source* is a place the provider obtains records from. The provider splits a citation
//! key, such as `arxiv:1211.1037`, at its first colon: the part before it, trimmed and
//! lowercased, is the *prefix*, which chooses the record source registered under it; the
//! rest is handed to that record source as written. The setting `sources`
//! ([`ProviderConfig::sources`], [`SourceSettings`]) maps each prefix to a record source.
//! By default there are four, each registered under the prefix of its kind's name:
//!
//! - `arxiv`: the record of a preprint on the arXiv. When the arXiv gives the DOI (digital
//!   object identifier) of the work's published version, the record is the published
//!   version's, fetched from doi.org and kept under the arXiv citation key;
//! - `doi`: the record that doi.org gives for a DOI;
//! - `manual`: the rest of the citation key is the citation entry itself, written in FLM:
//!   `\cite{manual:{A. Author, Some title (2024)}}`. A comma inside braces belongs to the
//!   citation key, and the citation key may hold FLM commands,
//!   `\cite{manual:\emph{Some title}}`;
//! - `bib`: the record of that name (its field `id`) in the author's bibliography files,
//!   written in CSL-JSON or CSL-YAML.
//!
//! A citation key with no colon, or whose prefix has no record source, is not handled by
//! this provider: the provider returns no answer for it, and the next citation provider of
//! the service slot is asked. The library reports a citation key that no citation provider
//! claims. One kind of record source may be registered under several prefixes, for example
//! a second set of bibliography files under the prefix `thesis`.
//!
//! # Fetching and the cache file
//!
//! The crate [`autocitefetch`] fetches the records: it sends the HTTP requests to the arXiv
//! and to doi.org (many citation keys in one request where the server accepts it), waits
//! between two requests as each server asks, tries a failed request again, follows a
//! preprint to its published version, and keeps a *cache file*. The cache file holds the
//! fetched records from one build to the next, each for a lifetime that its record source
//! sets: ten days for the arXiv, 360 days for doi.org. The cache file is
//! `.flm-citations.jsonl` ([`CACHE_BASE`]), in the folder that the program embedding the
//! host gives in [`CitationsHost::cache_folder`]; while the provider works, the crate also
//! writes files whose names begin with `._.flm-citations` in that folder, and removes them.
//! The records of `manual` citation keys and of bibliography files are never written into
//! the cache file: they are made again at every call of the provider, and the bibliography
//! files are read again.
//!
//! # The central cache of the records of all documents
//!
//! Beside the cache file, the provider keeps the records that the arXiv and doi.org answer
//! in one cache shared by every document, [`StandardProvider::CACHE`], which it opens
//! through the handle of the caches ([`crate::cache`]) of the manager that the program
//! embedding the host gives. A record fetched for one document is stored in both; the cache
//! file receives the expiry times of the central copy, so that the two copies of one fetch
//! are equal. When the provider reads a record, it takes, of the two copies, the one that
//! expires later, and writes a central copy that it takes into the cache file. A document
//! with no cache file yet therefore gets its records from the central cache without a
//! request, and a record that another document fetched more recently is used. The cache
//! file is never copied into the central cache, since a document may have been received from
//! someone else. The records of `manual` citation keys and of bibliography files are kept
//! in neither. Over a manager that keeps nothing, the provider uses the cache file alone.
//!
//! A kept record stands in for its record source when the source cannot be reached: after a
//! failed fetch, the crate uses the kept record however old it is. When that record is past
//! its lifetime (its hard expiry), the provider's answer for each citation key that the
//! record serves carries a warning that gives the failure; the library reports it at the
//! citation key's first citation. A kept record that is only due for a refresh gets no
//! warning.
//!
//! # What the provider reads and writes on the machine
//!
//! The provider reads the bibliography files, a citation style file, and the file of DOIs
//! that a record source of the kind `arxiv` may be given to replace the DOIs the arXiv
//! gives (its field `override_dois_file`, [`SourceSettings::Arxiv`]) through
//! [`crate::files`], during the build that needs them, so each one is on that build's list
//! of files read ([`ServiceFiles`](crate::files::ServiceFiles)). The host's rule is that
//! every file is read through [`crate::files`] and written through [`crate::output`]. The
//! provider makes three exceptions to it:
//!
//! - The network: the crate `autocitefetch` opens the connections, through the [`Fetcher`]
//!   that the program embedding the host gives ([`CitationsHost::fetcher`];
//!   [`network_fetcher`] connects to the internet).
//! - The cache file: the crate reads and writes it itself.
//! - The root of the caches, which holds the central cache: the manager that the program
//!   embedding the host gives ([`CacheHost::manager`](crate::cache::CacheHost::manager))
//!   reads and writes it itself.
//!
//! The network fetcher and the folder of the cache file come from
//! [`CitationsHost`], which the program embedding the host fills; no setting of a document
//! can set them.
//!
//! # Several citation keys of one work
//!
//! Two citation keys may refer to one work: `arXiv:1211.1037` and `arxiv:1211.1037`, or a
//! DOI written in two letter cases. The library treats them as one cited work when the
//! provider gives both the same *identity*
//! ([`CitationAnswer::with_identity`](flm_core::stdlib::features::citations::CitationAnswer::with_identity)).
//! The provider's identity of a citation key is the record's `id` as the crate writes it:
//! the prefix as the settings write it, a colon, and the rest of the citation key brought to
//! the spelling its record source uses (a DOI trimmed and lowercased, an arXiv identifier
//! or a bibliography record's name trimmed, a `manual` citation key as written).
//!
//! A preprint and its published version are two works, even when the crate followed the
//! preprint to its published version, because an author may mean to cite both; the
//! provider gives them two identities. When a document cites an arXiv citation key whose
//! record was taken from the published version, and also cites the DOI citation key of
//! that published version, the provider's answer for the arXiv citation key carries a
//! warning that gives both citation keys
//! ([`CitationAnswer::with_warning`](flm_core::stdlib::features::citations::CitationAnswer::with_warning)).
//! The library reports the warning at the first citation of the arXiv citation key.
//!
//! # Progress of a fetch
//!
//! Fetching can take seconds: the arXiv asks for about three seconds between two requests.
//! The provider reports a fetch on the progress sheet of the build ([`flm_core::progress`]),
//! under the scope of the call it is made from
//! ([`ProviderCx::progress`](flm_core::stdlib::features::citations::ProviderCx::progress)):
//! for each pass of the fetch that has citation keys to fetch, one row [`FETCH_KIND`],
//! counted in those keys; and, for each HTTP request and each wait, an `Info` log line that
//! gives its address or its expected duration. A source answers its keys in groups (one
//! request for many arXiv identifiers, one request for each DOI): the row advances by the
//! number of keys of a group when the source has answered that group, whether a record was
//! found for a key or not. The wait for the cache to be written gets a line only when the
//! cache is a file. The program embedding the host decides what to show of them. A fetch
//! under way is not interrupted when the build is cancelled: the provider returns when the
//! crate does, and the build discards what it returns.
//!
//! # How a citation entry is made
//!
//! Some records carry a *ready-made citation entry*, a citation entry written in advance:
//! the record that a `manual` citation key produces always carries one, and a record of a
//! bibliography file carries one when it holds the field `_ready_formatted` or the older
//! field `_formatted_flm_text`. The provider takes, in this order: the text under the name
//! `flm` of `_ready_formatted`, read as FLM; else the text under the first other name of
//! `_ready_formatted` in alphabetical order, shown as literal characters; else the text of
//! `_formatted_flm_text`, read as FLM.
//!
//! Every other record is set in a *citation style*, a file of the Citation Style Language
//! that says in which order and with which formatting (italic, bold) the authors, the
//! title, the journal, the volume and the year are written. The crate [`hayagriva`] (the
//! *citation formatter*) applies it. By default the style is the one the program ships,
//! "American Physical Society - et al." as patched for FLM (the volume in bold, the year
//! last, in parentheses); the setting `style` chooses another ([`ProviderConfig::style`],
//! [`CitationStyle`]). The citation style is a host setting; it is unrelated to the
//! library's *citation counter style*, which writes the numbers of the cited works and the
//! delimiters around them
//! ([`CitationsConfig::style`](flm_core::stdlib::features::citations::CitationsConfig::style)).
//!
//! A citation entry depends on its record and the citation style alone, never on the other
//! works cited. The provider builds it from the citation formatter's structured result,
//! not from a text parsed back
//! ([`EntryPiece`]): an italic run becomes emphasized content, a bold run strong content,
//! and any other formatting (small capitals, superscript) shows its characters plainly.
//! The characters of a record are shown as they are, except a formula written between two
//! dollar signs, `$x^2$` in a title, which becomes an inline formula even when the
//! document's own text does not read dollar signs as formula delimiters (the front-matter
//! setting `flm.parsing.dollar_math`). The number that a numeric citation style writes at
//! the head of a citation entry is left out, since the library's list of cited works shows
//! its own.
//!
//! After the citation entry come the links that the settings choose
//! ([`ProviderConfig::doi_link`], [`ProviderConfig::arxiv_link`],
//! [`ProviderConfig::url_link`]): to the work's DOI, to its arXiv page, and to the record's
//! address. A ready-made citation entry is shown as it is, with no link.
//!
//! # The exports
//!
//! The outputs `bibtex` ([`BIBTEX`]) and `csl-json` ([`CSL_JSON`]), each with a workflow of
//! the same name, write the records of the citation keys that a document cites into one
//! file, in BibTeX or in CSL-JSON: one record per citation key as written, in order of
//! first citation. The `flm` program writes one with `flm render -p bibtex`, where `-p`
//! chooses the profile, and with it the workflow that delivers the output. The stage [`RECORDS_STAGE`] obtains the records through the
//! provider, and so through the same cache file, into the table [`RECORDS`]; the stages
//! [`BIBTEX_STAGE`] and [`CSL_JSON_STAGE`] each write one file into its output's table
//! ([`BIBTEX_DOCUMENT`], [`CSL_JSON_DOCUMENT`]). A citation key with no record is reported
//! as [`NoRecord`] and left out of the file.
//!
//! # Map of the items
//!
//! - The provider and its settings: [`StandardProvider`], [`ProviderConfig`],
//!   [`SourceSettings`], [`CitationStyle`], [`UrlLink`].
//! - What the program embedding the host gives: [`CitationsHost`], [`network_fetcher`],
//!   and the constants [`CACHE_BASE`] and [`DEFAULT_USER_AGENT`].
//! - The central cache: [`StandardProvider::CACHE`].
//! - The row of a fetch on the progress sheet: [`FETCH_KIND`].
//! - The exports: the outputs [`BIBTEX`] and [`CSL_JSON`], their media types
//!   [`BIBTEX_MEDIA_TYPE`] and [`CSL_JSON_MEDIA_TYPE`], the tables [`RECORDS`]
//!   ([`CitedRecords`]), [`BIBTEX_DOCUMENT`] and [`CSL_JSON_DOCUMENT`], the stages
//!   [`RECORDS_STAGE`], [`BIBTEX_STAGE`] and [`CSL_JSON_STAGE`], and the diagnostics
//!   [`NoRecord`] and [`ExportFailed`].
//! - The fetching crate, re-exported: [`autocitefetch`], [`Fetcher`].

mod export;
mod fetch;
mod format;
mod provider;
mod store;

pub use fetch::{FETCH_KIND, network_fetcher};
pub use export::{
    BIBTEX, BIBTEX_DOCUMENT, BIBTEX_MEDIA_TYPE, BIBTEX_STAGE, CSL_JSON, CSL_JSON_DOCUMENT, CSL_JSON_MEDIA_TYPE,
    CSL_JSON_STAGE, CitedRecords, ExportFailed, NoRecord, RECORDS, RECORDS_STAGE,
};
pub(crate) use export::{Export, ExportStage, RecordsStage};
pub use format::{CitationStyle, UrlLink};
pub use provider::{CACHE_BASE, CitationsHost, DEFAULT_USER_AGENT, ProviderConfig, SourceSettings, StandardProvider};

/// The crate that fetches the records, re-exported so that a program embedding the host
/// can use its types, such as a [`Fetcher`] of its own, without depending on the crate
/// itself.
pub use autocitefetch;
pub use autocitefetch::Fetcher;

#[cfg(doc)]
use flm_core::stdlib::features::citations::{CitationProvider, EntryPiece};
