//! The TypeScript check (step E2b, the plan's section 6.3): the fixtures
//! `tests/js/lowerjs/main.js` and `tests/js/wordcount/main.js`, checked as
//! JavaScript against the declarations `flm.d.ts` by the TypeScript compiler
//! of the repository's npm workspace, with the project of its member `flm-ext`
//! (`npm/flm-ext/tsconfig.json`: `checkJs`, not `strict`), so that a method a
//! fixture calls with a wrong name or a wrong number of arguments fails. The
//! fixture `twopack` is left out: it writes a property on `globalThis`, which
//! the check refuses.
//!
//! The check runs only when `FLM_TESTS_TYPESCRIPT` is `1`, `node` is on the
//! `PATH` and the compiler is installed (`npm install` in `npm/`); otherwise
//! it writes a line beginning `skipped:` and passes, which the battery's
//! summary lists.

use std::path::PathBuf;
use std::process::Command;

/// The compiler's script, from the crate's root.
const TSC: &str = "../npm/node_modules/typescript/lib/tsc.js";

/// The project of the check, from the crate's root.
const PROJECT: &str = "../npm/flm-ext/tsconfig.json";

#[test]
fn the_fixtures_type_check_against_the_declarations() {
    if std::env::var("FLM_TESTS_TYPESCRIPT").as_deref() != Ok("1") {
        eprintln!("skipped: FLM_TESTS_TYPESCRIPT is not set");
        return;
    }
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no node on the PATH");
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !root.join(TSC).is_file() {
        eprintln!("skipped: run `npm install` in npm/ for the TypeScript compiler");
        return;
    }
    let output = Command::new("node")
        .arg(TSC)
        .args(["-p", PROJECT])
        .current_dir(&root)
        .output()
        .expect("node runs the TypeScript compiler");
    let report = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "the TypeScript check of `tests/js/lowerjs/main.js` and `tests/js/wordcount/main.js` against `flm.d.ts` must pass:\n{report}"
    );
}
