//! Semantic analysis of trait definitions: generic bound collection and where-predicate
//! reference collection.
//!
//! Extracts structured info ([`TraitBounds`]) from `syn::ItemTrait`, serving codegen's
//! bound inheritance, preprocess's `A<>` copying, and `#blanket` generic-arg reuse.
//! `mod.rs` aggregates the re-exports; callers write `crate::analyze::X`.

pub(crate) mod trait_bounds;

pub(crate) use trait_bounds::*;
