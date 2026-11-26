//! The progress sheet as a script reports on it: a stage written in
//! JavaScript declares a counted row through `cx.progress()` and advances it;
//! a script that never returns stops when its build is cancelled.

#![cfg(feature = "quickjs")]

mod common;

use flm_core::core::run::RunOutcome;
use flm_core::core::settings::PackageSettings;
use flm_core::progress::{Count, Kind, Observer, RowState, Sheet, Unit};
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::json;

use common::{ext_items, registry_with, run_demanding_under};

/// The extension `wc`: the stage `wc.count` declares a row `wc.words`, of a
/// title and a label, counting three items, advances it three times and
/// finishes it.
const MAIN: &str = r#"
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { Unit } from "flm:progress";

export default function (ext) {
  ext.package("_", { features: { count: { register(b) {
    b.table("wc.seen", "what the stage did");
    b.stage({
      decl: () => new StageDecl("wc.count", Effect.Compute, Scope.Document).writes("wc.seen"),
      run(cx, out) {
        const words = cx.progress().child("wc.words", "Counting words", "all").start_counted(Unit.Items, 3);
        for (let i = 0; i < 3; i++) {
          words.advance(1);
        }
        words.finish();
        out.table("wc.seen").push({ counted: 3 });
      },
    });
  } } } });
}
"#;

#[test]
fn a_scripted_stage_declares_a_counted_row_under_its_own() {
    let factory = common::js_factory(&JsHost::new(JsSettings::default()), MAIN);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let (sheet, build) = Sheet::start(&Kind::new("test.build"), None, Observer::default(), None);
    let run = run_demanding_under(&registry, &["wc.seen"], &["Text."], build.scope());
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(ext_items(&run, "wc.seen"), [json!({ "counted": 3 })]);
    build.finish();
    let rows = sheet.snapshot().rows;
    let words = rows.iter().find(|row| row.kind == "wc.words").expect("the stage's row of words");
    assert_eq!(words.state, RowState::Finished);
    assert_eq!(words.count, Some(Count { unit: Unit::Items, total: 3, done: 3 }));
    assert_eq!(words.title.as_deref(), Some("Counting words"));
    assert_eq!(words.label.as_deref(), Some("all"));
    let parent = rows.iter().find(|row| Some(row.id) == words.parent).expect("its parent");
    assert_eq!(parent.kind, "wc.count");
}

/// The extension `wc`: the stage `wc.spin` starts a row `wc.loop`, then never
/// returns.
const SPIN: &str = r#"
import { StageDecl, Effect, Scope } from "flm:core/stage";

export default function (ext) {
  ext.package("_", { features: { spin: { register(b) {
    b.table("wc.seen", "what the stage did");
    b.stage({
      decl: () => new StageDecl("wc.spin", Effect.Compute, Scope.Document).writes("wc.seen"),
      run(cx, out) {
        cx.progress().child("wc.loop", null, null).start();
        for (;;) {}
      },
    });
  } } } });
}
"#;

#[test]
fn a_script_that_never_returns_stops_when_its_build_is_cancelled() {
    // The time budget, far longer than the test, is not what stops the script.
    let settings = JsSettings { time_budget: Some(std::time::Duration::from_secs(600)), ..JsSettings::default() };
    let factory = common::js_factory(&JsHost::new(settings), SPIN);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let (sheet, build) = common::sheet_cancelled_at("wc.loop");
    let started = std::time::Instant::now();
    let run = run_demanding_under(&registry, &["wc.seen"], &["Text."], build.scope());
    assert!(started.elapsed() < std::time::Duration::from_secs(60), "{:?}", started.elapsed());
    assert_eq!(run.outcome(), Some(RunOutcome::Cancelled));
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let rows = sheet.snapshot().rows;
    let spun = rows.iter().find(|row| row.kind == "wc.loop").expect("the stage's row");
    assert_eq!(spun.state, RowState::Failed);
}
