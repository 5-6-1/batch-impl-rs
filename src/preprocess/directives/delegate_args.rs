//! Delegate argument forwarding: which `syn::Pat` patterns are safe to
//! forward verbatim into a delegated call, and call-argument collection.

use std::collections::HashSet;

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};

/// Names reserved across the complete input, including later parameters,
/// body tokens and shape slots that codegen can subsequently substitute.
/// Raw and ordinary identifiers share a namespace. Generated names are also
/// reserved so argument bindings and template helpers cannot collide.
#[derive(Clone)]
pub(super) struct ForwardNames {
    used: HashSet<String>,
}

impl ForwardNames {
    pub(super) fn new(scope: &[TokenTree], trait_def: &syn::ItemTrait) -> Self {
        let mut pending = scope.to_vec();
        pending.extend(trait_def.to_token_stream());
        let mut used = HashSet::new();
        while let Some(token) = pending.pop() {
            match token {
                TokenTree::Ident(id) => {
                    let name = id.to_string();
                    used.insert(name.strip_prefix("r#").unwrap_or(&name).to_owned());
                }
                TokenTree::Group(group) => pending.extend(group.stream()),
                _ => {}
            }
        }
        Self { used }
    }

    pub(super) fn fresh(&mut self, base: &str) -> Ident {
        let mut name = base.to_owned();
        while !self.used.insert(name.clone()) {
            name.push('_');
        }
        Ident::new(&name, Span::mixed_site())
    }
}

/// Give pattern-only parameters collision-free bindings before forwarding.
/// The entire original signature is reserved before this mutates any input.
pub(super) fn name_unforwardable_params(sig: &mut syn::Signature, names: &mut ForwardNames) {
    let mut index = 0;
    for input in &mut sig.inputs {
        if let syn::FnArg::Typed(param) = input
            && !pat_is_forwardable(&param.pat)
        {
            *param.pat = syn::Pat::Ident(syn::PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: names.fresh(&format!("arg{index}")),
                subpat: None,
            });
            index += 1;
        }
    }
}

/// Whether a pattern's token stream can be used directly as an expression in
/// a delegation call (`(a, b)` → tuple rebuild, `[x, y]` → array rebuild,
/// `&x` → reference, `Foo { x }` → struct rebuild). `ref x` (`by_ref`),
/// guards / `x @ pat` (`subpat`), `_` and other pattern-only forms cannot —
/// the caller gives them fresh identifier bindings instead. Recurses into compound
/// patterns so `(ref x, y)` is caught, not just a bare `ref x`.
pub(crate) fn pat_is_forwardable(pat: &syn::Pat) -> bool {
    match pat {
        syn::Pat::Ident(pi) => pi.by_ref.is_none() && pi.subpat.is_none(),
        syn::Pat::Tuple(t) => t.elems.iter().all(pat_is_forwardable),
        syn::Pat::Slice(s) => s.elems.iter().all(pat_is_forwardable),
        syn::Pat::Paren(p) => pat_is_forwardable(&p.pat),
        syn::Pat::Reference(r) => pat_is_forwardable(&r.pat),
        syn::Pat::Struct(s) => {
            s.rest.is_none() && s.fields.iter().all(|f| pat_is_forwardable(&f.pat))
        }
        syn::Pat::TupleStruct(ts) => ts.elems.iter().all(pat_is_forwardable),
        // `x: T` type-ascription patterns cannot appear in an expression
        // position (`(x : u32)` is not a valid expression)
        syn::Pat::Type(_) => false,
        // Pattern-only forms: cannot appear in an expression position
        syn::Pat::Wild(_) => false,
        syn::Pat::Or(_) => false,
        syn::Pat::Range(_) => false,
        syn::Pat::Lit(_) => false,
        syn::Pat::Const(_) => false,
        syn::Pat::Path(_) => false,
        syn::Pat::Macro(_) => false,
        syn::Pat::Rest(_) => false,
        syn::Pat::Verbatim(_) => false,
        syn::Pat::Guard(_) => false,
        // syn's `Pat` is `#[non_exhaustive]`: new variants fall back to
        // a fresh identifier binding
        _ => false,
    }
}

/// Collects the argument tokens to forward in a delegation call (skipping
/// the `self` receiver).
///
/// `#delegate` names pattern-only params before calling this. Forwardable
/// parameters keep their original patterns, and their token streams are
/// used directly as an expression in the call — `(a, b)` binds `a`/`b` and
/// `(a, b)` rebuilds the tuple, `[x, y]` rebuilds the array, `x` forwards by
/// name. The `Err` path is a defensive fallback for patterns that cannot be
/// forwarded (its text names the offending pattern for the diagnostic).
pub(crate) fn collect_call_args(sig: &syn::Signature) -> Result<Vec<TokenStream>, String> {
    let mut args = vec![];
    for arg in &sig.inputs {
        match arg {
            syn::FnArg::Receiver(_) => {}
            syn::FnArg::Typed(pat_type) => {
                let pat = &*pat_type.pat;
                match pat {
                    syn::Pat::Ident(pi) => {
                        let id = &pi.ident;
                        args.push(quote!(#id));
                    }
                    // `_` cannot appear after #delegate's naming pass;
                    // anything else keeps its pattern and its token stream
                    // works as an expression (`(a, b)` → tuple rebuild).
                    syn::Pat::Wild(_) => {
                        return Err(quote!(#pat_type).to_string());
                    }
                    _ => {
                        args.push(quote!(#pat));
                    }
                }
            }
        }
    }
    Ok(args)
}
