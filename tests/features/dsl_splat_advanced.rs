//! dsl.rs advanced splat tests: idempotency, empty splats, trailing commas,
//! lone splat at the slice position, generic-arg splats, left-operand
//! semantics (`*[...]` distribute / `*(...)` append), splat powers, and the
//! one-layer expansion rule.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

struct SplatA;
struct SplatB;
struct SplatC;
struct SplatD;
struct SplatE;
struct Pair<A, B>(A, B);
struct Triple<A, B, C>(A, B, C);

// nested splat is idempotent; empty splat is a no-op
#[batch_impl((*(*[SplatD, SplatE])))]
trait SplatNested {}

#[batch_impl([SplatA, *()])]
trait SplatEmpty {}

#[test]
fn splat_idempotent_and_empty() {
    fn assert_n<T: SplatNested>() {}
    assert_n::<(SplatD, SplatE)>();
    fn assert_e<T: SplatEmpty>() {}
    assert_e::<SplatA>();
}

// trailing-comma splat; empty splat in the middle of a tuple
#[batch_impl((*(SplatA,)))]
trait SplatTrailingComma {}

#[batch_impl((SplatA, *(), SplatB))]
trait SplatMiddleEmpty {}

#[test]
fn splat_trailing_comma_and_middle_empty() {
    fn assert_t<T: SplatTrailingComma>() {}
    assert_t::<(SplatA,)>();
    fn assert_m<T: SplatMiddleEmpty>() {}
    assert_m::<(SplatA, SplatB)>();
}

// `[*(a,b)]` — lone splat at the slice position flattens into a list
// (syntax parity with `(*(a,b))` → `(a,b)`).
#[batch_impl([*(SplatA, SplatB)])]
trait SplatLoneArray {}

#[test]
fn splat_lone_array() {
    fn assert_t<T: SplatLoneArray>() {}
    assert_t::<SplatA>();
    assert_t::<SplatB>();
}

// generic-arg splat: `Pair<*(A, B)>` → `Pair<A, B>` (one impl, multi-arg)
// — distinct from `Pair<[A, B]>` which dispatches.
#[batch_impl(Pair<*(SplatA, SplatB)>)]
trait SplatGenArgs {}

#[batch_impl(Pair<*(SplatA, *(SplatB))>)]
trait SplatGenArgsNested {}

#[test]
fn splat_generic_args() {
    fn assert_t<T: SplatGenArgs>() {}
    assert_t::<Pair<SplatA, SplatB>>();
    fn assert_n<T: SplatGenArgsNested>() {}
    assert_n::<Pair<SplatA, SplatB>>();
}

// Splat rules: R1 `T^*(A,B)` ≡ `T-A-B` (right operand always flattens);
// R2 left semantics by source — `*[...]` distributes `^T` (`*[A^T,B^T]`,
// enabling composition `X^*[A,B]^T` = `X<A^T, B^T>`, one impl), `*(...)`
// appends (`*(A,B,...,T)`, list semantics).
#[batch_impl(Pair^*[Vec, Box]^u16)]
trait SplatRule2 {}

#[batch_impl(Pair^*(Vec<u8>, Box<u8>))]
trait SplatRule1 {}

#[batch_impl((SplatA, SplatB)^*(SplatC, SplatD))]
trait SplatConcat2 {}

#[batch_impl(Triple^*(SplatA, SplatB)^SplatC)]
trait SplatParenAppend {}

#[batch_impl(*(SplatA, SplatB)^SplatC)]
trait SplatParenLeft {}

#[batch_impl(*[Vec, Box]^SplatC)]
trait SplatBracketLeft {}

#[test]
fn splat_rules() {
    fn assert_r2<T: SplatRule2>() {}
    assert_r2::<Pair<Vec<u16>, Box<u16>>>();
    fn assert_r1<T: SplatRule1>() {}
    assert_r1::<Pair<Vec<u8>, Box<u8>>>();
    fn assert_c<T: SplatConcat2>() {}
    assert_c::<(SplatA, SplatB, SplatC, SplatD)>();
    // Source-driven left semantics: `*(...)` appends the operand
    // (list — mirrors TyTuple), `*[...]` distributes it (set — mirrors
    // TyArray).
    fn assert_pa<T: SplatParenAppend>() {}
    assert_pa::<Triple<SplatA, SplatB, SplatC>>();
    fn assert_pl<T: SplatParenLeft>() {}
    assert_pl::<SplatA>();
    assert_pl::<SplatB>();
    assert_pl::<SplatC>();
    fn assert_bl<T: SplatBracketLeft>() {}
    assert_bl::<Vec<SplatC>>();
    assert_bl::<Box<SplatC>>();
}

// `*(A,B)^N` — pow Cartesian combos re-wrap into splats:
// `*(A,B)^2` = `[*(A,A), *(A,B), *(B,A), *(B,B)]`. Each combo is a
// param-position list — a right-splat chain flattens it into the container
// (`A^*(A,B)^2` = `A<A,A>`/`A<A,B>`/...; a lone target flattens to
// duplicates, E0119 — use `(A,B)^2` for tuple impls). `*()^N` (empty
// splat) keeps its splat shape so a carrier appends the fresh params into
// it: `T^*()^2` = `<A,B>T<A,B>` (bare `*()^N` lone target → E0207).
#[batch_impl(Pair^*(SplatA, SplatB)^2)]
trait SplatTuplePow {}

#[batch_impl(Pair^*()^2)]
trait SplatEmptyPowCarrier {}

#[test]
fn splat_pow() {
    // `Pair^*(A,B)^2` — the 4 Cartesian combos flatten into Pair's args.
    fn assert_p<T: SplatTuplePow>() {}
    assert_p::<Pair<SplatA, SplatA>>();
    assert_p::<Pair<SplatA, SplatB>>();
    assert_p::<Pair<SplatB, SplatA>>();
    assert_p::<Pair<SplatB, SplatB>>();
    // `Pair^*()^2` emits `impl<P0, P1> SplatEmptyPowCarrier for
    // Pair<P0, P1>` — the carrier consumes the full fresh declaration.
    fn assert_c<T: SplatEmptyPowCarrier>() {}
    assert_c::<Pair<SplatA, SplatB>>();
}

// Splat expands ONE layer: tuples are types and stay intact — `*((a,b),)`
// is one tuple impl, and `*(a,b,(c,d))` keeps `(c,d)` as a single element.
#[batch_impl(*((SplatA, SplatB)))]
trait SplatTupleKeep {}

#[batch_impl(*(SplatA, SplatB, (SplatC, SplatD)))]
trait SplatTupleKeepList {}

#[batch_impl(*(SplatA, SplatB)^(SplatC, SplatD))]
trait SplatGroupRight {}

// The repeat-list shorthand: `Pair^*(*@u*)^2` = `Pair<@u*, @u*>` — one
// `@u*` written once, Cartesian over both param positions.
#[batch_impl(Pair^*(*@u*)^2)]
trait RepeatList {}

#[test]
fn splat_one_layer() {
    fn assert_k<T: SplatTupleKeep>() {}
    assert_k::<(SplatA, SplatB)>();
    fn assert_kl<T: SplatTupleKeepList>() {}
    assert_kl::<SplatA>();
    assert_kl::<SplatB>();
    assert_kl::<(SplatC, SplatD)>();
    // `^(c,d)` (group right) appends the tuple intact — same shape as
    // writing `*(a,b,(c,d))` directly.
    fn assert_gr<T: SplatGroupRight>() {}
    assert_gr::<SplatA>();
    assert_gr::<SplatB>();
    assert_gr::<(SplatC, SplatD)>();
    // `Pair^*(*@u*)^2` — Cartesian over both positions (spot-check a few).
    fn assert_rl<T: RepeatList>() {}
    assert_rl::<Pair<u8, u8>>();
    assert_rl::<Pair<u8, usize>>();
    assert_rl::<Pair<u128, u32>>();
    assert_rl::<Pair<usize, usize>>();
}
