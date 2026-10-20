//! The key-value argument of an element: its keys in the row's order, each
//! value written by its kind ([`KeyValue`]).

use super::*;

impl Emitter<'_> {
    /// Writes a name: bare where it may stand bare, else braced as escaped
    /// text.
    pub(super) fn name_value(&mut self, name: &str) {
        if is_bare_name(name) {
            self.put(name);
        } else {
            self.put("{");
            self.text_value(name);
            self.put("}");
        }
    }

    /// Writes document text inside an argument.
    pub(super) fn text_value(&mut self, text: &str) {
        let flow = std::mem::replace(&mut self.flow, true);
        self.text(text);
        self.flow = flow;
    }

    /// Writes the key-value argument of an element of the row `row`: the keys
    /// `values` gives, in the row's order, or nothing where it gives none.
    pub(super) fn keys(&mut self, row: &Row, values: Vec<(&'static str, KeyValue<'_>)>) {
        let order: Vec<&'static str> =
            row.keys.iter().filter(|key| !key.is_entry()).map(|key| key.key).collect();
        self.keys_in(row, &order, values);
    }

    /// Writes the key-value argument of one entry of a list of the row `row`
    /// (`\flmxItem[…]`), in the order of the row's entry keys.
    pub(super) fn entry_keys(&mut self, row: &Row, values: Vec<(&'static str, KeyValue<'_>)>) {
        let order: Vec<&'static str> =
            row.keys.iter().filter(|key| key.is_entry()).map(|key| key.key).collect();
        self.keys_in(row, &order, values);
    }

    pub(super) fn keys_in(&mut self, row: &Row, order: &[&'static str], values: Vec<(&'static str, KeyValue<'_>)>) {
        if values.is_empty() {
            return;
        }
        let mut values: Vec<Option<(&'static str, KeyValue<'_>)>> = values.into_iter().map(Some).collect();
        self.no_break += 1;
        self.put("[");
        let mut first = true;
        for key in order.iter().copied() {
            for slot in values.iter_mut() {
                if slot.as_ref().is_some_and(|(name, _)| *name == key) {
                    let (name, value) = slot.take().expect("checked");
                    if !first {
                        self.put(", ");
                    }
                    first = false;
                    self.key_value(name, value);
                }
            }
        }
        debug_assert!(values.iter().all(Option::is_none), "a key outside the row of {}", row.element);
        self.put("]");
        self.no_break -= 1;
    }

    /// Writes one `key=value`.
    pub(super) fn key_value(&mut self, key: &str, value: KeyValue<'_>) {
        self.put(key);
        self.put("=");
        match value {
            KeyValue::Name(name) => self.name_value(&name),
            KeyValue::Words(words) => {
                self.put("{");
                for (index, word) in words.iter().enumerate() {
                    if index > 0 {
                        self.put(" ");
                    }
                    if is_bare_name(word) {
                        self.put(word);
                    } else {
                        // Braced, so that the lookup does not split it at a
                        // comma or a space it holds.
                        self.put("{");
                        self.text_value(word);
                        self.put("}");
                    }
                }
                self.put("}");
            }
            KeyValue::Content(nodes) => {
                self.put("{");
                self.inline_content(None, &nodes);
                self.put("}");
            }
            KeyValue::Text(text) => {
                self.put("{");
                self.text_value(&text);
                self.put("}");
            }
            KeyValue::Address(address) => {
                self.put("{");
                self.put(&anchor_spelling(&address));
                self.put("}");
            }
            KeyValue::Anchor(anchor) if is_bare_name(&anchor) => self.put(&anchor),
            KeyValue::Anchor(anchor) => {
                self.put("{");
                self.put(&anchor_spelling(&anchor));
                self.put("}");
            }
            KeyValue::Length(length) => self.put(&latex_length(&length)),
            KeyValue::Number(number) => self.put(&number.to_string()),
            KeyValue::Bool(value) => self.put(if value { "true" } else { "false" }),
            KeyValue::Specs(specs) => {
                self.put("{");
                self.put(&specs.join(", "));
                self.put("}");
            }
        }
    }
}
