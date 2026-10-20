//! The extension `rows`, written against the guest client `flm_ext::guest` and
//! compiled into the test program of `flm-ext/tests/native_handlers.rs`: one
//! package per group of the handler rows that `flm-ext/tests/wasm_engine.rs`
//! drives through scripted WebAssembly guests and that case 1 and the stub do
//! not reach (case 1 and the stub cover the stages, the expansion hooks, the
//! lowering rules, `define`, `packages`, `register` and `rehearse`), and one
//! package for the scenarios the guest client's own tests answer with a test
//! hook in place of the host (`src/guest/registry.rs`, and the helper's
//! declaration of `tests/guest_forms.rs`).
//!
//! - `rows#box`: `\rowsbox{content}\label{…}` (the element `rows.box`), lowered
//!   to its content by a rule body of its own, carrying a `Referenceable` and a
//!   `Sectioning` (level 2, the first argument as the title) attached in the
//!   element's declaration; `\rowspart{title}` (the element `rows.part`), whose
//!   `Sectioning` (level 3) `attach` adds.
//! - `rows#rewrite`: the rewrite stage `rows.respell`, publishing
//!   `rows.respelled`, which replaces `colour` in characters by `color`.
//! - `rows#cite`: the citation provider `rows.bib`, contributed to the slot of
//!   the providers, which answers every key that it holds no entry.
//! - `rows#template`: the template module `tiny` of the composer: its
//!   description (for the HTML output), one file it ships, the page it renders.
//! - `rows#hooks`: a late-initialization hook pushing a warning, and a
//!   finalizer declaring the table `rows.late` and the stage `rows.fill`.
//! - `rows#forms`: a helper's declaration whose `docs` and `modes` the host
//!   answers (`\rowsgreet`, math mode only); a fallback and a scoped package
//!   whose elements carry lent aspects; a value contributed first to a slot of
//!   the feature's own, which a stage reads back.
//! - `rows#backend`: the backend `rows`, its lowering the library's and its
//!   emit and final stages its own, with the output `rows`, and the rule that
//!   lowers an image on it (as the JavaScript fixture `tests/js/typst/` does).

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::artifact::{Artifact, ArtifactId};
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::data::table::TableValue;
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_ext::guest::core::registry::{
    BackendDef, BackendStage, Contributed, FeatureBuilder, Finalizer, LateInit, OutputDef, StandardTable,
};
use flm_ext::guest::core::run::{RewriteCx, StageCx};
use flm_ext::guest::core::services::Arity;
use flm_ext::guest::core::stage::{Effect, Needs, RewriteDecl, RewriteStage, Scope, Stage, StageDecl};
use flm_ext::guest::lang::mint::{ArgDef, ElementDef, FallbackDef, FallbackKind, Mode, ScopedPackageDef, SyntaxDef};
use flm_ext::guest::lang::rewrites::userdefs::{Definition, UserDefinition};
use flm_ext::guest::lang::structural::{COMMENT, CONTAINER, GROUP, PARBREAK, TEXT};
use flm_ext::guest::lang::{BuiltLang, FlmDiagnostics};
use flm_ext::guest::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, ContentRef, RefPoint, Referenceable, Sectioning,
};
use flm_ext::guest::model::element::{Aspect, ElementSpec};
use flm_ext::guest::stdcore::backends::lowering::{LoweringStage, lowered_table_name};
use flm_ext::guest::stdcore::backends::pres::{ALL, UNSUPPORTED};
use flm_ext::guest::stdcore::features::citations::{
    CitationProvider, CitationProviderError, CitationRequest, CitationResult, ProviderCx,
};
use flm_ext::guest::stdcore::features::compose::{
    Accepts, TemplateContext, TemplateCx, TemplateDescription, TemplateFile, TemplateModule,
};
use flm_ext::guest::stdcore::features::refs::{label_fields_arg, label_uses, point_labels};
use flm_ext::guest::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::Node;
use flm_ext::guest::techy::transform::{Restage, Restager};
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Map, Value, json};

/// The table the rewrite stage publishes.
pub const RESPELLED: &str = "rows.respelled";

/// The table the finalizer declares.
pub const LATE: &str = "rows.late";

/// The table the stage reading the feature's own slot writes.
pub const READ: &str = "rows.read";

/// The backend's artifact set, also the output's table.
pub const DOCUMENT: &str = "rows.document";

/// What the citation provider answers for every key.
pub const NO_ENTRY: &str = "the guest holds no entry";

flm_ext::guest::extension!(extension);

fn extension(ext: &mut Extension) {
    let package = |feature: &str, register: fn(&mut FeatureBuilder, &Value) -> Result<(), ExtError>| {
        Package::new().uses(["stdcore"]).feature(feature, Feature::new().title(feature).register(register))
    };
    ext.package("box", package("box", register_box));
    ext.package("rewrite", package("rewrite", register_rewrite));
    ext.package("cite", package("cite", register_cite));
    ext.package("template", package("template", register_template));
    ext.package("hooks", package("hooks", register_hooks));
    ext.package("forms", package("forms", register_forms));
    ext.package("backend", package("backend", register_backend));
}

// --- rows#box: a rule's body, `points` and the sectioning ------------------------------------------

/// The rule's body of `\rowsbox` and `\rowspart`: the first argument's content.
struct FirstArgument;

impl Lower for FirstArgument {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        Ok(Lowering::Emit(lw.lower_arg(cx, node, 0)?))
    }
}

/// The reference point of `\rowsbox`: its labels, its first argument shown.
struct Points;

impl Referenceable for Points {
    fn points(&self, node: &Node) -> Result<Vec<RefPoint>, ExtError> {
        let (labels, malformed) = point_labels(&label_uses(node)?)?;
        Ok(vec![RefPoint {
            kind: "rowsbox".to_string(),
            labels,
            computed: 0,
            malformed,
            anchor: AnchorPolicy::FromFirstLabel,
            number: None,
            content: Some(ContentRef::Argument(0)),
            accepts: AcceptedRefTypes::Any,
        }])
    }
}

/// A section of level `level`; with `titled`, its first argument is its title in the
/// table of contents (otherwise the trait's default, no title).
struct Section {
    level: u32,
    titled: bool,
}

impl Sectioning for Section {
    fn level(&self, _: &Node) -> Result<u32, ExtError> {
        Ok(self.level)
    }

    fn toc_title(&self, _: &Node) -> Result<Option<ContentRef>, ExtError> {
        Ok(self.titled.then_some(ContentRef::Argument(0)))
    }
}

/// Declares the macro `name` of the arguments `args`, the element `spec`, lowered to its
/// first argument by a rule body of its own.
fn declare_lowered(b: &mut FeatureBuilder, owner: &str, name: &str, args: Vec<ArgDef>, spec: ElementSpec) -> Result<(), ExtError> {
    let element = spec.name.clone();
    b.element(ElementDef::new(owner, [name], SyntaxDef::Macro { args }, spec))?;
    let rule = format!("{element}.lower.default");
    b.lowering_rule(LoweringRule::new(rule, &element, LoweringDecl::new(), Arc::new(FirstArgument), DefaultMark::Everywhere)?)
}

fn register_box(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    let spec = ElementSpec::new("rows.box")
        .with(Aspect::Referenceable(Arc::new(Points)))?
        .with(Aspect::Sectioning(Arc::new(Section { level: 2, titled: true })))?;
    declare_lowered(b, &owner, "rowsbox", vec![ArgDef::mandatory("content"), label_fields_arg()?], spec)?;
    declare_lowered(b, &owner, "rowspart", vec![ArgDef::mandatory("title")], ElementSpec::new("rows.part"))?;
    b.attach("rows.part", Aspect::Sectioning(Arc::new(Section { level: 3, titled: false })))
}

// --- rows#rewrite: a rewrite stage -------------------------------------------------------------------

/// Replaces `colour` in characters by `color`.
struct Respell;

impl RewriteStage for Respell {
    fn decl(&self) -> RewriteDecl {
        RewriteDecl::new("rows.respell", RESPELLED).docs("Replaces `colour` by `color`.")
    }

    fn needs(&self) -> Needs {
        Needs::new().after("flm.userdefs").before("flm.blocks.structured")
    }

    fn restage(&self, node: &Node, _: &RewriteCx, rs: &mut Restager) -> Result<Restage, ExtError> {
        if node.is_chars()? {
            let text = node.chars()?.unwrap_or_default();
            if text.contains("colour") {
                return Ok(Restage::Emit(vec![rs.stage_chars(&text.replace("colour", "color"), node)?]));
            }
        }
        Ok(Restage::Descend)
    }
}

fn register_rewrite(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    b.rewrite_stage(Arc::new(Respell))
}

// --- rows#cite: a citation provider ------------------------------------------------------------------

/// Answers every key that it holds no entry; its `answer_all` is the trait's default,
/// which runs in the guest.
struct Bib;

impl CitationProvider for Bib {
    fn name(&self) -> String {
        "rows.bib".to_string()
    }

    fn answer(&self, _: &CitationRequest, _: &ProviderCx) -> Result<CitationResult, ExtError> {
        Ok(Err(CitationProviderError::new(NO_ENTRY)))
    }
}

fn register_cite(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    b.contribute_to("flm.citations.providers", Contributed::CitationProvider(Arc::new(Bib)))
}

// --- rows#template: a template module ----------------------------------------------------------------

/// The template `tiny`: for the HTML output, shipping one style file, rendering a fixed
/// page.
struct Tiny;

impl TemplateModule for Tiny {
    fn name(&self) -> String {
        "tiny".to_string()
    }

    fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
        Ok(TemplateDescription { output: "html".to_string(), accepts: Accepts::All, requires: Vec::new(), config: Map::new() })
    }

    fn files(&self, _: &TemplateContext, _: &TemplateCx) -> Result<Vec<TemplateFile>, ExtError> {
        Ok(vec![TemplateFile::Content {
            name: "style".to_string(),
            extension: "css".to_string(),
            media_type: "text/css".to_string(),
            text: "h1 {}\n".to_string(),
        }])
    }

    fn render(&self, data: &TemplateContext, _: &TemplateCx) -> Result<String, ExtError> {
        let style = data.artifacts.iter().find(|(name, _)| name == "style").map(|(_, path)| path.as_str()).unwrap_or_default();
        Ok(format!("<html>tiny {style}</html>\n"))
    }
}

fn register_template(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    b.peer_slot("flm.compose.templates", Contributed::TemplateModule(Arc::new(Tiny)))
}

// --- rows#hooks: a late-initialization hook and a finalizer -----------------------------------------

/// Pushes a warning once the language is built.
struct Late;

impl LateInit for Late {
    fn late_init(&self, _: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), ExtError> {
        diags.push(Diagnostic::warning("rows.late", "the language is built"))
    }
}

/// Declares the table `rows.late` and the stage `rows.fill`, in the second round of the
/// registration.
struct Finalize;

impl Finalizer for Finalize {
    fn finalize(&self, b: &mut FeatureBuilder) -> Result<(), ExtError> {
        b.table(LATE, "the second round's table")?;
        b.stage(Arc::new(Push { name: "rows.fill", table: LATE, item: json!({ "filled": true }) }))
    }
}

/// A stage of the document named `name` pushing `item` into `table`.
struct Push {
    name: &'static str,
    table: &'static str,
    item: Value,
}

impl Stage for Push {
    fn decl(&self) -> StageDecl {
        StageDecl::new(self.name, Effect::Compute, Scope::Document).writes(self.table)
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, _: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        out.table(self.table)?.push(self.item.clone())
    }
}

fn register_hooks(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    b.late_init(Arc::new(Late))?;
    b.finalizer(Arc::new(Finalize))
}

// --- rows#backend: a backend of the extension's own stages -------------------------------------------

/// The emit stage of the backend `rows`: one item after the lowering, saying whether it
/// read the lowered trees (the table `lowered`, the library's name for them).
struct Emit {
    lowered: String,
}

impl Stage for Emit {
    fn decl(&self) -> StageDecl {
        StageDecl::new("rows.emit", Effect::Compute, Scope::Document).writes("rows.output")
    }

    fn needs(&self) -> Needs {
        Needs::new().reads(self.lowered.as_str())
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let read = matches!(cx.get(&self.lowered)?, Some(TableValue::Lowered(_)));
        out.table("rows.output")?.push(json!(format!("emitted, the lowered trees read: {read}")))
    }
}

/// The final stage of the backend `rows`: the emitted text as the entry artifact.
struct Final;

impl Stage for Final {
    fn decl(&self) -> StageDecl {
        StageDecl::new("rows.final", Effect::Compute, Scope::Document).writes(DOCUMENT)
    }

    fn needs(&self) -> Needs {
        Needs::new().reads("rows.output")
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let Some(TableValue::Ext(table)) = cx.get("rows.output")? else { return Ok(()) };
        let text = table.items()?.first().and_then(Value::as_str).unwrap_or_default().to_string();
        let artifact = Artifact::bytes(ArtifactId::new("rows", "document"), "text/plain", text.into_bytes()).as_entry();
        out.table(DOCUMENT)?.push(flm_ext::adapter::values::JsonForm::to_value(&artifact))
    }
}

/// The rule's body that lowers an image on the backend `rows`: the stand-in.
struct NoImage;

impl Lower for NoImage {
    fn lower(&self, _: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let payload = json!({ "element": IMAGE, "message": "the backend `rows` writes no image" });
        Ok(Lowering::Emit(vec![lw.pres(cx, UNSUPPORTED, &payload, vec![])?]))
    }
}

/// The element of an image, whose library rule serves the library's backends alone.
const IMAGE: &str = "flm.graphics.image";

fn register_backend(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    let lowered = lowered_table_name("rows")?;
    b.table_named(&lowered, StandardTable::LoweredTrees, "the backend's lowered trees")?;
    b.table("rows.output", "the emitted text, one item")?;
    b.table_named(DOCUMENT, StandardTable::ArtifactSet, "the backend's one artifact")?;
    let native: Vec<String> = ALL.iter().chain(&[TEXT, GROUP, COMMENT, CONTAINER, PARBREAK]).map(|name| name.to_string()).collect();
    let backend = BackendDef::new(
        "rows",
        native,
        BackendStage::Library(LoweringStage::new("rows")?),
        BackendStage::Own(Arc::new(Emit { lowered })),
        BackendStage::Own(Arc::new(Final)),
    )?;
    b.backend(backend.docs("A backend of its own stages."))?;
    b.output(OutputDef::new("rows", DOCUMENT, DOCUMENT, "text/plain", "txt").docs("the emitted text"))?;
    let decl = LoweringDecl::new().produces(UNSUPPORTED);
    b.lowering_rule(LoweringRule::new("rows.image.lower.rows", IMAGE, decl, Arc::new(NoImage), DefaultMark::OnBackends(vec!["rows".into()]))?)
}

// --- rows#forms: the values holding handler objects, against the host --------------------------------

/// Reads the values of the slot `rows.data` and pushes them.
struct ReadSlot;

impl Stage for ReadSlot {
    fn decl(&self) -> StageDecl {
        StageDecl::new("rows.read", Effect::Compute, Scope::Document).writes(READ)
    }

    fn needs(&self) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        out.table(READ)?.push(json!({ "values": cx.services("rows.data")? }))
    }
}

fn register_forms(b: &mut FeatureBuilder, _: &Value) -> Result<(), ExtError> {
    let owner = b.feature()?;
    // A helper's declaration, its documentation and its modes sent to the host.
    let greet = Definition::macro_("rowsgreet", vec![]).content("Hello");
    let element = element_name_under("rows", &greet)?;
    let declared = UserDefinition::new(greet.clone())?.element_def(&owner, &element)?.docs("Greets.")?.modes([Mode::Math])?;
    b.element(declared)?;
    b.lowering_rule(expand_in_tree_rule_for(&element, &greet)?)?;
    // A fallback and a scoped package, whose elements carry lent aspects.
    let loose = ElementSpec::new("rows.loose").with(Aspect::Referenceable(Arc::new(Points)))?;
    b.fallback(FallbackDef::new([Mode::Text], FallbackKind::Environment, loose))?;
    let item = ElementSpec::new("rows.item").with(Aspect::Sectioning(Arc::new(Section { level: 4, titled: false })))?;
    let item = ElementDef::new(&owner, ["rowsitem"], SyntaxDef::Macro { args: vec![] }, item);
    b.scoped_package(ScopedPackageDef::new("rows.scoped", vec![item], "The items of nothing in particular."))?;
    // A value contributed first to a slot of the feature's own, read back by a stage.
    b.service_slot("rows.data", Arity::OrderedMany, "values for the stage `rows.read`")?;
    b.contribute_first_to("rows.data", Contributed::Data(json!({ "k": [1, 2] })))?;
    b.table(READ, "the values of the slot `rows.data`")?;
    b.stage(Arc::new(ReadSlot))
}
