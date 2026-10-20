//! The deterministic hasher behind every digest this crate computes
//! ([`Fnv1a64`]), and [`digest_of`], which digests any hashable value with it.
//!
//! A **digest** is a 64-bit number computed from a set of inputs, which the
//! library uses to tell whether two sets of inputs are the same. A digest must
//! come out the same in every process and in every build of the same inputs,
//! so that a number computed in one program can be compared with a number
//! computed in another. The standard library's default hasher gives no such
//! guarantee: its keys are randomized per process and its algorithm is not
//! specified. This module therefore implements its own hasher, [`Fnv1a64`],
//! the 64-bit variant of the Fowler-Noll-Vo hash (FNV-1a), and every digest of
//! this crate is computed with it from data written out field by field.
//!
//! The library computes these digests with it:
//!
//! - **The registry digest**, a number for everything about a registry that can
//!   change what processing produces, read with
//!   [`FlmRegistry::digest`](crate::core::registry::FlmRegistry::digest). It is
//!   computed when the registry is built ([`crate::core::registry::build`])
//!   from these inputs and no others: the versions of this crate and of techy,
//!   the build identity the embedding program gives
//!   ([`RegistryInputs::build_identity`](crate::core::registry::RegistryInputs::build_identity)),
//!   the parsing configuration, the structural elements and their aspects,
//!   each feature's name and version, the element and scoped-package
//!   declarations, the fallback declarations
//!   ([`FeatureBuilder::fallback`](crate::core::registry::FeatureBuilder::fallback))
//!   and override declarations
//!   ([`FeatureBuilder::override_callable`](crate::core::registry::FeatureBuilder::override_callable)),
//!   the rewrite stages with the tables they publish and the capabilities those
//!   tables refine, the ordering constraints between stages, the
//!   option declarations, and every feature's own [digest
//!   input](crate::core::registry::FeatureBuilder::digest_input). What a stage
//!   computes and what a backend emits are not inputs: they follow from these.
//! - **The digest of option values**: each option value is digested with
//!   [`digest_of`], and a set of values with [`OptionValues::digest`] and
//!   [`ResolvedOptions::digest`](crate::model::options::ResolvedOptions::digest).
//! - **The identity of a plan**, [`PlanKey`](crate::core::plan::PlanKey), which
//!   combines the registry digest with what the plan request asks for,
//!   including the digest of its render option values.
//! - **The stamp of an external table**, [`Stamp`](crate::core::run::Stamp),
//!   which records the registry digest, the rewrite stages the plan schedules
//!   and a digest of the option values they read, so that a run refuses an external
//!   table produced under a different registry or a different reading of the
//!   document.
//! - **Names derived from content**: the name of the LaTeX style file
//!   ([`content_hash`](crate::stdcore::backends::latex::style::content_hash))
//!   and the names of the image copies the graphics feature publishes.
//!
//! # Position of this module
//!
//! This module depends on the standard library alone. It is part of
//! [`crate::model`] because [`crate::model::options`] digests option values
//! with it; the other digests are computed in [`crate::core`], one layer up.
//!
//! [`OptionValues::digest`]: crate::model::options::OptionValues::digest

use std::hash::{Hash, Hasher};

/// The 64-bit Fowler-Noll-Vo hasher (FNV-1a): it gives the same result for the
/// same input in every process, in every build and on every architecture, and
/// is fast to compute.
///
/// Create one with [`Fnv1a64::new`], feed it data, and read the digest with
/// [`value`](Fnv1a64::value). Data is fed either through the inherent methods
/// [`write_bytes`](Fnv1a64::write_bytes), [`write_str`](Fnv1a64::write_str)
/// and [`write_u64`](Fnv1a64::write_u64), or through the standard [`Hash`]
/// trait (`value.hash(&mut hasher)`), since this type implements [`Hasher`].
/// [`digest_of`] does the latter for one value.
///
/// Every integer fed through the [`Hasher`] implementation, including the
/// length that a slice's `Hash` implementation writes, is fed as
/// little-endian bytes, and a `usize` or `isize` is widened to 64 bits first.
/// The same value therefore gives the same digest on a big-endian or a 32-bit
/// machine as on any other.
///
/// The hasher is not meant to resist deliberate collisions; it serves to
/// recognize the same inputs again, not to authenticate them.
///
/// # Examples
///
/// ```
/// use flm_core::model::digest::Fnv1a64;
///
/// let mut hasher = Fnv1a64::new();
/// hasher.write_str("flm.refs");
/// hasher.write_u64(3);
/// let digest = hasher.value();
///
/// let mut again = Fnv1a64::new();
/// again.write_str("flm.refs");
/// again.write_u64(3);
/// assert_eq!(digest, again.value());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    /// A hasher that has been fed nothing: its [`value`](Fnv1a64::value) is the
    /// FNV-1a offset basis, `0xcbf29ce484222325`.
    pub const fn new() -> Fnv1a64 {
        Fnv1a64(Self::OFFSET)
    }

    /// Feeds `bytes`, in order. Every other method that feeds data, the
    /// [`Hasher`] methods included, is built on this one.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    /// Feeds the bytes of `text`, then the byte `0xff`, which never occurs in
    /// UTF-8 text.
    ///
    /// The final byte marks where the string ends, so two strings fed one
    /// after the other do not digest like another pair with the same
    /// concatenation: `"ab"` then `"c"` differs from `"a"` then `"bc"`.
    pub fn write_str(&mut self, text: &str) {
        self.write_bytes(text.as_bytes());
        self.write_bytes(&[0xff]);
    }

    /// Feeds `value` as eight little-endian bytes, so that the digest does not
    /// depend on the machine's byte order.
    pub fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    /// The digest of everything fed so far.
    ///
    /// Reading the digest changes nothing: more data can be fed afterwards,
    /// and the next reading covers everything fed. [`Hasher::finish`] returns
    /// the same number.
    pub const fn value(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a64 {
    fn default() -> Fnv1a64 {
        Fnv1a64::new()
    }
}

impl Hasher for Fnv1a64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.write_bytes(bytes);
    }

    // The provided methods of `Hasher` would feed native-endian bytes and a
    // host-sized `usize`; these feed little-endian bytes of a fixed width.
    fn write_u8(&mut self, i: u8) {
        self.write_bytes(&[i]);
    }

    fn write_u16(&mut self, i: u16) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_u32(&mut self, i: u32) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_u64(&mut self, i: u64) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_u128(&mut self, i: u128) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_usize(&mut self, i: usize) {
        self.write_bytes(&(i as u64).to_le_bytes());
    }

    fn write_i8(&mut self, i: i8) {
        self.write_bytes(&[i as u8]);
    }

    fn write_i16(&mut self, i: i16) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_i32(&mut self, i: i32) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_i64(&mut self, i: i64) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_i128(&mut self, i: i128) {
        self.write_bytes(&i.to_le_bytes());
    }

    fn write_isize(&mut self, i: isize) {
        self.write_bytes(&(i as i64).to_le_bytes());
    }
}

/// The FNV-1a digest of any hashable value: a fresh [`Fnv1a64`] fed through
/// the value's own [`Hash`] implementation.
///
/// Two values that feed the same data to a hasher give the same digest, in
/// every process, in every build and on every architecture. The library
/// digests every option value this way
/// ([`OptionValues::set`](crate::model::options::OptionValues::set)).
///
/// # Examples
///
/// ```
/// use flm_core::model::digest::digest_of;
///
/// assert_eq!(digest_of("width=80"), digest_of("width=80"));
/// assert_ne!(digest_of("ab"), digest_of("ba"));
/// ```
pub fn digest_of<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut hasher = Fnv1a64::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_is_stable_and_order_sensitive() {
        assert_eq!(digest_of("a"), digest_of("a"));
        assert_ne!(digest_of("ab"), digest_of("ba"));
        let mut split = Fnv1a64::new();
        split.write_str("a");
        split.write_str("b");
        let mut joined = Fnv1a64::new();
        joined.write_str("ab");
        assert_ne!(split.value(), joined.value());
    }

    #[test]
    fn integers_hash_as_little_endian_bytes_of_a_fixed_width() {
        let mut expected = Fnv1a64::new();
        expected.write_bytes(&[0x02, 0x01]);
        assert_eq!(digest_of(&0x0102u16), expected.value());
        let mut widened = Fnv1a64::new();
        widened.write_bytes(&[1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(digest_of(&1usize), widened.value());
        assert_eq!(digest_of(&1usize), digest_of(&1u64));
    }

    #[test]
    fn the_empty_digest_is_the_offset_basis() {
        assert_eq!(Fnv1a64::new().value(), 0xcbf2_9ce4_8422_2325);
    }
}
