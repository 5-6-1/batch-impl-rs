//! Parsing layer: DSL precedence-climbing parser and angle-bracket generic parsing.

mod chain;
mod generic;
mod parse_atom;
mod primary;
mod space;
mod trailing;
pub(crate) use chain::parse_item;
pub(crate) use generic::split_at_depth0;
pub(crate) use primary::parse_primary;
pub(crate) use space::*;
pub(crate) use trailing::strip_attachments;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};

use crate::apply::err_ty_at;
use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::util::{MAX_NEST_DEPTH, compile_error_str};

/// Resolves `@N` / `@g_i` position references inside a token chunk that is
/// **not** parsed as a type (angle-group contents go through flat token
/// splitting in `parse_type_params`, so `Box<@0>` would otherwise keep the
/// raw `@0`). Recurses into groups; `@` followed by a non-digit errors.
pub(crate) fn resolve_at_refs(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let at_span = p.span();
                match tokens.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        let name = at_ref_name(&lit.to_string()).ok_or_else(|| {
                            compile_error_str(
                                "batch-impl: `@` in a type must be followed by a \
                                 position digit (e.g. `@0` or `@0_1`)",
                                at_span,
                            )
                        })?;
                        let ident = Ident::new(&name, at_span);
                        out.push(TokenTree::Ident(ident));
                        i += 2;
                    }
                    _ => {
                        return Err(compile_error_str(
                            "batch-impl: `@` in a type must be a position digit (e.g. `@0` or `@0_1`)",
                            at_span,
                        ));
                    }
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                let mut new_g =
                    Group::new(g.delimiter(), resolve_at_refs(&inner)?.into_iter().collect());
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
                i += 1;
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// DSL parse entry: strips trailing `{...}` code blocks / `where{...}` suffixes,
/// attaching them via apply to the type parsed from the remaining tokens.
///
/// Consecutive attachments (`T{a}{b}` / `T where{...}`) are a **linear chain**; strip by loop
/// removes recursion (deep bodies overflow the stack); iteration removes any depth limit.
///
/// `depth` counts chained type segments — every "parse the remaining tokens and
/// apply the current unit" recursion in [`parse_primary`] adds one level. Flat
/// chains (`<T><U>...X`, `Trait<A> Trait<B>... X`, `#[a] #[b]... X`) build a
/// deep `Ty` tree without any group nesting, so the token-level group guard
/// (`angle_collect`'s `MAX_NEST_DEPTH`) cannot catch them; this entry enforces
/// the same limit. Attachments are counted separately below (they wrap the
/// type one `WithCode`/`WithWhere` level per body).
pub(crate) fn parse_primitive(
    tokens: &[TokenTree], trait_name: Option<&Ident>, depth: usize,
) -> Ty {
    if depth > MAX_NEST_DEPTH {
        return err_ty_at(
            &format!(
                "batch-impl: chained type segments exceed {} levels (limit {}); \
                 split into separate impl-specs",
                depth, MAX_NEST_DEPTH,
            ),
            tokens.first().map_or_else(proc_macro2::Span::call_site, |t| t.span()),
        );
    }
    // Collect attachments outside-in (outer first); `rest` shrinks to the innermost base
    let (mut attaches, rest) = strip_attachments(tokens);
    // Attachment-chain guard: each attachment nests the type one wrapper
    // level, so a flat chain of bodies overflows the same downstream
    // traversals as a deep operator chain — capped at the same limit.
    if attaches.len() > MAX_NEST_DEPTH {
        return err_ty_at(
            &format!(
                "batch-impl: trailing attachment chain (`{{...}}` / `where{{...}}` / \
                 `impl{{...}}`) exceeds {} levels (limit {}); split into separate impl-specs",
                attaches.len(),
                MAX_NEST_DEPTH,
            ),
            tokens.first().map_or_else(proc_macro2::Span::call_site, |t| t.span()),
        );
    }
    let mut ty = if rest.is_empty() {
        // The whole operand is a bare block chain (`{a}{b}`): the innermost block is the "top-level item
        // injection" base (inner `None` mark); empty attaches = empty input, so parse atomically
        match attaches.pop() {
            Some(inner) => inner,
            None => parse_primary(rest, trait_name, depth),
        }
    } else {
        parse_primary(rest, trait_name, depth)
    };
    // Apply from inside out (attaches tail = innermost)
    while let Some(block) = attaches.pop() {
        ty = block.apply(ty);
    }
    ty
}

// ============================================================
// Atom-level parsing
// ============================================================
