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
    parse_names_from_tokens,
};
use crate::util::is_single_colon;
use crate::util::{compile_err, compile_err_at, compile_error_str};

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
    let t_bound = if param_names.is_empty() {
        quote!(#trait_full_path)
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#trait_full_path #args_group)
    };
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
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#trait_full_path #args_group)
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
            Some(preds) => resolve_target_predicates(preds, &t, trait_full_path)?,
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
                // Method: deref delegation
                syn::TraitItem::Fn(f) => {
                    let sig = f.sig.clone();
                    let call_args = collect_call_args(&sig).map_err(|pat| {
                        compile_err!(
                            "batch-impl: #blanket method `{}::{}` param `{}` cannot be \
                             forwarded: only `self` and plain identifier patterns are supported",
                            trait_def.ident, name, pat
                        )
                    })?;
                    let body = quote! { (#self_ty) . #name ( #(#call_args),* ) };
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
        spec_streams.push(quote! {
            #impl_generics #trait_part #wrapper_ty ^ #t #where_part { #methods }
        });
    }
    Ok(quote!(#(#spec_streams),*).into_iter().collect())
}

/// Replaces the positional reference `@0` in wrapper where predicates with
/// the target generic name (fresh T), and `@trait` with the full trait path
/// (local name, or the `#ext::Trait:` external path for `batch_impl_only`).
/// `@N` (N>0) out-of-range errors: blanket generates only one target generic.
/// Other tokens after `@` error — wrapper where only accepts positional
/// references and `@trait`.
fn resolve_target_predicates(
    preds: &[TokenTree], t: &TokenStream, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < preds.len() {
        match &preds[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => match preds.get(i + 1) {
                Some(TokenTree::Literal(lit)) if lit.to_string() == "0" => {
                    out.extend(t.clone());
                    i += 2;
                }
                Some(TokenTree::Literal(lit)) => {
                    return Err(compile_err!(
                        "batch-impl: #blanket wrapper where `@{}` out of range \
                         (only `@0` refers to the target generic)",
                        lit
                    ));
                }
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    out.extend(trait_full_path.clone());
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: in #blanket wrapper where, `@` must be \
                         followed by a positional number (e.g. `@0`) or `@trait`",
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

/// A single `#blanket` wrapper element: type expression + deref depth +
/// optional bound predicates.
struct BlanketWrapper {
    /// Wrapper type expression (without the `:N` annotation), applied as-is
    /// to the fresh generic via `^T`.
    ty: TokenStream,
    /// Deref depth of the delegation body (`*` count = depth + 1); `:N`
    /// explicit annotation or default 1.
    depth: usize,
    /// Wrapper bound predicates (group content of the trailing `where{...}`,
    /// `@0` unresolved). Merged into the impl where clause (parallel with
    /// trait generic where predicates, zero-analysis merge).
    where_preds: Option<Vec<TokenTree>>,
}

/// Parses the `#blanket` body wrapper list (`&,Box^Arc:2,Cow<'_>`,
/// comma-separated).
///
/// An element = arbitrary type token stream + optional trailing `:N` depth
/// annotation (Alone `:` + numeric literal; does not clash with the Joint `:`
/// of path `::`). Elements may be nested/prefilled forms (`&Box`, `Box^Arc`,
/// `Cow<'_>`). Three syntax-necessarily-wrong cases are kept as errors:
/// `*const`/`*mut` (safe code cannot deref a raw pointer to delegate), `self`
/// (meaningless), and empty elements / invalid `:N`.
fn parse_blanket_wrappers(
    tokens: &[TokenTree],
) -> Result<Vec<BlanketWrapper>, TokenStream> {
    let mut wrappers = vec![];
    let mut current: Vec<TokenTree> = vec![];
    let flush = |mut current: Vec<TokenTree>,
                 wrappers: &mut Vec<BlanketWrapper>|
     -> Result<(), TokenStream> {
        if current.is_empty() {
            return Err(compile_error_str(
                "batch-impl: #blanket wrapper list contains an empty element \
                 (e.g. `&,Box`); separate elements with `,`",
                proc_macro2::Span::call_site(),
            ));
        }
        // Trailing `where{...}` bound predicates (last part of the element,
        // `@0` refers to the target generic; after `:N`)
        let where_preds = if let Some(TokenTree::Group(g)) = current.last()
            && g.delimiter() == delimiter![{}]
            && let Some(TokenTree::Ident(id)) = current.get(current.len() - 2)
            && id == "where"
        {
            let inner = g.stream().into_iter().collect();
            current.truncate(current.len() - 2);
            Some(inner)
        } else {
            None
        };
        // Trailing `:N` depth annotation (Alone `:`; rules in the doc)
        let mut depth = 1usize;
        let mut ty_end = current.len();
        for i in (0..current.len()).rev() {
            if is_single_colon(&current, i) {
                match &current.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        depth = lit.to_string().parse().map_err(|_| {
                            compile_err!(
                                "batch-impl: #blanket `:{}` has an invalid depth \
                                 (must be a positive integer, e.g. `Box^Arc:2`)",
                                lit
                            )
                        })?;
                        if depth == 0 {
                            return Err(compile_error_str(
                                "batch-impl: #blanket `:0` is meaningless \
                                 (deref depth must be ≥ 1)",
                                lit.span(),
                            ));
                        }
                        ty_end = i;
                    }
                    Some(other) => {
                        return Err(compile_err_at!(
                            other.span(),
                            "batch-impl: after #blanket `:{}` must come a number \
                             (e.g. `Box^Arc:2`)",
                            other
                        ));
                    }
                    None => {}
                }
                break;
            }
        }
        let ty_tokens = &current[..ty_end];
        match ty_tokens {
            [] => Err(compile_error_str(
                "batch-impl: #blanket `:N` is missing the wrapper type before it \
                 (e.g. `Box^Arc:2`)",
                proc_macro2::Span::call_site(),
            )),
            // Built-in wrapper constant: `@Cow` → `Cow<'_>` + inherent bound
            // predicates (deref target = T::Owned, requiring
            // `@0: ToOwned + ?Sized` and `@0::Owned: @trait`; @0/@trait are
            // replaced at resolve time)
            [TokenTree::Punct(at), TokenTree::Ident(name)]
                if at.as_char() == '@' && name == "Cow" =>
            {
                let preds: Vec<TokenTree> =
                    quote!(@0: ToOwned + ?Sized, @0::Owned: @trait)
                        .into_iter()
                        .collect();
                // quote does not pair angle brackets — `Cow<'_>` needs a
                // manually built <> group (blanket output no longer goes
                // through angle_collect; a flat `<` would remain)
                let args = Group::new(delimiter![<>], quote!('_));
                wrappers.push(BlanketWrapper {
                    ty: quote!(Cow #args),
                    depth,
                    where_preds: Some(preds),
                });
                Ok(())
            }
            [TokenTree::Punct(a), TokenTree::Ident(n)]
                if a.as_char() == '*' && (n == "const" || n == "mut") =>
            {
                Err(compile_error_str(
                    "batch-impl: #blanket does not support `*const`/`*mut` \
                     wrappers (deref is unsafe, cannot delegate); write \
                     #delegate by hand",
                    ty_tokens[0].span(),
                ))
            }
            [TokenTree::Ident(id)] if id == "self" => Err(compile_error_str(
                "batch-impl: #blanket does not support `self` wrappers \
                 (delegation is meaningless); write #delegate by hand",
                ty_tokens[0].span(),
            )),
            _ => {
                let ty = ty_tokens.iter().cloned().collect();
                wrappers.push(BlanketWrapper { ty, depth, where_preds });
                Ok(())
            }
        }
    };
    for tt in tokens {
        if let TokenTree::Punct(p) = tt
            && p.as_char() == ','
        {
            flush(current, &mut wrappers)?;
            current = vec![];
        } else {
            current.push(tt.clone());
        }
    }
    flush(current, &mut wrappers)?;
    Ok(wrappers)
}
