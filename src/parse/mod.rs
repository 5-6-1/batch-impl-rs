//! Parsing layer: DSL precedence-climbing parser and angle-bracket generic parsing.
//!
//! # Precedence hierarchy and its invariants
//!
//! Low → high: `;` < `,` < **space** (left-assoc, the Space level) < `.`
//! (right-assoc, the Dot level) < **blocks** (Prim). The chain layers cut
//! between **blocks** ([`crate::parse::space::parse_block`]): a block is the
//! smallest self-contained type fragment, and the space (which is not a
//! token) is recognized by the adjacency of two block starts. List levels
//! (`;` / `,`) cut by stop characters (`parse_operand` → `take_segment`).
//!
//! The invariants that keep the layers sound:
//!
//! 1. Blocks never swallow the type they would apply to (`&mut u8` is the
//!    two blocks `&mut` + `u8`) — except where Rust syntax forces a whole
//!    fragment: lifetime references (`&'a mut u8`) and the fn family
//!    (`fn(u8) -> u8`).
//! 2. All semantic combination happens in **apply** — the parse layer only
//!    cuts blocks. `<>` is a `TyTypeParam` block; whether it is a generic
//!    declaration or a trait/type argument is decided by the apply
//!    combinators (`TyTypeParam` as a left operand declares; a right operand
//!    with bounds/const declares; a plain-type right operand extends).
//! 3. Attachments (`{...}` / `where{...}` / `impl{...}`) are blocks too, so
//!    their position in the chain is irrelevant — the wrapper apply is
//!    "combine the inner, then re-wrap".

mod blocks;
mod chain;
mod generic;
mod ident_blocks;
mod parse_atom;
mod space;
pub(crate) use chain::parse_item;
pub(crate) use generic::split_at_depth0;
pub(crate) use space::*;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};

use crate::ast::*;
use crate::util::{Cursor, compile_error_str};

/// Resolves `@N` / `@g_i` position references inside a token chunk that is
/// **not** parsed as a type (angle-group contents go through flat token
/// splitting in `parse_type_params`, so `Box<@0>` would otherwise keep the
/// raw `@0`). Recurses into groups; every reference folds into the
/// self-delimiting carrier form (`@` + Brace group) that the parse layer and
/// the codegen resolvers both recognize; `@` followed by a non-digit errors.
pub(crate) fn resolve_at_refs(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let at_span = p.span();
                match tokens.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        let lit_str = lit.to_string();
                        // `@N..` open range / `@N..M` / `@N..=M` closed range, or
                        // the grouped forms `@L_N..` / `@L_N..M` / `@L_N..=M`
                        // (within generator group L — stable across array
                        // dispatch) → the structured carrier `@{...}`. The
                        // Brace group is an atomic unit, so a range may appear
                        // anywhere a single `@N` can (`Wrapper<@0..>`,
                        // `<@0.. as T>::Scalar`).
                        let range_lit = parse_range_literal(&lit_str);
                        if let Some((group, start)) = range_lit
                            && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                            && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        {
                            let mut consumed = 4;
                            // `@N..=M`: an `=` after the dots
                            if matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=')
                            {
                                consumed += 1;
                            }
                            // closed `@N..M` / `@N..=M`: an end literal
                            let end = match tokens.get(i + consumed) {
                                Some(TokenTree::Literal(el)) => {
                                    let Some(e) = el.to_string().parse::<usize>().ok() else {
                                        return Err(compile_error_str(
                                            "batch-impl: a `@N..M` range must end with a number (e.g. `@0..=2`)",
                                            at_span,
                                        ));
                                    };
                                    consumed += 1;
                                    crate::ast::fresh::FreshEnd::Closed(e)
                                }
                                _ => crate::ast::fresh::FreshEnd::Open,
                            };
                            let r = crate::ast::fresh::FreshRef { group, start, end };
                            out.extend(crate::ast::fresh::fresh_ref_tokens(r, at_span));
                            i += consumed;
                            continue;
                        }
                        let r = parse_single_ref_token(&lit_str).ok_or_else(|| {
                            compile_error_str(
                                "batch-impl: `@` in a type must be followed by a \
                                 position digit (e.g. `@0` or `@0_1`)",
                                at_span,
                            )
                        })?;
                        out.extend(crate::ast::fresh::fresh_ref_tokens(r, at_span));
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

/// Parses the literal after `@` in a range reference: `N` (flat) or `L_N`
/// (grouped, like `@g_i`). Returns `(group, start)` — `group: None` for the
/// flat form. Only digits-with-optional-underscore shapes qualify; anything
/// else (a bare digit is handled by the single-`@N` path) returns `None`.
pub(crate) fn parse_range_literal(s: &str) -> Option<(Option<usize>, usize)> {
    if let Ok(n) = s.parse::<usize>() {
        return Some((None, n));
    }
    let (l, n) = s.split_once('_')?;
    Some((Some(l.parse::<usize>().ok()?), n.parse::<usize>().ok()?))
}

/// Parses a single-position reference literal (`N` / `g_i`) into its
/// structured form — the token-chunk counterpart of `parse::blocks`'s
/// `parse_single_ref`.
fn parse_single_ref_token(lit: &str) -> Option<crate::ast::fresh::FreshRef> {
    use crate::ast::fresh::{FreshEnd, FreshRef};
    if let Ok(n) = lit.parse::<usize>() {
        return Some(FreshRef { group: None, start: n, end: FreshEnd::Single });
    }
    let (l, i) = lit.split_once('_')?;
    Some(FreshRef { group: Some(l.parse().ok()?), start: i.parse().ok()?, end: FreshEnd::Single })
}

/// The Prim level parses one block — a stable entry (reachable via
/// `parse_item(Prim)`); the chain layers live in `chain.rs` / `space.rs`.
/// A token that cannot open a block falls through to the primitive
/// validation (`-` retirement, stray `;`/`=`/`@`/`#`, ...).
pub(crate) fn parse_primitive(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    let mut cursor = Cursor::new(tokens);
    parse_block(&mut cursor, trait_name).unwrap_or_else(|| crate::parse::generic::primitive(tokens))
}

#[cfg(test)]
mod tests {
    // Verify the fn-family / trait-object block branches parse without error.
    // These run the real parse pipeline (angle_collect -> parse_item).
    fn parse_ok(s: &str) {
        let ts: proc_macro2::TokenStream = s.parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = super::parse_item(&mut c, crate::ast::Op::Comma, None);
        assert!(ty.is_some(), "parse failed for: {s}");
    }

    #[test]
    fn fn_mut_parses() {
        parse_ok("dyn FnMut(u8) -> u8");
    }

    #[test]
    fn fn_once_parses() {
        parse_ok("dyn FnOnce(u8) -> u8");
    }

    #[test]
    fn impl_trait_parses() {
        parse_ok("impl Fn(u8) -> u8");
        parse_ok("impl Iterator + Clone");
    }

    #[test]
    fn for_hrtb_parses() {
        parse_ok("for<'a> fn(&'a u8) -> &'a u8");
    }

    #[test]
    fn prefix_puncts_parse() {
        // `?` / `!` prefix puncts are passthrough blocks; `self` is the
        // identity prefix (`self.T` => `T`).
        parse_ok("?Sized");
        parse_ok("! u8");
        parse_ok("self u8");
        parse_ok("self.Box u8");
    }
}
