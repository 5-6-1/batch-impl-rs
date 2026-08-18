//! Split test modules for the batch-impl functional/regression/Ext suites.
//!
//! The old single-file test crates (`dsl.rs`, `regression.rs`,
//! `ext1_impl.rs`, `ext2_impl.rs`) were split into per-feature modules here
//! (each under 350 lines) and mounted by the thin entry `tests/dsl.rs`
//! (`mod features;`). Every module is a self-contained `mod` — shared
//! helper types/macros live next to their tests.

pub(crate) mod dsl_at_refs;
pub(crate) mod dsl_basic;
pub(crate) mod dsl_blanket;
pub(crate) mod dsl_blanket_generic;
pub(crate) mod dsl_consts;
pub(crate) mod dsl_directives;
pub(crate) mod dsl_distribution;
pub(crate) mod dsl_entry_macros;
pub(crate) mod dsl_generic_families;
pub(crate) mod dsl_generics;
pub(crate) mod dsl_macro_meta;
pub(crate) mod dsl_open_extension;
pub(crate) mod dsl_operators;
pub(crate) mod dsl_receivers;
pub(crate) mod dsl_splat_advanced;
pub(crate) mod dsl_splat_basic;
pub(crate) mod dsl_trait_args;
pub(crate) mod dsl_where;
pub(crate) mod ext1_basic;
pub(crate) mod ext1_boundary;
pub(crate) mod ext1_conflicts;
pub(crate) mod ext1_nested;
pub(crate) mod ext1_trait_where;
pub(crate) mod ext2_basic;
pub(crate) mod ext2_boundary;
pub(crate) mod ext2_conflicts;
pub(crate) mod ext2_nested;
pub(crate) mod ext2_shape_forms;
pub(crate) mod regression_arrays_prefix;
pub(crate) mod regression_basics;
pub(crate) mod regression_consistency;
pub(crate) mod regression_macros_path;
