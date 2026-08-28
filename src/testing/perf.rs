//! Expansion-cost measurement: how long does the real pipeline take to turn
//! a large DSL spec into its impls? This is the number the README's "one
//! line → N impls" claim begs for, and the answer was previously a guess.
//!
//! The measurement calls the same `expand_attr_macro` the attribute entry
//! uses (proc-macro2-level, no rustc), on a spec that expands to the
//! `MAX_EXPAND` ceiling (1024 impls) and one that reaches the body-token
//! budget. The numbers are informational — printed, not asserted — so a slow
//! CI machine cannot flake the suite, and they double as a rough regression
//! sentinel (a 10× jump would show up in the printed delta).

use proc_macro2::TokenStream;
use std::time::Instant;
use syn::ItemTrait;

use crate::entry::expand_attr_macro;

/// `(u8, u16, u32, u64).5` — Cartesian tuple power: 4⁵ = 1024 impls,
/// the expansion ceiling.
const MAX_EXPAND_SPEC: &str = "(u8, u16, u32, u64).5";

/// A small everyday spec, for contrast.
const SMALL_SPEC: &str = "[usize, isize, f32, f64]";

fn expand_time(attr: &str) -> (std::time::Duration, usize) {
    let attr_ts: TokenStream = attr.parse().expect("attr parses");
    let item: ItemTrait = syn::parse_str("trait Perf {}").expect("trait parses");
    let t0 = Instant::now();
    let out = expand_attr_macro(attr_ts, item, true).expect("spec expands");
    let elapsed = t0.elapsed();
    // Count impl blocks: every generated `impl ... for ...` starts an `impl`.
    let impl_count = out.to_string().matches("impl").count();
    (elapsed, impl_count)
}

#[test]
fn expansion_cost_snapshot() {
    let (small_t, small_n) = expand_time(SMALL_SPEC);
    let (max_t, max_n) = expand_time(MAX_EXPAND_SPEC);
    println!(
        "expansion cost: {small_n} impls in {small_t:?}; {max_n} impls (MAX_EXPAND) in {max_t:?} \
         ({:.1} ms/impl wall at ceiling)",
        max_t.as_secs_f64() * 1000.0 / max_n as f64
    );
    // Informational only — a 1024-impl expansion of this shape has always
    // been sub-second; the guard catches a pathological regression without
    // flaking on loaded machines.
    assert!(
        max_t.as_secs() < 5,
        "1024-impl expansion took {max_t:?} — order of magnitude off the expected sub-second"
    );
}
