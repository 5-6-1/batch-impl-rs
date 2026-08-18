use batch_impl::batch_impl;

// A lifetime argument (`'_` of `Cow<'_, A>`) cannot bind a type argument
// (`u8` of the leaf): the parameter classes differ, the verbatim compare
// fails. `Cow<'_, A>` can only match a `Cow<'_, X>`-shaped leaf.
struct Pair2<A, B>(A, B);

#[batch_impl(Pair2<u8, u16> impl{Cow<'_, A>} { fn mk(x: u8) -> A { x } })]
trait BadLifetimeArg {
    fn mk(x: u8) -> u16;
}

fn main() {}
