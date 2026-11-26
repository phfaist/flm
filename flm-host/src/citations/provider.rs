//! The standard provider ([`StandardProvider`]): its settings ([`ProviderConfig`],
//! [`SourceSettings`]), what the program embedding the host gives it ([`CitationsHost`]),
//! and how it answers the library's requests for citation entries. The
//! [module documentation](super) describes the behavior as a whole.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use autocitefetch::csl::cite_id;
use autocitefetch::source::{ArxivSource, BibliographyFileSource, DoiSource, ManualSource, load_override_dois};
use autocitefetch::{
    CacheStore, CitationManager, Clock, CslValue, Fetcher, Interpreted, RetryPolicy, RetryingFetcher, Source,
    StoragePolicy,
};
use autocitefetch_std::{BlockingTimer, SingleFileCacheStore, SystemClock};
use cachette::{Cost, TtlPolicy, ValueSort};
use flm_core::lang::{FlmOrigin, FlmSource};
use flm_core::progress::Scope;
use flm_core::stdlib::features::citations::{
    CitationAnswer, CitationEntry, CitationProvider, CitationProviderError, CitationRequest, CitationResult,
    EntryPiece, ProviderCx,
};
use serde::Deserialize;

use super::fetch::{CallFetcher, FetchProgress, block_on, read_file};
use super::format::{CitationStyle, Formatter, Links, UrlLink};
use super::store::CitationStore;
use crate::cache::{Instance, ServiceCaches, ValuesDeclaration, ValuesEngine};
use crate::roots::Roots;

/// The base name of the cache file, `.flm-citations`. The cache file is
/// `.flm-citations.jsonl`, in the folder given in [`CitationsHost::cache_folder`]; the
/// temporary files that the crate writes in that folder while it works have names
/// beginning with `._.flm-citations`. The pattern `.flm-citations.jsonl` therefore matches
/// the cache file, and the pattern `._.flm-citations*` the temporary files.
pub const CACHE_BASE: &str = ".flm-citations";

/// The default of [`ProviderConfig::user_agent`], the program's name sent to the servers
/// that records are fetched from: `flm/<version> (+<address of the project>)`.
pub const DEFAULT_USER_AGENT: &str = concat!("flm/", env!("CARGO_PKG_VERSION"), " (+https://github.com/phfaist/flm)");

/// The name under which a ready-made citation entry written in FLM is stored in a record's
/// field `_ready_formatted`: the record source `manual` stores its citation entries under
/// it, and a bibliography file's record may hold one under it.
const FLM_MARKUP: &str = "flm";

/// The largest number of links followed from a citation key's record to the record it
/// points to (a preprint to its published version), by the crate and by the provider when
/// it reads a citation key's chain ([`chain`]). It is the crate's own
/// default, set explicitly on the crate's manager so that both use the same number.
const MAX_CHAIN_DEPTH: usize = 16;

/// The expiry policy of the citation records, in the store of a call and in the central
/// cache: cachette's default, with a grace period that never ends. A kept record therefore
/// stands in for a record that cannot be fetched again, however old it is.
const RECORDS_TTL: TtlPolicy = TtlPolicy { grace: Duration::MAX, ..TtlPolicy::DEFAULT };

/// What the program embedding the host gives the standard provider about the machine: the
/// network fetcher and the folder of the cache file. No setting of a document can set them.
///
/// A program gives it in
/// [`session::Request::citations`](crate::session::Request::citations), or in
/// [`StdParts::citations`](crate::stdpkg::StdParts::citations) when it builds the package
/// `std` itself. The `flm` program gives [`network_fetcher`](super::network_fetcher), the
/// folder of the document (the current folder for a document given as text).
///
/// The [`Default`] value has no network fetcher and no cache folder: a test,
/// or a program that fetches nothing, uses it. With it, a citation key whose record must
/// be fetched over the network has no record, and bibliography files and `manual` citation
/// keys still have theirs.
#[derive(Clone, Default)]
pub struct CitationsHost {
    /// The fetcher of internet addresses ([`network_fetcher`](super::network_fetcher)
    /// connects to the internet), or `None` for a program given no network access. With
    /// `None`, a citation key whose record must be fetched over the network has no record,
    /// and the library reports an Error at its first citation saying why.
    ///
    /// The provider runs the fetcher on the thread that calls the provider, with no
    /// asynchronous runtime: it polls the future of each HTTP request again and again, with
    /// a waker that does nothing, until the future is ready. The future must therefore make
    /// progress each time it is polled, as the futures of the fetchers of the crate
    /// `autocitefetch-std` do (they complete at the first poll). A future that waits for a
    /// wake-up from an asynchronous runtime never completes, and the provider's call never
    /// returns.
    pub fetcher: Option<Arc<dyn Fetcher + Send + Sync>>,
    /// The folder that holds the cache file `.flm-citations.jsonl` ([`CACHE_BASE`]), written
    /// as the host writes paths ([`crate::files`]) and converted to the system's form when
    /// the file is opened, or `None` for no cache file: the records fetched from the arXiv
    /// and doi.org are then kept only in the central cache ([`StandardProvider::CACHE`]),
    /// when the manager of the request keeps it, and the other records for one call of the
    /// provider only. The `flm` program gives the folder of the document.
    pub cache_folder: Option<String>,
}

impl fmt::Debug for CitationsHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CitationsHost")
            .field("fetcher", &self.fetcher.as_ref().map(|_| "…"))
            .field("cache_folder", &self.cache_folder)
            .finish()
    }
}

/// The settings of the standard provider: where the records of the cited works come from,
/// what is kept of them, the program's name sent to servers, the citation style, and the
/// links appended to a citation entry.
///
/// They are the settings of the feature `citations-standard`
/// ([`crate::stdpkg::citations_standard`]): a package file writes them under
/// `features: citations-standard:`, a document's front matter under
/// `flm: features: citations-standard:`, and the settings sources are combined as
/// [`crate::settings`] describes. They are fixed when the registry is built.
///
/// The settings that give a file (the bibliography files, the file of DOIs, the style
/// file) are paths. A relative path is relative to the place where it is written, and the
/// provider receives it as a qualified path with that place's root: `doc:refs.bib` from
/// the front matter, `ext+NAME:refs.bib` from the files of the extension `NAME`, and
/// `cwd:refs.bib` from the command line. A one-file extension (`NAME.yaml`) has no files,
/// so a relative path written in it names no file. What the provider needs of the machine
/// is not a setting: see [`CitationsHost`].
///
/// # Examples
///
/// ```
/// use flm_host::citations::ProviderConfig;
///
/// let config = ProviderConfig::default();
/// assert_eq!(config.sources.keys().collect::<Vec<_>>(), ["arxiv", "bib", "doi", "manual"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderConfig {
    /// The record sources, each under its prefix: the record of a citation key
    /// `PREFIX:REST` is obtained from the record source registered under `PREFIX`, the
    /// citation key's prefix being read trimmed and lowercased. A prefix here must therefore
    /// be non-empty, contain no colon, and be written in lowercase with no surrounding
    /// spaces; a prefix of another form makes the registry build fail. The default is four
    /// record sources, `arxiv`, `doi`, `manual` and `bib`, each of the kind of its name. A
    /// package file or a front matter adds a record source under a new prefix, and removes
    /// one by setting its prefix to `false`; the other record sources stay. One kind of
    /// record source may be registered under several prefixes.
    pub sources: BTreeMap<String, SourceSettings>,
    /// The bibliography files that a record source of the kind `bib` reads when its own
    /// `files` is not written: files in CSL-JSON or CSL-YAML, each holding a list of records
    /// (each with its name in its field `id`) or a map from a record's name to the record.
    /// When two files hold a record of the same name, the file listed later wins. Each path
    /// is relative to the folder of the file in which it is written, or to the current
    /// folder for a value given on the command line, or absolute.
    pub bibliography: Vec<String>,
    /// The fields removed from a fetched record: by default `abstract` and `reference`,
    /// which doi.org fills with the work's abstract and the list of every work it cites.
    /// The fields of this list are removed when a record is read, so a document that lists
    /// a field never receives it. The cache file and the central cache
    /// ([`StandardProvider::CACHE`]) store each of the two fields `abstract` and `reference`
    /// only when this list does not contain it; the central cache keeps the records of
    /// documents that store different fields in separate instances.
    pub dropped_fields: Vec<String>,
    /// The program's name, sent with every HTTP request to the servers that records are
    /// fetched from (the header `user-agent`), as the arXiv and doi.org ask callers to do.
    /// By default `flm/<version> (+<address of the project>)`.
    /// The servers welcome a contact address in it.
    pub user_agent: String,
    /// The citation style that citation entries are set in: `{file: my-style.csl}`, a style
    /// file of the Citation Style Language, relative to the folder of the file in which it
    /// is written, or to the current folder for a value given on the command line, or
    /// absolute; or `{name: ieee}`, a style that the citation formatter `hayagriva` ships,
    /// by its name. When not written, the style that the program ships: "American Physical
    /// Society - et al.", with the volume in bold and the year last, in parentheses.
    pub style: Option<CitationStyle>,
    /// Whether a link to the work's DOI, showing `DOI`, is appended to a citation entry whose
    /// record has a DOI; `true` by default.
    pub doi_link: bool,
    /// Whether a link to the work's page on the arXiv, showing its arXiv identifier, is
    /// appended to a citation entry whose record has an arXiv identifier; `true` by
    /// default.
    pub arxiv_link: bool,
    /// When a link to the record's address (its field `URL`), showing `URL`, is appended to a
    /// citation entry whose record has an address: `always`, `never`, or
    /// `only-if-no-other-link` (the default), when the record has neither a DOI nor an arXiv
    /// identifier, whether or not the links to those are appended.
    pub url_link: UrlLink,
}

impl Default for ProviderConfig {
    fn default() -> ProviderConfig {
        ProviderConfig {
            sources: SourceSettings::defaults(),
            bibliography: Vec::new(),
            dropped_fields: vec![String::from("abstract"), String::from("reference")],
            user_agent: String::from(DEFAULT_USER_AGENT),
            style: None,
            doi_link: true,
            arxiv_link: true,
            url_link: UrlLink::default(),
        }
    }
}

/// One record source, as the setting `sources` of [`ProviderConfig`] writes it: its kind,
/// written in the field `kind` (`arxiv`, `doi`, `manual` or `bib`), and that kind's own
/// fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SourceSettings {
    /// The arXiv: the rest of the citation key is an arXiv identifier, `1211.1037`. When the
    /// arXiv gives the DOI of the work's published version, the record is the published
    /// version's, fetched from a record source of the kind `doi` and kept under the arXiv
    /// citation key.
    Arxiv {
        /// The prefix of the record source in which the published version's DOI is looked
        /// up, or `null` to keep the arXiv's record. When it is not written: the record
        /// source of the kind `doi` if there is exactly one, and none if there is none;
        /// with several, it must be written.
        #[serde(default, deserialize_with = "written")]
        chain_dois_to: Option<Option<String>>,
        /// DOIs given by hand, as a map from an arXiv identifier, without its version, to a
        /// DOI: the DOI replaces the one the arXiv gives, and `null` keeps the arXiv's record
        /// of that preprint.
        #[serde(default)]
        override_dois: BTreeMap<String, Option<String>>,
        /// A JSON file of DOIs given by hand: a map from an arXiv identifier to a DOI or
        /// `null`, read as `override_dois` is; `override_dois` wins over it for the same
        /// arXiv identifier. The path is relative to the folder of the file in which it is
        /// written, or to the current folder for a value given on the command line, or
        /// absolute.
        #[serde(default)]
        override_dois_file: Option<String>,
    },
    /// doi.org: the rest of the citation key is a DOI, `10.1103/PhysRevA.86.052329`, in any
    /// letter case.
    Doi {},
    /// Citation entries written by hand: the rest of the citation key is the citation entry
    /// itself, in FLM.
    Manual {},
    /// Bibliography files: the rest of the citation key is the name (the field `id`) of a
    /// record in them.
    Bib {
        /// The bibliography files, in CSL-JSON or CSL-YAML; when two files hold a record of
        /// the same name, the file listed later wins. Each path is relative to the folder of
        /// the file in which it is written, or to the current folder for a value given on
        /// the command line, or absolute. When not written, the files of the setting
        /// `bibliography`.
        #[serde(default)]
        files: Option<Vec<String>>,
    },
}

/// Reads a value that is written, `null` included, as `Some`, so that a field set to `null`
/// differs from a field not written, which stays `None` through `#[serde(default)]`.
fn written<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

impl SourceSettings {
    /// Returns the four default record sources, by prefix: `arxiv`, `doi`, `manual` and
    /// `bib`, each of the kind of its name, with that kind's defaults.
    pub fn defaults() -> BTreeMap<String, SourceSettings> {
        BTreeMap::from([
            (
                String::from("arxiv"),
                SourceSettings::Arxiv {
                    chain_dois_to: None,
                    override_dois: BTreeMap::new(),
                    override_dois_file: None,
                },
            ),
            (String::from("doi"), SourceSettings::Doi {}),
            (String::from("manual"), SourceSettings::Manual {}),
            (String::from("bib"), SourceSettings::Bib { files: None }),
        ])
    }

    /// Returns `key`, the rest of a citation key, in the spelling under which the crate keeps
    /// its record: this record source's spelling rule (a DOI trimmed and lowercased, an
    /// arXiv identifier or a bibliography record's name trimmed, a `manual` citation key as
    /// written), as the crate applies it.
    fn spelling(&self, key: &str) -> String {
        match self {
            SourceSettings::Arxiv { .. } => ArxivSource::new().normalize_key(key),
            SourceSettings::Doi {} => DoiSource::new().normalize_key(key),
            SourceSettings::Manual {} => ManualSource::new(FLM_MARKUP).normalize_key(key),
            SourceSettings::Bib { .. } => BibliographyFileSource::new([]).normalize_key(key),
        }
    }
}

/// The host's citation provider: it obtains the records of the cited works, fetched
/// through the crate `autocitefetch` or read from bibliography files, and makes each
/// work's citation entry ([`CitationProvider`]). The [module documentation](super)
/// describes its behavior: the record sources, the cache file, the identities, the
/// citation style and the links.
///
/// The feature `citations-standard` ([`crate::stdpkg::citations_standard`]) builds it with
/// [`StandardProvider::new`] and contributes it to the library's service slot of citation
/// providers ([`PROVIDERS_SLOT`](flm_core::stdlib::features::citations::PROVIDERS_SLOT)).
/// The library's retrieve stage
/// ([`RETRIEVE`](flm_core::stdlib::features::citations::RETRIEVE)) then calls
/// [`answer_all`](CitationProvider::answer_all)
/// once per document, with the cited citation keys that no earlier citation provider
/// claimed, in order of first citation.
///
/// The provider holds its settings and what the program embedding the host gave it. At
/// each call it opens the cache, sets up the crate, fetches, makes the citation entries
/// and drops everything else, so that nothing of one call is kept for the next except the
/// cache file and the central cache ([`StandardProvider::CACHE`]).
// The crate's manager is built in each call, never kept: it holds its reporter as `Rc` and
// its store and sources without `Send`, while a provider of the library's slot must be
// `Send + Sync`.
#[derive(Debug, Clone)]
pub struct StandardProvider {
    config: ProviderConfig,
    /// For each record source of the kind `arxiv`, by prefix: the prefix of the record
    /// source in which a preprint's DOI is looked up, or `None` to keep the arXiv's record.
    chains: BTreeMap<String, Option<String>>,
    style: StyleSource,
    roots: Roots,
    host: CitationsHost,
    caches: ServiceCaches,
}

/// Where the provider's citation style comes from: prepared when the provider was built, or
/// a style file read at each call.
#[derive(Debug, Clone)]
enum StyleSource {
    /// A style prepared when the provider was built: the shipped style, or a style that the
    /// citation formatter ships, chosen by name.
    Ready(Formatter),
    /// The path of a style file, read at every call, so that a file changed between two
    /// builds changes the citation entries.
    File(String),
}

/// The record of a citation key, or the reason it has none, as text.
pub(super) type RecordResult = Result<CslValue, String>;

/// What one call of the crate gives for a citation key of this provider: its record or the
/// reason it has none, and the citation key's *chain*: the ids of the records that the
/// crate's read step leads to from the citation key's own, in order (for a preprint whose
/// published version the arXiv gives, the id of the published version's DOI). The chain
/// is empty for a citation key whose record is its own, or that has no record.
pub(super) struct Fetched {
    /// The record, or the reason there is none.
    pub(super) record: RecordResult,
    /// The ids that the citation key's record leads through, as the crate writes them
    /// (`doi:10.1/x`).
    pub(super) chain: Vec<String>,
    /// The kept records past their hard expiry that the citation key is given after a failed
    /// fetch of them: its own record, or a record of its chain.
    pub(super) expired: Vec<Expired>,
}

/// A kept record past its hard expiry that the crate used after a failed fetch of it.
pub(super) struct Expired {
    /// The id of the record when it is a record of the citation key's chain; `None` for the
    /// citation key's own record.
    pub(super) served: Option<String>,
    /// The failure of the fetch.
    pub(super) error: String,
}

impl StandardProvider {
    /// The central cache of the citation records of all documents,
    /// `flm.citations-standard.autocitefetch.items`: the items that the servers answered,
    /// kept by the engine `Sqlite`, without blobs. Its values are volatile: each item is
    /// stored with the lifetime that its record source gives (ten days for the arXiv, 360
    /// days for doi.org), and the declared lifetime is 30 days. Its expiry policy is
    /// cachette's default with a grace period that never ends, so that maintenance removes a
    /// record only to keep the root under its size limit. A value's cost is 1000 ms.
    ///
    /// Its instance key is the object of the two flags `keep_abstract` and `keep_reference`
    /// of the storage policy (see [`ProviderConfig::dropped_fields`]): the documents with
    /// the same flags share one instance. The key of an item is `KIND:KEY`, the kind of its
    /// record source (`arxiv` or `doi`; the items of no other kind enter the cache) and the
    /// key in the record source's spelling, whatever prefix a document registers the record
    /// source under.
    pub const CACHE: ValuesDeclaration = ValuesDeclaration {
        name: Cow::Borrowed("flm.citations-standard.autocitefetch.items"),
        version: 1,
        engine: ValuesEngine::Sqlite,
        blobs: false,
        sort: ValueSort::Volatile { lifetime: Duration::from_secs(30 * 24 * 60 * 60), policy: RECORDS_TTL },
        default_cost: Cost::new(1000).unwrap(),
    };

    /// Creates the provider with the settings `config`, reading its files (bibliography
    /// files, a style file, the file of DOIs of a record source of the kind `arxiv`,
    /// `override_dois_file`, each by its qualified path) through `roots`, fetching and
    /// keeping its cache file as `host` gives, and keeping the records of all documents in
    /// the central cache ([`StandardProvider::CACHE`]), which it opens through `caches`. The
    /// shipped citation style, or a style chosen by name, is prepared now; a style file is
    /// read at each call.
    ///
    /// Each path of `config` (`bibliography`, `files` and `override_dois_file` of a record
    /// source, the `file` of `style`) is a qualified path `ROOT:PATH`; the settings give it
    /// in that form, and a program that builds a [`ProviderConfig`] in code writes it so. A
    /// bibliography file and the file of DOIs may also be an address `http://` or
    /// `https://`, fetched as the records are. A citation style file is read from a folder:
    /// an address there, and the root `https`, are refused. A path with no root or with the
    /// root `self` is not read, and is reported as a file that cannot be read is.
    ///
    /// # Errors
    ///
    /// A text stating the problem, which the feature `citations-standard` reports as a
    /// registry build error, when:
    ///
    /// - a prefix of `sources` is empty, contains a colon, or is not written in lowercase
    ///   with no surrounding spaces (a citation key's prefix is read trimmed and
    ///   lowercased, so no citation key could select it);
    /// - a record source of the kind `arxiv` does not set `chain_dois_to` while several
    ///   record sources are of the kind `doi`, or sets it to a prefix whose record source is
    ///   not of the kind `doi`;
    /// - `style` gives the name of a style that the citation formatter does not ship, or the
    ///   shipped style or the named style cannot be used (it takes its rules from a style
    ///   that the citation formatter does not ship, or it has no `<bibliography>` element).
    pub fn new(
        config: ProviderConfig,
        roots: Roots,
        host: CitationsHost,
        caches: ServiceCaches,
    ) -> Result<StandardProvider, String> {
        for prefix in config.sources.keys() {
            if prefix.is_empty() || prefix.contains(':') || *prefix != prefix.trim().to_lowercase() {
                return Err(format!(
                    "the prefix `{prefix}` in the setting `sources` must be non-empty, contain no colon, and be \
                     written in lowercase with no surrounding spaces, since the prefix of a citation key is read \
                     trimmed and lowercased"
                ));
            }
        }
        let dois: Vec<&String> =
            config.sources.iter().filter(|(_, source)| matches!(source, SourceSettings::Doi {})).map(|(p, _)| p).collect();
        let mut chains = BTreeMap::new();
        for (prefix, source) in &config.sources {
            let SourceSettings::Arxiv { chain_dois_to, .. } = source else { continue };
            let chain = match chain_dois_to {
                Some(Some(target)) if !dois.contains(&target) => {
                    return Err(format!(
                        "the setting `sources.{prefix}.chain_dois_to` must be the prefix of an entry of `sources` \
                         whose `kind` is `doi`, and `{target}` is not"
                    ));
                }
                Some(chain) => chain.clone(),
                None => match dois.as_slice() {
                    [] => None,
                    [one] => Some((*one).clone()),
                    several => {
                        let names: Vec<String> = several.iter().map(|name| format!("`{name}`")).collect();
                        return Err(format!(
                            "the setting `sources.{prefix}.chain_dois_to` must give the prefix of the entry of \
                             `sources` in which the DOI of a preprint's published version is looked up, since \
                             several entries of `sources` have the `kind` `doi`: {}",
                            names.join(", ")
                        ));
                    }
                },
            };
            chains.insert(prefix.clone(), chain);
        }
        let style = match &config.style {
            None => StyleSource::Ready(Formatter::shipped()?),
            Some(CitationStyle::Name(name)) => StyleSource::Ready(Formatter::archived(name)?),
            Some(CitationStyle::File(file)) => StyleSource::File(file.clone()),
        };
        Ok(StandardProvider { config, chains, style, roots, host, caches })
    }

    /// Returns the record source that the prefix of the citation key `key` selects, with the
    /// prefix as the settings write it and the rest of the citation key as written; `None`
    /// for a citation key with no colon, or whose prefix has no record source.
    fn source_of<'k>(&self, key: &'k str) -> Option<(&String, &SourceSettings, &'k str)> {
        let (prefix, rest) = key.split_once(':')?;
        let (prefix, source) = self.config.sources.get_key_value(&prefix.trim().to_lowercase())?;
        Some((prefix, source, rest))
    }

    /// Returns the crate's implementation of the record source `source`, whose prefix in the
    /// settings is `prefix` (for a record source of the kind `arxiv`, `prefix` finds, among
    /// the provider's chains, the record source in which a preprint's DOI is looked up);
    /// `file_dois` are the DOIs that its file `override_dois_file` gives, under those given
    /// in the settings. A record source of the kind `bib` gets a lifetime of zero, so that
    /// its records are never written into the cache file and the bibliography files are
    /// read at every call.
    fn source(&self, prefix: &str, source: &SourceSettings, file_dois: &[(String, Option<String>)]) -> CrateSource {
        match source {
            SourceSettings::Arxiv { override_dois, .. } => {
                let chain = self.chains.get(prefix).cloned().flatten();
                CrateSource::Arxiv(
                    ArxivSource::new()
                        .chain_dois_to(chain.as_deref())
                        .with_override_dois(file_dois.iter().cloned())
                        .with_override_dois(override_dois.iter().map(|(id, doi)| (id.clone(), doi.clone()))),
                )
            }
            SourceSettings::Doi {} => CrateSource::Doi(DoiSource::new()),
            SourceSettings::Manual {} => CrateSource::Manual(ManualSource::new(FLM_MARKUP)),
            SourceSettings::Bib { files } => {
                let files = files.clone().unwrap_or_else(|| self.config.bibliography.clone());
                // The lifetime zero keeps what the files hold out of the cache file: the files
                // are read at every call, so they are among the files the build read, and a
                // file changed since the last build changes the entries.
                CrateSource::Bib(BibliographyFileSource::new(files).with_parser(parse_records).with_ttl(Duration::ZERO))
            }
        }
    }

    /// Returns the storage policy of the store of a call: the expiry policy
    /// [`RECORDS_TTL`], and what is kept of the fields `abstract` and `reference`: each is
    /// kept when the setting [`ProviderConfig::dropped_fields`] does not list it, so that the
    /// setting alone decides whether a record has the field.
    fn storage_policy(&self) -> StoragePolicy {
        let kept = |field: &str| !self.config.dropped_fields.iter().any(|dropped| dropped == field);
        StoragePolicy { ttl: RECORDS_TTL, keep_abstract: kept("abstract"), keep_reference: kept("reference") }
    }

    /// Returns the store of a call over the cache file `file` and the central cache, opened
    /// through the handle of the caches. Without a central cache (a manager that keeps
    /// nothing), the store is the file and memory.
    fn store(&self, file: Option<SingleFileCacheStore>) -> CitationStore {
        let policy = self.storage_policy();
        let instance = Instance::Shared(serde_json::json!({
            "keep_abstract": policy.keep_abstract,
            "keep_reference": policy.keep_reference,
        }));
        let central = self.caches.open_values(&StandardProvider::CACHE, &instance).ok();
        let kinds = self
            .config
            .sources
            .iter()
            .filter_map(|(prefix, source)| match source {
                SourceSettings::Arxiv { .. } => Some((prefix.clone(), "arxiv")),
                SourceSettings::Doi {} => Some((prefix.clone(), "doi")),
                SourceSettings::Manual {} | SourceSettings::Bib { .. } => None,
            })
            .collect();
        CitationStore::new(policy, kinds, file.map(SingleFileCacheStore::into_inner), central)
    }

    /// Returns the identity of the citation key `key` if it is this provider's (see
    /// [`identity`]), such as `doi:10.1/x`, the id under which the crate keeps its record;
    /// `None` otherwise.
    fn identity_of(&self, key: &str) -> Option<String> {
        self.source_of(key).map(|(prefix, source, rest)| identity(prefix, source, rest))
    }

    /// Returns what the crate gives for each citation key of `keys`, in order: `None` for a
    /// citation key that is not this provider's (no colon, or a prefix with no record
    /// source), and for each other citation key its record or the reason it has none, with
    /// its chain and its expired records ([`Fetched`]). All the citation keys are fetched in
    /// one call of the crate, with the cache file in the folder that
    /// [`CitationsHost::cache_folder`] gives, when it gives one, and the central cache
    /// ([`StandardProvider::CACHE`]). When the cache file cannot be opened, every citation
    /// key of this provider gets that error as its reason.
    ///
    /// The fetch reports under `progress`, the scope of the call it is made from: a row
    /// [`FETCH_KIND`](super::FETCH_KIND) for each pass that has citation keys to fetch, and
    /// an `Info` log line for each HTTP request and each wait. A fetch under way is not
    /// interrupted when the build is cancelled.
    ///
    /// The provider's [`answer_all`](CitationProvider::answer_all) and the stage
    /// [`RECORDS_STAGE`](super::RECORDS_STAGE) both call it.
    pub(super) fn records(&self, keys: &[&str], progress: &Scope) -> Vec<Option<Fetched>> {
        let sources: Vec<Option<(&String, &SourceSettings, &str)>> = keys.iter().map(|key| self.source_of(key)).collect();
        let mine: Vec<(&String, &SourceSettings, &str)> = sources.iter().flatten().copied().collect();
        let mut records = if mine.is_empty() {
            Vec::new()
        } else {
            match &self.host.cache_folder {
                Some(folder) => {
                    let opened = match crate::files::system::to_system(folder) {
                        Err(reason) => Err(format!(
                            "the path ‘{folder}’ given for the citation cache file names no folder on this system: {reason}"
                        )),
                        Ok(system) => SingleFileCacheStore::with_base(system, CACHE_BASE).map_err(|error| {
                            format!("the citation cache file in the folder `{folder}` cannot be opened: {error}")
                        }),
                    };
                    match opened {
                        Ok(file) => self.fetch(self.store(Some(file)), true, &mine, progress),
                        Err(detail) => failed(mine.len(), &detail),
                    }
                }
                None => self.fetch(self.store(None), false, &mine, progress),
            }
        }
        .into_iter();
        sources.iter().map(|source| source.and_then(|_| records.next())).collect()
    }

    /// Returns the records of the citation keys `mine` (each a citation key of this
    /// provider, split into its prefix and the rest), in order, with their chains and their
    /// expired records ([`Fetched::expired`]), using the store `store` (with a cache file
    /// when `cache_file` is true), reporting under `progress`. When the crate cannot be set
    /// up or its retrieval fails as a whole, every citation key gets that error as its
    /// reason. A citation key whose retrieval failed gets the crate's failure messages for
    /// it, those of the published version of a preprint included, joined by `; `.
    ///
    /// The file `override_dois_file` of a record source of the kind `arxiv` is read when a
    /// citation key of `mine` has that record source's prefix. When it cannot be read or
    /// used, every such citation key gets the crate's error as its reason and is not
    /// fetched.
    ///
    /// When the crate uses a kept record past its hard expiry after a failed fetch of it,
    /// each citation key that the record serves has it among its expired records, with the
    /// failure; [`answer_all`](CitationProvider::answer_all) gives the citation key a warning
    /// for each. A kept record that is only due for a refresh is not among them.
    fn fetch(
        &self,
        store: CitationStore,
        cache_file: bool,
        mine: &[(&String, &SourceSettings, &str)],
        progress: &Scope,
    ) -> Vec<Fetched> {
        let network = self.host.fetcher.as_deref();
        let fetcher = CallFetcher { roots: &self.roots, network, user_agent: &self.config.user_agent };
        let reporter = Rc::new(FetchProgress::new(progress, cache_file));
        // The DOIs of the override files, read as the crate reads a source's requests, and
        // the reason a file cannot be read or used, by prefix.
        let mut file_dois: BTreeMap<&str, Vec<(String, Option<String>)>> = BTreeMap::new();
        let mut unreadable: BTreeMap<&str, String> = BTreeMap::new();
        {
            let retrying =
                RetryingFetcher::new(&fetcher, &BlockingTimer, RetryPolicy::default()).with_reporter(&*reporter);
            for (prefix, source) in &self.config.sources {
                let SourceSettings::Arxiv { override_dois_file: Some(file), .. } = source else { continue };
                if !mine.iter().any(|(cited, _, _)| *cited == prefix) {
                    continue;
                }
                match block_on(load_override_dois(&retrying, file)) {
                    Ok(dois) => {
                        file_dois.insert(prefix, dois);
                    }
                    Err(error) => {
                        unreadable.insert(prefix, error.to_string());
                    }
                }
            }
        }
        let dois_of = |prefix: &str| file_dois.get(prefix).map_or(&[][..], Vec::as_slice);
        let mut manager = CitationManager::new(fetcher, store, SystemClock, BlockingTimer)
            .with_max_chain_depth(MAX_CHAIN_DEPTH)
            .with_dropped_csl_fields(self.config.dropped_fields.iter().cloned())
            .with_reporter(Rc::clone(&reporter) as Rc<dyn autocitefetch::Reporter>);
        // The record sources as the crate's read step interprets the kept records, for the
        // chains.
        let mut readers: BTreeMap<&str, Box<dyn Source>> = BTreeMap::new();
        for (prefix, source) in &self.config.sources {
            manager = match self.source(prefix, source, dois_of(prefix)).register(manager, prefix) {
                Ok(manager) => manager,
                Err(error) => return failed(mine.len(), &error.to_string()),
            };
            readers.insert(prefix, self.source(prefix, source, dois_of(prefix)).boxed());
        }
        let mut cites: Vec<(String, String)> = Vec::new();
        for (prefix, _, rest) in mine {
            let cite = ((*prefix).clone(), rest.to_string());
            if !unreadable.contains_key(prefix.as_str()) && !cites.contains(&cite) {
                cites.push(cite);
            }
        }
        let report = match block_on(manager.retrieve(&cites)) {
            Ok(report) => report,
            Err(error) => return failed(mine.len(), &error.to_string()),
        };
        // The kept records used after a failed fetch, with the failure, that are past their
        // hard expiry.
        let now = SystemClock.now();
        let expired: Vec<(String, String)> = reporter
            .take_served()
            .into_iter()
            .filter(|(id, _)| {
                matches!(manager.store().get(id), Ok(Some(kept)) if kept.expiry.expires <= now)
            })
            .collect();
        // Each failure under the identity of the key the author wrote, which a failure of
        // the published version of a preprint names as its origin.
        let mut failures: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for failure in &report.failures {
            let (root, message) = match &failure.origin {
                Some((prefix, key)) => {
                    (format!("{prefix}:{key}"), format!("`{}:{}`: {}", failure.prefix, failure.key, failure.message))
                }
                None => (format!("{}:{}", failure.prefix, failure.key), failure.message.clone()),
            };
            failures.entry(root).or_default().push(message);
        }
        mine.iter()
            .map(|(prefix, source, rest)| {
                if let Some(reason) = unreadable.get(prefix.as_str()) {
                    return Fetched { record: Err(reason.clone()), chain: Vec::new(), expired: Vec::new() };
                }
                let id = identity(prefix, source, rest);
                let record = manager.get(prefix, rest).map_err(|error| match failures.get(&id) {
                    Some(messages) => messages.join("; "),
                    None => error.to_string(),
                });
                let chained = |record| Ok((record, chain(manager.store(), &readers, prefix, &source.spelling(rest))?));
                match record.and_then(chained) {
                    Ok((record, chain)) => {
                        let expired = expired
                            .iter()
                            .filter(|(served, _)| *served == id || chain.contains(served))
                            .map(|(served, error)| Expired {
                                served: (*served != id).then(|| served.clone()),
                                error: error.clone(),
                            })
                            .collect();
                        Fetched { record: Ok(record), chain, expired }
                    }
                    Err(error) => Fetched { record: Err(error), chain: Vec::new(), expired: Vec::new() },
                }
            })
            .collect()
    }

    /// Returns the formatter of this call: the style prepared when the provider was built, or
    /// the style file read now; or the reason the style file cannot be read or used.
    fn formatter(&self) -> Result<Formatter, String> {
        match &self.style {
            StyleSource::Ready(formatter) => Ok(formatter.clone()),
            StyleSource::File(file) => {
                let bytes = read_file(&self.roots, file)?;
                let xml = String::from_utf8(bytes)
                    .map_err(|_| format!("Invalid citation style file ‘{file}’: it must be UTF-8 text"))?;
                Formatter::from_xml(&xml).map_err(|detail| format!("the citation style file `{file}` {detail}"))
            }
        }
    }

    /// Returns the links that the settings choose to append after a citation entry.
    fn links(&self) -> Links {
        Links { doi: self.config.doi_link, arxiv: self.config.arxiv_link, url: self.config.url_link }
    }
}

/// Returns the identity of the citation key made of `prefix` and `rest`, for its record
/// source `source`: the prefix as the settings write it, a colon, and `rest` in the record
/// source's spelling, `doi:10.1/x`. It is the id under which the crate keeps the citation
/// key's record.
fn identity(prefix: &str, source: &SourceSettings, rest: &str) -> String {
    cite_id(prefix, &source.spelling(rest))
}

/// Returns the chain of the record kept under the id of `prefix` and `key` (the key in its
/// record source's spelling) in `store`: the ids it leads to, in order, read as the crate's
/// read step reads them, each kept record interpreted by the record source of its prefix in
/// `readers` and each next key brought to its record source's spelling, at most
/// [`MAX_CHAIN_DEPTH`] of them; or the reason the cache cannot be read. The chain ends
/// without an error where no record is kept under the next id, where the record there is the
/// record itself, or where an id comes back.
fn chain<S: CacheStore>(
    store: &S,
    readers: &BTreeMap<&str, Box<dyn Source>>,
    prefix: &str,
    key: &str,
) -> Result<Vec<String>, String> {
    let mut chain: Vec<String> = Vec::new();
    let (mut prefix, mut key) = (prefix.to_string(), key.to_string());
    let mut current = cite_id(&prefix, &key);
    while chain.len() < MAX_CHAIN_DEPTH {
        let kept = store
            .get(&current)
            .map_err(|error| format!("the record of `{current}` in the citation cache file cannot be read: {error}"))?;
        let Some(kept) = kept else { break };
        let Some(reader) = readers.get(prefix.as_str()) else { break };
        let Interpreted::Chained { prefix: next_prefix, key: next_key, .. } = reader.interpret(&key, kept.item) else {
            break;
        };
        let next_key = match readers.get(next_prefix.as_str()) {
            Some(target) => target.normalize_key(&next_key),
            None => next_key,
        };
        let next = cite_id(&next_prefix, &next_key);
        if next == current || chain.contains(&next) {
            break;
        }
        chain.push(next.clone());
        (prefix, key, current) = (next_prefix, next_key, next);
    }
    Ok(chain)
}

/// A record source of the crate, made from its settings ([`StandardProvider::source`]).
enum CrateSource {
    Arxiv(ArxivSource),
    Doi(DoiSource),
    Manual(ManualSource),
    Bib(BibliographyFileSource),
}

impl CrateSource {
    /// Returns `manager` with this record source registered under the prefix `prefix`.
    fn register<F, S, C, T>(
        self,
        manager: CitationManager<F, S, C, T>,
        prefix: &str,
    ) -> autocitefetch::Result<CitationManager<F, S, C, T>>
    where
        F: Fetcher,
        S: CacheStore,
        C: autocitefetch::Clock,
        T: autocitefetch::Timer,
    {
        match self {
            CrateSource::Arxiv(source) => manager.register(prefix, source),
            CrateSource::Doi(source) => manager.register(prefix, source),
            CrateSource::Manual(source) => manager.register(prefix, source),
            CrateSource::Bib(source) => manager.register(prefix, source),
        }
    }

    /// Returns this record source as a trait object.
    fn boxed(self) -> Box<dyn Source> {
        match self {
            CrateSource::Arxiv(source) => Box::new(source),
            CrateSource::Doi(source) => Box::new(source),
            CrateSource::Manual(source) => Box::new(source),
            CrateSource::Bib(source) => Box::new(source),
        }
    }
}

/// Returns the error `detail` once for each of `count` citation keys.
fn failed(count: usize, detail: &str) -> Vec<Fetched> {
    (0..count).map(|_| Fetched { record: Err(detail.to_string()), chain: Vec::new(), expired: Vec::new() }).collect()
}

/// Returns the text of the warning for the citation key `key`, given as the author wrote it,
/// which is given the kept record `expired` past its hard expiry.
fn expired_warning(key: &str, expired: &Expired) -> String {
    let error = &expired.error;
    match &expired.served {
        None => format!("Failed to refresh the expired record of ‘{key}’: {error}"),
        Some(served) => format!("Failed to refresh the expired record ‘{served}’ of ‘{key}’: {error}"),
    }
}

/// Returns the text of the warning for the citation key `key`, whose chain leads to the
/// identity of the citation key `other`; both are cited in the document and given as the
/// author wrote them.
fn chained_warning(key: &str, other: &str) -> String {
    format!("`{key}` and `{other}` are cited as two works, while the record of `{key}` is taken from `{other}`, its published version")
}

impl CitationProvider for StandardProvider {
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
        let mut results = self.answer_all(std::slice::from_ref(request), cx);
        results.pop().unwrap_or(Ok(None))
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &ProviderCx<'_>) -> Vec<CitationResult> {
        let keys: Vec<&str> = requests.iter().map(|request| request.key.as_str()).collect();
        let identities: Vec<Option<String>> = keys.iter().map(|key| self.identity_of(key)).collect();
        let records = self.records(&keys, cx.progress());
        // The style is read once per call, and only where a record needs it.
        let mut formatter: Option<Result<Formatter, String>> = None;
        records
            .into_iter()
            .zip(&keys)
            .map(|(fetched, key)| {
                let Some(Fetched { record, chain, expired }) = fetched else { return Ok(None) };
                let record = record.map_err(CitationProviderError::new)?;
                let entry = match ready_entry(&record, cx) {
                    Some(entry) => entry?,
                    None => {
                        let formatter = formatter.get_or_insert_with(|| self.formatter());
                        let formatter = formatter.as_ref().map_err(|detail| CitationProviderError::new(detail.as_str()))?;
                        let pieces = formatter.entry(&record, self.links()).map_err(CitationProviderError::new)?;
                        cx.build_entry(&pieces)?
                    }
                };
                let mut answer = CitationAnswer::new(entry);
                for expired in &expired {
                    answer = answer.with_warning(expired_warning(key, expired));
                }
                if let Some(id) = record.get("id").and_then(CslValue::as_str) {
                    answer = answer.with_identity(id);
                }
                // Two keys cited as two works where this key's chain leads to the other: they
                // stay two works, since the author may mean the distinction, and the warning
                // tells the author that the two keys lead to the same work.
                for (other, identity) in keys.iter().zip(&identities) {
                    if identity.as_ref().is_some_and(|identity| chain.contains(identity)) {
                        answer = answer.with_warning(chained_warning(key, other));
                    }
                }
                Ok(Some(answer))
            })
            .collect()
    }
}

/// Returns the ready-made citation entry that `record` carries, or `None` when it carries
/// none: the text under the name `flm` of the field `_ready_formatted`, parsed as FLM;
/// else the text under the first other name of `_ready_formatted`, in alphabetical order,
/// shown as literal characters; else the text of the older field `_formatted_flm_text`,
/// parsed as FLM. A ready-made citation entry is shown as it is, with no link appended.
fn ready_entry(record: &CslValue, cx: &ProviderCx<'_>) -> Option<Result<CitationEntry, CitationProviderError>> {
    let id = record.get("id").and_then(CslValue::as_str).unwrap_or_default();
    let flm = |text: &str| {
        let source = FlmSource::new(text).with_origin(FlmOrigin::synthetic(format!("the citation entry of `{id}`")));
        cx.parse_entry(Arc::new(source))
    };
    if let Some(ready) = record.get("_ready_formatted").and_then(CslValue::as_object) {
        if let Some(text) = ready.get(FLM_MARKUP).and_then(CslValue::as_str) {
            return Some(flm(text));
        }
        let mut others: Vec<(&String, &CslValue)> = ready.iter().collect();
        others.sort_by_key(|(name, _)| *name);
        if let Some(text) = others.first().and_then(|(_, text)| text.as_str()) {
            return Some(cx.build_entry(&[EntryPiece::text(text)]));
        }
    }
    record.get("_formatted_flm_text").and_then(CslValue::as_str).map(flm)
}

/// Returns the records of a bibliography file, its content `bytes` read as JSON where it is
/// valid JSON and as YAML otherwise: a list of records, each with its name in its field
/// `id`, or a map from a record's name to the record.
///
/// # Errors
///
/// A text that the crate writes after the file's name: the content is neither JSON nor
/// YAML, or it is not encoded as UTF-8.
fn parse_records(bytes: &[u8]) -> Result<CslValue, String> {
    if let Ok(value) = serde_json::from_slice(bytes) {
        return Ok(value);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| String::from("must be CSL-JSON or CSL-YAML, encoded as UTF-8"))?;
    crate::seed::read_front_matter(text, 1)
        .map(|node| node.to_json())
        .map_err(|error| format!("must be CSL-JSON or CSL-YAML: {}", error.detail))
}
