//! The by-name dispatcher: the methods of the extension API's façades, and its
//! free functions, reached by name with JSON arguments.
//!
//! The typed traits of [`api`](crate::api) are the extension API; a binding
//! whose values cross as JSON reaches the same methods through [`call`], one
//! glue function for every method. The dispatcher carries no logic of its own:
//! per handle kind, a `match` over the method names (the Rust names verbatim),
//! each arm converting the arguments ([`values`](crate::adapter::values)),
//! calling the typed method and converting the result back, lending any object
//! it answers into the call's table.
//!
//! The arms are generated from the sources under `src/api/` into the crate's
//! module `adapter::generated` (by the test `tests/api_surface.rs`); this
//! module holds the entry, the kind check, and the arms the generator cannot
//! express, each with its reason (the module `extra`).
//!
//! # The methods, by handle kind
//!
//! | Kind | Methods |
//! |---|---|
//! | `feature_builder` | `feature`, `element`, `fallback`, `override_callable`, `table`, `table_refining`, `definition_text_table`, `table_named`, `presentation_element`, `backend`, `output`, `workflow_lowering`, `stage`, `capability`, `lowering_rule`, `peer_table`, `peer_backend`, `peer_element`, `style_def`, `script_def`, `rewrite_stage`, `require_service`, `service_slot`, `contribute_to`, `contribute_first_to`, `peer_slot`, `digest_input`, `attach`, `collection_member`, `scoped_package`, `argument_role`, `peer_argument_role`, `late_init`, `finalizer` |
//! | `stage_cx` | `scope`, `tree_id`, `tree`, `frozen_tree`, `my_tree`, `trees`, `walk`, `addr_of`, `get`, `option`, `service`, `services`, `host` |
//! | `host_service` | `name`, `call` |
//! | `files` | `read_text` |
//! | `provider_cx` | `parse_entry`, `build_entry`, `host` |
//! | `template_cx` | `host` |
//! | `stage_out` | `table`, `diags` |
//! | `table_piece` | `push` |
//! | `ext_table_read` | `items` |
//! | `trees_table` | `trees`, `tree` |
//! | `lowered_trees` | `trees`, `tree`, `requires`, `rounds` |
//! | `node` | every method of [`Node`](crate::api::techy::node::Node) |
//! | `key_vals` | `len`, `is_empty`, `keyval`, `get`, `iter` |
//! | `key_val_entry` | `key`, `value`, `value_content` |
//! | `expand_cx` | `stage_chars`, `definition` |
//! | `expander` | `depth`, `recursion_limit`, `lang`, `instantiate`, `recover`, `diagnose` |
//! | `user_definition` | `definition`, `element_def`, `element_def_with`, `variant_for_use` |
//! | `mode_variant` | `template` |
//! | `element_def` | `docs`, `modes` (a declaration a helper made; each answers a new one) |
//! | `lowering_rule` | `docs` (answers a new rule) |
//! | `lowerer` | `rule`, `lang`, `backend`, `comment_start`, `tree_id`, `addr`, `addr_of`, `placement`, `get`, `get_at`, `producer_of`, `option`, `lower_children`, `lower_body`, `lower_arg`, `lower_arg_named`, `lower_nodes`, `default`, `pres`, `pres_slots`, `text`, `splice`, `require`, `diagnose` (every method of [`Lowerer`](crate::api::core::lowering::Lowerer) but `lower_region`, whose address cannot be read back from data) |
//! | `rewrite_cx` | `tree_id`, `layout`, `option` |
//! | `restager` | `restage_subtree`, `restage_children`, `restage_argument`, `restage_argument_named`, `restage_slot`, `restage_invocation`, `restage_argument_with_content`, `restage_slot_with_content`, `stage_chars`, `diagnose` |
//! | `restaged_argument` | `is_provided` |
//! | `restaged_slot` | `name` |
//! | `compiled_template`, `placeholder_bindings`, `staged`, `stage`, `expand`, `lower`, `lower_cx`, `library_stage`, `rewrite_stage`, `citation_entry`, `citation_provider`, `template_module`, `referenceable`, `sectioning`, `arg_def`, `built_lang`, `late_init`, `finalizer`, `template_language` | none: they are passed back as arguments |
//! | `flm_diagnostics` | `push` |
//! | `counters` | `record`, `format` |
//! | `anchors` | `id` |
//! | `value_bindings` | `with` (answers new bindings) |
//!
//! | Module | Functions |
//! |---|---|
//! | `core/artifact` | `media_type_of` |
//! | `lang/mint` | `arg_def_from_code` |
//! | `lang/text` | `chars_text`, `read_chars` |
//! | `core/lowering` | `chars_at_lowering`, `spelling_at_lowering`, `LoweringRule::new`, `pres_tree` |
//! | `techy/extract` | `parse_keyval_drop_annotations` |
//! | `lang/rewrites/blocks` | `argument_holds_blocks` |
//! | `lang/rewrites/userdefs` | `UserDefinition::new`, `DefinitionBindings::new`, `TemplateLanguage::new`, `compile_template_with`, `ValueBindings::new`, `instantiate` |
//! | `stdcore/features/userdefs` | `element_name_under`, `expand_in_tree_rule_for`, `write_math_run` |
//! | `lang/structural` | `names` |
//! | `stdcore/backends/html` | `names` |
//! | `stdcore/backends/lowering` | `LoweringStage::new`, `lowered_table_name` |
//! | `stdcore/backends/pres` | `names` |
//! | `stdcore/features/math` | `names`, `equation_lines`, `formula_display`, `environment_numbered`, `line_frames`, `number_text`, `stage_tag_content`, `environment_opening`, `line_closing`, `stage_lines`, `normalize_line_newlines`, `environment_closing`, `group_opening`, `group_closing` |
//! | `stdcore/features/refs` | `names`, `label_fields_arg`, `label_uses`, `point_labels`, `stage_label`, `stage_label_group` |
//!
//! # The value shapes
//!
//! Arguments and results are the shapes of plan A, section 4.5: a handle is
//! `{"@handle": {"kind", "call", "index"}}`; `Option<T>` is `T` or `null`; a
//! tuple is the list of its elements (`trees()` answers `[[id, node], …]`,
//! `group_delimiters()` `[open, close]`); `&[u8]` is a string whose UTF-8
//! bytes are taken; a method answering nothing answers `null`. No number may
//! be a floating-point one, no map key may begin with `$`, and a data map may
//! not hold the key `@handle` or `@durable`, in either direction ([`ExtError::Value`]).
//!
//! A table read by name answers `{"ext": <handle>}`, `{"trees": <handle>}`,
//! `{"lowered": <handle>}` (a handle of kind `lowered_trees`) or
//! `{"projection": <value>}`; a table of an output's files answers the
//! projection that
//! [`artifact_set_to_value`](crate::adapter::values::artifact_set_to_value)
//! writes. An artifact pushed into a table of an output's files is the data
//! form of [`artifact_from_value`](crate::adapter::values::artifact_from_value).
//!
//! The host services' shapes: `host` takes `[name]` and answers a handle of
//! kind `files` (the `files` service) or `host_service` (any other);
//! `read_text` takes `[scope, path]`, the scope `"extension"` or
//! `"document"`, and answers a text or `null`; `call` takes `[method, args]`
//! and answers data.
//!
//! The citation providers' shapes: `parse_entry` takes `[source, origin]`
//! and `build_entry` `[[part, …]]`, each part `{"text": …}`, `{"source": …}`,
//! `{"emphasized": [parts]}`, `{"strong": [parts]}` or `{"link":
//! {"address", "content": [parts]}}`; both answer a handle of kind
//! `citation_entry`. A provider is handed its requests as `[{"key"}, …]`
//! ([`citation_requests_to_value`](crate::adapter::values::citation_requests_to_value))
//! and answers one item per request, `null`, `{"error": text}` or
//! `{"entry", "identity", "warnings"}`
//! ([`citation_results_from_value`](crate::adapter::values::citation_results_from_value)).
//! A template module's `describe` answers `{"output", "accepts",
//! "requires", "config"}`
//! ([`template_description_from_value`](crate::adapter::values::template_description_from_value)),
//! its `files` a list of `{"name", "extension", "media_type", "text"}` or
//! `{"name", "file", "scope", "media_type"}`
//! ([`template_files_from_value`](crate::adapter::values::template_files_from_value)),
//! and both `files` and `render` receive the data of
//! [`template_context_to_value`](crate::adapter::values::template_context_to_value).
//!
//! The lowering façade's own shapes: an address is `{"tree", "node"}`
//! (answered, never read back); a staged node is a handle of kind `staged`;
//! `get_at` takes `[name, node]` and answers the items of the extension's
//! table `name` at that node ([`ExtTableRead::items_at`](crate::api::core::data::table::ExtTableRead::items_at),
//! no `flm-core` counterpart), or `null` when the table is absent;
//! `pres_slots` takes its slots as `[[name, [staged, …]], …]`;
//! `LoweringRule::new` takes `[name, element, decl, body, mark]`, the
//! declaration as `{"requires", "reads", "reads_opt", "options", "produces",
//! "selection_requires"}` (lists of names, each optional), the body a handle
//! of kind `lower`, the mark `"everywhere"`, `"never"` or `{"on_backends":
//! [...]}`; `pres_tree` takes `[lowerer, cx, tree]`, the tree as
//! [`pres_tree_from_value`](crate::adapter::values::pres_tree_from_value) reads it.
//!
//! The durable objects (a template language, a compiled template; see
//! [`durable`](crate::adapter::durable)): answered as a durable token `{"@durable": {"kind", "table",
//! "index"}}`, the kind `template_language` or `compiled_template`, plain data
//! a handler may keep beyond its call; passed back as an argument, a token is
//! resolved against the durable table of the running handler's feature and
//! lent into the call as the object it names (a token of another feature's
//! table, of another registry's, or naming no entry is
//! [`ExtError::StaleDurable`]). `lang` answers a handle of kind `built_lang`,
//! recorded as the lowerer or the expander that answered it.
//! `compile_template_with` takes `[spec, lang, language]`, the specification
//! as [`template_spec_from_value`](crate::adapter::values::template_spec_from_value)
//! reads it; `instantiate` takes `[site, template, bindings]`, the bindings a
//! handle of kind `placeholder_bindings` or `value_bindings`, and answers the
//! root node of the new tree; `ValueBindings.with` takes `[name, node]`, the
//! node the root of a tree.
//!
//! The data-valued slots and the numbering's tables: `contribute_to`,
//! `contribute_first_to` and `peer_slot` given one of the standard
//! data-valued slots take `[slot, value]`, the value the slot's data form; a
//! point (`Counters.record`, `Counters.format`, `Anchors.id`) is `{"addr":
//! {"tree", "node"}, "index"}`, read back against the trees the reader sees.
//!
//! The registration's own shapes: `service_slot` takes `[name, arity, docs]`,
//! the arity `"single"` or `"ordered-many"`; `contribute_to`,
//! `contribute_first_to` and `peer_slot` take `[slot, value]`, the value data
//! or a handle of kind `citation_provider` or `template_module` (a handler
//! object the binding lent in);
//! `table_refining` takes `[name, parent,
//! docs]`; `table_named` takes `[name, type, docs]`, the type one of
//! `"lowered-trees"`, `"lowered-pieces"`, `"regions"`, `"artifact-set"`;
//! `presentation_element` takes `[name, docs]`; `backend` takes `[{"name",
//! "native": [name, …], "lowering", "emit", "final_stage", "docs"}]`, each
//! stage a handle of kind `library_stage` (`LoweringStage::new` on the module
//! `stdcore/backends/lowering` answers one) or `stage`, `docs` optional
//! ([`backend_def_from_table`](crate::adapter::values::backend_def_from_table));
//! `output` takes `[{"name", "capability", "table", "media_type",
//! "extension", "docs"}]`, `docs` optional; `peer_table`, `peer_backend`, `peer_element` and
//! `definition_text_table` take `[name, docs]`; `style_def` and `script_def`
//! take `[{"id", "content": [part, …], "docs"}]`, each part `{"inline": text}`,
//! `{"file": path}`, `{"url": address}` or `{"table": name}`
//! ([`definition_parts_from_value`](crate::adapter::values::definition_parts_from_value)).
//! `attach` takes `[element, aspect]`, the aspect `{"BlockRoleAspect": role}`,
//! or `{"Expand": h}`, `{"Referenceable": h}`, `{"Sectioning": h}` with a
//! handle of the kind its key names
//! ([`aspect_from_value`](crate::adapter::values::aspect_from_value));
//! `collection_member` takes `[collection, member]`; `argument_role` takes
//! `[name, docs]`; `peer_argument_role` takes `[name]`; `scoped_package` takes
//! `[{"name", "elements": [declaration, …], "docs"}]`, each declaration
//! read as `element` reads one; `late_init` and `finalizer` take a handle of
//! the kind `late_init` or `finalizer`. An argument list takes a handle of
//! the kind `arg_def` in place of a code (`label_fields_arg` answers one).
//! A late-initialization hook's `push` takes a diagnostic in the shape a
//! stage's diagnostics take.
//!
//! The `refs` library's shapes: a label use is `{"text", "span": {"start",
//! "end"}, "label": {"ref_type", "ref_label"} | null}`; `point_labels` takes
//! a list of them and answers `[labels, malformed]`, a malformed label
//! `{"text", "span", "position"}`; `stage_label` takes `[cx, lw, text,
//! label | null]` and answers a staged node; `stage_label_group` takes `[cx,
//! lw, [staged, …], empty_group]` and answers a list of staged nodes. A
//! reference point a script's `Referenceable` answers is read by
//! [`ref_point_from_value`](crate::adapter::aspects::ref_point_from_value).
//!
//! The restage façade's shapes (a rewrite stage's handler): `rewrite_stage`
//! takes `[stage]`, a handle of kind `rewrite_stage` the binding lent in, its
//! declaration `{"name", "publishes", "refines", "default", "docs"}`
//! (`refines` a name or `null`; the last three optional) and its needs as a
//! stage's; the handler answers `"descend"` or `{"emit": [staged, …]}`
//! ([`restage_from_value`](crate::adapter::values::restage_from_value)). The
//! arguments, in order: `restage_subtree` and `restage_children` take
//! `[node]` and answer a list of staged nodes; `restage_argument` and
//! `restage_slot` take `[node, index]`, `restage_argument_named`
//! `[node, name]`, and answer a handle of kind `restaged_argument` or
//! `restaged_slot`; `restage_argument_with_content` and
//! `restage_slot_with_content` take `[node, index, [staged, …]]`;
//! `restage_invocation` takes `[node, [argument, …], [slot, …]]` and answers
//! a staged node, each bundle taken out of the call's table (a bundle given
//! a second time is refused); `stage_chars` takes `[text, like]`, `like` a
//! node; `diagnose` takes a diagnostic as `Expander.diagnose` does;
//! `RewriteCx.option` takes `[name]`; `tree_id` and `layout` take nothing
//! and answer a text (`tree#0`; `blocks`, `inline` or `inline-or-blocks`).
//!
//! The `math` library's shapes: the parts that stage, and `write_math_run`,
//! take `[cx, lw, x]` (the context, the lowerer, then a node, a frame, a
//! list of frames or a list of nodes) and answer a list of staged nodes;
//! `line_frames` takes `[reader, anchors, counters, node]`, `reader` a handle
//! of kind `stage_cx` or `lowerer` and the two flags booleans;
//! `normalize_line_newlines` takes `[content, after_line_end, inner]` and
//! answers `{"text", "trailing"}`; a line is
//! [`equation_line_to_value`](crate::adapter::values::equation_line_to_value)'s shape and a frame
//! [`line_frame_to_value`](crate::adapter::values::line_frame_to_value)'s.

use serde_json::Value;

use crate::adapter::durable::DurableKind;
use crate::adapter::generated;
use crate::adapter::handles::{CallTable, Entry, Handle, HandleKind, MODULE_PATHS};
use crate::adapter::values::{argument, check_arguments, durable_from_value, handle_to_value, is_durable, Args};
use crate::api::lang::rewrites::userdefs::TemplateLanguage;
use crate::runtime::ExtError;

/// Calls `method` on the object `handle` stands for, with `args` (a JSON list,
/// in the method's argument order; a handle argument in its JSON shape), and
/// answers the result as JSON (a new object as a handle lent into `table`).
///
/// For a free function, `handle` is the module's handle and `method` the
/// function's name. For a constructor (`UserDefinition::new`,
/// `DefinitionBindings::new`), `method` is `"Type::new"` on the module's
/// handle.
///
/// # Errors
///
/// [`ExtError::StaleHandle`], [`ExtError::Argument`] (unknown method, wrong
/// kind, bad argument), [`ExtError::Value`] (a value that cannot cross, its
/// text beginning with the method's name: "`push`: a number must be an
/// integer, not `0.5`"), and whatever the façade's method reports.
pub fn call<'c>(table: &mut CallTable<'c>, handle: Handle, method: &str, args: &Value) -> Result<Value, ExtError> {
    dispatch(table, handle, method, args).map_err(|error| match error {
        // An argument error found below the arm (a stale node in a list, a
        // handle of the wrong kind) names the method it was found in.
        ExtError::Argument { method: found, detail } if found.is_empty() => {
            ExtError::Argument { method: method.to_string(), detail }
        }
        // So does a value that cannot cross, as an argument or as an answer.
        ExtError::Value { detail } => ExtError::Value { detail: format!("`{method}`: {detail}") },
        other => other,
    })
}

/// [`call`], before the method is named in its argument errors.
fn dispatch<'c>(table: &mut CallTable<'c>, handle: Handle, method: &str, args: &Value) -> Result<Value, ExtError> {
    check_arguments(args)?;
    let resolved = resolve_durables(table, args)?;
    let args = Args::new(resolved.as_ref().unwrap_or(args))?;
    if handle.kind == HandleKind::Module {
        let path = usize::try_from(handle.index)
            .ok()
            .and_then(|index| MODULE_PATHS.get(index))
            .filter(|_| handle.call == 0)
            .ok_or_else(|| {
                argument(format!(
                    "a module handle must have the call 0 and an index below {}, not the call {} and the index {}",
                    MODULE_PATHS.len(),
                    handle.call,
                    handle.index
                ))
            })?;
        return generated::module(table, path, method, &args);
    }
    let lent = table.entry(handle)?.kind();
    if lent != handle.kind {
        return Err(ExtError::Argument {
            method: String::new(),
            detail: format!(
                "the object at index {} of the running call is of the kind `{}`, not `{}`",
                handle.index,
                lent.name(),
                handle.kind.name()
            ),
        });
    }
    generated::dispatch_kind(table, handle, method, &args)
}

/// The arguments with every durable token among them (`{"@durable": …}`, an
/// argument of the list) resolved against the call's durable table and lent
/// into the call as the object it names, written as that object's handle:
/// every arm then takes a durable object as it takes a per-call one. `None`
/// when the list holds no durable token.
///
/// # Errors
///
/// A token that is not well formed ([`ExtError::Argument`]); a token the
/// call's durable table does not hold ([`ExtError::StaleDurable`]); a token
/// in a call that holds no durable table ([`ExtError::Argument`]).
fn resolve_durables(table: &mut CallTable<'_>, args: &Value) -> Result<Option<Value>, ExtError> {
    let Value::Array(list) = args else { return Ok(None) };
    if !list.iter().any(is_durable) {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        if !is_durable(item) {
            out.push(item.clone());
            continue;
        }
        let token = durable_from_value(item)?;
        let durable = table.durable().cloned().ok_or_else(|| {
            argument(format!(
                "the `{}` object {} can be used only in a call given the builder, a stage's context, the expander, the lowerer or a late-initialization hook's language",
                token.kind.name(),
                token.index
            ))
        })?;
        let entry = match token.kind {
            DurableKind::TemplateLanguage => {
                durable.template_language(token)?;
                Entry::TemplateLanguage(TemplateLanguage::from_token(token))
            }
            DurableKind::CompiledTemplate => Entry::CompiledTemplate(Box::new(durable.compiled_template(token)?)),
        };
        out.push(handle_to_value(table.lend(entry)));
    }
    Ok(Some(Value::Array(out)))
}

/// The error of a method the object's kind does not have.
pub(crate) fn no_method(kind: HandleKind, method: &str) -> ExtError {
    ExtError::Argument {
        method: String::new(),
        detail: format!("an object of the kind `{}` has no method `{method}`", kind.name()),
    }
}

/// The entry of `$handle` in `$table`, of the variant `$variant` (the kind
/// `$kind`), or the function answers the mismatch.
macro_rules! entry {
    ($table:expr, $handle:expr, $variant:ident, $kind:ident) => {
        match $table.entry($handle)? {
            $crate::adapter::handles::Entry::$variant(inner) => inner,
            other => {
                return Err($crate::adapter::values::mismatch($crate::adapter::handles::HandleKind::$kind, other.kind()))
            }
        }
    };
}
pub(crate) use entry;

/// The arms the generator cannot express, each with its reason; the generated
/// kind functions and the generated module function end in these.
pub(crate) mod extra {
    use serde_json::Value;

    use super::no_method;
    use crate::adapter::handles::{CallTable, Entry, Handle, HandleKind, LangEntry, TableEntry};
    use crate::api::lang::rewrites::userdefs;
    use crate::api::lang::BuiltLang;
    use crate::adapter::values::{
        argument, arg_def_to_value, check_data, format_options_from_value, handle_to_value, mismatch, node_from_table, Args,
        FromArg, IntoReply,
    };
    use crate::api::stdcore::features::numbering::{Counters, NumberSegment};
    use crate::api::stdcore::features::refs::Anchors;
    use crate::sealed::Token;
    use crate::api;
    use crate::api::core::data::table::TableValue;
    use crate::api::techy::node::Node;
    use crate::api::stdcore::features::math;
    use crate::runtime::ExtError;

    /// `StageOut::table` answers `&mut dyn TablePiece`, a borrow of the
    /// output, which cannot be lent into the table beside it: the piece is
    /// recorded as its output's handle and the table's name, and asked for
    /// again on every push.
    pub(crate) fn stage_out(table: &mut CallTable<'_>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match method {
            "table" => {
                args.arity(1)?;
                let name = args.text(0)?;
                entry!(table, handle, StageOut, StageOut).table(name)?;
                Ok(crate::adapter::values::handle_to_value(table.lend(Entry::TablePiece { out: handle, name: name.to_string() })))
            }
            _ => Err(no_method(HandleKind::StageOut, method)),
        }
    }

    /// `TablePiece::push`: the piece is resolved anew through its output's
    /// entry on every push, as [`stage_out`] recorded it.
    pub(crate) fn table_piece(table: &mut CallTable<'_>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match method {
            "push" => {
                args.arity(1)?;
                let (out, name) = match table.entry(handle)? {
                    Entry::TablePiece { out, name } => (*out, name.clone()),
                    other => return Err(mismatch(HandleKind::TablePiece, other.kind())),
                };
                entry!(table, out, StageOut, StageOut).table(&name)?.push(args.get(0).clone())?;
                Ok(Value::Null)
            }
            _ => Err(no_method(HandleKind::TablePiece, method)),
        }
    }

    /// `Expander::instantiate` takes the context, the use site, the template
    /// and the bindings at once, so that an argument in the wrong place is
    /// reported at the first wrong position of the four (`tests/boundary.rs`,
    /// "wrong order"); the generated arm would copy the use site first and
    /// report it before the context.
    pub(crate) fn expander(table: &mut CallTable<'_>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match method {
            "instantiate" => {
                args.arity(4)?;
                let handles = [handle, args.handle(0)?, args.handle(1)?, args.handle(2)?, args.handle(3)?];
                let staged = match table.entries_mut(method, handles)? {
                    [Entry::Expander(xp), Entry::ExpandCx(cx), Entry::Node(site), Entry::CompiledTemplate(template), Entry::PlaceholderBindings(bindings)] => {
                        xp.instantiate(&mut **cx, &**site, &**template, &**bindings)?
                    }
                    [Entry::Expander(xp), Entry::ExpandCx(cx), Entry::Node(site), Entry::CompiledTemplate(template), Entry::ValueBindings(bindings)] => {
                        xp.instantiate(&mut **cx, &**site, &**template, &*bindings)?
                    }
                    [_, cx, site, template, bindings] => {
                        let bindings = match bindings.kind() {
                            HandleKind::ValueBindings => HandleKind::PlaceholderBindings,
                            other => other,
                        };
                        let expected = [
                            (cx.kind(), HandleKind::ExpandCx),
                            (site.kind(), HandleKind::Node),
                            (template.kind(), HandleKind::CompiledTemplate),
                            (bindings, HandleKind::PlaceholderBindings),
                        ];
                        let (actual, kind) = expected
                            .into_iter()
                            .find(|(actual, kind)| actual != kind)
                            .unwrap_or((HandleKind::Expander, HandleKind::Expander));
                        return Err(mismatch(kind, actual));
                    }
                };
                staged.into_reply(table)
            }
            "lang" => {
                args.arity(0)?;
                entry!(table, handle, Expander, Expander);
                Ok(handle_to_value(table.lend(Entry::BuiltLang(LangEntry::Of(handle)))))
            }
            _ => Err(no_method(HandleKind::Expander, method)),
        }
    }

    /// The table `name` as the lowerer `handle` answers it: the counters and
    /// anchors tables recorded ([`TableEntry::Recorded`]), since their
    /// façades borrow the lowerer; an extension's own table, a rewrite's
    /// trees and a backend's lowered trees as owned copies
    /// ([`get_owned`](crate::sealed::LowererCore::get_owned)); any other
    /// table as its projection.
    fn lowerer_get(table: &mut CallTable<'_>, handle: Handle, name: &str) -> Result<Value, ExtError> {
        enum Got {
            None,
            Projection(Value),
            Counters,
            Anchors,
            Owned,
        }
        let got = match entry!(table, handle, Lowerer, Lowerer).get(name)? {
            None => Got::None,
            Some(TableValue::Projection(value)) => Got::Projection(value),
            Some(TableValue::Counters(_)) => Got::Counters,
            Some(TableValue::Anchors(_)) => Got::Anchors,
            Some(TableValue::Ext(_) | TableValue::Trees(_) | TableValue::Lowered(_)) => Got::Owned,
        };
        match got {
            Got::None => Ok(Value::Null),
            Got::Projection(projection) => Ok(serde_json::json!({ "projection": projection })),
            Got::Counters => Ok(serde_json::json!({ "counters": handle_to_value(table.lend(Entry::Counters(TableEntry::Recorded { lowerer: handle, name: name.to_string() }))) })),
            Got::Anchors => Ok(serde_json::json!({ "anchors": handle_to_value(table.lend(Entry::Anchors(TableEntry::Recorded { lowerer: handle, name: name.to_string() }))) })),
            Got::Owned => {
                let owned = entry!(table, handle, Lowerer, Lowerer).get_owned(Token::new(), name)?;
                owned.into_reply(table)
            }
        }
    }

    /// The counters table `handle` stands for, given to `f`: the façade
    /// lent, or asked of the lowerer again for a recorded one.
    fn with_counters<R>(
        table: &mut CallTable<'_>,
        handle: Handle,
        f: impl FnOnce(&dyn Counters) -> Result<R, ExtError>,
    ) -> Result<R, ExtError> {
        let recorded = match table.entry(handle)? {
            Entry::Counters(TableEntry::Recorded { lowerer, name }) => Some((*lowerer, name.clone())),
            Entry::Counters(TableEntry::Lent(_)) => None,
            other => return Err(mismatch(HandleKind::Counters, other.kind())),
        };
        match recorded {
            None => match table.entry(handle)? {
                Entry::Counters(TableEntry::Lent(counters)) => f(&**counters),
                other => Err(mismatch(HandleKind::Counters, other.kind())),
            },
            Some((lowerer, name)) => match entry!(table, lowerer, Lowerer, Lowerer).get(&name)? {
                Some(TableValue::Counters(counters)) => f(&*counters),
                _ => Err(argument(format!("the table `{name}` must be the numbering feature's counters table"))),
            },
        }
    }

    /// The anchors table `handle` stands for, given to `f`, as
    /// [`with_counters`].
    fn with_anchors<R>(
        table: &mut CallTable<'_>,
        handle: Handle,
        f: impl FnOnce(&dyn Anchors) -> Result<R, ExtError>,
    ) -> Result<R, ExtError> {
        let recorded = match table.entry(handle)? {
            Entry::Anchors(TableEntry::Recorded { lowerer, name }) => Some((*lowerer, name.clone())),
            Entry::Anchors(TableEntry::Lent(_)) => None,
            other => return Err(mismatch(HandleKind::Anchors, other.kind())),
        };
        match recorded {
            None => match table.entry(handle)? {
                Entry::Anchors(TableEntry::Lent(anchors)) => f(&**anchors),
                other => Err(mismatch(HandleKind::Anchors, other.kind())),
            },
            Some((lowerer, name)) => match entry!(table, lowerer, Lowerer, Lowerer).get(&name)? {
                Some(TableValue::Anchors(anchors)) => f(&*anchors),
                _ => Err(argument(format!("the table `{name}` must be the references feature's anchors table"))),
            },
        }
    }

    /// `Counters.record` and `Counters.format`: the point is read back from
    /// data by the façade, against the trees its reader sees
    /// ([`PointsCore::point_from_value`]), which no argument conversion
    /// can do; a recorded table is asked of its lowerer again.
    pub(crate) fn counters<'c>(table: &mut CallTable<'c>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match method {
            "record" => {
                args.arity(1)?;
                let record = with_counters(table, handle, |counters| {
                    let point = counters.point_from_value(Token::new(), method, args.get(0))?;
                    Ok(counters.record(point))
                })?;
                record.into_reply(table)
            }
            "format" => {
                args.arity(2)?;
                let options = format_options_from_value(args.get(1))?;
                let segments: Option<Vec<NumberSegment<'c>>> = with_counters(table, handle, |counters| {
                    let point = counters.point_from_value(Token::new(), method, args.get(0))?;
                    counters.format(point, &options)
                })?;
                segments.into_reply(table)
            }
            _ => Err(no_method(HandleKind::Counters, method)),
        }
    }

    /// `Anchors.id`, as [`counters`].
    pub(crate) fn anchors(table: &mut CallTable<'_>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match method {
            "id" => {
                args.arity(1)?;
                let id = with_anchors(table, handle, |anchors| {
                    let point = anchors.point_from_value(Token::new(), method, args.get(0))?;
                    Ok(anchors.id(point))
                })?;
                id.into_reply(table)
            }
            _ => Err(no_method(HandleKind::Anchors, method)),
        }
    }

    /// `Lowerer.get_at` has no façade method: `addr_of`, then `get`, then
    /// [`ExtTableRead::items_at`](api::core::data::table::ExtTableRead::items_at),
    /// composed here (an extension's own table, restricted to the items at
    /// one node).
    pub(crate) fn lowerer(table: &mut CallTable<'_>, handle: Handle, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match method {
            "get" => {
                args.arity(1)?;
                lowerer_get(table, handle, args.text(0)?)
            }
            "lang" => {
                args.arity(0)?;
                entry!(table, handle, Lowerer, Lowerer);
                Ok(handle_to_value(table.lend(Entry::BuiltLang(LangEntry::Of(handle)))))
            }
            "get_at" => {
                args.arity(2)?;
                let name = args.text(0)?;
                let node = node_from_table(table, args.handle(1)?)?;
                let lw = entry!(table, handle, Lowerer, Lowerer);
                let addr = lw.addr_of(&*node)?;
                match lw.get(name)? {
                    None => Ok(Value::Null),
                    Some(TableValue::Ext(read)) => {
                        // Data, checked on its way out (the reply's conversion
                        // would need the table, which the read borrows).
                        let items = Value::Array(read.items_at(&addr));
                        check_data(&items)?;
                        Ok(items)
                    }
                    Some(_) => Err(argument(format!(
                        "the table `{name}` must be a table an extension declared, whose items are data"
                    ))),
                }
            }
            _ => Err(no_method(HandleKind::Lowerer, method)),
        }
    }

    /// The module functions written by hand: `arg_def_from_code`, whose
    /// reply (an `ArgDef`) is written with the code it was read from, which
    /// the reply's conversion cannot know; `line_frames`, whose first
    /// argument is one of two kinds (`stage_cx` or `lowerer`), the
    /// `FrameReader` borrowing the entry across the call. Any other name is
    /// no function of the module.
    pub(crate) fn module<'c>(table: &mut CallTable<'c>, path: &str, method: &str, args: &Args<'_>) -> Result<Value, ExtError> {
        match (path, method) {
            ("lang/mint", "arg_def_from_code") => {
                args.arity(1)?;
                let code = args.text(0)?;
                Ok(arg_def_to_value(&api::lang::mint::arg_def_from_code(code)?))
            }
            ("stdcore/features/math", "line_frames") => {
                args.arity(4)?;
                let (anchors, counters) = (args.flag(1)?, args.flag(2)?);
                let node = <Box<dyn Node<'c> + 'c> as FromArg>::from_arg(table, args, 3)?;
                let frames = match table.entry(args.handle(0)?)? {
                    Entry::StageCx(cx) => math::line_frames(math::FrameReader::Stage(*cx), anchors, counters, &*node)?,
                    Entry::Lowerer(lw) => math::line_frames(math::FrameReader::Rule(&**lw), anchors, counters, &*node)?,
                    other => {
                        return Err(argument(format!(
                            "argument 1 must be a stage's context (`stage_cx`) or a rule's lowerer (`lowerer`), not an \
                             object of the kind `{}`",
                            other.kind().name()
                        )));
                    }
                };
                frames.into_reply(table)
            }
            ("lang/rewrites/userdefs", "TemplateLanguage::new") => {
                args.arity(1)?;
                let language = with_lang(table, args.handle(0)?, |lang| userdefs::TemplateLanguage::new(lang))?;
                language.into_reply(table)
            }
            ("lang/rewrites/userdefs", "compile_template_with") => {
                args.arity(3)?;
                let spec = <userdefs::TemplateSpec as FromArg>::from_arg(table, args, 0)?;
                let language = entry!(table, args.handle(2)?, TemplateLanguage, TemplateLanguage).clone();
                let template = with_lang(table, args.handle(1)?, |lang| userdefs::compile_template_with(&spec, lang, &language))?;
                template.into_reply(table)
            }
            ("lang/rewrites/userdefs", "instantiate") => {
                args.arity(3)?;
                let site = <Box<dyn Node<'c> + 'c> as FromArg>::from_arg(table, args, 0)?;
                let node = match table.entries_mut(method, [args.handle(1)?, args.handle(2)?])? {
                    [Entry::CompiledTemplate(template), Entry::PlaceholderBindings(bindings)] => {
                        userdefs::instantiate(&*site, &**template, &**bindings)?
                    }
                    [Entry::CompiledTemplate(template), Entry::ValueBindings(bindings)] => {
                        userdefs::instantiate(&*site, &**template, &*bindings)?
                    }
                    [Entry::CompiledTemplate(_), other] => return Err(mismatch(HandleKind::PlaceholderBindings, other.kind())),
                    [other, _] => return Err(mismatch(HandleKind::CompiledTemplate, other.kind())),
                };
                node.into_reply(table)
            }
            _ => Err(argument(format!("the module `{path}` has no function `{method}`"))),
        }
    }

    /// The built language `handle` stands for, given to `f`: the façade
    /// lent, or asked of its lowerer or expander again for a recorded one
    /// ([`LangEntry::Of`]).
    fn with_lang<R>(
        table: &mut CallTable<'_>,
        handle: Handle,
        f: impl FnOnce(&dyn BuiltLang) -> Result<R, ExtError>,
    ) -> Result<R, ExtError> {
        let of = match table.entry(handle)? {
            Entry::BuiltLang(LangEntry::Of(of)) => Some(*of),
            Entry::BuiltLang(LangEntry::Lent(_)) => None,
            other => return Err(mismatch(HandleKind::BuiltLang, other.kind())),
        };
        match of {
            None => match table.entry(handle)? {
                Entry::BuiltLang(LangEntry::Lent(lang)) => f(*lang),
                other => Err(mismatch(HandleKind::BuiltLang, other.kind())),
            },
            Some(of) => match table.entry(of)? {
                Entry::Lowerer(lw) => f(&*lw.lang()),
                Entry::Expander(xp) => f(&*xp.lang()),
                other => Err(mismatch(HandleKind::Lowerer, other.kind())),
            },
        }
    }
}
