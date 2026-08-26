#![allow(clippy::unnecessary_cast)]
//! dsl.rs `@` constant-system tests: built-in name/range families,
//! `batch_trait!` custom constants (lazy expansion, chained refs, values
//! containing `<...>`), and bare-list value forms.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================
// 35. @ constant system: built-in name families / range families / batch_trait! custom
// ============================================================
#[batch_impl(@u8..u128)]
trait UintConst {}

#[batch_impl(@scalar)]
trait ScalarConst {}

#[batch_impl(@num)]
trait NumConst {}

trait ConstA {}
trait ConstB {}
batch_trait!(
    @nums=[u8, u16, u32];
    @uints=@u*;
    ConstA: @nums;
    ConstB: [Box, Rc].@uints;
);

#[test]
fn const_system() {
    fn _u<T: UintConst>(_: &T) {}
    _u(&0u8);
    _u(&0u64);
    _u(&0u128);

    fn _s<T: ScalarConst>(_: &T) {}
    _s(&true);
    _s(&'a');
    _s(&0f64);
    _s(&0usize);
    _s(&0i16);

    fn _n<T: NumConst>(_: &T) {}
    _n(&0f32);
    _n(&0i128);

    fn _a<T: ConstA>(_: &T) {}
    _a(&0u8);
    _a(&0u32);
    fn _b<T: ConstB>(_: &T) {}
    _b(&Box::new(0u8));
    _b(&Rc::new(0usize));
}

// ============================================================
// 38. Review additions: lazy-expansion value forms
//     (values embedding range-family references / bare list values / lists embedding
//     references; check_value_refs endpoint detection uses split_range_endpoint —
//     the bare name `@u8` is not in the built-in name families; definition segments must
//     all precede the trait segments (leading syntax))
// ============================================================
trait RangeVal {}
trait RangeValNested {}
trait BareVal {}
batch_trait!(
    @rv=@u8..u128;
    @nested=[bool, @rv];
    @bare=u8, u32;
    RangeVal: @rv;
    RangeValNested: @nested;
    BareVal: @bare;
);

#[test]
fn lazy_value_forms() {
    fn _r<T: RangeVal>() {}
    _r::<u8>();
    _r::<u64>();
    _r::<u128>();

    fn _n<T: RangeValNested>() {}
    _n::<bool>();
    _n::<u16>();

    fn _b<T: BareVal>() {}
    _b::<u8>();
    _b::<u32>();
}

// ============================================================
// 32. batch_trait! custom @ constant values containing <...> (`@` pairs before `<>`)
//     (0.6.1 fixed the pipeline order `@ <> # where`: previously the @inner of `Vec<@inner>`
//     was paired into the <> group and expand_consts did not enter the group, leaving it
//     behind — an observed compile error)
// ============================================================
trait FooMap {}
trait FooNest {}

batch_trait!(
    @map = HashMap<u32, String>;
    FooMap: @map
);

// Nested: @inner's value contains <...>, @outer references @inner — lazy expansion recursion
batch_trait!(
    @inner = Vec<u8>;
    @outer = Vec<@inner>;
    FooNest: @outer
);

#[test]
fn trait_const_value_with_angles() {
    fn _check_map<T: FooMap>() {}
    fn _check_nest<T: FooNest>() {}
    _check_map::<HashMap<u32, String>>();
    _check_nest::<Vec<Vec<u8>>>();
}

// ------------------------------------------------------------
// Open-ended range families: `@..u32` (family minimum) and
// `@i16..` (family maximum) — either endpoint omittable. The spec
// body is shared by every generated impl.
// ------------------------------------------------------------

#[batch_impl(@..u32 { fn total(&self) -> u128 { *self as u128 } })]
trait Sum {
    fn total(&self) -> u128;
}

#[test]
fn open_left_range_family() {
    fn check<T: Sum>(t: &T) {
        let _ = t.total();
    }
    check(&8u8);
    check(&16u16);
    check(&32u32);
}

#[batch_impl(@i16.. { fn lo(&self) -> i128 { *self as i128 } })]
trait Neg {
    fn lo(&self) -> i128;
}

#[test]
fn open_right_range_family() {
    fn check<T: Neg>(t: &T) {
        let _ = t.lo();
    }
    check(&16i16);
    check(&64i64);
    check(&128i128);
}
