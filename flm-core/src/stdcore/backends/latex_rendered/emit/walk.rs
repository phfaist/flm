//! The walk over a tree, dispatching each node to its element's writer, and
//! the writers of paragraphs, blocks and spans.

use super::*;

impl Emitter<'_> {
    pub(super) fn run(&mut self, carrier: Option<&NodeRef<'_, Flm>>, nodes: &[NodeRef<'_, Flm>]) {
        for node in nodes {
            if !self.descend() {
                return;
            }
            self.node(carrier, node);
            self.ascend();
        }
    }

    pub(super) fn node(&mut self, carrier: Option<&NodeRef<'_, Flm>>, node: &NodeRef<'_, Flm>) {
        if let Some(text) = node.chars() {
            self.text(text);
            return;
        }
        if node.is_comment() {
            return;
        }
        if is_element(node, structural::PARBREAK) {
            self.end_flow();
            return;
        }
        if !is_presentation(node) {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            self.run(Some(node), &children);
            return;
        }
        let name = element_of(node).name.clone();
        match name.as_str() {
            construct::MACRO => return self.macro_call(node),
            construct::ENVIRONMENT => return self.environment(node),
            construct::RAW => return self.raw(node),
            construct::DEFINITION => return self.definition(node),
            _ => {}
        }
        let Some(row) = row(name.as_str()) else {
            return self.unexpected(node, "the element");
        };
        match name.as_str() {
            pres::PAR => self.par(node, row),
            pres::BLOCK => self.block(node, row),
            pres::BLOCK_HEADING => self.heading(node, row),
            pres::RUNIN_HEADING => self.runin_heading(node, row),
            pres::SPAN => self.span(node, row),
            pres::LIST => self.list(node, row),
            pres::CELLS => self.cells(node, row),
            pres::LINES => self.lines(node, row),
            pres::LINE => self.line(node, row),
            pres::VERBATIM => self.verbatim(node, row),
            pres::MATH_PASSTHROUGH => self.math_passthrough(node),
            pres::MATH_TEXT => self.math_text(node),
            pres::LINK => self.link(node, row),
            pres::IMAGE => self.image(node, row),
            pres::UNSUPPORTED => self.unsupported(node, row),
            pres::DROPPED => self.dropped(node, row),
            pres::UNKNOWN => self.unknown(node, row),
            pres::DECO_RUNIN | pres::DECO_FINAL | pres::DECO_MARGIN => self.decoration(node, row),
            pres::DECO_MARK => self.deco_mark(carrier, node),
            pres::SUBTREE => self.subtree(node),
            _ => self.unexpected(node, "the element"),
        }
    }

    /// The command's or environment's name of `row`.
    pub(super) fn command_of(row: &Row) -> &'static str {
        match row.form {
            Form::Command(name) | Form::Environment(name) => name,
            Form::CommandOrEnvironment { command, .. } => command,
            _ => "",
        }
    }

    /// `\<command>[keys]{content}`, inline.
    pub(super) fn inline_command(
        &mut self,
        node: &NodeRef<'_, Flm>,
        row: &Row,
        values: Vec<(&'static str, KeyValue<'_>)>,
        content: &[NodeRef<'_, Flm>],
    ) {
        self.require_row(row);
        self.inline();
        self.put("\\");
        self.put(Self::command_of(row));
        self.keys(row, values);
        self.put("{");
        self.inline_content(Some(node), content);
        self.put("}");
    }

    /// `\begin{<environment>}[keys]`, the blocks, `\end{<environment>}`.
    pub(super) fn environment_of(
        &mut self,
        node: &NodeRef<'_, Flm>,
        row: &Row,
        values: Vec<(&'static str, KeyValue<'_>)>,
        content: &[NodeRef<'_, Flm>],
    ) {
        let name = Self::command_of(row);
        self.require_row(row);
        self.begin_block();
        self.put(&format!("\\begin{{{name}}}"));
        self.keys(row, values);
        self.newline();
        self.container(Some(node), content);
        self.put(&format!("\\end{{{name}}}"));
        self.end_block();
    }

    // --- paragraphs, blocks, spans ----------------------------------------------------------

    /// A paragraph: a LaTeX paragraph, `\flmxTarget{…}` first where it has an
    /// anchor; nothing for one with no anchor that shows nothing.
    pub(super) fn par(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let anchor = payload_of::<ParPayload>(node).and_then(|payload| payload.anchor.clone());
        let content = body(node);
        if anchor.is_none() && shows_nothing(&content) {
            return;
        }
        self.require_row(row);
        self.end_flow();
        self.begin_flow();
        if let Some(anchor) = anchor {
            self.target(&anchor);
        }
        self.run(Some(node), &content);
        self.end_flow();
    }

    /// `\flmxTarget{<anchor>}`.
    pub(in crate::stdcore::backends::latex_rendered) fn target(&mut self, anchor: &str) {
        self.no_break += 1;
        self.put("\\flmxTarget{");
        self.put(&anchor_spelling(anchor));
        self.put("}");
        self.no_break -= 1;
    }

    /// The keys a block's or a span's role, kinds, anchor and document role
    /// give.
    pub(super) fn common_values(
        role: Option<&Name>,
        kinds: &[Name],
        anchor: Option<&String>,
        doc_role: Option<pres::DocRole>,
    ) -> Vec<(&'static str, KeyValue<'static>)> {
        let mut values = Vec::new();
        if let Some(role) = role {
            values.push(("role", KeyValue::Name(role.to_string())));
        }
        if !kinds.is_empty() {
            values.push(("kinds", KeyValue::Words(kinds.iter().map(Name::to_string).collect())));
        }
        if let Some(anchor) = anchor {
            values.push(("anchor", KeyValue::Anchor(anchor.clone())));
        }
        if let Some(doc_role) = doc_role {
            values.push(("doc-role", KeyValue::Name(String::from(doc_role.as_str()))));
        }
        values
    }

    pub(super) fn block(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<BlockPayload>(node);
        let values = payload.map_or_else(Vec::new, |payload| {
            Self::common_values(payload.role.as_ref(), &payload.kinds, payload.anchor.as_ref(), payload.doc_role)
        });
        let content = body(node);
        self.environment_of(node, row, values, &content);
    }

    pub(super) fn span(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<SpanPayload>(node);
        let mut values = Vec::new();
        if let Some(style) = payload.and_then(|payload| payload.style) {
            values.push(("style", KeyValue::Name(String::from(style.as_str()))));
        }
        if let Some(payload) = payload {
            values.extend(Self::common_values(
                payload.role.as_ref(),
                &payload.kinds,
                payload.anchor.as_ref(),
                payload.doc_role,
            ));
        }
        let content = body(node);
        self.inline_command(node, row, values, &content);
    }
}
