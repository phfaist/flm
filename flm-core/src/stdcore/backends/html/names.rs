//! The naming functions of the HTML contract: how a presentation element's
//! name, a role and a payload field become the class names, attribute names
//! and custom properties the output carries, and the checks a name passes
//! before it is written.
//!
//! The **HTML contract** is the set of class names and attributes a stylesheet
//! or a script may rely on in the HTML backend's output, stated as data in
//! [`CONVENTIONS`](super::CONVENTIONS). Its names are not written down one by
//! one: the four functions below derive them from the names the presentation
//! vocabulary already has, so that a class cannot drift from the element it
//! represents.
//!
//! - [`class_of_element`] — the class every node of a presentation element
//!   carries: the element's name with every hyphen doubled and every dot turned
//!   into one hyphen, so `flm.pres.block-heading` is written
//!   `flm-pres-block--heading`.
//! - [`class_of_role`] — the second class, for the payload's **role**: the
//!   dotted, feature-owned name saying what a container is
//!   (`flm.heading.number`; see [decorations, roles and
//!   kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)),
//!   rewritten the same way (`flm-heading-number`).
//! - [`data_attribute`] — the attribute one other payload field is written
//!   under: [`DATA_PREFIX`] followed by the field's name with `_` and `.`
//!   turned into hyphens, so the field `source_span` is written
//!   `data-flm-source-span`.
//! - [`style_property`] — the custom property a payload's **length** field is
//!   written as, in the element's `style` attribute: [`STYLE_PROPERTY_PREFIX`]
//!   followed by the field's name turned by the same rule, so the field
//!   `indent_left` is written `--flm-element-indent-left`.
//!
//! The [conventions table](super::conventions), not these functions, decides
//! which field becomes which attribute and which elements carry a role: these
//! functions only say how a name is written once the table has chosen it.
//!
//! [`is_valid_token`] is the check a role and a kind token pass before they
//! are written: the `class` attribute and `data-flm-kind` both hold a list of
//! tokens separated by whitespace, so a name holding whitespace would be read
//! as several of them.
//!
//! The two remaining functions, [`is_valid_tag_name`] and
//! [`is_valid_attribute_name`], are the check applied to a tag or attribute
//! name that comes from a [construct node](super::construct) (a node that a
//! lowering rule stages, that is, builds into the lowered tree, to write one
//! HTML tag of its own; see [`Lowerer`](crate::core::lowering::Lowerer)) or
//! from the render options. Such a name is chosen by the rule's author or the host, so it is
//! checked before it is written; the names this module derives pass the
//! check by construction.

/// The prefix of every attribute this backend writes for a payload field that
/// is not one of HTML's own attributes: `data-flm-`.
///
/// `data-*` is HTML's own space for attributes an application defines, so an
/// attribute written under this prefix is valid HTML and collides with nothing
/// a browser or another library defines. See [`data_attribute`].
pub const DATA_PREFIX: &str = "data-flm-";

/// The prefix of every custom property this backend writes in an element's
/// `style` attribute for a payload's length field: `--flm-element-`.
///
/// A custom property is a CSS variable: it sets nothing by itself, and a
/// stylesheet rule applies it (`padding-left:
/// var(--flm-element-indent-left, 0)`), so a page's own rules remain free to
/// apply it differently. See [`style_property`].
pub const STYLE_PROPERTY_PREFIX: &str = "--flm-element-";

/// A dotted name as one class name: every `-` becomes `--`, and every `.`
/// becomes `-`.
///
/// The two replacements happen together, in one pass over the characters, so
/// that a hyphen the name already held is doubled and a hyphen written for a
/// dot is not.
///
/// **A class produced here reads back to exactly one name**: reading the class
/// from left to right, a single hyphen was a dot and a double hyphen was a
/// hyphen, so `flm-pres-block--heading` can only have come from
/// `flm.pres.block-heading`. That property holds for a **well-formed dotted
/// name**, which is one whose segments — the parts between the dots — are
/// non-empty and neither begin nor end with a hyphen. A name that breaks the
/// condition can share its class with another one: `flm.pres.-x` and
/// `flm.pres-.x` are both written `flm-pres---x`, and `flm.pres..x` is written
/// `flm-pres--x`, the class of the well-formed name `flm.pres-x`. **Nothing
/// checks the condition**: the names of this library's own vocabulary and the
/// dotted names features declare satisfy it by the way dotted names are
/// written, and a check at every class would cost more than it is worth.
fn dotted_as_class(name: &str) -> String {
    let mut class = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '-' => class.push_str("--"),
            '.' => class.push('-'),
            _ => class.push(ch),
        }
    }
    class
}

/// Returns the class a node of the presentation element `element` carries: the
/// element's name with every hyphen doubled and every dot turned into one
/// hyphen.
///
/// The doubling keeps the class readable backwards: a single hyphen in the
/// class represents a dot in the name and a double hyphen a hyphen, so
/// one class can come from one element name only. That holds for a
/// **well-formed dotted name** — one whose segments, the parts between the
/// dots, are non-empty and neither begin nor end with a hyphen — and **nothing
/// checks the condition**, neither here nor at the registry: a name that
/// breaks it can share its class with another one (`flm.pres.-x` and
/// `flm.pres-.x` are both written `flm-pres---x`; `flm.pres..x` is written
/// `flm-pres--x`, the class of `flm.pres-x`). A feature
/// author who writes names the way dotted names are written never breaks it.
/// [`class_of_role`] applies the same rewriting to a role.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::class_of_element;
/// use flm_core::stdcore::backends::pres;
///
/// assert_eq!(class_of_element(pres::BLOCK_HEADING), "flm-pres-block--heading");
/// assert_eq!(class_of_element(pres::PAR), "flm-pres-par");
/// assert_eq!(class_of_element(pres::DECO_RUNIN), "flm-pres-deco-runin");
/// ```
pub fn class_of_element(element: &str) -> String {
    dotted_as_class(element)
}

/// Returns the class a node carries for its payload's role: the role's name with every
/// hyphen doubled and every dot turned into one hyphen.
///
/// A **role** is a feature-owned dotted name on a presentation payload that
/// says what a container is (a theorem's heading, a quotation's attribution;
/// see [`payload`](crate::stdcore::backends::pres::payload)). Because the
/// feature that introduced it owns the name, two features cannot declare the
/// same role by accident. The class is the same rewriting [`class_of_element`]
/// applies, applied to the role's name.
///
/// A class reads back to exactly one role name when the name is well formed;
/// [`class_of_element`] states the condition, which nothing checks.
///
/// Because a stylesheet selects on these classes, **a role's name is a public
/// interface**: renaming a role changes the class, and every stylesheet written
/// against the old one stops matching. The same is true of a presentation
/// element's name.
///
/// A role is one class and no more, which holds as long as the role's name is
/// one token: a name holding whitespace would be read as several classes,
/// because that is how the `class` attribute separates them. Nothing in the
/// library's name type enforces that, so the emitter checks a role with
/// [`is_valid_token`] before it writes the class and refuses a name that fails
/// — see that function for what is refused and what the emitter does instead.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::class_of_role;
///
/// assert_eq!(class_of_role("flm.heading"), "flm-heading");
/// ```
pub fn class_of_role(role: &str) -> String {
    dotted_as_class(role)
}

/// Returns whether `name` may be written as **one token**: one class among the
/// classes of an element, or one token of the `data-flm-kind` attribute.
///
/// Both of those attributes hold a list of tokens separated by whitespace, so
/// a name holding whitespace would be read as several tokens, and an empty
/// name as none: a node whose role is `flm.a b` would carry the classes
/// `flm-a` and `b`, the second representing nothing. A name that passes this
/// check is one token wherever it is written.
///
/// Such a name comes from a feature author, not from a document: it is the
/// role or the kind a lowering rule put in a presentation node's payload, and
/// the library's name type accepts any text. The emitter therefore checks it
/// with this function, leaves out the class or the token that fails, and
/// reports it as `flm.html.invalid-name`
/// ([`InvalidName`](super::emit::InvalidName)); everything else about the node
/// is written as usual.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::is_valid_token;
///
/// assert!(is_valid_token("flm.heading"));
/// assert!(is_valid_token("enumerate"));
/// assert!(!is_valid_token(""));
/// assert!(!is_valid_token("flm.a b"));
/// ```
pub fn is_valid_token(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(char::is_whitespace)
}

/// Returns the attribute name a payload field is written under: [`DATA_PREFIX`]
/// followed by `field` with every `_` and `.` turned into `-`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::data_attribute;
///
/// assert_eq!(data_attribute("level"), "data-flm-level");
/// assert_eq!(data_attribute("source_span"), "data-flm-source-span");
/// ```
pub fn data_attribute(field: &str) -> String {
    prefixed_field(DATA_PREFIX, field)
}

/// Returns the custom property a payload's length field is written as, in the
/// element's `style` attribute: [`STYLE_PROPERTY_PREFIX`] followed by `field`
/// turned into a name by the rule of [`data_attribute`], every `_` and `.`
/// turned into `-`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::style_property;
///
/// assert_eq!(style_property("indent_left"), "--flm-element-indent-left");
/// assert_eq!(style_property("width"), "--flm-element-width");
/// ```
pub fn style_property(field: &str) -> String {
    prefixed_field(STYLE_PROPERTY_PREFIX, field)
}

/// `prefix` followed by `field` with every `_` and `.` turned into `-`: the
/// only rule by which a payload field's name becomes a name on the page.
fn prefixed_field(prefix: &str, field: &str) -> String {
    let mut name = String::with_capacity(prefix.len() + field.len());
    name.push_str(prefix);
    for ch in field.chars() {
        name.push(if ch == '_' || ch == '.' { '-' } else { ch });
    }
    name
}

/// Returns whether `name` may be written as a tag name: an ASCII letter followed by
/// any number of ASCII letters, digits and `-`.
///
/// This is narrower than HTML's own rule, so that the check stays simple and
/// safe. It admits every element name HTML has and every custom element name
/// written in ASCII (which must hold a `-`), and it admits nothing that could
/// end the tag early or start something else (no space, no `/`, no `>`, no
/// `<`, nothing outside ASCII), so a name that passes is written unchanged and
/// the tag it opens is the tag that was asked for.
///
/// A name reaches this check from a [construct node](super::construct)'s
/// payload, which a lowering rule wrote, or from the tag a host maps a heading
/// kind to ([`HtmlRenderOptions::heading_tags`](super::HtmlRenderOptions::heading_tags));
/// the emitter checks it before opening the tag and reports a name that fails
/// instead of writing it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::is_valid_tag_name;
///
/// assert!(is_valid_tag_name("div"));
/// assert!(is_valid_tag_name("tex-html"));
/// assert!(!is_valid_tag_name(""));
/// assert!(!is_valid_tag_name("2div"));
/// assert!(!is_valid_tag_name("div onclick=x"));
/// assert!(!is_valid_tag_name("div>"));
/// ```
pub fn is_valid_tag_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

/// Returns whether `name` may be written as an attribute name: an ASCII letter
/// followed by any number of ASCII letters, digits, `-`, `_`, `:` and `.`.
///
/// It is [`is_valid_tag_name`]'s set with the three characters attribute names
/// carry in practice — `data-flm-level`, `aria-label`, `xml:lang`,
/// `data.pinned` — and, like it, admits nothing that could end the attribute
/// or start another: no space, no `=`, no quotation mark, no `/`, no `>`.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::names::is_valid_attribute_name;
///
/// assert!(is_valid_attribute_name("data-flm-level"));
/// assert!(is_valid_attribute_name("xml:lang"));
/// assert!(!is_valid_attribute_name("href=\"x\" onclick"));
/// assert!(!is_valid_attribute_name("-hidden"));
/// ```
pub fn is_valid_attribute_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::pres;

    /// The other direction of the mapping, written only here: a class back to
    /// the name it came from. A double hyphen was a hyphen, a single one a dot.
    fn name_of_class(class: &str) -> String {
        let mut name = String::with_capacity(class.len());
        let mut chars = class.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '-' {
                if chars.peek() == Some(&'-') {
                    chars.next();
                    name.push('-');
                } else {
                    name.push('.');
                }
            } else {
                name.push(ch);
            }
        }
        name
    }

    #[test]
    fn every_element_of_the_vocabulary_has_a_class_of_its_own() {
        let mut classes: Vec<String> = pres::ALL.iter().map(|e| class_of_element(e)).collect();
        classes.sort();
        let count = classes.len();
        classes.dedup();
        assert_eq!(classes.len(), count, "two elements share one class");
        assert!(classes.iter().all(|class| !class.contains('.')));
        assert!(classes.iter().all(|class| is_valid_attribute_name(class)));
    }

    #[test]
    fn a_hyphen_is_doubled_and_a_dot_becomes_one_hyphen() {
        assert_eq!(class_of_element(pres::BLOCK_HEADING), "flm-pres-block--heading");
        assert_eq!(class_of_element(pres::MATH_PASSTHROUGH), "flm-pres-math--passthrough");
        assert_eq!(class_of_element(pres::MATH_TEXT), "flm-pres-math--text");
        assert_eq!(class_of_element(pres::DECO_MARK), "flm-pres-deco--mark");
        assert_eq!(class_of_element(pres::DECO_RUNIN), "flm-pres-deco-runin");
        assert_eq!(class_of_element(pres::PAR), "flm-pres-par");
        assert_eq!(class_of_role("flm.heading"), "flm-heading");
    }

    #[test]
    fn a_class_reads_back_to_exactly_one_name() {
        // Every element of the vocabulary, and every role the standard package
        // writes — today the one role of the headings feature.
        let roles = [crate::stdcore::features::headings::numbers::NUMBER_ROLE];
        for name in pres::ALL.iter().chain(roles.iter()) {
            assert_eq!(&name_of_class(&class_of_element(name)), name, "{name:?}");
            assert_eq!(&name_of_class(&class_of_role(name)), name, "{name:?}");
        }
        // And a feature author's own well-formed names.
        for name in ["mypkg.theorem.body", "mypkg.run-in", "a-b.c-d.e"] {
            assert_eq!(name_of_class(&class_of_role(name)), name, "{name:?}");
        }
    }

    #[test]
    fn a_role_becomes_the_same_kind_of_class() {
        assert_eq!(class_of_role("flm.heading"), "flm-heading");
        assert_eq!(class_of_role("mypkg.theorem.body"), "mypkg-theorem-body");
        assert_eq!(class_of_role("mypkg.run-in.body"), "mypkg-run--in-body");
    }

    #[test]
    fn a_name_that_would_be_read_as_several_tokens_is_not_one() {
        // The vocabulary's own names, and every role and kind spelled the way
        // a dotted name is spelled, are one token.
        for name in ["flm.heading", "enumerate", "mypkg.theorem.body", "ref"] {
            assert!(is_valid_token(name), "{name:?}");
        }
        // Whitespace separates tokens in a class attribute and in
        // `data-flm-kind`, and an empty name is no token at all.
        for name in ["", " ", "flm.a b", "a\tb", "a\nb", "trailing "] {
            assert!(!is_valid_token(name), "{name:?}");
        }
        // Every element of the vocabulary passes it, as does every class
        // derived from one.
        for element in pres::ALL {
            assert!(is_valid_token(element), "{element:?}");
            assert!(is_valid_token(&class_of_element(element)), "{element:?}");
        }
    }

    #[test]
    fn a_field_becomes_a_data_attribute_with_hyphens() {
        assert_eq!(data_attribute("kind"), "data-flm-kind");
        assert_eq!(data_attribute("indent_right"), "data-flm-indent-right");
        assert_eq!(data_attribute("a.b_c"), "data-flm-a-b-c");
        assert!(is_valid_attribute_name(&data_attribute("indent_left")));
    }

    #[test]
    fn a_name_that_could_break_out_of_its_tag_is_refused() {
        for bad in ["", " div", "div ", "div/", "div>", "<div", "di v", "café", "d\"v", "-x", "1x"]
        {
            assert!(!is_valid_tag_name(bad), "{bad:?}");
            assert!(!is_valid_attribute_name(bad), "{bad:?}");
        }
        for good in ["a", "div", "tex-html", "h1", "myElement-x"] {
            assert!(is_valid_tag_name(good), "{good:?}");
            assert!(is_valid_attribute_name(good), "{good:?}");
        }
        // The three characters an attribute name may hold and a tag name may not.
        for name in ["xml:lang", "data_flm", "a.b"] {
            assert!(is_valid_attribute_name(name), "{name:?}");
            assert!(!is_valid_tag_name(name), "{name:?}");
        }
    }
}
