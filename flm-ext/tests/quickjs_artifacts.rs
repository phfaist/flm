//! The files of an output written and read by a stage in JavaScript: a
//! content given as a text, `{ text }` or `{ base64 }`; the read of an
//! artifact set made by another stage, each file with its path under the
//! host's naming of files; an artifact read pushed back as it is; an output
//! declared as made from another (`OutputDef.made_from`).

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::artifact::{Artifact, ArtifactData, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, RelPath};
use flm_core::core::data::table::TableHandle;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::core::settings::PackageSettings;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::json;

use common::{ext_items, js_extension, js_loader, js_manifest};

/// The extension `wc`: the feature `files` declares the artifact sets
/// `wc.files` and `wc.copy`, the table `wc.seen` and the output `wc`, made
/// from the output `text`, whose files are `wc.files` and whose main file has
/// the extension `out`. The stage `wc.write` writes three files into
/// `wc.files`: the main file as a bare text, an image in base64, and a folder
/// holding a file in base64 and a file as `{ text }`. The stage `wc.read`
/// reads `wc.files`, pushes what it read into `wc.seen`, and each artifact it
/// read, as it is, into `wc.copy`.
const MAIN: &str = r#"
import { StageDecl, Effect, Scope, Needs } from "flm:core/stage";
import { StandardTable, OutputDef } from "flm:core/registry";
import { Artifact, ArtifactId } from "flm:core/artifact";

export default function (ext) {
  ext.package("_", { features: { files: { register(b) {
    b.table_named("wc.files", StandardTable.ArtifactSet, "the files of the output");
    b.table_named("wc.copy", StandardTable.ArtifactSet, "a copy of the files");
    b.table("wc.seen", "what the reader read");
    b.output(new OutputDef("wc", "wc.files", "wc.files", "text/plain", "out").made_from("text").docs("The test's output."));
    b.stage({
      decl: () => new StageDecl("wc.write", Effect.Compute, Scope.Document).writes("wc.files"),
      run(cx, out) {
        const files = out.table("wc.files");
        files.push(Artifact.bytes(new ArtifactId("wc", "document"), "text/plain", "Hello.").with_extension("txt").as_entry());
        files.push(Artifact.bytes(new ArtifactId("wc", "image"), "image/png", { base64: "iVBOR/8A" }).with_extension("png"));
        files.push(Artifact.folder(new ArtifactId("wc", "figures"), [["a.bin", { base64: "/wAB" }], ["b.txt", { text: "b" }]]));
      },
    });
    b.stage({
      decl: () => new StageDecl("wc.read", Effect.Compute, Scope.Document).writes("wc.seen").writes("wc.copy"),
      needs: () => new Needs().reads("wc.files"),
      run(cx, out) {
        const read = cx.get("wc.files").projection;
        out.table("wc.seen").push({
          violations: read.violations,
          files: read.artifacts.map((artifact) => [artifact.id.name, artifact.path, artifact.data]),
        });
        for (const artifact of read.artifacts) out.table("wc.copy").push(artifact);
      },
    });
  } } } });
}
"#;

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

/// The registry of the standard package, the extension `wc` and the naming of
/// files [`JobNamer`].
fn registry() -> Arc<FlmRegistry> {
    let loader = js_loader(&JsHost::new(JsSettings::default()), js_extension("wc", &[("main.js", MAIN)], js_manifest("main.js")));
    let package = common::factory_of(&loader, "wc").build(&PackageSettings::new()).expect("the package builds");
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(package);
    builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// The artifact set `name` after `run`.
fn artifacts(run: &Run, name: &str) -> ArtifactSet {
    run.table(TableHandle::<ArtifactSet>::new(name)).expect("the table is declared").cloned().unwrap_or_default()
}

#[test]
fn a_script_writes_contents_in_three_forms_and_reads_the_set_back_with_paths() {
    let registry = registry();
    let run = common::run_demanding(&registry, &["wc.files", "wc.seen", "wc.copy"], &["x"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let files = artifacts(&run, "wc.files");
    assert_eq!(files.entry().and_then(Artifact::as_bytes), Some(&b"Hello."[..]));
    assert_eq!(
        files.get(&ArtifactId::new("wc", "image")).and_then(Artifact::as_bytes),
        Some(&[0x89, b'P', b'N', b'G', 0xff, 0x00][..])
    );
    assert_eq!(
        files.get(&ArtifactId::new("wc", "figures")).map(|folder| folder.data.clone()),
        Some(ArtifactData::Files(vec![(RelPath::new("a.bin"), vec![0xff, 0x00, 0x01]), (RelPath::new("b.txt"), b"b".to_vec())]))
    );
    // The read: in the set's order, each content `{ text }` or `{ base64 }`, each path
    // the one the host writes the file at.
    assert_eq!(
        ext_items(&run, "wc.seen"),
        [json!({
            "violations": [],
            "files": [
                ["document", "job.out", { "bytes": { "text": "Hello." } }],
                ["figures", "job-figures", { "files": [["a.bin", { "base64": "/wAB" }], ["b.txt", { "text": "b" }]] }],
                ["image", "job-image.png", { "bytes": { "base64": "iVBOR/8A" } }],
            ],
        })]
    );
    // Each artifact read was pushed back unchanged.
    let copy = artifacts(&run, "wc.copy");
    assert_eq!(files.iter().collect::<Vec<_>>(), copy.iter().collect::<Vec<_>>());
}

#[test]
fn a_script_declares_the_outputs_its_output_is_made_from() {
    let registry = registry();
    let output = registry.output("wc").expect("declared");
    let made_from: Vec<&str> = output.def.made_from.iter().map(|name| name.as_str()).collect();
    assert_eq!(made_from, ["text"]);
    assert_eq!(output.def.entry.extension, "out");
}
