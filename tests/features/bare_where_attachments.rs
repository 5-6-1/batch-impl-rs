//! A bare `where` region that ends in a **trailing comma** must keep the spec's attachments: the comma
//! belongs to the predicate list (Rust allows it to end in a comma), not to the attribute's spec list.
//! alga2 hit the regression: `where P, {body}` reported "a bare `{...}` block without an attached type
//! generates no impl" (0.9.6 accepted it), and `where P, impl{(A@..,)} #n{0}` split the spec into a
//! target-less template, which rustc reported as ``expected type, found `{` `` pointing at the whole
//! attribute. Both are locked here, together with the controls that always worked.

use batch_impl::batch_impl;

// A: trailing comma before the body block.
#[batch_impl(
    <T: Clone> TrWhereComma<T> T where
        T: Clone,
    { fn f(&self) -> u8 { 0 } }
)]
trait TrWhereComma<T> {
    fn f(&self) -> u8;
}

#[test]
fn trailing_comma_before_a_body_keeps_the_spec() {
    assert_eq!(1u8.f(), 0);
}

/// The template collects the targets into a variadic segment, so the receiver is `(u8, u16, (P0,),)` —
/// assert by **calling** the generated method on that exact type (a bound assertion would leave the
/// method dead, and the suite denies warnings).
// B: trailing comma before an `impl{...}` template (the template is an attachment, not a spec).
#[batch_impl((u8, u16) ().1..=2 where @0..: Clone,
    impl{(A@..,)} #n{0})]
trait TrWhereTemplate {
    fn n(&self) -> usize;
}

#[test]
fn trailing_comma_before_a_template_keeps_the_template() {
    // Fully qualified: three of these traits generate `n` for the same tuple shape, so a method call
    // would be ambiguous (E0034) — the point here is *which* impl exists, not method resolution.
    assert_eq!(TrWhereTemplate::n(&(1u8, 2u16, (3u32,),)), 0);
    assert_eq!(TrWhereTemplate::n(&(1u8, 2u16, (3u32, 4u32),)), 0);
}

// C: the braced spelling with the same template (never regressed, kept as the pair's control).
#[batch_impl((u8, u16) ().1..=2 where{@0..: Clone} impl{(A@..,)} #n{0})]
trait TrWhereBraced {
    fn n(&self) -> usize;
}

#[test]
fn braced_where_with_the_same_template_agrees() {
    assert_eq!(TrWhereBraced::n(&(1u8, 2u16, (3u32,),)), 0);
}

// D: no trailing comma and no template comma (the shape the earlier tests used).
#[batch_impl((u8, u16) ().1..=2 where @0..: Clone impl{(A@..)} #n{0})]
trait TrWherePlain {
    fn n(&self) -> usize;
}

#[test]
fn plain_bare_where_still_works() {
    assert_eq!(TrWherePlain::n(&(1u8, 2u16, (3u32,),)), 0);
}
