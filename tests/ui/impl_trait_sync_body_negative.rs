use batch_impl::batch_impl;

struct Additive;

// Body sync is opt-in via a template that actually carries `Tr<>` — a
// template without it (`impl{(A@..,)}`) leaves the body's `X<>` unsynced
// (surfaces as rustc's E0107).
#[batch_impl(
    BodySync<Additive> ().1..=1 where{@0..: BodySync<>} impl{(A@..,)}
    #SIZE{7}
    #tag{<Self as BodySync<>>::SIZE},
)]
trait BodySync<Oa> {
    const SIZE: u32;
    fn tag(&self) -> u32;
}

fn main() {}
