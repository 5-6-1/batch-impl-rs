//! `A<>` / `A<bindings>` preprocessing: copy the trait generics.
//!
//! After directive preprocessing and where rewriting, before DSL parsing,
//! scans the top-level token stream for `Ident` + angle groups (the pairing
//! output of `angle_collect`, with empty or binding-only args) and expands
//! them into an angle-group sequence — same shape as `angle_collect`'s
//! pairing output, so the parse layer need not distinguish the source.

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::analyze::TraitBounds;
use crate::util::scan_stop;

/// Whether the args are "binding-only" (`Item = T, K = U`: every top-level
/// comma segment contains `=`). Only binding-only args allow the
/// `A<bindings>` copy expansion; `A<T, Item=U>` with positional args is
/// ordinary DSL syntax (not expanded; positional args are declared by the
/// user).
fn args_all_bindings(args: &[TokenTree]) -> bool {
    let mut rest = args;
    while let Some(idx) = scan_stop(rest, &[',']) {
        // Segment must contain a top-level `=` (binding)
        if scan_stop(&rest[..idx], &['=']).is_none() {
            return false;
        }
        rest = &rest[idx + 1..];
    }
    scan_stop(rest, &['=']).is_some()
}

/// Renders the formal-param segment of `A<>`: type params use the bound
/// merged by [`TraitBounds`] (inline + where predicates); lifetimes / consts
/// are copied as-is.
fn render_formals(
    trait_def: &ItemTrait, trait_bounds: &TraitBounds,
) -> Vec<TokenStream> {
    let mut formals = vec![];
    for (i, p) in trait_def.generics.params.iter().enumerate() {
        match p {
            syn::GenericParam::Lifetime(_) | syn::GenericParam::Const(_) => {
                formals.push(quote!(#p));
            }
            syn::GenericParam::Type(tp) => {
                let id = &tp.ident;
                match trait_bounds.params.get(i).and_then(|t| t.bound.clone()) {
                    Some(b) => formals.push(quote!(#id: #b)),
                    None => formals.push(quote!(#id)),
                }
            }
        }
    }
    formals
}

/// `A<>` / `A<bindings>` preprocessing: scans the top-level token stream for
/// `Ident` + angle groups (pairing output of `angle_collect`, empty or
/// binding-only args) and expands to `angle-group(formals) Ident
/// angle-group(args + bindings)` — same shape as `angle_collect`'s pairing
/// output; the parse layer need not distinguish the source.
///
/// - Only **top-level** `Ident` + angle groups are handled (`B<A<>>` is
///   nested in a group and not expanded; `A<T, Item=U>` with positional args
///   is ordinary DSL syntax, not expanded);
/// - A trait with no generic params is passthrough (`A<>` is parsed by the
///   DSL as empty args and renders `A`);
/// - Only `#[batch_impl]` / `#[batch_impl_only]` can use it (needs the trait
///   definition to render formals); `batch_trait!` has no trait definition,
///   so `A<>` passthroughs as-is.
pub(crate) fn expand_empty_trait_generics(
    tokens: &[TokenTree], trait_def: &ItemTrait, trait_bounds: &TraitBounds,
) -> Result<Vec<TokenTree>, TokenStream> {
    if trait_def.generics.params.is_empty() {
        return Ok(tokens.to_vec());
    }
    // Pre-render the arg-name list (used as the angle group's args at
    // expansion)
    let arg_names = crate::analyze::generic_param_names(&trait_def.generics);
    let formals = render_formals(trait_def, trait_bounds);
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // `Ident` + angle group (pairing output of `angle_collect`) —
            // expanded only at top level: empty args (`A<>`) or
            // **binding-only args** (`A<Item=T>`) → positional args copy the
            // trait formals, bindings kept as-is; `A<T, Item=U>` with
            // positional args is ordinary DSL syntax (not expanded).
            // `Ident<>` inside a group (nested like `B<A<>>`) is not handled.
            TokenTree::Ident(id) => {
                let group = match tokens.get(i + 1) {
                    Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>] => g,
                    _ => {
                        out.push(tokens[i].clone());
                        i += 1;
                        continue;
                    }
                };
                let args: Vec<TokenTree> = group.stream().into_iter().collect();
                let bindings_only = !args.is_empty() && args_all_bindings(&args);
                if args.is_empty() || bindings_only {
                    // Expand into an angle-group sequence (the pairing-output
                    // shape of `angle_collect`): `angle-group(<'a, T: bounds,
                    // const N>) A angle-group(<'a, T, N, Item = T>)`
                    out.push(
                        proc_macro2::Group::new(
                            delimiter![<>],
                            quote!(#(#formals),*),
                        )
                        .into(),
                    );
                    out.extend(quote!(#id));
                    let args_ts: TokenStream = if args.is_empty() {
                        quote!(#(#arg_names),*)
                    } else {
                        let bind_ts: TokenStream = args.iter().cloned().collect();
                        quote!(#(#arg_names),* , #bind_ts)
                    };
                    out.push(proc_macro2::Group::new(delimiter![<>], args_ts).into());
                    i += 2;
                } else {
                    out.push(tokens[i].clone());
                    i += 1;
                }
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
