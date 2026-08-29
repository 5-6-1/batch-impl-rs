//! Helper functions of the `#blanket` directive (kept under the 350-line
//! cap by living in their own file): `Self`-return detection, the `@0`
//! target marker, grouped trait-path rendering, and wrapper-where
//! `@trait` resolution.

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::util::compile_error_str;

/// Whether a method's return type references **bare `Self`** (making blanket
/// delegation unsound: the forwarded call returns the inner type, not the
/// wrapper's `Self`). `Self::Assoc` (an associated-type projection) is
/// **allowed**: it resolves through the projected item
/// (`type Output = <T as Trait>::Output;` — the wrapper's `Self::Output` is
/// `T::Output`), so it type-checks when the selection covers the associated
/// item and fails naturally (E0046) when it does not.
pub(crate) fn return_type_refs_self(output: &syn::ReturnType) -> bool {
    match output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => ty_refs_bare_self(ty),
    }
}

/// Whether a method signature (parameters **or** return type) references
/// bare `Self` — the full delegation-unsoundness check: a `Self` parameter
/// (`fn cmp(&self, other: Self)`) forwards the wrapper's `Self`, not the
/// inner type, exactly like a `Self` return. `Self::Assoc` stays allowed
/// (see [`return_type_refs_self`]).
pub(crate) fn sig_refs_bare_self(sig: &syn::Signature) -> bool {
    return_type_refs_self(&sig.output)
        || sig
            .inputs
            .iter()
            .any(|i| matches!(i, syn::FnArg::Typed(pt) if ty_refs_bare_self(&pt.ty)))
}

/// A type's token stream references bare `Self` (not `Self::`) — recursing
/// into groups, so a `Self` inside `(Self, u8)` / `Box<Self>` is caught too
/// (a group is a single top-level token; a top-level-only scan would miss
/// it and emit a delegation that silently forwards the wrong type).
fn ty_refs_bare_self(ty: &syn::Type) -> bool {
    let tokens = ty.to_token_stream().into_iter().collect::<Vec<_>>();
    ty_tokens_refs_bare_self(&tokens)
}

fn ty_tokens_refs_bare_self(tokens: &[TokenTree]) -> bool {
    tokens.iter().enumerate().any(|(i, tt)| match tt {
        TokenTree::Ident(id) if id == "Self" => {
            // `Self::` — an associated projection, allowed (see above)
            !(matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == ':')
                && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == ':'))
        }
        TokenTree::Group(g) => {
            ty_tokens_refs_bare_self(&g.stream().into_iter().collect::<Vec<_>>())
        }
        _ => false,
    })
}

/// Whether a wrapper's main part contains the `@0` target marker (`@` +
/// literal `0`, possibly nested inside groups) — the position decision only;
/// the marker itself is resolved by the parse layer into the fresh name.
pub(crate) fn has_at0(tokens: &[TokenTree]) -> bool {
    let v = tokens.to_vec();
    v.iter().enumerate().any(|(i, tt)| match tt {
        TokenTree::Punct(p) if p.as_char() == '@' => {
            matches!(v.get(i + 1), Some(TokenTree::Literal(l)) if l.to_string() == "0")
        }
        TokenTree::Group(g) => has_at0(&g.stream().into_iter().collect::<Vec<_>>()),
        _ => false,
    })
}

/// `Trait<X, Y>` with grouped angle args — blanket runs after `angle_collect`
/// and its output is no longer paired, so the group is built manually. An
/// empty param list yields the bare path.
pub(crate) fn trait_with_args(path: &TokenStream, param_names: &[TokenStream]) -> TokenStream {
    if param_names.is_empty() {
        quote!(#path)
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#path #args_group)
    }
}

/// Replaces `@trait` in wrapper where predicates with the full trait path
/// (local name, or the `#ext::Trait:` external path for `batch_impl_only`).
/// `@N` position references are **kept as-is** and resolved by codegen's
/// `resolve_where_at` like any user where predicate (blanket's fresh generic
/// is the only fresh, so `@0` indexes it); other tokens after `@` error.
pub(crate) fn resolve_target_predicates(
    preds: &[TokenTree], trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < preds.len() {
        match &preds[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => match preds.get(i + 1) {
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    out.extend(trait_full_path.clone());
                    i += 2;
                }
                // `@0` / `@N`: keep as-is for codegen; other forms error
                Some(TokenTree::Literal(lit)) if lit.to_string().parse::<usize>().is_ok() => {
                    out.push(preds[i].clone());
                    out.push(TokenTree::Literal(lit.clone()));
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: in #blanket wrapper where, `@` must be \
                         followed by a position digit (e.g. `@0`) or `@trait`",
                        preds[i].span(),
                    ));
                }
            },
            _ => {
                out.push(preds[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
