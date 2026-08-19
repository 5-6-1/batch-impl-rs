//! Type-class style batch impls: a numeric hierarchy (`Num` → `UNum`/`INum`/`FNum`)
//! plus a `From<bool>` instance for a generic fraction type.
//!
//! Showcases four DSL layers in one file:
//! - `@` name families (`@u*`/`@i*`/`@f*`/`@num`) expand to the built-in type lists;
//! - `batch_trait!` segments fill whole classes with one line each;
//! - the splat pow `Frac.*(*@u*).2` feeds one list into both generic positions
//!   (6 × 6 = 36 combos);
//! - a spec-level trait segment with concrete args (`From<bool>`) substitutes
//!   the trait param into directive-copied bodies (`fn from(value: bool)`).
//!
//! Order of the pieces follows the dependency chain: the data type (`Frac`)
//! first, then the base class (`Num`), then the subclasses (`UNum`/`INum`/
//! `FNum`) that build on it.

// The point is generating the impls, not using every trait method — the
// hierarchy exists to be filled by the DSL.
#![allow(dead_code)]

use batch_impl::{batch_impl, batch_impl_only, batch_trait};

// --- the data: a signed fraction ---

/// A signed fraction: `positive` plus `num`/`denom` of two number types.
struct Frac<T, U> {
    positive: bool,
    num: T,
    denom: U,
}

// --- base class `Num`: defined and filled by `#[batch_impl]` ---

// The macro emits the trait definition plus its impls. `@num` fills every
// built-in number; the `<T: UNum, U: UNum>` segment fills the fraction —
// the `UNum` bound is satisfied by the `batch_trait!` impls below (the
// class constraint closes the loop).
#[batch_impl(
    @num #from_bool{i.into()},
    <T: UNum, U: UNum> Frac<T, U> #from_bool{
        Frac {
            positive: true,
            num: T::from_bool(i),
            denom: U::from_bool(true),
        }
    }
)]
trait Num {
    fn from_bool(i: bool) -> Self;
}

// --- subclasses: declared here, filled by `batch_trait!` below ---

/// Unsigned subclass (the `@u*` family).
trait UNum: Num {}
/// Signed/float subclass with a sign query.
trait INum: Num {
    fn positive(&self) -> bool;
}
/// Float subclass.
trait FNum: INum {}

// --- `batch_trait!`: multi-segment, one line per class ---

batch_trait! {
    UNum: @u*;
    INum: [@i*, @f*] {
        fn positive(&self) -> bool { *self >= Self::from_bool(false) }
    },
    <T: UNum, U: UNum> Frac<T, U> {
        fn positive(&self) -> bool { self.positive }
    };
    FNum: @f*
}

// --- `#[batch_impl_only]`: trait generic args + splat pow ---

// `From<bool>` pins the trait's `T` to `bool`, so the copied signature
// `fn from(value: T)` becomes `fn from(value: bool)`; the splat pow
// `Frac.*(*@u*).2` feeds the `@u*` list into both generic positions —
// 6 × 6 = 36 `impl From<bool> for Frac<u8, u8>` ... `Frac<usize, usize>`.
// The dummy trait is discarded (only its signatures are read).
#[batch_impl_only(
    From<bool>
    Frac<*(*@u*).2>
    #from{
        Frac {
            positive: true,
            num: value.into(),
            denom: true.into(),
        }
    }
)]
pub trait From<T>: Sized {
    fn from(value: T) -> Self;
}

fn main() {
    // `From<bool>` — concrete types so inference picks one of the 36 impls
    let f: Frac<u8, u16> = Frac::from(true);
    assert_eq!(f.num, 1u8);
    assert_eq!(f.denom, 1u16);
    assert!(f.positive);

    // `Num::from_bool` on the number classes
    assert_eq!(<u8 as Num>::from_bool(true), 1u8);
    assert_eq!(<i32 as Num>::from_bool(false), 0i32);
    assert_eq!(<f32 as Num>::from_bool(true), 1.0f32);

    // `INum::positive` — the `Frac` instance reads its field, the numeric
    // instances compare against zero
    assert!(<Frac<u8, u16> as INum>::positive(&f));
    assert!(<i32 as INum>::positive(&7i32));
    assert!(!<f32 as INum>::positive(&-1.5f32));
    // `FNum` floats inherit `positive` from `INum` (call via INum)
    assert!(<f64 as INum>::positive(&0.5f64));

    println!("36 From<bool> impls + the Num/INum/FNum hierarchy all work");
}
