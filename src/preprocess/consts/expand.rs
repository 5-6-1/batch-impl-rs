//! Per-`@` recognition and expansion ([`try_expand_at`]) plus reference
//! visibility validation inside constant values ([`check_value_refs`]).
//!
//! Split from `table` so the built-in constant tables / entry points
//! (`table.rs`) and the per-token expansion logic stay under the per-file
//! line budget. `table.rs` (entry + tables) → `expand.rs` (single-`@`
//! logic) is the only dependency direction.

use proc_macro2::{TokenStream, TokenTree};

use crate::preprocess::consts::ctx::ConstCtx;
use crate::preprocess::{builtin_named, builtin_range, render_list, split_range_endpoint};
use crate::util::{compile_err, compile_err_at, compile_error_str, is_joint_punct_at, is_punct_at};

/// Recognizes and expands an `@` constant reference at `tokens[0]`; returns
/// `Some((expanded output, tokens consumed))`; `None` keeps it as-is
/// (batch_trait!'s `@trait` — handled by segment-level substitution).
///
/// Forms (`@` is `tokens[0]`):
/// - `@` Ident `=` … → user definition segment (appears only during
///   `collect_user_consts`'s leading collection; treated as an error here —
///   attribute macro entries do not support custom constants)
/// - `@` Ident `..` Ident → range family
/// - `@trait` → full trait path (attribute macro entries; batch_trait!
///   returns `None` to keep)
/// - `@` Ident → name family / user table
pub(crate) fn try_expand_at(
    tokens: &[TokenTree], ctx: ConstCtx,
) -> Result<Option<(Vec<TokenTree>, usize)>, TokenStream> {
    let Some(TokenTree::Ident(name)) = tokens.get(1) else {
        // `@N` position references (Literal after `@`) are codegen-resolved
        // (impl generic list known only there) — keep as-is, no error here.
        if matches!(tokens.get(1), Some(TokenTree::Literal(_))) {
            return Ok(None);
        }
        return Err(compile_error_str(
            "batch-impl: `@` must be followed by a constant name (e.g. `@u*`, \
             `@u8..u128`)",
            tokens[0].span(),
        ));
    };
    let name_str = name.to_string();
    // Definition segments: `@name=...` are consumed only during
    // `collect_user_consts`'s leading collection (batch_trait! only);
    // reaching here means a definition in the wrong entry or position —
    // attribute macros do not support custom constants (0.8.0 reverted the
    // 0.7.2 feature), a batch_trait! definition after a segment is
    // non-leading. One message per context (0.7.2 unified them when both
    // entries carried a table; the split is restored with the revert).
    if let Some(TokenTree::Punct(eq)) = tokens.get(2)
        && eq.as_char() == '='
    {
        let msg = match ctx {
            ConstCtx::Attribute { .. } => {
                "batch-impl: custom constants are not supported by \
                 `#[batch_impl]` / `#[batch_impl_only]` — write the type \
                 matrix directly with `^` / `-` / `*` instead"
            }
            ConstCtx::Trait { .. } => {
                "batch-impl: constant definition must appear before all trait \
                 segments (only the leading position can define; end each \
                 definition with `;`)"
            }
        };
        return Err(compile_error_str(msg, tokens[0].span()));
    }
    // Range family: `@` Ident `..` Ident (`..` is Joint '.' + any '.';
    // optional `=`)
    if is_joint_punct_at(tokens, 2, '.') && is_punct_at(tokens, 3, '.') {
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
        return Ok((vec![render_list(types.iter().map(|s| s.as_str()))], end_idx + 1).into());
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
            Some(path) => Ok((path.clone().into_iter().collect(), 2).into()),
            None => Ok(None),
        };
    }
    // `@all` family: expands to a Bracket group `[a,b,c]` (uniform with
    // `@u*` list forms), batch_impl-only (needs trait_def to select items);
    // batch_trait! errors.
    if let Some((kinds, default, receiver)) = crate::preprocess::resolve_all_marker(&name_str) {
        return match ctx.trait_def() {
            Some(td) => {
                let ids = crate::preprocess::get_trait_item_names(
                    td, kinds.0, kinds.1, kinds.2, default, receiver,
                );
                Ok((vec![render_list(ids.iter())], 2).into())
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
                    Ok((decl, 2).into())
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
        return Ok((expanded.clone(), 2).into());
    }
    // `@u*` / `@i*` / `@f*`: wildcard name family (Ident + `*`); consumed = 3
    let star = is_punct_at(tokens, 2, '*');
    let lookup = if star { format!("{}*", name_str) } else { name_str.clone() };
    match builtin_named(&lookup) {
        Some(types) => {
            Ok((vec![render_list(types.iter().copied())], if star { 3 } else { 2 }).into())
        }
        None => {
            // `@all_fresh` is a where-predicate selector resolved by codegen
            // (each fresh generic gets the predicate tail) — keep it as-is
            // here; the constant stage must not claim it.
            if name_str == "all_fresh" {
                return Ok(None);
            }
            Err(compile_err_at!(
                tokens[0].span(),
                "batch-impl: unknown @ constant `@{}`; built-ins: `@u*` `@i*` `@f*` \
             `@num` `@scalar` and ranges `@u8..u128` `@i8..i128` `@f32..f64`{}",
                lookup,
                // batch_trait! can define user constants (defined before the
                // reference); attribute macros cannot — no suffix there.
                match ctx {
                    ConstCtx::Trait { .. } =>
                        "; user constants must be defined \
                     before the reference (defining them later has no effect)",
                    ConstCtx::Attribute { .. } => "",
                }
            ))
        }
    }
}

/// Validates `@` reference visibility inside constant values: the constant
/// name after each `@` must be in (defined user constants ∪ built-in
/// constants). Circular references (`@a=@a`) and forward references
/// (`@a=@b` with `@b` defined later) are intercepted here — under lazy
/// expansion a circular ref would recurse forever, and erroring at the
/// definition beats erroring at the use site. Recurses into all groups (the
/// `@u*` of `[Vec<@u*>]` is inside an angle group).
pub(crate) fn check_value_refs(
    tokens: &[TokenTree], table: &std::collections::HashMap<String, Vec<TokenTree>>, def_name: &str,
) -> Result<(), TokenStream> {
    check_value_refs_at(tokens, table, def_name, 0)
}

/// Recursive core of [`check_value_refs`] with a nesting guard (mirrors
/// `expand_consts`'s `MAX_NEST_DEPTH` — a deeply nested constant value must
/// error out instead of overflowing the stack).
fn check_value_refs_at(
    tokens: &[TokenTree], table: &std::collections::HashMap<String, Vec<TokenTree>>,
    def_name: &str, depth: usize,
) -> Result<(), TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, " in a constant value"));
    }
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
                let star = is_punct_at(tokens, i + 2, '*');
                let lookup = if star { format!("{}*", name_str) } else { name_str.clone() };
                // A range-family endpoint (`u8`) is only a valid reference when
                // followed by `..` (the full `@u8..u128` form); a bare `@u8`
                // is not a constant and must fail here (at the definition),
                // not at the use site.
                let is_range = is_punct_at(tokens, i + 2, '.');
                // `@trait` is a segment-level special marker (replaced with
                // the current segment's trait path after batch_trait!
                // segmentation), not a constant reference — skip the
                // visibility check
                let known = name_str == "trait"
                    || builtin_named(&lookup).is_some()
                    || (is_range && split_range_endpoint(&name_str).is_some())
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
                // Guard before materializing the group's stream (same
                // rationale as expand_consts_at).
                if depth + 1 > crate::util::MAX_NEST_DEPTH {
                    return Err(crate::util::depth_err(&tokens[i..i + 1], " in a constant value"));
                }
                check_value_refs_at(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                    table,
                    def_name,
                    depth + 1,
                )?;
                i += 1;
            }
            _ => i += 1,
        }
    }
    Ok(())
}
