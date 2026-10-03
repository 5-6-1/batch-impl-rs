use batch_impl::batch_impl;

// A generator in the generic-declaration position has no carrier for its
// fresh declarations — the `<>` block *is* the impl's parameter list, so the
// freshs would be declared and never used (E0392). Targeted error instead of
// garbage Rust (before the check the `@`-carrying declaration tokens reached
// rustc as "expected type, found `@`").
#[batch_impl(<*[].3> Vec<u8>)]
trait BadDecl {}

fn main() {}
