//! Split test modules for the batch-impl functional/regression/impl-entry
//! and shape-template suites.
//!
//! The old single-file test crates (`dsl.rs`, `regression.rs`,
//! `impl_entry_impl.rs`, `shape_template_impl.rs`) were split into per-feature modules here
//! (each under 350 lines) and mounted by the thin entry `tests/dsl.rs`
//! (`mod features;`). Every module is a self-contained `mod` — shared
//! helper types/macros live next to their tests.

pub(crate) mod block_model;
pub(crate) mod dsl_at_refs;
pub(crate) mod dsl_basic;
pub(crate) mod dsl_blanket;
pub(crate) mod dsl_blanket_generic;
pub(crate) mod dsl_bound_generator;
pub(crate) mod dsl_consts;
pub(crate) mod dsl_directives;
pub(crate) mod dsl_distribution;
pub(crate) mod dsl_dyn_for;
pub(crate) mod dsl_entry_macros;
pub(crate) mod dsl_generic_families;
pub(crate) mod dsl_generics;
pub(crate) mod dsl_macro_meta;
pub(crate) mod dsl_open_extension;
pub(crate) mod dsl_operators;
pub(crate) mod dsl_range_at;
pub(crate) mod dsl_receivers;
pub(crate) mod dsl_splat_advanced;
pub(crate) mod dsl_splat_basic;
pub(crate) mod dsl_trait_args;
pub(crate) mod dsl_where;
pub(crate) mod dup_params;
pub(crate) mod impl_entry_basic;
pub(crate) mod impl_entry_boundary;
pub(crate) mod impl_entry_conflicts;
pub(crate) mod impl_entry_extras;
pub(crate) mod impl_entry_nested;
pub(crate) mod impl_entry_trait_where;
pub(crate) mod regression_arrays_prefix;
pub(crate) mod regression_basics;
pub(crate) mod regression_consistency;
pub(crate) mod regression_macros_path;
pub(crate) mod shape_template_basic;
pub(crate) mod shape_template_boundary;
pub(crate) mod shape_template_conflicts;
pub(crate) mod shape_template_nested;
pub(crate) mod shape_template_shape_forms;
pub(crate) mod shape_template_trait_sync;
pub(crate) mod shape_template_varseg;
