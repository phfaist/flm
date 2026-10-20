//! The writers of the other elements of the vocabulary: headings, lists,
//! tables of cells, lines, verbatim text, images, links, the three marks,
//! decorations and placed trees.

use super::*;

impl Emitter<'_> {
    /// The keys of a heading: kind, level, toc, anchor, number, the title's
    /// plain text (its characters, every run of white space one space, as a
    /// piece's words are computed), and the alternative title, the content of
    /// the heading's `alt` slot, where it has one that is not empty.
    pub(super) fn heading_values<'t>(node: &NodeRef<'t, Flm>) -> Vec<(&'static str, KeyValue<'t>)> {
        let mut values = Vec::new();
        if let Some(payload) = payload_of::<HeadingPayload>(node) {
            values.push(("kind", KeyValue::Name(payload.name.to_string())));
            values.push(("level", KeyValue::Number(u64::from(payload.level))));
            values.push(("toc", KeyValue::Bool(payload.in_toc)));
            if let Some(anchor) = &payload.anchor {
                values.push(("anchor", KeyValue::Anchor(anchor.clone())));
            }
            if let Some(number) = payload.number.as_ref().filter(|number| !number.is_empty()) {
                values.push(("number", KeyValue::Text(number.clone())));
            }
        }
        let plain = plain_text(body(node)).split_whitespace().collect::<Vec<_>>().join(" ");
        values.push(("plain", KeyValue::Text(plain)));
        let alt: Vec<NodeRef<'t, Flm>> =
            slot_named(node, pres::ALT_SLOT).map(|slice| slice.iter().collect()).unwrap_or_default();
        if !alt.is_empty() {
            values.push(("alt", KeyValue::Content(alt)));
        }
        values
    }

    /// `\flmxHeading[keys]{title}`, on a line of its own.
    pub(super) fn heading(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        self.require_row(row);
        self.begin_block();
        self.put("\\flmxHeading");
        let values = Self::heading_values(node);
        self.keys(row, values);
        self.put("{");
        let content = body(node);
        self.inline_content(Some(node), &content);
        self.put("}");
        self.end_block();
    }

    /// The stop after a run-in title: the render option, unless it is empty,
    /// the title is blank or it ends in sentence punctuation already.
    pub(super) fn stop(&self, content: &[NodeRef<'_, Flm>]) -> Option<String> {
        let stop = &self.options.run_in_stop;
        (!stop.is_empty()
            && holds_text(content.iter().copied())
            && !ends_in_sentence_punctuation(content.iter().copied()))
        .then(|| stop.clone())
    }

    /// `\flmxRuninHeading[keys]{title}`, inline inside its run-in decoration,
    /// the stop as the key `stop`.
    pub(super) fn runin_heading(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let content = body(node);
        let mut values = Self::heading_values(node);
        if let Some(stop) = self.stop(&content) {
            values.push(("stop", KeyValue::Text(stop)));
        }
        self.inline_command(node, row, values, &content);
    }

    // --- lists, cells, lines, verbatim, images ----------------------------------------------

    /// `\begin{flmxList}[kinds, anchor, depth]`, then for each entry
    /// `\flmxItem[label, anchor]` on a line of its own followed by the
    /// entry's blocks (a paragraph's inline run where the entry is inline),
    /// a blank line between two entries, then `\end{flmxList}`. The depth
    /// counts the lists open around this one, from one.
    pub(super) fn list(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<ListPayload>(node);
        let mut values = Vec::new();
        if let Some(payload) = payload {
            if !payload.kinds.is_empty() {
                values.push(("kinds", KeyValue::Words(payload.kinds.iter().map(Name::to_string).collect())));
            }
            if let Some(anchor) = &payload.anchor {
                values.push(("anchor", KeyValue::Anchor(anchor.clone())));
            }
        }
        values.push(("depth", KeyValue::Number(u64::from(self.lists_open) + 1)));
        self.require_row(row);
        self.begin_block();
        self.put("\\begin{flmxList}");
        self.keys(row, values);
        self.newline();
        self.lists_open += 1;
        let last = std::mem::replace(&mut self.last, Last::Nothing);
        for (index, (label, item)) in list_entries(node).into_iter().enumerate() {
            self.line_start();
            if index > 0 {
                self.newline();
            }
            let mut item_values = Vec::new();
            let label: Vec<NodeRef<'_, Flm>> = label.iter().collect();
            if !shows_nothing(&label) {
                item_values.push(("label", KeyValue::Content(label)));
            }
            let anchor = payload.and_then(|payload| payload.entry_anchors.get(index).cloned().flatten());
            if let Some(anchor) = anchor {
                item_values.push(("anchor", KeyValue::Anchor(anchor)));
            }
            self.put("\\flmxItem");
            self.entry_keys(row, item_values);
            self.newline();
            let item: Vec<NodeRef<'_, Flm>> = item.iter().collect();
            self.last = Last::Nothing;
            self.container(Some(node), &item);
        }
        self.last = last;
        self.lists_open -= 1;
        self.line_start();
        self.put("\\end{flmxList}");
        self.end_block();
    }

    /// `\begin{flmxCells}[rows, cols, cells]`, then the rows of the grid
    /// ([`CellsPayload::grid`](pres::CellsPayload::grid)): every grid
    /// position a column, the positions separated by `&`, each row ended by
    /// `\\` and a newline. A cell's content is written braced at its top-left
    /// position ([`cell`](Self::cell)); an empty position and one a spanning
    /// cell covers hold nothing.
    pub(super) fn cells(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let Some((payload, contents)) = cells_of(node) else {
            return self.unexpected(node, "a table of cells without its payload,");
        };
        let values = vec![
            ("rows", KeyValue::Number(u64::from(payload.rows))),
            ("cols", KeyValue::Number(u64::from(payload.cols))),
            ("cells", KeyValue::Specs(payload.cells.iter().map(cell_spec).collect())),
        ];
        self.require_row(row);
        self.begin_block();
        self.put("\\begin{flmxCells}");
        self.keys(row, values);
        self.newline();
        for grid_row in payload.grid() {
            for (column, position) in grid_row.iter().enumerate() {
                if column > 0 {
                    self.put(" & ");
                }
                if let GridPosition::TopLeft(index) = position {
                    let content: Vec<NodeRef<'_, Flm>> =
                        contents.get(*index).cloned().flatten().map(|slice| slice.iter().collect()).unwrap_or_default();
                    self.cell(node, &content);
                }
            }
            self.put(" \\\\");
            self.newline();
        }
        self.put("\\end{flmxCells}");
        self.end_block();
    }

    /// One cell's content, braced: its blocks with `\par` between consecutive
    /// blocks, a paragraph's inline run as it is (after the target of its
    /// anchor, where it has one), any other block as it is written anywhere.
    /// The braces keep a `\\` or a `&` of the content inside the cell. A
    /// `\par`, not a `\\`, separates the blocks: a `\\` after a block that
    /// ends between paragraphs (a list, a verbatim block) has no line to end,
    /// and one before a paragraph that begins with `[` reads it as a length.
    pub(super) fn cell(&mut self, node: &NodeRef<'_, Flm>, content: &[NodeRef<'_, Flm>]) {
        let flow = self.flow;
        let last = std::mem::replace(&mut self.last, Last::Nothing);
        self.put("{");
        for (index, item) in content.iter().enumerate() {
            if index > 0 {
                self.put("\\par ");
            }
            // A paragraph continues the cell's line; any other block starts
            // on a line of its own.
            self.flow = is_element(item, pres::PAR);
            if self.flow {
                let anchor = payload_of::<ParPayload>(item).and_then(|payload| payload.anchor.clone());
                if let Some(anchor) = anchor {
                    self.target(&anchor);
                }
                let inner = body(item);
                self.run(Some(item), &inner);
            } else {
                self.run(Some(node), std::slice::from_ref(item));
            }
        }
        self.put("}");
        self.flow = flow;
        self.last = last;
    }

    /// `\begin{flmxLines}`, its lines, `\end{flmxLines}`.
    pub(super) fn lines(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let content = body(node);
        self.environment_of(node, row, Vec::new(), &content);
    }

    /// `\flmxLine[indent-left, indent-right, align]{content}` on a line of its
    /// own, the three keys always written.
    pub(super) fn line(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let mut values = Vec::new();
        if let Some(payload) = payload_of::<LinePayload>(node) {
            values.push(("indent-left", KeyValue::Length(payload.indent_left)));
            values.push(("indent-right", KeyValue::Length(payload.indent_right)));
            values.push(("align", KeyValue::Name(String::from(payload.align.as_str()))));
        }
        self.require_row(row);
        self.begin_block();
        self.put("\\flmxLine");
        self.keys(row, values);
        self.put("{");
        let content = body(node);
        self.inline_content(Some(node), &content);
        self.put("}");
        self.end_block();
    }

    /// The text of a verbatim node: its characters, a group's children's.
    pub(super) fn verbatim_text(&mut self, nodes: &[NodeRef<'_, Flm>]) -> String {
        let mut out = String::new();
        for node in nodes {
            match node.kind() {
                NodeKind::Chars { .. } => out.push_str(node.chars().unwrap_or("")),
                NodeKind::Group(_) if node.group_type() == Some(FlmGroupType::Content) => {
                    if self.descend() {
                        let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
                        let text = self.verbatim_text(&children);
                        out.push_str(&text);
                        self.ascend();
                    }
                }
                _ => out.push_str(&recompose_source(&[*node])),
            }
        }
        out
    }

    /// Verbatim text, escaped ([`escape_verbatim`]), never broken across
    /// lines: a block as `\begin{flmxVerbatimBlock}[kind, lang]`, one
    /// `\flmxVerbLine{…}` per line of the text (a final newline ending the
    /// last line), `\end{flmxVerbatimBlock}`; inline text as
    /// `\flmxVerbatim[kind, lang]{…}`, a newline in it written as a space.
    pub(super) fn verbatim(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<VerbatimPayload>(node);
        let mut values = Vec::new();
        if let Some(payload) = payload {
            values.push(("kind", KeyValue::Name(String::from(payload.kind.as_str()))));
            if let Some(lang) = &payload.lang {
                values.push(("lang", KeyValue::Name(lang.clone())));
            }
        }
        let content = body(node);
        let text = self.verbatim_text(&content);
        self.require_row(row);
        self.no_break += 1;
        if payload.is_some_and(|payload| payload.display == VerbatimDisplay::Block) {
            self.begin_block();
            self.put("\\begin{flmxVerbatimBlock}");
            self.keys(row, values);
            self.newline();
            let text = text.strip_suffix('\n').unwrap_or(&text);
            for line in text.split('\n') {
                self.put("\\flmxVerbLine{");
                self.put(&escape_verbatim(line));
                self.put("}");
                self.newline();
            }
            self.put("\\end{flmxVerbatimBlock}");
            self.end_block();
        } else {
            self.inline();
            self.put("\\flmxVerbatim");
            self.keys(row, values);
            self.put("{");
            self.put(&escape_verbatim(&text.replace('\n', " ")));
            self.put("}");
        }
        self.no_break -= 1;
    }

    /// `\flmxImage[width, height, alt, display, anchor]{path}`, the path
    /// the graphics feature published for this backend written as an
    /// address; on a line of its own for a block image, inline otherwise.
    pub(super) fn image(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let Some(payload) = payload_of::<ImagePayload>(node) else {
            return self.unexpected(node, "an image without its payload,");
        };
        let mut values = Vec::new();
        if let Some(width) = payload.width {
            values.push(("width", KeyValue::Length(width)));
        }
        if let Some(height) = payload.height {
            values.push(("height", KeyValue::Length(height)));
        }
        if let Some(alt) = &payload.alt {
            values.push(("alt", KeyValue::Text(alt.clone())));
        }
        values.push(("display", KeyValue::Name(String::from(payload.display.as_str()))));
        if let Some(anchor) = &payload.anchor {
            values.push(("anchor", KeyValue::Anchor(anchor.clone())));
        }
        self.require_row(row);
        let block = payload.display == ImageDisplay::Block;
        if block {
            self.begin_block();
        } else {
            self.inline();
        }
        self.put("\\flmxImage");
        self.keys(row, values);
        self.put("{");
        self.put(&escape_url(&payload.src));
        self.put("}");
        if block {
            self.end_block();
        }
    }

    // --- links ------------------------------------------------------------------------

    /// `\flmxLink[keys]{content}`: the kind, the address, the target of an
    /// internal link (the address without its `#`), the document role, the
    /// label and whether the content shows the address. A link inside another
    /// link's content is written as its content alone.
    pub(super) fn link(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let content = body(node);
        if self.links_open > 0 {
            // A link inside a link's content is written as its content alone.
            return self.run(Some(node), &content);
        }
        let mut values = Vec::new();
        if let Some(payload) = payload_of::<LinkPayload>(node) {
            values.push(("kind", KeyValue::Name(payload.kind.to_string())));
            values.push(("href", KeyValue::Address(payload.href.clone())));
            if let Some(target) = payload.href.strip_prefix('#') {
                values.push(("target", KeyValue::Anchor(String::from(target))));
            }
            if let Some(doc_role) = payload.doc_role {
                values.push(("doc-role", KeyValue::Name(String::from(doc_role.as_str()))));
            }
            if let Some(label) = &payload.label {
                values.push(("label", KeyValue::Text(label.clone())));
            }
            if payload.shows_target {
                values.push(("shows-target", KeyValue::Bool(true)));
            }
        }
        self.links_open += 1;
        self.inline_command(node, row, values, &content);
        self.links_open -= 1;
    }

    // --- the marks ------------------------------------------------------------------------

    /// `\<command>[keys]{first}{second}`, two text arguments.
    pub(super) fn mark(&mut self, row: &Row, values: Vec<(&'static str, KeyValue<'_>)>, first: &str, second: &str) {
        self.require_row(row);
        self.inline();
        self.put("\\");
        self.put(Self::command_of(row));
        self.keys(row, values);
        for argument in [first, second] {
            self.no_break += 1;
            self.put("{");
            self.text_value(argument);
            self.put("}");
            self.no_break -= 1;
        }
    }

    pub(super) fn unsupported(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<UnsupportedPayload>(node);
        let element = payload.map_or_else(String::new, |payload| payload.element.to_string());
        let message = payload.map_or_else(String::new, |payload| payload.message.clone());
        self.mark(row, Vec::new(), &element, &message);
    }

    pub(super) fn dropped(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<DroppedPayload>(node);
        let condition = payload.map_or_else(String::new, |payload| payload.condition.to_string());
        let detail = payload.map_or_else(String::new, |payload| payload.detail.clone());
        self.mark(row, Vec::new(), &condition, &detail);
    }

    /// An unattributed macro or environment: the mark, and an environment's
    /// body after it.
    pub(super) fn unknown(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let payload = payload_of::<UnknownPayload>(node);
        let kind = payload.map_or(UnknownKind::Macro, |payload| payload.kind);
        let name = payload.map_or_else(String::new, |payload| payload.name.clone());
        let spelling = payload.map_or_else(String::new, |payload| payload.spelling.clone());
        let values = vec![("kind", KeyValue::Name(String::from(kind.as_str())))];
        self.mark(row, values, &name, &spelling);
        if kind == UnknownKind::Environment {
            let content = body(node);
            self.run(Some(node), &content);
        }
    }

    // --- decorations, placed trees --------------------------------------------------------------

    /// A decoration, at its mark (or where it stands in a tree no lowering
    /// driver built): `\flmxDecoRunin[stop={.}]{…}`, `\flmxDecoFinal{…}`,
    /// `\flmxDecoMargin{…}`.
    pub(super) fn decoration(&mut self, node: &NodeRef<'_, Flm>, row: &Row) {
        let content = body(node);
        let mut values = Vec::new();
        if is_element(node, pres::DECO_RUNIN) {
            if let Some(stop) = self.stop(&content) {
                values.push(("stop", KeyValue::Text(stop)));
            }
        }
        self.inline_command(node, row, values, &content);
    }

    /// A decoration mark: the decoration it stands for, at its position.
    pub(super) fn deco_mark(&mut self, carrier: Option<&NodeRef<'_, Flm>>, node: &NodeRef<'_, Flm>) {
        let Some(carrier) = carrier else { return };
        let Some(deco) = decoration_at(carrier, node) else { return };
        let Some(row) = row(element_of(&deco).name.as_str()) else { return };
        self.decoration(&deco, row);
    }

    /// A placed tree: the segment being written ends here, and the regions
    /// stage writes the placed tree's LaTeX in its place.
    pub(super) fn subtree(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<SubtreePayload>(node) else { return };
        self.begin_block();
        let at = self.sink.position();
        self.placements.push((at, payload.tree, node.span().clone()));
        self.last = Last::Block;
    }
}
