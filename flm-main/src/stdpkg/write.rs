//! The feature `write` of the package `stdmain`: the stage that hands the files of every
//! output that the plan demands
//! ([`PlanRequest`](flm_core::core::plan::PlanRequest)) to the host's sink, the
//! destination of the files, and the host's naming scheme for those files.
//!
//! An output's artifact set holds the output as values, one artifact per file, each
//! identified by its producer and a name and never by a path ([`ArtifactSet`]). Writing it
//! takes two parts that the host gives ([`StdParts`](super::StdParts)): a naming scheme,
//! which turns the identity of each artifact into a path ([`ArtifactNamer`]; the host's
//! naming scheme is [`Naming`](crate::output::Naming)), and a sink, the destination that
//! receives the named files: the disk, memory or the standard output ([`Sink`]).
//!
//! The feature registers:
//!
//! - the naming scheme, contributed to the library's service slot `flm.artifacts.namer`
//!   ([`NAMER_SLOT`]) under every command. A service slot is the declaration, by the
//!   feature that owns an interaction, of a dotted name, the type of the values the slot
//!   takes and its arity
//!   ([`service_slot`](flm_core::core::registry::FeatureBuilder::service_slot)). The
//!   naming scheme is contributed under every command because a stage may ask for the
//!   path of an artifact whether or not the output is written;
//! - the service slot [`SINK_SLOT`] (`flm.write.sink`), which takes one sink, and the sink
//!   itself when the host gives one;
//! - the capability [`WRITTEN`] (`flm.output.written`), which a command that writes an
//!   output demands. A capability is a registered dotted name for a state of processing
//!   that a stage brings about ([`CapabilityDef`]);
//! - the setting `write.output_names` ([`WriteRenderOptions`]), bound to the render
//!   request and combined entry by entry: a map from an output's name to the path, relative
//!   to the output folder, of that output's entry file (its main file), taken exactly as
//!   written;
//! - the stage [`STAGE`] (`flm.write.files`), which provides [`WRITTEN`]. For every output
//!   the plan demands, in the registry's order, it reads the output's artifact set and
//!   gives each artifact its path ([`crate::output::place`]), under the outputs of the
//!   command ([`NamingScope::of`]): the entry artifact of an output that
//!   `write.output_names` lists takes the path the setting gives, and every other artifact
//!   the path that the naming scheme of the slot `flm.artifacts.namer` gives
//!   ([`EntryNames`]). The other files of a renamed output keep the scheme's stem: their
//!   producers ask the naming scheme for their paths during the run, before the output
//!   they belong to is known, and only this stage names an entry artifact. An entry of
//!   `write.output_names` that names no output of the registry is the Error
//!   [`UnknownOutput`]. Two files of two outputs at
//!   one path are one file when their bytes and media types are equal, written once and
//!   listed under both outputs; when they differ, they are an error. The stage then hands
//!   the files of every output to the sink in one delivery, or, when any output could not
//!   be placed or two files collide, hands nothing: the files of a command are written
//!   together or not at all. It reports each problem as the Error [`WriteFailed`], and an
//!   unknown output in `write.output_names` as the Error [`UnknownOutput`], which also
//!   leaves every output unwritten.
//!
//! Under a command that writes nothing (`flm check`), the host gives no sink, nothing
//! demands [`WRITTEN`], and the stage is registered but left out of the plan; an unknown
//! output in `write.output_names` is then not reported.
//!
//! The stage's effect is input and output ([`Effect::Io`]), so the plan orders it after
//! every stage that computes or checks. A run that has recorded an Error diagnostic stops
//! before it (the library's error gate) unless the host asks the run to continue with
//! errors
//! ([`RunOptions::continue_with_errors`](flm_core::core::run::RunOptions::continue_with_errors)).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use flm_core::core::artifact::{ArtifactNamer, ArtifactSet, NamingScope};
use flm_core::core::data::store::StageOut;
use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::PlanCx;
use flm_core::core::registry::{BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature, RegisteredOutput};
use flm_core::core::run::StageCx;
use flm_core::core::services::Arity;
use flm_core::core::settings::SettingsDecl;
use flm_core::model::options::Bind;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::{FlmOrigin, FlmSpan};
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::techy::error::{Diagnostic, DiagnosticInfo};
use flm_core::techy::source::{Source, SourceSpan};

use crate::output::{EntryNames, NamedFile, Sink, place};

/// The name under which the feature is registered, `write`.
pub const FEATURE: &str = "write";

/// The name of the service slot `flm.write.sink`, which takes the one [`Sink`] that the
/// stage [`STAGE`] hands files to.
pub const SINK_SLOT: &str = "flm.write.sink";

/// The name of the stage that hands the files of the outputs to the sink,
/// `flm.write.files`.
pub const STAGE: &str = "flm.write.files";

/// The capability `flm.output.written`: every output the plan demands was handed to the
/// sink. The host adds it to the demands of a plan whose command writes an output.
pub const WRITTEN: &str = "flm.output.written";

/// Condition `flm.write.failed` (Error): an output that the plan demands cannot be
/// written. The stage [`STAGE`] reports it once per problem: no naming scheme or no sink
/// to hand the output to, no artifact set made for the output, an artifact whose path
/// leaves the output's location, two artifacts of the output given one path, an artifact
/// that holds no bytes, two outputs writing different files at one path, or a file that
/// the sink cannot write. Any problem but the last leaves every output of the command
/// unwritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteFailed {
    /// The output's name.
    pub output: String,
    /// The problem, as the placing of the output's files ([`place`]) or the sink states
    /// it.
    pub detail: String,
}

impl fmt::Display for WriteFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the output `{}` cannot be written: {}", self.output, self.detail)
    }
}

impl DiagnosticInfo for WriteFailed {
    const IDENTIFIER: &'static str = "flm.write.failed";
}

/// Condition `flm.write.unknown-output` (Error): an entry of the setting
/// `write.output_names` ([`WriteRenderOptions::output_names`]) names no output of the
/// registry. The stage [`STAGE`] reports it once per such entry, and then writes no output
/// of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownOutput {
    /// The name the entry gives.
    pub name: String,
    /// The names of the registry's outputs, in name order.
    pub outputs: Vec<String>,
}

impl fmt::Display for UnknownOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outputs: Vec<String> = self.outputs.iter().map(|output| format!("`{output}`")).collect();
        write!(
            f,
            "`{}` in the setting `write.output_names` must be the name of an output of the packages in use: {}",
            self.name,
            outputs.join(", ")
        )
    }
}

impl DiagnosticInfo for UnknownOutput {
    const IDENTIFIER: &'static str = "flm.write.unknown-output";
}

/// The options of the feature `write`, set on the render request: see the
/// [module documentation](self).
///
/// # Examples
///
/// ```
/// use flm_main::stdpkg::write::WriteRenderOptions;
///
/// let options: WriteRenderOptions =
///     serde_json::from_value(serde_json::json!({"output_names": {"bibtex": "refs.bib"}})).unwrap();
/// assert_eq!(options.output_names["bibtex"], "refs.bib");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct WriteRenderOptions {
    /// The path of the main file of each output named here, by the output's name, relative
    /// to the output folder and taken exactly as written: `refs` names a file `refs`, with
    /// no extension added. A folder part is allowed (`exports/refs.bib`). The other files
    /// of the output keep the names that the command gives them. No entry by default.
    pub output_names: BTreeMap<String, String>,
}

/// Returns the declaration of the feature's settings: the fields of
/// [`WriteRenderOptions`] are its keys; `output_names` is combined entry by entry, one
/// entry per output, with no default entry.
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .options::<WriteRenderOptions>(
            Bind::Render,
            "The name of the main file of some outputs (`output_names`): a map from an \
             output's name to the path of that output's main file, relative to the output \
             folder and taken exactly as written. The other files of the output keep the \
             names the command gives them. Set per render request.",
        )
        .merge_entries("output_names", || Ok(serde_json::Value::Object(serde_json::Map::new())))
}

/// The feature `write`, holding the naming scheme and the sink that the host made for one
/// command: see the [module documentation](self).
pub struct WriteFeature {
    namer: Arc<dyn ArtifactNamer>,
    sink: Option<Arc<dyn Sink>>,
}

impl WriteFeature {
    /// Returns the feature that gives the artifacts their paths through `namer` and hands
    /// the files to `sink`. With no sink, the feature contributes the naming scheme alone,
    /// and its stage reports [`WriteFailed`] for every output that a plan demands.
    pub fn new(namer: Arc<dyn ArtifactNamer>, sink: Option<Arc<dyn Sink>>) -> WriteFeature {
        WriteFeature { namer, sink }
    }
}

impl FlmFeature for WriteFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Writing the output")
            .docs(
                "the stage that hands the files of every output the plan demands to the destination \
                 the program gives: the disk, memory or the standard output",
            )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.service_slot::<dyn Sink>(
            SINK_SLOT,
            Arity::Single,
            "the destination of an output's files once each file has its path",
        )?;
        if let Some(sink) = &self.sink {
            b.contribute_to::<dyn Sink>(SINK_SLOT, Arc::clone(sink))?;
        }
        b.contribute_to::<dyn ArtifactNamer>(NAMER_SLOT, Arc::clone(&self.namer))?;
        b.capability(
            CapabilityDef::new(WRITTEN)
                .docs("every output the plan demands was handed to the destination the program gives"),
        )?;
        b.stage(Arc::new(WriteStage))
    }
}

/// The stage `flm.write.files`: see the [module documentation](self).
struct WriteStage;

/// Returns every output of `registry` whose capability is among `demands`, with its
/// artifact-set table, in the registry's order.
fn demanded_outputs<'r>(
    registry: &'r flm_core::core::registry::FlmRegistry,
    demands: &std::collections::BTreeSet<flm_core::model::names::CapName>,
) -> Vec<(&'r RegisteredOutput, TableHandle<ArtifactSet>)> {
    registry
        .outputs()
        .filter(|output| demands.contains(&output.def.capability))
        .map(|output| (output, TableHandle::new(output.def.table.clone())))
        .collect()
}

/// Returns, for each path that two files of `files` hold with different bytes or media
/// types, the output of the earlier file, the output of the later one, and the path; once
/// per path.
fn collisions(files: &[NamedFile]) -> Vec<(&str, &str, &str)> {
    let mut found: Vec<(&str, &str, &str)> = Vec::new();
    for (index, later) in files.iter().enumerate() {
        let earlier = files[..index]
            .iter()
            .find(|earlier| earlier.path == later.path && (earlier.bytes != later.bytes || earlier.media_type != later.media_type));
        if let Some(earlier) = earlier
            && !found.iter().any(|(_, _, path)| *path == later.path)
        {
            found.push((&earlier.output, &later.output, &later.path));
        }
    }
    found
}

/// Returns an empty span whose origin is the stage, for a problem that has no place in
/// the document.
fn nowhere() -> FlmSpan {
    let source = Arc::new(Source::new("").with_origin(FlmOrigin::synthetic(STAGE)));
    SourceSpan::new(&source, 0..0)
}

impl Stage for WriteStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(STAGE, Effect::Io, Scope::Document)
            .provides(WRITTEN)
            .docs("hands the files of every output the plan demands to the destination the program gives")
    }

    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let mut needs = Needs::new().option::<WriteRenderOptions>();
        for (_, table) in demanded_outputs(cx.registry(), cx.demands()) {
            needs = needs.reads(table);
        }
        needs
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let services = cx.services_all();
        let namer = services.service_named::<dyn ArtifactNamer>(NAMER_SLOT);
        let sink = services.service_named::<dyn Sink>(SINK_SLOT);
        let scope = NamingScope::of(cx.registry(), cx.plan().demands());
        let failed = |output: &str, detail: String| {
            Diagnostic::error(WriteFailed { output: output.to_string(), detail }, nowhere())
        };
        let mut files: Vec<NamedFile> = Vec::new();
        let reported = out.diags.len();
        let options = cx.option::<WriteRenderOptions>();
        if let Ok(options) = options {
            let outputs: Vec<String> = cx.registry().outputs().map(|output| output.def.name.to_string()).collect();
            for name in options.output_names.keys().filter(|name| !outputs.contains(name)) {
                let unknown = UnknownOutput { name: name.clone(), outputs: outputs.clone() };
                out.diags.push(Diagnostic::error(unknown, nowhere()));
            }
        }
        let demanded = demanded_outputs(cx.registry(), cx.plan().demands());
        if demanded.is_empty() {
            return;
        }
        for (entry, table) in demanded {
            let output = entry.def.name.to_string();
            let failed = |detail: String| failed(&output, detail);
            let (Some(namer), Some(_)) = (namer, sink) else {
                out.diags.push(failed(String::from("the program gives no destination for its files")));
                continue;
            };
            let options = match &options {
                Ok(options) => options,
                Err(error) => {
                    out.diags.push(failed(format!("the setting `write.output_names` cannot be read: {error}")));
                    continue;
                }
            };
            let set = match cx.get(&table) {
                Ok(Some(set)) => set,
                Ok(None) => {
                    out.diags.push(failed(String::from("none of its files was made")));
                    continue;
                }
                Err(error) => {
                    out.diags.push(failed(error.to_string()));
                    continue;
                }
            };
            match place(entry, set, &EntryNames::new(&options.output_names, namer), &scope) {
                Ok(placed) => files.extend(placed),
                Err(errors) => {
                    for error in errors {
                        out.diags.push(failed(error.detail));
                    }
                }
            }
        }
        for (earlier, later, path) in collisions(&files) {
            let detail = format!("the output `{earlier}` and the output `{later}` must not write different content at one path, `{path}`");
            out.diags.push(failed(later, detail));
        }
        if out.diags.len() > reported {
            return;
        }
        let Some(sink) = sink else { return };
        if let Err(errors) = sink.receive(&files) {
            for error in errors {
                // A problem with no file of its own is the first output's.
                let file = error.path.as_deref().and_then(|path| files.iter().find(|file| file.path == path));
                let output = file.or(files.first()).map_or("", |file| file.output.as_str());
                out.diags.push(failed(output, error.detail));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Naming;
    use crate::stdpkg::{self, StdParts};
    use flm_core::core::plan::{Overrides, Plan, workflow_request};
    use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::model::doc::{Doc, DocContent};
    use flm_core::model::names::CapName;
    use flm_core::model::options::OptionValues;
    use flm_core::techy::error::Severity;

    #[test]
    fn a_demanded_output_with_no_sink_to_receive_it_is_an_error() {
        let parts = StdParts::new(Arc::new(Naming::Folder));
        let stdmain = stdpkg::stdmain_factory(parts).unwrap();
        let stdcore = flm_core::stdcore::factory().unwrap();
        let mut builder = RegistryBuilder::new();
        let processor = flm_core::math_passthrough::factory().unwrap();
        for factory in [&stdcore, &stdmain, &processor] {
            builder.use_package(factory.build(&Default::default()).unwrap());
        }
        let registry = builder.build(&RegistryInputs::new()).unwrap();
        let mut request = workflow_request(&registry, "text", &Overrides::none()).unwrap();
        request.demands.push(CapName::from(WRITTEN));
        // Two stages assemble the text: the library's fixed skeleton and the composer.
        request.prefer.push(flm_core::stdcore::features::compose::stage_name("text").into());
        let plan = Arc::new(Plan::build(&registry, &request).unwrap());
        let doc = Arc::new(Doc::with_options(DocContent::new(), OptionValues::new()));
        let mut run = Run::new(doc, plan, Externals::none()).unwrap();
        run.execute(&RunOptions::new()).unwrap();
        let failed: Vec<_> = run
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.identifier() == WriteFailed::IDENTIFIER)
            .collect();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].severity(), Severity::Error);
    }

    #[test]
    fn an_unknown_output_names_the_setting_and_lists_the_outputs() {
        let unknown = UnknownOutput { name: String::from("foo"), outputs: vec![String::from("html"), String::from("text")] };
        assert_eq!(
            unknown.to_string(),
            "`foo` in the setting `write.output_names` must be the name of an output of the packages in use: `html`, `text`"
        );
    }

    #[test]
    fn the_setting_is_declared_render_bound_and_merged_entry_by_entry() {
        let decl = settings();
        assert_eq!(decl.key("output_names").map(|key| key.bind), Some(Bind::Render));
        assert!(decl.mark_at("output_names").is_some_and(|mark| mark.merge_entries));
    }

    fn file(path: &str, output: &str, bytes: &[u8]) -> NamedFile {
        NamedFile {
            path: path.to_string(),
            output: output.to_string(),
            entry: false,
            media_type: String::from("text/css"),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn equal_files_at_one_path_are_one_file_and_different_ones_collide_once() {
        let shared = [file("p-a.css", "html", b"a"), file("p-a.css", "text", b"a")];
        assert!(collisions(&shared).is_empty());
        let differing = [
            file("p-a.css", "html", b"a"),
            file("p-a.css", "text", b"b"),
            file("p-a.css", "bibtex", b"c"),
            file("p-b.css", "html", b"b"),
        ];
        assert_eq!(collisions(&differing), [("html", "text", "p-a.css")]);
        let mut typed = file("p-a.css", "text", b"a");
        typed.media_type = String::from("text/plain");
        assert_eq!(collisions(&[file("p-a.css", "html", b"a"), typed]), [("html", "text", "p-a.css")]);
    }
}
