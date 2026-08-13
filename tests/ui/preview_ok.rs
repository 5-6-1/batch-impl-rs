use batch_impl::batch_preview;

// The preview reports the expansion through a compile_error! message (the
// only stable terminal channel) — deterministic, snapshot-locked here.
batch_preview! {
    #[batch_impl(usize, isize)]
    trait Pv {}
}

fn main() {}
