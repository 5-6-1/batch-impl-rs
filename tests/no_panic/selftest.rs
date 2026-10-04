//! Self-tests for the guard's **detectors**.
//!
//! The walk is loud about its own health — the file floors, an unreadable file,
//! an empty skip set all fail it — but the detectors were not: a visitor arm
//! that stopped matching (a `syn` upgrade reshaping a node, a refactor narrowing
//! a `matches!`) would report nothing and the whole guard would pass vacuously.
//! That is the one failure mode its promise cannot tolerate, so every arm is fed
//! a synthetic production file (or macro body) here. Each positive control sits
//! next to its negative ones: a detector that fires on everything is as useless
//! as one that fires on nothing.
//!
//! **Recorded gap (second review, N2), not closed here**: a hand-written
//! *index* inside a macro token stream (`quote!(v[0])`). clippy's
//! `indexing_slicing` is a HIR lint and cannot see a macro body, and the token
//! scan looks for panic macros and `unwrap` / `expect` calls — not for `[…]`,
//! which cannot be told apart from an array type (`[u8; 4]`), an array literal
//! or an attribute without a parser. No such site exists in the tree (the
//! indexing census came back empty), so it is a coverage hole, not a live bug;
//! the negative control below pins today's behaviour.

use proc_macro2::TokenStream;

use super::guard::{Guard, scan_file};
use super::tokens::scan_macro_tokens;

/// Scans one synthetic source as if it were `src/synthetic.rs`.
fn scan(src: &str) -> Vec<String> {
    let parsed = syn::parse_file(src).expect("the fixture parses");
    let mut violations = vec![];
    scan_file(&parsed, "src/synthetic.rs".to_string(), &mut violations);
    violations
}

/// Scans one synthetic macro body.
fn scan_tokens(tokens: TokenStream) -> Vec<String> {
    let mut violations = vec![];
    let mut guard = Guard { file: "src/synthetic.rs".to_string(), violations: &mut violations };
    scan_macro_tokens(&tokens, &mut guard);
    violations
}

/// Whether any violation mentions `needle`.
fn reported(violations: &[String], needle: &str) -> bool {
    violations.iter().any(|v| v.contains(needle))
}

fn assert_reports(src: &str, needle: &str) {
    let violations = scan(src);
    assert!(reported(&violations, needle), "`{src}` reported {violations:?}, not `{needle}`");
}

fn assert_tokens_report(tokens: TokenStream, needle: &str) {
    let violations = scan_tokens(tokens);
    assert!(
        reported(&violations, needle),
        "the macro body reported {violations:?}, not `{needle}`"
    );
}

/// The detectors this guard promises, each with a source that has to trip it. The individual tests
/// below check one detector against its near-miss controls; this table checks the *inventory*, which
/// nothing did: probe C's G9 deleted `visit_expr_call` together with its two self-test assertions and
/// the suite stayed green while a live `Option::unwrap(o)` sat in production code. Adding a detector
/// means adding a row here; removing one means the row's source stops reporting.
const DETECTORS: &[(&str, &str)] = &[
    ("panic!", "fn f() { panic!(\"boom\"); }"),
    ("unwrap()", "fn f() { let _ = x.unwrap(); }"),
    ("expect()", "fn f() { let _ = x.expect(\"boom\"); }"),
    ("assert!", "fn f() { assert!(true); }"),
    ("assert_eq!", "fn f() { assert_eq!(1, 1); }"),
    ("unreachable!", "fn f() { unreachable!(); }"),
    ("todo!", "fn f() { todo!(); }"),
    ("unimplemented!", "fn f() { unimplemented!(); }"),
];

#[test]
fn every_promised_detector_is_live() {
    for (what, src) in DETECTORS {
        let violations = scan(src);
        assert!(
            !violations.is_empty(),
            "nothing reports `{what}` any more: `{src}` came back clean, so that arm of the visitor \
             has stopped matching (this is the failure the whole no-panic promise cannot tolerate)"
        );
    }
    // The count is the inventory: a detector quietly dropped from this table would otherwise take its
    // own evidence with it.
    assert_eq!(DETECTORS.len(), 8, "the detector inventory changed - say why, in this table");
}

/// Every panic macro the guard names, plus the exactness of the `#[cfg(test)]`
/// gate (the earlier version skipped `#[cfg(not(test))]` items too).
#[test]
fn panic_macros_are_detected_and_the_cfg_test_gate_is_exact() {
    for mac in [
        "assert",
        "assert_eq",
        "assert_ne",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
        "panic",
        "unreachable",
        "todo",
        "unimplemented",
    ] {
        let src = format!("fn f() {{ {mac}!(SIDE); }}");
        assert_reports(&src, &format!("`{mac}!` is a panic construct"));
    }
    // The gate skips a `#[cfg(test)]` item — and its whole subtree.
    assert!(scan("#[cfg(test)] mod tests { fn t() { assert!(true); } }").is_empty());
    assert!(scan("#[cfg(test)] fn f() { panic!(); }").is_empty());
    // …but only the exact predicate counts: production-only code is scanned.
    assert_reports("#[cfg(not(test))] fn f() { assert!(true); }", "`assert!`");
    assert_reports("#[cfg(all(test, unix))] fn f() { panic!(); }", "`panic!`");
}

/// `unwrap` / `expect` in both spellings, with the near-misses next to them.
#[test]
fn unwrap_and_expect_are_detected_in_both_spellings() {
    assert_reports("fn f(o: Option<u8>) -> u8 { o.unwrap() }", "`.unwrap(…)` can panic");
    assert_reports("fn f(o: Option<u8>) -> u8 { o.expect(\"x\") }", "`.expect(…)` can panic");
    // The qualified form is the one clippy's `unwrap_used` misses.
    assert_reports("fn f(o: Option<u8>) -> u8 { Option::unwrap(o) }", "qualified `::unwrap(…)`");
    assert_reports(
        "fn f(r: Result<u8, u8>) -> u8 { Result::expect(r, \"x\") }",
        "qualified `::expect(…)`",
    );
    // Negatives: a different method, a bare local helper, a literal, an index.
    assert!(scan("fn f(o: Option<u8>) -> u8 { o.unwrap_or(0) }").is_empty());
    assert!(scan("fn f(o: Option<u8>) -> u8 { o.unwrap_or_default() }").is_empty());
    assert!(scan("fn expect(x: u8) -> u8 { x } fn f() -> u8 { expect(1) }").is_empty());
    assert!(scan("fn f() -> &'static str { \"unwrap()\" }").is_empty());
    assert!(scan("fn f(v: &[u8]) -> u8 { v[0] }").is_empty());
}

/// The `allow` / `expect` family in every position the guard claims: the file's
/// own inner attribute, an item, a statement, a `cfg_attr` arm, and the blanket
/// silencers — plus the prose and literal near-misses.
#[test]
fn the_allow_family_is_detected_in_every_position() {
    let silenced = "silences the no-panic deny family";
    assert_reports("#![allow(clippy::unwrap_used)] fn f() {}", silenced);
    assert_reports("#[allow(clippy::expect_used)] fn f() {}", silenced);
    assert_reports("#[expect(clippy::unreachable)] fn f() {}", silenced);
    assert_reports("fn f() { #[allow(clippy::todo)] let x = 1; let _ = x; }", silenced);
    assert_reports("#[cfg_attr(unix, allow(clippy::unwrap_used))] fn f() {}", "inside `cfg_attr`");
    assert_reports("#[allow(clippy::indexing_slicing)] fn f() {}", "INDEXING_ALLOW_EXCEPTIONS");
    for blanket in ["all", "warnings", "restriction"] {
        assert_reports(
            &format!("#[allow({blanket})] fn f() {{}}"),
            "silences every lint on this item",
        );
        assert_reports(
            &format!("#[allow(clippy::{blanket})] fn f() {{}}"),
            "silences every lint on this item",
        );
    }
    // Negatives: an unrelated lint, a doc comment, a string literal.
    assert!(scan("#[allow(dead_code)] fn f() {}").is_empty());
    assert!(scan("#[allow(clippy::needless_return)] fn f() {}").is_empty());
    assert!(scan("/// mentions `allow(clippy::unwrap_used)` in prose\nfn f() {}").is_empty());
    assert!(scan("fn f() { let _ = \"#[allow(clippy::unwrap_used)]\"; }").is_empty());
}

/// Macro bodies: invisible to clippy (HIR) and to syn's default visitor, which
/// is why they have their own token scan.
#[test]
fn macro_token_streams_are_scanned() {
    assert_tokens_report(quote::quote!(x.unwrap()), ".unwrap(…) can panic (inside a macro token");
    assert_tokens_report(
        quote::quote!(o.expect("x")),
        ".expect(…) can panic (inside a macro token",
    );
    assert_tokens_report(
        quote::quote!(Option::unwrap(o)),
        "qualified `::unwrap(…)` call can panic",
    );
    assert_tokens_report(
        quote::quote!(panic!("x")),
        "`panic!` is a panic construct (inside a macro",
    );
    assert_tokens_report(
        quote::quote!(
            #[allow(clippy::unwrap_used)]
            fn f() {}
        ),
        "silences the no-panic deny family (inside a macro token stream)",
    );
    // A nested group is descended into (`quote!(fn f() { assert!(true); })`).
    assert_tokens_report(
        quote::quote!(
            fn f() {
                assert!(true);
            }
        ),
        "`assert!`",
    );
    // Negatives: a different method, a literal's text, and the recorded gap.
    assert!(scan_tokens(quote::quote!(x.unwrap_or(0))).is_empty());
    assert!(scan_tokens(quote::quote!(let s = "unwrap()";)).is_empty());
    assert!(scan_tokens(quote::quote!(v[0])).is_empty());
    // The gate still wins: a `#[cfg(test)]` item's macro body is not scanned.
    assert!(scan("#[cfg(test)] mod t { fn f() { quote!(x.unwrap()); } }").is_empty());
}
