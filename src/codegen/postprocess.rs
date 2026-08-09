//! Codegen postprocess: transformations over `ImplParts` after extraction.
//! Trait generic substitution (`From<bool>`: `value: T` → `value: bool` in
//! directive-copied bodies) lives here — `ImplParts` carries both the trait
//! arg names (`trait_generic_names`) and the full body (fn signature + user
//! code block), so the substitution needs no plumbing through preprocess.

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, TokenStream, TokenTree};

use crate::ast::*;
use crate::codegen::impl_parts::ImplParts;
use crate::parse::split_at_depth0;

/// Substitute each trait generic param with its concrete arg in the impl body
/// (the directive-copied fn signature plus the user's code block).
///
/// `trait_param_names` comes from the entry trait definition (`From<T>` →
/// `[T]`), paired positionally with `ImplParts::trait_generic_names` (the
/// spec-level args, `From<bool>` → `[bool]`). Token-level recursive: syn's
/// quote groups parameter tokens, so the replacement descends into groups.
/// Limitation: a *function* generic param that happens to share a trait
/// param's name would be substituted too (rare; renamed params avoid it).
pub(crate) fn substitute_trait_generics(
    parts: &mut ImplParts, trait_param_names: &[Ident],
) {
    let Some(body) = parts.body.take() else {
        return;
    };
    if trait_param_names.is_empty() || parts.trait_generic_names.is_empty() {
        parts.body = Some(body);
        return;
    }
    // Pair type/const param names with their concrete args, skipping lifetime
    // args (`'static` — a TokenStream starting with a `'` punct): bodies
    // reference their own impl lifetimes, never substituted trait args.
    let map = trait_param_names
        .iter()
        .zip(parts.trait_generic_names.iter().filter(|ts| {
            !matches!(
                (*ts).clone().into_iter().next(),
                Some(TokenTree::Punct(p)) if p.as_char() == '\''
            )
        }))
        .map(|(name, arg)| (name.clone(), arg.clone()))
        .collect::<Vec<_>>();
    parts.body = Some(replace_idents(body, &map));
}

/// Recursively replace every ident equal to a mapped trait param name.
fn replace_idents(ts: TokenStream, map: &[(Ident, TokenStream)]) -> TokenStream {
    ts.into_iter()
        .flat_map(|tt| match &tt {
            TokenTree::Ident(id) => map
                .iter()
                .find(|(name, _)| name == id)
                .map(|(_, repl)| repl.clone())
                .unwrap_or_else(|| TokenStream::from(tt.clone())),
            TokenTree::Group(g) => {
                let inner = replace_idents(g.stream(), map);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                TokenStream::from(TokenTree::Group(ng))
            }
            other => TokenStream::from(other.clone()),
        })
        .collect()
}

/// Expand splat elements inside `TyTuple` at the Ty-structure level (the
/// codegen postprocess — parse/apply/expand keep `*()`/`*[]` whole). A splat
/// element becomes its flat elements with fresh declarations hoisted:
/// `(A, *(B,C))` → `(A,B,C)`, `(*(()^3))` → `<P0,P1,P2>(P0,P1,P2)`.
/// Generic args (`T<*(A,B)>`) are handled separately by [`expand_splats`]
/// (token level), because `TyTypeParam` stores params as token streams.
pub(crate) fn expand_splat_elems(ty: Ty) -> Ty {
    let Ty { span, kind } = ty;
    match kind {
        TyKind::Tuple(t) => {
            let mut flat = vec![];
            let mut decl = None;
            for e in t.0 {
                if matches!(e.kind, TyKind::Splat(_)) {
                    let (mut es, d) = splat_expand(e);
                    flat.append(&mut es);
                    decl = merge_decls(decl, d);
                } else {
                    flat.push(expand_splat_elems(e));
                }
            }
            let tuple = TyTuple(flat).to_ty().with_span(span);
            match decl {
                Some(d) => TyWithType(d, tuple.into()).to_ty().with_span(span),
                None => tuple,
            }
        }
        TyKind::Group(g) => {
            TyGroup(Box::new(expand_splat_elems(*g.0))).to_ty().with_span(span)
        }
        TyKind::WithCode(wc) => {
            let inner = wc.0.map(|e| Box::new(expand_splat_elems(*e)));
            TyWithCode(inner, wc.1).to_ty().with_span(span)
        }
        TyKind::WithType(wt) => TyWithType(wt.0, Box::new(expand_splat_elems(*wt.1)))
            .to_ty()
            .with_span(span),
        TyKind::WithTrait(wt) => {
            TyWithTrait(wt.0, Box::new(expand_splat_elems(*wt.1)))
                .to_ty()
                .with_span(span)
        }
        TyKind::WithWhere(ww) => {
            let inner = ww.0.map(|e| Box::new(expand_splat_elems(*e)));
            TyWithWhere(inner, ww.1).to_ty().with_span(span)
        }
        TyKind::WithPrefix(wp) => {
            let inner = wp.1.map(|e| Box::new(expand_splat_elems(*e)));
            TyWithPrefix(wp.0, inner).to_ty().with_span(span)
        }
        TyKind::WithAttr(wa) => {
            let inner = wa.1.map(|e| Box::new(expand_splat_elems(*e)));
            TyWithAttr(wa.0, inner).to_ty().with_span(span)
        }
        // Leaves and token-stream-bearing nodes (Generic / Trait / Splat /
        // PrimitiveArray / Fn / ...) stay — splats in generic args are
        // expanded by `expand_splats` at the token level after rendering.
        other => Ty { span, kind: other },
    }
}

/// Expand residual splats in an impl-header token stream: a `*` punct
/// directly followed by a `(...)` / `[...]` group becomes the group's
/// comma-separated elements. Per the splat-survival principle,
/// parse/apply/expand never flatten `*()`/`*[]`; the single expansion point
/// is this codegen postprocess — `T<*(A,B)>` → `T<A,B>`,
/// `Map<*(K,V)>` → `Map<K,V>`. Recurses into groups so nested splats expand
/// in place. Applied only to impl headers (generics / trait path / target /
/// where) — bodies never go through this, so `a * b` inside a fn body is
/// untouched; `*const T` / `*mut T` (a `*` followed by an ident) stay as-is.
pub(crate) fn expand_splats(tokens: TokenStream) -> TokenStream {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut out = TokenStream::new();
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '*'
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
            && matches!(g.delimiter(), Delimiter::Parenthesis | Delimiter::Bracket)
        {
            // `*(...)` / `*[...]` -> the group's comma-separated elements
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let chunks = split_at_depth0(&inner, ',');
            for (k, chunk) in chunks.iter().enumerate() {
                if k > 0 {
                    out.extend([TokenTree::Punct(Punct::new(',', Spacing::Alone))]);
                }
                out.extend(expand_splats(chunk.iter().cloned().collect()));
            }
            i += 2;
            continue;
        }
        match &tokens[i] {
            TokenTree::Group(g) => {
                let inner = expand_splats(g.stream());
                let mut ng = Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.extend([TokenTree::Group(ng)]);
            }
            other => out.extend([other.clone()]),
        }
        i += 1;
    }
    out
}
