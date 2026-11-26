//! The caches: what the program gives of them ([`CacheHost`]), and the handle through which
//! the host's features and host services open their sessions ([`ServiceCaches`]).
//!
//! A *cache* keeps the result of costly work that FLM does outside its own processing (a
//! program it runs, a record it fetches from a server, a value an extension computes), so
//! that the work need not be done again. The crate [`cachette`], re-exported here, keeps
//! the caches: a [`Manager`] keeps them under one *root*, bounds their total size and
//! removes old data. The program that embeds the host makes the manager, in the form it
//! chooses (a folder on disk, a file system of its own, memory, or a manager that keeps
//! nothing), and gives it in the field [`cache`](crate::session::Request::cache) of its
//! request. The host defines no trait of its own for a cache.
//!
//! The code that fills a cache, its *owner*, declares it once: a values cache with a
//! [`ValuesDeclaration`], a folder cache with a [`FolderCacheDeclaration`]. It opens a
//! session of the cache through the [`ServiceCaches`] it was given, never through the
//! manager directly: the handle builds the instance key ([`Instance`]), chooses the engine
//! that the manager offers, and keeps the sessions of the build under way. A cache that an
//! extension declares ([`CacheDeclaration`]) is opened as an [`ExtensionCache`].
//!
//! A failure of a cache is never a failure of a build: an operation of a session never
//! fails, and the events go to the manager's reporter. An owner that finds no value does
//! its work as on a first run.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// The crate that keeps the caches, re-exported so that a program embedding the host can
/// make its manager, its clock and its reporter without depending on the crate itself.
pub use cachette;

use cachette::{
    Cost, Engine, FolderCacheDeclaration, FolderSession, FolderUse, InstanceKey, Manager, ManagerError, TtlPolicy,
    ValueSort, ValuesCacheDeclaration, ValuesSession,
};
use flm_ext::host::{
    CacheDeclaration, CacheEngine, CacheKind, CacheScope, CacheSort, CacheTime, HostCaller, HostServiceError,
};

/// What the program gives the host of its caches: the manager, and the text that
/// identifies the document.
///
/// The default is a manager that keeps nothing ([`Manager::none`]) and no document: every
/// owner then does its work at each build. A program that wants results kept for the life
/// of its process only gives [`Manager::in_memory`]; the `flm` program gives a manager over
/// a folder of the user's caches ([`Manager::open_path`]).
///
/// **The manager's clock is the system's clock.** A manager computes the expiry times of
/// the values it stores with the clock it was given, and some owners compare these times
/// with the system's clock (the crate that fetches citation records does). A program that
/// gives a manager with a clock of its own, other than [`SystemClock`](cachette::SystemClock),
/// gives one that reads the same time as the system's clock.
#[derive(Clone, Debug)]
pub struct CacheHost {
    /// The manager that keeps the caches.
    pub manager: Manager,
    /// The text that identifies the document among all the documents that use the
    /// manager's root, such as the canonical absolute path of its file; `None` when the
    /// program gives none. A cache kept per document ([`Instance::Document`]) has an
    /// instance for each such text, and, without one, keeps nothing.
    pub document: Option<String>,
}

impl Default for CacheHost {
    fn default() -> CacheHost {
        CacheHost { manager: Manager::none(), document: None }
    }
}

/// The engine that an owner states for the values of a values cache, in a form that does
/// not depend on the cargo feature `sqlite`.
///
/// The handle opens the cache with the engine stated when the manager offers it. The
/// SQLite engine needs a manager made by [`Manager::open_path`] and a program built with
/// the cargo feature `sqlite`; on any other manager, the handle opens the cache with the
/// engine [`Jsonl`](ValuesEngine::Jsonl) when its values have no blobs, and
/// [`Files`](ValuesEngine::Files) when they have. A values cache therefore works on every
/// form of the manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValuesEngine {
    /// One JSONL file, every value in memory; values without blobs only.
    Jsonl,
    /// Files per value.
    Files,
    /// One SQLite database.
    Sqlite,
}

/// The declaration of a values cache, as its owner states it to the handle: the fields of a
/// [`ValuesCacheDeclaration`], with the engine as a [`ValuesEngine`].
///
/// A declaration of the host's own code is a `const` item, whose name is written
/// `Cow::Borrowed("flm.…")`.
#[derive(Clone, Debug)]
pub struct ValuesDeclaration {
    /// The name of the cache, by the rules of [`ValuesCacheDeclaration::name`]. Every name
    /// begins with `flm.`.
    pub name: Cow<'static, str>,
    /// The version of the cache: each version has data of its own.
    pub version: u32,
    /// The engine that keeps the values on disk.
    pub engine: ValuesEngine,
    /// Whether the values have blobs.
    pub blobs: bool,
    /// Whether the values are stable or volatile, with their lifetime.
    pub sort: ValueSort,
    /// The cost of a value for which the owner states none: an estimate of the time needed
    /// to produce it again.
    pub default_cost: Cost,
}

impl ValuesDeclaration {
    /// Checks the declaration alone, as [`Manager::none`] checks it: its name, and its
    /// engine with its blobs.
    ///
    /// # Errors
    ///
    /// The error of [`Manager::check`] for an incorrect declaration.
    pub fn check(&self) -> Result<(), ManagerError> {
        Manager::none().check(&self.for_manager(&Manager::none()))
    }

    /// Returns the declaration of cachette with which `manager` opens the cache: the engine
    /// stated when `manager` offers it. For [`ValuesEngine::Sqlite`] on a manager that does
    /// not offer SQLite, the engine is `Jsonl` when the values have no blobs, and `Files`
    /// when they have.
    fn for_manager(&self, manager: &Manager) -> ValuesCacheDeclaration {
        let fallback = if self.blobs { Engine::Files } else { Engine::Jsonl };
        let declaration = |engine| ValuesCacheDeclaration {
            name: self.name.clone(),
            version: self.version,
            engine,
            blobs: self.blobs,
            sort: self.sort,
            default_cost: self.default_cost,
            migration: None,
        };
        match self.engine {
            ValuesEngine::Jsonl => declaration(Engine::Jsonl),
            ValuesEngine::Files => declaration(Engine::Files),
            ValuesEngine::Sqlite => sqlite(manager, declaration(fallback)),
        }
    }
}

/// Returns `fallback` with the SQLite engine when `manager` offers it, else `fallback`.
#[cfg(feature = "sqlite")]
fn sqlite(manager: &Manager, fallback: ValuesCacheDeclaration) -> ValuesCacheDeclaration {
    let declaration = ValuesCacheDeclaration { engine: Engine::Sqlite, ..fallback.clone() };
    if manager.check(&declaration).is_ok() { declaration } else { fallback }
}

/// Returns `fallback`: a program built without the cargo feature `sqlite` offers no SQLite
/// engine.
#[cfg(not(feature = "sqlite"))]
fn sqlite(_manager: &Manager, fallback: ValuesCacheDeclaration) -> ValuesCacheDeclaration {
    fallback
}

/// Which instance of a cache a session uses, from which the handle builds its instance key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instance {
    /// A cache shared by every document, whose instance key is the value that its owner
    /// states (`null` when it states none).
    Shared(serde_json::Value),
    /// A cache per document: the instance key is the object `{"document": TEXT}`, with the
    /// [`document`](CacheHost::document) that the program gave. Without one, a session of
    /// the cache is without a cache.
    Document,
    /// A cache of an extension: the instance key is the object `{"extension": IDENTITY}`,
    /// with the member `document` as well for a cache per document (`per_document`), which
    /// without a document's text is without a cache.
    Extension {
        /// The extension's identity.
        identity: String,
        /// Whether the cache has an instance per document.
        per_document: bool,
    },
}

/// A cache that an extension declares ([`CacheDeclaration`]), as the handle opens it: the
/// declaration of cachette and the instance.
///
/// The cache's name is `flm.ext.STEM.NAME`, where `STEM` is the extension's stem in
/// lowercase with `@` left out and `/` written `.`, and `NAME` the name the package
/// declares. The instance key is the object `{"extension": IDENTITY}`, with the extension's
/// identity, and, for the scope `document`, the member `document` as well
/// ([`Instance::Extension`]): two extensions of one name in two places never share data,
/// and an extension updated in place keeps its caches.
#[derive(Clone, Debug)]
pub struct ExtensionCache {
    /// The declaration of a values cache or of a folder cache.
    pub declaration: ExtensionCacheDeclaration,
    /// The instance of the cache.
    pub instance: Instance,
}

/// The declaration of an extension's cache: a values cache or a folder cache.
#[derive(Clone, Debug)]
pub enum ExtensionCacheDeclaration {
    /// A values cache.
    Values(ValuesDeclaration),
    /// A folder cache, which one session at a time uses ([`FolderUse::Exclusive`]).
    Folder(FolderCacheDeclaration),
}

impl ExtensionCache {
    /// Returns the cache `name` that the package of `caller` declares as `declared`.
    ///
    /// A time `unlimited` is [`Duration::MAX`] for a value and for the grace period of
    /// volatile values, and no idle lifetime for a folder. A volatile cache's policy
    /// ([`TtlPolicy`]) has the three settings of the declaration's `expiry`. The cost is at
    /// most [`i64::MAX`] ms, the largest that cachette keeps.
    pub fn new(caller: &HostCaller, name: &str, declared: &CacheDeclaration) -> ExtensionCache {
        let name = Cow::Owned(extension_cache_name(&caller.extension.stem, name));
        let default_cost = Cost::new(declared.cost_ms.get().min(i64::MAX as u64)).expect("a cost from 1 ms to i64::MAX ms");
        let declaration = match &declared.kind {
            CacheKind::Values { engine, blobs, sort } => ExtensionCacheDeclaration::Values(ValuesDeclaration {
                name,
                version: declared.version.get(),
                engine: match engine {
                    CacheEngine::Jsonl => ValuesEngine::Jsonl,
                    CacheEngine::Files => ValuesEngine::Files,
                    CacheEngine::Sqlite => ValuesEngine::Sqlite,
                },
                blobs: *blobs,
                sort: match sort {
                    CacheSort::Stable { unused } => ValueSort::Stable { idle_lifetime: duration(*unused) },
                    CacheSort::Volatile { lifetime_s, expiry } => ValueSort::Volatile {
                        lifetime: Duration::from_secs(*lifetime_s),
                        policy: TtlPolicy {
                            stale_percent: u32::from(expiry.stale_percent.get()),
                            jitter_percent: u32::from(expiry.jitter_percent.get()),
                            grace: duration(expiry.grace),
                        },
                    },
                },
                default_cost,
            }),
            CacheKind::Folder { unused } => ExtensionCacheDeclaration::Folder(FolderCacheDeclaration {
                name,
                version: declared.version.get(),
                usage: FolderUse::Exclusive,
                idle_lifetime: match unused {
                    CacheTime::Seconds(seconds) => Some(Duration::from_secs(*seconds)),
                    CacheTime::Unlimited => None,
                },
                default_cost,
            }),
        };
        let instance = Instance::Extension {
            identity: caller.extension.identity.clone(),
            per_document: declared.scope == CacheScope::Document,
        };
        ExtensionCache { declaration, instance }
    }

    /// Returns the values cache `name` that the package of `caller` declares: its
    /// declaration and its instance, which [`ServiceCaches::open_values`] opens.
    ///
    /// # Errors
    ///
    /// The refusal of a name that the package does not declare, or that it declares as a
    /// folder cache.
    pub fn values(caller: &HostCaller, name: &str) -> Result<(ValuesDeclaration, Instance), HostServiceError> {
        match ExtensionCache::declared(caller, name)? {
            ExtensionCache { declaration: ExtensionCacheDeclaration::Values(declaration), instance } => {
                Ok((declaration, instance))
            }
            ExtensionCache { declaration: ExtensionCacheDeclaration::Folder(_), .. } => {
                Err(wrong_kind(caller, name, "a values cache (it declares a folder)"))
            }
        }
    }

    /// Returns the folder cache `name` that the package of `caller` declares: its
    /// declaration and its instance, which [`ServiceCaches::open_folder`] opens.
    ///
    /// # Errors
    ///
    /// The refusal of a name that the package does not declare, or that it declares as a
    /// values cache.
    pub fn folder(caller: &HostCaller, name: &str) -> Result<(FolderCacheDeclaration, Instance), HostServiceError> {
        match ExtensionCache::declared(caller, name)? {
            ExtensionCache { declaration: ExtensionCacheDeclaration::Folder(declaration), instance } => {
                Ok((declaration, instance))
            }
            ExtensionCache { declaration: ExtensionCacheDeclaration::Values(_), .. } => {
                Err(wrong_kind(caller, name, "a folder cache (it declares values)"))
            }
        }
    }

    /// Returns the cache `name` that the package of `caller` declares, or the refusal of a
    /// name that it does not declare.
    fn declared(caller: &HostCaller, name: &str) -> Result<ExtensionCache, HostServiceError> {
        let (package, extension) = (&caller.package, &caller.extension.stem);
        let declared = caller.caches.get(name).ok_or_else(|| HostServiceError {
            detail: format!("missing declaration of the cache ‘{name}’ in the package ‘{package}’ of the extension ‘{extension}’"),
        })?;
        Ok(ExtensionCache::new(caller, name, declared))
    }

    /// Checks the declaration alone, as [`Manager::none`] checks it: its whole name (its
    /// characters and its length), and a values cache's engine with its blobs.
    ///
    /// # Errors
    ///
    /// The error of [`Manager::check`] for an incorrect declaration.
    pub fn check(&self) -> Result<(), ManagerError> {
        match &self.declaration {
            ExtensionCacheDeclaration::Values(declaration) => declaration.check(),
            ExtensionCacheDeclaration::Folder(declaration) => Manager::none().check(declaration),
        }
    }

    /// Checks each cache that the package of `caller` declares alone ([`check`](Self::check)),
    /// as a host service that serves the caches checks the package when it loads
    /// ([`HostService::check_package`](flm_ext::host::HostService::check_package)).
    ///
    /// # Errors
    ///
    /// The refusal of the caches that cachette refuses, each named with cachette's message.
    pub fn check_package(caller: &HostCaller) -> Result<(), HostServiceError> {
        let refused: Vec<String> = caller
            .caches
            .iter()
            .filter_map(|(name, declared)| {
                let error = ExtensionCache::new(caller, name, declared).check().err()?;
                Some(error.message().to_owned())
            })
            .collect();
        if refused.is_empty() { Ok(()) } else { Err(HostServiceError { detail: refused.join("; ") }) }
    }
}

/// The refusal of the cache `name` of the package of `caller`, which must be `what`.
fn wrong_kind(caller: &HostCaller, name: &str, what: &str) -> HostServiceError {
    let (package, extension) = (&caller.package, &caller.extension.stem);
    HostServiceError {
        detail: format!("the cache ‘{name}’ of the package ‘{package}’ of the extension ‘{extension}’ must be {what}"),
    }
}

/// The cachette name of the cache `name` of the extension whose stem is `stem`:
/// `flm.ext.STEM.NAME`, `STEM` in lowercase with `@` left out and `/` written `.`.
fn extension_cache_name(stem: &str, name: &str) -> String {
    let stem: String = stem.chars().filter(|c| *c != '@').map(|c| if c == '/' { '.' } else { c.to_ascii_lowercase() }).collect();
    format!("flm.ext.{stem}.{name}")
}

/// The duration of `time`: [`Duration::MAX`] for `unlimited`.
fn duration(time: CacheTime) -> Duration {
    match time {
        CacheTime::Seconds(seconds) => Duration::from_secs(seconds),
        CacheTime::Unlimited => Duration::MAX,
    }
}

/// The key under which the handle keeps a session of a values cache while a build is
/// attached: the cache's name, its version and the canonical text of its instance key.
type KeptKey = (String, u32, String);

/// The caches of a registry's services: the one place that opens the sessions of the
/// host's features and host services and builds their instance keys.
///
/// [`prepare`](crate::session::prepare) makes it from the request's [`CacheHost`] and gives
/// it to each feature and host service that uses a cache; its clones share everything.
/// [`build`](crate::session::build) attaches to it for the time of its run
/// ([`attach`](ServiceCaches::attach)). While a build is attached, a session of a values
/// cache is kept under its cache's name, version and instance key, given again to every
/// caller that opens it, and closed when the run ends, whatever its outcome; a caller holds
/// a session for the time of one call. Without an attached build, a session lasts until its
/// caller drops it. A folder session is never kept: its caller holds it for the time it
/// uses the folder.
///
/// The builds made from one [`Prepared`](crate::session::Prepared) take turns: a second
/// build waits until the first has detached.
#[derive(Clone)]
pub struct ServiceCaches {
    shared: Arc<Shared>,
}

/// What the clones of a [`ServiceCaches`] share.
struct Shared {
    host: CacheHost,
    /// The sessions of values caches of the attached build; `None` when no build is
    /// attached.
    kept: Mutex<Option<HashMap<KeptKey, Arc<ValuesSession>>>>,
    turn: Mutex<()>,
}

impl ServiceCaches {
    /// Returns the handle over `host`, with no build attached.
    pub fn new(host: CacheHost) -> ServiceCaches {
        ServiceCaches { shared: Arc::new(Shared { host, kept: Mutex::new(None), turn: Mutex::new(()) }) }
    }

    /// Returns the instance key of `instance`, or `None` for a cache per document when the
    /// program gave no document's text.
    ///
    /// # Errors
    ///
    /// The error of [`InstanceKey::new`] for a key that cachette refuses (longer than
    /// 64 KiB, or nested more than 64 levels deep).
    pub fn instance_key(&self, instance: &Instance) -> Result<Option<InstanceKey>, ManagerError> {
        let document = self.shared.host.document.as_deref();
        let value = match instance {
            Instance::Shared(value) => value.clone(),
            Instance::Document => match document {
                Some(document) => serde_json::json!({ "document": document }),
                None => return Ok(None),
            },
            Instance::Extension { identity, per_document: false } => serde_json::json!({ "extension": identity }),
            Instance::Extension { identity, per_document: true } => match document {
                Some(document) => serde_json::json!({ "extension": identity, "document": document }),
                None => return Ok(None),
            },
        };
        InstanceKey::new(&value).map(Some)
    }

    /// Opens a session of the values cache `declaration`, on the instance `instance`.
    ///
    /// While a build is attached, the session is the one kept for the cache's name, version
    /// and instance key, opened by the first caller. A cache per document without a
    /// document's text gives a session without a cache.
    ///
    /// # Errors
    ///
    /// An error only for a declaration or an instance key that cachette refuses
    /// ([`ManagerError`] of the kind `Configuration`); the owner then does its work as
    /// without a cache.
    pub fn open_values(
        &self,
        declaration: &ValuesDeclaration,
        instance: &Instance,
    ) -> Result<Arc<ValuesSession>, ManagerError> {
        let manager = &self.shared.host.manager;
        let Some(key) = self.instance_key(instance)? else {
            return Manager::none().open(&declaration.for_manager(manager), &InstanceKey::new(&())?).map(Arc::new);
        };
        let declaration = declaration.for_manager(manager);
        let mut kept = lock(&self.shared.kept);
        let Some(sessions) = kept.as_mut() else {
            drop(kept);
            return manager.open(&declaration, &key).map(Arc::new);
        };
        let kept_key = (declaration.name.to_string(), declaration.version, key.as_json().to_string());
        if let Some(session) = sessions.get(&kept_key) {
            return Ok(Arc::clone(session));
        }
        let session = Arc::new(manager.open(&declaration, &key)?);
        sessions.insert(kept_key, Arc::clone(&session));
        Ok(session)
    }

    /// Opens a session of the folder cache `declaration`, on the instance `instance`. The
    /// session is never kept: the caller holds it for the time it uses the folder. A manager
    /// that offers no folder cache (one not made by [`Manager::open_path`]), and a cache per
    /// document without a document's text, give a session with no folder.
    ///
    /// # Errors
    ///
    /// An error only for a declaration or an instance key that cachette refuses
    /// ([`ManagerError`] of the kind `Configuration`).
    pub fn open_folder(
        &self,
        declaration: &FolderCacheDeclaration,
        instance: &Instance,
    ) -> Result<FolderSession, ManagerError> {
        let manager = &self.shared.host.manager;
        let without_folder = || Manager::none().open_folder(declaration, &InstanceKey::new(&())?);
        let Some(key) = self.instance_key(instance)? else {
            return without_folder();
        };
        if manager.check(declaration).is_err() {
            return without_folder();
        }
        manager.open_folder(declaration, &key)
    }

    /// Attaches a build until the returned guard is dropped: the sessions of values caches
    /// opened meanwhile are kept, and closed when the guard is dropped. Blocks while another
    /// build is attached.
    pub fn attach(&self) -> AttachedCaches<'_> {
        let turn = lock(&self.shared.turn);
        *lock(&self.shared.kept) = Some(HashMap::new());
        AttachedCaches { caches: self, _turn: turn }
    }
}

impl Default for ServiceCaches {
    /// The handle over the default [`CacheHost`]: a manager that keeps nothing.
    fn default() -> ServiceCaches {
        ServiceCaches::new(CacheHost::default())
    }
}

impl fmt::Debug for ServiceCaches {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceCaches").field("host", &self.shared.host).finish_non_exhaustive()
    }
}

/// The guard returned by [`ServiceCaches::attach`]: dropping it closes the sessions kept for
/// the attached build, and lets the next build attach.
#[must_use = "the build is detached as soon as the guard is dropped"]
pub struct AttachedCaches<'a> {
    caches: &'a ServiceCaches,
    _turn: MutexGuard<'a, ()>,
}

impl Drop for AttachedCaches<'_> {
    fn drop(&mut self) {
        // The sessions are closed after the lock is released, since closing one writes to
        // the cache.
        let kept = lock(&self.caches.shared.kept).take();
        drop(kept);
    }
}

impl fmt::Debug for AttachedCaches<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachedCaches").finish_non_exhaustive()
    }
}

/// Locks `mutex`, also after a panic of another holder: the data it guards stays valid.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
