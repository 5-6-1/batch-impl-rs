// A **constant range** (`@..u128` / `@u8..u128`) inside an `impl{...}` shape
// template is a DSL operator, not a variadic segment — the template must be
// a standard Rust type, so this is a targeted user error, never a panic.
// Regression guard: the mark_template postcondition must not mistake the
// `@` of a range (preceded by `<` / `,` / `(`, not an ident) for an unmarked
// `ident@..` segment (detection used to drop the ident-prefix check).
use batch_impl::batch_impl;

#[batch_impl(Box u8 impl{Vec<@..u128>})]
trait T {}
