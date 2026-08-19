//! dsl.rs trait generic-arg tests: concrete trait args substitute into
//! directive-copied bodies (`From<bool>`: `value: T` → `value: bool`), and
//! args pointing at an impl generic.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl, batch_impl_only};

// Trait generic args (`Conv<bool>` in the spec): the trait param in copied
// directive signatures is substituted — `fn conv(value: T)` becomes
// `fn conv(value: bool)` (compiling proves the substitution; a raw `T`
// would be E0425). The real trait is defined here; the batch_impl_only
// item below is only a discarded signature source.
trait Conv<T>: Sized {
    fn conv(_value: T) -> Self;
}

#[batch_impl_only(
    Conv<bool>
    Pair<[*(SplatA).2,*(SplatB).2]>
    #conv{unimplemented!()}
)]
pub trait Conv<T>: Sized {
    fn conv(_value: T) -> Self;
}

struct SplatA;
struct SplatB;
struct Pair<A, B>(A, B);

#[test]
fn trait_generic_args() {
    // Reference the generated method (proves the impl exists with the
    // substituted signature) — never call it (unimplemented! body).
    fn assert_c<T: Conv<bool>>() {
        let _ = <T as Conv<bool>>::conv;
    }
    assert_c::<Pair<SplatA, SplatA>>();
    assert_c::<Pair<SplatB, SplatB>>();
}

// Trait generic args pointing at an impl generic: `<U>GenU<U>()` — the
// generic declaration `<U>` + trait segment `A<U>` + target `()` + a
// directive. The trait param `T` substitutes to `U` (`fn foo(_: T)` →
// `fn foo(_: U)`, referencing the impl generic).
#[batch_impl(<U>GenU<U>() #foo{})]
trait GenU<T> {
    fn foo(_: T);
}

#[test]
fn trait_generic_args_to_impl_generic() {
    fn assert_gu<T: GenU<u8>>() {
        let _ = <T as GenU<u8>>::foo;
    }
    assert_gu::<()>();
}
