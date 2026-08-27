//! Per-`@` recognition and expansion ([`try_expand_at`]) plus reference
//! visibility validation inside constant values ([`check_value_refs`]).
//!
//! Split from `table` so the built-in constant tables / entry points
//! (`table.rs`) and the per-token expansion logic stay under the per-file
//! line budget. `table.rs` (entry + tables) → `expand.rs` (single-`@`
//! logic) is the only dependency direction.

use proc_macro2::{TokenStream, TokenTree};

use crate::preprocess::consts::ctx::ConstCtx;
use crate::preprocess::{builtin_named, builtin_range_open, render_list, split_range_endpoint};
use crate::util::{compile_err, compile_err_at, compile_error_str, is_joint_punct_at, is_punct_at};

/// Recognizes and expands an `@` constant reference at `tokens[0]`; returns
/// `Some((expanded output, tokens consumed))`; `None` keeps it as-is
/// (batch_trait!'s `@trait` — handled by segment-level substitution).
///
/// Forms (`@` is `tokens[0]`):
/// - `@` Ident `=` … → user definition segment (appears only during
///   `collect_user_consts`'s leading collection; treated as an error here —
///   attribute macro entries do not support custom constants)
/// - `..` [`=`] Ident → open-left range family (`@..u128`)
/// - `@` Ident `..` [`=`] Ident? → range family (`@u8..u128`, `@u16..`)
/// - `@trait` → full trait path (attribute macro entries; batch_trait!
///   returns `None` to keep)
/// - `@` Ident → name family / user table
pub(crate) fn try_expand_at(
    tokens: &[TokenTree], ctx: ConstCtx,
) -> Result<Option<(Vec<TokenTree>, usize)>, TokenStream> {
    // Open-left range family: `@..u128` / `@..=i64` — the family minimum
    // fills the omitted start. Recognized before the Ident requirement
    // (there is no leading name in this form). The operator dictionary reads
    // `..` / `..=` as one unit.
    if let Some((crate::util::Op::DotDot, _) | (crate::util::Op::DotDotEq, _)) =
        crate::util::read_op(tokens, 1)
    {
        let end_idx = if let Some(TokenTree::Punct(eq)) = tokens.get(3)
            && eq.as_char() == '='
        {
            4
        } else {
            3
        };
        let Some(TokenTree::Ident(end)) = tokens.get(end_idx) else {
            return Err(compile_err!(
                "batch-impl: range constant `@..` must name the family's \
                 maximum endpoint (e.g. `@..u128`, `@..f64`)"
            ));
        };
        let types = builtin_range_open(None, Some(&end.to_string()))
            .map_err(|msg| compile_err!("batch-impl: {}", msg))?;
        return Ok((vec![render_list(types.iter().map(|s| s.as_str()))], end_idx + 1).into());
    }
    let Some(TokenTree::Ident(name)) = tokens.get(1) else {
        // `@N` position references (Literal after `@`) are codegen-resolved
        // (impl generic list known only there) — keep as-is on every entry
        // (the ItemImpl entry resolves them too, against the fresh generics
        // a generator hoisted; a dangling ref errors at codegen).
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
            ConstCtx::Attribute { .. } | ConstCtx::ItemImpl { .. } => {
                "batch-impl: custom constants are not supported by \
                 `#[batch_impl]` / `#[batch_impl_only]` — write the type \
                 matrix directly with `.` / space / `*` instead"
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
    // optional `=`). Endpoint resolution: an ident right after the dots
    // whose width is legal is the endpoint (whitespace-insensitive, like
    // every pre-existing form); an ident that fails width validation is NOT
    // an endpoint — it is the next DSL item (`@i16.. Neg`) — and the open
    // family (`@i16..` ≡ `@i16..i128`) is emitted instead, ident left
    // unconsumed. Byte-position adjacency distinguishes glued from
    // separated for the `=` half (the second dot of `..` lexes `Alone`
    // either way).
    if is_joint_punct_at(tokens, 2, '.') && is_punct_at(tokens, 3, '.') {
        let dots_end = tokens[3].span();
        let eq_adj = matches!(tokens.get(4), Some(TokenTree::Punct(eq)) if eq.as_char() == '=')
            && crate::util::spans_adjacent(dots_end, tokens[4].span());
        let endpoint: Option<(usize, String)> = if eq_adj {
            tokens.get(5).and_then(|t| match t {
                TokenTree::Ident(end) => Some((5, end.to_string())),
                _ => None,
            })
        } else {
            tokens.get(4).and_then(|t| match t {
                TokenTree::Ident(end) => Some((4, end.to_string())),
                _ => None,
            })
        };
        let (types, consumed) = match endpoint {
            // a legal-width endpoint wins regardless of adjacency
            // (`@u8.. u128` — whitespace-insensitive like every
            // pre-existing form)
            Some((idx, end)) if split_range_endpoint(&end).is_some() => (
                builtin_range_open(Some(&name_str), Some(&end))
                    .map_err(|msg| compile_err!("batch-impl: {}", msg))?,
                idx + 1,
            ),
            // `..=X` where X exists but fails width validation: a typo
            Some((5, bad)) if eq_adj => {
                return Err(compile_err!(
                    "batch-impl: range constant `@{}..=` has an invalid end \
                     point `{}`",
                    name_str,
                    bad
                ));
            }
            // `..=` with nothing after: missing, not a shorthand
            None if eq_adj => {
                return Err(compile_err!(
                    "batch-impl: range constant `@{}..=` is missing an end \
                     point (e.g. `@u16..=u64`)",
                    name_str
                ));
            }
            // no usable endpoint: the open family, ident left unconsumed
            // (`@i16.. Neg` — Neg is the next DSL item)
            _ => (
                builtin_range_open(Some(&name_str), None)
                    .map_err(|msg| compile_err!("batch-impl: {}", msg))?,
                4,
            ),
        };
        return Ok((vec![render_list(types.iter().map(|s| s.as_str()))], consumed).into());
    }
    // `@trait`: Attribute (batch_impl/only) = full trait path (local name or
    // `#ext::Trait:` external path); ItemImpl = the impl's own trait path
    // (`None` on an inherent impl — an error); Trait (batch_trait!) = return
    // None, keep as-is — batch_trait! has multiple segments with different
    // trait names, and `@trait` is expanded by entry's post-segmentation
    // segment-level substitution into the current segment's path (the
    // `@type_t=<T>@trait<T>` cross-segment reuse scenario). None also avoids
    // lazy expansion recursing on `@trait` itself (expand to as-is → hit
    // again → recurse).
    if name_str == "trait" {
        return match ctx.trait_full_path() {
            Some(path) => Ok((path.clone().into_iter().collect(), 2).into()),
            None if ctx.is_item_impl() => Err(compile_err!(
                "batch-impl: `@trait` is not available on an inherent impl \
                 (there is no trait to refer to)"
            )),
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
            None if ctx.is_item_impl() => Err(compile_err!(
                "batch-impl: `@{}` is not available on the ItemImpl entry \
                 (no trait definition to select items from)",
                name_str
            )),
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
            None if ctx.is_item_impl() => Err(compile_err!(
                "batch-impl: `@{}` is not available on the ItemImpl entry \
                 (no trait definition to read generic parameters from)",
                name_str
            )),
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
             `@num` `@scalar` and ranges `@u8..u128` `@..u128` `@u16..`{}",
                lookup,
                // batch_trait! can define user constants (defined before the
                // reference); attribute macros cannot — no suffix there.
                match ctx {
                    ConstCtx::Trait { .. } =>
                        "; user constants must be defined \
                      before the reference (defining them later has no effect)",
                    ConstCtx::Attribute { .. } | ConstCtx::ItemImpl { .. } => "",
                }
            ))
        }
    }
}
