//! dsl.rs macro-meta-layer completion tests: `@trait` / `@Cow` /
//! blanket wrapper where / `[a,b]` args / where style, `@all` status-marker
//! review additions, and the review-fix locks (B1 codegen `@trait`
//! case-sensitivity, B2 `@` inside None groups from macro variables).
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;
use std::borrow::Cow;
use std::rc::Rc;

// ============================================================
// 33. Macro meta-layer completion: @trait / @Cow / blanket wrapper where / [a,b] args / where style
// ============================================================
// @trait: batch_impl expands the local trait name (referenced in blanket wrapper where predicates)
#[batch_impl(#blanket(@all_methods){Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}})]
trait CowWhereTrait {
    fn klen(&self) -> usize;
}
impl CowWhereTrait for str {
    fn klen(&self) -> usize {
        self.len()
    }
}
impl CowWhereTrait for String {
    fn klen(&self) -> usize {
        self.len()
    }
}

// @Cow: built-in constant (Cow<'_> + inherent constraints, deref target = T::Owned)
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowConstTrait {
    fn clen(&self) -> usize;
}
impl CowConstTrait for str {
    fn clen(&self) -> usize {
        self.len()
    }
}
impl CowConstTrait for String {
    fn clen(&self) -> usize {
        self.len()
    }
}

// [a,b] hand-written directive args + @all subtraction -[a,b] exclusion
#[batch_impl(u8 #fill([m1, m2]){1} #fill(@all, -[m1, m2]){3})]
trait BracketArgs {
    fn m1(&self) -> u32;
    fn m2(&self) -> u32;
    fn m3(&self) -> u32;
}

// where style: <> keeps only the names, constraints go in where
#[batch_impl(<T> WhereStyle<T> Vec<T> where{T: Clone} { fn wdup(&self) -> usize { self.len() } })]
trait WhereStyle<T: Clone> {
    fn wdup(&self) -> usize;
}

#[test]
fn macro_meta_complete() {
    let c: Cow<'static, str> = Cow::Borrowed("abc");
    assert_eq!(c.klen(), 3); // @trait predicate (@0::Owned: @trait → T::Owned: CowWhereTrait)
    assert_eq!(c.clen(), 3); // @Cow built-in
    let s: Cow<'static, str> = Cow::Owned("xy".to_string());
    assert_eq!(s.klen(), 2);
    assert_eq!(s.clen(), 2);
    assert_eq!(0u8.m1(), 1); // [m1, m2] filled with 1
    assert_eq!(0u8.m2(), 1);
    assert_eq!(0u8.m3(), 3); // @all -[m1, m2] → m3 filled with 3
    let v = vec![1u32];
    assert_eq!(v.wdup(), 1); // where style
}

// ============================================================
// 35. Review additions: all @all status-marker kinds / marker subtraction / @trait top-level
//     spec / [a,b] delegate args / blanket wrapper where @0 / multi-arg tuple @N
// ============================================================
// @all_required (fn + const kinds) fills only required items; defaults are kept
#[batch_impl(u32 #fill(@all_required){4})]
trait ReqMix2 {
    fn rfn(&self) -> u32;
    fn dfn(&self) -> u32 {
        1
    }
    const RC: u32;
    const DC: u32 = 2;
}

// @all_default_constants: only overrides consts with default values (methods excluded)
#[batch_impl(u64 #fill(@all_default_constants){8})]
trait DefConstOnly {
    fn m(&self) -> u32 {
        3
    }
    const C: u32 = 7;
}

// @all_required_types: only fills required types (trait associated type defaults are a
// nightly feature E0658, so `@all_default_types` is unavailable on stable — const/fn
// defaults are stable)
#[batch_impl(u16 #fill(@all_required_types){u16})]
trait ReqTypesOnly {
    type RT;
}

// Marker subtraction: @all_methods - @all_default_methods = required methods only
#[batch_impl(u8 #fill(@all_methods, -@all_default_methods){1})]
trait MarkerMinus2 {
    fn r1(&self) -> u32;
    fn r2(&self) -> u32;
    fn d1(&self) -> u32 {
        9
    }
}

// @trait top-level expansion: the spec's trait-name part is written as `@trait<T>`
// (lazy expansion consumes 2 tokens; the remaining `<T>` is paired by angle_collect)
#[batch_impl(<T> @trait<T> Vec<T> { fn tl(&self) -> usize { self.len() } })]
trait AtTraitSpec<T> {
    fn tl(&self) -> usize;
}

// [a,b] args in #delegate: Box<Vec<u32>> delegates dl1/dl2
#[batch_impl(
    Vec<u32> {
        fn dl1(&self) -> usize { self.len() }
        fn dl2(&self) -> usize { self.len() }
    },
    Box.Vec.u32 #delegate([dl1, dl2]){**self}
)]
trait DelBr {
    fn dl1(&self) -> usize;
    fn dl2(&self) -> usize;
}

// blanket wrapper where with only @0 (no @trait): `Box where{@0: Copy}`
#[batch_impl(u32 { fn own(&self) -> u32 { *self } })]
#[batch_impl(#blanket(own){Box where{@0: Copy}})]
trait OwnAt0 {
    fn own(&self) -> u32;
}

// @N positional reference: ().3 where{@2: Clone} (fresh generic in the third slot)
#[batch_impl(().3 where{@2: Clone} { fn tk3() -> u32 { 3 } })]
trait TupleWhereAt3 {
    fn tk3() -> u32;
}

#[test]
fn macro_meta_review_extras() {
    assert_eq!(0u32.rfn(), 4);
    assert_eq!(0u32.dfn(), 1); // default kept
    assert_eq!(<u32 as ReqMix2>::RC, 4);
    assert_eq!(<u32 as ReqMix2>::DC, 2); // default kept

    assert_eq!(<u64 as DefConstOnly>::C, 8); // default const overridden
    assert_eq!(0u64.m(), 3); // methods excluded

    fn _check_t<T: ReqTypesOnly>() {}
    _check_t::<u16>();
    let _: <u16 as ReqTypesOnly>::RT = 5u16;

    assert_eq!(0u8.r1(), 1);
    assert_eq!(0u8.r2(), 1);
    assert_eq!(0u8.d1(), 9); // default method kept

    let v = vec![1u32, 2];
    assert_eq!(v.tl(), 2);

    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(b.dl1(), 3);
    assert_eq!(b.dl2(), 3);

    assert_eq!(Box::new(5u32).own(), 5);
    assert_eq!(<(u8, u16, u32) as TupleWhereAt3>::tk3(), 3);
}

// ============================================================
// 36. Review fix lock: B1 (codegen @trait case-sensitivity) + B2 (@ inside None groups from macro variables)
// ============================================================
// B1: @trait in ordinary where predicates (codegen resolve_where_at path —
// previously compared id == "Trait" with a capital, wrongly rejecting @trait)
#[batch_impl(<T> WhereAtTrait<T> Vec<T> where{T: @trait<T>} { fn wn(&self) -> usize { self.len() } })]
trait WhereAtTrait<T: Clone> {
    fn wn(&self) -> usize;
}
impl WhereAtTrait<u32> for u32 {
    fn wn(&self) -> usize {
        1
    }
}

// B2: macro-variable expansion produces real None groups ($($spec)* repeated expansion);
// @u* inside groups must expand
macro_rules! make_impls {
    ($($spec:tt)*) => {
        #[batch_impl($($spec)*)]
        trait MacroGenTrait {
            fn gm(&self) -> u32;
        }
    };
}
make_impls!([Box, Rc].@u* { fn gm(&self) -> u32 { 9 } });

#[test]
fn review_fixes_locked() {
    let v = vec![1u32];
    assert_eq!(v.wn(), 1); // B1: @trait expands correctly in ordinary where
    let b = Box::new(1u32);
    let r = Rc::new(1u32);
    assert_eq!(b.gm(), 9); // B2: @u* expands inside macro-variable None groups
    assert_eq!(r.gm(), 9);
}
