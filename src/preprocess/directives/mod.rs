//! The `#` directive system — `#fill` / `#delegate` / `#blanket` and the open
//! extension (`#name(args){body}` → top-level macro call).
//!
//! Files:
//! - [`name_list`] — directive argument name lists (`@all` markers,
//!   `-name` / `-[a, b]` subtraction);
//! - [`trait_items`] — trait item lookups (`#name` / `#fill` / `#delegate`
//!   resolve item signatures from the annotated trait) plus the `@all`-family
//!   marker specs;
//! - [`delegate_args`] — delegate argument forwarding patterns;
//! - [`blanket`] — `#blanket` expansion (wrapper matrix → delegation specs);
//! - [`blanket_wrappers`] — blanket wrapper parsing (`wrapper^T` forms).

mod blanket;
mod blanket_wrappers;
mod delegate_args;
mod name_list;
mod trait_items;

pub(crate) use blanket::expand_blanket;
pub(crate) use blanket_wrappers::*;
pub(crate) use delegate_args::*;
pub(crate) use name_list::*;
pub(crate) use trait_items::*;
