//! Delegate argument forwarding: which `syn::Pat` patterns are safe to
//! forward verbatim into a delegated call, and call-argument collection.

use proc_macro2::TokenStream;
use quote::quote;

/// Whether a pattern's token stream can be used directly as an expression in
/// a delegation call (`(a, b)` → tuple rebuild, `[x, y]` → array rebuild,
/// `&x` → reference, `Foo { x }` → struct rebuild). `ref x` (`by_ref`),
/// guards / `x @ pat` (`subpat`), `_` and other pattern-only forms cannot —
/// the caller renames them to `arg{i}` instead. Recurses into compound
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
        // renaming (`arg{i}`), which is always safe
        _ => false,
    }
}

/// Collects the argument tokens to forward in a delegation call (skipping
/// the `self` receiver).
///
/// `#delegate` renames `_` wildcard params to `arg{i}` before calling this.
/// Every other parameter keeps its original pattern, and its token stream is
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
                    // `_` cannot appear (renamed to `arg{i}` by #delegate);
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
