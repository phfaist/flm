//! The test helpers of `flm_core::dump`: the golden-file helper
//! (`flm_core::dump::golden`) — a missing golden fails and shows the actual
//! output, `UPDATE_GOLDEN` rewrites it, a mismatch fails, JSON goldens compare
//! as values — and the diagnostics helper (`flm_core::dump::diag`) — selection
//! by identifier, covered text and position, the assertions on severity, frame
//! count, data and how many diagnostics remain, and the absence assertion.

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use flm_core::dump::diag::ExpectDiag;
use flm_core::dump::golden::{check_json_golden, check_text_golden, GoldenError, GoldenMode};
use flm_core::lang::DollarMathDisabled;
use serde_json::json;
use techy::error::Severity;
use techy::serialize::SerialValue;

use common::{lang, macro_def, parse};

// --- the golden helper ------------------------------------------------------------------------

/// A scratch golden path in a directory of its own, unique to this test
/// process and `name` (tests run in parallel, so no two share a directory);
/// [`Scratch::drop`] removes the file and the directory.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let mut path = std::env::temp_dir();
        path.push(format!("flm-core-dump-helpers-{}-{name}", std::process::id()));
        path.push(name);
        Scratch { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(directory) = self.path.parent() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

#[test]
fn a_missing_golden_fails_with_the_actual_output() {
    let scratch = Scratch::new("missing.txt");
    let error = check_text_golden(&scratch.path, "out\n", GoldenMode::Compare).unwrap_err();
    let path = scratch.path.clone();
    assert_eq!(error, GoldenError::Missing { path, actual: String::from("out\n") });
}

#[test]
fn update_mode_writes_the_golden_which_then_compares_equal() {
    let scratch = Scratch::new("written.txt");
    check_text_golden(&scratch.path, "out\n", GoldenMode::Update).unwrap();
    assert_eq!(std::fs::read_to_string(&scratch.path).unwrap(), "out\n");
    check_text_golden(&scratch.path, "out\n", GoldenMode::Compare).unwrap();
}

#[test]
fn a_mismatching_text_golden_fails_with_both_texts() {
    let scratch = Scratch::new("mismatch.txt");
    check_text_golden(&scratch.path, "one\n", GoldenMode::Update).unwrap();
    let error = check_text_golden(&scratch.path, "two\n", GoldenMode::Compare).unwrap_err();
    assert_eq!(
        error,
        GoldenError::Mismatch {
            path: scratch.path.clone(),
            expected: String::from("one\n"),
            actual: String::from("two\n"),
        }
    );
}

#[test]
fn a_json_golden_compares_as_a_value_not_as_text() {
    let scratch = Scratch::new("hand-edited.json");
    std::fs::create_dir_all(scratch.path.parent().unwrap()).unwrap();
    std::fs::write(&scratch.path, "{\"a\":   [1,\n 2]}").unwrap();
    let value = SerialValue::Map(vec![(
        String::from("a"),
        SerialValue::List(vec![SerialValue::Int(1), SerialValue::Int(2)]),
    )]);
    check_json_golden(&scratch.path, &value, GoldenMode::Compare).unwrap();
}

#[test]
fn a_wrong_json_golden_value_fails() {
    let scratch = Scratch::new("wrong.json");
    check_json_golden(&scratch.path, &SerialValue::Int(1), GoldenMode::Update).unwrap();
    let error =
        check_json_golden(&scratch.path, &SerialValue::Int(2), GoldenMode::Compare).unwrap_err();
    assert!(matches!(error, GoldenError::Mismatch { .. }), "{error:?}");
}

#[test]
fn a_json_golden_that_is_not_json_is_unreadable() {
    let scratch = Scratch::new("garbage.json");
    std::fs::create_dir_all(scratch.path.parent().unwrap()).unwrap();
    std::fs::write(&scratch.path, "not json").unwrap();
    let error =
        check_json_golden(&scratch.path, &SerialValue::Null, GoldenMode::Compare).unwrap_err();
    assert!(matches!(error, GoldenError::Unreadable { .. }), "{error:?}");
}

#[test]
fn the_update_mode_is_selected_by_exactly_one() {
    assert_eq!(GoldenMode::from_setting(Some("1")), GoldenMode::Update);
    assert_eq!(GoldenMode::from_setting(Some("true")), GoldenMode::Compare);
    assert_eq!(GoldenMode::from_setting(Some("")), GoldenMode::Compare);
    assert_eq!(GoldenMode::from_setting(None), GoldenMode::Compare);
}

// --- the diagnostics helper ---------------------------------------------------------------

/// Whether `f` panics (the helper's failure mode).
fn fails(f: impl FnOnce()) -> bool {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = catch_unwind(AssertUnwindSafe(f)).is_err();
    std::panic::set_hook(hook);
    outcome
}

const DOLLAR: &str = "flm.lang.dollar-math-disabled";

#[test]
fn expect_selects_a_diagnostic_by_its_identifier() {
    let lang = lang(vec![macro_def("t", "x", 0)]);
    let fragment = parse(&lang, "a $b");
    let diagnostic = fragment.diags().expect(DOLLAR).once();
    assert_eq!(diagnostic.identifier(), DOLLAR);
}

#[test]
fn expect_fails_when_no_diagnostic_carries_the_identifier() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "a");
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR);
    }));
}

#[test]
fn at_selects_by_the_covered_source_text() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "a $b$");
    fragment.diags().expect(DOLLAR).at("$").count(2);
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR).at("b");
    }));
}

#[test]
fn at_line_col_selects_by_position() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "a\n$b$");
    fragment.diags().expect(DOLLAR).at_line_col(2, 1).once();
    fragment.diags().expect(DOLLAR).at_line_col(2, 3).once();
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR).at_line_col(1, 1);
    }));
}

#[test]
fn data_compares_the_projection_as_a_value() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "$b$");
    fragment
        .diags()
        .expect(DOLLAR)
        .at_line_col(1, 1)
        .data(json!({ "suggestion": DollarMathDisabled::SUGGESTION }));
}

#[test]
fn a_wrong_data_value_fails() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "$b$");
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR).data(json!({ "suggestion": "nope" }));
    }));
}

#[test]
fn severity_is_assertable() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "$b$");
    fragment.diags().expect(DOLLAR).severity(Severity::Error).count(2);
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR).severity(Severity::Note);
    }));
}

#[test]
fn frame_count_is_assertable() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "$b$");
    let selection = fragment.diags().expect(DOLLAR);
    let frames = selection.diagnostics()[0].frames().len();
    selection.frames(frames).count(2);
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR).frames(frames + 1);
    }));
}

#[test]
fn count_asserts_how_many_remain() {
    let lang = lang(vec![]);
    let fragment = parse(&lang, "$b$");
    fragment.diags().expect(DOLLAR).count(2);
    assert!(fails(|| {
        fragment.diags().expect(DOLLAR).count(1);
    }));
}

#[test]
fn expect_none_passes_only_when_the_identifier_is_absent() {
    let lang = lang(vec![]);
    let clean = parse(&lang, "a");
    clean.diags().expect_none(DOLLAR);
    let dollar = parse(&lang, "$b$");
    assert!(fails(|| {
        dollar.diags().expect_none(DOLLAR);
    }));
}

#[test]
fn data_compares_maps_without_regard_to_their_entry_order() {
    // A condition of the crate whose fields are declared out of key order
    // (`table`, `producer`, `element`); the `json!` literal lists them in key
    // order.
    use flm_core::core::run::MissingEntry;
    use flm_core::lang::FlmOrigin;
    use techy::error::Diagnostic;
    use techy::source::{Source, SourceSpan};

    let source = std::sync::Arc::new(Source::new("x").with_origin(FlmOrigin::none()));
    let condition = MissingEntry {
        table: "flm.anchors".into(),
        producer: "flm.refs.anchors".into(),
        element: "flm.heading".into(),
    };
    let diagnostic = Diagnostic::error(condition, SourceSpan::new(&source, 0..1));
    vec![diagnostic].expect("flm.run.missing-entry").data(json!({
        "element": "flm.heading",
        "producer": "flm.refs.anchors",
        "table": "flm.anchors",
    }));
}
