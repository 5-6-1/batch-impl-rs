//! dsl.rs operator tests: the space left-associative apply (the successor of
//! `-`), the bare trait name as the impl trait head, nested generic merging
//! (`<A><B>T`), and operand-strictness legal forms.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl, batch_impl_only};
use std::collections::HashMap;

// ============================================================
// 19. Space operator (left-associative, the successor of `-`)
// ============================================================
#[batch_impl(HashMap u32 String)]
trait DashMapGen {}

#[test]
fn dash_op() {
    fn check<T: DashMapGen>(_: &T) {}
    check(&HashMap::<u32, String>::new());
}

// ============================================================
// Space application semantics (0.8.4, replaces the `-` operator)
// ============================================================
struct SpaceBox<T>(T);
struct Pair<T, U>(T, U);

// `Tr u8` — a bare trait name applies as the impl trait (`impl Tr for u8`)
#[batch_impl(SpaceMark u8 { fn tag(&self) -> &'static str { "u8" } })]
trait SpaceMark {
    fn tag(&self) -> &'static str;
}

// `Tr<A> u8` — the trait head with args still applies to the target
#[batch_impl(SpaceGen<u16> u16 { fn tag(&self) -> &'static str { "u16" } })]
trait SpaceGen<T> {
    fn tag(&self) -> &'static str;
}

// `X <u8>` — the angle group is a separate space unit, so `X` stays a plain
// generic (the type `X<u8>`), not a trait application
#[batch_impl(SpaceBox <u8> { fn bits(&self) -> u32 { 8 } })]
trait SpaceType {
    fn bits(&self) -> u32;
}

// space accumulation: `Pair u8 u16` = `Pair<u8, u16>`
#[batch_impl(Pair u8 u16 { fn pair(&self) -> (u8, u16) { (self.0, self.1) } })]
trait SpaceAcc {
    fn pair(&self) -> (u8, u16);
}

// `fn(A) B` = `fn(A) -> B` (the space fills the return type)
#[batch_impl(fn(u32) String { fn call(&self, x: u32) -> String { format!("{}", x) } })]
trait SpaceFnRet {
    fn call(&self, x: u32) -> String;
}

// `fn(A) -> Box u8` — the arrow's return type is a full space-expression
#[batch_impl(fn(u8) -> SpaceBox u8 { fn w(&self) -> SpaceBox<u8> { SpaceBox(1u8) } })]
trait SpaceFnRetBox {
    fn w(&self) -> SpaceBox<u8>;
}

#[test]
fn space_apply_semantics() {
    fn check<T: SpaceMark>(t: &T) {
        assert_eq!(t.tag(), "u8");
    }
    check(&8u8);

    fn check_t<T: SpaceGen<u16>>(t: &T) {
        assert_eq!(t.tag(), "u16");
    }
    check_t(&16u16);

    fn check2<T: SpaceType>(t: &T) {
        assert_eq!(t.bits(), 8);
    }
    check2(&SpaceBox(8u8));

    fn check3<T: SpaceAcc>(t: &T) {
        assert_eq!(t.pair(), (1u8, 2u16));
    }
    check3(&Pair(1u8, 2u16));

    fn check4<T: SpaceFnRet>(f: &T) {
        assert_eq!(f.call(7), "7");
    }
    let f: fn(u32) -> String = |x| format!("{}", x);
    check4(&f);

    fn check5<T: SpaceFnRetBox>(f: &T) {
        assert_eq!(f.w().0, 1u8);
    }
    let g: fn(u8) -> SpaceBox<u8> = |x| SpaceBox(x);
    check5(&g);
}

// ============================================================
// 20. Nested generic merging <T> Describe<T> [Vec<T>, <U> HashMap<T, U>]
// ============================================================
#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> {
    fn describe(&self) -> String;
}

#[test]
fn nested_generic_list() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.describe(), "len=3");
    let m: HashMap<i32, String> = HashMap::from([(1, String::from("a"))]);
    assert_eq!(m.describe(), "len=1");
}

// ============================================================
// 23. `<A><B>T` merging (apply chain merged into impl<A, B>)
// ============================================================
trait PairAB<A, B> {
    fn pair(&self) -> (A, B);
}

#[batch_impl_only(
    <A> <B> PairAB<A, B> (A, B) where{ A: Clone, B: Clone }
    { fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
)]
trait PairAB<A, B> {
    fn pair(&self) -> (A, B);
}

#[test]
fn nested_generics_merge() {
    let p = (1u32, String::from("x"));
    assert_eq!(p.pair(), (1u32, String::from("x")));
}

// ============================================================
// 31. Empty operand strictness: legal forms are unaffected
//     (trailing commas / empty tuple `()` / empty base `[]` are real tokens, not empty operands)
// ============================================================
// Trailing commas must be guarded with #[rustfmt::skip]: rustfmt removes trailing commas
// from single-line macro invocations, so this case is the regression vehicle for
// "trailing commas are legal"
#[rustfmt::skip]
#[batch_impl(usize, isize,)]
trait TrailingCommaOk {}

#[batch_impl(())]
trait EmptyTupleOk {}

#[batch_impl(usize, isize)]
trait NoTrailingIssue {}

#[test]
fn strictness_legal_forms() {
    fn check<T: TrailingCommaOk>() {}
    check::<usize>();
    check::<isize>();

    fn check2<T: EmptyTupleOk>() {}
    check2::<()>();

    fn check3<T: NoTrailingIssue>() {}
    check3::<isize>();
}
