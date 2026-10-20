//! The construct elements of the LaTeX family, written natively: a macro, an
//! environment, a raw piece, and a definition recorded for the style file.

use super::*;

impl Emitter<'_> {
    pub(super) fn macro_call(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<LatexMacroPayload>(node) else {
            return self.unexpected(node, "a macro construct without its payload,");
        };
        self.inline();
        self.put("\\");
        self.put(&payload.name);
        // The first argument written, if any, ends the control word.
        self.control_word = payload.name.ends_with(|ch: char| ch.is_ascii_alphabetic());
        self.arguments(node, &payload.args, payload.packed);
    }

    pub(super) fn arguments(&mut self, node: &NodeRef<'_, Flm>, args: &[LatexArgShape], packed: bool) {
        if packed {
            self.put("{");
        }
        for (shape, slot) in arg_slots(node, args) {
            let nodes: Vec<NodeRef<'_, Flm>> = slot.map(|slot| slot.iter().collect()).unwrap_or_default();
            match shape {
                LatexArgShape::Star => self.put("*"),
                LatexArgShape::Optional => {
                    let plain = nodes.iter().all(|node| {
                        if let Some(text) = node.chars() {
                            !text.contains(']')
                        } else if let Some(raw) = payload_of::<LatexRawPayload>(node) {
                            !raw.text.contains(']')
                        } else {
                            node.group_type() == Some(FlmGroupType::Content)
                        }
                    });
                    self.put(if plain { "[" } else { "[{" });
                    self.inline_content(Some(node), &nodes);
                    self.put(if plain { "]" } else { "}]" });
                }
                LatexArgShape::Mandatory => {
                    self.put("{");
                    self.inline_content(Some(node), &nodes);
                    self.put("}");
                }
                LatexArgShape::EscVerb => {
                    let text = self.verbatim_text(&nodes);
                    self.no_break += 1;
                    self.put("{");
                    self.put(&escape_verbatim(&text));
                    self.put("}");
                    self.no_break -= 1;
                }
            }
        }
        if packed {
            self.put("}");
        }
    }

    pub(super) fn environment(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<LatexEnvironmentPayload>(node) else {
            return self.unexpected(node, "an environment construct without its payload,");
        };
        self.begin_block();
        self.put(&format!("\\begin{{{}}}", payload.name));
        self.arguments(node, &payload.args, payload.packed);
        self.newline();
        let content: Vec<NodeRef<'_, Flm>> =
            slot_named(node, pres::BODY_SLOT).map(|slot| slot.iter().collect()).unwrap_or_default();
        if payload.verbatim {
            let text = self.verbatim_text(&content);
            self.flush();
            self.sink.verbatim_block(&text);
            self.advance(&text);
            self.line_start();
        } else {
            self.container(Some(node), &content);
        }
        self.put(&format!("\\end{{{}}}", payload.name));
        self.end_block();
    }

    pub(super) fn raw(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<LatexRawPayload>(node) else {
            return self.unexpected(node, "a raw construct without its payload,");
        };
        self.inline();
        self.put_raw(node, &LatexRaw::from_raw_payload(payload));
    }

    /// A definition construct: recorded for the style file once by name,
    /// written nowhere in the flow.
    pub(super) fn definition(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<LatexDefinitionPayload>(node) else {
            return self.unexpected(node, "a definition construct without its payload,");
        };
        let name = match payload.kind {
            LatexDefinitionKind::Macro => format!("\\{}", payload.name),
            _ => payload.name.clone(),
        };
        if self.definitions.iter().any(|definition| definition.name == name) {
            return;
        }
        let mut sub = Emitter::new(self.options);
        sub.depth = self.depth;
        sub.flow = true;
        sub.no_break = 1;
        let slot = |slot: &str| -> Vec<NodeRef<'_, Flm>> {
            slot_named(node, slot).map(|slot| slot.iter().collect()).unwrap_or_default()
        };
        let parameters = if payload.packed { payload.args.min(1) } else { payload.args };
        match payload.kind {
            LatexDefinitionKind::Macro => {
                if payload.robust {
                    let specifiers = vec!["+m"; parameters].join(" ");
                    sub.put(&format!("\\ProvideDocumentCommand\\{}{{{specifiers}}}", payload.name));
                } else if parameters == 0 {
                    sub.put(&format!("\\providecommand\\{}", payload.name));
                } else {
                    sub.put(&format!("\\providecommand\\{}[{parameters}]", payload.name));
                }
                sub.put("{");
                sub.run(Some(node), &slot(pres::BODY_SLOT));
                sub.put("}");
            }
            LatexDefinitionKind::Environment => {
                let mut specifiers = vec!["+m"; parameters];
                if payload.body_argument {
                    specifiers.push("+b");
                }
                sub.put(&format!(
                    "\\ProvideDocumentEnvironment{{{}}}{{{}}}",
                    payload.name,
                    specifiers.join(" ")
                ));
                sub.put("{");
                sub.run(Some(node), &slot(BEGIN_SLOT));
                sub.put("}{");
                sub.run(Some(node), &slot(END_SLOT));
                sub.put("}");
            }
            LatexDefinitionKind::Declaration => {
                sub.put(&format!("\\ifcsname {}\\endcsname\\else ", payload.name));
                sub.run(Some(node), &slot(pres::BODY_SLOT));
                sub.put("\\fi");
            }
        }
        sub.newline();
        let Emitter { sink, requires, definitions, refused, diags, .. } = sub;
        self.refused |= refused;
        self.requires.extend(requires);
        for definition in definitions {
            if !self.definitions.iter().any(|seen| seen.name == definition.name) {
                self.definitions.push(definition);
            }
        }
        for diagnostic in diags.iter() {
            self.diags.push(diagnostic.clone());
        }
        self.definitions.push(StagedDefinition { name, text: sink.finish().text });
    }
}
