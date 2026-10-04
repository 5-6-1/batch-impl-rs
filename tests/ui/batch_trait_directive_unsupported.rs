//! `batch_trait!` has no trait definition, so `#` directives are unsupported - and the message now
//! says exactly that and names the directive. Probe D's F2 measured the old outcome: the parse
//! layer's "missing operand after the space application", which names neither the directive nor the
//! documented reason (`src/doc/batch_trait.md:43-49`).
use batch_impl::batch_trait;

batch_trait! { Bm: u8 #fill([m]){0}; }

fn main() {}
