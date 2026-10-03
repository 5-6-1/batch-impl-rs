//! `X<>` (empty angle brackets) → `X<spec args>`. Every `X<>` — the spec's own
//! trait or any other ident — fills with the spec trait application's arguments
//! (parsed from the spec's trait part — no state). A trait application with no
//! arguments syncs to the bare ident (brackets dropped).
//!
//! **Which surfaces**: where predicates and `impl{...}` templates fill as tokens
//! (`sync_trait_application`), and everything in the **type structure** — the
//! target type and the impl-generic bounds — fills through [`sync_tree`] /
//! [`sync_bound_ty`], because there the `X<>` is already a parsed empty-param
//! node. A **switch template** (`impl{@trait<>}` / `impl{Tr<>}`, the
//! empty-bracket spec trait alone) additionally turns on **body** sync.
//!
//! `@trait<>` (preprocessing) expands to the trait path + `<>`, then this
//! pass fills the brackets.

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;

use crate::ast::{Ty, TyBoundList, TyGeneric, TyKind, TyPrimitive, TyTrait, TyTypeParam};
use crate::codegen::extract::ImplParts;
use crate::util::{is_punct_at, slice_from};

/// `X<>` sync across an [`ImplParts`]: where predicates, `impl{...}`
/// templates, impl-generic bounds **and the target type** fill with the spec trait
/// application's arguments. A **switch template** (`impl{@trait<>}` / `impl{Tr<>}`
/// — the empty-bracket spec trait alone) additionally turns on **body** sync; the
/// switch itself is consumed (it does not match Self like an ordinary shape
/// template). Returns `Err` on a sync error (reported by the caller), and otherwise
/// the synced **templates** — the caller parses them next
/// (`render::parse_impl_templates`), because `impl{GenW<>}` is only valid Rust
/// after this pass.
pub(crate) fn sync_impl_parts(
    parts: &mut ImplParts, trait_name: &TokenStream,
) -> Result<Vec<TokenStream>, TokenStream> {
    let Some(trait_ident) = trait_last_ident(trait_name) else {
        return Ok(vec![]);
    };
    let trait_args = parts.trait_generic_names.clone();
    let mut body_sync = false;
    let mut matched = Vec::new();
    for t in std::mem::take(&mut parts.impl_templates) {
        let is_switch =
            is_switch_template(&t.clone().into_iter().collect::<Vec<_>>(), &trait_ident);
        let s = sync_trait_application(t, &trait_args)?;
        if is_switch {
            body_sync = true;
        } else {
            matched.push(s);
        }
    }
    let mut synced = Vec::with_capacity(parts.where_clauses.len());
    for w in &parts.where_clauses {
        synced.push(sync_trait_application(w.clone(), &trait_args)?);
    }
    parts.where_clauses = synced;
    // Empty brackets in the **type structure** take the spec's args too — the
    // rule is a property of the node, not of the surface it sits on. That covers
    // impl-generic bounds (`<T: Marker<>>`) and the target type, including a
    // `dyn … + Marker<>` tail: while the tail was a token bag the sync never saw
    // it, so `Box<dyn Marker<> + Send>` rendered `Box<dyn Marker + Send>` — the
    // marker vanished silently (measured). One post-order walk (`sync_tree`)
    // applies `sync_bound_ty` wherever it can change something, and accumulates
    // the first error instead of unwinding (`map_children` cannot return).
    let mut sync_err = None;
    for (_, bound) in &mut parts.impl_generics {
        if let Some(b) = bound {
            *b = sync_tree(b.clone(), &trait_args, &mut sync_err);
        }
    }
    if sync_err.is_none() {
        parts.target_type = sync_tree(parts.target_type.clone(), &trait_args, &mut sync_err);
    }
    if let Some(e) = sync_err {
        return Err(e);
    }
    if body_sync && let Some(b) = &mut parts.body {
        *b = sync_trait_application(b.clone(), &trait_args)?;
    }
    Ok(matched)
}

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
    while let Some(cur) = tokens.get(i) {
        // `Ident` + an empty `<>` (paired group or flat `< >`) — extract the
        // ident and the advance distance in one step (the check and the
        // destructure share the same match, so they cannot drift).
        let ident_angle = match cur {
            TokenTree::Ident(id) if empty_angle_at(tokens, i + 1) => {
                // 2 tokens for a paired group, 3 for flat `< >`.
                let adv =
                    if matches!(tokens.get(i + 1), Some(TokenTree::Group(_))) { 2 } else { 3 };
                Some((id.clone(), adv))
            }
            _ => None,
        };
        if let Some((id, adv)) = ident_angle {
            // Fill the brackets with the spec's trait args; a trait
            // application with no args drops the brackets (`X<>` → `X`).
            let mut ts = quote!(#id);
            if !args.is_empty() {
                ts.extend(quote!(<#(#args),*>));
            }
            out.extend(ts);
            i += adv;
            continue;
        }
        if let TokenTree::Group(g) = cur {
            if depth + 1 > crate::util::MAX_NEST_DEPTH {
                return Err(crate::util::depth_err(std::slice::from_ref(cur), ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let synced = sync_at(&inner, args, depth + 1)?;
            let mut ng = Group::new(g.delimiter(), synced.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(cur.clone());
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
    match slice_from(tokens, idx + 1) {
        [TokenTree::Punct(lt), TokenTree::Punct(gt)] => lt.as_char() == '<' && gt.as_char() == '>',
        [TokenTree::Group(g)] => g.delimiter() == delimiter![<>] && g.stream().is_empty(),
        _ => false,
    }
}

/// Whether a param list holds nothing at all — the shape an `X<>` parses to
/// (both params and bindings empty), which is what the renderer drops. The single
/// predicate [`sync_bound_ty`] and the tree walk both ask.
fn is_empty_params(tp: &TyTypeParam) -> bool {
    tp.params.is_empty() && tp.bindings.is_empty()
}

/// Applies [`sync_bound_ty`] at every node of `ty`, children first: the
/// empty-bracket rule is position-independent, so one post-order pass covers a
/// `dyn … + X<>` tail, a bound nested in a generic argument, and so on. The first
/// error is accumulated rather than unwound, because the traversal authority
/// ([`Ty::map_children`]) cannot return.
fn sync_tree(ty: Ty, args: &[TokenStream], err: &mut Option<TokenStream>) -> Ty {
    let ty = ty.map_children(&mut |c| sync_tree(c, args, err));
    if err.is_some() {
        return ty;
    }
    // Only an empty-bracket node can change — calling the sync on everything
    // would clone each subtree for nothing (the review already flagged that
    // shape in `collect_errors`).
    let can_fill = match &ty.kind {
        TyKind::Generic(g) => is_empty_params(&g.1),
        TyKind::Trait(t) => is_empty_params(&t.1),
        _ => false,
    };
    if !can_fill {
        return ty;
    }
    match sync_bound_ty(&ty, args) {
        Ok(t) => t,
        Err(e) => {
            *err = Some(e);
            ty
        }
    }
}

/// Syncs an empty `X<>` in a **bound** Ty (called only while a switch template is
/// present). Unlike where predicates / templates (TokenStream passthrough — the
/// empty brackets survive as tokens), a bound is parsed by the DSL: an `X<>`
/// becomes an empty-param `TyTrait` / `TyGeneric` — and rendering drops the empty
/// brackets (`params_to_tokens` renders only the base when params and bindings are
/// empty). This works on the Ty structure: every empty-param `TyTrait` /
/// `TyGeneric` gets the spec's trait args filled in.
pub(crate) fn sync_bound_ty(ty: &Ty, args: &[TokenStream]) -> Result<Ty, TokenStream> {
    match &ty.kind {
        TyKind::Generic(g) if is_empty_params(&g.1) => {
            // The parser only builds a `TyGeneric` for an ident that carries a written
            // `<>` and is *not* the annotated trait (`Vec<>`), while a bare `Vec` stays a
            // `TyPrimitive` and the trait itself is a `TyTrait`. With no trait arguments to
            // fill from, the brackets used to be dropped here, leaving rustc to report
            // E0107 about a *bare* `Vec`, with nothing pointing at the `<>`.
            if args.is_empty() {
                return Err(
                    quote::quote_spanned!(ty.span => ::core::compile_error!("batch-impl: an empty `<>` on an ident that is not the annotated trait has nothing to fill from — write the arguments out, or drop the `<>`");),
                );
            }
            Ok(TyGeneric(g.0.clone(), filled_params(args)).to_ty().with_span(ty.span))
        }
        TyKind::Trait(t) if is_empty_params(&t.1) => {
            Ok(TyTrait(t.0.clone(), filled_params(args)).to_ty().with_span(ty.span))
        }
        // A `+`-joined bound list (`A<> + B + C`): sync every element
        // (the structured list keeps each `X<>` as its own Ty).
        TyKind::BoundList(b) => {
            let elems =
                b.0.iter().map(|e| sync_bound_ty(e, args)).collect::<Result<Vec<_>, _>>()?;
            Ok(TyBoundList(elems).to_ty().with_span(ty.span))
        }
        // Any other bound shape: leave as-is (a `Wrapper<X<>>` nested empty
        // bracket is reached by `sync_tree`'s children pass, not from here).
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

    /// The tree walk reaches empty brackets **nested** in the type structure
    /// (`Box<SyncMarker<>>`) — while a `dyn … + X<>` tail was a token bag the
    /// walk could not, and the marker disappeared silently. One post-order pass
    /// covers a generic argument, a tuple element and the bound list alike.
    #[test]
    fn sync_tree_fills_a_nested_empty_bracket() {
        let flat =
            "Box<SyncMarker<>>".parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
        let paired = crate::preprocess::angle_collect(&flat).unwrap();
        let ty = crate::parse::parse_item(
            &mut crate::util::Cursor::new(&paired),
            crate::ast::Op::Comma,
            crate::parse::Ctx::default(),
        )
        .unwrap();
        let mut err = None;
        let out = sync_tree(ty, &args(&["T"]), &mut err);
        assert!(err.is_none(), "sync error");
        let s = out.to_token_stream().to_string();
        assert!(s.contains("SyncMarker < T >"), "got: {s}");
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

    #[test]
    fn bound_plus_chain_fills_empty_angle() {
        // `A<> + B + C` — the structured bound list syncs each element, so
        // the empty `A<>` fills while the others stay untouched
        let a = TyTrait(quote!(A), TyTypeParam { params: vec![], bindings: vec![] }).to_ty();
        let b = TyPrimitive(quote!(B)).to_ty();
        let c = TyPrimitive(quote!(C)).to_ty();
        let bound = TyBoundList(vec![a, b, c]).to_ty();
        let out = sync_bound_ty(&bound, &args(&["Additive", "Multiplicative"])).unwrap();
        assert_eq!(out.to_token_stream().to_string(), "A < Additive , Multiplicative > + B + C");
    }
}
