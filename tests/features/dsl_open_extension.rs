//! dsl.rs §28: the open-extension mechanism — unknown `#name(args){body}`
//! becomes a top-level macro call (`{! m!{...}}` 4-segment protocol), plus
//! the manual top-level / in-impl forms.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl, batch_preprocess_test};

// ============================================================
// 28. Open extension mechanism: user macros expand to items based on the trait
//     `usize #batch_preprocess_test(add,inc){*self+1}` expands to a top-level
//     macro call `{ ! batch_preprocess_test!{(add,inc){*self+1} trait AddInc {...}} }`
//     — the `!` marks top-level emission: the spec body `{usize}` is prepended
//     to the macro input (4 segments) and the macro emits its own impl.
//     The manual in-impl form `T {m!{...}}` (no `!`) keeps the call in the
//     impl body (associated items); `T {! m!{...}}` is the top-level form
//     with a user-written input. batch_preprocess_test! parses
//     method names/body/trait and generates fn definitions (or a full impl
//     in the top-level form), equivalent to handing `#fill` to a user macro
//     (each type can carry its own; the trait is not duplicated)
// ============================================================
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

// Top-level form with a user-written macro call (`{! ...}` attach): the spec
// body `{T}` is prepended — the macro receives 4 segments and emits its own
// impl, same as the `#cmd` form.
#[batch_impl(u16 {! batch_preprocess_test!{(add,inc){*self+3} trait AddIncU16 { fn add(&self) -> Self; fn inc(&self) -> Self; }} })]
trait AddIncU16 {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

// Manual in-impl form: the macro call lands in the impl body (no `!`) — the
// user writes the full input including the trait; the macro emits associated
// fn definitions.
#[batch_impl(u8 { batch_preprocess_test!{(add,inc){*self+2} trait AddIncU8 { fn add(&self) -> Self; fn inc(&self) -> Self; }} })]
trait AddIncU8 {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

#[test]
fn open_extension_fn_like_macro() {
    assert_eq!(5usize.add(), 6);
    assert_eq!(5usize.inc(), 6);
    assert_eq!(5u16.add(), 8);
    assert_eq!(5u16.inc(), 8);
    assert_eq!(5u8.add(), 7);
    assert_eq!(5u8.inc(), 7);
}
