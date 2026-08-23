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

use crate::ast::{fresh_param, take_group};
use crate::preprocess::{
    angle_collect, build_from_item, collect_call_args, get_trait_item, parse_blanket_wrappers,
    parse_names_from_tokens,
};
use crate::util::compile_err;

/// `#blanket(@all){&,Box,Rc}` — blanket delegation: emits one complete spec
/// per wrapper type.
///
/// Equivalent to hand-writing `<T: Trait> wrapper.T #delegate(selected){*…*self}`
/// for each wrapper — no wrapper matrix or delegation bodies to write.
/// Wrapper elements are **arbitrary type expressions** (`&`/`&mut`/`Box`/`Rc`/
/// `Arc`/`MyPtr`/`Box.Arc`/`Cow<'_>` etc.), applied to a fresh generic via
/// `.T`: target type = wrapper expression + `.T` (`Box.Arc:2` → `Box<Arc<T>>`,
/// `Cow<'_>` → `Cow<'_, T>`). **Nested wrappers must be chained with `.`**
/// (`Box.Arc`); `<` prefilling is append semantics (`Box<Arc>.T` =
/// `Box<Arc, T>`, an error).
///
/// Deref depth of the delegation body: `:N` annotation (`Box.Arc:2`) or the
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
    args_group: &Group, body: &Group, trait_def: &ItemTrait, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    // body is a Brace group (`angle_collect` does not enter), so flat `<...>`
    // such as `Cow<'_>` inside were not paired — do one pairing pass here
    // (body is an independent fragment; pairing is safe and side-effect-free).
    let body_tokens = angle_collect(&body.stream().into_iter().collect::<Vec<_>>())?;
    let wrappers = parse_blanket_wrappers(&body_tokens)?;
    let method_names =
        parse_names_from_tokens(&args_group.stream().into_iter().collect::<Vec<_>>(), trait_def)?;
    // Fresh generic: avoids clashing with other names (same mechanism as the
    // `().N` tuple generic); group 0 position 0 — the blanket is the spec's
    // only fresh generator, and the codegen sweeper renumbers it to
    // `_Param_0_BatchGen_`.
    let t = fresh_param(take_group(), 0);

    // Generic trait copy: param order = trait params first, fresh T last
    // (`T: Foo<X>` references X; reversed order is E0401).
    let generics = &trait_def.generics;
    let param_names = crate::analyze::generic_param_names(generics);
    // T's bound: `Trait<X>` (with args) or bare `Trait`.
    // Args must be grouped into an angle group (same as trait_part) — once
    // grouped, parsing is correct without relying on idempotence.
    let t_bound = crate::preprocess::directives::blanket_helpers::trait_with_args(
        trait_full_path,
        &param_names,
    );
    // blanket runs after angle_collect and its output is no longer paired,
    // so groups must be built manually. The bound uses trait_full_path (with
    // `#[batch_impl_only(#ext::Trait: ...)]` it is an external path; a local
    // dummy trait name would not resolve in the path-prefix scenario).
    // `<>` keeps only names: generic TypeParams take just the ident,
    // const/lifetime as-is (`const N: usize` needs the full declaration; a
    // bare name `N` is E0747), + fresh T; all bounds (trait param inline
    // bounds + `T: Trait` + trait where) go into where.
    // Full const/lifetime declarations (`const N: usize` needs the full form;
    // a bare name `N` is E0747) + fresh T; all bounds (trait param inline
    // bounds + `T: Trait` + trait where) go into where. Note this is NOT the
    // same as `generic_param_names` (which yields bare names for matching).
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
    // auto-appends the trait name — and a prefix wrapper `&.T` as target
    // cannot follow the trait name; `Trait &.T` would not parse)
    let trait_part = if param_names.is_empty() {
        quote!()
    } else {
        crate::preprocess::directives::blanket_helpers::trait_with_args(
            trait_full_path,
            &param_names,
        )
    };
    // The `T as Trait<X>` form for assoc-item projections
    let as_trait = if param_names.is_empty() {
        quote!(#t as #trait_full_path)
    } else {
        quote!(#t as #trait_full_path < #(#param_names),*>)
    };

    // By-value receiver methods (`fn consume(self)`): the deref forward
    // moves the inner value out of the wrapper, which only type-checks for
    // Copy-ish targets — and never for `&` wrappers. Warnings have no
    // stable channel (proc_macro_diagnostic is E0658), so the guidance rides
    // a `#[doc]` note on every generated impl: visible in rustdoc / IDE
    // hover, zero compile risk.
    let by_value = method_names
        .iter()
        .filter_map(|name| {
            get_trait_item(trait_def, name).ok().and_then(|item| match item {
                syn::TraitItem::Fn(f)
                    if matches!(
                        f.sig.receiver().map(|r| &r.kind),
                        Some(syn::ReceiverKind::Value | syn::ReceiverKind::Typed(..))
                    ) =>
                {
                    (name.to_string()).into()
                }
                _ => None,
            })
        })
        .collect::<Vec<_>>();
    let doc_note = if by_value.is_empty() {
        quote!()
    } else {
        let names = by_value.join(", ");
        let note = format!(
            "batch-impl: by-value method(s) `{}` forwarded via deref — the forward moves the inner value out of the wrapper, so shared wrappers (`&`, `Rc`) cannot type-check; select `@all_ref_methods` to keep the trait default or hand-write them with `#name{{..}}` if rustc rejects the impl",
            names
        );
        quote!(#[doc = #note])
    };
    let mut spec_streams = vec![];
    for wrapper in &wrappers {
        // Wrapper where predicates: `@0` → target generic name; merged into
        // where (zero-analysis parallel merge)
        let wrapper_preds = match &wrapper.where_preds {
            Some(preds) => {
                crate::preprocess::directives::blanket_helpers::resolve_target_predicates(
                    preds,
                    trait_full_path,
                )?
            }
            None => vec![],
        };
        // Insert predicate streams as wholes (commas between predicates are
        // already in the token streams; cannot connect with per-token commas)
        let mut where_streams = base_preds.clone();
        if wrapper.is_unsized {
            // `Box@?` — the fresh generic is `?Sized` (the `T: Trait` bound
            // would otherwise imply `Sized`); supports unsized targets like
            // `Box<dyn Trait>`.
            where_streams.push(quote!(#t : ?Sized));
        }
        if let Some(wc) = &generics.where_clause {
            let preds = &wc.predicates;
            where_streams.push(quote!(#preds));
        }
        if !wrapper_preds.is_empty() {
            let wrapper_stream: TokenStream = wrapper_preds.into_iter().collect();
            where_streams.push(wrapper_stream);
        }
        let where_part =
            if where_streams.is_empty() { quote!() } else { quote!(where { #(#where_streams),* }) };
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
                    // `Self` in the signature (parameters **or** return)
                    // breaks delegation: the body forwards the inner value
                    // (`(**self).m()` / `t::m()`), whose parameter/return
                    // types are the inner `T`, but the impl's `Self` is the
                    // wrapper — `fn new() -> Self` through `Box` used to emit
                    // `t::new()` and fail with rustc's E0308 at the generated
                    // impl; `fn cmp(&self, other: Self)` has the same problem
                    // in the parameters. Report with guidance.
                    if crate::preprocess::directives::blanket_helpers::sig_refs_bare_self(&f.sig) {
                        return Err(compile_err!(
                            "batch-impl: #blanket method `{}::{}` takes/returns \
                             `Self` (bare or `Self::Assoc` projection); blanket delegation \
                             forwards the inner type, which cannot match the wrapper's \
                             `Self` — write a `#name{{...}}` body for this wrapper instead",
                            trait_def.ident,
                            name
                        ));
                    }
                    let call_args = collect_call_args(&sig).map_err(|pat| {
                        compile_err!(
                            "batch-impl: #blanket method `{}::{}` param `{}` cannot be \
                             forwarded: only `self` and plain identifier patterns are supported",
                            trait_def.ident,
                            name,
                            pat
                        )
                    })?;
                    let body = if f.sig.receiver().is_none() {
                        quote! { #t :: #name ( #(#call_args),* ) }
                    } else {
                        // `&self`/`&mut self` reach the inner through the
                        // reference AND the wrapper layers (`**self` =
                        // depth + 1 derefs); a by-value `self` IS the
                        // wrapper, so one deref fewer (`*self` = depth
                        // derefs — 0.7.2 fix: the extra star dereferenced the
                        // inner type, E0614).
                        let derefs = if matches!(
                            f.sig.receiver().map(|r| &r.kind),
                            Some(syn::ReceiverKind::Value | syn::ReceiverKind::Typed(..))
                        ) {
                            wrapper.depth
                        } else {
                            wrapper.depth + 1
                        };
                        // Build the deref chain structurally (no string
                        // parsing — the no-panic promise): N `*` puncts
                        // followed by `self`, always a valid expression.
                        let stars: TokenStream = std::iter::repeat_n(
                            TokenTree::Punct(proc_macro2::Punct::new(
                                '*',
                                proc_macro2::Spacing::Alone,
                            )),
                            derefs,
                        )
                        .collect();
                        let self_ty: TokenStream = quote!(#stars self);
                        quote! { (#self_ty) . #name ( #(#call_args),* ) }
                    };
                    methods.extend(build_from_item(item, &body));
                }
                // Assoc type/const: projection (not through self)
                syn::TraitItem::Type(t) if !t.generics.params.is_empty() => {
                    // Generic associated type (GAT): project with the GAT's
                    // own params — `type Iter<'a> where Self: 'a` →
                    // `type Iter<'a> where Self: 'a = <T as Trait>::Iter<'a>;`
                    // (the bare projection would be missing the lifetime
                    // argument, E0107).
                    let args = t
                        .generics
                        .params
                        .iter()
                        .map(|p| match p {
                            syn::GenericParam::Lifetime(ld) => quote!(#ld),
                            syn::GenericParam::Type(tp) => quote!(#tp),
                            syn::GenericParam::Const(cp) => quote!(#cp),
                        })
                        .collect::<Vec<_>>();
                    let body = quote! { < #as_trait >::#name < #(#args),* > };
                    methods.extend(build_from_item(item, &body));
                }
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
        // `@0` in the wrapper's main part marks the target position: emit the
        // wrapper as-is and let the parse layer resolve `@0` into the fresh
        // generic name (so `T` can sit anywhere, e.g. `(u32, @0, u8)`).
        // Without `@0` the wrapper is applied as `wrapper.T` (target appended
        // last) — the existing behavior.
        let wrapper_vec: Vec<_> = wrapper_ty.clone().into_iter().collect();
        let target: TokenStream =
            if crate::preprocess::directives::blanket_helpers::has_at0(&wrapper_vec) {
                quote!(#wrapper_ty)
            } else {
                quote!(#wrapper_ty . #t)
            };
        spec_streams.push(quote! {
            #doc_note #impl_generics #trait_part #target #where_part { #methods }
        });
    }
    Ok(quote!(#(#spec_streams),*).into_iter().collect())
}
