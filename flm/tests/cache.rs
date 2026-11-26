//! The program's cache, run as the binary: `flm render --no-cache` keeps nothing on disk,
//! even where the environment variable `FLM_CACHE_DIR` names the folder of the cache; and
//! `flm cache` lists the instances of a cache's folder and removes those of a cache.

use std::borrow::Cow;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use flm_host::cache::cachette::{
    Cost, DiscardReports, Manager, ManagerSettings, PutOptions, Record, SystemClock, ValueSort,
};
use flm_host::cache::{CacheHost, Instance, ServiceCaches, ValuesDeclaration, ValuesEngine};

#[test]
fn render_with_no_cache_does_not_create_the_folder_of_flm_cache_dir() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-cache-no-cache");
    let _ = std::fs::remove_dir_all(&folder);
    let render = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_flm"))
            .args(["render", "-p", "text", "-c", "Hello."])
            .args(args)
            .env("FLM_CACHE_DIR", &folder)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "Hello.\n");
    };
    render(&["--no-cache"]);
    assert!(!folder.exists());
    // The same command with the cache: its maintenance makes the folder.
    render(&[]);
    assert!(folder.exists());
    let _ = std::fs::remove_dir_all(&folder);
}

/// The values cache `name`, kept by the JSONL engine.
fn declaration(name: &'static str) -> ValuesDeclaration {
    ValuesDeclaration {
        name: Cow::Borrowed(name),
        version: 1,
        engine: ValuesEngine::Jsonl,
        blobs: false,
        sort: ValueSort::Stable { idle_lifetime: Duration::from_secs(24 * 60 * 60) },
        default_cost: Cost::new(10).unwrap(),
    }
}

#[test]
fn cache_list_names_both_caches_and_clean_removes_only_the_cache_named() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-cache-command");
    let _ = std::fs::remove_dir_all(&root);
    let manager = Manager::open_path(&root, ManagerSettings::default(), SystemClock::new(), DiscardReports).unwrap();
    let caches = ServiceCaches::new(CacheHost { manager, document: None });
    for name in ["flm.tests.command.first", "flm.tests.command.second"] {
        let shared = Instance::Shared(serde_json::Value::Null);
        let session = caches.open_values(&declaration(name), &shared).unwrap();
        session.put("a", Record::from_serialize(&"kept").unwrap(), Vec::new(), PutOptions::default());
    }
    drop(caches);
    let cache = |args: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
        let output = command.arg("cache").args(args).arg("--cache-dir").arg(&root).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    };
    let listed = |text: &str| -> Vec<String> {
        text.lines().skip(1).map(|line| line.split_whitespace().next().unwrap().to_string()).collect()
    };
    let before = cache(&["list"]);
    assert_eq!(before.lines().next(), Some(format!("Cache folder: {}", root.display()).as_str()));
    assert_eq!(listed(&before), ["flm.tests.command.first", "flm.tests.command.second"]);
    let cleaned = cache(&["clean", "flm.tests.command.first"]);
    assert!(cleaned.starts_with("Removed 1 instance ("), "{cleaned}");
    assert_eq!(listed(&cache(&["list"])), ["flm.tests.command.second"]);
    // An invalid name among the names stops the command before it removes anything.
    let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
    let refused = command.args(["cache", "clean", "flm.tests.command.second", "not a name"]);
    assert_eq!(refused.arg("--cache-dir").arg(&root).output().unwrap().status.code(), Some(2));
    assert_eq!(listed(&cache(&["list"])), ["flm.tests.command.second"]);
    let _ = std::fs::remove_dir_all(&root);
}
