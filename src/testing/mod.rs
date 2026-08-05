//! Test infrastructure (compiled only under `cfg(test)`): feeds proptest-generated random
//! tokens into the real macro entry points, promising "no panic on user input".

pub(crate) mod fuzz;
