//! The feature `citations-standard` of the package `stdmain` (cargo feature `citations`): it
//! contributes the host's provider of citation entries ([`crate::citations`]) to the
//! library's `citations` feature ([`flm_core::stdcore::features::citations`]), and
//! registers two exports of the records of the cited works.
//!
//! The library's `citations` feature numbers the works that a document cites and asks
//! providers for the citation entry of each work, the formatted text that the list of
//! references shows; the providers are contributed to its service slot
//! `flm.citations.providers`. The feature `citations-standard` contributes
//! [`StandardProvider`] to that service slot. The provider obtains the record of each
//! cited work (its details as data: authors, title, journal, year), fetched from the arXiv
//! or doi.org through the crate `autocitefetch` or read from the author's bibliography
//! files, and makes the work's citation entry from it; a citation key with the prefix
//! `manual:` holds the citation entry itself. The service slot belongs to the
//! `citations` feature, so this is a peer contribution
//! ([`FeatureBuilder::peer_slot`]): where the `citations` feature is switched off, the
//! registry build drops it and records one note
//! ([`UnresolvedPeer`](flm_core::core::registry::UnresolvedPeer)) among its build
//! diagnostics.
//!
//! The feature also registers:
//!
//! - the table [`RECORDS`] (`flm.citations-standard.records`), which holds the record of
//!   every citation key that the document cites, and the stage that fills it by asking the
//!   provider;
//! - two exports, the outputs `bibtex` and `csl-json`. For each one, it registers a table
//!   that holds the output's file, the stage that writes the records into that file, and a
//!   workflow of the output's name, which writes that output and runs every check of the
//!   document.
//!
//! # Settings
//!
//! The feature's configuration is [`ProviderConfig`]. A feature's configuration is the
//! plain Rust value the feature is constructed from; it is fixed when the registry is
//! built and cannot vary per document or per render ([`flm_core::core::settings`]). Its
//! keys:
//!
//! - `sources`: the record sources, by the prefix of the citation keys that each one
//!   serves. The settings key `sources` is combined entry by entry over the settings
//!   sources (the ranked places where the `flm` program reads settings,
//!   [`crate::settings`]), so a package of an extension ([`crate::packages`]) adds a record source,
//!   and `false` written for a record source removes it;
//! - `bibliography`: the author's bibliography files;
//! - `dropped_fields`: the fields left out of a record;
//! - `user_agent`: the name by which the program presents itself to the servers it
//!   fetches records from;
//! - `style`: the citation style the entries are set in: a style file of the Citation
//!   Style Language (`file`) or a style of the formatter's archive by its name (`name`);
//!   without it, the style the program ships
//!   ([`CitationStyle`](crate::citations::CitationStyle));
//! - `doi_link`, `arxiv_link` and `url_link`: the links appended to an entry.
//!
//! A path written in these settings (`bibliography`, a record source's `files` and
//! `override_dois_file`, the `file` of `style`) is relative to the folder of the file in
//! which it is written, or absolute; [`crate::settings`] resolves it. What concerns the
//! machine (the access to the network, the folder of the cache file, the listener told of
//! the progress of a fetch) is not a setting: the program embedding the host gives it
//! ([`CitationsHost`]).

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, OutputDef, WorkflowDef};
use flm_core::core::settings::SettingsDecl;
use flm_core::stdcore::VALIDATED;
use flm_core::stdcore::features::citations::{CitationProvider, PROVIDERS_SLOT};

use crate::citations::{CitationsHost, Export, ExportStage, ProviderConfig, RECORDS, RecordsStage, SourceSettings, StandardProvider};
use crate::files::ServiceFiles;

/// The name under which the feature is registered, `citations-standard`.
pub const FEATURE: &str = "citations-standard";

/// Returns the declaration of the feature's settings. The fields of [`ProviderConfig`] are
/// its keys; `sources` is combined entry by entry, starting from the four default record
/// sources; the keys that hold paths are marked, so that each path is resolved against the
/// folder of the file in which it is written.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<ProviderConfig>(
            "Where the records of cited works come from (`sources`, `bibliography`), the fields \
             left out of a record (`dropped_fields`), the name by which the program presents \
             itself to the servers it fetches records from (`user_agent`), the citation style the \
             entries are set in (`style`), and the links appended to an entry (`doi_link`, \
             `arxiv_link`, `url_link`). Fixed when the registry is built: it cannot vary per \
             document or per render.",
        )
        .merge_entries("sources", || Ok(serde_json::Value::Object(default_sources())))
        .path("bibliography")
        .path("sources.*.files")
        .path("sources.*.override_dois_file")
        .path("style.file")
}

/// Returns the four default record sources, `arxiv`, `bib`, `doi` and `manual`, written as
/// data: each of the kind of its name, with no other setting.
fn default_sources() -> serde_json::Map<String, serde_json::Value> {
    SourceSettings::defaults()
        .into_keys()
        .map(|prefix| {
            let entry = serde_json::json!({ "kind": prefix });
            (prefix, entry)
        })
        .collect()
}

/// The feature `citations-standard`, which contributes the host's provider of citation
/// entries to the library's `citations` feature: see the [module documentation](self).
///
/// [`stdmain_factory`](super::stdmain_factory) makes it from the settings and from the parts of the
/// command ([`StdParts`](super::StdParts)).
#[derive(Debug, Clone)]
pub struct CitationsStandardFeature {
    config: ProviderConfig,
    files: ServiceFiles,
    host: CitationsHost,
}

impl CitationsStandardFeature {
    /// Returns the feature with the configuration `config`. Its provider reads the
    /// bibliography files through `files`, and fetches records and keeps its cache file as
    /// `host` provides.
    pub fn new(config: ProviderConfig, files: ServiceFiles, host: CitationsHost) -> CitationsStandardFeature {
        CitationsStandardFeature { config, files, host }
    }
}

impl FlmFeature for CitationsStandardFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Citation entries").docs(
            "the host's provider of citation entries: it makes a cited work's entry from the work's \
             record, which it fetches from the arXiv or doi.org (keeping it in a cache file) or \
             reads from the author's bibliography files; a citation key with the prefix \
             `manual:` holds the entry itself",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let provider = StandardProvider::new(self.config.clone(), self.files.clone(), self.host.clone())
            .map_err(|detail| BuildError::Feature { feature: FEATURE.into(), detail })?;
        let provider = Arc::new(provider);
        b.peer_slot::<dyn CitationProvider>(PROVIDERS_SLOT, Arc::clone(&provider) as Arc<dyn CitationProvider>)?;
        b.table(
            RECORDS,
            "the record of every citation key the document cites, in the order of first citation",
        )?;
        b.stage(Arc::new(RecordsStage::new(provider)))?;
        for (export, extension, what) in [(Export::Bibtex, "bib", "BibTeX"), (Export::CslJson, "json", "CSL-JSON")] {
            let (name, table) = (export.name(), export.table());
            b.table(
                table,
                format!("the {what} file holding the records of the cited works, as an artifact set of one file"),
            )?;
            b.stage(Arc::new(ExportStage(export)))?;
            b.output(
                OutputDef::new(name, table.name(), table.name(), export.media_type(), extension)
                    .docs(format!(
                        "the records of the cited works as a {what} file, each identified by its \
                         citation key as written"
                    )),
            )?;
            b.workflow(
                WorkflowDef::new(name)
                    .output(name)
                    .demand(VALIDATED)
                    .docs(format!("write the records of the cited works as a {what} file, running every check")),
            )?;
        }
        Ok(())
    }
}
