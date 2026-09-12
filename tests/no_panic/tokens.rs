//! Token-level scans for the no-panic guard: macro bodies and `cfg_attr` arms.
//!
//! Split out of the guard itself (per-file budget) and organized by the one thing
//! they have in common: `syn` gives no structure here, so every rule is
//! deliberately **shape-exact** and matches by `Ident` / `Punct` / `Group`
//! position instead of by text. A `Literal`'s contents — a doc string mentioning
//! `allow(clippy::unwrap_used)`, a string containing `unwrap()` — can therefore
//! never produce a hit.
//!
//! **Scope, stated because it is a hole rather than a decision**: this scan
//! covers the panic family (macros, `unwrap` / `expect`, `allow` / `expect`
//! arms) — not the **indexing** family. clippy's `indexing_slicing` is a HIR
//! lint and cannot see a macro body, and a `[…]` group here cannot be told apart
//! from an array type (`[u8; 4]`), an array literal or an attribute without a
//! parser, so a hand-written index inside `quote!(…)` would pass both legs. No
//! such site exists in the tree (the indexing census came back empty); the hole
//! is recorded in `selftest.rs` and the dev-changelog rather than papered over
//! with a heuristic that would fire on array types.
//!
//! Every rule here is exercised by a fixture in `selftest.rs` — a `syn` upgrade
//! or a narrowed `matches!` must fail that file, not quietly report nothing.

use super::guard::{BLANKET_ALLOWS, FORBIDDEN_ALLOWS, FORBIDDEN_MACROS, Guard, INDEXING_LINTS};

/// The `allow` / `expect` arms nested anywhere inside a `cfg_attr`'s token stream, as
/// `(kind, lint idents)`. Recurses through groups, so `cfg_attr(a, cfg_attr(b, allow(…)))`
/// is covered too; `Literal` tokens are skipped on purpose.
pub(super) fn cfg_attr_arms(tokens: &proc_macro2::TokenStream) -> Vec<(String, Vec<String>)> {
    use proc_macro2::{Delimiter, TokenTree};
    let trees = tokens.clone().into_iter().collect::<Vec<_>>();
    let mut out = vec![];
    let mut i = 0;
    while let Some(tree) = trees.get(i) {
        match tree {
            TokenTree::Ident(id) => {
                let name = id.to_string();
                if (name == "allow" || name == "expect")
                    && let Some(TokenTree::Group(group)) = trees.get(i + 1)
                    && group.delimiter() == Delimiter::Parenthesis
                {
                    let lints = group
                        .stream()
                        .into_iter()
                        .filter_map(|tree| match tree {
                            TokenTree::Ident(id) => Some(id.to_string()),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    out.push((name, lints));
                }
            }
            TokenTree::Group(group) => out.extend(cfg_attr_arms(&group.stream())),
            _ => {}
        }
        i += 1;
    }
    out
}

/// The `(full path, last segment)` pairs of one attribute's lint list, filtered to the lints
/// this guard cares about.
pub(super) fn nested_lints(attr: &syn::Attribute) -> Vec<(String, String)> {
    let mut out = vec![];
    let _ = attr.parse_nested_meta(|meta| {
        let full =
            meta.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");
        if let Some(ident) = meta.path.segments.last().map(|s| s.ident.to_string())
            && (FORBIDDEN_ALLOWS.contains(&ident.as_str())
                || INDEXING_LINTS.contains(&ident.as_str())
                || BLANKET_ALLOWS.contains(&ident.as_str()))
        {
            out.push((full, ident));
        }
        Ok(())
    });
    out
}

/// Whether the token at `index` is preceded by `::` (a qualified path segment, as in
/// `Option::unwrap(o)`), so a bare method name is not confused with an associated call.
fn is_path_qualified(trees: &[proc_macro2::TokenTree], index: usize) -> bool {
    use proc_macro2::TokenTree;
    let punct =
        |at: Option<&TokenTree>| matches!(at, Some(TokenTree::Punct(p)) if p.as_char() == ':');
    let Some(first) = index.checked_sub(2) else { return false };
    punct(trees.get(first)) && punct(trees.get(first + 1))
}

/// Token-level scan of a macro body: an ident that is a forbidden macro name
/// immediately followed by `!`, or `.` + `unwrap` / `expect` + a `()` group, or a
/// qualified `Option::unwrap(o)` / `Result::expect(r, …)`. Deliberately shape-exact — a
/// name that merely *starts* with `unwrap` (`unwrap_or`) is not a panic path, and a
/// literal's text (a string containing "unwrap()") is a `Literal` token, never an `Ident`.
pub(super) fn scan_macro_tokens(tokens: &proc_macro2::TokenStream, guard: &mut Guard<'_>) {
    use proc_macro2::{Delimiter, TokenTree};
    let trees = tokens.clone().into_iter().collect::<Vec<_>>();
    let mut i = 0;
    while let Some(tree) = trees.get(i) {
        match tree {
            TokenTree::Ident(id) => {
                let name = id.to_string();
                if FORBIDDEN_MACROS.contains(&name.as_str())
                    && matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                {
                    let span = id.span();
                    guard.report(
                        &format!("`{id}!` is a panic construct (inside a macro token stream)"),
                        span,
                    );
                } else if (name == "unwrap" || name == "expect")
                    && is_path_qualified(&trees, i)
                    && matches!(
                        trees.get(i + 1),
                        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis
                    )
                {
                    let span = id.span();
                    guard.report(
                        &format!(
                            "a qualified `::{name}(…)` call can panic \
                                 (inside a macro token stream)"
                        ),
                        span,
                    );
                }
            }
            TokenTree::Punct(p) if p.as_char() == '#' => {
                // A `#[…]` attribute **inside** a macro body: syn hands a macro only tokens, so
                // `visit_attribute` never sees it. `cfg_attr_arms` walks the bracket group and
                // finds any `allow` / `expect` arm in it (including a nested `cfg_attr`).
                if let Some(TokenTree::Group(group)) = trees.get(i + 1)
                    && group.delimiter() == Delimiter::Bracket
                {
                    let span = group.span();
                    for (kind, lints) in cfg_attr_arms(&group.stream()) {
                        guard.report_token_lints(&kind, &lints, span);
                    }
                }
            }
            TokenTree::Punct(p) if p.as_char() == '.' => {
                let Some(TokenTree::Ident(method)) = trees.get(i + 1) else {
                    i += 1;
                    continue;
                };
                let name = method.to_string();
                let Some(TokenTree::Group(group)) = trees.get(i + 2) else {
                    i += 1;
                    continue;
                };
                let is_call_group = group.delimiter() == Delimiter::Parenthesis;
                let panics = is_call_group
                    && (name == "expect" || (name == "unwrap" && group.stream().is_empty()));
                if panics {
                    let span = method.span();
                    guard.report(
                        &format!(".{name}(…) can panic (inside a macro token stream)"),
                        span,
                    );
                }
            }
            TokenTree::Group(group) => scan_macro_tokens(&group.stream(), guard),
            _ => {}
        }
        i += 1;
    }
}
