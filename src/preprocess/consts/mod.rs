//! The `@` constant system — the macro-meta layer of the DSL.
//!
//! Built-in type-family constants (`@u*` / `@num` / `@u8..u128`), user-defined
//! constants (`batch_trait!` leading `@name=value;` segments), and the
//! where-predicate selectors (`@all_fresh` / `@N..M` are passed through here
//! and resolved by codegen). Files:
//! - [`table`] — built-in constant tables and the entry points
//!   (`expand_consts` / `collect_user_consts`);
//! - [`expand`] — per-`@` recognition ([`try_expand_at`]) and reference
//!   visibility validation ([`check_value_refs`]);
//! - [`ctx`] — expansion context ([`ExpandCtx`]) shared by both.

mod ctx;
mod expand;
mod table;

pub(crate) use ctx::*;
pub(crate) use expand::*;
pub(crate) use table::*;
