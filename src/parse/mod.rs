//! Parsing layer: DSL precedence-climbing parser and angle-bracket generic parsing.

mod chain;
mod generic;
mod parse_atom;
mod primary;
mod trailing;
pub(crate) use chain::parse_item;
pub(crate) use generic::split_at_depth0;
pub(crate) use primary::parse_primary;
pub(crate) use trailing::split_trailing_body;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};

use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::util::compile_error_str;

/// Resolves `@N` / `@g_i` position references inside a token chunk that is
/// **not** parsed as a type (angle-group contents go through flat token
/// splitting in `parse_type_params`, so `Box<@0>` would otherwise keep the
/// raw `@0`). Recurses into groups; `@` followed by a non-digit errors.
pub(crate) fn resolve_at_refs(
    tokens: &[TokenTree],
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let at_span = p.span();
                match tokens.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        let name =
                            at_ref_name(&lit.to_string()).ok_or_else(|| {
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
                let mut new_g = Group::new(
                    g.delimiter(),
                    resolve_at_refs(&inner)?.into_iter().collect(),
                );
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
pub(crate) fn parse_primitive(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> Ty {
    // Collect attachments outside-in (outer first); `rest` shrinks to the innermost base
    let mut attaches = vec![];
    let mut rest = tokens;
    loop {
        let split = split_trailing_body(rest);
        match (split.body, split.is_where) {
            (Some(body), false) => {
                attaches.push(TyWithCode(None, TyCodeBlock(body)).into());
                rest = split.tokens;
            }
            (Some(w), true) => {
                attaches.push(TyWithWhere(None, TyWhere(w)).into());
                rest = split.tokens;
            }
            _ => break,
        }
    }
    let mut ty = if rest.is_empty() {
        // The whole operand is a bare block chain (`{a}{b}`): the innermost block is the "top-level item
        // injection" base (inner `None` mark); empty attaches = empty input, so parse atomically
        match attaches.pop() {
            Some(inner) => inner,
            None => parse_primary(rest, trait_name),
        }
    } else {
        parse_primary(rest, trait_name)
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
