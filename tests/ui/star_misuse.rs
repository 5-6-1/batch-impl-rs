//! `*` misuse — it must be a splat (`*[...]` / `*(...)`) or a raw pointer (`*const`/`*mut`).
use batch_impl::batch_impl;

#[batch_impl(*u8)]
trait StarMisuse {}

fn main() {}
