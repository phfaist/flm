//! Between features, the feature placed last wins: wherever registration
//! order picks one of several competing items that different features
//! contribute, the item of the feature placed last in the feature order is
//! taken, and within one feature, the feature's own order is kept.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use flm_core::core::registry::{
    FeatureChain, FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs,
};
use flm_core::core::services::Arity;
use flm_core::lang::FlmDiagnostics;
use flm_core::lang::mint::{ElementDef, SyntaxDef};
use flm_core::model::names::Name;
use flm_core::stdcore::backends::latex::preamble::{self, LatexPreambleDef, SLOT};
use flm_core::stdcore::backends::latex_rendered::{self, LatexRenderedRenderOptions};

use common::element;
use common::pipeline::feature;

/// Builds `features`, added in this order, with the host's `chains` and the
/// host's value `host` in the service slot `t.list`.
fn build(features: Vec<Arc<dyn FlmFeature>>, chains: &[&str], host: Option<&'static str>) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    for feature in features {
        builder.feature(feature);
    }
    for text in chains {
        builder.feature_order(FeatureChain::parse(text, "test").expect("a chain"));
    }
    if let Some(value) = host {
        builder.contribute_to("t.list", Arc::new(value.to_string()));
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

// --- elements ----------------------------------------------------------------------------------

/// Two features declaring one element value, each under a macro of its own.
fn sharing(chains: &[&str]) -> (Arc<FlmRegistry>, Arc<flm_core::model::element::ElementSpec>) {
    let shared = element("t.shared");
    let declare = |owner: &'static str, name: &'static str| {
        let value = Arc::clone(&shared);
        feature(owner, move |b| {
            b.element(ElementDef::new(owner, [name], SyntaxDef::Macro { args: Vec::new() }, Arc::clone(&value)))
        })
    };
    (build(vec![declare("t.a", "one"), declare("t.b", "two")], chains, None), shared)
}

fn owner_of(registry: &FlmRegistry, value: &Arc<flm_core::model::element::ElementSpec>) -> String {
    let idx = registry.elem_idx(value).expect("the value is registered");
    registry.element(idx).expect("an entry").owner.to_string()
}

#[test]
fn the_owner_of_an_element_value_two_features_declare_is_the_feature_placed_last() {
    let (registry, shared) = sharing(&[]);
    assert_eq!(owner_of(&registry, &shared), "t.b");
    assert_eq!(registry.elements_named("t.shared").len(), 1);
    let (registry, shared) = sharing(&["t.b < t.a"]);
    assert_eq!(owner_of(&registry, &shared), "t.a");
}

#[test]
fn a_lookup_by_type_name_takes_the_value_of_the_feature_placed_last() {
    let declare = |owner: &'static str, first: &'static str, second: &'static str| {
        feature(owner, move |b| {
            b.element(ElementDef::new(owner, [first], SyntaxDef::Macro { args: Vec::new() }, element("t.heading")))?;
            b.element(ElementDef::new(owner, [second], SyntaxDef::Macro { args: Vec::new() }, element("t.heading")))
        })
    };
    let features = || vec![declare("t.a", "afirst", "asecond"), declare("t.b", "bfirst", "bsecond")];
    let owner_of_named = |registry: &FlmRegistry| {
        let value = registry.element_named("t.heading").expect("a value");
        let idx = registry.elem_idx(value).expect("registered");
        // Of one feature's two values, the first in numbering order.
        let first_of_owner = registry
            .elements_named("t.heading")
            .iter()
            .copied()
            .find(|candidate| registry.element(*candidate).unwrap().owner == registry.element(idx).unwrap().owner)
            .expect("the owner's values");
        assert_eq!(first_of_owner, idx);
        registry.element(idx).unwrap().owner.to_string()
    };
    assert_eq!(owner_of_named(&build(features(), &[], None)), "t.b");
    assert_eq!(owner_of_named(&build(features(), &["t.b < t.a"], None)), "t.a");
}

// --- service slots ------------------------------------------------------------------------------

/// The feature declaring the slot `t.list` of strings.
fn list_owner() -> Arc<dyn FlmFeature> {
    feature("t.owner", |b| b.service_slot::<String>("t.list", Arity::OrderedMany, "a list"))
}

/// A feature contributing `values` to `t.list`, those ending in `!` with
/// `contribute_first_to`.
fn contributing(name: &'static str, values: &'static [&'static str]) -> Arc<dyn FlmFeature> {
    feature(name, move |b| {
        for value in values {
            if value.ends_with('!') {
                b.contribute_first_to("t.list", Arc::new(value.to_string()))?;
            } else {
                b.contribute_to("t.list", Arc::new(value.to_string()))?;
            }
        }
        Ok(())
    })
}

#[test]
fn a_slot_read_for_precedence_takes_the_feature_placed_last_first_and_the_host_last() {
    let features = || vec![list_owner(), contributing("t.a", &["a1", "a!", "a2"]), contributing("t.b", &["b1", "b!", "b2"])];
    let read = |registry: &FlmRegistry| -> (Vec<String>, Vec<String>, Option<String>) {
        let services = registry.services();
        (
            services.services_named::<String>("t.list").cloned().collect(),
            services.services_by_precedence_named::<String>("t.list").cloned().collect(),
            services.service_named::<String>("t.list").cloned(),
        )
    };
    let (assembled, ranked, one) = read(&build(features(), &[], Some("host")));
    // The assembled order is the list as contributed, the prepended values first.
    assert_eq!(assembled, ["a!", "b!", "a1", "a2", "b1", "b2", "host"]);
    assert_eq!(ranked, ["b!", "a!", "b1", "b2", "a1", "a2", "host"]);
    assert_eq!(one.as_deref(), Some("b!"));
    let (_, ranked, one) = read(&build(features(), &["t.b < t.a"], Some("host")));
    assert_eq!(ranked, ["a!", "b!", "a1", "a2", "b1", "b2", "host"]);
    assert_eq!(one.as_deref(), Some("a!"));
    // Without prepended values, the host's value comes after every feature's.
    let plain = || vec![list_owner(), contributing("t.a", &["a1"]), contributing("t.b", &["b1"])];
    let (_, ranked, one) = read(&build(plain(), &[], Some("host")));
    assert_eq!(ranked, ["b1", "a1", "host"]);
    assert_eq!(one.as_deref(), Some("b1"));
    assert_eq!(build(plain(), &[], Some("host")).services().service::<String>().map(String::as_str), Some("b1"));
}

// --- LaTeX definition pieces --------------------------------------------------------------------

/// The feature declaring the slot of definition pieces.
fn pieces_owner() -> Arc<dyn FlmFeature> {
    feature("t.latex", |b| b.service_slot::<LatexPreambleDef>(SLOT, Arity::OrderedMany, "pieces"))
}

/// A feature contributing the pieces `(name, content)`, in this order.
fn piece_feature(name: &'static str, pieces: &'static [(&'static str, &'static str)]) -> Arc<dyn FlmFeature> {
    feature(name, move |b| {
        for (piece, content) in pieces {
            b.contribute_to(SLOT, Arc::new(LatexPreambleDef::new(piece, *content, "a piece")))?;
        }
        Ok(())
    })
}

fn contents(pieces: &[&LatexPreambleDef]) -> Vec<String> {
    pieces.iter().map(|piece| piece.content.clone()).collect()
}

#[test]
fn the_piece_of_the_feature_placed_last_defines_an_identifier_at_its_own_place() {
    let features = || {
        vec![
            pieces_owner(),
            piece_feature("t.a", &[("shared", "% a's"), ("other", "% a's other"), ("shared", "% a's second")]),
            piece_feature("t.b", &[("before", "% b's before"), ("shared", "% b's")]),
        ]
    };
    let registry = build(features(), &[], None);
    let services = registry.services();
    let shared = preamble::piece_id("shared");
    assert_eq!(preamble::piece_named(services, shared.as_str()).map(|piece| piece.content.as_str()), Some("% b's"));
    assert_eq!(contents(&preamble::definitions(services)), ["% a's other", "% b's before", "% b's"]);
    // The latex-rendered backend writes the piece that defines the identifier.
    let mut diags = FlmDiagnostics::new();
    let required: BTreeSet<Name> = BTreeSet::from([shared.clone()]);
    let options = LatexRenderedRenderOptions::default();
    let resolved = latex_rendered::resolve(services, &registry, &required, &options, &mut diags);
    assert_eq!(contents(&resolved.pieces), ["% b's"]);

    // With the chain placing `t.b` first, `t.a`'s first piece defines it.
    let registry = build(features(), &["t.b < t.a"], None);
    let services = registry.services();
    assert_eq!(preamble::piece_named(services, shared.as_str()).map(|piece| piece.content.as_str()), Some("% a's"));
    assert_eq!(contents(&preamble::definitions(services)), ["% b's before", "% a's", "% a's other"]);
}

#[test]
fn one_piece_contributed_several_times_defines_its_identifier_once() {
    // One value contributed twice by `t.a` and once by `t.b`: it is written
    // once, at the first of its places.
    let piece = Arc::new(LatexPreambleDef::new("shared", "% the shared piece", "a piece"));
    let contributing = |name: &'static str, times: usize| {
        let piece = Arc::clone(&piece);
        feature(name, move |b| {
            for _ in 0..times {
                b.contribute_to(SLOT, Arc::clone(&piece))?;
                b.contribute_to(SLOT, Arc::new(LatexPreambleDef::new(name, format!("% {name}'s own"), "a piece")))?;
            }
            Ok(())
        })
    };
    let registry = build(vec![pieces_owner(), contributing("t.a", 2), contributing("t.b", 1)], &[], None);
    let services = registry.services();
    assert_eq!(contents(&preamble::definitions(services)), ["% the shared piece", "% t.a's own", "% t.b's own"]);
    // The latex-rendered backend writes it once.
    let mut diags = FlmDiagnostics::new();
    let required: BTreeSet<Name> = BTreeSet::from([preamble::piece_id("shared")]);
    let options = LatexRenderedRenderOptions::default();
    let resolved = latex_rendered::resolve(services, &registry, &required, &options, &mut diags);
    assert_eq!(contents(&resolved.pieces), ["% the shared piece"]);
}
