//! The store that the standard provider ([`StandardProvider`](super::StandardProvider))
//! gives the crate `autocitefetch` for one call: the items of this call in memory, the
//! document's cache file, and the central cache of the citation records of all documents
//! ([`StandardProvider::CACHE`](super::StandardProvider::CACHE)).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use autocitefetch::store::{decode_item, encode_item};
use autocitefetch::{CacheStore, CachedItem, CslValue, StoragePolicy, StoreCache, StoreError, Timestamp};
use cachette::{Expiry, Fields, JsonlStore, NewValue, PutOptions, Record, StdFs, Store, ValuesSession};

/// The store of one call of the provider, over three parts: the items that this call stored
/// or removed, in memory; the document's cache file, when the program gave a folder; and a
/// session of the central cache, when the manager keeps it.
///
/// - **Storing.** An item with a lifetime of zero (the records of bibliography files and of
///   `manual` citation keys) is kept in memory only. Every other item is stored in the
///   central cache, when its record source is of a kind that the central cache holds, then
///   in the file with the expiry times that the central cache holds for it, so that the two
///   copies of one fetch are equal; when the central cache does not hold the item just
///   stored (no central cache, a write that failed), the file's expiry times are computed
///   from the lifetime.
/// - **Reading.** The item stored or removed in this call answers first. Else the store reads
///   both copies and answers the one whose hard expiry is later, the file's when both are
///   equal; a central copy that answers is written into the file with its expiry times.
/// - **Removing.** The item is removed from the three parts.
/// - The central cache is never filled from the file: a file comes with a document, which
///   may have been received from someone else, and the central cache serves every document.
/// - **Failures.** A failure of the file is the error of the operation, which the crate
///   returns from its retrieval; this includes the write of a central copy into the file
///   during a read. An operation of the session of the central cache never fails: its events
///   go to the manager's reporter.
///
/// The key of an item in the central cache is `KIND:KEY`, the kind of its record source
/// (`arxiv`, `doi`) and the key in the record source's spelling, so that the documents that
/// register a record source under different prefixes share its items. The crate gives the
/// store each item under its id `PREFIX:KEY`.
pub(super) struct CitationStore {
    policy: StoragePolicy,
    /// The kind of each prefix whose items the central cache holds.
    kinds: BTreeMap<String, &'static str>,
    /// Every `put` (`Some`) and `remove` (`None`) of this call, by id.
    memory: RefCell<HashMap<String, Option<CachedItem>>>,
    /// The document's cache file.
    file: Option<StoreCache<JsonlStore<StdFs>>>,
    /// The session of the central cache, when it keeps the items.
    central: Option<Arc<ValuesSession>>,
}

impl CitationStore {
    /// Returns the store with the storage policy `policy`, over the cache file `file`, and the
    /// session `central` of the central cache, which holds the items of the prefixes of
    /// `kinds`, each with the kind of its record source. A session without a cache counts as
    /// no central cache.
    pub(super) fn new(
        policy: StoragePolicy,
        kinds: BTreeMap<String, &'static str>,
        file: Option<StoreCache<JsonlStore<StdFs>>>,
        central: Option<Arc<ValuesSession>>,
    ) -> CitationStore {
        let file = file.map(|file| file.with_policy(policy));
        let central = central.filter(|session| session.has_cache());
        CitationStore { policy, kinds, memory: RefCell::new(HashMap::new()), file, central }
    }

    /// Returns the session of the central cache and the key under which it holds the item of
    /// `id`, or `None` when there is no central cache or it does not hold the items of the
    /// prefix of `id`.
    fn central(&self, id: &str) -> Option<(&ValuesSession, String)> {
        let session = self.central.as_deref()?;
        let (prefix, key) = id.split_once(':')?;
        let kind = self.kinds.get(prefix)?;
        Some((session, format!("{kind}:{key}")))
    }

    /// Returns the copy of the central cache kept under `key` in `session`.
    fn central_copy(session: &ValuesSession, key: &str) -> Option<CachedItem> {
        let found = session.get(key)?;
        let expiry = found.fields.expiry?;
        decode_item(&found.record).map(|item| CachedItem { item, expiry })
    }

    /// Returns the expiry times of the value that `session` holds under `key`, when it is the
    /// value with the record `record` written at `now` or later: the one that the write just
    /// made. Returns `None` when the write did not take place, so that the value held is an
    /// earlier one, or none.
    ///
    /// The manager's clock and the clock of the crate are both the system clock, and the crate
    /// takes `now` before it stores the item.
    fn central_written(session: &ValuesSession, key: &str, record: &Record, now: Timestamp) -> Option<Expiry> {
        let found = session.get(key)?;
        let fresh = found.fields.written.is_some_and(|written| written >= now) && found.record == *record;
        if fresh { found.fields.expiry } else { None }
    }

    /// Writes `item` into the file under `id`, with the expiry times of `item`.
    fn write_file(file: &StoreCache<JsonlStore<StdFs>>, id: &str, item: &CachedItem) -> Result<(), StoreError> {
        let fields = Fields { expiry: Some(item.expiry), ..Default::default() };
        let value = NewValue::new(encode_item(&item.item)?, fields);
        file.with_store(|store| store.put(id, value))
    }
}

impl CacheStore for CitationStore {
    fn policy(&self) -> StoragePolicy {
        self.policy
    }

    fn get(&self, id: &str) -> Result<Option<CachedItem>, StoreError> {
        if let Some(entry) = self.memory.borrow().get(id) {
            return Ok(entry.clone());
        }
        let local = match &self.file {
            Some(file) => file.get(id)?,
            None => None,
        };
        let central = self.central(id).and_then(|(session, key)| Self::central_copy(session, &key));
        match (local, central) {
            (Some(local), Some(central)) if central.expiry.expires > local.expiry.expires => {
                if let Some(file) = &self.file {
                    Self::write_file(file, id, &central)?;
                }
                Ok(Some(central))
            }
            (Some(local), _) => Ok(Some(local)),
            (None, Some(central)) => {
                if let Some(file) = &self.file {
                    Self::write_file(file, id, &central)?;
                }
                Ok(Some(central))
            }
            (None, None) => Ok(None),
        }
    }

    fn put(&self, id: &str, item: &CslValue, ttl: Duration, now: Timestamp) -> Result<(), StoreError> {
        let computed = CachedItem { item: item.clone(), expiry: self.policy.ttl.expiry(now, ttl, id) };
        if computed.expiry.is_ephemeral() {
            self.memory.borrow_mut().insert(id.to_string(), Some(computed));
            return Ok(());
        }
        // The central cache computes the expiry times it stores with the manager's clock; the
        // file receives the same ones.
        let central = self.central(id).and_then(|(session, key)| {
            let record = encode_item(item).ok()?;
            let options = PutOptions { lifetime: Some(ttl), cost: None };
            session.put(&key, record.clone(), Vec::new(), options);
            Self::central_written(session, &key, &record, now)
        });
        let kept = match central {
            Some(expiry) => CachedItem { item: item.clone(), expiry },
            None => computed,
        };
        let written = match &self.file {
            Some(file) => Self::write_file(file, id, &kept),
            None => Ok(()),
        };
        self.memory.borrow_mut().insert(id.to_string(), Some(kept));
        written
    }

    fn remove(&self, id: &str) -> Result<(), StoreError> {
        self.memory.borrow_mut().insert(id.to_string(), None);
        if let Some((session, key)) = self.central(id) {
            session.remove(&key);
        }
        match &self.file {
            Some(file) => file.remove(id),
            None => Ok(()),
        }
    }

    /// Makes the writes to the file durable; the session of the central cache makes its
    /// writes durable when it is closed.
    fn flush(&self) -> Result<(), StoreError> {
        match &self.file {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }

    /// Removes from the file the items past their grace period; the maintenance of the
    /// manager prunes the central cache.
    fn prune(&self, now: Timestamp) -> Result<usize, StoreError> {
        match &self.file {
            Some(file) => file.prune(now),
            None => Ok(0),
        }
    }
}
