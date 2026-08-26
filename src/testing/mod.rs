//! Test infrastructure (compiled only under `cfg(test)`): feeds proptest-generated random
//! tokens into the real macro entry points, promising "no panic on user input".

pub(crate) mod fuzz;

use std::alloc::{GlobalAlloc, Layout, System};

/// The allocation guard for the test build: any single allocation above
/// [`GUARD_LIMIT`] **panics** instead of aborting. An allocation failure
/// aborts the process (unwind-less), which proptest cannot catch — the whole
/// test binary dies without a failing case, and an adversarial input that
/// balloons memory reads as an environment flake. Turning oversized
/// allocations into panics hands them to proptest's normal machinery: the
/// case is shrunk, printed, and persisted to `proptest-regressions/`.
///
/// The limit is far above anything the expansion limits legitimately produce
/// (a full spec is capped at 1024 leaves / 64k body tokens — kilobytes), so
/// a hit is always a bug: an unchecked growth path or an
/// allocate-before-check.
#[allow(unsafe_code)]
pub(crate) struct GuardAlloc;

const GUARD_LIMIT: usize = 256 * 1024 * 1024;

#[allow(unsafe_code)]
unsafe impl GlobalAlloc for GuardAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > GUARD_LIMIT {
            panic!("fuzz alloc guard: {layout:?} exceeds {GUARD_LIMIT} bytes");
        }
        // SAFETY: forwards to the system allocator, contract-preserving.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: pointer was produced by `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > GUARD_LIMIT {
            panic!("fuzz alloc guard: realloc to {new_size} bytes exceeds {GUARD_LIMIT}");
        }
        // SAFETY: pointer was produced by `System.alloc` with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
