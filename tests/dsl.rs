//! Functional regression tests for the batch-impl DSL.
//!
//! This crate used to hold ~2400 lines of tests; it was split into
//! per-feature modules under `tests/features/` (each under 350 lines) and
//! this file is now the thin entry mounting them. `cargo test --test dsl`
//! runs the whole suite.

mod features;
