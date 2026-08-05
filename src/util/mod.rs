//! Shared utilities: token cursor scanning ([`Cursor`]) and compile-time diagnostic
//! construction ([`compile_error_str`]).
//!
//! This directory has no business dependencies and is referenced by every layer; `mod.rs`
//! aggregates the re-exports, so callers write `crate::util::X` (not submodule paths).

pub(crate) mod diagnostic;
pub(crate) mod scan;

pub(crate) use diagnostic::*;
pub(crate) use scan::*;
