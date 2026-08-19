//! dsl.rs splat core tests: `*[...]` / `*(...)` flattening, trait-path
//! splats, splat powers as generic args, generator args in `<>`, and the
//! main splat scenario matrix.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl, batch_impl_only};

struct SplatA;
struct SplatB;
struct SplatC;
struct SplatD;
struct SplatE;
struct SplatF;
struct Pair<A, B>(A, B);

#[batch_impl([SplatA, *[SplatD, SplatE, SplatF]])]
trait SplatArr {}

#[batch_impl((SplatA, SplatB, SplatC).*(SplatD, SplatE, SplatF))]
trait SplatConcat {}

#[batch_impl((*(().3)))]
trait SplatGen {}

#[batch_impl((SplatA, *(().3)))]
trait SplatGenFlat {}

#[batch_impl(*[Vec, Box].SplatF)]
trait SplatLeft {}

#[batch_impl(Pair.*(SplatD, SplatE))]
trait SplatArgs {}

// Trait segment + right splat: `Conv<bool> Pair.*(A, B)` — the splat stays
// whole through parse/apply and expands only in codegen: `Pair<A, B>` (the
// old behavior misparsed to `Pair<A<B>>`). The real `Conv` is defined
// here; the `#[batch_impl_only]` dummy below is discarded after its
// signatures are read (the former single file had the same pattern).
#[allow(dead_code)] // the generated impls use the trait; the method is never called
pub trait Conv<T>: Sized {
    fn conv(_value: T) -> Self;
}

#[batch_impl_only(Conv<bool> Pair.*(SplatA, SplatB) #conv{unimplemented!()})]
pub trait Conv<T>: Sized {
    fn conv(_value: T) -> Self;
}

// Trait-path splat args: `Conv2<*(A,B)> Pair` — a splat as a trait generic
// arg expands in codegen to its elements: `Conv2<A, B>`.
struct SplatPair2;
#[batch_impl(Conv2<*(SplatA, SplatB)> SplatPair2 #conv2{SplatPair2})]
trait Conv2<T, U>: Sized {
    fn conv2(_value: T, _other: U) -> Self;
}
fn assert_cv2<T: Conv2<SplatA, SplatB>>() {}

#[test]
fn trait_path_splat() {
    assert_cv2::<SplatPair2>();
    let _ = <SplatPair2 as Conv2<SplatA, SplatB>>::conv2(SplatA, SplatB);
}

// Splat power as a generic arg: `Frac<*(*@u*).2>` distributes the pow's
// Cartesian result (`[*(u8,u8), ...]`) into one impl per pair — 36 total.
// The literal `T<[A,B]>` array path is parse-time (`has_array_arg`); pow
// results enter params as a `TyArray` and distribute in `expand`.
struct SplatPow<T, U>(T, U);
#[batch_impl(SplatPow<*(*@u*).2>)]
trait SplatPowArg {}
#[batch_impl(SplatPow<*(@u*).2>)]
trait SplatPowArg2 {}
fn assert_pow<T: SplatPowArg>() {}
fn assert_pow2<T: SplatPowArg2>() {}

#[test]
fn splat_pow_arg() {
    assert_pow::<SplatPow<u8, u8>>();
    assert_pow::<SplatPow<u8, u16>>();
    assert_pow::<SplatPow<usize, usize>>();
    assert_pow2::<SplatPow<u16, u8>>();
    assert_pow2::<SplatPow<usize, u128>>();
}

// Generator args in `<>`: `().2` hoists fresh decls and keeps the tuple as
// one arg — `GenWrap<().2>` = `impl<P0,P1> T for GenWrap<(P0,P1)>`; `*().2`
// flattens instead — `GenPair2<*().2>` = `impl<P0,P1> T for GenPair2<P0,P1>`.
struct GenWrap<X>(X);
struct GenPair2<A, B>(A, B);
#[batch_impl(GenWrap<().2>)]
trait GenTupleArg {}
#[batch_impl(GenPair2<*().2>)]
trait GenSplatArg {}

fn assert_gt<T: GenTupleArg>() {}
fn assert_gs<T: GenSplatArg>() {}

#[test]
fn gen_args_in_angle() {
    assert_gt::<GenWrap<(u8, u16)>>();
    assert_gs::<GenPair2<u8, u16>>();
    let _ = GenWrap((0u8, 0u16));
    let _ = GenPair2(0u8, 0u16);
}

// Generator splats in trait args hoist their fresh declarations into the
// impl generics (`Conv<*().2> X` = `impl<P0,P1> Conv<P0,P1> for X`) —
// the trait-arg position follows the generic-arg rule (0.7.2; previously the
// declaration was dropped and rustc reported E0412 on the fresh names).
struct GenConvPair<A, B>(A, B);
#[batch_impl(GenConv<*().2> GenConvPair<u8, u16>)]
trait GenConv<T, U> {}

// The parenthesized form `*(().3)` behaves like the bare `*().3`.
struct GenTrio<A, B, C>(A, B, C);
#[batch_impl(GenTrio<*(().3)>)]
trait GenSplatArg3 {}

fn assert_gc<T: GenConv<u8, u16>>() {}
fn assert_g3<T: GenSplatArg3>() {}

#[test]
fn gen_splat_trait_args_hoist() {
    assert_gc::<GenConvPair<u8, u16>>();
    assert_g3::<GenTrio<u8, u16, u32>>();
}

// Nested generic-arg splat: `Map<*(K,V)>` — a splat as one generic arg
// expands in codegen to its elements: `Map<K,V>`.
struct SplatMap<K, V>(K, V);
#[batch_impl(SplatMap<*(SplatA, SplatB)>)]
trait SplatGenericArg {}

// Container rule: a group whose content is a lone splat parses as the
// matching container holding the splat as one element — `(*(a,b))` =
// `( *(a,b) )` (tuple), `[*(a,b)]` = `[ *(a,b) ]` (array); the splat element
// expands only in codegen, so the rendered result is `(a, b)` / `[a, b]`.
// `(*(a,b))` ≡ `(*(a,b),)` on one code path.
struct SplatOne<X>(X);
#[batch_impl(SplatOne.(*(SplatA, SplatB)))]
trait SplatTupArg {}
#[batch_impl(SplatOne.(*(SplatA, SplatB),))]
trait SplatTupArgT {}
#[batch_impl((*(SplatA, SplatB)))]
trait SplatTupLone {}
#[batch_impl((*[SplatA, SplatB]))]
trait SplatTupArr {}
#[batch_impl((*()))]
trait SplatTupEmpty {}

// Splat survival: array elements keep their splat until consumption —
// `[*(A),*(B)].2` repeats each element (`[*(A,A),*(B,B)]`), so the splat
// pow drives both generic positions: `Pair.[*(SplatA),*(SplatB)].2` =
// `[Pair<SplatA,SplatA>, Pair<SplatB,SplatB>]`.
#[batch_impl(Pair.[*(SplatA),*(SplatB)].2)]
trait SplatSurvival {}

#[test]
fn splat_scenarios() {
    fn assert_t<T: SplatArr>() {}
    assert_t::<SplatA>();
    assert_t::<SplatD>();
    assert_t::<SplatF>();
    fn assert_c<T: SplatConcat>() {}
    assert_c::<(SplatA, SplatB, SplatC, SplatD, SplatE, SplatF)>();
    fn assert_g<T: SplatGen>() {}
    assert_g::<(u8, u16, u32)>();
    fn assert_gf<T: SplatGenFlat>() {}
    assert_gf::<(SplatA, u8, u16, u32)>();
    fn assert_l<T: SplatLeft>() {}
    assert_l::<Vec<SplatF>>();
    assert_l::<Box<SplatF>>();
    fn assert_args<T: SplatArgs>() {}
    assert_args::<Pair<SplatD, SplatE>>();
    fn assert_cv<T: Conv<bool>>() {}
    assert_cv::<Pair<SplatA, SplatB>>();
    fn assert_ga<T: SplatGenericArg>() {}
    assert_ga::<SplatMap<SplatA, SplatB>>();
    fn assert_tu<T: SplatTupArg>() {}
    assert_tu::<SplatOne<(SplatA, SplatB)>>();
    fn assert_tut<T: SplatTupArgT>() {}
    assert_tut::<SplatOne<(SplatA, SplatB)>>();
    fn assert_tl<T: SplatTupLone>() {}
    assert_tl::<(SplatA, SplatB)>();
    fn assert_tar<T: SplatTupArr>() {}
    assert_tar::<(SplatA, SplatB)>();
    fn assert_te<T: SplatTupEmpty>() {}
    assert_te::<()>();
    fn assert_s<T: SplatSurvival>() {}
    assert_s::<Pair<SplatA, SplatA>>();
    assert_s::<Pair<SplatB, SplatB>>();
}
