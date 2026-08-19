use batch_impl::batch_preview;

// `Box.Vec-u32` = `Box-Vec-u32` = `Box<Vec, u32>` (the `-` identity) — the
// preview renders the impl and attaches the associativity-miswrite note.
batch_preview! {
    #[batch_impl(Box.Vec-u32)]
    trait Pv2 {}
}

fn main() {}
