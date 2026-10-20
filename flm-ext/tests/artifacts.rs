//! The artifact set a scripted stage writes and reads (`api::core::artifact`):
//! the content of a file as a text, `{ text }` or `{ base64 }`, the read of a
//! set with the path of each file, with and without the host's naming of
//! files; the standard table types declared by name
//! (`FeatureBuilder::table_named`) and the typed writes of a scripted stage,
//! through the Rust façade and through the dispatcher.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::artifact::{Artifact, ArtifactData, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, FileRef, NamingScope, RelPath};
use flm_core::core::data::table::{TableHandle, TableType};
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::region::Regions;
use flm_core::core::registry::{FlmRegistry, OutputDef, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::lowering::{LoweredTrees, NAMER_SLOT};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_core::stdcore::backends::pieces::stage::LoweredPieces;
use flm_ext::adapter::values::{artifact_from_value, artifact_set_to_value};
use flm_ext::api::core::artifact::media_type_of;
use flm_ext::api::core::data::store::StageOut;
use flm_ext::api::core::data::table::TableValue;
use flm_ext::api::core::registry::StandardTable;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::json_double::{module, JsonDouble};
use common::{registry_with, try_registry_with};

/// What the handlers of a test observed, in order.
type Log = Arc<Mutex<Vec<String>>>;

/// Records `line` in `log`.
fn note(log: &Log, line: impl Into<String>) {
    log.lock().expect("the log").push(line.into());
}

/// The run of a plan demanding `demands` over the one-tree document `text`.
fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], text: &str) -> Run {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The artifact set `name` after `run`.
fn artifacts(run: &Run, name: &str) -> ArtifactSet {
    run.table(TableHandle::<ArtifactSet>::new(name)).expect("the table is declared").cloned().unwrap_or_default()
}

/// The entry artifact `aw:document`, of media type `text/plain`, holding `text`.
fn document(text: &str) -> Value {
    json!({
        "id": { "owner": "aw", "name": "document" },
        "media_type": "text/plain",
        "entry": true,
        "data": { "bytes": text },
        "extension": "txt",
        "refs": [{ "owner": "aw", "name": "figures" }],
    })
}

/// The folder artifact `aw:figures` of two files, at a fixed path.
fn figures() -> Value {
    json!({
        "id": { "owner": "aw", "name": "figures" },
        "media_type": "",
        "data": { "files": [["a.svg", "<svg/>"], ["b.txt", "b"]] },
        "fixed_path": "figures",
        "entry": null,
    })
}

/// A stage's writing work: given the output and the log.
type Write = dyn Fn(&mut dyn StageOut, &Log) -> Result<(), ExtError> + Send + Sync;

/// The package `aw`, whose feature `files` declares the artifact set
/// `aw.files` and the table `aw.seen`, a document-scope stage `aw.write`
/// writing the set through `write`, and a stage `aw.read` reading the set back
/// by name (its answer logged) and writing `aw.seen`.
fn writing(log: &Log, write: Arc<Write>) -> Arc<FlmRegistry> {
    let log = Arc::clone(log);
    registry_with([Double::new("aw")
        .feature("files", move |b| {
            b.table_named("aw.files", StandardTable::ArtifactSet, "The files of the output.")?;
            b.table("aw.seen", "What the reader saw.")?;
            let (write, writer_log) = (Arc::clone(&write), Arc::clone(&log));
            let decl = StageDecl::new("aw.write", Effect::Compute, Scope::Document).writes("aw.files");
            b.stage(stage(decl, Needs::new(), move |_, out| write(out, &writer_log)))?;
            let reader_log = Arc::clone(&log);
            let decl = StageDecl::new("aw.read", Effect::Compute, Scope::Document).writes("aw.seen");
            b.stage(stage(decl, Needs::new().reads("aw.files"), move |cx, _| {
                let read = cx.get("aw.files").map(|_| "read").map_err(|error| error.kind().to_string());
                note(&reader_log, format!("read: {read:?}"));
                Ok(())
            }))
        })
        .package()])
}

/// Pushes `item` into the table `aw.files`, logging a refusal's text.
fn push(out: &mut dyn StageOut, log: &Log, item: Value) -> Result<(), ExtError> {
    if let Err(error) = out.table("aw.files")?.push(item) {
        note(log, format!("{}: {}", error.kind(), error));
    }
    Ok(())
}

// --- the artifact set --------------------------------------------------------------------------

#[test]
fn a_scripted_stage_writes_an_artifact_set_declared_by_name() {
    let log: Log = Arc::default();
    let registry = writing(
        &log,
        Arc::new(|out, log| {
            push(out, log, document("Hello."))?;
            push(out, log, figures())
        }),
    );
    let run = run_demanding(&registry, &["aw.files"], "x");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let set = artifacts(&run, "aw.files");
    assert_eq!(set.len(), 2);
    let entry = set.entry().expect("an entry artifact");
    assert_eq!(entry.id, ArtifactId::new("aw", "document"));
    assert_eq!(entry.media_type, "text/plain");
    assert_eq!(entry.as_bytes(), Some(&b"Hello."[..]));
    assert_eq!(entry.extension.as_deref(), Some("txt"));
    assert_eq!(entry.refs, vec![ArtifactId::new("aw", "figures")]);
    assert_eq!(entry.fixed_path, None);
    let folder = set.get(&ArtifactId::new("aw", "figures")).expect("the folder");
    assert!(!folder.entry);
    assert_eq!(folder.fixed_path, Some(RelPath::new("figures")));
    assert_eq!(
        folder.data,
        ArtifactData::Files(vec![(RelPath::new("a.svg"), b"<svg/>".to_vec()), (RelPath::new("b.txt"), b"b".to_vec())])
    );
    assert!(log.lock().expect("the log").is_empty(), "{log:?}");
}

#[test]
fn a_second_entry_and_a_repeated_identity_are_the_push_s_errors() {
    let log: Log = Arc::default();
    let registry = writing(
        &log,
        Arc::new(|out, log| {
            push(out, log, document("One."))?;
            let mut second = figures();
            second["entry"] = json!(true);
            push(out, log, second)?;
            push(out, log, document("Two."))
        }),
    );
    let run = run_demanding(&registry, &["aw.files"], "x");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        *log.lock().expect("the log"),
        [
            "core: an output has only one main file, `aw:document`, so `aw:figures` must not also be marked as the main file",
            "core: two different files of an output must not share the identity `aw:document`",
        ]
    );
    // The set is left as it was by each refused push.
    let set = artifacts(&run, "aw.files");
    assert_eq!(set.len(), 1);
    assert_eq!(set.entry().and_then(Artifact::as_bytes), Some(&b"One."[..]));
}

#[test]
fn an_artifact_of_the_wrong_shape_is_refused_with_the_shape() {
    // The refusal names the field or the shape at fault, never the value
    // itself (ruling S10): an artifact's text is not copied into it.
    let shape = "an artifact must be written `{ id: { owner, name }, media_type, data: { bytes: CONTENT } | { files: \
                 [[path, CONTENT], …] }, entry, extension, fixed_path, refs }` (`CONTENT`: a text, `{ text: text }` or `{ \
                 base64: text }`); ";
    let data = "its field `data` must be `{ bytes: CONTENT }` or `{ files: [[path, CONTENT], …] }`";
    for (wrong, fault) in [
        (json!("secret text"), "this one is a text"),
        (json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "text/plain" }), data),
        (json!({ "id": "aw:x", "media_type": "text/plain", "data": { "bytes": "" } }), "its field `id` must be `{ owner, name }`, two texts"),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "text/plain", "data": { "bytes": "secret" }, "size": 3 }),
            "`size` is not one of its fields",
        ),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "text/plain", "data": { "bytes": "" }, "entry": "yes" }),
            "its field `entry` must be `true`, `false` or `null`, not a text",
        ),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": 3, "data": { "bytes": "" } }),
            "its field `media_type` must be a text",
        ),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "", "data": { "bytes": "" }, "refs": "aw:y" }),
            "its field `refs` must be a list or `null`, not a text",
        ),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "", "data": { "bytes": "" }, "refs": ["aw:y"] }),
            "each item of its field `refs` must be `{ owner, name }`, two texts",
        ),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "", "data": { "bytes": "" }, "extension": 1 }),
            "its field `extension` must be a text or `null`, not a number",
        ),
        (
            json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "text/plain", "data": { "files": [["a"]] } }),
            "each file of its field `data` must be `[path, CONTENT]`",
        ),
        (json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "text/plain", "data": { "bytes": "", "files": [] } }), data),
    ] {
        let error = artifact_from_value(&wrong).expect_err("a wrong shape");
        assert_eq!(error.kind(), "value");
        assert_eq!(error.to_string(), format!("{shape}{fault}"));
        assert!(!error.to_string().contains("secret"), "{error}");
    }
    // A content of another form, or a text that is not base64, is refused with the
    // artifact's identity, and the file's path in a folder artifact.
    let forms = "must be a text, `{ text: text }` or `{ base64: text }`";
    let map = ", a map, must be `{ text: text }` or `{ base64: text }`";
    let base64 = " must be base64 in the standard alphabet, with padding";
    for (content, fault) in [
        (json!([1, 2]), format!(" {forms}, not a list")),
        (json!(null), format!(" {forms}, not `null`")),
        (json!({ "text": "secret", "base64": "c2VjcmV0" }), String::from(map)),
        (json!({ "base64": 3 }), String::from(map)),
        (json!({ "base64": "secret!" }), String::from(base64)),
        (json!({ "base64": "c2VjcmV0dA" }), String::from(base64)),
    ] {
        let wrong = json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "text/plain", "data": { "bytes": content } });
        let error = artifact_from_value(&wrong).expect_err("a content refused");
        assert_eq!(error.to_string(), format!("the content of the artifact `aw:x`{fault}"));
        let folder = json!({ "id": { "owner": "aw", "name": "x" }, "media_type": "", "data": { "files": [["a.bin", content]] } });
        let error = artifact_from_value(&folder).expect_err("a file's content refused");
        assert_eq!(error.to_string(), format!("the content of the file `a.bin` of the artifact `aw:x`{fault}"));
        assert!(!error.to_string().contains("secret"), "{error}");
    }
    // Through a stage: the push's error, the set unchanged.
    let log: Log = Arc::default();
    let registry = writing(&log, Arc::new(|out, log| push(out, log, json!({ "id": "x" }))));
    let run = run_demanding(&registry, &["aw.files"], "x");
    assert!(artifacts(&run, "aw.files").is_empty());
    assert_eq!(*log.lock().expect("the log"), [format!("value: {shape}its field `id` must be `{{ owner, name }}`, two texts")]);
}

#[test]
fn the_three_forms_of_a_content_are_pushed() {
    let log: Log = Arc::default();
    let registry = writing(
        &log,
        Arc::new(|out, log| {
            let mut entry = document("unused");
            entry["data"] = json!({ "bytes": { "text": "Hello." } });
            push(out, log, entry)?;
            push(out, log, image())?;
            let mut folder = figures();
            folder["data"] = json!({ "files": [["a.bin", { "base64": "AAEC" }], ["b.txt", "bare"]] });
            push(out, log, folder)
        }),
    );
    let run = run_demanding(&registry, &["aw.files"], "x");
    assert!(log.lock().expect("the log").is_empty(), "{log:?}");
    let set = artifacts(&run, "aw.files");
    assert_eq!(set.entry().and_then(Artifact::as_bytes), Some(&b"Hello."[..]));
    assert_eq!(set.get(&ArtifactId::new("aw", "image")).and_then(Artifact::as_bytes), Some(&PNG[..]));
    assert_eq!(
        set.get(&ArtifactId::new("aw", "figures")).map(|folder| folder.data.clone()),
        Some(ArtifactData::Files(vec![(RelPath::new("a.bin"), vec![0, 1, 2]), (RelPath::new("b.txt"), b"bare".to_vec())]))
    );
}

#[test]
fn malformed_base64_is_the_push_s_error_naming_the_artifact() {
    let log: Log = Arc::default();
    let registry = writing(
        &log,
        Arc::new(|out, log| {
            let mut entry = document("unused");
            entry["data"] = json!({ "bytes": { "base64": "not base64" } });
            push(out, log, entry)
        }),
    );
    let run = run_demanding(&registry, &["aw.files"], "x");
    assert!(artifacts(&run, "aw.files").is_empty());
    assert_eq!(
        *log.lock().expect("the log"),
        ["value: the content of the artifact `aw:document` must be base64 in the standard alphabet, with padding"]
    );
}

/// The bytes of the binary artifact `aw:image`, which are not UTF-8.
const PNG: [u8; 6] = [0x89, b'P', b'N', b'G', 0xff, 0x00];

/// The artifact `aw:image`, of media type `image/png`, holding [`PNG`] in base64.
fn image() -> Value {
    json!({
        "id": { "owner": "aw", "name": "image" },
        "media_type": "image/png",
        "data": { "bytes": { "base64": "iVBOR/8A" } },
        "extension": "png",
    })
}

/// Names an entry artifact `job.EXT`, any other `job-NAME.EXT` (`EXT` the
/// extension it is given, `bin` without one) and a folder `job-NAME`.
struct JobNamer;

impl ArtifactNamer for JobNamer {
    fn url_of(&self, artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
        let base = if artifact.entry { String::from("job") } else { format!("job-{}", artifact.id.name) };
        match artifact.folder {
            true => RelPath::new(base),
            false => RelPath::new(format!("{base}.{}", artifact.extension.unwrap_or("bin"))),
        }
    }
}

/// What the stage `aw.read` of [`reading`] read from the table `aw.files`.
type Seen = Arc<Mutex<Option<Value>>>;

/// The package `aw`: the feature `files` declares the artifact sets `aw.files`
/// and `aw.copy` and the output `aw`, whose files are `aw.files` and whose main
/// file has the extension `out`; the stage `aw.write` writes `aw.files` through
/// `write`, and the stage `aw.read` keeps its read of `aw.files` in `seen` and
/// pushes each artifact it read into `aw.copy` as it is. The registry holds
/// the naming of files [`JobNamer`] when `namer` is true.
fn reading(log: &Log, write: Arc<Write>, seen: &Seen, namer: bool) -> Arc<FlmRegistry> {
    let (log, seen) = (Arc::clone(log), Arc::clone(seen));
    let double = Double::new("aw").feature("files", move |b| {
        b.table_named("aw.files", StandardTable::ArtifactSet, "The files of the output.")?;
        b.table_named("aw.copy", StandardTable::ArtifactSet, "A copy of the files.")?;
        b.output(OutputDef::new("aw", "aw.files", "aw.files", "text/plain", "out"))?;
        let (write, writer_log) = (Arc::clone(&write), Arc::clone(&log));
        let decl = StageDecl::new("aw.write", Effect::Compute, Scope::Document).writes("aw.files");
        b.stage(stage(decl, Needs::new(), move |_, out| write(out, &writer_log)))?;
        let seen = Arc::clone(&seen);
        let decl = StageDecl::new("aw.read", Effect::Compute, Scope::Document).writes("aw.copy");
        b.stage(stage(decl, Needs::new().reads("aw.files"), move |cx, out| {
            let Some(TableValue::Projection(read)) = cx.get("aw.files")? else { panic!("a projection") };
            for artifact in read["artifacts"].as_array().expect("a list") {
                out.table("aw.copy")?.push(artifact.clone())?;
            }
            *seen.lock().expect("seen") = Some(read);
            Ok(())
        }))
    });
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(double.package());
    if namer {
        builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    }
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// Writes the entry artifact `aw:document`, the binary artifact `aw:image` and
/// the folder artifact `aw:figures` (a binary file and a text file, at the
/// fixed path `figures`).
fn three_files(out: &mut dyn StageOut, log: &Log) -> Result<(), ExtError> {
    push(out, log, document("Hello."))?;
    push(out, log, image())?;
    let mut folder = figures();
    folder["data"] = json!({ "files": [["a.bin", { "base64": "/wAB" }], ["b.txt", "b"]] });
    push(out, log, folder)
}

#[test]
fn a_stage_reads_an_artifact_set_with_the_path_of_each_file() {
    let (log, seen): (Log, Seen) = Default::default();
    let registry = reading(&log, Arc::new(three_files), &seen, true);
    let run = run_demanding(&registry, &["aw.files", "aw.copy"], "x");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let read = seen.lock().expect("seen").clone().expect("read");
    // In the set's order (by identity); each content `{ text }` when it is UTF-8 and
    // `{ base64 }` otherwise; the main file named with its output's extension.
    assert_eq!(
        read,
        json!({
            "artifacts": [
                {
                    "id": { "owner": "aw", "name": "document" }, "media_type": "text/plain", "entry": true,
                    "data": { "bytes": { "text": "Hello." } }, "fixed_path": null,
                    "refs": [{ "owner": "aw", "name": "figures" }], "extension": "txt", "path": "job.out",
                },
                {
                    "id": { "owner": "aw", "name": "figures" }, "media_type": "", "entry": false,
                    "data": { "files": [["a.bin", { "base64": "/wAB" }], ["b.txt", { "text": "b" }]] },
                    "fixed_path": "figures", "refs": [], "extension": null, "path": "figures",
                },
                {
                    "id": { "owner": "aw", "name": "image" }, "media_type": "image/png", "entry": false,
                    "data": { "bytes": { "base64": "iVBOR/8A" } }, "fixed_path": null, "refs": [],
                    "extension": "png", "path": "job-image.png",
                },
            ],
            "violations": [],
        })
    );
    // Each artifact read was pushed back unchanged, its `path` ignored.
    let (files, copy) = (artifacts(&run, "aw.files"), artifacts(&run, "aw.copy"));
    assert_eq!(files.iter().collect::<Vec<_>>(), copy.iter().collect::<Vec<_>>());
    assert!(log.lock().expect("the log").is_empty(), "{log:?}");
}

#[test]
fn without_a_naming_of_files_only_a_fixed_path_is_known() {
    let (log, seen): (Log, Seen) = Default::default();
    let registry = reading(&log, Arc::new(three_files), &seen, false);
    let run = run_demanding(&registry, &["aw.files", "aw.copy"], "x");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let read = seen.lock().expect("seen").clone().expect("read");
    let paths: Vec<&Value> = read["artifacts"].as_array().expect("a list").iter().map(|artifact| &artifact["path"]).collect();
    assert_eq!(paths, [&Value::Null, &json!("figures"), &Value::Null]);
}

/// A stage run once per tree writes, in each of two trees, a different file of one
/// identity: the set combining the two runs' files keeps one and records the violation,
/// which the read gives in `violations`.
#[test]
fn a_read_gives_the_violations_the_set_recorded() {
    let (log, seen): (Log, Seen) = Default::default();
    let (writer_log, read_into) = (Arc::clone(&log), Arc::clone(&seen));
    let double = Double::new("aw").feature("files", move |b| {
        b.table_named("aw.files", StandardTable::ArtifactSet, "The files of the output.")?;
        b.table("aw.seen", "What the reader saw.")?;
        let log = Arc::clone(&writer_log);
        let decl = StageDecl::new("aw.write", Effect::Compute, Scope::PerTree).writes("aw.files");
        b.stage(stage(decl, Needs::new(), move |cx, out| {
            let mut file = image();
            file["data"] = json!({ "bytes": cx.tree_id().unwrap_or_default() });
            push(out, &log, file)
        }))?;
        let seen = Arc::clone(&read_into);
        let decl = StageDecl::new("aw.read", Effect::Compute, Scope::Document).writes("aw.seen");
        b.stage(stage(decl, Needs::new().reads("aw.files"), move |cx, _| {
            let Some(TableValue::Projection(read)) = cx.get("aw.files")? else { panic!("a projection") };
            *seen.lock().expect("seen") = Some(read);
            Ok(())
        }))
    });
    let registry = registry_with([double.package()]);
    let mut content = DocContent::new();
    for text in ["One tree.", "Another tree."] {
        let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    let plan = Arc::new(Plan::build(&registry, &PlanRequest::new().demand("aw.seen")).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    assert!(log.lock().expect("the log").is_empty(), "{log:?}");
    let read = seen.lock().expect("seen").clone().expect("read");
    assert_eq!(read["artifacts"].as_array().map(Vec::len), Some(1), "{read}");
    assert_eq!(read["violations"], json!(["two different files of an output must not share the identity `aw:image`"]));
}

#[test]
fn a_set_holding_a_reference_to_a_file_is_refused_naming_the_artifact() {
    let mut set = ArtifactSet::new();
    let mut held = Artifact::bytes(ArtifactId::new("aw", "big"), "video/mp4", Vec::new());
    held.data = ArtifactData::FileRef(FileRef { location: String::from("/tmp/big.mp4") });
    set.insert(held).expect("inserted");
    let error = artifact_set_to_value(&set, &|_| None).expect_err("refused");
    assert_eq!(error.kind(), "value");
    assert_eq!(
        error.to_string(),
        "the artifact `aw:big` must hold its bytes to be read by an extension, not refer to a file holding them"
    );
}

// --- the typed writes and the standard tables ----------------------------------------------------

#[test]
fn a_stage_writing_a_table_of_lowered_trees_fails_the_build() {
    let double = Double::new("aw").feature("files", |b| {
        b.table_named("aw.lowered", StandardTable::LoweredTrees, "Lowered trees.")?;
        let decl = StageDecl::new("aw.write", Effect::Compute, Scope::Document).writes("aw.lowered");
        b.stage(stage(decl, Needs::new(), |_, _| Ok(())))
    });
    let error = try_registry_with([double.package()]).expect_err("the build fails").to_string();
    assert!(
        error.contains(
            "the stage `aw.write` of the package `_` of the extension `aw` writes the table `aw.lowered`, which holds \
             values of the type `LoweredTrees`; a stage of an extension writes a table of its own, a table of a \
             definition's text or a table of an artifact set"
        ),
        "{error}"
    );
}

#[test]
fn table_named_declares_a_table_of_each_standard_type() {
    let double = Double::new("aw").feature("tables", |b| {
        b.table_named("aw.lowered", StandardTable::LoweredTrees, "Lowered trees.")?;
        b.table_named("aw.pieces", StandardTable::LoweredPieces, "Lowered pieces.")?;
        b.table_named("aw.regions", StandardTable::Regions, "Regions.")?;
        b.table_named("aw.files", StandardTable::ArtifactSet, "Files.")
    });
    let registry = registry_with([double.package()]);
    for (name, ty) in [
        ("aw.lowered", TableType::of::<LoweredTrees>()),
        ("aw.pieces", TableType::of::<LoweredPieces>()),
        ("aw.regions", TableType::of::<Regions>()),
        ("aw.files", TableType::of::<ArtifactSet>()),
    ] {
        let entry = registry.table_named(name).unwrap_or_else(|| panic!("`{name}` is declared"));
        assert_eq!(entry.def.ty, ty, "{name}");
    }
    let outside = Double::new("aw").feature("tables", |b| b.table_named("files", StandardTable::ArtifactSet, "Files."));
    let error = try_registry_with([outside.package()]).expect_err("the name rule").to_string();
    assert!(error.contains("the table `files` declared by the package `_` of the extension `aw` must begin with `aw.`"), "{error}");
}

#[test]
fn media_type_of_answers_the_host_s_table() {
    assert_eq!(media_type_of("css").as_deref(), Some("text/css"));
    assert_eq!(media_type_of("HTML").as_deref(), Some("text/html"));
    assert_eq!(media_type_of("typ"), None);
}

// --- through the dispatcher ------------------------------------------------------------------------

#[test]
fn an_artifact_set_is_declared_and_written_through_the_dispatcher() {
    let log: Log = Arc::default();
    let seen = Arc::clone(&log);
    let double = JsonDouble::new("aw").feature("files", move |js, b| {
        js.call(b, "table_named", json!(["aw.files", "artifact-set", "The files of the output."]))?;
        let refused = js.call(b, "table_named", json!(["aw.other", "tree-set", "x"])).map_err(|error| error.to_string());
        note(&seen, format!("{refused:?}"));
        let css = js.call(&module("core/artifact"), "media_type_of", json!(["css"]))?;
        let typ = js.call(&module("core/artifact"), "media_type_of", json!(["typ"]))?;
        note(&seen, format!("{css} {typ}"));
        let stage = js.lend_stage(
            &json!({ "name": "aw.write", "effect": "compute", "scope": "document", "writes": ["aw.files"] }),
            &json!({}),
            |js, _, out| {
                let piece = js.call(out, "table", json!(["aw.files"]))?;
                js.call(&piece, "push", json!([document("Through JSON.")]))?;
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([stage]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let run = run_demanding(&registry, &["aw.files"], "x");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let set = artifacts(&run, "aw.files");
    assert_eq!(set.entry().and_then(Artifact::as_bytes), Some(&b"Through JSON."[..]));
    let log = log.lock().expect("the log").clone();
    assert_eq!(log[1], "\"text/css\" null");
    assert!(log[0].contains("a standard table type"), "{log:?}");
}
