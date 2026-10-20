//! Mirrors [`flm_core::stdcore::backends`]: the names of the presentation
//! elements and of the HTML construct, which a lowering rule's body stages,
//! and the lowered trees a backend's emit stage reads.

pub mod html;
pub mod latex;
pub mod lowering;
pub mod pres;
