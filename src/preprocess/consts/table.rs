//! The `@` constant system: expansion of built-in type-family constants and
//! user-defined constants.
//!
//! Syntax (at the token-stream level):
//! - **Name families**: `@u*` / `@i*` / `@f*` / `@num` / `@scalar`
//! - **Range families**: `@u8..u128` / `@i8..i128` / `@f32..f64` (inclusive;
//!   width validated), with either endpoint omittable — the family's minimum
//!   / maximum fills in (`@..u128` ≡ `@u8..u128`, `@u16..` ≡ `@u16..u128`;
//!   at least one concrete endpoint must anchor the family)
//! - **User-defined** (only `batch_trait!`): a leading `@name=value;` segment
//!   whose value is any DSL expression (may reference built-in constants;
//!   references to already-defined user constants work naturally in
//!   definition order)
//!
//! The expansion output is a Bracket list (`[u8, u16, ...]`), token-for-token
//! equivalent to a hand-written list, flowing through the normal pipeline.
//! In the macro-meta layer (architecture.md "syntax-domain isolation"): only
//! lexical substitution; no in-domain parsing.
//!
//! Pipeline position: before `angle_collect` (expansions may contain flat
//! `<...>` that `angle_collect` pairs uniformly afterwards), before directive
//! preprocessing (expansions containing `#name` directives are still
//! processed normally).

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;

use crate::preprocess::consts::ctx::{ConstCtx, UserConsts};
use crate::util::{bracket_is_passthrough, compile_err, is_impl_template, is_punct};

/// Built-in name families: `@name` → list of type identifiers.
pub(crate) fn builtin_named(name: &str) -> Option<Vec<&'static str>> {
    match name {
        "u*" => vec!["u8", "u16", "u32", "u64", "u128", "usize"].into(),
        "i*" => vec!["i8", "i16", "i32", "i64", "i128", "isize"].into(),
        "f*" => vec!["f32", "f64"].into(),
        "num" => vec![
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
            "f32", "f64",
        ]
        .into(),
        "scalar" => vec![
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
            "f32", "f64", "bool", "char",
        ]
        .into(),
        _ => None,
    }
}

/// Renders a list of type names as a Bracket list group (`[u8, u16, ...]`).
/// Generic over the name iterator item (`&str` or `String` both work).
pub(crate) fn render_list<S: ToString>(names: impl IntoIterator<Item = S>) -> TokenTree {
    let idents = names
        .into_iter()
        .map(|s| Ident::new(&s.to_string(), Span::call_site()))
        .collect::<Vec<_>>();
    Group::new(delimiter![[]], quote!(#(#idents),*)).into()
}

/// Expands `@` constant references in a token stream (built-in + user table).
///
/// Recursion rules match `angle_collect` / `expand_tokens`; only `Brace`
/// (and passthrough `[...]` — `ident![...]` / `#[...]`) is not entered
/// (`@` in a body is pattern syntax `x @ pat`).
pub(crate) fn expand_consts(
    tokens: &[TokenTree], ctx: ConstCtx,
) -> Result<Vec<TokenTree>, TokenStream> {
    expand_consts_at(tokens, ctx, 0)
}

/// Recursive core of [`expand_consts`] with a nesting guard (mirrors
/// `angle_collect`'s `MAX_NEST_DEPTH` — an accidental extra bracket must
/// error out instead of overflowing the stack).
fn expand_consts_at(
    tokens: &[TokenTree], ctx: ConstCtx, depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, ""));
    }
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // `delimiter![<>]` and `delimiter![none]` are the same value
            // (Delimiter::None). Under the new order (`@` before `<>`
            // pairing), no angle groups exist in the stream when
            // expand_consts runs (angle_collect has not run), so any None
            // group must be a real transparent group (macro-variable output
            // from `$(...)*`/`$x:ty` expansion) — recurse into it to expand
            // the inner `@` (in 0.6.0's `<> @` order angle_collect flattened
            // first and this was not entered; after the order fix, not
            // recursing would leave inner `@` behind).
            TokenTree::Group(g)
                if g.delimiter() == delimiter![()]
                    || g.delimiter() == delimiter![[]]
                    || g.delimiter() == delimiter![none] =>
            {
                // Macro calls (`foo!(...)` / `foo![...]`) and attributes
                // (`#[...]`) pass through untouched — their contents are
                // user Rust, not DSL. `#name(...)` directive arguments and
                // plain `(A, B)` tuples / `<...>` angle groups still recurse
                // (the group's previous token is an Ident, not `!`/`#`).
                if bracket_is_passthrough(tokens, i) {
                    result.push(tokens[i].clone());
                } else {
                    // Guard before materializing the group's stream: the
                    // recursion-entry check runs after `stream()`/collect, so
                    // check the next level here to fail before touching the
                    // subtree.
                    if depth + 1 > crate::util::MAX_NEST_DEPTH {
                        return Err(crate::util::depth_err(&tokens[i..i + 1], ""));
                    }
                    let inner = g.stream().into_iter().collect::<Vec<_>>();
                    result.push(
                        Group::new(
                            g.delimiter(),
                            expand_consts_at(&inner, ctx, depth + 1)?.into_iter().collect(),
                        )
                        .into(),
                    );
                }
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '@' => {
                // A carrier (`@` + Brace group — a fresh reference `@{0}` /
                // the `@{}` body-slot switch) is codegen's concern, never a
                // constant: pass both tokens through untouched.
                if crate::ast::fresh::is_carrier_at(tokens, i) {
                    result.push(tokens[i].clone());
                    result.push(tokens[i + 1].clone());
                    i += 2;
                    continue;
                }
                match crate::preprocess::try_expand_at(&tokens[i..], ctx)? {
                    // Lazy expansion: user constant values store tokens as-is
                    // (may contain nested `@` references and DSL operations);
                    // after splicing, expand recursively (circular refs are
                    // already intercepted at definition, so recursion
                    // terminates).
                    Some((expanded, consumed)) => {
                        let expanded = expand_consts_at(&expanded, ctx, depth + 1)?;
                        result.extend(expanded);
                        i += consumed;
                    }
                    // `None` (batch_trait!'s `@trait`, or `@N` position refs —
                    // Literal after `@`): keep as-is and do not recurse
                    // (otherwise `@trait` expands to itself → hit again →
                    // infinite recursion; `@N` is resolved by codegen where
                    // the impl generic list is known)
                    None => {
                        result.push(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            // `where{...}` predicate suffix: a Brace group right after the
            // `where` ident is a DSL structure (not a body), so enter it to
            // expand `@trait` (batch_impl knows the trait path) — `@N` stays
            // untouched for codegen. Bare `where pred {body}` has its
            // predicate at the top level and is already covered by the loop.
            TokenTree::Ident(id) if id == "where" => {
                if let Some(TokenTree::Group(g)) = tokens.get(i + 1)
                    && g.delimiter() == delimiter![{}]
                {
                    let inner = g.stream().into_iter().collect::<Vec<_>>();
                    let expanded = expand_consts_at(&inner, ctx, depth + 1)?.into_iter().collect();
                    result.push(tokens[i].clone());
                    result.push(Group::new(delimiter![{}], expanded).into());
                    i += 2;
                } else {
                    result.push(tokens[i].clone());
                    i += 1;
                }
            }
            // `impl{...}` shape template: a Brace group
            // right after the `impl` ident is the shape template, entered to
            // expand `@trait` / `@` constants — the remaining tokens form a
            // standard Rust type parsed by syn in codegen. Bodies are never
            // entered (an `impl` inside `{body}` is inside a Brace group this
            // walker skips). The `impl{...}` discrimination is centralized in
            // `util::is_impl_template` (shared with `where_process`); the
            // guard below re-checks instead of unwrapping — no-panic promise.
            TokenTree::Ident(_) if is_impl_template(tokens, i) => {
                if let Some(TokenTree::Group(g)) = tokens.get(i + 1)
                    && g.delimiter() == delimiter![{}]
                {
                    let inner = g.stream().into_iter().collect::<Vec<_>>();
                    let expanded = expand_consts_at(&inner, ctx, depth + 1)?.into_iter().collect();
                    result.push(tokens[i].clone());
                    result.push(Group::new(delimiter![{}], expanded).into());
                    i += 2;
                } else {
                    result.push(tokens[i].clone());
                    i += 1;
                }
            }
            _ => {
                result.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(result)
}

/// Collects `batch_trait!`'s leading user constant definition segments:
/// `@name=value;` (zero or more).
pub(crate) fn collect_user_consts(
    tokens: &[TokenTree],
) -> Result<(Vec<TokenTree>, UserConsts), TokenStream> {
    let mut i = 0;
    let mut table = UserConsts::new();
    while let Some(TokenTree::Punct(at)) = tokens.get(i) {
        if at.as_char() != '@' {
            break;
        }
        let Some(TokenTree::Ident(name)) = tokens.get(i + 1) else { break };
        let Some(TokenTree::Punct(eq)) = tokens.get(i + 2) else { break };
        if eq.as_char() != '=' {
            break;
        }
        let name_str = name.to_string();
        // `@trait` is a segment-level special marker (replaced with the
        // current segment's trait path after batch_trait! segmentation) and
        // cannot be used as a constant name (otherwise the special marker
        // intercepts it and the segment-level substitution silently shadows
        // it)
        if name_str == "trait" {
            return Err(compile_err!(
                "batch-impl: constant name `@trait` is a reserved marker \
                 (segment-level substitution into a trait path); please rename"
            ));
        }
        // `@all` / `@all_*` are reserved item selectors (methods / types /
        // params / lifetimes) — a user constant with such a name would be
        // shadowed by the built-in selector lookup, so reject at the
        // definition instead of failing confusingly at the use site
        if name_str == "all" || name_str.starts_with("all_") {
            return Err(compile_err!(
                "batch-impl: constant name `@{}` is a reserved `@all` \
                 selector; please rename",
                name_str
            ));
        }
        // Name collision with a built-in constant → error (prevent
        // accidental override)
        if builtin_named(&name_str).is_some() {
            return Err(compile_err!(
                "batch-impl: user constant `@{}` collides with a built-in \
                 constant name; please rename",
                name_str
            ));
        }
        // Value: up to the depth-0 `;`
        let mut j = i + 3;
        let mut end = None;
        while j < tokens.len() {
            if is_punct(&tokens[j], ';') {
                end = Some(j);
                break;
            }
            j += 1;
        }
        let Some(end) = end else {
            return Err(compile_err!(
                "batch-impl: constant definition `@{}=...` is missing the \
                 trailing `;`",
                name_str
            ));
        };
        let value = tokens[i + 3..end].to_vec();
        // Value is arbitrary tokens (lazy expansion); reference visibility is
        // validated in `check_value_refs`
        crate::preprocess::check_value_refs(&value, &table, &name_str)?;
        table.insert(name_str, value);
        i = end + 1;
    }
    Ok((tokens[i..].to_vec(), table))
}
