//! dsl.rs where-clause tests: the `where{...}` suffix, bare `where predicate
//! {body}` (comma predicates / macro bodies / multiple segments), and the
//! where-attached-to-list modifier form.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;
use std::rc::Rc;

// ============================================================
// 21. `where{...}` DSL suffix
// ============================================================
#[batch_impl(<T: Clone> Sortable<T> Vec<T> where{ T: Ord } {
    fn is_sorted(&self) -> bool {
        self.windows(2).all(|w| w[0] <= w[1])
    }
})]
trait Sortable<T> {
    fn is_sorted(&self) -> bool;
}

#[test]
fn dsl_where_clause() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert!(v.is_sorted());
    let v: Vec<i32> = vec![3, 1, 2];
    assert!(!v.is_sorted());
}

// ============================================================
// 22. `where{...}` suffix form (postfix)
// ============================================================
#[batch_impl(
    <T> Singleton<T> Vec<T> where{ T: Clone + Default }
    { fn only(&self) -> T { self.first().cloned().unwrap_or_default() } }
)]
trait Singleton<T> {
    fn only(&self) -> T;
}

#[test]
fn suffix_where_clause() {
    let v: Vec<i32> = vec![42];
    assert_eq!(v.only(), 42);
    let v: Vec<String> = vec![];
    assert_eq!(v.only(), String::new());
}

// ============================================================
// 24. List modifier + `where{...}` (where attached to the outer Array)
// ============================================================
#[batch_impl(
    <T> WrapOrd<T> [Box, Rc]^Vec<T> where{ T: Ord }
    { fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) } }
)]
trait WrapOrd<T> {
    fn is_sorted(&self) -> bool;
}

#[test]
fn where_with_list_modifier() {
    assert!(WrapOrd::<i32>::is_sorted(&Box::new(vec![1, 2, 3])));
    assert!(!WrapOrd::<i32>::is_sorted(&Rc::new(vec![3, 1, 2])));
}

// ============================================================
// 25. Bare `where predicate {body}` (new syntax; comma predicates are not split by specs)
// ============================================================
#[batch_impl(
    <A> <B> PairComma<A, B> (A, B)
    where A: Clone, B: Clone #both{ (self.0.clone(), self.1.clone()) }
)]
trait PairComma<A, B> {
    fn both(&self) -> (A, B);
}

#[test]
fn where_bare_comma_predicates() {
    let p = (1u32, String::from("x"));
    assert_eq!(PairComma::both(&p), (1u32, String::from("x")));
}

// ============================================================
// 26. Bare where + `m!{}` macro body (a macro invocation is not a body boundary)
// ============================================================
macro_rules! m {
    () => {
        u32
    };
}

#[batch_impl(
    <T> FnRet<T> Vec<T> where T: Fn(u32) -> m!{}
    { fn ret_is_ok(&self) -> bool { true } }
)]
trait FnRet<T> {
    fn ret_is_ok(&self) -> bool;
}

#[test]
fn where_macro_body_excluded() {
    let v: Vec<fn(u32) -> u32> = vec![|x| x + 1];
    assert!(v.ret_is_ok());
}

// ============================================================
// 27. Bare where with multiple segments (`where A where B`) + empty code block
// ============================================================
#[batch_impl(
    <T> MultiOrd<T> Vec<T> where T: Ord where T: Clone {}
)]
trait MultiOrd<T> {}

#[test]
fn where_bare_multi_clause() {
    fn check<T: MultiOrd<i32>>() {}
    check::<Vec<i32>>();
}
