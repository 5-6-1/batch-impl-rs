use batch_impl::batch_impl;

// A `where{...}` predicate is spliced into the impl, so the DSL checks the
// final wording itself once every stage has run (the `X<>` fill, the `@`
// resolution, the shape-template slots). A predicate missing its `:` reads as
// a parse error on the whole attribute otherwise, and a splat inside a
// predicate is expanded by no stage (the where clause is token-level all the
// way to the output).
#[batch_impl(u8 where{A B})]
trait WhereNotAPredicate {}

fn main() {}
