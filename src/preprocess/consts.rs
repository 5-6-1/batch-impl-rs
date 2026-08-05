//! The `@` constant system: expansion of built-in type-family constants and
//! user-defined constants.
//!
//! Syntax (at the token-stream level):
//! - **Name families**: `@u*` / `@i*` / `@f*` / `@num` / `@scalar`
//! - **Range families**: `@u8..u128` / `@i8..i128` / `@f32..f64` (inclusive; width validated)
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
use std::collections::HashMap;

use crate::preprocess::consts_ctx::{ConstCtx, UserConsts};
use crate::util::bracket_is_passthrough;
use crate::util::{compile_err, compile_err_at, compile_error_str};

/// Built-in name families: `@name` → list of type identifiers.
fn builtin_named(name: &str) -> Option<Vec<&'static str>> {
    match name {
        "u*" => Some(vec!["u8", "u16", "u32", "u64", "u128", "usize"]),
        "i*" => Some(vec!["i8", "i16", "i32", "i64", "i128", "isize"]),
        "f*" => Some(vec!["f32", "f64"]),
        "num" => Some(vec![
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
            "i128", "isize", "f32", "f64",
        ]),
        "scalar" => Some(vec![
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
            "i128", "isize", "f32", "f64", "bool", "char",
        ]),
        _ => None,
    }
}

/// Parses a range-family endpoint (`u8` / `i32` / `f64`) into (family, width).
/// An illegal width (e.g. `u9`, `f8`) returns `None` (family matched but
/// width not in the legal set).
fn split_range_endpoint(s: &str) -> Option<(char, u32)> {
    let (fam, width_str) = s.split_at(1);
    let fam = fam.chars().next()?;
    let width: u32 = width_str.parse().ok()?;
    let legal: &[u32] = match fam {
        'u' | 'i' => &[8, 16, 32, 64, 128],
        'f' => &[32, 64],
        _ => return None,
    };
    legal.contains(&width).then_some((fam, width))
}

/// Built-in range families: `@u8..u128` (inclusive) → type list in ascending
/// width. Mismatched endpoint families or start > end return `Err` (the
/// caller builds the diagnostic).
fn builtin_range(start: &str, end: &str) -> Result<Vec<String>, String> {
    let Some((fam1, w1)) = split_range_endpoint(start) else {
        return Err(format!(
            "`@{}` has an invalid width (legal: u/i are 8/16/32/64/128, \
             f is 32/64)",
            start
        ));
    };
    let Some((fam2, w2)) = split_range_endpoint(end) else {
        return Err(format!(
            "`@{}` has an invalid width (legal: u/i are 8/16/32/64/128, \
             f is 32/64)",
            end
        ));
    };
    if fam1 != fam2 {
        return Err(format!(
            "range endpoint families mismatch: `{}` and `{}`",
            start, end
        ));
    }
    if w1 > w2 {
        return Err(format!(
            "range start is greater than end: `{}..{}`",
            start, end
        ));
    }
    let widths: &[u32] = match fam1 {
        'u' | 'i' => &[8, 16, 32, 64, 128],
        _ => &[32, 64],
    };
    Ok(widths
        .iter()
        .filter(|w| **w >= w1 && **w <= w2)
        .map(|w| format!("{}{}", fam1, w))
        .collect())
}

/// Renders a list of type names as a Bracket list group (`[u8, u16, ...]`).
fn render_list<'a>(names: impl IntoIterator<Item = &'a str>) -> TokenTree {
    let idents: Vec<Ident> =
        names.into_iter().map(|s| Ident::new(s, Span::call_site())).collect();
    Group::new(delimiter![[]], quote!(#(#idents),*)).into()
}

/// Same as above, taking a `String` iterator (`@all`-family item names).
fn render_list_strings(names: impl IntoIterator<Item = String>) -> TokenTree {
    let idents: Vec<Ident> =
        names.into_iter().map(|s| Ident::new(&s, Span::call_site())).collect();
    Group::new(delimiter![[]], quote!(#(#idents),*)).into()
}

/// Recognizes and expands an `@` constant reference at `tokens[i]`; returns
/// `Some((expanded output, tokens consumed))`; `None` keeps it as-is
/// (batch_trait!'s `@trait` — handled by segment-level substitution).
///
/// Forms (`@` is `tokens[i]`):
/// - `@` Ident `=` … → user definition segment (appears only during
///   `collect_user_consts`'s leading collection; treated as an error here —
///   attribute macro entries do not support custom constants)
/// - `@` Ident `..` Ident → range family
/// - `@trait` → full trait path (attribute macro entries; batch_trait!
///   returns `None` to keep)
/// - `@` Ident → name family / user table
fn try_expand_at(
    tokens: &[TokenTree], ctx: ConstCtx,
) -> Result<Option<(Vec<TokenTree>, usize)>, TokenStream> {
    let Some(TokenTree::Ident(name)) = tokens.get(1) else {
        // `@N` position references (Literal after `@`) are codegen-resolved
        // (impl generic list known only there) — keep as-is, no error here.
        if matches!(tokens.get(1), Some(TokenTree::Literal(_))) {
            return Ok(None);
        }
        let sp = tokens
            .first()
            .map(|t| t.span())
            .unwrap_or_else(proc_macro2::Span::call_site);
        return Err(compile_error_str(
            "batch-impl: `@` must be followed by a constant name (e.g. `@u*`, \
             `@u8..u128`)",
            sp,
        ));
    };
    let name_str = name.to_string();
    // Definition segments: `@name=...` are consumed only during
    // `collect_user_consts`'s leading collection; appearing here, the
    // diagnostic differs by context — user_table != None = wrong position,
    // None = custom not supported.
    if let Some(TokenTree::Punct(eq)) = tokens.get(2)
        && eq.as_char() == '='
    {
        let msg = if ctx.user_table().is_some() {
            format!(
                "batch-impl: constant definition `@{}=...` must appear before all \
                 `batch_trait!` trait segments (only the leading position can \
                 define)",
                name_str
            )
        } else {
            "batch-impl: `#[batch_impl]` / `#[batch_impl_only]` do not support \
             custom constant definitions; custom constants are supported only \
             by `batch_trait!` (leading `@name=value;` segment)"
                .to_string()
        };
        let sp = tokens
            .first()
            .map(|t| t.span())
            .unwrap_or_else(proc_macro2::Span::call_site);
        return Err(compile_error_str(&msg, sp));
    }
    // Range family: `@` Ident `..` Ident (`..` is Joint '.' + any '.';
    // optional `=`)
    if let Some(TokenTree::Punct(d1)) = tokens.get(2)
        && d1.as_char() == '.'
        && d1.spacing() == proc_macro2::Spacing::Joint
        && let Some(TokenTree::Punct(d2)) = tokens.get(3)
        && d2.as_char() == '.'
    {
        let end_idx = if let Some(TokenTree::Punct(eq)) = tokens.get(4)
            && eq.as_char() == '='
        {
            5
        } else {
            4
        };
        let Some(TokenTree::Ident(end)) = tokens.get(end_idx) else {
            return Err(compile_err!(
                "batch-impl: range constant `@{}{}..` is missing an end point \
                 (e.g. `@u8..u128`)",
                name_str,
                ".."
            ));
        };
        let types = builtin_range(&name_str, &end.to_string())
            .map_err(|msg| compile_err!("batch-impl: {}", msg))?;
        return Ok(Some((
            vec![render_list(types.iter().map(|s| s.as_str()))],
            end_idx + 1,
        )));
    }
    // `@trait`: Attribute (batch_impl/only) = full trait path (local name or
    // `#ext::Trait:` external path); Trait (batch_trait!) = return None, keep
    // as-is — batch_trait! has multiple segments with different trait names,
    // and `@trait` is expanded by entry's post-segmentation segment-level
    // substitution into the current segment's path (the `@type_t=<T>@trait<T>`
    // cross-segment reuse scenario). None also avoids lazy expansion
    // recursing on `@trait` itself (expand to as-is → hit again → recurse).
    if name_str == "trait" {
        return match ctx.trait_full_path() {
            Some(path) => Ok(Some((path.clone().into_iter().collect(), 2))),
            None => Ok(None),
        };
    }
    // `@all` family: expands to a Bracket group `[a,b,c]` (uniform with
    // `@u*` list forms), batch_impl-only (needs trait_def to select items);
    // batch_trait! errors.
    if let Some((kinds, default, receiver)) =
        crate::preprocess::resolve_all_marker(&name_str)
    {
        return match ctx.trait_def() {
            Some(td) => {
                let ids = crate::preprocess::get_trait_item_names(
                    td, kinds.0, kinds.1, kinds.2, default, receiver,
                );
                Ok(Some((
                    vec![render_list_strings(ids.iter().map(|i| i.to_string()))],
                    2,
                )))
            }
            None => Err(compile_err!(
                "batch-impl: `@{}` is supported only by `#[batch_impl]` / \
                 `#[batch_impl_only]` (needs a trait definition to select \
                 items; `batch_trait!` is a function-like macro without one)",
                name_str
            )),
        };
    }
    // `@all_type_params` / `@all_const_params` / `@all_lifetimes`: generic-
    // parameter family (batch_impl-only, needs trait_def); expands to a flat
    // `<...>` declaration (paired by angle_collect, bounds via same-name
    // inheritance); batch_trait! errors.
    if let Some(gf) = crate::preprocess::resolve_generic_marker(&name_str) {
        return match ctx.trait_def() {
            Some(td) => match crate::preprocess::get_trait_generic_decl(td, gf) {
                Some(decl) => {
                    let decl = decl.into_iter().collect();
                    Ok(Some((decl, 2)))
                }
                None => Err(compile_err!(
                    "batch-impl: `@{}` cannot expand — trait `{}` has no {} \
                     parameters",
                    name_str,
                    td.ident,
                    match gf {
                        crate::preprocess::GenericFilter::Type => "type",
                        crate::preprocess::GenericFilter::Const => "const",
                        crate::preprocess::GenericFilter::Lifetime => "lifetime",
                    }
                )),
            },
            None => Err(compile_err!(
                "batch-impl: `@{}` is supported only by `#[batch_impl]` / \
                 `#[batch_impl_only]` (needs a trait definition to read its \
                 generic parameters; `batch_trait!` is a function-like macro \
                 without one)",
                name_str
            )),
        };
    }
    if let Some(expanded) = ctx.user_table().and_then(|t| t.get(&name_str)) {
        return Ok(Some((expanded.clone(), 2)));
    }
    // `@u*` / `@i*` / `@f*`: wildcard name family (Ident + `*`); consumed = 3
    let star =
        matches!(tokens.get(2), Some(TokenTree::Punct(p)) if p.as_char() == '*');
    let lookup = if star { format!("{}*", name_str) } else { name_str.clone() };
    match builtin_named(&lookup) {
        Some(types) => Ok(Some((
            vec![render_list(types.iter().copied())],
            if star { 3 } else { 2 },
        ))),
        None => Err(compile_err_at!(
            tokens[0].span(),
            "batch-impl: unknown @ constant `@{}`; built-ins: `@u*` `@i*` `@f*` \
             `@num` `@scalar` and ranges `@u8..u128` `@i8..i128` `@f32..f64`\
             {}",
            lookup,
            if ctx.user_table().is_some() {
                "; batch_trait! user constants must be defined before the \
                 reference (defining them later has no effect)"
            } else {
                ""
            }
        )),
    }
}

/// Validates `@` reference visibility inside constant values: the constant
/// name after each `@` must be in (defined user constants ∪ built-in
/// constants). Circular references (`@a=@a`) and forward references
/// (`@a=@b` with `@b` defined later) are intercepted here — under lazy
/// expansion a circular ref would recurse forever, and erroring at the
/// definition beats erroring at the use site. Recurses into all groups (the
/// `@u*` of `[Vec<@u*>]` is inside an angle group).
fn check_value_refs(
    tokens: &[TokenTree], table: &HashMap<String, Vec<TokenTree>>, def_name: &str,
) -> Result<(), TokenStream> {
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let Some(TokenTree::Ident(name)) = tokens.get(i + 1) else {
                    return Err(compile_error_str(
                        "batch-impl: inside a constant value, `@` must be followed \
                     by a constant name (e.g. `@u*`, `@u8..u128`)",
                        tokens[i].span(),
                    ));
                };
                let name_str = name.to_string();
                // `@u*` / `@i*` / `@f*` wildcard: Ident + `*` consumes 3 tokens
                let star = matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '*');
                let lookup =
                    if star { format!("{}*", name_str) } else { name_str.clone() };
                // `@trait` is a segment-level special marker (replaced with
                // the current segment's trait path after batch_trait!
                // segmentation), not a constant reference — skip the
                // visibility check
                let known = name_str == "trait"
                    || builtin_named(&lookup).is_some()
                    || split_range_endpoint(&name_str).is_some()
                    || table.contains_key(&name_str);
                if !known {
                    return Err(compile_err!(
                        "batch-impl: constant `@{}` references unknown `@{}` \
                         (undefined or defined later; inside a constant \
                         definition, only built-in constants or previously \
                         defined constants can be referenced)",
                        def_name,
                        name_str
                    ));
                }
                i += if star { 3 } else { 2 };
            }
            TokenTree::Group(g) => {
                check_value_refs(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                    table,
                    def_name,
                )?;
                i += 1;
            }
            _ => i += 1,
        }
    }
    Ok(())
}

/// Expands `@` constant references in a token stream (built-in + user table).
///
/// Recursion rules match `angle_collect` / `expand_tokens`; only `Brace`
/// (and passthrough `[...]` — `ident![...]` / `#[...]`) is not entered
/// (`@` in a body is pattern syntax `x @ pat`).
pub(crate) fn expand_consts(
    tokens: &[TokenTree], ctx: ConstCtx,
) -> Result<Vec<TokenTree>, TokenStream> {
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
                if g.delimiter() == delimiter![[]]
                    && bracket_is_passthrough(tokens, i)
                {
                    result.push(tokens[i].clone());
                } else {
                    let inner: Vec<_> = g.stream().into_iter().collect();
                    result.push(
                        Group::new(
                            g.delimiter(),
                            expand_consts(&inner, ctx)?.into_iter().collect(),
                        )
                        .into(),
                    );
                }
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '@' => {
                match try_expand_at(&tokens[i..], ctx)? {
                    // Lazy expansion: user constant values store tokens as-is
                    // (may contain nested `@` references and DSL operations);
                    // after splicing, expand recursively (circular refs are
                    // already intercepted at definition, so recursion
                    // terminates).
                    Some((expanded, consumed)) => {
                        let expanded = expand_consts(&expanded, ctx)?;
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
                    let inner: Vec<_> = g.stream().into_iter().collect();
                    let expanded = expand_consts(&inner, ctx)?.into_iter().collect();
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
            if let TokenTree::Punct(p) = &tokens[j]
                && p.as_char() == ';'
            {
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
        let value: Vec<TokenTree> = tokens[i + 3..end].to_vec();
        // Value is arbitrary tokens (lazy expansion); reference visibility is
        // validated in `check_value_refs`
        check_value_refs(&value, &table, &name_str)?;
        table.insert(name_str, value);
        i = end + 1;
    }
    Ok((tokens[i..].to_vec(), table))
}
