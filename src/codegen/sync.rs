//! `X<>` (empty angle brackets) → `X<spec args>`, **switched on by a
//! switch template** (`impl{@trait<>}` / `impl{Tr<>}` — the empty-bracket
//! spec trait alone). While the switch is on, every `X<>` — the spec's own
//! trait or any other ident — fills with the spec trait application's
//! arguments (parsed from the spec's trait part — no state). Without a
//! switch, no `X<>` is touched at all. A trait application with no
//! arguments syncs to the bare ident (brackets dropped).
//!
//! `@trait<>` (preprocessing) expands to the trait path + `<>`, then this
//! pass fills the brackets.

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;

use crate::ast::{Ty, TyGeneric, TyKind, TyPrimitive, TyTrait, TyTypeParam};
use crate::util::is_punct_at;

/// The spec trait's last path-segment ident — the name that marks a
/// **switch template** (`impl{Tr<>}` triggers body sync).
pub(crate) fn trait_last_ident(trait_name: &TokenStream) -> Option<Ident> {
    let mut last = None;
    for t in trait_name.clone() {
        if let TokenTree::Ident(id) = t {
            last = Some(id);
        }
    }
    last
}

/// Fills every `X<>` in `tokens` with `X<args>` (called only while a switch
/// template is present). `args` are the spec trait's arguments — empty when
/// the trait application has none, the brackets are then dropped.
pub(crate) fn sync_trait_application(
    tokens: TokenStream, args: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    let v = tokens.into_iter().collect::<Vec<_>>();
    sync_at(&v, args, 0).map(|o| o.into_iter().collect())
}

/// Whether `tokens[i]` is an empty angle bracket pair (the pairing output of
/// `angle_collect` — `Semiring<>` in a where predicate is `Ident` + an empty
/// `delimiter![<>]` group; in an `impl{...}` template — which `angle_collect`
/// never enters — it stays flat `Ident < >`).
fn empty_angle_at(tokens: &[TokenTree], i: usize) -> bool {
    matches!(tokens.get(i), Some(TokenTree::Group(g))
        if g.delimiter() == delimiter![<>] && g.stream().is_empty())
        || (is_punct_at(tokens, i, '<') && is_punct_at(tokens, i + 1, '>'))
}

fn sync_at(
    tokens: &[TokenTree], args: &[TokenStream], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        let is_ident_angle =
            matches!(&tokens[i], TokenTree::Ident(_)) && empty_angle_at(tokens, i + 1);
        if is_ident_angle {
            // `is_ident_angle` guarantees an Ident here; the `let else`
            // keeps the no-panic promise on any internal drift.
            let Some(TokenTree::Ident(id)) = tokens.get(i) else {
                return Err(crate::util::depth_err(tokens, ""));
            };
            // Fill the brackets with the spec's trait args; a trait
            // application with no args drops the brackets (`X<>` → `X`).
            let mut ts = quote!(#id);
            if !args.is_empty() {
                ts.extend(quote!(<#(#args),*>));
            }
            out.extend(ts);
            i += if matches!(tokens[i + 1], TokenTree::Group(_)) { 2 } else { 3 };
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > crate::util::MAX_NEST_DEPTH {
                return Err(crate::util::depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let synced = sync_at(&inner, args, depth + 1)?;
            let mut ng = Group::new(g.delimiter(), synced.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    Ok(out)
}

/// Whether a template is a **switch template** (`impl{Tr<>}` — the
/// empty-bracket spec trait alone): it does not match Self like an ordinary
/// shape template; it only syncs `Tr<>` → `Tr<...>` and turns on body sync.
/// Both the flat `Ident < >` shape (impl templates are never angle-paired)
/// and the paired empty-group shape are recognized; the trait ident may be
/// path-qualified (`impl{mod::Tr<>}` — `@trait` expands to the full path).
pub(crate) fn is_switch_template(tokens: &[TokenTree], trait_ident: &Ident) -> bool {
    // find the last ident — the (possibly path-qualified) trait name
    let Some(idx) = tokens.iter().rposition(|t| matches!(t, TokenTree::Ident(_))) else {
        return false;
    };
    let Some(TokenTree::Ident(id)) = tokens.get(idx) else {
        return false;
    };
    if id != trait_ident {
        return false;
    }
    // the ident must be followed by an empty `<>` pair (flat or group)
    match &tokens[idx + 1..] {
        [TokenTree::Punct(lt), TokenTree::Punct(gt)] => lt.as_char() == '<' && gt.as_char() == '>',
        [TokenTree::Group(g)] => g.delimiter() == delimiter![<>] && g.stream().is_empty(),
        _ => false,
    }
}

/// Syncs an empty `X<>` in an impl-generic **bound** Ty (called only while a
/// switch template is present). Unlike where predicates / templates
/// (TokenStream passthrough — the empty brackets survive as tokens), a
/// bound is parsed by the DSL: an `X<>` becomes an empty-param `TyTrait` /
/// `TyGeneric` — and rendering drops the empty brackets (`params_to_tokens`
/// renders only the base when params and bindings are empty). This works on
/// the Ty structure: every empty-param `TyTrait` / `TyGeneric` gets the
/// spec's trait args filled in.
pub(crate) fn sync_bound_ty(ty: &Ty, args: &[TokenStream]) -> Result<Ty, TokenStream> {
    match &ty.kind {
        TyKind::Generic(g) if g.1.params.is_empty() && g.1.bindings.is_empty() => {
            Ok(TyGeneric(g.0.clone(), filled_params(args)).to_ty().with_span(ty.span))
        }
        TyKind::Trait(t) if t.1.params.is_empty() && t.1.bindings.is_empty() => {
            Ok(TyTrait(t.0.clone(), filled_params(args)).to_ty().with_span(ty.span))
        }
        // Any other bound shape: leave as-is (a `Wrapper<X<>>` nested empty
        // bracket is out of scope for now — it renders as the bare `X`).
        _ => Ok(ty.clone()),
    }
}

/// The spec's trait args as a filled `TyTypeParam` (each arg a bare
/// `TyPrimitive`).
fn filled_params(args: &[TokenStream]) -> TyTypeParam {
    TyTypeParam {
        params: args.iter().map(|a| (Box::new(TyPrimitive(a.clone()).to_ty()), None)).collect(),
        bindings: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn args(list: &[&str]) -> Vec<TokenStream> {
        list.iter().map(|a| a.parse::<TokenStream>().unwrap()).collect()
    }

    #[test]
    fn where_predicate_fills_args() {
        // after angle_collect, `Semiring<>` is Ident + an empty None group;
        // the flat `Semiring < >` spelling (as here) is handled the same way
        let ts = "@0.. : Semiring < >".parse::<TokenStream>().unwrap();
        let out = sync_trait_application(ts, &args(&["Additive", "Multiplicative"])).unwrap();
        assert_eq!(out.to_string(), "@ 0 .. : Semiring < Additive , Multiplicative >");
    }

    #[test]
    fn bare_trait_without_args_drops_brackets() {
        let ts = "@0.. : Sized < >".parse::<TokenStream>().unwrap();
        let out = sync_trait_application(ts, &[]).unwrap();
        assert_eq!(out.to_string(), "@ 0 .. : Sized");
    }

    #[test]
    fn other_ident_fills() {
        // any `X<>` — not just the spec's own trait — gets the spec's args
        let ts = "@0.. : Other < >".parse::<TokenStream>().unwrap();
        let out = sync_trait_application(ts, &args(&["Additive"])).unwrap();
        assert_eq!(out.to_string(), "@ 0 .. : Other < Additive >");
    }

    #[test]
    fn flat_template_shape() {
        // impl{...} templates are not angle-paired: flat `Ident < >`
        let ts = "impl { Semiring < > }".parse::<TokenStream>().unwrap();
        let out = sync_trait_application(ts, &args(&["Additive", "Multiplicative"])).unwrap();
        assert_eq!(out.to_string(), "impl { Semiring < Additive , Multiplicative > }");
    }

    #[test]
    fn switch_template_flat() {
        let ts = "Tr < >".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
    }

    #[test]
    fn switch_template_group() {
        let ts = "Tr < >".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
    }

    #[test]
    fn switch_template_path_qualified() {
        // `@trait` expands to the full path (batch_impl_only external paths):
        // `mod :: Tr < >` — the switch must still be recognized
        let ts = "mod :: Tr < >".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
        // deeper path
        let ts = "crate :: ext :: Tr < >".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
    }

    #[test]
    fn switch_template_not_recognized() {
        // a filled template is not a switch
        let ts = "Tr < Additive >".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(!is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
        // a different name is not a switch
        let ts = "Other < >".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(!is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
        // a plain ident (no brackets) is not a switch
        let ts = "Tr".parse::<TokenStream>().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        assert!(!is_switch_template(&v, &Ident::new("Tr", proc_macro2::Span::call_site())));
    }

    #[test]
    fn other_trait_untouched() {
        // a non-empty angle group is not an `X<>` — untouched
        let ts = "@0.. : Module < (), () >".parse::<TokenStream>().unwrap();
        let out = sync_trait_application(ts, &args(&["Additive"])).unwrap();
        assert_eq!(out.to_string(), "@ 0 .. : Module < () , () >");
    }

    #[test]
    fn bound_ty_fills_args() {
        // `<T: BoundSync<>>` — an empty-param TyGeneric gets the spec's args
        let base = TyPrimitive(quote!(BoundSync)).to_ty();
        let empty = TyTypeParam { params: vec![], bindings: vec![] };
        let bound = TyGeneric(Box::new(base), empty).to_ty();
        let out = sync_bound_ty(&bound, &args(&["Additive", "Multiplicative"])).unwrap();
        assert_eq!(out.to_token_stream().to_string(), "BoundSync < Additive , Multiplicative >");
    }

    #[test]
    fn bound_trait_ty_fills_args() {
        // the actual bound shape of `<T: BoundSync<>>`
        let tp = TyTypeParam { params: vec![], bindings: vec![] };
        let bound = TyTrait(quote!(BoundSync), tp).to_ty();
        let out = sync_bound_ty(&bound, &args(&["Additive", "Multiplicative"])).unwrap();
        assert_eq!(out.to_token_stream().to_string(), "BoundSync < Additive , Multiplicative >");
    }

    #[test]
    fn bound_ty_wrong_name_untouched() {
        // a non-empty bound (not an `X<>`) stays untouched
        let base = TyPrimitive(quote!(Module)).to_ty();
        let params = vec![(Box::new(TyPrimitive(quote!(A)).to_ty()), None)];
        let tp = TyTypeParam { params, bindings: vec![] };
        let bound = TyGeneric(Box::new(base), tp).to_ty();
        let out = sync_bound_ty(&bound, &args(&["Additive"])).unwrap();
        assert_eq!(out.to_token_stream().to_string(), "Module < A >");
    }

    #[test]
    fn bound_other_name_fills() {
        // an empty `X<>` bound for a non-spec ident still gets the args
        let tp = TyTypeParam { params: vec![], bindings: vec![] };
        let bound = TyTrait(quote!(Module), tp).to_ty();
        let out = sync_bound_ty(&bound, &args(&["Additive", "Multiplicative"])).unwrap();
        assert_eq!(out.to_token_stream().to_string(), "Module < Additive , Multiplicative >");
    }
}
