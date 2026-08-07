//! `#blanket` blanket-delegation directive: parses the wrapper list and emits
//! one complete delegation spec per wrapper.
//!
//! Complements `expand_directive`'s `Vec<TokenTree>` return contract: the
//! output is multiple complete specs (comma-separated) that can only stand
//! alone as specs (self-contained generics/target/delegation; see the
//! attachment semantics under "syntax-domain isolation" in architecture.md).

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::ast::fresh_param;
use crate::preprocess::{
    angle_collect, build_from_item, collect_call_args, get_trait_item,
    parse_blanket_wrappers, parse_names_from_tokens,
};
use crate::util::{compile_err, compile_error_str};

/// `#blanket(@all){&,Box,Rc}` — blanket delegation: emits one complete spec
/// per wrapper type.
///
/// Equivalent to hand-writing `<T: Trait> wrapper^T #delegate(selected){*…*self}`
/// for each wrapper — no wrapper matrix or delegation bodies to write.
/// Wrapper elements are **arbitrary type expressions** (`&`/`&mut`/`Box`/`Rc`/
/// `Arc`/`MyPtr`/`Box^Arc`/`Cow<'_>` etc.), applied to a fresh generic via
/// `^T`: target type = wrapper expression + `^T` (`Box^Arc:2` → `Box<Arc<T>>`,
/// `Cow<'_>` → `Cow<'_, T>`). **Nested wrappers must be chained with `^`**
/// (`Box^Arc`); `<` prefilling is append semantics (`Box<Arc>^T` =
/// `Box<Arc, T>`, an error).
///
/// Deref depth of the delegation body: `:N` annotation (`Box^Arc:2`) or the
/// default 1 — `*` count = N + 1 (self is `&wrapper<T>`; deref the self
/// reference, then N wrapper layers). The default is always 1; the macro never
/// guesses inner Deref layers; nested wrappers must be explicit `:N` (a
/// mistake degrades to a rustc "method not found" error, as warned in the
/// docs). `*const`/`*mut` (safe code cannot deref a raw pointer to delegate),
/// `self` (meaningless), empty elements, and invalid `:N` all error.
///
/// **Generic traits** (`trait Foo<X> where X: Clone`) are supported: the trait
/// params are copied into the impl generics (params first, fresh `T` last —
/// `T: Foo<X>` references X; reversed order is E0401), args = param names;
/// trait-level where predicates passthrough into the impl where clause.
/// **Assoc type/const delegation**: when `@all` includes const/type items, a
/// projection is emitted — `type Item = <T as Foo<X>>::Item;` /
/// `const N: Ty = <T as Foo<X>>::N;` (not through self), solving "cannot
/// delegate traits with required associated types".
/// By-value receiver methods (`fn consume(self)`) delegation semantics depend
/// on the wrapper's Deref/move capability, indistinguishable at macro
/// expansion time — keep full pass-through + rustc fallback.
pub(crate) fn expand_blanket(
    args_group: &Group, body: &Group, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    // body is a Brace group (`angle_collect` does not enter), so flat `<...>`
    // such as `Cow<'_>` inside were not paired — do one pairing pass here
    // (body is an independent fragment; pairing is safe and side-effect-free).
    let body_tokens = angle_collect(&body.stream().into_iter().collect::<Vec<_>>())?;
    let wrappers = parse_blanket_wrappers(&body_tokens)?;
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
    // Fresh generic: avoids clashing with other names (same mechanism as the
    // `()^N` tuple generic)
    let t = fresh_param();

    // Generic trait copy: param order = trait params first, fresh T last
    // (`T: Foo<X>` references X; reversed order is E0401).
    let generics = &trait_def.generics;
    let param_names = crate::analyze::generic_param_names(generics);
    // T's bound: `Trait<X>` (with args) or bare `Trait`.
    // Args must be grouped into an angle group (same as trait_part) — once
    // grouped, parsing is correct without relying on idempotence.
    let t_bound = trait_with_args(trait_full_path, &param_names);
    // blanket runs after angle_collect and its output is no longer paired,
    // so groups must be built manually. The bound uses trait_full_path (with
    // `#[batch_impl_only(#ext::Trait: ...)]` it is an external path; a local
    // dummy trait name would not resolve in the path-prefix scenario).
    // `<>` keeps only names: generic TypeParams take just the ident,
    // const/lifetime as-is (`const N: usize` needs the full declaration; a
    // bare name `N` is E0747), + fresh T; all bounds (trait param inline
    // bounds + `T: Trait` + trait where) go into where.
    let impl_names: Vec<TokenStream> = generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Type(tp) => {
                let id = &tp.ident;
                quote!(#id)
            }
            syn::GenericParam::Const(cp) => quote!(#cp),
            syn::GenericParam::Lifetime(ld) => quote!(#ld),
        })
        .collect();
    let impl_generics = if impl_names.is_empty() {
        Group::new(delimiter![<>], quote!(#t))
    } else {
        Group::new(delimiter![<>], quote!(#(#impl_names),* , #t))
    };
    // Base where predicates: `T: Trait` (trait param inline bounds are handled
    // by codegen's bound-inheritance logic — the blanket spec generic X has no
    // bound, inheritance adds `X: Clone`; moving them here too would duplicate
    // the inheritance)
    let base_preds: Vec<TokenStream> = vec![quote!(#t : #t_bound)];
    // The trait-name part of the spec: only needed for generic traits (pass
    // args `Trait<X>`); omitted for non-generic traits (batch_impl output
    // auto-appends the trait name — and a prefix wrapper `&^T` as target
    // cannot follow the trait name; `Trait &^T` would not parse)
    let trait_part = if param_names.is_empty() {
        quote!()
    } else {
        trait_with_args(trait_full_path, &param_names)
    };
    // The `T as Trait<X>` form for assoc-item projections
    let as_trait = if param_names.is_empty() {
        quote!(#t as #trait_full_path)
    } else {
        quote!(#t as #trait_full_path < #(#param_names),*>)
    };

    let mut spec_streams = vec![];
    for wrapper in &wrappers {
        let star = "*".repeat(wrapper.depth + 1);
        let self_ty: TokenStream = format!("{}self", star).parse().unwrap();
        // Wrapper where predicates: `@0` → target generic name; merged into
        // where (zero-analysis parallel merge)
        let wrapper_preds = match &wrapper.where_preds {
            Some(preds) => resolve_target_predicates(preds, trait_full_path)?,
            None => vec![],
        };
        // Insert predicate streams as wholes (commas between predicates are
        // already in the token streams; cannot connect with per-token commas)
        let mut where_streams = base_preds.clone();
        if let Some(wc) = &generics.where_clause {
            let preds = &wc.predicates;
            where_streams.push(quote!(#preds));
        }
        if !wrapper_preds.is_empty() {
            let wrapper_stream: TokenStream = wrapper_preds.into_iter().collect();
            where_streams.push(wrapper_stream);
        }
        let where_part = if where_streams.is_empty() {
            quote!()
        } else {
            quote!(where { #(#where_streams),* })
        };
        let mut methods = TokenStream::new();
        for name in &method_names {
            let item = get_trait_item(trait_def, name)?;
            match item {
                // Method: deref delegation; static methods (no receiver)
                // delegate through the fresh generic `t` — the same forwarding
                // as assoc items (`t::make(...)`), valid because the blanket
                // impl carries the `t: Trait` bound.
                syn::TraitItem::Fn(f) => {
                    let sig = f.sig.clone();
                    let call_args = collect_call_args(&sig).map_err(|pat| {
                        compile_err!(
                            "batch-impl: #blanket method `{}::{}` param `{}` cannot be \
                             forwarded: only `self` and plain identifier patterns are supported",
                            trait_def.ident, name, pat
                        )
                    })?;
                    let body = if f.sig.receiver().is_none() {
                        quote! { #t :: #name ( #(#call_args),* ) }
                    } else {
                        quote! { (#self_ty) . #name ( #(#call_args),* ) }
                    };
                    methods.extend(build_from_item(item, &body));
                }
                // Assoc type/const: projection (not through self)
                syn::TraitItem::Type(_) | syn::TraitItem::Const(_) => {
                    let body = quote! { < #as_trait >::#name };
                    methods.extend(build_from_item(item, &body));
                }
                // Theoretically unreachable (trait defs only have
                // fn/const/type); defensive error
                _ => {
                    return Err(compile_err!(
                        "batch-impl: #blanket does not support `{}` in trait `{}` \
                         (unknown item form)",
                        trait_def.ident,
                        name
                    ));
                }
            }
        }
        let wrapper_ty = &wrapper.ty;
        // `@0` in the wrapper's main part marks the target position: replace
        // it with the fresh generic and emit the wrapper as-is (so `T` can sit
        // anywhere, e.g. `(u32, @0, u8)`). Without `@0` the wrapper is applied
        // as `wrapper^T` (target appended last) — the existing behavior.
        let wrapper_vec: Vec<_> = wrapper_ty.clone().into_iter().collect();
        let target: TokenStream = if has_at0(&wrapper_vec) {
            replace_at0(&wrapper_vec, &t).into_iter().collect()
        } else {
            quote!(#wrapper_ty ^ #t)
        };
        spec_streams.push(quote! {
            #impl_generics #trait_part #target #where_part { #methods }
        });
    }
    Ok(quote!(#(#spec_streams),*).into_iter().collect())
}

/// Whether a wrapper's main part contains the `@0` target marker (`@` +
/// literal `0`, possibly nested inside groups).
fn has_at0(tokens: &[TokenTree]) -> bool {
    let v: Vec<_> = tokens.to_vec();
    v.iter().enumerate().any(|(i, tt)| match tt {
        TokenTree::Punct(p) if p.as_char() == '@' => {
            matches!(v.get(i + 1), Some(TokenTree::Literal(l)) if l.to_string() == "0")
        }
        TokenTree::Group(g) => {
            has_at0(&g.stream().into_iter().collect::<Vec<_>>())
        }
        _ => false,
    })
}

/// Replaces every `@0` in the wrapper's main part with the blanket's fresh
/// target generic name (recursing into groups).
fn replace_at0(tokens: &[TokenTree], t: &TokenStream) -> Vec<TokenTree> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p)
                if p.as_char() == '@'
                    && matches!(
                        tokens.get(i + 1),
                        Some(TokenTree::Literal(l)) if l.to_string() == "0"
                    ) =>
            {
                out.extend(t.clone());
                i += 2;
            }
            TokenTree::Group(g) => {
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                let mut new_g = Group::new(
                    g.delimiter(),
                    replace_at0(&inner, t).into_iter().collect(),
                );
                new_g.set_span(g.span());
                out.push(new_g.into());
                i += 1;
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    out
}

/// `Trait<X, Y>` with grouped angle args — blanket runs after `angle_collect`
/// and its output is no longer paired, so the group is built manually. An
/// empty param list yields the bare path.
fn trait_with_args(path: &TokenStream, param_names: &[TokenStream]) -> TokenStream {
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
fn resolve_target_predicates(
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
                Some(TokenTree::Literal(lit))
                    if lit.to_string().parse::<usize>().is_ok() =>
                {
                    out.push(preds[i].clone());
                    out.push(preds[i + 1].clone());
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
