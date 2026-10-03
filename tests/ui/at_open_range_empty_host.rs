//! A fresh range with nothing to cover left an empty argument list: with no fresh
//! generics in scope, `Vec<@0..>` became `Vec<>`, and the only complaint came from
//! rustc (E0107) pointing at the type rather than at the attribute. `@0..` is the
//! spelling the changelog recommends in place of the removed `@all_fresh`, so the
//! slip is a realistic one.
use batch_impl::batch_impl;

#[batch_impl(Vec<@0..> #d{"x".to_string()})]
trait D {
    fn d(&self) -> String;
}

fn main() {}
