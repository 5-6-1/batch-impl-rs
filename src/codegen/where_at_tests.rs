//! Where-predicate `@` reference tests (kept under the 350-line cap by
//! living in their own file): `@N` / `@g_i` position references, `@N..M`
//! ranges, group references, and open ranges.

use super::where_at::resolve_where_at;
use crate::analyze::extract_trait_bounds;
use crate::ast::*;
use crate::codegen::generate_impl;
use proc_macro2::{Group, TokenStream, TokenTree};
use quote::quote;
use syn::parse_quote;

fn fresh_names(n: usize) -> Vec<TokenStream> {
    (0..n).map(|i| format!("_Param_0_{}_BatchGen_", i).parse().unwrap()).collect()
}

fn resolve(s: &str, names: &[TokenStream]) -> String {
    let pred: TokenStream = s.parse().unwrap();
    resolve_where_at(&pred, names).unwrap().to_string()
}

#[test]
fn open_range_from_second() {
    // `@1..` open range: every fresh from index 1 to the last one
    let names = fresh_names(4);
    assert_eq!(
        resolve("@1.. : Bound", &names),
        "_Param_0_1_BatchGen_ : Bound , _Param_0_2_BatchGen_ : Bound , \
             _Param_0_3_BatchGen_ : Bound"
    );
}

#[test]
fn open_range_empty_when_past_end() {
    // arity 1: no "from the second element" predicate — the open range
    // truncates to zero instead of erroring (alga2's `@1..` requirement)
    let names = fresh_names(1);
    assert_eq!(resolve("@1.. : Bound", &names), "");
}

#[test]
fn at_ref_inside_group_resolves() {
    // angle_collect pairs `<>` into a None group; `@0` inside is a value
    // reference and must resolve (recursion mirrors resolve_at_refs)
    let names = fresh_names(2);
    let inner: TokenStream = "Scalar = @0 :: Scalar".parse().unwrap();
    let none = Group::new(proc_macro2::Delimiter::None, inner);
    let pred = TokenStream::from(TokenTree::Group(none));
    assert_eq!(
        resolve_where_at(&pred, &names).unwrap().to_string(),
        "Scalar = _Param_0_0_BatchGen_ :: Scalar"
    );
}

#[test]
fn range_tail_value_ref() {
    // the tail after a range subject is scanned for `@N` too (the
    // alga2 scenario: `Scalar = @0::Scalar` inside the bound)
    let names = fresh_names(3);
    let out = resolve("@1.. : Module < Scalar = @0 :: Scalar >", &names);
    assert_eq!(
        out,
        "_Param_0_1_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar > , \
             _Param_0_2_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar >"
    );
}

#[test]
fn closed_range_tail_value_ref() {
    let names = fresh_names(3);
    let out = resolve("@1..=2 : Module < Scalar = @0 :: Scalar >", &names);
    assert_eq!(
        out,
        "_Param_0_1_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar > , \
             _Param_0_2_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar >"
    );
}

/// `WhereArr<>` expansion: impl generics `[T, const N: usize]` (parse-layer name is
/// `const N`; the keyword is needed to render), trait args `[T, N]`, predicate
/// `[T; N]: Sized` referencing N — after normalization the check passes and the
/// expansion has no compile_error (regression guard against IDE/stale false positives)
#[test]
fn const_param_where_predicate_no_error() {
    let trait_def: syn::ItemTrait = parse_quote!(
        trait WhereArr<T, const N: usize>
        where
            [T; N]: Sized,
        {
        }
    );
    let tb = extract_trait_bounds(&trait_def);
    let target = TyTuple(vec![]).to_ty();
    let trait_ty = TyTrait(
        quote!(WhereArr),
        TyTypeParam {
            params: vec![
                (Box::new(TyPrimitive(quote!(T)).to_ty()), None),
                (Box::new(TyPrimitive(quote!(N)).to_ty()), None),
            ],
            bindings: vec![],
        },
    );
    let wrapped = TyWithTrait(trait_ty, target.into());
    let impl_ty = TyWithType(
        TyTypeParam {
            params: vec![
                (Box::new(TyPrimitive(quote!(T)).to_ty()), None),
                (
                    Box::new(TyPrimitive(quote!(const N)).to_ty()),
                    Some(TyPrimitive(quote!(usize)).to_ty()),
                ),
            ],
            bindings: vec![],
        },
        wrapped.into(),
    )
    .into();
    let out = generate_impl(impl_ty, &quote!(WhereArr), false, &tb, &[]).to_string();
    assert!(!out.contains("compile_error"), "expansion must not contain compile_error: {out}");
    assert!(out.contains("where [T ; N] : Sized"), "missing where predicate: {out}");
    assert!(
        out.contains("impl < T , const N : usize > WhereArr < T , N >"),
        "unexpected impl generics: {out}"
    );
}
