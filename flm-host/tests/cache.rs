//! The handle of the services' caches (`ServiceCaches`): the sessions kept while a build is
//! attached, the instance keys of a cache per document, and the engine of a declaration on
//! a manager in memory; the host's own declarations, each checked alone; and the
//! declaration of cachette that a volatile cache of an extension gives; the host service
//! `cache`, across builds and documents and for keys in any order of their members. No test
//! touches the disk.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use flm_host::cache::cachette::{
    Cost, DiscardReports, Manager, ManagerSettings, PutOptions, Record, SystemClock, ValueSort,
};
use flm_host::cache::{CacheHost, Instance, ServiceCaches, ValuesDeclaration, ValuesEngine};

/// A values cache without blobs, kept by the SQLite engine on a manager that offers it.
const RECORDS: ValuesDeclaration = ValuesDeclaration {
    name: Cow::Borrowed("flm.tests.handle.records"),
    version: 1,
    engine: ValuesEngine::Sqlite,
    blobs: false,
    sort: ValueSort::Stable { idle_lifetime: Duration::from_secs(24 * 60 * 60) },
    default_cost: Cost::new(10).unwrap(),
};

/// The handle over a manager in memory, for the document `document`.
fn caches(document: Option<&str>) -> ServiceCaches {
    ServiceCaches::new(CacheHost { manager: in_memory(), document: document.map(str::to_string) })
}

#[test]
fn a_build_attached_keeps_one_session_per_cache_and_key_until_its_run_ends() {
    let caches = caches(None);
    let shared = Instance::Shared(serde_json::Value::Null);
    let attached = caches.attach();
    let first = caches.open_values(&RECORDS, &shared).unwrap();
    let second = caches.open_values(&RECORDS, &shared).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    drop(attached);
    let after = caches.open_values(&RECORDS, &shared).unwrap();
    assert!(!Arc::ptr_eq(&first, &after));
}

#[test]
fn a_cache_per_document_has_an_instance_key_per_document_and_none_without_a_document() {
    let thesis = caches(Some("/papers/thesis.flm"));
    let letter = caches(Some("/papers/letter.flm"));
    let key = |caches: &ServiceCaches| caches.instance_key(&Instance::Document).unwrap().unwrap();
    assert_eq!(key(&thesis), key(&thesis));
    assert_ne!(key(&thesis), key(&letter));
    let without = caches(None);
    assert_eq!(without.instance_key(&Instance::Document).unwrap(), None);
    assert!(!without.open_values(&RECORDS, &Instance::Document).unwrap().has_cache());
}

#[test]
fn a_values_cache_declared_with_sqlite_keeps_a_value_on_a_manager_in_memory() {
    let caches = caches(None);
    let session = caches.open_values(&RECORDS, &Instance::Shared(serde_json::Value::Null)).unwrap();
    assert!(session.has_cache());
    session.put("a", Record::from_serialize(&"kept").unwrap(), Vec::new(), PutOptions::default());
    drop(session);
    let session = caches.open_values(&RECORDS, &Instance::Shared(serde_json::Value::Null)).unwrap();
    let found = session.get("a").unwrap();
    assert_eq!(found.record.deserialize::<String>().unwrap(), "kept");
}

#[test]
fn each_declaration_of_the_host_is_correct_alone() {
    // Each is checked as a manager that keeps nothing checks it, with the engine that the
    // handle opens it with on such a manager.
    let declarations: &[&ValuesDeclaration] = &[
        #[cfg(feature = "graphics")]
        &flm_host::graphics::MagickConverter::CACHE,
        #[cfg(feature = "citations")]
        &flm_host::citations::StandardProvider::CACHE,
        &flm_host::download::CACHE,
    ];
    for declaration in declarations {
        declaration.check().unwrap_or_else(|error| panic!("{}: {error:?}", declaration.name));
    }
}

#[test]
fn a_volatile_cache_of_an_extension_gives_a_volatile_declaration_with_its_lifetime_and_policy() {
    use flm_ext::engine::ExtensionIdentity;
    use std::num::{NonZeroU32, NonZeroU64};

    use flm_ext::host::{
        CacheDeclaration, CacheEngine, CacheExpiry, CacheKind, CachePercent, CacheScope, CacheSort, CacheTime, HostCaller,
    };
    use flm_host::cache::{ExtensionCache, ExtensionCacheDeclaration};
    let caller = HostCaller {
        extension: ExtensionIdentity { identity: String::from("/x/@phf/fetcher/flmextension.yaml"), stem: String::from("@phf/Fetcher"), name: String::from("@phf/Fetcher") },
        package: String::from("_"),
        caches: Default::default(),
        digest_input: Arc::from(Vec::new()),
    };
    let declared = |grace| CacheDeclaration {
        version: NonZeroU32::new(3).unwrap(),
        kind: CacheKind::Values {
            engine: CacheEngine::Sqlite,
            blobs: false,
            sort: CacheSort::Volatile {
                lifetime_s: 3600,
                expiry: CacheExpiry {
                    stale_percent: CachePercent::new(50).unwrap(),
                    jitter_percent: CachePercent::new(5).unwrap(),
                    grace,
                },
            },
        },
        cost_ms: NonZeroU64::new(250).unwrap(),
        scope: CacheScope::Shared,
    };
    for (grace, expected) in [(CacheTime::Seconds(600), Duration::from_secs(600)), (CacheTime::Unlimited, Duration::MAX)] {
        let cache = ExtensionCache::new(&caller, "items", &declared(grace));
        let ExtensionCacheDeclaration::Values(declaration) = &cache.declaration else { panic!("a values cache") };
        assert_eq!((declaration.name.as_ref(), declaration.version), ("flm.ext.phf.fetcher.items", 3));
        let ValueSort::Volatile { lifetime, policy } = declaration.sort else { panic!("volatile values") };
        assert_eq!(lifetime, Duration::from_secs(3600));
        assert_eq!((policy.stale_percent, policy.jitter_percent, policy.grace), (50, 5, expected));
        assert_eq!(declaration.default_cost.as_millis(), 250);
        assert_eq!(cache.check().map_err(|error| error.message().to_string()), Ok(()));
    }
}

// --- the host service `cache` ----------------------------------------------------------------

/// The extension `wc`, whose citation provider answers each key through its cache `answers`
/// (of the scope `SCOPE`): `made` when the cache holds no value for the key, and stores
/// one; `kept RECORD` when it holds one.
#[cfg(feature = "js")]
const ANSWERING: &str = r#"
import { PROVIDERS_SLOT } from "flm:stdlib/features/citations";

export default function (ext) {
  ext.package("_", {
    host: ["cache"],
    caches: { answers: { version: 1, values: { engine: "sqlite" }, unused_s: 86400, cost_ms: 5, scope: "SCOPE" } },
    features: { bib: { register(b) {
      b.contribute_to(PROVIDERS_SLOT, { name: "wc.kept", answer(request, cx) {
        const cache = cx.host("cache");
        const key = { cited: request.key };
        const kept = cache.call("get", { cache: "answers", key });
        if (kept === null) {
          cache.call("put", { cache: "answers", key, record: `answer to ${request.key}` });
          return { entry: cx.build_entry([{ text: "made" }]) };
        }
        return { entry: cx.build_entry([{ text: `kept ${kept.record}` }]) };
      } });
    } } } });
}
"#;

/// The text output of the document `doc/NAME.flm`, citing `k`, prepared afresh over
/// `manager` with the extension [`ANSWERING`] of the scope `scope`; the document's text for
/// the caches is `NAME`.
#[cfg(feature = "js")]
fn answered(manager: &Manager, scope: &str, name: &str) -> String {
    use flm_core::progress::Scope;
    use flm_host::files::MemoryFiles;
    use flm_host::output::{MemorySink, Sink};
    use flm_host::packages::ExtensionFolder;
    use flm_host::session::{Input, Request, build, prepare};

    let path = format!("doc/{name}.flm");
    let files = MemoryFiles::new()
        .with("defs/wc/flmextension.yaml", "extension:\n  engine: js\n  js:\n    main: main.js\n")
        .with("defs/wc/main.js", &ANSWERING.replace("SCOPE", scope))
        .with(&path, "---\nflm:\n  use: [wc]\n---\nA \\cite{k}.\n");
    let mut request = Request::new(Input::File(path));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request.extension_folders = vec![ExtensionFolder { path: String::from("defs"), required: true }];
    request.cache = CacheHost { manager: manager.clone(), document: Some(name.to_string()) };
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    let built = build(&prepared, &Scope::none()).expect("built");
    let diagnostics: Vec<String> = built.diagnostics.iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let entry = sink.files().into_values().find(|file| file.entry).expect("an output");
    String::from_utf8(entry.bytes).unwrap()
}

/// A manager in memory.
fn in_memory() -> Manager {
    Manager::in_memory(ManagerSettings::default(), SystemClock::new(), DiscardReports).unwrap()
}

#[cfg(feature = "js")]
#[test]
fn a_value_stored_during_one_build_is_read_during_the_next_from_another_preparation() {
    let manager = in_memory();
    let first = answered(&manager, "shared", "paper");
    assert!(first.contains("[1] made"), "{first}");
    let second = answered(&manager, "shared", "paper");
    assert!(second.contains("[1] kept answer to k"), "{second}");
}

#[cfg(feature = "js")]
#[test]
fn a_cache_of_the_scope_document_keeps_the_values_of_two_documents_apart() {
    let manager = in_memory();
    assert!(answered(&manager, "document", "thesis").contains("[1] made"));
    let letter = answered(&manager, "document", "letter");
    assert!(letter.contains("[1] made"), "{letter}");
    let thesis = answered(&manager, "document", "thesis");
    assert!(thesis.contains("[1] kept answer to k"), "{thesis}");
}

#[test]
fn two_keys_that_differ_in_the_order_of_an_object_s_members_read_the_same_value() {
    use flm_core::progress::Scope;
    use flm_ext::engine::ExtensionIdentity;
    use std::num::{NonZeroU32, NonZeroU64};

    use flm_ext::host::{CacheDeclaration, CacheEngine, CacheKind, CacheScope, CacheSort, CacheTime, HostAnswer, HostCaller, HostService};
    use flm_host::host_cache::CacheService;
    use serde_json::json;

    let declared = CacheDeclaration {
        version: NonZeroU32::MIN,
        kind: CacheKind::Values {
            engine: CacheEngine::Sqlite,
            blobs: true,
            sort: CacheSort::Stable { unused: CacheTime::Unlimited },
        },
        cost_ms: NonZeroU64::new(5).unwrap(),
        scope: CacheScope::Shared,
    };
    let caller = HostCaller {
        extension: ExtensionIdentity { identity: String::from("/x/wc/flmextension.yaml"), stem: String::from("wc"), name: String::from("wc") },
        package: String::from("_"),
        caches: [(String::from("answers"), declared)].into_iter().collect(),
        digest_input: Arc::from(Vec::new()),
    };
    let service = CacheService::new(ServiceCaches::new(CacheHost { manager: in_memory(), document: None }));
    let call = |method: &str, args| match service.call(&caller, method, &args, &Scope::none()) {
        Ok(HostAnswer::Data(value)) => value,
        other => panic!("{other:?}"),
    };
    let stored = json!({ "formula": "x^2", "options": { "display": true, "scale": 2 } });
    let blobs = json!({ "image.png": { "base64": "AP8=" }, "page.txt": { "text": "x" } });
    assert_eq!(call("put", json!({ "cache": "answers", "key": stored, "record": [1, "one"], "blobs": blobs })), json!(null));
    let reordered = json!({ "options": { "scale": 2, "display": true }, "formula": "x^2" });
    let found = call("get", json!({ "cache": "answers", "key": reordered, "blobs": ["image.png", "page.txt", "none"] }));
    assert_eq!(
        found,
        json!({ "record": [1, "one"], "blobs": blobs, "freshness": null, "refresh": false })
    );
}
