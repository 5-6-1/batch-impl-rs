//! Codegen postprocess: transformations over `ImplParts` after extraction.
//! Trait generic substitution (`From<bool>`: `value: T` → `value: bool` in
//! directive-copied bodies) lives here — `ImplParts` carries both the trait
//! arg names (`trait_generic_names`) and the full body (fn signature + user
//! code block), so the substitution needs no plumbing through preprocess.

use proc_macro2::{Ident, TokenStream, TokenTree};

use crate::codegen::impl_parts::ImplParts;

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
