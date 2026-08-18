//! regression.rs §17-18: macro invocations interacting with the DSL
//! (passthrough of `m![]` bodies / directives / bare where) and the
//! `#path::to::Trait:` external path prefix for `batch_impl_only`.
//! (split from the former single-file `tests/regression.rs`)

use batch_impl::{batch_impl, batch_impl_only};

// ============================================================
// 17. Macro invocations m![] interacting with the DSL
//     - m![] as a target type / generic argument / where predicate passthrough
//     - the m![] macro body is a passthrough macro argument: directives (#name) and bare
//       where must not enter the macro body
// ============================================================
macro_rules! ty {
    () => { Vec<u8> };
}
macro_rules! passthrough {
    ($($t:tt)*) => { $($t)* };
}

#[batch_impl(ty![])]
trait MacroBracketA {}
#[batch_impl(passthrough![Vec<u8>])]
trait MacroBracketB {}
#[batch_impl(Box<ty![]>)]
trait MacroBracketC {}

#[batch_impl(
    <T> MacroBracketFnRet<T> Vec<T> where T: Fn() -> ty![]
    { fn ok(&self) -> bool { true } }
)]
trait MacroBracketFnRet<T> {
    fn ok(&self) -> bool;
}

#[test]
fn macro_bracket_passthrough() {
    fn a<T: MacroBracketA>(_: &T) {}
    fn b<T: MacroBracketB>(_: &T) {}
    fn c<T: MacroBracketC>(_: &T) {}
    a(&vec![1u8]);
    b(&vec![1u8]);
    c(&Box::new(vec![1u8]));
    let v: Vec<fn() -> Vec<u8>> = vec![|| vec![1u8]];
    assert!(v.ok());
}

// --- m![] macro bodies do not expand directives and do not process bare where ---
trait MacroBracketDirective {
    fn len(&self) -> usize;
}

macro_rules! len_ty {
    (#len{ $n:expr }) => {
        u8
    };
}

#[batch_impl_only(
    usize #len{5},
    len_ty![#len{5}] #len{6}
)]
trait MacroBracketDirective {
    fn len(&self) -> usize;
}

#[test]
fn macro_bracket_directive_not_expanded() {
    assert_eq!(0usize.len(), 5);
    assert_eq!(0u8.len(), 6);
}

trait MacroBracketWhere<T> {
    fn ok2(&self) -> bool;
}

macro_rules! m2 {
    (where) => { Vec<u8> };
}

#[batch_impl_only(
    <T> MacroBracketWhere<T> Vec<T> where T: Fn() -> m2![where]
    { fn ok2(&self) -> bool { true } }
)]
trait MacroBracketWhere<T> {
    fn ok2(&self) -> bool;
}

#[test]
fn macro_bracket_where_not_processed() {
    let v: Vec<fn() -> Vec<u8>> = vec![|| vec![1u8]];
    assert!(v.ok2());
}

// ============================================================
// 18. Path prefix `#path::to::Trait:` (batch_impl_only)
//     - the generated impl references a real trait in an external module
//     - the dummy trait is still used to read directive signatures
//     - `Trait<T>` in the DSL is recognized as a trait generic application by the trailing
//       ident of the path
// ============================================================
mod ext {
    pub mod traits {
        pub trait PathPrefixTrait {
            fn tag(&self) -> &'static str;
        }

        pub trait PathPrefixGen<T> {
            fn head(&self) -> T;
        }
    }
}

// the dummy trait is discarded by batch_impl_only; import the real trait here so methods can be called
use ext::traits::{PathPrefixGen, PathPrefixTrait};

#[batch_impl_only(
    #ext::traits::PathPrefixTrait: usize #tag{"usize"}, isize #tag{"isize"}
)]
trait PathPrefixTrait {
    fn tag(&self) -> &'static str;
}

#[test]
fn cmp_path_prefix_directive() {
    assert_eq!(0usize.tag(), "usize");
    assert_eq!(0isize.tag(), "isize");
}

#[batch_impl_only(
    #ext::traits::PathPrefixGen: <T: Clone> PathPrefixGen<T> Vec<T>
    { fn head(&self) -> T { self[0].clone() } }
)]
trait PathPrefixGen<T> {
    fn head(&self) -> T;
}

#[test]
fn cmp_path_prefix_trait_generic() {
    assert_eq!(vec![1i32].head(), 1);
    assert_eq!(vec![String::from("x")].head(), "x");
}

// path prefix + #blanket: the delegated bound must use the full external path
// (a bare dummy name cannot be resolved)
#[batch_impl_only(
    #ext::traits::PathPrefixTrait: u8 #tag{"u8"},
    #blanket(@all){&, Box}
)]
trait PathPrefixTrait {
    fn tag(&self) -> &'static str;
}

#[test]
fn cmp_path_prefix_blanket() {
    // `&u8` matches both u8's own impl and the blanket `&T` impl, requiring UFCS disambiguation
    assert_eq!(<&u8 as PathPrefixTrait>::tag(&&0u8), "u8");
    assert_eq!(Box::new(1u8).tag(), "u8");
}
