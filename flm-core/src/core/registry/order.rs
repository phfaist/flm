//! The order of features: the chains that state it, and how the registry
//! build places the features in their final order.
//!
//! # Chains
//!
//! A **chain** is a statement that some features come in a given order:
//! the text `a < b < c` places the feature `a` before `b`, and `b` before
//! `c` ([`FeatureChain`]). Chains name features, never packages. A package
//! carries the chains it needs
//! ([`PackageEntry::feature_order`](super::PackageEntry::feature_order)), and
//! the embedding program gives the chains of a document
//! ([`RegistryBuilder::feature_order`](super::RegistryBuilder::feature_order)).
//! The build combines every chain it is given: the union of their statements.
//!
//! [`FeatureChain::parse`] reads a chain from its text and checks two things
//! only: that the text has the form `NAME < NAME < … < NAME`, with two or more
//! names, and that no name appears twice in it. Whether the names are features
//! of the registry, and whether the chains contradict each other, is known
//! only when the registry is built.
//!
//! # The final order
//!
//! The **implicit order** of the features is the order in which they were
//! added to the [`RegistryBuilder`](super::RegistryBuilder): each package after
//! the packages it uses, each package's features in the package's order. Before
//! the features register, the build places them in their **final order**:
//!
//! 1. Each chain is reduced to the features of the registry, in the order
//!    written: with no feature `x`, the chain `a < x < b` becomes `a < b`. A
//!    statement about a feature that is not in the registry is thus ignored, a
//!    misspelled name included.
//! 2. Two features that are consecutive in a reduced chain are ordered. A
//!    feature is **explicitly before** another when a sequence of such steps
//!    leads from the first to the second; the sequence passes only through
//!    features of the registry. The chains `a < x` and `x < b` place `a`
//!    before `b` only when `x` is a feature of the registry, while the single
//!    chain `a < x < b` places `a` before `b` in any case.
//! 3. The features are sorted so that each comes after every feature
//!    explicitly before it. At each step, among the features whose explicit
//!    predecessors are all placed, the one that comes first in the implicit
//!    order is placed next.
//!
//! The sort does not keep the implicit order of every pair of features that
//! the chains leave unordered: with the implicit order `x`, `y`, `z` and the
//! single chain `z < x`, the final order is `y`, `z`, `x`. Without any chain,
//! the final order is the implicit order.
//!
//! Chains that order a feature before itself, through features of the
//! registry, make the build fail with [`BuildError::FeatureOrderCycle`]; the
//! features then register in their implicit order, so that the first phase of
//! the build reports its other errors with the cycle. A cycle that passes
//! through a feature that is not in the registry disappears with the
//! reduction.
//!
//! The final order is the registration order everywhere: the features register
//! in that order, so it decides the order of the scope stack of the language
//! (where two features define one name, the definition of the feature placed
//! last applies), the numbering of what they declare, and the ties between
//! stages. Whether two features that define one name may do so without an
//! override depends on whether the feature placed last is explicitly after
//! the other (see [`crate::lang::mint::packages`]).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::fmt;
use std::sync::Arc;

use crate::core::registry::builder::BuildError;
use crate::core::registry::feature::FlmFeature;
use crate::model::names::FeatureName;

/// One chain of the feature order: feature names in the order they are to be
/// registered, and the text that says where the chain comes from.
///
/// The chain `a < b < c` states that the feature `a` comes before `b` and `b`
/// before `c`. The [module documentation](self) gives how the registry build
/// combines chains and places the features. Read a chain from its text with
/// [`FeatureChain::parse`].
///
/// `Display` writes the names separated by ` < `.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::FeatureChain;
///
/// let chain = FeatureChain::parse("math-mathjax < phfqit", "package phfpack")?;
/// assert_eq!(chain.names(), ["math-mathjax", "phfqit"]);
/// assert_eq!(chain.origin(), "package phfpack");
/// assert_eq!(chain.to_string(), "math-mathjax < phfqit");
/// # Ok::<(), flm_core::core::registry::ChainError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureChain {
    names: Vec<FeatureName>,
    origin: String,
}

impl FeatureChain {
    /// Reads a chain from `text`, a list of feature names separated by `<`,
    /// such as `math-mathjax < phfqit`; `origin` says where the chain comes
    /// from, for messages.
    ///
    /// A name is one or more characters, none of them whitespace or `<`.
    /// Whitespace around each `<` and at either end of the text is allowed.
    /// The function checks two things only: that the text has the form
    /// `NAME < NAME < … < NAME`, with two or more names, and that no name
    /// appears twice. It does not check that the names are features of any
    /// registry.
    ///
    /// The origin is a text the caller chooses, such as `package phfpack` or
    /// `front matter of doc.flm, line 4`; the registry build quotes it when
    /// the chain is part of a cycle ([`BuildError::FeatureOrderCycle`]).
    ///
    /// # Errors
    ///
    /// [`ChainError::NotAChain`] when the text does not have the form, and
    /// [`ChainError::RepeatedName`] when a name appears twice.
    pub fn parse(text: &str, origin: impl Into<String>) -> Result<FeatureChain, ChainError> {
        let mut names: Vec<FeatureName> = Vec::new();
        for piece in text.split('<') {
            let name = piece.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(ChainError::NotAChain { text: text.to_string() });
            }
            if names.iter().any(|seen| seen.as_str() == name) {
                return Err(ChainError::RepeatedName {
                    text: text.to_string(),
                    name: FeatureName::from(name),
                });
            }
            names.push(FeatureName::from(name));
        }
        if names.len() < 2 {
            return Err(ChainError::NotAChain { text: text.to_string() });
        }
        Ok(FeatureChain { names, origin: origin.into() })
    }

    /// The feature names, in the order the chain places them.
    pub fn names(&self) -> &[FeatureName] {
        &self.names
    }

    /// The text that says where the chain comes from, as given to
    /// [`FeatureChain::parse`] or [`FeatureChain::with_origin`].
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Returns the chain with the origin `origin` in place of its own: for a
    /// caller that reads a chain before it knows where the chain is written
    /// (the line of a file, the name of a package).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::registry::FeatureChain;
    ///
    /// let chain = FeatureChain::parse("math-mathjax < phfqit", "feature_order[1]")?;
    /// let chain = chain.with_origin("package phfpack");
    /// assert_eq!(chain.origin(), "package phfpack");
    /// assert_eq!(chain.to_string(), "math-mathjax < phfqit");
    /// # Ok::<(), flm_core::core::registry::ChainError>(())
    /// ```
    pub fn with_origin(self, origin: impl Into<String>) -> FeatureChain {
        FeatureChain { origin: origin.into(), ..self }
    }
}

impl fmt::Display for FeatureChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.names.iter().map(|name| name.as_str()).collect();
        f.write_str(&names.join(" < "))
    }
}

/// A text that [`FeatureChain::parse`] cannot read as a chain.
///
/// `Display` writes the message, which quotes the text as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// The text does not have the form `NAME < NAME < … < NAME` with two or
    /// more names: it holds a single name, an empty name, or a name with
    /// whitespace in it.
    NotAChain {
        /// The text as written.
        text: String,
    },
    /// The text names one feature twice.
    RepeatedName {
        /// The text as written.
        text: String,
        /// The name that appears twice.
        name: FeatureName,
    },
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainError::NotAChain { text } => write!(
                f,
                "the chain `{text}` must have the form `NAME < NAME < … < NAME`, with two or \
                 more feature names"
            ),
            ChainError::RepeatedName { text, name } => write!(
                f,
                "each feature must appear only once in the chain `{text}`, but `{name}` appears \
                 more than once"
            ),
        }
    }
}

impl std::error::Error for ChainError {}

/// The features of a build in their final order, and the explicit order
/// between them.
pub(crate) struct Placement {
    /// The features, in their final order.
    pub(crate) features: Vec<Arc<dyn FlmFeature>>,
    /// Every pair `(a, b)` of features such that `a` is explicitly before `b`.
    pub(crate) explicitly_before: BTreeSet<(FeatureName, FeatureName)>,
    /// [`BuildError::FeatureOrderCycle`] when the reduced chains order a
    /// feature before itself; the features then keep their implicit order.
    pub(crate) cycle: Option<BuildError>,
}

/// Places `features`, given in their implicit order, in their final order
/// under `chains`, as the [module documentation](self) describes.
///
/// When the reduced chains order a feature before itself, the placement keeps
/// the implicit order and carries [`BuildError::FeatureOrderCycle`]
/// ([`Placement::cycle`]). The error gives one cycle, from the feature of the
/// cycle that comes first in the implicit order, and every chain that orders
/// two consecutive features of the cycle.
pub(crate) fn place(features: Vec<Arc<dyn FlmFeature>>, chains: &[FeatureChain]) -> Placement {
    let names: Vec<FeatureName> = features.iter().map(|feature| feature.info().name).collect();
    let mut positions: BTreeMap<&FeatureName, Vec<usize>> = BTreeMap::new();
    for (position, name) in names.iter().enumerate() {
        positions.entry(name).or_default().push(position);
    }
    let reduced: Vec<Vec<&FeatureName>> = chains
        .iter()
        .map(|chain| chain.names.iter().filter(|name| positions.contains_key(name)).collect())
        .collect();

    let mut successors: BTreeMap<&FeatureName, BTreeSet<&FeatureName>> = BTreeMap::new();
    let mut preds: BTreeMap<usize, BTreeSet<usize>> =
        (0..names.len()).map(|position| (position, BTreeSet::new())).collect();
    for chain in &reduced {
        for pair in chain.windows(2) {
            successors.entry(pair[0]).or_default().insert(pair[1]);
            for &later in &positions[pair[1]] {
                let set = preds.get_mut(&later).expect("every position has an entry");
                set.extend(positions[pair[0]].iter().copied());
            }
        }
    }

    let nodes: Vec<usize> = (0..names.len()).collect();
    let (order, cycle) = match sort_by_priority(&nodes, &preds) {
        Ok(order) => (order, None),
        Err(cycle) => {
            let least = cycle
                .iter()
                .enumerate()
                .min_by_key(|(_, position)| **position)
                .map(|(at, _)| at)
                .unwrap_or(0);
            let mut cycle: Vec<&FeatureName> = cycle.iter().map(|position| &names[*position]).collect();
            cycle.rotate_left(least);
            let steps: Vec<(&FeatureName, &FeatureName)> = (0..cycle.len())
                .map(|at| (cycle[at], cycle[(at + 1) % cycle.len()]))
                .collect();
            let mut making: Vec<usize> = Vec::new();
            for step in &steps {
                for (index, chain) in reduced.iter().enumerate() {
                    let orders_step = chain.windows(2).any(|pair| (pair[0], pair[1]) == *step);
                    if orders_step && !making.contains(&index) {
                        making.push(index);
                    }
                }
            }
            let error = BuildError::FeatureOrderCycle {
                cycle: cycle.into_iter().cloned().collect(),
                chains: making.into_iter().map(|index| chains[index].clone()).collect(),
            };
            (nodes, Some(error))
        }
    };

    let mut explicitly_before: BTreeSet<(FeatureName, FeatureName)> = BTreeSet::new();
    for &start in successors.keys() {
        let mut stack: Vec<&FeatureName> = successors[start].iter().copied().collect();
        while let Some(next) = stack.pop() {
            if explicitly_before.insert((start.clone(), next.clone())) {
                if let Some(further) = successors.get(next) {
                    stack.extend(further.iter().copied());
                }
            }
        }
    }

    let mut slots: Vec<Option<Arc<dyn FlmFeature>>> = features.into_iter().map(Some).collect();
    let features = order
        .into_iter()
        .map(|position| slots[position].take().expect("each position is placed once"))
        .collect();
    Placement { features, explicitly_before, cycle }
}

/// Sorts `nodes` so that every node comes after the nodes `preds` gives for
/// it, taking at each step, among the nodes whose predecessors are all placed,
/// the one that comes first in `nodes` (Kahn's algorithm with a priority
/// queue). The registry build sorts features with it, and the plan builder
/// sorts stages with it.
///
/// `preds` holds an entry for every node of `nodes`. A predecessor that is not
/// among `nodes` is never placed, so a node that has one is never placed
/// either.
///
/// When some nodes cannot be placed, returns one cycle among them: each node
/// of the returned list comes before the next, and the last before the first.
/// The search starts from the least node left unplaced and follows, from each
/// node, its least predecessor left unplaced.
pub(crate) fn sort_by_priority<N: Ord + Copy>(
    nodes: &[N],
    preds: &BTreeMap<N, BTreeSet<N>>,
) -> Result<Vec<N>, Vec<N>> {
    let position: BTreeMap<N, usize> =
        nodes.iter().enumerate().map(|(at, node)| (*node, at)).collect();
    let mut waiting: Vec<usize> = nodes.iter().map(|node| preds[node].len()).collect();
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (at, node) in nodes.iter().enumerate() {
        for pred in &preds[node] {
            if let Some(&from) = position.get(pred) {
                successors[from].push(at);
            }
        }
    }
    let mut ready: BinaryHeap<Reverse<usize>> =
        (0..nodes.len()).filter(|at| waiting[*at] == 0).map(Reverse).collect();
    let mut order: Vec<N> = Vec::with_capacity(nodes.len());
    while let Some(Reverse(at)) = ready.pop() {
        order.push(nodes[at]);
        for &next in &successors[at] {
            waiting[next] -= 1;
            if waiting[next] == 0 {
                ready.push(Reverse(next));
            }
        }
    }
    if order.len() == nodes.len() {
        return Ok(order);
    }
    let placed: BTreeSet<N> = order.into_iter().collect();
    let live: BTreeSet<N> = nodes.iter().copied().filter(|node| !placed.contains(node)).collect();
    let start = *live.iter().next().expect("a cycle has nodes");
    let mut path: Vec<N> = vec![start];
    let mut seen: BTreeMap<N, usize> = BTreeMap::from([(start, 0)]);
    let mut cursor = start;
    loop {
        let next = preds[&cursor]
            .iter()
            .copied()
            .find(|pred| live.contains(pred))
            .expect("a node left unplaced has a predecessor left unplaced");
        if let Some(&at) = seen.get(&next) {
            let mut cycle: Vec<N> = path[at..].to_vec();
            cycle.reverse();
            return Err(cycle);
        }
        seen.insert(next, path.len());
        path.push(next);
        cursor = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::registry::builder::FeatureBuilder;
    use crate::core::registry::feature::FeatureInfo;

    struct Named(&'static str);

    impl FlmFeature for Named {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new(self.0, self.0)
        }

        fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            Ok(())
        }
    }

    fn features(names: &[&'static str]) -> Vec<Arc<dyn FlmFeature>> {
        names.iter().map(|name| Arc::new(Named(name)) as Arc<dyn FlmFeature>).collect()
    }

    fn chains(texts: &[&str]) -> Vec<FeatureChain> {
        texts
            .iter()
            .enumerate()
            .map(|(at, text)| FeatureChain::parse(text, format!("chain {at}")).expect("a chain"))
            .collect()
    }

    fn placed(names: &[&'static str], texts: &[&str]) -> Placement {
        let placement = place(features(names), &chains(texts));
        if let Some(error) = &placement.cycle {
            panic!("the features are placed: {error}");
        }
        placement
    }

    fn order_of(placement: &Placement) -> Vec<String> {
        placement.features.iter().map(|feature| feature.info().name.to_string()).collect()
    }

    fn before(placement: &Placement, a: &str, b: &str) -> bool {
        placement.explicitly_before.contains(&(FeatureName::from(a), FeatureName::from(b)))
    }

    #[test]
    fn a_chain_reads_names_separated_by_less_than_with_spaces_or_without() {
        for text in ["a < b < c", "a<b<c", "  a <b<   c ", "a\t<\nb < c"] {
            let chain = FeatureChain::parse(text, "here").expect("a chain");
            assert_eq!(chain.names(), ["a", "b", "c"], "{text}");
            assert_eq!(chain.to_string(), "a < b < c");
        }
        let chain = FeatureChain::parse("math-mathjax < phfqit", "package phfpack").expect("a chain");
        assert_eq!(chain.names(), ["math-mathjax", "phfqit"]);
        assert_eq!(chain.origin(), "package phfpack");
        // Any character but whitespace and `<` belongs to a name: no other
        // character is treated specially.
        let chain = FeatureChain::parse("a>b < c.d/e", "here").expect("a chain");
        assert_eq!(chain.names(), ["a>b", "c.d/e"]);
    }

    #[test]
    fn a_text_not_of_the_form_is_refused_with_the_form_expected() {
        for text in ["a", "", "  ", "a <", "< a", "a < < b", "a > b", "a, b", "a b < c", "a < b c"] {
            let error = FeatureChain::parse(text, "here").expect_err(text);
            assert_eq!(error, ChainError::NotAChain { text: text.to_string() });
            assert_eq!(
                error.to_string(),
                format!(
                    "the chain `{text}` must have the form `NAME < NAME < … < NAME`, with two \
                     or more feature names"
                )
            );
        }
    }

    #[test]
    fn a_name_written_twice_is_refused() {
        let error = FeatureChain::parse("a < b < a", "here").expect_err("a name twice");
        assert_eq!(error, ChainError::RepeatedName { text: "a < b < a".into(), name: "a".into() });
        assert_eq!(
            error.to_string(),
            "each feature must appear only once in the chain `a < b < a`, but `a` appears more than once"
        );
    }

    #[test]
    fn without_chains_the_final_order_is_the_implicit_order() {
        let placement = placed(&["c", "a", "b"], &[]);
        assert_eq!(order_of(&placement), ["c", "a", "b"]);
        assert!(placement.explicitly_before.is_empty());
    }

    #[test]
    fn features_not_loaded_drop_out_of_chains() {
        // Philippe's example: every feature loaded but `x`.
        let names = ["a", "b", "c", "e", "f", "z", "z2"];
        let placement = placed(&names, &["a < b < f < z", "a < x", "x < c", "b < z < z2", "z < e < z2"]);
        assert!(before(&placement, "a", "e"), "through `b` and `z`");
        assert!(!before(&placement, "a", "c") && !before(&placement, "c", "a"));
        assert!(before(&placement, "a", "z2") && before(&placement, "f", "e"));
        assert!(!before(&placement, "e", "f") && !before(&placement, "c", "z2"));
        assert_eq!(order_of(&placement), ["a", "b", "c", "f", "z", "e", "z2"]);
        // The chain through `x` says nothing about `a` and `c`; the single
        // chain `a < x < c` would.
        let placement = placed(&["c", "a"], &["a < x < c"]);
        assert!(before(&placement, "a", "c"));
        assert_eq!(order_of(&placement), ["a", "c"]);
    }

    #[test]
    fn ties_are_broken_by_the_implicit_order() {
        // The example of the guides: the chain `z < x` moves `x` after `z`,
        // so the pair `x`, `y` does not keep its implicit order.
        let placement = placed(&["x", "y", "z"], &["z < x"]);
        assert_eq!(order_of(&placement), ["y", "z", "x"]);
        // Features no chain names keep their implicit order around the others.
        let placement = placed(&["p", "b", "q", "a", "r"], &["a < b"]);
        assert_eq!(order_of(&placement), ["p", "q", "a", "b", "r"]);
        // A chain that agrees with the implicit order changes nothing.
        let placement = placed(&["a", "b", "c", "d"], &["a < c", "b < d"]);
        assert_eq!(order_of(&placement), ["a", "b", "c", "d"]);
    }

    #[test]
    fn a_cycle_among_loaded_features_names_the_cycle_and_its_chains() {
        let chains = vec![
            FeatureChain::parse("b < c", "package phfpack").expect("a chain"),
            FeatureChain::parse("p < q", "package other").expect("a chain"),
            FeatureChain::parse("c < x < a", "front matter of doc.flm, line 4").expect("a chain"),
            FeatureChain::parse("a < b", "package base").expect("a chain"),
        ];
        let placement = place(features(&["p", "a", "b", "c", "q"]), &chains);
        // The features keep their implicit order.
        assert_eq!(order_of(&placement), ["p", "a", "b", "c", "q"]);
        let error = placement.cycle.expect("a cycle is an error");
        match &error {
            BuildError::FeatureOrderCycle { cycle, chains } => {
                assert_eq!(cycle, &[FeatureName::from("a"), "b".into(), "c".into()]);
                let made: Vec<String> = chains.iter().map(|chain| chain.to_string()).collect();
                assert_eq!(made, ["a < b", "b < c", "c < x < a"]);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(
            error.to_string(),
            "the feature order must not contain a cycle, but the chains `a < b` (package base), \
             `b < c` (package phfpack), and `c < x < a` (front matter of doc.flm, line 4) order \
             the features in the cycle `a < b < c < a`"
        );
    }

    #[test]
    fn a_cycle_only_through_a_feature_not_loaded_disappears() {
        let placement = placed(&["a", "b"], &["a < x", "x < b", "b < a"]);
        assert_eq!(order_of(&placement), ["b", "a"]);
        assert!(before(&placement, "b", "a") && !before(&placement, "a", "b"));
    }

    #[test]
    fn the_priority_sort_takes_the_first_ready_node_and_finds_a_cycle() {
        let preds: BTreeMap<u32, BTreeSet<u32>> =
            BTreeMap::from([(1, BTreeSet::new()), (2, BTreeSet::from([3])), (3, BTreeSet::new())]);
        assert_eq!(sort_by_priority(&[1, 2, 3], &preds), Ok(vec![1, 3, 2]));
        let preds: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::from([
            (1, BTreeSet::new()),
            (2, BTreeSet::from([4])),
            (3, BTreeSet::from([2])),
            (4, BTreeSet::from([3])),
        ]);
        // 4 comes before 2, 2 before 3 and 3 before 4; the search starts from 2.
        assert_eq!(sort_by_priority(&[1, 2, 3, 4], &preds), Err(vec![3, 4, 2]));
    }
}
