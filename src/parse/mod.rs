//! Parsing layer: DSL precedence-climbing parser and angle-bracket generic parsing.
//!
//! # Precedence hierarchy and its invariants
//!
//! Low → high: `;` < `,` < **space** (left-assoc, the Dash level) < `.`
//! (right-assoc, the Caret level) < **blocks** (Prim). The chain layers cut
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

use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::util::{Cursor, compile_error_str};

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

/// The Prim level parses one block — a stable entry (reachable via
/// `parse_item(Prim)`); the chain layers live in `chain.rs` / `space.rs`.
/// A token that cannot open a block falls through to the primitive
/// validation (`-` retirement, stray `;`/`=`/`@`/`#`, ...).
pub(crate) fn parse_primitive(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    let mut cursor = Cursor::new(tokens);
    parse_block(&mut cursor, trait_name).unwrap_or_else(|| crate::parse::generic::primitive(tokens))
}
