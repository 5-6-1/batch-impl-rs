// A top-level bare `ident@..` is NOT a variadic segment (those live only
// inside `impl{...}` templates, consumed by mark_varseg): `A@..` reaches
// expand_consts as a malformed open constant range and must report a
// targeted user error — never a panic (regression guard for the canary
// relocation: the mark_template postcondition must not fire here).
use batch_impl::batch_impl;

#[batch_impl(A@..)]
trait T {}
